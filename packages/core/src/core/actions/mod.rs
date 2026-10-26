//! Saved launch definitions live only in encrypted profile documents.
use std::collections::HashSet;
use zeroize::Zeroize;

use super::{
    Error, Result, UnlockedVault, browser,
    format::{ActionDoc, now},
    ids,
};
use crate::protocol::{
    ActionIdParams, ActionInput, ActionSaveParams, EnvPair, EnvResolveParams, LaunchAction,
};

const MAX_ACTIONS: usize = 32;

/// Resolved secrets are zeroized even if validation or process spawning fails.
/// Deliberately has no Debug or serialization implementation.
pub struct ActionEnvironment {
    pub vars: Vec<EnvPair>,
}
impl Drop for ActionEnvironment {
    fn drop(&mut self) {
        for p in &mut self.vars {
            p.value.zeroize();
        }
    }
}

impl UnlockedVault {
    /// List confidential action metadata without revealing environment values.
    pub fn list_actions(&self, profile_id: &str) -> Result<Vec<LaunchAction>> {
        Ok(self
            .read_profile_doc(profile_id)?
            .actions
            .iter()
            .map(ActionDoc::to_protocol)
            .collect())
    }

    /// Find exactly one saved action, refusing malformed/duplicate synced definitions.
    pub fn get_action(&self, p: &ActionIdParams) -> Result<LaunchAction> {
        ids::check_id(&p.action_id)?;
        let actions = self.list_actions(&p.profile_id)?;
        let mut matches = actions.into_iter().filter(|a| a.id == p.action_id);
        let a = matches
            .next()
            .ok_or_else(|| Error::NotFound("action".into()))?;
        if matches.next().is_some() {
            return Err(Error::integrity("duplicate action ID"));
        }
        Ok(a)
    }

    /// Validate references again at launch: deleted/moved entries and keys fail closed.
    pub fn action_environment(
        &self,
        profile_id: &str,
        action_id: &str,
    ) -> Result<ActionEnvironment> {
        let a = self.get_action(&ActionIdParams {
            profile_id: profile_id.into(),
            action_id: action_id.into(),
        })?;
        let input = ActionInput {
            name: a.name,
            command: a.command,
            env_entry_ids: a.env_entry_ids,
            env_keys: a.env_keys,
            urls: a.urls,
        };
        Self::validate_action(&input)?;
        self.resolve_action_env(profile_id, &input)
    }

    fn resolve_action_env(
        &self,
        profile_id: &str,
        input: &ActionInput,
    ) -> Result<ActionEnvironment> {
        let mut resolved = ActionEnvironment {
            vars: self
                .env_resolve(&EnvResolveParams {
                    profile_id: profile_id.into(),
                    entry_ids: Some(input.env_entry_ids.clone()),
                })?
                .vars,
        };
        if let Some(keys) = &input.env_keys {
            if keys
                .iter()
                .any(|key| !resolved.vars.iter().any(|v| &v.key == key))
            {
                return Err(Error::invalid(
                    "an action variable is missing from its selected environment entries",
                ));
            }
            for p in &mut resolved.vars {
                if !keys.contains(&p.key) {
                    p.value.zeroize();
                }
            }
            resolved.vars.retain(|v| keys.contains(&v.key));
        }
        Ok(resolved)
    }

    fn validate_action(input: &ActionInput) -> Result<()> {
        if input.name.trim().is_empty()
            || input.name.len() > 128
            || input.name.chars().any(char::is_control)
        {
            return Err(Error::invalid(
                "action name must be 1–128 characters without control characters",
            ));
        }
        if input.command.len() > 16_384
            || input.command.contains('\0')
            || input.urls.len() > 16
            || input.env_entry_ids.len() > 128
            || input.env_keys.as_ref().is_some_and(|k| k.len() > 1024)
        {
            return Err(Error::invalid("action exceeds its size limits"));
        }
        if input.command.trim().is_empty() && input.urls.is_empty() {
            return Err(Error::invalid("an action needs a command or browser URL"));
        }
        let mut ids = HashSet::new();
        for id in &input.env_entry_ids {
            super::ids::check_id(id)?;
            if !ids.insert(id) {
                return Err(Error::invalid("duplicate environment entry"));
            }
        }
        if let Some(keys) = &input.env_keys {
            let mut seen = HashSet::new();
            for key in keys {
                if key.is_empty()
                    || key.len() > 256
                    || !super::environment::is_valid_key(key)
                    || !seen.insert(key)
                {
                    return Err(Error::invalid(
                        "invalid or duplicate environment variable name",
                    ));
                }
            }
        }
        for u in &input.urls {
            if u.len() > 4096 {
                return Err(Error::invalid("action URL is too long"));
            }
            let normalized = browser::normalize_start_url(u)?;
            if !normalized.starts_with("http://") && !normalized.starts_with("https://") {
                return Err(Error::invalid("action URLs must use HTTP or HTTPS"));
            }
        }
        Ok(())
    }

    /// Save an explicit definition; no command is executed by writes or sync.
    pub fn save_action(&mut self, p: &ActionSaveParams) -> Result<LaunchAction> {
        let action = self.save_action_uncommitted(p)?;
        self.commit(&format!("profile({}): update actions", p.profile_id))?;
        Ok(action)
    }

    pub(crate) fn save_action_uncommitted(&mut self, p: &ActionSaveParams) -> Result<LaunchAction> {
        self.ensure_writable()?;
        Self::validate_action(&p.input)?;
        drop(self.resolve_action_env(&p.profile_id, &p.input)?);
        let mut doc = self.read_profile_doc(&p.profile_id)?;
        let id = match &p.id {
            Some(id) => {
                ids::check_id(id)?;
                id.clone()
            }
            None => ids::new_id(),
        };
        let next = ActionDoc {
            id: id.clone(),
            name: p.input.name.trim().into(),
            command: p.input.command.clone(),
            env_entry_ids: p.input.env_entry_ids.clone(),
            env_keys: p.input.env_keys.clone(),
            urls: p
                .input
                .urls
                .iter()
                .map(|u| browser::normalize_start_url(u))
                .collect::<Result<_>>()?,
        };
        if p.id.is_some() {
            let matching: Vec<_> = doc
                .actions
                .iter()
                .enumerate()
                .filter(|(_, a)| a.id == id)
                .map(|(i, _)| i)
                .collect();
            match matching.as_slice() {
                [i] => doc.actions[*i] = next.clone(),
                [] => return Err(Error::NotFound("action".into())),
                _ => return Err(Error::integrity("duplicate action ID")),
            }
        } else {
            if doc.actions.len() >= MAX_ACTIONS {
                return Err(Error::invalid("a profile can have at most 32 actions"));
            }
            doc.actions.push(next.clone());
        }
        doc.updated_at = now();
        self.persist_actions(&doc)?;
        Ok(next.to_protocol())
    }

    /// Remove a saved definition without touching a running terminal.
    pub fn delete_action(&mut self, p: &ActionIdParams) -> Result<()> {
        self.ensure_writable()?;
        self.get_action(p)?;
        let mut doc = self.read_profile_doc(&p.profile_id)?;
        doc.actions.retain(|a| a.id != p.action_id);
        doc.updated_at = now();
        self.persist_actions(&doc)?;
        self.commit(&format!("profile({}): update actions", p.profile_id))?;
        Ok(())
    }

    fn persist_actions(&mut self, doc: &super::format::ProfileDoc) -> Result<()> {
        let hash = self.write_profile_doc(doc)?;
        self.manifest
            .profile_mut(&doc.id)
            .ok_or_else(|| Error::NotFound("profile".into()))?
            .hash = hash;
        self.save_manifest()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn legacy_profiles_default_to_no_actions_and_disk_fields_use_snake_case() {
        let old = serde_json::json!({"version":1,"id":"test","name":"old","color":"#2563eb","icon":"key","created_at":"now","updated_at":"now"});
        let doc = super::super::format::ProfileDoc::from_json(&serde_json::to_vec(&old).unwrap())
            .unwrap();
        assert!(doc.actions.is_empty());
        let a = ActionDoc {
            id: "id".into(),
            name: "name".into(),
            command: "echo confidential".into(),
            env_entry_ids: vec![],
            env_keys: None,
            urls: vec![],
        };
        let disk = serde_json::to_value(&a).unwrap();
        assert!(disk.get("env_entry_ids").is_some());
        let wire = serde_json::to_value(a.to_protocol()).unwrap();
        assert!(wire.get("envEntryIds").is_some());
        assert!(!format!("{:?}", a.to_protocol()).contains("confidential"));
    }
}
