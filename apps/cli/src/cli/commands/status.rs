use super::*;

pub(super) fn status(s: &mut Session, json: bool) -> CliResult {
    let st: VaultStatus = s.vault_status()?;
    if json {
        print_proto_json(&st, "VaultStatus")?;
    } else if !st.initialized {
        outln!(
            "no vault{}",
            st.path
                .as_deref()
                .map(|p| format!(" at {p}"))
                .unwrap_or_default()
        );
        outln!("run `latch init` to create one");
    } else {
        outln!("vault:   {}", st.path.clone().unwrap_or_default());
        outln!("id:      {}", st.vault_id.clone().unwrap_or_default());
        outln!(
            "state:   {}{}",
            if st.locked { "locked" } else { "unlocked" },
            locks_in(&st)
        );
        if let Some(t) = &st.trust {
            outln!("fingerprint: {} ({})", t.fingerprint, trust_state_name(t));
        }
        if st.rotation_pending {
            outln!("rotation: interrupted key rotation pending (run `latch doctor --fix`)");
        }
        for p in &st.integrity_problems {
            outln!("integrity: {p}");
        }
        if st.memory_locked == Some(false) {
            outln!("memory:  not locked (secrets may be swapped to disk)");
        }
        let labels: Vec<_> = st.recipients.iter().map(|r| r.label.as_str()).collect();
        outln!("recipients: {}", labels.join(", "));
        outln!(
            "git:     {}{}, ahead {}, behind {}{}",
            if st.dirty { "dirty" } else { "clean" },
            if st.has_remote { "" } else { ", no remote" },
            st.ahead,
            st.behind,
            st.last_sync_at
                .as_deref()
                .map(|t| format!(", last sync {t}"))
                .unwrap_or_default()
        );
    }
    if st.initialized && !st.locked {
        integrity_probe(s)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reports_uninitialized_agent_without_unlocking_or_creating_a_vault() {
        let mut fixture = crate::cli::commands::test_support::EmptySession::new();
        status(&mut fixture.session, true).unwrap();
        let state: VaultStatus = fixture.session.vault_status().unwrap();
        assert!(!state.initialized);
        assert!(state.locked);
    }
}
