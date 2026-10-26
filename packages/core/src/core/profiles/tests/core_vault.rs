// Profiles acceptance cases; shared imports/helpers come from core_vault_support.rs.

#[test]
fn profiles_crud_and_listing() {
    let t = tempfile::tempdir().unwrap();
    let mut v = new_vault(&t.path().join("v"));
    let a = v.create_profile(&profile_input("acme")).unwrap();
    let mut pinned = profile_input("zeta");
    pinned.pinned = true;
    let z = v.create_profile(&pinned).unwrap();
    assert!(matches!(
        v.create_profile(&profile_input("ACME")),
        Err(Error::AlreadyExists(_))
    ));
    let list = v.list_profiles().unwrap();
    assert_eq!(
        list.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(),
        vec!["zeta", "acme"]
    );
    let got = v.get_profile(&a.id).unwrap();
    assert_eq!(got.browser.bookmarks[0].title, "Jira");
    let mut input = profile_input("acme-renamed");
    input.color = "#FF0000".into();
    let u = v
        .update_profile(&ProfileUpdateParams {
            id: a.id.clone(),
            input,
        })
        .unwrap();
    assert_eq!(u.name, "acme-renamed");
    assert_eq!(u.color, "#ff0000");
    assert_eq!(u.created_at, a.created_at);
    assert_eq!(v.find_profile("ACME-RENAMED").unwrap().id, a.id);
    assert_eq!(v.find_profile(&z.id).unwrap().name, "zeta");

    // Delete removes entries too.
    for (name, payload) in all_payloads().into_iter().take(3) {
        v.create_entry(&entry_input(&a.id, name, payload)).unwrap();
    }
    assert_eq!(v.find_profile(&a.id).unwrap().entry_count, 3);
    v.delete_profile(&a.id).unwrap();
    assert!(!t.path().join("v/profiles").join(&a.id).exists());
    assert_eq!(v.list_profiles().unwrap().len(), 1);
    assert!(
        v.list_entries(&ListEntriesParams {
            profile_id: None,
            query: None,
            entry_type: None
        })
        .unwrap()
        .is_empty()
    );
    assert_eq!(
        v.get_profile(&a.id).unwrap_err().code(),
        ErrorCode::NotFound
    );
    assert_eq!(
        v.delete_profile("nonsense").unwrap_err().code(),
        ErrorCode::InvalidParams
    );
    let mut bad = profile_input("x");
    bad.color = "red".into();
    assert_eq!(
        v.create_profile(&bad).unwrap_err().code(),
        ErrorCode::InvalidParams
    );
}

#[test]
fn profiles_normalize_placeholder_start_pages_on_create_and_update() {
    let t = tempfile::tempdir().unwrap();
    let mut vault = new_vault(&t.path().join("v"));
    let mut input = profile_input("placeholder-pages");
    input.browser.start_pages = vec!["".into(), "https://first.example".into(), " \t".into()];
    let created = vault.create_profile(&input).unwrap();
    assert_eq!(created.browser.start_pages, vec!["https://first.example"]);
    assert_eq!(
        vault.get_profile(&created.id).unwrap().browser.start_pages,
        created.browser.start_pages
    );
    input.browser.start_pages = vec![
        " ".into(),
        "https://second.example/path".into(),
        String::new(),
    ];
    let updated = vault
        .update_profile(&ProfileUpdateParams {
            id: created.id.clone(),
            input: input.clone(),
        })
        .unwrap();
    assert_eq!(
        updated.browser.start_pages,
        vec!["https://second.example/path"]
    );
    assert_eq!(
        vault.get_profile(&created.id).unwrap().browser.start_pages,
        updated.browser.start_pages
    );
    input.browser.start_pages = vec![String::new(), " \t".into()];
    assert!(
        vault
            .update_profile(&ProfileUpdateParams {
                id: created.id.clone(),
                input: input.clone()
            })
            .unwrap()
            .browser
            .start_pages
            .is_empty()
    );
    for (index, page) in [
        "javascript:alert(1)",
        "file:///etc/passwd",
        "ftp://example.com",
        "--no-sandbox",
    ]
    .into_iter()
    .enumerate()
    {
        input.name = format!("unsafe-{index}");
        input.browser.start_pages = vec![String::new(), page.into()];
        assert!(vault.create_profile(&input).is_err());
        assert!(
            vault
                .update_profile(&ProfileUpdateParams {
                    id: created.id.clone(),
                    input: input.clone()
                })
                .is_err()
        );
        assert!(
            vault
                .get_profile(&created.id)
                .unwrap()
                .browser
                .start_pages
                .is_empty()
        );
    }
    input.name = "too-many-pages".into();
    input.browser.start_pages = vec![String::new(); 65];
    assert!(vault.create_profile(&input).is_err());
    assert!(
        vault
            .update_profile(&ProfileUpdateParams {
                id: created.id,
                input
            })
            .is_err()
    );
}
