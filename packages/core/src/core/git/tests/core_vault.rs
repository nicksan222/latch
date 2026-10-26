// Git acceptance cases; shared imports/helpers come from core_vault_support.rs.

#[test]
fn commit_messages_respect_names_in_commits() {
    let t = tempfile::tempdir().unwrap();
    let mut v = new_vault(&t.path().join("v"));
    let p = v.create_profile(&profile_input("acme")).unwrap();
    let e = v
        .create_entry(&entry_input(
            &p.id,
            "GitHub",
            EntryPayload::Secret(SecretData {
                value: Some("s3cr3t-value".into()),
            }),
        ))
        .unwrap();
    let log = v.git().log(10).unwrap();
    assert_eq!(log[0].message, format!("entry({}): create {}", p.id, e.id));
    assert_eq!(log[1].message, format!("profile({}): create", p.id));
    v.set_vault_settings(true, false, false).unwrap();
    assert!(v.config().sync.names_in_commits);
    let mut input = entry_input(
        &p.id,
        "GitHub",
        EntryPayload::Secret(SecretData { value: None }),
    );
    input.notes = "changed".into();
    v.update_entry(&EntryUpdateParams {
        id: e.id.clone(),
        input,
    })
    .unwrap();
    assert_eq!(
        v.git().log(1).unwrap()[0].message,
        "entry(acme): update GitHub"
    );
    for c in v.git().log(50).unwrap() {
        assert!(!c.message.contains("s3cr3t"));
    }
    // Settings survive reopen and do not break the manifest binding.
    let u = unlock(&t.path().join("v"));
    assert!(u.integrity_problems().is_empty());
    assert!(u.config().sync.names_in_commits);
}
