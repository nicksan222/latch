// Secrets acceptance cases; shared imports/helpers come from core_vault_support.rs.

#[test]
fn every_entry_type_round_trips() {
    let t = tempfile::tempdir().unwrap();
    let path = t.path().join("v");
    let mut v = new_vault(&path);
    let p = v.create_profile(&profile_input("acme")).unwrap();
    for (name, payload) in all_payloads() {
        let created = v
            .create_entry(&entry_input(&p.id, name, payload.clone()))
            .unwrap();
        assert!(!created.redacted_fields.is_empty(), "{name}");
        let full = v
            .get_entry(&GetEntryParams {
                id: created.id.clone(),
                reveal: true,
            })
            .unwrap();
        assert!(full.redacted_fields.is_empty());
        assert_eq!(full.name, name);
        assert_eq!(full.tags, vec!["test"]);
        assert_eq!(full.notes, "some notes");
        let mut expected = payload.clone();
        if let EntryPayload::File(f) = &mut expected {
            f.size = 20; // computed from content
        }
        assert_eq!(full.payload, expected, "{name}");
        // Persisted: reopen and read again.
        let again = unlock(&path)
            .get_entry(&GetEntryParams {
                id: created.id.clone(),
                reveal: true,
            })
            .unwrap();
        assert_eq!(again.payload, expected);
    }
    // No plaintext on disk.
    for f in walk(&path) {
        if f.components().any(|c| c.as_os_str() == ".git") {
            continue;
        }
        let s = String::from_utf8_lossy(&std::fs::read(&f).unwrap()).into_owned();
        for m in SECRET_MARKERS {
            assert!(!s.contains(m), "{m} leaked in {f:?}");
        }
        assert!(
            !s.contains("GitHub") && !s.contains("acme work"),
            "metadata leaked in {f:?}"
        );
    }
    let report = v
        .doctor(&DoctorParams {
            fix: false,
            rebuild_manifest: false,
            staged: false,
        })
        .unwrap();
    assert!(report.ok, "{:?}", report.issues);
}

#[test]
fn redaction_paths_match_protocol_docs() {
    let t = tempfile::tempdir().unwrap();
    let mut v = new_vault(&t.path().join("v"));
    let p = v.create_profile(&profile_input("acme")).unwrap();
    let mut expected = std::collections::HashMap::new();
    expected.insert("GitHub", vec!["password", "totp.secret", "custom.Recovery"]);
    expected.insert("Webhook secret", vec!["value"]);
    expected.insert("App env", vec!["vars.DATABASE_URL"]);
    expected.insert("staging.pem", vec!["content"]);
    expected.insert("Runbook", vec!["body"]);
    expected.insert("Deploy key", vec!["privateKey", "passphrase"]);
    expected.insert("Prod DB", vec!["password"]);
    expected.insert("Stripe", vec!["key", "secret"]);
    for (name, payload) in all_payloads() {
        let e = v.create_entry(&entry_input(&p.id, name, payload)).unwrap();
        let got = v
            .get_entry(&GetEntryParams {
                id: e.id.clone(),
                reveal: false,
            })
            .unwrap();
        assert_eq!(got.redacted_fields, expected[name], "{name}");
        let json = serde_json::to_string(&got).unwrap();
        for m in SECRET_MARKERS {
            assert!(!json.contains(m), "{m} in redacted {name}");
        }
        for path in &got.redacted_fields {
            let r = v
                .reveal(&RevealParams {
                    id: e.id.clone(),
                    field: path.clone(),
                })
                .unwrap();
            assert!(!r.value.is_empty(), "{name} {path}");
        }
        if name == "Prod DB" {
            let url = v
                .reveal(&RevealParams {
                    id: e.id.clone(),
                    field: "url".into(),
                })
                .unwrap();
            assert_eq!(
                url.value,
                "postgres://app:db-pass@db.internal:5432/app?sslmode=require"
            );
        }
        if name == "App env" {
            // Non-hidden values are visible without reveal.
            let EntryPayload::Env(env) = &got.payload else {
                panic!()
            };
            assert_eq!(env.vars[1].value.as_deref(), Some("3000"));
            assert_eq!(env.vars[0].value, None);
        }
        assert_eq!(
            v.reveal(&RevealParams {
                id: e.id.clone(),
                field: "nope".into()
            })
            .unwrap_err()
            .code(),
            ErrorCode::NotFound
        );
    }
}

#[test]
fn update_keeps_redacted_secrets() {
    let t = tempfile::tempdir().unwrap();
    let mut v = new_vault(&t.path().join("v"));
    let p = v.create_profile(&profile_input("acme")).unwrap();
    let (name, payload) = all_payloads().remove(0);
    let e = v.create_entry(&entry_input(&p.id, name, payload)).unwrap();
    // Client edits the redacted entry: renames, changes username and adds a url.
    let mut input = EntryInput {
        profile_id: e.profile_id.clone(),
        name: "GitHub (work)".into(),
        tags: e.tags.clone(),
        notes: e.notes.clone(),
        favorite: true,
        payload: e.payload.clone(),
    };
    if let EntryPayload::Login(l) = &mut input.payload {
        l.username = "alice2".into();
        l.urls.push("https://gist.github.com".into());
        l.custom.push(CustomField {
            key: "New".into(),
            value: Some("fresh".into()),
            hidden: true,
        });
    }
    let updated = v
        .update_entry(&EntryUpdateParams {
            id: e.id.clone(),
            input: input.clone(),
        })
        .unwrap();
    assert_eq!(updated.name, "GitHub (work)");
    let full = v
        .get_entry(&GetEntryParams {
            id: e.id.clone(),
            reveal: true,
        })
        .unwrap();
    let EntryPayload::Login(l) = full.payload else {
        panic!()
    };
    assert_eq!(l.username, "alice2");
    assert_eq!(l.password.as_deref(), Some("gh-password-1"));
    assert_eq!(l.totp.unwrap().secret.as_deref(), Some(TOTP_SECRET));
    assert_eq!(
        l.custom
            .iter()
            .find(|c| c.key == "Recovery")
            .unwrap()
            .value
            .as_deref(),
        Some("recovery-code-1")
    );
    assert_eq!(
        l.custom
            .iter()
            .find(|c| c.key == "New")
            .unwrap()
            .value
            .as_deref(),
        Some("fresh")
    );
    assert_eq!(l.urls.len(), 2);
    // Explicit new value replaces.
    if let EntryPayload::Login(l) = &mut input.payload {
        l.password = Some("new-pass".into());
    }
    v.update_entry(&EntryUpdateParams {
        id: e.id.clone(),
        input,
    })
    .unwrap();
    let r = v
        .reveal(&RevealParams {
            id: e.id.clone(),
            field: "password".into(),
        })
        .unwrap();
    assert_eq!(r.value, "new-pass");
    let s = v
        .list_entries(&ListEntriesParams {
            profile_id: None,
            query: None,
            entry_type: None,
        })
        .unwrap();
    assert!(s[0].favorite);
}

#[test]
fn entries_list_query_move_delete() {
    let t = tempfile::tempdir().unwrap();
    let mut v = new_vault(&t.path().join("v"));
    let a = v.create_profile(&profile_input("acme")).unwrap();
    let b = v.create_profile(&profile_input("side")).unwrap();
    let mut ids = std::collections::HashMap::new();
    for (name, payload) in all_payloads() {
        let e = v.create_entry(&entry_input(&a.id, name, payload)).unwrap();
        ids.insert(name, e.id);
    }
    let q = |v: &latch::core::UnlockedVault,
             pid: Option<&str>,
             q: Option<&str>,
             t: Option<EntryType>| {
        v.list_entries(&ListEntriesParams {
            profile_id: pid.map(str::to_owned),
            query: q.map(str::to_owned),
            entry_type: t,
        })
        .unwrap()
        .into_iter()
        .map(|s| s.name)
        .collect::<Vec<_>>()
    };
    assert_eq!(q(&v, None, None, None).len(), 8);
    assert_eq!(q(&v, None, Some("git"), None), vec!["GitHub"]);
    assert_eq!(q(&v, None, Some("alice"), None), vec!["GitHub"]); // subtitle
    assert_eq!(q(&v, None, Some("db.internal"), None), vec!["Prod DB"]);
    assert_eq!(q(&v, None, None, Some(EntryType::Env)), vec!["App env"]);
    assert_eq!(q(&v, Some(&b.id), None, None), Vec::<String>::new());
    let summaries = v
        .list_entries(&ListEntriesParams {
            profile_id: None,
            query: Some("env".into()),
            entry_type: None,
        })
        .unwrap();
    assert_eq!(summaries[0].subtitle.as_deref(), Some("2 vars"));
    let gh = v.find_entry(&a.id, "github").unwrap();
    assert!(gh.has_totp);

    // Duplicate names within a profile are rejected.
    let dup = entry_input(
        &a.id,
        "github",
        EntryPayload::Secret(SecretData {
            value: Some("x".into()),
        }),
    );
    assert_eq!(
        v.create_entry(&dup).unwrap_err().code(),
        ErrorCode::AlreadyExists
    );

    // Move.
    let moved = v
        .move_entry(&MoveEntryParams {
            id: ids["Stripe"].clone(),
            profile_id: b.id.clone(),
        })
        .unwrap();
    assert_eq!(moved.profile_id, b.id);
    assert_eq!(q(&v, Some(&b.id), None, None), vec!["Stripe"]);
    let r = v
        .reveal(&RevealParams {
            id: ids["Stripe"].clone(),
            field: "key".into(),
        })
        .unwrap();
    assert_eq!(r.value, "sk_test_key");
    assert!(
        !t.path()
            .join("v")
            .join(format::entry_file(&a.id, &ids["Stripe"]))
            .exists()
    );
    let msg = &v.git().log(1).unwrap()[0].message;
    assert!(
        msg.starts_with(&format!(
            "entry({}): move {} to {}",
            a.id, ids["Stripe"], b.id
        )),
        "{msg}"
    );

    // Delete.
    v.delete_entry(&ids["Runbook"]).unwrap();
    assert_eq!(
        v.get_entry(&GetEntryParams {
            id: ids["Runbook"].clone(),
            reveal: false
        })
        .unwrap_err()
        .code(),
        ErrorCode::NotFound
    );
    assert_eq!(q(&v, None, None, None).len(), 7);
    // Missing profile.
    let orphan = entry_input(
        &latch::core::ids::new_id(),
        "x",
        EntryPayload::Note(NoteData { body: None }),
    );
    assert_eq!(
        v.create_entry(&orphan).unwrap_err().code(),
        ErrorCode::NotFound
    );
}

#[test]
fn entry_size_limit() {
    let t = tempfile::tempdir().unwrap();
    let mut v = new_vault(&t.path().join("v"));
    let p = v.create_profile(&profile_input("acme")).unwrap();
    use base64::Engine;
    let big = base64::engine::general_purpose::STANDARD.encode(vec![7u8; 900 * 1024]);
    let e = v.create_entry(&entry_input(
        &p.id,
        "big",
        EntryPayload::File(FileData {
            filename: "big.bin".into(),
            mime: String::new(),
            content_base64: Some(big),
            size: 0,
        }),
    ));
    let err = e.unwrap_err();
    assert_eq!(err.code(), ErrorCode::InvalidParams);
    assert!(err.to_string().contains("1 MiB"));
    let ok = base64::engine::general_purpose::STANDARD.encode(vec![7u8; 600 * 1024]);
    v.create_entry(&entry_input(
        &p.id,
        "ok",
        EntryPayload::File(FileData {
            filename: "ok.bin".into(),
            mime: String::new(),
            content_base64: Some(ok),
            size: 0,
        }),
    ))
    .unwrap();
}

#[test]
fn errors_never_contain_secret_values() {
    let t = tempfile::tempdir().unwrap();
    let mut v = new_vault(&t.path().join("v"));
    let p = v.create_profile(&profile_input("acme")).unwrap();
    let bad_totp = EntryPayload::Login(LoginData {
        username: String::new(),
        password: Some("pw-marker-xyz".into()),
        urls: vec![],
        totp: Some(TotpConfig {
            secret: Some("!!not-base32-marker!!".into()),
            algorithm: TotpAlgorithm::Sha1,
            digits: 6,
            period: 30,
            issuer: None,
        }),
        custom: vec![],
    });
    let e = v
        .create_entry(&entry_input(&p.id, "x", bad_totp))
        .unwrap_err();
    assert!(!e.to_string().contains("marker"));
    let bad_file = EntryPayload::File(FileData {
        filename: "f".into(),
        mime: String::new(),
        content_base64: Some("@@secret-marker@@".into()),
        size: 0,
    });
    let e = v
        .create_entry(&entry_input(&p.id, "y", bad_file))
        .unwrap_err();
    assert!(!e.to_string().contains("marker"));
    let e = latch::core::environment::parse("KEY=\"unterminated secret-marker").unwrap_err();
    assert!(!e.to_string().contains("marker"));
    // Debug of an unlocked vault shows no key material.
    let dbg = format!("{v:?}");
    assert!(dbg.contains("UnlockedVault") && !dbg.contains("vault_key"));
}
