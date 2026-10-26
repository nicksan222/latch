//! SshKey secret strategy.

use super::super::*;

secret_kind! {
    id: "ssh_key",
    ui: (5, "SSH key", Some("privateKey")),
    defaults: serde_json::json!({"privateKey": "", "publicKey": "", "passphrase": null, "comment": ""}),
    wire_secrets: &[("privateKey", false), ("passphrase", true)],
    variant: SshKey, data: SshKeyData, doc: SshKeyDoc,
    diff_fields: &["publicKey", "privateKey", "comment"],
    validate_ui: validate_ui,
    merge: from_input,
    payload: to_payload,
    paths: secret_paths,
    reveal: reveal_field,
    subtitle: subtitle,
    digests: field_digests,
}

fn from_input(d: &SshKeyData, s: Option<&SshKeyDoc>) -> Result<SshKeyDoc> {
    Ok(SshKeyDoc {
        private_key: keep(&d.private_key, s.map(|s| &s.private_key)),
        public_key: d.public_key.trim().to_owned(),
        passphrase: keep_opt(&d.passphrase, s.map(|s| &s.passphrase)),
        comment: d.comment.clone(),
    })
}

fn to_payload(d: &SshKeyDoc, reveal: bool) -> SshKeyData {
    SshKeyData {
        private_key: opt(reveal, &d.private_key),
        public_key: d.public_key.clone(),
        passphrase: if reveal { d.passphrase.clone() } else { None },
        comment: d.comment.clone(),
    }
}

fn secret_paths(d: &SshKeyDoc) -> Vec<String> {
    let mut v = Vec::new();
    v.push("privateKey".into());
    if d.passphrase.is_some() {
        v.push("passphrase".into());
    }
    v
}

fn reveal_field(d: &SshKeyDoc, field: &str) -> Result<Zeroizing<String>> {
    let nf = || Error::NotFound(format!("field `{field}`"));
    let z = |s: &str| Ok(Zeroizing::new(s.to_owned()));
    match field {
        "privateKey" => z(&d.private_key),
        "publicKey" => z(&d.public_key),
        "passphrase" => d.passphrase.as_deref().map_or_else(|| Err(nf()), z),
        _ => Err(nf()),
    }
}

fn subtitle(d: &SshKeyDoc) -> String {
    if d.comment.is_empty() {
        d.public_key
            .split_whitespace()
            .next()
            .unwrap_or_default()
            .to_owned()
    } else {
        d.comment.clone()
    }
}

fn field_digests(d: &SshKeyDoc, put: &mut dyn FnMut(&str, [u8; 32])) {
    put("privateKey", digest(&d.private_key));
    put("publicKey", digest(&d.public_key));
    put("passphrase", digest(&d.passphrase));
    put("comment", digest(&d.comment));
}

fn validate_ui(d: &SshKeyData, errors: &mut BTreeMap<String, String>) {
    #[allow(unused_imports)]
    use crate::core::domain::validation::{issue, keys, urlish};
    if let Some(key) = d.private_key.as_deref().filter(|s| !s.is_empty())
        && !regex::Regex::new(r"-----BEGIN [A-Z ]*PRIVATE KEY-----")
            .expect("static regex")
            .is_match(key)
    {
        issue(errors, "privateKey", "Expected a PEM/OpenSSH private key");
    }
    if !d.public_key.is_empty()
        && !regex::Regex::new(
            r"^(ssh-(ed25519|rsa|dss)|ecdsa-sha2-[a-z0-9-]+|sk-[a-z0-9@.-]+) [A-Za-z0-9+/=]+",
        )
        .expect("static regex")
        .is_match(&d.public_key)
    {
        issue(errors, "publicKey", "Expected an OpenSSH public key");
    }
}
