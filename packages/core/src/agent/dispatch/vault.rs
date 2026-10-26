//! Vault lifecycle and trust handlers behind the shared RPC dispatcher.

use super::{Agent, problem_strings, state};
use crate::core::vault::uninitialized_status;
use crate::core::{Error, Result, UnlockedVault, Vault, crypto};
use crate::protocol::{
    AgentEventKind, DoctorParams, DoctorReport, ForgetTrustParams, InitParams, InitResult,
    TrustStatus, UnlockParams, VaultStatus, VaultTrustState,
};

/// Longest per-session auto-lock override (`vault.unlock` `timeoutSecs`).
const MAX_UNLOCK_TIMEOUT_SECS: u64 = 7 * 24 * 3600;

fn trust_wire(t: &crate::core::TrustInfo) -> TrustStatus {
    TrustStatus {
        state: match t.state {
            crate::core::TrustState::FirstUse => VaultTrustState::FirstUse,
            crate::core::TrustState::Pinned => VaultTrustState::Pinned,
            crate::core::TrustState::Stale => VaultTrustState::Stale,
        },
        fingerprint: t.fingerprint.clone(),
        key_seq: t.key_seq,
    }
}

impl Agent {
    // ----- vault lifecycle --------------------------------------------------

    pub(crate) fn vault_status(&self) -> VaultStatus {
        let memory_locked = crypto::hardening_status().map(|h| h.memory_locked);
        let s = self.slot.read().unwrap_or_else(|p| p.into_inner());
        if let Some(v) = &s.unlocked {
            let mut st = v.status();
            st.locks_in_secs = self.locks_in_secs();
            st.trust = Some(trust_wire(&v.trust_info()));
            st.integrity_problems = problem_strings(v);
            st.rotation_pending = v.rotation_pending();
            st.memory_locked = memory_locked;
            return st;
        }
        let mut st = match &s.path {
            None => uninitialized_status(None),
            Some(p) => match Vault::open(p) {
                Ok(v) => {
                    let mut st = v.status();
                    st.trust = v.trust_info().ok().map(|t| trust_wire(&t));
                    st
                }
                Err(_) => uninitialized_status(Some(p)),
            },
        };
        st.memory_locked = memory_locked;
        st
    }

    /// `vault.trust`: from the unlocked handle, else the locked vault.
    pub(super) fn trust_status(&self) -> Result<TrustStatus> {
        let s = self.slot.read().unwrap_or_else(|p| p.into_inner());
        if let Some(v) = &s.unlocked {
            return Ok(trust_wire(&v.trust_info()));
        }
        let p = s
            .path
            .clone()
            .ok_or_else(|| Self::no_vault_or_locked(&None))?;
        drop(s);
        Ok(trust_wire(&Vault::open(p)?.trust_info()?))
    }

    /// `vault.forgetTrust`: locked state required unless `confirm` (then the
    /// vault is locked first, so no handle keeps using the old trust).
    pub(super) fn forget_trust(&self, fp: ForgetTrustParams) -> Result<TrustStatus> {
        if self.is_unlocked() {
            if !fp.confirm {
                return Err(Error::Conflict(
                    "lock the vault before resetting trust (or pass confirm)".into(),
                ));
            }
            self.lock();
        }
        let path = self
            .slot
            .read()
            .unwrap_or_else(|p| p.into_inner())
            .path
            .clone()
            .ok_or_else(|| Self::no_vault_or_locked(&None))?;
        let v = Vault::open(path)?;
        v.forget_trust()?;
        self.emit(AgentEventKind::VaultChanged, None, None, None);
        Ok(trust_wire(&v.trust_info()?))
    }

    fn register(&self, path: &std::path::Path, vault_id: &str) -> Result<()> {
        let mut vf = state::VaultsFile::load(&self.config_dir);
        vf.activate(path, vault_id);
        vf.save(&self.config_dir)
    }

    /// Make `v` the active vault (locked).
    pub(super) fn activate(&self, v: Vault) -> Result<VaultStatus> {
        let same = {
            let s = self.slot.read().unwrap_or_else(|p| p.into_inner());
            s.path.as_deref() == Some(v.path())
        };
        if !same {
            self.lock();
            self.slot.write().unwrap_or_else(|p| p.into_inner()).path = Some(v.path().to_owned());
        }
        self.register(v.path(), v.id())?;
        self.emit(AgentEventKind::VaultChanged, None, None, None);
        Ok(self.vault_status())
    }

    pub(super) fn apply_git_identity(&self, v: &UnlockedVault) {
        let s = self.settings_snapshot();
        if let (Some(n), Some(e)) = (&s.git_name, &s.git_email)
            && !n.trim().is_empty()
            && !e.trim().is_empty()
            && v.git().is_repo()
        {
            let _ = v.git().set_identity(n.trim(), e.trim());
        }
    }

    pub(super) fn vault_init(&self, mut ip: InitParams) -> Result<InitResult> {
        let s = self.settings_snapshot();
        if ip.git_name.is_none() {
            ip.git_name = s.git_name.clone();
        }
        if ip.git_email.is_none() {
            ip.git_email = s.git_email.clone();
        }
        let (uv, res) = Vault::init(&ip)?;
        self.lock();
        self.register(uv.path(), uv.id())?;
        {
            let mut slot = self.slot.write().unwrap_or_else(|p| p.into_inner());
            slot.path = Some(uv.path().to_owned());
            slot.unlocked = Some(uv);
            self.unlocked
                .store(true, std::sync::atomic::Ordering::SeqCst);
        }
        self.set_unlocked(None);
        self.emit(AgentEventKind::VaultChanged, None, None, None);
        self.emit(AgentEventKind::Unlocked, None, None, None);
        let mut res = res;
        res.status = self.vault_status();
        Ok(res)
    }

    pub(super) fn vault_unlock(&self, up: UnlockParams) -> Result<VaultStatus> {
        if let Some(t) = up.timeout_secs
            && (t == 0 || t > MAX_UNLOCK_TIMEOUT_SECS)
        {
            return Err(Error::InvalidParams(
                "timeoutSecs must be between 1 s and 7 days".into(),
            ));
        }
        // Lifecycle generation at the start: a lock, shutdown or vault
        // switch while the KDF runs cancels this unlock.
        let generation = self.generation.load(std::sync::atomic::Ordering::SeqCst);
        let path = {
            let s = self.slot.read().unwrap_or_else(|p| p.into_inner());
            if s.unlocked.is_some() {
                drop(s);
                self.set_timeout_override(up.timeout_secs);
                return Ok(self.vault_status());
            }
            s.path
                .clone()
                .ok_or_else(|| Self::no_vault_or_locked(&None))?
        };
        let vault = Vault::open(&path)?;
        // KDF runs without holding the slot lock.
        let uv = vault.unlock_with(&up)?;
        self.apply_git_identity(&uv);
        let problems = uv.integrity_problems().len();
        {
            let mut s = self.slot.write().unwrap_or_else(|p| p.into_inner());
            if s.path.as_deref() != Some(path.as_path()) {
                return Err(Error::Conflict(
                    "the active vault changed during unlock".into(),
                ));
            }
            if self.generation.load(std::sync::atomic::Ordering::SeqCst) != generation
                || self.is_shutting_down()
            {
                // Dropping `uv` zeroizes the keys.
                return Err(Error::Conflict(
                    "the vault was locked while unlocking; unlock again".into(),
                ));
            }
            if s.unlocked.is_none() {
                s.unlocked = Some(uv);
            }
            self.unlocked
                .store(true, std::sync::atomic::Ordering::SeqCst);
        }
        self.set_unlocked(up.timeout_secs);
        self.emit(AgentEventKind::Unlocked, None, None, None);
        if problems > 0 {
            self.emit(
                AgentEventKind::VaultChanged,
                None,
                None,
                Some(format!(
                    "integrity: {problems} problem(s) found; data access is blocked until `doctor` repairs the vault"
                )),
            );
        }
        Ok(self.vault_status())
    }

    pub(super) fn doctor(&self, dp: DoctorParams) -> Result<DoctorReport> {
        let path = self
            .slot
            .read()
            .unwrap_or_else(|p| p.into_inner())
            .path
            .clone();
        if !self.is_unlocked() || dp.staged {
            // Locked (or pre-commit): plaintext scan only, no keys needed.
            if dp.staged || !dp.fix && !dp.rebuild_manifest {
                let p = path.ok_or_else(|| Self::no_vault_or_locked(&None))?;
                return Vault::open(p)?.scan(dp.staged);
            }
        }
        let (report, before, after) = self.write(|v| {
            let before = v.integrity_problems().len();
            let r = v.doctor(&dp)?;
            Ok((r, before, v.integrity_problems().len()))
        })?;
        if before != after || !report.fixed.is_empty() {
            let msg = (before > 0 && after == 0).then(|| "integrity restored".to_owned());
            self.emit(AgentEventKind::VaultChanged, None, None, msg);
            self.emit(AgentEventKind::ProfilesChanged, None, None, None);
            self.emit(AgentEventKind::EntriesChanged, None, None, None);
        }
        Ok(report)
    }
}
