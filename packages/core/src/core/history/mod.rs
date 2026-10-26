//! Entry history from git, restore of a previous version, and trash
//! (entries deleted within the last 30 days, found in git history).
//!
//! History decrypts adjacent versions in memory to compute which field paths
//! changed; values never leave this module.

use super::error::{Error, Result};
use super::format::*;
use super::ids;
use super::secrets;
use super::vault::UnlockedVault;
use crate::protocol::{Entry, HistoryItem, RestoreParams, TrashItem, TrashParams};

/// How far back the trash looks.
pub const TRASH_DAYS: u32 = 30;

fn entry_glob(id: &str) -> String {
    format!(":(glob){PROFILES_DIR}/*/entries/{id}.age")
}

/// Commit ids passed by callers must look like hex object names (also
/// prevents option injection into git).
pub(crate) fn check_commit(c: &str) -> Result<()> {
    if (4..=64).contains(&c.len()) && c.bytes().all(|b| b.is_ascii_hexdigit()) {
        Ok(())
    } else {
        Err(Error::invalid("malformed commit id"))
    }
}

impl UnlockedVault {
    /// Versions of an entry, newest first. Works for deleted entries too.
    pub fn history(&self, id: &str) -> Result<Vec<HistoryItem>> {
        self.check_ok()?;
        ids::check_id(id)?;
        if !self.git.is_repo() {
            return Ok(vec![]);
        }
        let spec = entry_glob(id);
        let items = self.git.log_paths(&[spec.as_str()], &[])?;
        if items.is_empty() {
            return Err(Error::NotFound("entry history".into()));
        }
        // Oldest → newest to diff against the previous version.
        let mut out: Vec<HistoryItem> = vec![];
        let mut prev: Option<EntryDoc> = None;
        for item in items.iter().rev() {
            let present = item.changes.iter().find(|c| c.status != 'D');
            let (changed, deleted) = match present {
                Some(c) => {
                    let doc = self
                        .git
                        .show(&item.info.commit, &c.path)?
                        .and_then(|ct| self.decode_entry(&ct).ok());
                    match doc {
                        Some(doc) => {
                            let changed = secrets::changed_fields(prev.as_ref(), &doc);
                            prev = Some(doc);
                            (changed, false)
                        }
                        None => (vec![], false),
                    }
                }
                None => (vec![], true),
            };
            out.push(HistoryItem {
                commit: item.info.commit.clone(),
                at: item.info.at.clone(),
                author: item.info.author.clone(),
                message: item.info.message.clone(),
                changed_fields: changed,
                deleted,
            });
        }
        out.reverse();
        Ok(out)
    }

    /// Restore the entry as it was at `commit` (re-encrypted with the current
    /// vault key; also recreates its profile if that was deleted).
    pub fn restore(&mut self, params: &RestoreParams) -> Result<Entry> {
        self.ensure_writable()?;
        ids::check_id(&params.id)?;
        check_commit(&params.commit)?;
        let path = self
            .git
            .ls_tree(&params.commit, PROFILES_DIR)?
            .into_iter()
            .find(|p| parse_entry_path(p).is_some_and(|(_, e)| e == params.id))
            .ok_or_else(|| Error::NotFound("entry at that commit".into()))?;
        let ct = self
            .git
            .show(&params.commit, &path)?
            .ok_or_else(|| Error::NotFound("entry at that commit".into()))?;
        let mut doc = self.decode_entry(&ct)?;
        if doc.id != params.id {
            return Err(Error::integrity("historical entry has a different id"));
        }
        // Current location wins; otherwise the historical profile.
        if let Some((p, _)) = self.manifest.entry(&doc.id) {
            let current_pid = p.id.clone();
            if current_pid != doc.profile_id {
                let old = self.root.join(entry_file(&current_pid, &doc.id));
                let _ = std::fs::remove_file(old);
                self.remove_manifest_entry(&doc.id);
                doc.profile_id = current_pid;
            }
        }
        if self.manifest.profile(&doc.profile_id).is_none() {
            let pct = self
                .git
                .show(&params.commit, &profile_file(&doc.profile_id))?
                .ok_or_else(|| Error::NotFound("profile of the restored entry".into()))?;
            let mut pdoc = self.decode_profile(&pct)?;
            if self
                .manifest
                .profiles
                .iter()
                .any(|p| p.name.eq_ignore_ascii_case(&pdoc.name))
            {
                pdoc.name = format!("{} (restored)", pdoc.name);
            }
            pdoc.updated_at = now();
            let hash = self.write_profile_doc(&pdoc)?;
            self.manifest.profiles.push(ManifestProfile {
                id: pdoc.id.clone(),
                name: pdoc.name.clone(),
                color: pdoc.color.clone(),
                icon: pdoc.icon.clone(),
                description: pdoc.description.clone(),
                pinned: pdoc.pinned,
                hash,
                entries: vec![],
            });
        }
        let clash = self.manifest.profile(&doc.profile_id).is_some_and(|p| {
            p.entries
                .iter()
                .any(|e| e.id != doc.id && e.name.eq_ignore_ascii_case(&doc.name))
        });
        if clash {
            doc.name = format!("{} (restored)", doc.name);
        }
        doc.updated_at = now();
        let hash = self.write_entry_doc(&doc)?;
        self.upsert_manifest_entry(&doc.profile_id, secrets::manifest_entry(&doc, hash));
        self.save_manifest()?;
        let short = &params.commit[..params.commit.len().min(8)];
        let msg = format!(
            "entry({}): restore {} from {short}",
            self.profile_label(&doc.profile_id),
            self.label(&doc.id, &doc.name)
        );
        self.commit(&msg)?;
        Ok(secrets::to_entry(&doc, false))
    }

    /// Entries deleted in the last 30 days that do not exist anymore.
    pub fn trash(&self, params: &TrashParams) -> Result<Vec<TrashItem>> {
        self.check_ok()?;
        if !self.git.is_repo() {
            return Ok(vec![]);
        }
        let since = format!("--since={TRASH_DAYS}.days.ago");
        let spec = format!(":(glob){PROFILES_DIR}/*/entries/*.age");
        let items = self
            .git
            .log_paths(&[spec.as_str()], &["--diff-filter=D", since.as_str()])?;
        let mut seen = std::collections::HashSet::new();
        let mut out = vec![];
        for item in &items {
            for ch in item.changes.iter().filter(|c| c.status == 'D') {
                let Some((pid, eid)) = parse_entry_path(&ch.path) else {
                    continue;
                };
                if self.manifest.entry(&eid).is_some() || !seen.insert(eid.clone()) {
                    continue;
                }
                if params.profile_id.as_ref().is_some_and(|p| *p != pid) {
                    continue;
                }
                let parent = self
                    .git
                    .run(&["rev-parse", &format!("{}^", item.info.commit)])?
                    .trim()
                    .to_owned();
                let Some(doc) = self
                    .git
                    .show(&parent, &ch.path)?
                    .and_then(|ct| self.decode_entry(&ct).ok())
                else {
                    continue;
                };
                out.push(TrashItem {
                    id: eid,
                    profile_id: pid,
                    name: doc.name.clone(),
                    entry_type: doc.entry_type(),
                    deleted_at: item.info.at.clone(),
                    commit: parent,
                });
            }
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commit_validation() {
        assert!(check_commit("abcdef12").is_ok());
        assert!(check_commit("--help").is_err());
        assert!(check_commit("abc").is_err());
        assert!(check_commit("HEAD").is_err());
        assert!(entry_glob("x").contains("entries/x.age"));
    }
}
