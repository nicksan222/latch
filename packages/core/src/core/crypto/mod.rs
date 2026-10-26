//! Cryptographic building blocks. No custom constructions: age for
//! encryption, Argon2id for the passphrase, HKDF-SHA256 to turn the vault key
//! into an age X25519 identity, Ed25519 for the manifest signature, BLAKE3 for
//! content hashes.

use std::io::{Read, Write};
use std::str::FromStr;
use std::time::Instant;

use age::x25519;
use base64::Engine;
use base64::engine::general_purpose::STANDARD as B64;
use ed25519_dalek::{Signer, SigningKey, VerifyingKey};
use secrecy::{ExposeSecret, SecretString};
use zeroize::{Zeroize, Zeroizing};

use super::error::{Error, Result};
use crate::protocol::{KdfBenchmark, KdfParams, RecipientKind};

/// Minimum Argon2id memory for production vaults (64 MiB).
pub const MIN_MEMORY_KIB: u32 = 64 * 1024;
/// Spec default: 256 MiB, 3 iterations, 4 lanes.
pub const DEFAULT_KDF: KdfParams = KdfParams {
    memory_kib: 256 * 1024,
    iterations: 3,
    parallelism: 4,
};
/// Cheap parameters for tests and fixtures. Only accepted when weak KDF
/// parameters are allowed (see [`weak_kdf_allowed`]).
pub const WEAK_KDF: KdfParams = KdfParams {
    memory_kib: 256,
    iterations: 1,
    parallelism: 1,
};
/// Upper bounds accepted from a (public, attacker-controllable) config file,
/// so a tampered config cannot make unlock allocate unbounded memory.
const MAX_MEMORY_KIB: u32 = 4 * 1024 * 1024;
const MAX_ITERATIONS: u32 = 64;
const MAX_PARALLELISM: u32 = 64;

pub const ENV_ALLOW_WEAK_KDF: &str = "LATCH_ALLOW_WEAK_KDF";
pub const SALT_LEN: usize = 16;

/// HKDF info string for vault key → age identity. Never change: it is part of
/// the format (FORMAT.md).
pub const VAULT_IDENTITY_INFO: &[u8] = b"latch/v1/vault-key/age-x25519-identity";

/// Weak KDF parameters are allowed under `cfg(test)` or with
/// `LATCH_ALLOW_WEAK_KDF=1`.
pub fn weak_kdf_allowed() -> bool {
    cfg!(test) || std::env::var(ENV_ALLOW_WEAK_KDF).is_ok_and(|v| v == "1")
}

/// Bounds that apply to every vault, including ones we merely open.
pub fn check_kdf_bounds(p: &KdfParams) -> Result<()> {
    if p.memory_kib < 8 * p.parallelism.max(1)
        || p.memory_kib > MAX_MEMORY_KIB
        || p.iterations == 0
        || p.iterations > MAX_ITERATIONS
        || p.parallelism == 0
        || p.parallelism > MAX_PARALLELISM
    {
        return Err(Error::invalid("KDF parameters out of range"));
    }
    Ok(())
}

/// Validate parameters for a new passphrase (init / change passphrase).
pub fn validate_new_kdf(p: &KdfParams) -> Result<()> {
    check_kdf_bounds(p)?;
    if p.memory_kib < MIN_MEMORY_KIB && !weak_kdf_allowed() {
        return Err(Error::invalid(format!(
            "KDF memory must be at least {} KiB",
            MIN_MEMORY_KIB
        )));
    }
    Ok(())
}

pub fn random_bytes<const N: usize>() -> [u8; N] {
    let mut b = [0u8; N];
    getrandom::getrandom(&mut b).expect("OS random number generator unavailable");
    b
}

fn argon2(p: &KdfParams) -> Result<argon2::Argon2<'static>> {
    let params = argon2::Params::new(p.memory_kib, p.iterations, p.parallelism, Some(32))
        .map_err(|_| Error::invalid("KDF parameters out of range"))?;
    Ok(argon2::Argon2::new(
        argon2::Algorithm::Argon2id,
        argon2::Version::V0x13,
        params,
    ))
}

/// Argon2id(passphrase, salt) → 32 bytes → age X25519 identity.
pub fn derive_passphrase_identity(
    passphrase: &SecretString,
    salt: &[u8],
    params: &KdfParams,
) -> Result<x25519::Identity> {
    check_kdf_bounds(params)?;
    let mut out = Zeroizing::new([0u8; 32]);
    argon2(params)?
        .hash_password_into(passphrase.expose_secret().as_bytes(), salt, out.as_mut())
        .map_err(|_| Error::Internal("argon2 failed".into()))?;
    Ok(identity_from_bytes(&out))
}

/// Build an age X25519 identity from 32 secret bytes (bech32 round trip,
/// because age does not expose a raw constructor). The bech32 string is
/// written straight into a pre-sized, zeroized buffer (no intermediate
/// lowercase copy, no reallocation leaving stale copies on the heap).
pub fn identity_from_bytes(bytes: &[u8; 32]) -> x25519::Identity {
    let hrp = bech32::Hrp::parse("age-secret-key-").expect("valid hrp");
    // "AGE-SECRET-KEY-1" (16) + 52 data chars + 6 checksum chars = 74.
    let mut s = Zeroizing::new(String::with_capacity(128));
    bech32::encode_upper_to_fmt::<bech32::Bech32, String>(&mut *s, hrp, bytes)
        .expect("32 bytes always encode");
    debug_assert!(s.len() < 128, "no reallocation");
    x25519::Identity::from_str(&s).expect("valid bech32 identity")
}

/// age public key (`age1…`) of an identity.
pub fn public_key(id: &x25519::Identity) -> String {
    id.to_public().to_string()
}

/// 32-byte secret, zeroized on drop and mlocked on a best-effort basis.
pub struct SecretKey32(Box<[u8; 32]>);

impl SecretKey32 {
    pub fn new(bytes: [u8; 32]) -> Self {
        let mut b = Box::new(bytes);
        let mut src = bytes;
        src.zeroize();
        mlock(b.as_mut_slice());
        SecretKey32(b)
    }
    /// Copy exactly 32 bytes from `bytes` straight into the locked buffer
    /// (no intermediate stack copy). `None` for any other length.
    pub fn from_slice(bytes: &[u8]) -> Option<Self> {
        if bytes.len() != 32 {
            return None;
        }
        let mut b = Box::new([0u8; 32]);
        mlock(b.as_mut_slice());
        b.copy_from_slice(bytes);
        Some(SecretKey32(b))
    }
    pub fn generate() -> Self {
        let mut b = Box::new([0u8; 32]);
        mlock(b.as_mut_slice());
        getrandom::getrandom(b.as_mut_slice()).expect("OS random number generator unavailable");
        SecretKey32(b)
    }
    pub fn expose(&self) -> &[u8; 32] {
        &self.0
    }
}

impl Clone for SecretKey32 {
    fn clone(&self) -> Self {
        Self::from_slice(self.0.as_slice()).expect("32 bytes")
    }
}

impl Drop for SecretKey32 {
    fn drop(&mut self) {
        self.0.zeroize();
        munlock(self.0.as_mut_slice());
    }
}

impl std::fmt::Debug for SecretKey32 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SecretKey32(<redacted>)")
    }
}

/// Vault key → age identity used for every profile, entry, manifest and
/// session archive.
pub fn vault_identity(vault_key: &SecretKey32) -> x25519::Identity {
    let hk = hkdf::Hkdf::<sha2::Sha256>::new(None, vault_key.expose());
    let mut okm = Zeroizing::new([0u8; 32]);
    hk.expand(VAULT_IDENTITY_INFO, okm.as_mut())
        .expect("32 bytes is a valid HKDF output length");
    identity_from_bytes(&okm)
}

// ---------------------------------------------------------------------------
// age encryption
// ---------------------------------------------------------------------------

/// A parsed recipient public key.
pub struct ParsedRecipient {
    pub kind: RecipientKind,
    pub recipient: Box<dyn age::Recipient + Send>,
}

/// Parse `age1…` (X25519), `ssh-ed25519 …`/`ssh-rsa …`, or `age1<plugin>1…`.
/// `kind_hint` lets passphrase/recovery recipients keep their kind.
pub fn parse_recipient(public_key: &str) -> Result<ParsedRecipient> {
    let s = public_key.trim();
    if s.starts_with("ssh-") {
        let r = age::ssh::Recipient::from_str(s)
            .map_err(|_| Error::invalid("unsupported or malformed SSH public key"))?;
        return Ok(ParsedRecipient {
            kind: RecipientKind::Ssh,
            recipient: Box::new(r),
        });
    }
    if let Ok(r) = x25519::Recipient::from_str(s) {
        return Ok(ParsedRecipient {
            kind: RecipientKind::Age,
            recipient: Box::new(r),
        });
    }
    if let Ok(r) = age::plugin::Recipient::from_str(s) {
        let name = r.plugin().to_owned();
        let plugin = age::plugin::RecipientPluginV1::new(&name, &[r], &[], age::NoCallbacks)
            .map_err(|_| Error::invalid(format!("age plugin `age-plugin-{name}` not found")))?;
        return Ok(ParsedRecipient {
            kind: RecipientKind::Plugin,
            recipient: Box::new(plugin),
        });
    }
    Err(Error::invalid(
        "not a recognized age, SSH or plugin public key",
    ))
}

/// Encrypt to a set of recipients.
pub fn encrypt_to(recipients: &[&dyn age::Recipient], plaintext: &[u8]) -> Result<Vec<u8>> {
    let enc = age::Encryptor::with_recipients(recipients.iter().copied())
        .map_err(|e| Error::Internal(format!("age encryption setup failed: {e}")))?;
    let mut out = Vec::with_capacity(plaintext.len() + 256);
    let mut w = enc
        .wrap_output(&mut out)
        .map_err(|e| Error::Internal(format!("age: {e}")))?;
    w.write_all(plaintext)
        .and_then(|_| w.finish().map(|_| ()))
        .map_err(|e| Error::Internal(format!("age: {e}")))?;
    Ok(out)
}

/// Encrypt to a single X25519 identity (the vault identity).
pub fn encrypt_to_identity(id: &x25519::Identity, plaintext: &[u8]) -> Result<Vec<u8>> {
    let r = id.to_public();
    encrypt_to(&[&r as &dyn age::Recipient], plaintext)
}

/// Why a decryption failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecryptFailure {
    /// None of the identities can open the file.
    NoMatchingKey,
    /// Not an age file, or the ciphertext/MAC is invalid.
    Corrupt,
}

/// Decrypt with any of the given identities.
pub fn decrypt_with(
    identities: &[&dyn age::Identity],
    ciphertext: &[u8],
) -> std::result::Result<Zeroizing<Vec<u8>>, DecryptFailure> {
    let dec = age::Decryptor::new_buffered(ciphertext).map_err(|_| DecryptFailure::Corrupt)?;
    let mut r = dec
        .decrypt(identities.iter().copied())
        .map_err(|e| match e {
            age::DecryptError::NoMatchingKeys => DecryptFailure::NoMatchingKey,
            _ => DecryptFailure::Corrupt,
        })?;
    let mut out = Zeroizing::new(Vec::with_capacity(ciphertext.len()));
    r.read_to_end(&mut out)
        .map_err(|_| DecryptFailure::Corrupt)?;
    Ok(out)
}

/// True when `bytes` starts with a parseable age v1 header.
pub fn has_valid_age_header(bytes: &[u8]) -> bool {
    bytes.starts_with(b"age-encryption.org/v1\n") && age::Decryptor::new_buffered(bytes).is_ok()
}

/// Load identities from an age identity file (`AGE-SECRET-KEY-1…`, plugin
/// identities) or an unencrypted OpenSSH private key.
pub fn load_identity_file(path: &std::path::Path) -> Result<Vec<Box<dyn age::Identity>>> {
    let data = Zeroizing::new(
        std::fs::read(path).map_err(|e| Error::io(format!("read {}", path.display()), e))?,
    );
    identities_from_bytes(&data, &path.display().to_string())
}

/// Parse identity file contents (see [`load_identity_file`]).
pub fn identities_from_bytes(data: &[u8], name: &str) -> Result<Vec<Box<dyn age::Identity>>> {
    if let Ok(f) = age::IdentityFile::from_buffer(data)
        && let Ok(ids) = f.into_identities()
        && !ids.is_empty()
    {
        return Ok(ids);
    }
    match age::ssh::Identity::from_buffer(data, Some(name.to_owned())) {
        Ok(age::ssh::Identity::Unencrypted(k)) => {
            let id = age::ssh::Identity::Unencrypted(k);
            Ok(vec![Box::new(id.with_callbacks(age::NoCallbacks))])
        }
        Ok(age::ssh::Identity::Encrypted(_)) => Err(Error::invalid(
            "passphrase-protected SSH keys are not supported as identity files",
        )),
        _ => Err(Error::invalid("not an age or SSH identity file")),
    }
}

// ---------------------------------------------------------------------------
// Signing and hashing
// ---------------------------------------------------------------------------

pub fn generate_signing_key() -> SigningKey {
    let seed = SecretKey32::generate();
    SigningKey::from_bytes(seed.expose())
}

pub fn encode_verifying_key(k: &VerifyingKey) -> String {
    format!("ed25519:{}", B64.encode(k.to_bytes()))
}

pub fn decode_verifying_key(s: &str) -> Result<VerifyingKey> {
    let b = s
        .trim()
        .strip_prefix("ed25519:")
        .and_then(|b| B64.decode(b).ok())
        .and_then(|b| <[u8; 32]>::try_from(b).ok())
        .ok_or_else(|| Error::integrity("malformed signing public key in config.toml"))?;
    VerifyingKey::from_bytes(&b).map_err(|_| Error::integrity("invalid signing public key"))
}

/// `ed25519:<base64 signature>`
pub fn sign(key: &SigningKey, msg: &[u8]) -> String {
    format!("ed25519:{}", B64.encode(key.sign(msg).to_bytes()))
}

pub fn verify(key: &VerifyingKey, msg: &[u8], sig: &str) -> bool {
    let Some(bytes) = sig
        .trim()
        .strip_prefix("ed25519:")
        .and_then(|b| B64.decode(b).ok())
        .and_then(|b| <[u8; 64]>::try_from(b).ok())
    else {
        return false;
    };
    key.verify_strict(msg, &ed25519_dalek::Signature::from_bytes(&bytes))
        .is_ok()
}

/// `blake3:<hex>`
pub fn hash(bytes: &[u8]) -> String {
    format!("blake3:{}", blake3::hash(bytes).to_hex())
}

pub fn b64(bytes: &[u8]) -> String {
    B64.encode(bytes)
}

pub fn b64_decode(s: &str) -> Option<Vec<u8>> {
    B64.decode(s.trim()).ok()
}

// ---------------------------------------------------------------------------
// KDF calibration
// ---------------------------------------------------------------------------

fn time_kdf(p: &KdfParams) -> Result<u32> {
    let salt = random_bytes::<SALT_LEN>();
    let mut out = Zeroizing::new([0u8; 32]);
    let start = Instant::now();
    argon2(p)?
        .hash_password_into(b"latch-benchmark", &salt, out.as_mut())
        .map_err(|_| Error::Internal("argon2 failed".into()))?;
    Ok(start.elapsed().as_millis().min(u32::MAX as u128) as u32)
}

/// Pick Argon2id parameters that take about `target_ms` on this machine.
/// Never goes below 64 MiB; grows memory up to 1 GiB, then iterations.
pub fn calibrate_kdf(target_ms: u32) -> Result<KdfBenchmark> {
    let target = target_ms.clamp(50, 10_000);
    let mut p = KdfParams {
        memory_kib: MIN_MEMORY_KIB,
        iterations: 1,
        parallelism: 4,
    };
    let mut ms = time_kdf(&p)?.max(1);
    while ms * 2 <= target && p.memory_kib < 1024 * 1024 {
        p.memory_kib *= 2;
        ms = time_kdf(&p)?.max(1);
    }
    if ms < target {
        let iters = (target / ms).clamp(1, 16);
        if iters > 1 {
            p.iterations = iters;
            ms = time_kdf(&p)?;
        }
    }
    Ok(KdfBenchmark {
        params: p,
        measured_ms: ms,
    })
}

// ---------------------------------------------------------------------------
// Process hardening
// ---------------------------------------------------------------------------

/// What [`harden_process`] achieved (all best effort).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HardeningReport {
    /// `RLIMIT_CORE` set to 0.
    pub core_dumps_disabled: bool,
    /// `PR_SET_DUMPABLE 0` (Linux): no core dumps, no same-user ptrace.
    pub not_dumpable: bool,
    /// `mlockall(MCL_CURRENT | MCL_FUTURE)` succeeded: no page of this
    /// process (keys, decrypted entries, Argon2 memory) can be swapped out.
    pub memory_locked: bool,
}

static HARDENING: std::sync::OnceLock<HardeningReport> = std::sync::OnceLock::new();

/// Result of the [`harden_process`] call in this process, if any (status
/// and doctor warn when memory could not be locked).
pub fn hardening_status() -> Option<HardeningReport> {
    HARDENING.get().copied()
}

/// Disable core dumps and ptrace-style dumping and lock all memory (best
/// effort). Call once at process start in the agent/CLI.
pub fn harden_process() {
    let _ = harden_process_report();
}

/// [`harden_process`], returning what succeeded. Never fails: every step is
/// best effort. `mlockall` is only attempted when `RLIMIT_MEMLOCK` is large
/// enough (unlimited or ≥ 4 GiB): with `MCL_FUTURE` and a small limit later
/// allocations (Argon2 needs up to 1 GiB) would fail.
pub fn harden_process_report() -> HardeningReport {
    if let Some(r) = HARDENING.get() {
        return *r;
    }
    #[allow(unused_mut)]
    let mut r = HardeningReport {
        core_dumps_disabled: false,
        not_dumpable: false,
        memory_locked: false,
    };
    #[cfg(unix)]
    {
        let rl = libc::rlimit {
            rlim_cur: 0,
            rlim_max: 0,
        };
        // SAFETY: setrlimit with a valid pointer to an initialized struct.
        r.core_dumps_disabled = unsafe { libc::setrlimit(libc::RLIMIT_CORE, &rl) } == 0;
    }
    #[cfg(target_os = "linux")]
    {
        // SAFETY: PR_SET_DUMPABLE takes a plain integer argument.
        r.not_dumpable = unsafe { libc::prctl(libc::PR_SET_DUMPABLE, 0, 0, 0, 0) } == 0;
        let mut lim = libc::rlimit {
            rlim_cur: 0,
            rlim_max: 0,
        };
        // SAFETY: getrlimit writes into a valid, initialized struct.
        let ok = unsafe { libc::getrlimit(libc::RLIMIT_MEMLOCK, &mut lim) } == 0;
        let enough =
            ok && (lim.rlim_cur == libc::RLIM_INFINITY || lim.rlim_cur >= 4 * 1024 * 1024 * 1024);
        if enough {
            // SAFETY: mlockall takes flags only; failure leaves state unchanged.
            r.memory_locked =
                unsafe { libc::mlockall(libc::MCL_CURRENT | libc::MCL_FUTURE | libc::MCL_ONFAULT) }
                    == 0;
        }
    }
    *HARDENING.get_or_init(|| r)
}

/// Best-effort mlock of a buffer (ignored when RLIMIT_MEMLOCK forbids it).
pub fn mlock(buf: &mut [u8]) {
    #[cfg(unix)]
    if !buf.is_empty() {
        // SAFETY: the pointer/length describe a live, owned allocation.
        unsafe {
            libc::mlock(buf.as_ptr() as *const libc::c_void, buf.len());
        }
    }
    #[cfg(not(unix))]
    let _ = buf;
}

pub fn munlock(buf: &mut [u8]) {
    #[cfg(unix)]
    if !buf.is_empty() {
        // SAFETY: as above.
        unsafe {
            libc::munlock(buf.as_ptr() as *const libc::c_void, buf.len());
        }
    }
    #[cfg(not(unix))]
    let _ = buf;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pw(s: &str) -> SecretString {
        SecretString::from(s.to_owned())
    }

    #[test]
    fn passphrase_identity_deterministic() {
        let salt = [7u8; 16];
        let a = derive_passphrase_identity(&pw("hunter2"), &salt, &WEAK_KDF).unwrap();
        let b = derive_passphrase_identity(&pw("hunter2"), &salt, &WEAK_KDF).unwrap();
        let c = derive_passphrase_identity(&pw("hunter3"), &salt, &WEAK_KDF).unwrap();
        assert_eq!(public_key(&a), public_key(&b));
        assert_ne!(public_key(&a), public_key(&c));
        assert!(
            a.to_string()
                .expose_secret()
                .starts_with("AGE-SECRET-KEY-1")
        );
        assert!(public_key(&a).starts_with("age1"));
    }

    #[test]
    fn identity_encoding_is_unchanged() {
        // The zeroized in-place encoding must produce exactly the identity
        // the plain bech32 string did (format compatibility).
        let bytes = [42u8; 32];
        let reference = bech32::encode::<bech32::Bech32>(
            bech32::Hrp::parse("age-secret-key-").unwrap(),
            &bytes,
        )
        .unwrap()
        .to_ascii_uppercase();
        let id = identity_from_bytes(&bytes);
        assert_eq!(id.to_string().expose_secret(), reference.as_str());
    }

    #[test]
    fn identity_bytes_roundtrip() {
        let id = identity_from_bytes(&[42u8; 32]);
        let s = id.to_string();
        let back = x25519::Identity::from_str(s.expose_secret()).unwrap();
        assert_eq!(public_key(&id), public_key(&back));
    }

    #[test]
    fn vault_identity_stable() {
        let k = SecretKey32::new([1u8; 32]);
        assert_eq!(
            public_key(&vault_identity(&k)),
            public_key(&vault_identity(&k.clone()))
        );
        let k2 = SecretKey32::new([2u8; 32]);
        assert_ne!(
            public_key(&vault_identity(&k)),
            public_key(&vault_identity(&k2))
        );
        assert_eq!(format!("{k:?}"), "SecretKey32(<redacted>)");
    }

    #[test]
    fn encrypt_decrypt() {
        let id = x25519::Identity::generate();
        let other = x25519::Identity::generate();
        let ct = encrypt_to_identity(&id, b"hello").unwrap();
        assert!(has_valid_age_header(&ct));
        let pt = decrypt_with(&[&id as &dyn age::Identity], &ct).unwrap();
        assert_eq!(&pt[..], b"hello");
        assert_eq!(
            decrypt_with(&[&other as &dyn age::Identity], &ct).unwrap_err(),
            DecryptFailure::NoMatchingKey
        );
        // Multiple identities: any matching one works.
        let pt = decrypt_with(&[&other as &dyn age::Identity, &id], &ct).unwrap();
        assert_eq!(&pt[..], b"hello");
        let mut bad = ct.clone();
        let n = bad.len();
        bad[n - 3] ^= 1;
        assert_eq!(
            decrypt_with(&[&id as &dyn age::Identity], &bad).unwrap_err(),
            DecryptFailure::Corrupt
        );
        assert_eq!(
            decrypt_with(&[&id as &dyn age::Identity], b"plain").unwrap_err(),
            DecryptFailure::Corrupt
        );
        assert!(!has_valid_age_header(b"plain"));
    }

    #[test]
    fn multi_recipient() {
        let a = x25519::Identity::generate();
        let b = x25519::Identity::generate();
        let ra = parse_recipient(&public_key(&a)).unwrap();
        let rb = parse_recipient(&public_key(&b)).unwrap();
        assert_eq!(ra.kind, RecipientKind::Age);
        let ct = encrypt_to(&[ra.recipient.as_ref(), rb.recipient.as_ref()], b"k").unwrap();
        assert!(decrypt_with(&[&a as &dyn age::Identity], &ct).is_ok());
        assert!(decrypt_with(&[&b as &dyn age::Identity], &ct).is_ok());
    }

    #[test]
    fn parse_recipients() {
        assert!(parse_recipient("nonsense").is_err());
        let ssh = "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIHsKLqeplhpW+uObz5dvMgjz1OxfM/XXUB+VHtZ6isGN alice@rust";
        assert_eq!(parse_recipient(ssh).unwrap().kind, RecipientKind::Ssh);
        // A plugin recipient whose plugin binary is absent is rejected cleanly.
        let fake = bech32::encode::<bech32::Bech32>(
            bech32::Hrp::parse("age1latchnosuchplugin").unwrap(),
            &[1u8; 32],
        )
        .unwrap();
        assert!(parse_recipient(&fake).is_err());
    }

    #[test]
    fn identity_file() {
        let t = tempfile::tempdir().unwrap();
        let id = x25519::Identity::generate();
        let p = t.path().join("key.txt");
        std::fs::write(
            &p,
            format!("# comment\n{}\n", id.to_string().expose_secret()),
        )
        .unwrap();
        let ids = load_identity_file(&p).unwrap();
        let ct = encrypt_to_identity(&id, b"x").unwrap();
        let refs: Vec<&dyn age::Identity> = ids.iter().map(|b| b.as_ref()).collect();
        assert!(decrypt_with(&refs, &ct).is_ok());
        std::fs::write(&p, "garbage").unwrap();
        assert!(load_identity_file(&p).is_err());
        assert!(load_identity_file(&t.path().join("missing")).is_err());
    }

    #[test]
    fn signatures() {
        let k = generate_signing_key();
        let vk = k.verifying_key();
        let enc = encode_verifying_key(&vk);
        let vk2 = decode_verifying_key(&enc).unwrap();
        let sig = sign(&k, b"msg");
        assert!(verify(&vk2, b"msg", &sig));
        assert!(!verify(&vk2, b"msg2", &sig));
        assert!(!verify(&vk2, b"msg", "ed25519:AAAA"));
        assert!(!verify(&vk2, b"msg", "garbage"));
        let other = generate_signing_key();
        assert!(!verify(&other.verifying_key(), b"msg", &sig));
        assert!(decode_verifying_key("ed25519:zz").is_err());
    }

    #[test]
    fn hashing() {
        assert_eq!(hash(b"a"), hash(b"a"));
        assert_ne!(hash(b"a"), hash(b"b"));
        assert!(hash(b"a").starts_with("blake3:"));
        assert_eq!(hash(b"").len(), "blake3:".len() + 64);
    }

    #[test]
    fn kdf_validation() {
        assert!(validate_new_kdf(&DEFAULT_KDF).is_ok());
        // cfg(test) allows weak params.
        assert!(validate_new_kdf(&WEAK_KDF).is_ok());
        assert!(
            check_kdf_bounds(&KdfParams {
                memory_kib: 64,
                iterations: 0,
                parallelism: 1
            })
            .is_err()
        );
        assert!(
            check_kdf_bounds(&KdfParams {
                memory_kib: u32::MAX,
                iterations: 1,
                parallelism: 1
            })
            .is_err()
        );
    }

    #[test]
    fn calibration_respects_minimum() {
        let b = calibrate_kdf(50).unwrap();
        assert!(b.params.memory_kib >= MIN_MEMORY_KIB);
        assert!(b.params.iterations >= 1);
    }

    #[test]
    fn harden_is_safe() {
        // Do not call harden_process() here: it would mlockall/undump the
        // whole test binary. Only the per-buffer helpers are exercised.
        assert!(SecretKey32::from_slice(&[1u8; 31]).is_none());
        assert_eq!(
            SecretKey32::from_slice(&[1u8; 32]).unwrap().expose(),
            &[1u8; 32]
        );
        let mut buf = [0u8; 64];
        mlock(&mut buf);
        munlock(&mut buf);
    }
}
