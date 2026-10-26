//! Profile CRUD, name validation and manifest summaries on [`UnlockedVault`].
//!
//! Vault lifecycle, authenticated reads/writes and batch rollback remain in
//! [`super::vault`]; entry behavior belongs to [`super::secrets`]. This module
//! owns profile mutations for every adapter without changing the public handle.
//!
//! Acceptance coverage: `tests/core_vault.rs` (`profiles_crud_and_listing` and
//! `profiles_normalize_placeholder_start_pages_on_create_and_update`).

use super::error::{Error, Result};
use super::format::*;
use super::ids;
use super::vault::UnlockedVault;
use crate::protocol::{Profile, ProfileInput, ProfileSummary, ProfileUpdateParams};

impl UnlockedVault {
    // ----- profiles --------------------------------------------------------

    fn summary(p: &ManifestProfile) -> ProfileSummary {
        ProfileSummary {
            id: p.id.clone(),
            name: p.name.clone(),
            color: p.color.clone(),
            icon: p.icon.clone(),
            description: p.description.clone(),
            pinned: p.pinned,
            entry_count: p.entries.len() as u32,
            browser_running: false,
        }
    }

    /// Pinned first, then by name.
    pub fn list_profiles(&self) -> Result<Vec<ProfileSummary>> {
        self.check_ok()?;
        let mut v: Vec<ProfileSummary> = self.manifest.profiles.iter().map(Self::summary).collect();
        v.sort_by(|a, b| {
            b.pinned
                .cmp(&a.pinned)
                .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
        });
        Ok(v)
    }

    pub fn get_profile(&self, id: &str) -> Result<Profile> {
        Ok(self.read_profile_doc(id)?.to_protocol())
    }

    /// Find a profile by id or (case-insensitive) name.
    pub fn find_profile(&self, name_or_id: &str) -> Result<ProfileSummary> {
        self.check_ok()?;
        self.manifest
            .profiles
            .iter()
            .find(|p| p.id == name_or_id)
            .or_else(|| {
                self.manifest
                    .profiles
                    .iter()
                    .find(|p| p.name.eq_ignore_ascii_case(name_or_id))
            })
            .map(Self::summary)
            .ok_or_else(|| Error::NotFound("profile".into()))
    }

    fn check_profile_name_free(&self, name: &str, except: Option<&str>) -> Result<()> {
        if self
            .manifest
            .profiles
            .iter()
            .any(|p| Some(p.id.as_str()) != except && p.name.eq_ignore_ascii_case(name.trim()))
        {
            return Err(Error::AlreadyExists("a profile with this name".into()));
        }
        Ok(())
    }

    fn manifest_profile_from(
        doc: &ProfileDoc,
        hash: String,
        entries: Vec<ManifestEntry>,
    ) -> ManifestProfile {
        ManifestProfile {
            id: doc.id.clone(),
            name: doc.name.clone(),
            color: doc.color.clone(),
            icon: doc.icon.clone(),
            description: doc.description.clone(),
            pinned: doc.pinned,
            hash,
            entries,
        }
    }

    pub fn create_profile(&mut self, input: &ProfileInput) -> Result<Profile> {
        let (p, msg) = self.create_profile_uncommitted(input)?;
        self.commit(&msg)?;
        Ok(p)
    }

    /// Create a profile without committing; returns the commit message the
    /// caller should use (batch operations such as import commit once).
    pub(crate) fn create_profile_uncommitted(
        &mut self,
        input: &ProfileInput,
    ) -> Result<(Profile, String)> {
        self.ensure_writable()?;
        let input = normalize_profile_input(input)?;
        self.check_profile_name_free(&input.name, None)?;
        let ts = now();
        let mut doc = ProfileDoc {
            version: FORMAT_VERSION,
            id: ids::new_id(),
            name: String::new(),
            color: String::new(),
            icon: String::new(),
            description: String::new(),
            pinned: false,
            browser: BrowserDoc::default(),
            actions: vec![],
            created_at: ts.clone(),
            updated_at: ts,
        };
        doc.apply_input(&input);
        // Register the generated path before any fallible write. Import's outer
        // transaction cannot learn the ID if this helper fails partway through.
        let mut txn = self.begin_txn();
        txn.created_dirs.push(profile_dir(&doc.id));
        let outcome = (|| -> Result<()> {
            let hash = self.write_profile_doc(&doc)?;
            self.manifest
                .profiles
                .push(Self::manifest_profile_from(&doc, hash, vec![]));
            self.save_manifest()
        })();
        if let Err(e) = outcome {
            self.rollback(txn);
            return Err(e);
        }
        let label = self.label(&doc.id, &doc.name);
        Ok((doc.to_protocol(), format!("profile({label}): create")))
    }

    pub fn update_profile(&mut self, params: &ProfileUpdateParams) -> Result<Profile> {
        let (p, msg) = self.update_profile_uncommitted(params)?;
        self.commit(&msg)?;
        Ok(p)
    }

    /// Update a profile without committing; returns the commit message.
    pub(crate) fn update_profile_uncommitted(
        &mut self,
        params: &ProfileUpdateParams,
    ) -> Result<(Profile, String)> {
        self.ensure_writable()?;
        let input = normalize_profile_input(&params.input)?;
        let mut doc = self.read_profile_doc(&params.id)?;
        self.check_profile_name_free(&input.name, Some(&params.id))?;
        doc.apply_input(&input);
        doc.updated_at = now();
        let hash = self.write_profile_doc(&doc)?;
        let mp = self
            .manifest
            .profile_mut(&doc.id)
            .ok_or_else(|| Error::NotFound("profile".into()))?;
        let entries = std::mem::take(&mut mp.entries);
        *mp = Self::manifest_profile_from(&doc, hash, entries);
        self.save_manifest()?;
        let label = self.label(&doc.id, &doc.name);
        Ok((doc.to_protocol(), format!("profile({label}): update")))
    }

    /// Delete a profile and all its entries (recoverable from git history).
    pub fn delete_profile(&mut self, id: &str) -> Result<()> {
        self.ensure_writable()?;
        ids::check_id(id)?;
        if self.manifest.profile(id).is_none() {
            return Err(Error::NotFound("profile".into()));
        }
        let label = self.profile_label(id);
        let dir = self.root.join(profile_dir(id));
        std::fs::remove_dir_all(&dir)
            .map_err(|e| Error::io(format!("remove {}", dir.display()), e))?;
        self.manifest.profiles.retain(|p| p.id != id);
        self.save_manifest()?;
        self.commit(&format!("profile({label}): delete"))?;
        Ok(())
    }
}
