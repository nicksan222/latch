use super::*;
use latch_rpc::v1;

fn entry(profile_id: &str, name: &str, data: v1::entry_payload::Data) -> v1::EntryInput {
    v1::EntryInput {
        profile_id: profile_id.into(),
        name: name.into(),
        payload: Some(v1::EntryPayload { data: Some(data) }),
        ..Default::default()
    }
}

fn id(id: &str) -> v1::IdParams {
    v1::IdParams { id: id.into() }
}

#[tokio::test]
async fn env_parse_selection_format_and_copy() {
    let mut h = Harness::start().await;
    h.init().await;
    let p = h.profile("environment").await;
    let parsed = h
        .client
        .env_parse(request(v1::EnvParseParams {
            text: "A=one\nB='two words'\n".into(),
        }))
        .await
        .unwrap()
        .into_inner();
    assert_eq!(parsed.items.len(), 2);
    assert_eq!(parsed.items[1].value.as_deref(), Some("two words"));
    expect_error(
        h.client
            .env_parse(request(v1::EnvParseParams {
                text: "bad key=value".into(),
            }))
            .await
            .unwrap_err(),
        v1::ErrorCode::InvalidParams,
    );

    let first = h
        .client
        .entries_create(request(entry(
            &p.id,
            "base",
            v1::entry_payload::Data::Env(v1::EnvData { vars: parsed.items }),
        )))
        .await
        .unwrap()
        .into_inner();
    let second = h
        .client
        .entries_create(request(entry(
            &p.id,
            "override",
            v1::entry_payload::Data::Env(v1::EnvData {
                vars: vec![v1::EnvVar {
                    key: "B".into(),
                    value: Some(SECRET.into()),
                    hidden: true,
                    comment: None,
                }],
            }),
        )))
        .await
        .unwrap()
        .into_inner();
    let all = h
        .client
        .env_resolve(request(v1::EnvResolveParams {
            profile_id: p.id.clone(),
            entry_ids: None,
        }))
        .await
        .unwrap()
        .into_inner();
    assert_eq!(all.vars.len(), 2);
    assert_eq!(
        all.vars.iter().find(|v| v.key == "B").unwrap().value,
        SECRET
    );
    let none = h
        .client
        .env_resolve(request(v1::EnvResolveParams {
            profile_id: p.id.clone(),
            entry_ids: Some(v1::OptionalStringList { items: vec![] }),
        }))
        .await
        .unwrap()
        .into_inner();
    assert!(none.vars.is_empty());
    let selected = h
        .client
        .env_resolve(request(v1::EnvResolveParams {
            profile_id: p.id.clone(),
            entry_ids: Some(v1::OptionalStringList {
                items: vec![second.id.clone(), first.id.clone()],
            }),
        }))
        .await
        .unwrap()
        .into_inner();
    assert_eq!(
        selected.vars.iter().find(|v| v.key == "B").unwrap().value,
        "two words"
    );
    let params = v1::EnvFormatParams {
        profile_id: p.id.clone(),
        entry_ids: None,
        format: v1::EnvFormat::Dotenv as i32,
    };
    let formatted = h
        .client
        .env_format(request(params.clone()))
        .await
        .unwrap()
        .into_inner();
    assert!(formatted.text.contains(SECRET));
    for (format, marker) in [
        (v1::EnvFormat::Export, "export B='"),
        (v1::EnvFormat::Json, "\"B\": \""),
    ] {
        let text = h
            .client
            .env_format(request(v1::EnvFormatParams {
                profile_id: p.id.clone(),
                entry_ids: None,
                format: format as i32,
            }))
            .await
            .unwrap()
            .into_inner()
            .text;
        assert!(text.contains(marker) && text.contains(SECRET));
    }
    let copied = h
        .client
        .env_copy(request(params))
        .await
        .unwrap()
        .into_inner();
    assert!(copied.clear_after_secs > 0);
    expect_error(
        h.client
            .env_format(request(v1::EnvFormatParams {
                profile_id: p.id.clone(),
                entry_ids: None,
                format: 999,
            }))
            .await
            .unwrap_err(),
        v1::ErrorCode::InvalidParams,
    );
    expect_error(
        h.client
            .env_resolve(request(v1::EnvResolveParams {
                profile_id: p.id,
                entry_ids: Some(v1::OptionalStringList {
                    items: vec!["missing".into()],
                }),
            }))
            .await
            .unwrap_err(),
        v1::ErrorCode::NotFound,
    );
}

#[tokio::test]
async fn totp_and_clipboard_are_typed_and_validate_fields() {
    let mut h = Harness::start().await;
    h.init().await;
    let p = h.profile("codes").await;
    let parsed = h
        .client
        .totp_parse(request(v1::EnvParseParams {
            text: "otpauth://totp/Example:user?secret=JBSWY3DPEHPK3PXP&issuer=Example".into(),
        }))
        .await
        .unwrap()
        .into_inner();
    assert_eq!(parsed.secret.as_deref(), Some("JBSWY3DPEHPK3PXP"));
    expect_error(
        h.client
            .totp_parse(request(v1::EnvParseParams {
                text: "not a valid totp secret!".into(),
            }))
            .await
            .unwrap_err(),
        v1::ErrorCode::InvalidParams,
    );
    let mut input = entry(
        &p.id,
        "login",
        v1::entry_payload::Data::Login(v1::LoginData {
            username: "fictional-user".into(),
            password: Some(SECRET.into()),
            totp: Some(parsed),
            ..Default::default()
        }),
    );
    input.favorite = true;
    let login = h
        .client
        .entries_create(request(input))
        .await
        .unwrap()
        .into_inner();
    let code = h
        .client
        .totp_get(request(id(&login.id)))
        .await
        .unwrap()
        .into_inner();
    assert_eq!(code.code.len(), 6);
    assert!(code.code.chars().all(|c| c.is_ascii_digit()) && code.expires_in <= 30);
    let pinned = h
        .client
        .totp_pinned(request(v1::Empty {}))
        .await
        .unwrap()
        .into_inner();
    assert!(pinned.items.iter().any(|e| e.id == login.id));
    let copied = h
        .client
        .clipboard_copy(request(v1::ClipboardCopyParams {
            id: login.id.clone(),
            field: "password".into(),
            clear_after_secs: Some(2),
        }))
        .await
        .unwrap()
        .into_inner();
    assert_eq!(copied.clear_after_secs, 2);
    expect_error(
        h.client
            .clipboard_copy(request(v1::ClipboardCopyParams {
                id: login.id,
                field: "does_not_exist".into(),
                clear_after_secs: Some(2),
            }))
            .await
            .unwrap_err(),
        v1::ErrorCode::NotFound,
    );
}

#[tokio::test]
async fn encrypted_archive_round_trip_and_invalid_import() {
    let mut h = Harness::start().await;
    h.init().await;
    let p = h.profile("source").await;
    h.client
        .entries_create(request(entry(
            &p.id,
            "token",
            v1::entry_payload::Data::Secret(v1::SecretData {
                value: Some(SECRET.into()),
            }),
        )))
        .await
        .unwrap();
    let archive = h.path("backup.latch.age");
    let export = h
        .client
        .export_run(request(v1::ExportParams {
            format: v1::ExportFormat::LatchArchive as i32,
            path: archive.display().to_string(),
            profile_id: Some(p.id.clone()),
            passphrase: Some(PASS.into()),
        }))
        .await
        .unwrap()
        .into_inner();
    assert_eq!(export.entries, 1);
    let bytes = std::fs::read(&archive).unwrap();
    assert!(bytes.starts_with(b"age-encryption.org/v1"));
    assert!(!String::from_utf8_lossy(&bytes).contains(SECRET));
    expect_error(
        h.client
            .export_run(request(v1::ExportParams {
                format: v1::ExportFormat::Json as i32,
                path: h.path("vault/forbidden.json").display().to_string(),
                profile_id: Some(p.id.clone()),
                passphrase: None,
            }))
            .await
            .unwrap_err(),
        v1::ErrorCode::Denied,
    );
    let target = h.profile("target").await;
    let mut import = v1::ImportParams {
        format: v1::ImportFormat::LatchArchive as i32,
        path: archive.display().to_string(),
        profile_id: Some(target.id.clone()),
        skip_duplicates: true,
        passphrase: Some("wrong passphrase".into()),
    };
    expect_error(
        h.client
            .import_preview(request(import.clone()))
            .await
            .unwrap_err(),
        v1::ErrorCode::BadCredentials,
    );
    import.passphrase = Some(PASS.into());
    let preview = h
        .client
        .import_preview(request(import.clone()))
        .await
        .unwrap()
        .into_inner();
    assert_eq!(preview.items.len(), 1);
    let applied = h
        .client
        .import_apply(request(import.clone()))
        .await
        .unwrap()
        .into_inner();
    assert_eq!(applied.created, 1);
    let second = h
        .client
        .import_apply(request(import))
        .await
        .unwrap()
        .into_inner();
    assert_eq!(second.skipped, 1);
    let listed = h
        .client
        .entries_list(request(v1::ListEntriesParams {
            profile_id: Some(target.id),
            ..Default::default()
        }))
        .await
        .unwrap()
        .into_inner();
    assert_eq!(listed.items.len(), 1);
    let revealed = h
        .client
        .entries_reveal(request(v1::RevealParams {
            id: listed.items[0].id.clone(),
            field: "value".into(),
        }))
        .await
        .unwrap()
        .into_inner();
    assert_eq!(revealed.value, SECRET);
}

#[tokio::test]
async fn each_supported_import_format_previews_and_applies_over_grpc() {
    let mut h = Harness::start().await;
    h.init().await;
    // The pass importer invokes gpg. This fixture replaces decryption only:
    // the importer still traverses the real pass-store files and parses the
    // output through the external agent process.
    let gpg = h.path("bin/gpg");
    std::fs::write(
        &gpg,
        "#!/bin/sh\nprintf 'fictional-pass\\nusername: fixture\\nurl: https://example.test/\\n'\n",
    )
    .unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&gpg, std::fs::Permissions::from_mode(0o700)).unwrap();
    let fixtures = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/import");
    let cases = [
        (v1::ImportFormat::Dotenv, "sample.env", None),
        (v1::ImportFormat::Bitwarden, "bitwarden.json", None),
        (v1::ImportFormat::OnePasswordCsv, "1password.csv", None),
        (v1::ImportFormat::KeepassXml, "keepass.xml", None),
        (v1::ImportFormat::BrowserCsv, "chrome.csv", None),
        (v1::ImportFormat::Pass, "pass-store", None),
        (v1::ImportFormat::Json, "generic.json", None),
        (v1::ImportFormat::Yaml, "generic.yaml", None),
        (
            v1::ImportFormat::LatchArchive,
            "archive.latch.age",
            Some("fixture passphrase"),
        ),
    ];
    for (index, (format, file, passphrase)) in cases.into_iter().enumerate() {
        let profile = h.profile(&format!("import-{index}")).await;
        let mut params = v1::ImportParams {
            format: format as i32,
            path: fixtures.join(file).display().to_string(),
            profile_id: Some(profile.id.clone()),
            skip_duplicates: true,
            passphrase: passphrase.map(str::to_owned),
        };
        let preview = h
            .client
            .import_preview(request(params.clone()))
            .await
            .unwrap()
            .into_inner();
        assert!(
            !preview.items.is_empty(),
            "{} preview unexpectedly empty",
            format.as_str_name()
        );
        assert!(
            preview
                .items
                .iter()
                .all(|item| item.profile_name == profile.name)
        );
        let applied = h
            .client
            .import_apply(request(params.clone()))
            .await
            .unwrap()
            .into_inner();
        assert!(
            applied.created > 0,
            "{} created no entries",
            format.as_str_name()
        );
        let listed = h
            .client
            .entries_list(request(v1::ListEntriesParams {
                profile_id: Some(profile.id.clone()),
                ..Default::default()
            }))
            .await
            .unwrap()
            .into_inner();
        assert_eq!(
            listed.items.len(),
            applied.created as usize,
            "{} entry count",
            format.as_str_name()
        );
        if format == v1::ImportFormat::Dotenv {
            let skipped = h
                .client
                .import_apply(request(params.clone()))
                .await
                .unwrap()
                .into_inner();
            assert_eq!(skipped.created, 0);
            assert_eq!(skipped.skipped, applied.created);
            params.skip_duplicates = false;
            let copied = h
                .client
                .import_apply(request(params))
                .await
                .unwrap()
                .into_inner();
            assert_eq!(copied.created, 1);
            let names = h
                .client
                .entries_list(request(v1::ListEntriesParams {
                    profile_id: Some(profile.id),
                    ..Default::default()
                }))
                .await
                .unwrap()
                .into_inner()
                .items
                .into_iter()
                .map(|entry| entry.name)
                .collect::<Vec<_>>();
            assert!(names.contains(&"sample.env (2)".to_owned()));
        }
    }
    expect_error(
        h.client
            .import_preview(request(v1::ImportParams {
                format: 999,
                path: fixtures.join("sample.env").display().to_string(),
                ..Default::default()
            }))
            .await
            .unwrap_err(),
        v1::ErrorCode::InvalidParams,
    );
}

#[tokio::test]
async fn every_export_format_writes_only_to_the_chosen_external_path() {
    let mut h = Harness::start().await;
    h.init().await;
    let p = h.profile("exports").await;
    h.client
        .entries_create(request(entry(
            &p.id,
            "env",
            v1::entry_payload::Data::Env(v1::EnvData {
                vars: vec![v1::EnvVar {
                    key: "TOKEN".into(),
                    value: Some(SECRET.into()),
                    hidden: true,
                    comment: None,
                }],
            }),
        )))
        .await
        .unwrap();
    h.client
        .entries_create(request(entry(
            &p.id,
            "secret",
            v1::entry_payload::Data::Secret(v1::SecretData {
                value: Some("different-fictional-secret".into()),
            }),
        )))
        .await
        .unwrap();
    for (format, file, expected_count) in [
        (v1::ExportFormat::Json, "export.json", 2),
        (v1::ExportFormat::Dotenv, "export.env", 1),
        (v1::ExportFormat::LatchArchive, "export.age", 2),
    ] {
        let path = h.path(file);
        let result = h
            .client
            .export_run(request(v1::ExportParams {
                format: format as i32,
                path: path.display().to_string(),
                profile_id: Some(p.id.clone()),
                passphrase: (format == v1::ExportFormat::LatchArchive).then(|| PASS.to_owned()),
            }))
            .await
            .unwrap()
            .into_inner();
        assert_eq!(
            result.entries,
            expected_count,
            "{} export count",
            format.as_str_name()
        );
        assert_eq!(result.path, path.display().to_string());
        let bytes = std::fs::read(&path).unwrap();
        if format == v1::ExportFormat::LatchArchive {
            assert!(bytes.starts_with(b"age-encryption.org/v1"));
            assert!(!String::from_utf8_lossy(&bytes).contains(SECRET));
        } else {
            let plaintext = String::from_utf8(bytes).unwrap();
            assert!(
                plaintext.contains(SECRET),
                "{} omitted env value",
                format.as_str_name()
            );
            if format == v1::ExportFormat::Dotenv {
                assert!(!plaintext.contains("different-fictional-secret"));
            }
        }
    }
    expect_error(
        h.client
            .export_run(request(v1::ExportParams {
                format: 999,
                path: h.path("unknown.out").display().to_string(),
                profile_id: Some(p.id),
                passphrase: None,
            }))
            .await
            .unwrap_err(),
        v1::ErrorCode::InvalidParams,
    );
    assert!(!h.path("unknown.out").exists());
}

#[tokio::test]
async fn recipient_add_remove_rotate_and_locked_access() {
    let mut h = Harness::start().await;
    h.init().await;
    let identity = age::x25519::Identity::generate();
    let key = identity.to_public().to_string();
    let initial = h
        .client
        .recipients_list(request(v1::Empty {}))
        .await
        .unwrap()
        .into_inner();
    let added = h
        .client
        .recipients_add(request(v1::AddRecipientParams {
            label: "second device".into(),
            public_key: key.clone(),
        }))
        .await
        .unwrap()
        .into_inner();
    assert_eq!(added.items.len(), initial.items.len() + 1);
    expect_error(
        h.client
            .recipients_add(request(v1::AddRecipientParams {
                label: "second device".into(),
                public_key: key,
            }))
            .await
            .unwrap_err(),
        v1::ErrorCode::AlreadyExists,
    );
    expect_error(
        h.client
            .recipients_add(request(v1::AddRecipientParams {
                label: "bad".into(),
                public_key: "not-an-age-key".into(),
            }))
            .await
            .unwrap_err(),
        v1::ErrorCode::InvalidParams,
    );
    let rotated = h
        .client
        .recipients_rotate(request(v1::Empty {}))
        .await
        .unwrap()
        .into_inner();
    assert_eq!(rotated.items.len(), added.items.len());
    let removed = h
        .client
        .recipients_remove(request(v1::RemoveRecipientParams {
            label: "second device".into(),
        }))
        .await
        .unwrap()
        .into_inner();
    assert_eq!(removed.items.len(), initial.items.len());
    expect_error(
        h.client
            .recipients_remove(request(v1::RemoveRecipientParams {
                label: "second device".into(),
            }))
            .await
            .unwrap_err(),
        v1::ErrorCode::NotFound,
    );
    h.client.vault_lock(request(v1::Empty {})).await.unwrap();
    expect_error(
        h.client
            .recipients_list(request(v1::Empty {}))
            .await
            .unwrap_err(),
        v1::ErrorCode::Locked,
    );
}

#[tokio::test]
async fn actions_validate_local_policy_and_persist_definitions() {
    let mut h = Harness::start().await;
    h.init().await;
    let p = h.profile("actions").await;
    let action = h
        .client
        .actions_save(request(v1::ActionSaveParams {
            profile_id: p.id.clone(),
            id: None,
            input: Some(v1::ActionInput {
                name: "Build".into(),
                command: "printf action-ok > action-ran".into(),
                ..Default::default()
            }),
        }))
        .await
        .unwrap()
        .into_inner();
    assert!(!action.id.is_empty());
    let listed = h
        .client
        .actions_list(request(id(&p.id)))
        .await
        .unwrap()
        .into_inner();
    assert_eq!(listed.actions.len(), 1);
    assert!(listed.working_directory.is_none());
    expect_error(
        h.client
            .actions_run(request(v1::ActionIdParams {
                profile_id: p.id.clone(),
                action_id: action.id.clone(),
            }))
            .await
            .unwrap_err(),
        v1::ErrorCode::InvalidParams,
    );
    expect_error(
        h.client
            .actions_configure(request(v1::ActionConfigureParams {
                profile_id: p.id.clone(),
                working_directory: Some(h.path("vault").display().to_string()),
            }))
            .await
            .unwrap_err(),
        v1::ErrorCode::InvalidParams,
    );
    let project = h.path("project");
    std::fs::create_dir_all(&project).unwrap();
    h.client
        .actions_configure(request(v1::ActionConfigureParams {
            profile_id: p.id.clone(),
            working_directory: Some(project.display().to_string()),
        }))
        .await
        .unwrap();
    let configured = h
        .client
        .actions_list(request(id(&p.id)))
        .await
        .unwrap()
        .into_inner();
    assert_eq!(
        configured.working_directory.as_deref(),
        Some(project.to_str().unwrap())
    );
    h.client
        .actions_run(request(v1::ActionIdParams {
            profile_id: p.id.clone(),
            action_id: action.id.clone(),
        }))
        .await
        .unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while !project.join("action-ran").exists() {
        assert!(
            std::time::Instant::now() < deadline,
            "action process did not run"
        );
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    h.client
        .actions_delete(request(v1::ActionIdParams {
            profile_id: p.id.clone(),
            action_id: action.id.clone(),
        }))
        .await
        .unwrap();
    assert!(
        h.client
            .actions_list(request(id(&p.id)))
            .await
            .unwrap()
            .into_inner()
            .actions
            .is_empty()
    );
}

#[tokio::test]
async fn browser_isolation_archive_restore_and_reset_over_socket() {
    let mut h = Harness::start().await;
    h.init().await;
    let p = h.profile("browser").await;
    let detected = h
        .client
        .browser_detect(request(v1::Empty {}))
        .await
        .unwrap()
        .into_inner();
    // The public catalog is the managed cache, never a list of host binaries.
    assert!(detected.items.is_empty());
    let launch = v1::BrowserLaunchParams {
        profile_id: p.id.clone(),
        url: Some("https://example.test/".into()),
        engine: None,
    };
    let opened = h
        .client
        .browser_launch(request(launch.clone()))
        .await
        .unwrap()
        .into_inner();
    assert_eq!(opened.profile_id, p.id);
    assert!(opened.pid > 0);
    let again = h
        .client
        .browser_launch(request(v1::BrowserLaunchParams {
            profile_id: p.id.clone(),
            url: None,
            engine: None,
        }))
        .await
        .unwrap()
        .into_inner();
    assert_eq!(again.pid, opened.pid);
    let list = h
        .client
        .browser_list(request(v1::Empty {}))
        .await
        .unwrap()
        .into_inner();
    assert_eq!(list.items.len(), 1);
    assert!(
        !h.client
            .browser_session_info(request(id(&p.id)))
            .await
            .unwrap()
            .into_inner()
            .exists
    );
    h.client.browser_close(request(id(&p.id))).await.unwrap();
    assert!(
        h.client
            .browser_list(request(v1::Empty {}))
            .await
            .unwrap()
            .into_inner()
            .items
            .is_empty()
    );
    let archived = h
        .client
        .browser_session_info(request(id(&p.id)))
        .await
        .unwrap()
        .into_inner();
    assert!(archived.exists && archived.size_bytes > 0);
    let restored = h
        .client
        .browser_launch(request(launch))
        .await
        .unwrap()
        .into_inner();
    assert!(restored.pid > 0);
    h.client.browser_close(request(id(&p.id))).await.unwrap();
    h.client.browser_reset(request(id(&p.id))).await.unwrap();
    assert!(
        !h.client
            .browser_session_info(request(id(&p.id)))
            .await
            .unwrap()
            .into_inner()
            .exists
    );
    expect_error(
        h.client
            .browser_launch(request(v1::BrowserLaunchParams {
                profile_id: p.id,
                url: Some("javascript:alert(1)".into()),
                engine: None,
            }))
            .await
            .unwrap_err(),
        v1::ErrorCode::InvalidParams,
    );
}
