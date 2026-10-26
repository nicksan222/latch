//! Git sync: plaintext scan, fetch, rebase onto upstream, conflict handling,
//! push.
//!
//! `manifest.age`/`manifest.sig` change in every commit, so they conflict on
//! every rebase step; they are never merged textually but regenerated from
//! the entry files after taking both sides. Profile documents that conflict
//! are resolved automatically (newest `updated_at` wins). Real conflicts are
//! entry files changed on both sides (or changed on one side and deleted on
//! the other); they are surfaced as [`ConflictSummary`] and resolved with
//! [`UnlockedVault::resolve`]. Concurrent changes to recipients/vault key on
//! both sides abort the sync with `Conflict`.
//!
//! Upstream is never trusted: before any fast-forward, reset or rebase the
//! upstream commit is authenticated as a whole ([`UnlockedVault::
//! authenticate_upstream`]): only vault paths, keychain consistent with the
//! local trust anchor and not rolled back, `vaultkey.age` consistent with
//! the keychain, manifest signed by the key in force and binding the whole
//! config, every listed file present with the signed hash. Each rebase step
//! re-signs only from authenticated inputs: the config on disk must be the
//! authenticated upstream one or the one of the local commit being replayed.
//!
//! The sync mode of a sync that stopped on conflicts is kept in
//! `.git/latch-sync-state` (untracked) so finishing it with `resolve` pushes
//! only when the original sync was allowed to push.
//!
//! During a rebase git's "ours" (index stage 2) is the upstream side
//! ("theirs" for the user) and stage 3 is the local commit being replayed
//! ("mine").

use std::collections::BTreeSet;

use super::crypto;
use super::error::{Error, Result};
use super::format::*;
use super::git::Git;
use super::secrets;
use super::trust;
use super::vault::{
    STATE_FILES, UnlockedVault, decrypt_vaultkey, load_state, record_sync, resolve_keys,
};
use crate::protocol::{
    ConflictDetail, ConflictSummary, Resolution, ResolveParams, SyncMode, SyncParams, SyncResult,
    SyncStatus,
};

const STAGE_THEIRS: u8 = 2; // upstream
const STAGE_MINE: u8 = 3; // local commit being replayed

fn stage(git: &Git, n: u8, path: &str) -> Result<Option<Vec<u8>>> {
    git.show_spec(&format!(":{n}:{path}"))
}

impl UnlockedVault {
    fn sync_branch(&self) -> String {
        if self.git.rebase_in_progress()
            && let Ok(p) = self.git.git_path("rebase-merge/head-name")
            && let Ok(s) = std::fs::read_to_string(p)
            && let Some(b) = s.trim().strip_prefix("refs/heads/")
        {
            return b.to_owned();
        }
        self.git.current_branch()
    }

    /// Conflicted entries of an in-progress sync.
    pub fn conflicts(&self) -> Result<Vec<ConflictSummary>> {
        if !self.git.is_repo() || !self.git.rebase_in_progress() {
            return Ok(vec![]);
        }
        let mut out = vec![];
        for path in self.git.unmerged_files()? {
            let Some((pid, eid)) = parse_entry_path(&path) else {
                continue;
            };
            let name = [STAGE_MINE, STAGE_THEIRS]
                .iter()
                .find_map(|s| {
                    stage(&self.git, *s, &path)
                        .ok()
                        .flatten()
                        .and_then(|ct| self.decode_entry(&ct).ok())
                        .map(|d| d.name.clone())
                })
                .unwrap_or_default();
            out.push(ConflictSummary {
                entry_id: eid,
                profile_id: pid,
                name,
            });
        }
        Ok(out)
    }

    pub fn sync_status(&self) -> Result<SyncStatus> {
        if !self.git.is_repo() {
            return Ok(SyncStatus {
                has_remote: false,
                remote_url: None,
                branch: String::new(),
                ahead: 0,
                behind: 0,
                dirty: false,
                last_sync_at: None,
                conflicts: vec![],
                log: vec![],
            });
        }
        let st = self.git.status()?;
        let status = self.status();
        Ok(SyncStatus {
            has_remote: self.git.remote_url().is_some(),
            remote_url: self.git.remote_url().map(|u| super::git::redact_url(&u)),
            branch: self.sync_branch(),
            ahead: st.ahead,
            behind: st.behind,
            dirty: st.dirty,
            last_sync_at: status.last_sync_at,
            conflicts: self.conflicts()?,
            log: self.git.log(50)?,
        })
    }

    /// Set (or with an empty string remove) the `origin` remote.
    pub fn set_remote(&mut self, url: &str) -> Result<SyncStatus> {
        if !self.git.is_repo() {
            return Err(Error::Git("vault is not a git repository".into()));
        }
        self.git.set_remote(url)?;
        self.sync_status()
    }

    /// Abort an in-progress sync (rebase) and reload.
    pub fn abort_sync(&mut self) -> Result<SyncStatus> {
        if self.git.rebase_in_progress() {
            self.git.run(&["rebase", "--abort"])?;
        }
        let _ = self.take_sync_mode();
        self.reload()?;
        self.sync_status()
    }

    fn sync_state_file(&self) -> Option<std::path::PathBuf> {
        self.git.git_path("latch-sync-state").ok()
    }

    fn save_sync_mode(&self, mode: SyncMode) -> Result<()> {
        let s = match mode {
            SyncMode::Both => "both",
            SyncMode::PullOnly => "pull_only",
            SyncMode::PushOnly => "push_only",
        };
        if let Some(p) = self.sync_state_file() {
            std::fs::write(&p, format!("{s}\n"))
                .map_err(|e| Error::io(format!("write {}", p.display()), e))?;
        }
        Ok(())
    }

    /// Mode of the interrupted sync (removed). Unknown/absent → None, which
    /// never pushes.
    fn take_sync_mode(&self) -> Option<SyncMode> {
        let p = self.sync_state_file()?;
        let s = std::fs::read_to_string(&p).ok()?;
        let _ = std::fs::remove_file(&p);
        match s.trim() {
            "both" => Some(SyncMode::Both),
            "pull_only" => Some(SyncMode::PullOnly),
            "push_only" => Some(SyncMode::PushOnly),
            _ => None,
        }
    }

    /// Authenticate the vault state at upstream commit `rev` before any of
    /// it is merged (see the module docs). Cached per commit.
    pub(crate) fn authenticate_upstream(&mut self, rev: &str) -> Result<()> {
        let oid = self
            .git
            .run(&["rev-parse", "--verify", "-q", &format!("{rev}^{{commit}}")])?
            .trim()
            .to_owned();
        if self.verified_onto.as_ref().is_some_and(|(o, _)| *o == oid) {
            return Ok(());
        }
        let short = oid[..oid.len().min(8)].to_owned();
        let fail =
            |m: &str| Error::integrity(format!("upstream commit {short}: {m}; sync refused"));
        let paths = self.git.tree_paths(&oid)?;
        if let Some(p) = paths.iter().find(|p| !is_vault_path(p)) {
            return Err(fail(&format!("contains `{p}`, which is not a vault file")));
        }
        let specs: Vec<String> = STATE_FILES.iter().map(|f| format!("{oid}:{f}")).collect();
        let blobs = self.git.read_blobs(&specs)?;
        let get = |f: &str| -> Result<&Vec<u8>> {
            let i = STATE_FILES
                .iter()
                .position(|x| *x == f)
                .expect("state file");
            blobs[i]
                .as_ref()
                .ok_or_else(|| fail(&format!("{f} is missing")))
        };
        let version: u32 = String::from_utf8_lossy(get(VERSION_FILE)?)
            .trim()
            .parse()
            .map_err(|_| fail(".latch/version is malformed"))?;
        if version == 0 || version > FORMAT_VERSION {
            return Err(Error::UnsupportedVersion {
                found: version,
                supported: FORMAT_VERSION,
            });
        }
        let config_bytes = get(CONFIG_FILE)?.clone();
        let config = ConfigDoc::parse(&String::from_utf8_lossy(&config_bytes))?;
        let id = self.config.vault.id.clone();
        if config.vault.id != id {
            return Err(fail("it belongs to a different vault id"));
        }
        let chain = KeychainDoc::parse(get(KEYCHAIN_FILE)?)?;
        trust::verify_chain(&chain, &id)?;
        trust::relation(&trust::pinned(&id)?, &chain.records)?;
        if self.git.head()?.is_some()
            && let Ok(base) = self.git.run(&["merge-base", "HEAD", &oid])
            && let Ok(Some(b)) = self.git.show(base.trim(), KEYCHAIN_FILE)
            && let Ok(bc) = KeychainDoc::parse(&b)
            && trust::relation(&bc.records, &chain.records)? == trust::Relation::Stale
        {
            return Err(fail("it rolls back a key rotation"));
        }
        let doc = decrypt_vaultkey(get(VAULTKEY_FILE)?, &self.secret).map_err(|e| match e {
            Error::BadCredentials => Error::Denied(
                "this identity cannot open the upstream vault key (removed as a recipient?)".into(),
            ),
            e => e,
        })?;
        let keys = resolve_keys(&config, &chain, &doc)?;
        let ct = get(MANIFEST_FILE)?;
        let sig = String::from_utf8_lossy(get(MANIFEST_SIG_FILE)?).into_owned();
        if !crypto::verify(&keys.signing.verifying_key(), ct, &sig) {
            return Err(fail("the manifest is not signed by the vault key"));
        }
        let pt = crypto::decrypt_with(&keys.all_identities(), ct)
            .map_err(|_| fail("the manifest cannot be decrypted"))?;
        let m = ManifestDoc::from_json(&pt)?;
        if m.config_hash != config.security_hash() {
            return Err(fail("config.toml does not match the signed manifest"));
        }
        let layout: BTreeSet<&String> = paths
            .iter()
            .filter(|p| parse_entry_path(p).is_some() || parse_profile_path(p).is_some())
            .collect();
        let mut listed: Vec<(String, String)> = vec![];
        for p in &m.profiles {
            listed.push((profile_file(&p.id), p.hash.clone()));
            for e in &p.entries {
                listed.push((entry_file(&p.id, &e.id), e.hash.clone()));
            }
        }
        let listed_set: BTreeSet<&String> = listed.iter().map(|(p, _)| p).collect();
        if layout != listed_set {
            return Err(fail("its files do not match the signed manifest"));
        }
        let specs: Vec<String> = listed.iter().map(|(p, _)| format!("{oid}:{p}")).collect();
        for ((p, h), blob) in listed.iter().zip(self.git.read_blobs(&specs)?) {
            if blob.as_deref().map(crypto::hash).as_deref() != Some(h.as_str()) {
                return Err(fail(&format!("{p} does not match the signed manifest")));
            }
        }
        self.verified_onto = Some((oid, config_bytes));
        Ok(())
    }

    fn result(&self, pulled: u32, pushed: u32) -> Result<SyncResult> {
        let status = self.sync_status()?;
        Ok(SyncResult {
            pulled,
            pushed,
            conflicts: status.conflicts.clone(),
            status,
        })
    }

    /// `doctor --staged`-style scan, fetch, rebase, resolve, push.
    pub fn sync(&mut self, params: &SyncParams) -> Result<SyncResult> {
        self.check_ok()?;
        if !self.git.is_repo() {
            return Err(Error::Git("vault is not a git repository".into()));
        }
        if self.git.rebase_in_progress() {
            return self.result(0, 0);
        }
        let bad: Vec<_> = super::doctor::scan(&self.root, false)?
            .into_iter()
            .filter(|i| i.severity == crate::protocol::Severity::Error)
            .collect();
        if !bad.is_empty() {
            let paths: Vec<String> = bad.iter().filter_map(|i| i.path.clone()).collect();
            return Err(Error::integrity(format!(
                "plaintext scan failed for {} file(s): {} (run doctor)",
                bad.len(),
                paths.join(", ")
            )));
        }
        // Commit stray vault changes so the rebase starts from a clean tree.
        if self.git.status()?.dirty {
            self.ensure_writable()?;
            self.commit("sync: commit pending changes")?;
        }
        if self.git.remote_url().is_none() {
            return self.result(0, 0);
        }
        self.git.fetch()?;
        let branch = self.sync_branch();
        let upstream = format!("origin/{branch}");
        let has_upstream = self.git.ref_exists(&format!("refs/remotes/{upstream}"));
        let mut pulled = 0;
        if has_upstream && params.mode != SyncMode::PushOnly {
            let (ahead, behind) = self.git.ahead_behind("HEAD", &upstream)?;
            if behind > 0 {
                self.authenticate_upstream(&upstream)?;
                pulled = behind;
                if self.git.head()?.is_none() {
                    self.git.run(&["reset", "--hard", "--quiet", &upstream])?;
                } else if ahead == 0 {
                    self.git
                        .run(&["merge", "--ff-only", "--quiet", &upstream])?;
                } else {
                    let out = self
                        .git
                        .run_with_identity(&["rebase", "--quiet", &upstream])?;
                    if !out.status.success() && !self.git.rebase_in_progress() {
                        return Err(Error::Git("rebase failed".into()));
                    }
                    self.save_sync_mode(params.mode)?;
                    let conflicts = self.drive_rebase()?;
                    if !conflicts.is_empty() {
                        return self.result(pulled, 0);
                    }
                    let _ = self.take_sync_mode();
                }
                self.after_pull()?;
            }
        }
        let mut pushed = 0;
        if params.mode != SyncMode::PullOnly && self.git.head()?.is_some() {
            let (ahead, behind) = if has_upstream {
                self.git.ahead_behind("HEAD", &upstream)?
            } else {
                let n: u32 = self
                    .git
                    .run(&["rev-list", "--count", "HEAD"])?
                    .trim()
                    .parse()
                    .unwrap_or(0);
                (n, 0)
            };
            if behind > 0 {
                return Err(Error::Conflict(
                    "the remote has new commits; pull first".into(),
                ));
            }
            if ahead > 0 {
                self.git.push(&branch)?;
                pushed = ahead;
            }
        }
        record_sync(&self.git);
        self.result(pulled, pushed)
    }

    /// After new upstream commits are in: reload keys (maybe rotated) and
    /// re-verify everything; fail closed on problems.
    fn after_pull(&mut self) -> Result<()> {
        self.reload()?;
        self.check_ok()
    }

    /// Advance an in-progress rebase as far as possible. Returns the real
    /// (entry) conflicts at the current stop, or empty when finished.
    pub(crate) fn drive_rebase(&mut self) -> Result<Vec<ConflictSummary>> {
        let mut guard = 0;
        while self.git.rebase_in_progress() {
            guard += 1;
            if guard > 10_000 {
                return Err(Error::Internal("rebase did not terminate".into()));
            }
            let unmerged = self.git.unmerged_files()?;
            let mut real = false;
            for path in &unmerged {
                if path == MANIFEST_FILE || path == MANIFEST_SIG_FILE {
                    continue; // regenerated below
                }
                if path == VAULTKEY_FILE
                    || path == CONFIG_FILE
                    || path == VERSION_FILE
                    || path == KEYCHAIN_FILE
                {
                    self.git.run(&["rebase", "--abort"])?;
                    let _ = self.take_sync_mode();
                    self.reload()?;
                    return Err(Error::Conflict(
                        "recipients or vault key changed on both sides; sync aborted (pull on one machine, then redo the change)".into(),
                    ));
                }
                if parse_profile_path(path).is_some() {
                    self.auto_resolve_profile(path)?;
                    continue;
                }
                if parse_entry_path(path).is_some() {
                    real = true;
                    continue;
                }
                // Generated public files: take upstream.
                if [README_FILE, GITATTRIBUTES_FILE, GITIGNORE_FILE].contains(&path.as_str()) {
                    self.take_stage(path, STAGE_THEIRS)?;
                    continue;
                }
                self.git.run(&["rebase", "--abort"])?;
                let _ = self.take_sync_mode();
                self.reload()?;
                return Err(Error::Conflict(format!(
                    "unexpected conflict in {path}; sync aborted"
                )));
            }
            if real {
                return self.conflicts();
            }
            self.continue_step()?;
        }
        Ok(vec![])
    }

    /// Write the given stage's content (or delete when absent) and stage it.
    fn take_stage(&self, path: &str, n: u8) -> Result<()> {
        match stage(&self.git, n, path)? {
            Some(bytes) => {
                write_atomic(&self.root.join(path), &bytes)?;
                self.git.run(&["add", "--", path])?;
            }
            None => {
                self.git
                    .run(&["rm", "--quiet", "--cached", "--ignore-unmatch", "--", path])?;
                let _ = std::fs::remove_file(self.root.join(path));
            }
        }
        Ok(())
    }

    fn auto_resolve_profile(&mut self, path: &str) -> Result<()> {
        let mine = stage(&self.git, STAGE_MINE, path)?;
        let theirs = stage(&self.git, STAGE_THEIRS, path)?;
        let ts = |b: &Option<Vec<u8>>| {
            b.as_ref()
                .and_then(|ct| self.decode_profile(ct).ok())
                .map(|p| p.updated_at.clone())
        };
        // Deleted vs modified: keep the modified side.
        let pick = match (&mine, &theirs) {
            (Some(_), None) => STAGE_MINE,
            (None, Some(_)) => STAGE_THEIRS,
            _ => {
                if ts(&mine) >= ts(&theirs) {
                    STAGE_MINE
                } else {
                    STAGE_THEIRS
                }
            }
        };
        self.take_stage(path, pick)
    }

    /// Abort the rebase, drop the saved mode and reload the local state.
    fn abort_with(&mut self, e: Error) -> Error {
        let _ = self.git.run(&["rebase", "--abort"]);
        let _ = self.take_sync_mode();
        let _ = self.reload();
        e
    }

    /// Regenerate the manifest from the working tree (authenticated inputs
    /// only) and continue the rebase.
    fn continue_step(&mut self) -> Result<()> {
        // The upstream this rebase replays onto must be authenticated.
        let onto = self
            .git
            .git_path("rebase-merge/onto")
            .ok()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .map(|s| s.trim().to_owned())
            .unwrap_or_default();
        if onto.is_empty() {
            return Err(self.abort_with(Error::integrity("cannot determine the rebase upstream")));
        }
        if let Err(e) = self.authenticate_upstream(&onto) {
            return Err(self.abort_with(e));
        }
        // config.toml must be exactly the authenticated upstream one or the
        // one of the local commit being replayed (no textual merges).
        let disk = std::fs::read(self.root.join(CONFIG_FILE)).unwrap_or_default();
        let upstream_ok = self.verified_onto.as_ref().is_some_and(|(_, c)| *c == disk);
        let ours_ok = self
            .git
            .show("REBASE_HEAD", CONFIG_FILE)
            .ok()
            .flatten()
            .is_some_and(|c| c == disk);
        if !upstream_ok && !ours_ok {
            return Err(self.abort_with(Error::Conflict(
                "config.toml changed on both sides; sync aborted (pull on one machine, then redo the change)".into(),
            )));
        }
        // Keys: keychain/vault key (upstream's after a rotation) checked
        // against the trust anchor.
        match load_state(&self.root, &self.secret) {
            Ok(st) => {
                self.config = st.config;
                self.chain = st.chain;
                self.keys = st.keys;
            }
            Err(e) => return Err(self.abort_with(e)),
        }
        let rebuilt = self.rebuild_manifest_from_disk(true)?;
        if let Some(i) = rebuilt.issues.first() {
            let msg = format!(
                "cannot rebuild manifest during sync: {} ({})",
                i.message,
                i.path.clone().unwrap_or_default()
            );
            return Err(self.abort_with(Error::integrity(msg)));
        }
        // Stage vault paths only (never stray files that appeared meanwhile).
        let tracked: BTreeSet<String> = self.git.ls_files()?.into_iter().collect();
        let mut stage: BTreeSet<String> = self.git.changed_paths()?.into_iter().collect();
        stage.extend(self.git.unmerged_files()?);
        let stage: Vec<String> = stage
            .into_iter()
            .filter(|p| is_vault_path(p) && (self.root.join(p).exists() || tracked.contains(p)))
            .collect();
        for chunk in stage.chunks(200) {
            let mut args = vec!["add", "-A", "--"];
            args.extend(chunk.iter().map(String::as_str));
            self.git.run(&args)?;
        }
        let out = self.git.run_with_identity(&["rebase", "--continue"])?;
        if !out.status.success()
            && self.git.rebase_in_progress()
            && self.git.unmerged_files()?.is_empty()
        {
            // Nothing left to commit for this step (changes already upstream).
            if !self.git.has_staged_changes()? {
                let out = self.git.run_with_identity(&["rebase", "--skip"])?;
                if !out.status.success()
                    && self.git.rebase_in_progress()
                    && self.git.unmerged_files()?.is_empty()
                {
                    return Err(Error::Git("rebase --skip failed".into()));
                }
            } else {
                return Err(Error::Git(format!(
                    "rebase --continue failed: {}",
                    String::from_utf8_lossy(&out.stderr)
                        .lines()
                        .next()
                        .unwrap_or("")
                )));
            }
        }
        Ok(())
    }

    fn conflict_path(&self, entry_id: &str) -> Result<String> {
        super::ids::check_id(entry_id)?;
        if !self.git.rebase_in_progress() {
            return Err(Error::NotFound("sync conflict".into()));
        }
        self.git
            .unmerged_files()?
            .into_iter()
            .find(|p| parse_entry_path(p).is_some_and(|(_, e)| e == entry_id))
            .ok_or_else(|| Error::NotFound("sync conflict for this entry".into()))
    }

    fn stage_doc(&self, path: &str, n: u8) -> Result<Option<EntryDoc>> {
        match stage(&self.git, n, path)? {
            Some(ct) => Ok(Some(self.decode_entry(&ct)?)),
            None => Ok(None),
        }
    }

    /// Both sides, decrypted and revealed (opening the resolver is the
    /// explicit user action). `None` = deleted on that side.
    pub fn conflict_detail(&self, entry_id: &str) -> Result<ConflictDetail> {
        let path = self.conflict_path(entry_id)?;
        Ok(ConflictDetail {
            entry_id: entry_id.to_owned(),
            mine: self
                .stage_doc(&path, STAGE_MINE)?
                .map(|d| secrets::to_entry(&d, true)),
            theirs: self
                .stage_doc(&path, STAGE_THEIRS)?
                .map(|d| secrets::to_entry(&d, true)),
        })
    }

    /// Resolve one conflicted entry; when no conflicts remain at this step
    /// the rebase continues (possibly stopping at the next conflict), and
    /// once finished the result is verified and pushed.
    pub fn resolve(&mut self, params: &ResolveParams) -> Result<SyncStatus> {
        let path = self.conflict_path(&params.entry_id)?;
        match params.resolution {
            Resolution::Mine => self.take_stage(&path, STAGE_MINE)?,
            Resolution::Theirs => self.take_stage(&path, STAGE_THEIRS)?,
            Resolution::Merged => {
                let input = params.merged.as_ref().ok_or_else(|| {
                    Error::invalid("merged input is required for a merged resolution")
                })?;
                secrets::validate_input(input)?;
                let (pid, eid) = parse_entry_path(&path).expect("checked");
                if input.profile_id != pid {
                    return Err(Error::invalid("a merged entry must stay in its profile"));
                }
                let mine = self.stage_doc(&path, STAGE_MINE)?;
                let theirs = self.stage_doc(&path, STAGE_THEIRS)?;
                let base = mine
                    .or(theirs)
                    .ok_or_else(|| Error::NotFound("either side of the conflict".into()))?;
                let mut doc = base.clone();
                doc.body = secrets::body_from_payload(&input.payload, Some(&base.body))?;
                doc.id = eid;
                doc.name = input.name.trim().to_owned();
                doc.tags = secrets::normalize_tags(&input.tags);
                doc.notes = input.notes.clone();
                doc.favorite = input.favorite;
                doc.updated_at = now();
                self.write_entry_doc(&doc)?;
                self.git.run(&["add", "--", &path])?;
            }
        }
        let remaining = self.drive_rebase()?;
        if remaining.is_empty() {
            // Push only if the sync that stopped was allowed to push.
            let mode = self.take_sync_mode();
            self.after_pull()?;
            if mode == Some(SyncMode::Both) && self.git.remote_url().is_some() {
                let branch = self.sync_branch();
                self.git.push(&branch)?;
            }
            record_sync(&self.git);
        }
        self.sync_status()
    }
}

#[cfg(test)]
mod tests {
    use super::super::vault::test_vault;
    use crate::protocol::*;

    #[test]
    fn status_without_remote() {
        let t = tempfile::tempdir().unwrap();
        let mut v = test_vault(t.path());
        let st = v.sync_status().unwrap();
        assert!(!st.has_remote && st.conflicts.is_empty());
        assert_eq!(st.branch, "main");
        assert_eq!(st.log.len(), 1);
        let r = v
            .sync(&SyncParams {
                mode: SyncMode::Both,
            })
            .unwrap();
        assert_eq!((r.pulled, r.pushed), (0, 0));
        assert!(v.conflicts().unwrap().is_empty());
        let id = crate::core::ids::new_id();
        assert_eq!(
            v.conflict_detail(&id).unwrap_err().code(),
            ErrorCode::NotFound
        );
        let e = v
            .resolve(&ResolveParams {
                entry_id: id,
                resolution: Resolution::Mine,
                merged: None,
            })
            .unwrap_err();
        assert_eq!(e.code(), ErrorCode::NotFound);
        assert!(v.abort_sync().unwrap().conflicts.is_empty());
    }
}
