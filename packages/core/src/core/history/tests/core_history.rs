// Entry history (changed field paths), restore, trash.

#[path = "../../../../tests/core_support/mod.rs"]
mod core_support;

use core_support::*;
use latch::protocol::*;

#[test]
fn history_restore_trash() {
    let t = tempfile::tempdir().unwrap();
    let path = t.path().join("v");
    let mut v = new_vault(&path);
    let p = v.create_profile(&profile_input("acme")).unwrap();
    let env = |a: &str| {
        EntryPayload::Env(EnvData {
            vars: vec![
                EnvVar {
                    key: "A".into(),
                    value: Some(a.into()),
                    hidden: true,
                    comment: None,
                },
                EnvVar {
                    key: "B".into(),
                    value: Some("b".into()),
                    hidden: false,
                    comment: None,
                },
            ],
        })
    };
    let e = v
        .create_entry(&entry_input(&p.id, "env", env("secret-one")))
        .unwrap();
    v.update_entry(&EntryUpdateParams {
        id: e.id.clone(),
        input: entry_input(&p.id, "env", env("secret-two")),
    })
    .unwrap();
    let mut renamed = entry_input(&p.id, "env renamed", env("secret-two"));
    renamed.tags = vec!["x".into()];
    v.update_entry(&EntryUpdateParams {
        id: e.id.clone(),
        input: renamed,
    })
    .unwrap();

    let h = v.history(&e.id).unwrap();
    assert_eq!(h.len(), 3);
    assert_eq!(h[0].changed_fields, vec!["name", "tags"]);
    assert_eq!(h[1].changed_fields, vec!["vars.A"]);
    assert!(h[2].changed_fields.contains(&"vars.A".to_owned()));
    assert!(h[2].message.contains("create"));
    assert!(
        h.iter()
            .all(|i| !i.deleted && i.author == "Alice" && i.at.ends_with('Z'))
    );
    let json = serde_json::to_string(&h).unwrap();
    assert!(!json.contains("secret-one") && !json.contains("secret-two"));

    // Restore the first version.
    let r = v
        .restore(&RestoreParams {
            id: e.id.clone(),
            commit: h[2].commit.clone(),
        })
        .unwrap();
    assert_eq!(r.name, "env");
    assert_eq!(
        v.reveal(&RevealParams {
            id: e.id.clone(),
            field: "vars.A".into()
        })
        .unwrap()
        .value,
        "secret-one"
    );
    let h2 = v.history(&e.id).unwrap();
    assert_eq!(h2.len(), 4);
    assert!(h2[0].message.contains("restore"));
    assert_eq!(
        v.restore(&RestoreParams {
            id: e.id.clone(),
            commit: "--evil".into()
        })
        .unwrap_err()
        .code(),
        ErrorCode::InvalidParams
    );

    // Delete → trash → restore.
    v.delete_entry(&e.id).unwrap();
    let h3 = v.history(&e.id).unwrap();
    assert!(h3[0].deleted);
    let trash = v.trash(&TrashParams { profile_id: None }).unwrap();
    assert_eq!(trash.len(), 1);
    assert_eq!(trash[0].id, e.id);
    assert_eq!(trash[0].name, "env");
    assert_eq!(trash[0].entry_type, EntryType::Env);
    assert!(
        v.trash(&TrashParams {
            profile_id: Some(latch::core::ids::new_id())
        })
        .unwrap()
        .is_empty()
    );
    v.restore(&RestoreParams {
        id: e.id.clone(),
        commit: trash[0].commit.clone(),
    })
    .unwrap();
    assert!(
        v.trash(&TrashParams { profile_id: None })
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        v.reveal(&RevealParams {
            id: e.id.clone(),
            field: "vars.A".into()
        })
        .unwrap()
        .value,
        "secret-one"
    );

    // Deleting a profile puts its entries in the trash; restore recreates it.
    let p2 = v.create_profile(&profile_input("gone")).unwrap();
    let e2 = v
        .create_entry(&entry_input(
            &p2.id,
            "k",
            EntryPayload::ApiKey(ApiKeyData {
                key: Some("k1".into()),
                secret: None,
                endpoint: None,
                expires_at: None,
            }),
        ))
        .unwrap();
    v.delete_profile(&p2.id).unwrap();
    let trash = v.trash(&TrashParams { profile_id: None }).unwrap();
    let item = trash.iter().find(|i| i.id == e2.id).unwrap();
    let restored = v
        .restore(&RestoreParams {
            id: e2.id.clone(),
            commit: item.commit.clone(),
        })
        .unwrap();
    assert_eq!(restored.profile_id, p2.id);
    assert_eq!(v.get_profile(&p2.id).unwrap().name, "gone");
    let r = v
        .doctor(&DoctorParams {
            fix: false,
            rebuild_manifest: false,
            staged: false,
        })
        .unwrap();
    assert!(r.ok, "{:?}", r.issues);
}

#[test]
fn history_follows_moves() {
    let t = tempfile::tempdir().unwrap();
    let mut v = new_vault(&t.path().join("v"));
    let a = v.create_profile(&profile_input("a")).unwrap();
    let b = v.create_profile(&profile_input("b")).unwrap();
    let e = v
        .create_entry(&entry_input(
            &a.id,
            "s",
            EntryPayload::Secret(SecretData {
                value: Some("x".into()),
            }),
        ))
        .unwrap();
    v.move_entry(&MoveEntryParams {
        id: e.id.clone(),
        profile_id: b.id.clone(),
    })
    .unwrap();
    let h = v.history(&e.id).unwrap();
    assert_eq!(h.len(), 2);
    assert!(!h[0].deleted);
    assert_eq!(h[0].changed_fields, vec!["profileId"]);
    assert!(
        v.trash(&TrashParams { profile_id: None })
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        v.history(&latch::core::ids::new_id()).unwrap_err().code(),
        ErrorCode::NotFound
    );
}
