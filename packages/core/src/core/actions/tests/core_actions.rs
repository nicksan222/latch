// Encrypted play-action persistence and least-privilege environment selection.
#[path = "../../../../tests/core_support/mod.rs"]
mod core_support;
use core_support::*;
use latch::protocol::*;

fn action(entry_id: &str) -> ActionInput {
    ActionInput {
        name: "Start development".into(),
        command: "bun run dev".into(),
        env_entry_ids: vec![entry_id.into()],
        env_keys: Some(vec!["PORT".into()]),
        urls: vec!["http://localhost:3000".into()],
    }
}

#[test]
fn actions_roundtrip_preserve_profile_edits_and_never_store_plaintext() {
    let t = tempfile::tempdir().unwrap();
    let path = t.path().join("vault");
    let mut v = new_vault(&path);
    let p = v.create_profile(&profile_input("project")).unwrap();
    let e = v
        .create_entry(&entry_input(
            &p.id,
            "Development",
            all_payloads().remove(2).1,
        ))
        .unwrap();
    let a = v
        .save_action(&ActionSaveParams {
            profile_id: p.id.clone(),
            id: None,
            input: action(&e.id),
        })
        .unwrap();
    assert_eq!(v.list_actions(&p.id).unwrap(), vec![a.clone()]);
    let resolved = v.action_environment(&p.id, &a.id).unwrap();
    assert_eq!(resolved.vars.len(), 1);
    assert_eq!(resolved.vars[0].key, "PORT");
    let mut input = profile_input("renamed");
    input.description = "Updated".into();
    v.update_profile(&ProfileUpdateParams {
        id: p.id.clone(),
        input,
    })
    .unwrap();
    drop(v);
    let mut v = unlock(&path);
    assert_eq!(v.list_actions(&p.id).unwrap(), vec![a.clone()]);
    let ct = std::fs::read(path.join("profiles").join(&p.id).join("profile.age")).unwrap();
    assert!(
        !ct.windows(b"bun run dev".len())
            .any(|w| w == b"bun run dev")
    );
    v.delete_action(&ActionIdParams {
        profile_id: p.id.clone(),
        action_id: a.id,
    })
    .unwrap();
    assert!(v.list_actions(&p.id).unwrap().is_empty());
}

#[test]
fn actions_reject_foreign_entries_missing_keys_and_unsafe_urls() {
    let t = tempfile::tempdir().unwrap();
    let mut v = new_vault(&t.path().join("vault"));
    let p = v.create_profile(&profile_input("project")).unwrap();
    let other = v.create_profile(&profile_input("other")).unwrap();
    let e = v
        .create_entry(&entry_input(
            &other.id,
            "Other env",
            all_payloads().remove(2).1,
        ))
        .unwrap();
    let mut input = action(&e.id);
    assert!(
        v.save_action(&ActionSaveParams {
            profile_id: p.id.clone(),
            id: None,
            input: input.clone()
        })
        .is_err()
    );
    input.env_entry_ids.clear();
    assert!(
        v.save_action(&ActionSaveParams {
            profile_id: p.id.clone(),
            id: None,
            input: input.clone()
        })
        .is_err()
    );
    input.env_keys = None;
    input.urls = vec!["javascript:alert(1)".into()];
    assert!(
        v.save_action(&ActionSaveParams {
            profile_id: p.id.clone(),
            id: None,
            input: input.clone()
        })
        .is_err()
    );
    input.urls.clear();
    input.command.clear();
    assert!(
        v.save_action(&ActionSaveParams {
            profile_id: p.id.clone(),
            id: None,
            input
        })
        .is_err()
    );
}

#[test]
fn deleted_environment_sources_fail_closed_at_run_time() {
    let t = tempfile::tempdir().unwrap();
    let mut v = new_vault(&t.path().join("vault"));
    let p = v.create_profile(&profile_input("project")).unwrap();
    let e = v
        .create_entry(&entry_input(&p.id, "env", all_payloads().remove(2).1))
        .unwrap();
    let a = v
        .save_action(&ActionSaveParams {
            profile_id: p.id.clone(),
            id: None,
            input: action(&e.id),
        })
        .unwrap();
    v.delete_entry(&e.id).unwrap();
    assert!(v.action_environment(&p.id, &a.id).is_err());
}
