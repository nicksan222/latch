//! Resolve and format a profile's environment from authenticated env entries.
//!
//! This module owns selection, profile isolation and merge ordering for every
//! adapter. Parsing/formatting primitives live in this module’s private parser; encrypted
//! document reads remain in [`super::vault`]. Values are returned only for an
//! explicit resolve/format request and are never written as plaintext files.
//!
//! Acceptance coverage: `tests/core_vault.rs::env_resolve_and_format` and the
//! action environment scenarios in `tests/core_actions.rs`.

mod dotenv;

pub use dotenv::{format, format_pairs, is_valid_key, merge, parse};

use super::error::{Error, Result};
use super::format::{EntryBody, EnvDoc};
use super::vault::UnlockedVault;
use crate::protocol::{EnvFormatParams, EnvResolveParams, EnvResolveResult, TextResult};

impl UnlockedVault {
    // ----- env -------------------------------------------------------------

    /// Merge env entries of a profile (all, in manifest order, or the named
    /// ones in the given order; later wins).
    pub fn env_resolve(&self, params: &EnvResolveParams) -> Result<EnvResolveResult> {
        self.check_ok()?;
        let p = self
            .manifest
            .profile(&params.profile_id)
            .ok_or_else(|| Error::NotFound("profile".into()))?;
        let ids: Vec<String> = match &params.entry_ids {
            Some(ids) => {
                for id in ids {
                    if !p.entries.iter().any(|e| &e.id == id) {
                        return Err(Error::NotFound("entry in this profile".into()));
                    }
                }
                ids.clone()
            }
            None => p
                .entries
                .iter()
                .filter(|e| e.entry_type == crate::protocol::EntryType::Env)
                .map(|e| e.id.clone())
                .collect(),
        };
        let mut docs = vec![];
        for id in &ids {
            let d = self.read_entry_doc(id)?;
            match &d.body {
                EntryBody::Env(_) => docs.push(d),
                _ => return Err(Error::invalid("entry is not an env entry")),
            }
        }
        let envs: Vec<&EnvDoc> = docs
            .iter()
            .filter_map(|d| match &d.body {
                EntryBody::Env(e) => Some(e),
                _ => None,
            })
            .collect();
        Ok(EnvResolveResult {
            vars: super::environment::merge(envs),
        })
    }

    pub fn env_format(&self, params: &EnvFormatParams) -> Result<TextResult> {
        let r = self.env_resolve(&EnvResolveParams {
            profile_id: params.profile_id.clone(),
            entry_ids: params.entry_ids.clone(),
        })?;
        Ok(TextResult {
            text: super::environment::format_pairs(&r.vars, params.format),
        })
    }
}
