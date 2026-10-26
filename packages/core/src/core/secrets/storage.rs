//! Entry CRUD, lookup, reveal and stored-login TOTP orchestration.
//!
//! Secret-kind validation, conversion and redaction stay in [`super`]; this
//! module applies them to the vault. Authenticated storage and write guards stay
//! in [`crate::core::vault`], and OTP generation stays in [`crate::core::totp`].
//! All adapters share these inherent [`UnlockedVault`] methods.
//!
//! Acceptance coverage: `tests/core_vault.rs` (`every_entry_type_round_trips`,
//! `update_keeps_redacted_secrets`, `entries_list_query_move_delete`,
//! `totp_and_pinned`, `errors_never_contain_secret_values`).

use super as secrets;
use crate::core::error::{Error, Result};
use crate::core::format::*;
use crate::core::ids;
use crate::core::vault::UnlockedVault;
use crate::protocol::{
    Entry, EntryInput, EntrySummary, EntryUpdateParams, GetEntryParams, ListEntriesParams,
    MoveEntryParams, PinnedTotp, RevealParams, RevealResult, TotpCode,
};

impl UnlockedVault {
    // ----- entries ---------------------------------------------------------

    fn entry_summary(p: &ManifestProfile, e: &ManifestEntry) -> EntrySummary {
        EntrySummary {
            id: e.id.clone(),
            profile_id: p.id.clone(),
            entry_type: e.entry_type,
            name: e.name.clone(),
            tags: e.tags.clone(),
            favorite: e.favorite,
            subtitle: e.subtitle.clone(),
            has_totp: e.has_totp,
            updated_at: e.updated_at.clone(),
        }
    }

    /// Metadata only, from the manifest. Sorted by name.
    pub fn list_entries(&self, params: &ListEntriesParams) -> Result<Vec<EntrySummary>> {
        self.check_ok()?;
        if let Some(pid) = &params.profile_id
            && self.manifest.profile(pid).is_none()
        {
            return Err(Error::NotFound("profile".into()));
        }
        let mut out: Vec<EntrySummary> = self
            .manifest
            .profiles
            .iter()
            .filter(|p| params.profile_id.as_ref().is_none_or(|id| *id == p.id))
            .flat_map(|p| {
                p.entries
                    .iter()
                    .filter(|e| secrets::matches(e, params.query.as_deref(), params.entry_type))
                    .map(move |e| Self::entry_summary(p, e))
            })
            .collect();
        out.sort_by(|a, b| {
            a.name
                .to_lowercase()
                .cmp(&b.name.to_lowercase())
                .then(a.id.cmp(&b.id))
        });
        Ok(out)
    }

    /// Find an entry in a profile by id or (case-insensitive) name.
    pub fn find_entry(&self, profile_id: &str, name_or_id: &str) -> Result<EntrySummary> {
        self.check_ok()?;
        let p = self
            .manifest
            .profile(profile_id)
            .ok_or_else(|| Error::NotFound("profile".into()))?;
        p.entries
            .iter()
            .find(|e| e.id == name_or_id)
            .or_else(|| {
                p.entries
                    .iter()
                    .find(|e| e.name.eq_ignore_ascii_case(name_or_id))
            })
            .map(|e| Self::entry_summary(p, e))
            .ok_or_else(|| Error::NotFound("entry".into()))
    }

    pub fn get_entry(&self, params: &GetEntryParams) -> Result<Entry> {
        Ok(secrets::to_entry(
            &self.read_entry_doc(&params.id)?,
            params.reveal,
        ))
    }

    /// One field value (secret paths, `url` for databases, `username`…).
    pub fn reveal(&self, params: &RevealParams) -> Result<RevealResult> {
        let doc = self.read_entry_doc(&params.id)?;
        let v = secrets::reveal_field(&doc.body, &params.field)?;
        Ok(RevealResult {
            value: v.to_string(),
        })
    }

    fn check_entry_name_free(&self, pid: &str, name: &str, except: Option<&str>) -> Result<()> {
        let p = self
            .manifest
            .profile(pid)
            .ok_or_else(|| Error::NotFound("profile".into()))?;
        if p.entries
            .iter()
            .any(|e| Some(e.id.as_str()) != except && e.name.eq_ignore_ascii_case(name.trim()))
        {
            return Err(Error::AlreadyExists(
                "an entry with this name in the profile".into(),
            ));
        }
        Ok(())
    }

    pub(crate) fn upsert_manifest_entry(&mut self, pid: &str, me: ManifestEntry) {
        if let Some(p) = self.manifest.profile_mut(pid) {
            if let Some(e) = p.entries.iter_mut().find(|e| e.id == me.id) {
                *e = me;
            } else {
                p.entries.push(me);
            }
        }
    }

    pub(crate) fn remove_manifest_entry(&mut self, id: &str) {
        for p in &mut self.manifest.profiles {
            p.entries.retain(|e| e.id != id);
        }
    }

    pub fn create_entry(&mut self, input: &EntryInput) -> Result<Entry> {
        let (e, msg) = self.create_entry_uncommitted(input)?;
        self.commit(&msg)?;
        Ok(e)
    }

    /// Create an entry without committing; returns the commit message the
    /// caller should use (batch operations such as import commit once).
    pub(crate) fn create_entry_uncommitted(
        &mut self,
        input: &EntryInput,
    ) -> Result<(Entry, String)> {
        self.ensure_writable()?;
        secrets::validate_input(input)?;
        ids::check_id(&input.profile_id)?;
        self.check_entry_name_free(&input.profile_id, &input.name, None)?;
        let body = secrets::body_from_payload(&input.payload, None)?;
        let ts = now();
        let doc = EntryDoc {
            version: FORMAT_VERSION,
            id: ids::new_id(),
            profile_id: input.profile_id.clone(),
            name: input.name.trim().to_owned(),
            tags: secrets::normalize_tags(&input.tags),
            notes: input.notes.clone(),
            favorite: input.favorite,
            created_at: ts.clone(),
            updated_at: ts,
            body,
        };
        let hash = self.write_entry_doc(&doc)?;
        self.upsert_manifest_entry(&doc.profile_id, secrets::manifest_entry(&doc, hash));
        self.save_manifest()?;
        let msg = format!(
            "entry({}): create {}",
            self.profile_label(&doc.profile_id),
            self.label(&doc.id, &doc.name)
        );
        Ok((secrets::to_entry(&doc, false), msg))
    }

    /// Update an entry. `None` secrets keep stored values. A different
    /// `input.profile_id` moves the entry.
    pub fn update_entry(&mut self, params: &EntryUpdateParams) -> Result<Entry> {
        self.ensure_writable()?;
        secrets::validate_input(&params.input)?;
        let old = self.read_entry_doc(&params.id)?;
        let target = params.input.profile_id.clone();
        ids::check_id(&target)?;
        self.check_entry_name_free(&target, &params.input.name, Some(&params.id))?;
        let body = secrets::body_from_payload(&params.input.payload, Some(&old.body))?;
        let mut doc = old.clone();
        doc.profile_id = target.clone();
        doc.name = params.input.name.trim().to_owned();
        doc.tags = secrets::normalize_tags(&params.input.tags);
        doc.notes = params.input.notes.clone();
        doc.favorite = params.input.favorite;
        doc.body = body;
        doc.updated_at = now();
        let moved = old.profile_id != target;
        let hash = self.write_entry_doc(&doc)?;
        if moved {
            let p = self.root.join(entry_file(&old.profile_id, &old.id));
            std::fs::remove_file(&p)
                .map_err(|e| Error::io(format!("remove {}", p.display()), e))?;
            self.remove_manifest_entry(&doc.id);
        }
        self.upsert_manifest_entry(&target, secrets::manifest_entry(&doc, hash));
        self.save_manifest()?;
        let msg = if moved {
            format!(
                "entry({}): move {} to {}",
                self.profile_label(&old.profile_id),
                self.label(&doc.id, &doc.name),
                self.profile_label(&target)
            )
        } else {
            format!(
                "entry({}): update {}",
                self.profile_label(&target),
                self.label(&doc.id, &doc.name)
            )
        };
        self.commit(&msg)?;
        Ok(secrets::to_entry(&doc, false))
    }

    /// Delete an entry (soft: restorable from git history via trash).
    pub fn delete_entry(&mut self, id: &str) -> Result<()> {
        self.ensure_writable()?;
        let doc = self.read_entry_doc(id)?;
        let p = self.root.join(entry_file(&doc.profile_id, &doc.id));
        std::fs::remove_file(&p).map_err(|e| Error::io(format!("remove {}", p.display()), e))?;
        self.remove_manifest_entry(id);
        self.save_manifest()?;
        let msg = format!(
            "entry({}): delete {}",
            self.profile_label(&doc.profile_id),
            self.label(&doc.id, &doc.name)
        );
        self.commit(&msg)?;
        Ok(())
    }

    pub fn move_entry(&mut self, params: &MoveEntryParams) -> Result<Entry> {
        let doc = self.read_entry_doc(&params.id)?;
        if doc.profile_id == params.profile_id {
            return Ok(secrets::to_entry(&doc, false));
        }
        let mut input = secrets::to_input(&doc);
        input.profile_id = params.profile_id.clone();
        self.update_entry(&EntryUpdateParams {
            id: params.id.clone(),
            input,
        })
    }

    // ----- TOTP ------------------------------------------------------------

    pub fn totp(&self, id: &str) -> Result<TotpCode> {
        let doc = self.read_entry_doc(id)?;
        match &doc.body {
            EntryBody::Login(LoginDoc { totp: Some(t), .. }) => crate::core::totp::code_now(t),
            _ => Err(Error::NotFound("TOTP for this entry".into())),
        }
    }

    /// Favorite logins with TOTP (tray).
    pub fn pinned_totp(&self) -> Result<Vec<PinnedTotp>> {
        self.check_ok()?;
        let mut v = vec![];
        for p in &self.manifest.profiles {
            for e in &p.entries {
                if e.has_totp && e.favorite {
                    v.push(PinnedTotp {
                        id: e.id.clone(),
                        profile_name: p.name.clone(),
                        name: e.name.clone(),
                    });
                }
            }
        }
        v.sort_by_key(|a| (a.profile_name.to_lowercase(), a.name.to_lowercase()));
        Ok(v)
    }
}
