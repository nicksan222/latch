use super::*;
use latch_rpc::v1::{self, entry_payload::Data};

fn input(profile_id: &str, name: &str, data: Data) -> v1::EntryInput {
    v1::EntryInput {
        profile_id: profile_id.into(),
        name: name.into(),
        payload: Some(v1::EntryPayload { data: Some(data) }),
        ..Default::default()
    }
}

fn samples(profile_id: &str) -> Vec<(v1::EntryType, v1::EntryInput, &'static str)> {
    vec![
        (
            v1::EntryType::Login,
            input(
                profile_id,
                "login",
                Data::Login(v1::LoginData {
                    username: "fictional-user".into(),
                    password: Some(SECRET.into()),
                    urls: vec!["https://example.test".into()],
                    ..Default::default()
                }),
            ),
            "password",
        ),
        (
            v1::EntryType::Secret,
            input(
                profile_id,
                "secret",
                Data::Secret(v1::SecretData {
                    value: Some(SECRET.into()),
                }),
            ),
            "value",
        ),
        (
            v1::EntryType::Env,
            input(
                profile_id,
                "env",
                Data::Env(v1::EnvData {
                    vars: vec![v1::EnvVar {
                        key: "TOKEN".into(),
                        value: Some(SECRET.into()),
                        hidden: true,
                        ..Default::default()
                    }],
                }),
            ),
            "vars.TOKEN",
        ),
        (
            v1::EntryType::File,
            input(
                profile_id,
                "file",
                Data::File(v1::FileData {
                    filename: "example.txt".into(),
                    mime: "text/plain".into(),
                    content_base64: Some("ZmljdGlvbmFs".into()),
                    size: 9,
                }),
            ),
            "content",
        ),
        (
            v1::EntryType::Note,
            input(
                profile_id,
                "note",
                Data::Note(v1::NoteData {
                    body: Some(SECRET.into()),
                }),
            ),
            "body",
        ),
        (
            v1::EntryType::SshKey,
            input(
                profile_id,
                "ssh",
                Data::SshKey(v1::SshKeyData {
                    private_key: Some(SECRET.into()),
                    public_key: "ssh-ed25519 fictional-public-key".into(),
                    ..Default::default()
                }),
            ),
            "privateKey",
        ),
        (
            v1::EntryType::Database,
            input(
                profile_id,
                "database",
                Data::Database(v1::DatabaseData {
                    engine: "postgres".into(),
                    host: "example.test".into(),
                    database: "sample".into(),
                    username: "fictional-user".into(),
                    password: Some(SECRET.into()),
                    ..Default::default()
                }),
            ),
            "password",
        ),
        (
            v1::EntryType::ApiKey,
            input(
                profile_id,
                "api key",
                Data::ApiKey(v1::ApiKeyData {
                    key: Some(SECRET.into()),
                    ..Default::default()
                }),
            ),
            "key",
        ),
    ]
}

#[tokio::test]
async fn all_entry_kinds_are_masked_until_explicit_reveal_and_survive_restart() {
    let mut h = Harness::start().await;
    h.init().await;
    let p = h.profile("all kinds").await;
    let mut ids = Vec::new();
    for (kind, source, field) in samples(&p.id) {
        let created = h
            .client
            .entries_create(request(source.clone()))
            .await
            .unwrap()
            .into_inner();
        assert_eq!(created.profile_id, p.id);
        assert!(created.redacted_fields.iter().any(|path| path == field));
        let id = created.id;
        let masked = h
            .client
            .entries_get(request(v1::GetEntryParams {
                id: id.clone(),
                reveal: false,
            }))
            .await
            .unwrap()
            .into_inner();
        assert!(masked.redacted_fields.iter().any(|path| path == field));
        let revealed = h
            .client
            .entries_reveal(request(v1::RevealParams {
                id: id.clone(),
                field: field.into(),
            }))
            .await
            .unwrap()
            .into_inner();
        assert_eq!(
            revealed.value,
            if kind == v1::EntryType::File {
                "ZmljdGlvbmFs"
            } else {
                SECRET
            }
        );
        let by_type = h
            .client
            .entries_list(request(v1::ListEntriesParams {
                profile_id: Some(p.id.clone()),
                r#type: Some(kind as i32),
                ..Default::default()
            }))
            .await
            .unwrap()
            .into_inner();
        assert!(
            by_type
                .items
                .iter()
                .any(|item| item.id == id && item.r#type == kind as i32)
        );
        ids.push((id, field));
    }
    let all = h
        .client
        .entries_list(request(v1::ListEntriesParams {
            profile_id: Some(p.id.clone()),
            ..Default::default()
        }))
        .await
        .unwrap()
        .into_inner();
    assert_eq!(all.items.len(), 8);
    h.restart().await;
    h.client
        .vault_unlock(request(v1::UnlockParams {
            passphrase: Some(PASS.into()),
            ..Default::default()
        }))
        .await
        .unwrap();
    for (id, field) in ids {
        assert_eq!(
            h.client
                .entries_reveal(request(v1::RevealParams {
                    id,
                    field: field.into()
                }))
                .await
                .unwrap()
                .into_inner()
                .value,
            if field == "content" {
                "ZmljdGlvbmFs"
            } else {
                SECRET
            }
        );
    }
}

#[tokio::test]
async fn update_history_move_trash_restore_and_rejected_write_are_atomic() {
    let mut h = Harness::start().await;
    h.init().await;
    let source = h.profile("source").await;
    let destination = h.profile("destination").await;
    let created = h
        .client
        .entries_create(request(input(
            &source.id,
            "account",
            Data::Login(v1::LoginData {
                username: "fictional-user".into(),
                password: Some(SECRET.into()),
                ..Default::default()
            }),
        )))
        .await
        .unwrap()
        .into_inner();
    let entry_id = created.id;

    let draft = input(
        &source.id,
        "renamed",
        Data::Login(v1::LoginData {
            username: "fictional-user".into(),
            password: None,
            ..Default::default()
        }),
    );
    h.client
        .entries_update(request(v1::EntryUpdateParams {
            id: entry_id.clone(),
            input: Some(draft.clone()),
        }))
        .await
        .unwrap();
    assert_eq!(
        h.client
            .entries_reveal(request(v1::RevealParams {
                id: entry_id.clone(),
                field: "password".into()
            }))
            .await
            .unwrap()
            .into_inner()
            .value,
        SECRET
    );

    let mut invalid = draft.clone();
    invalid.name = "invalid/name".into();
    invalid.payload = Some(v1::EntryPayload {
        data: Some(Data::Login(v1::LoginData {
            username: "fictional-user".into(),
            password: Some("fictional-rejected-value".into()),
            ..Default::default()
        })),
    });
    expect_error(
        h.client
            .entries_update(request(v1::EntryUpdateParams {
                id: entry_id.clone(),
                input: Some(invalid),
            }))
            .await
            .unwrap_err(),
        v1::ErrorCode::InvalidParams,
    );
    let after = h
        .client
        .entries_get(request(v1::GetEntryParams {
            id: entry_id.clone(),
            reveal: false,
        }))
        .await
        .unwrap()
        .into_inner();
    assert_eq!(after.name, "renamed");
    assert_eq!(
        h.client
            .entries_reveal(request(v1::RevealParams {
                id: entry_id.clone(),
                field: "password".into()
            }))
            .await
            .unwrap()
            .into_inner()
            .value,
        SECRET
    );

    let clear = input(
        &source.id,
        "renamed",
        Data::Login(v1::LoginData {
            username: "fictional-user".into(),
            password: Some(String::new()),
            ..Default::default()
        }),
    );
    h.client
        .entries_update(request(v1::EntryUpdateParams {
            id: entry_id.clone(),
            input: Some(clear),
        }))
        .await
        .unwrap();
    expect_error(
        h.client
            .entries_reveal(request(v1::RevealParams {
                id: entry_id.clone(),
                field: "password".into(),
            }))
            .await
            .unwrap_err(),
        v1::ErrorCode::NotFound,
    );
    let history = h
        .client
        .entries_history(request(v1::IdParams {
            id: entry_id.clone(),
        }))
        .await
        .unwrap()
        .into_inner();
    assert!(history.items.len() >= 3);
    let previous = &history.items[1];
    h.client
        .entries_restore(request(v1::RestoreParams {
            id: entry_id.clone(),
            commit: previous.commit.clone(),
        }))
        .await
        .unwrap();
    assert_eq!(
        h.client
            .entries_reveal(request(v1::RevealParams {
                id: entry_id.clone(),
                field: "password".into()
            }))
            .await
            .unwrap()
            .into_inner()
            .value,
        SECRET
    );

    let moved = h
        .client
        .entries_move(request(v1::MoveEntryParams {
            id: entry_id.clone(),
            profile_id: destination.id.clone(),
        }))
        .await
        .unwrap()
        .into_inner();
    assert_eq!(moved.profile_id, destination.id);
    h.client
        .entries_delete(request(v1::IdParams {
            id: entry_id.clone(),
        }))
        .await
        .unwrap();
    expect_error(
        h.client
            .entries_get(request(v1::GetEntryParams {
                id: entry_id.clone(),
                reveal: false,
            }))
            .await
            .unwrap_err(),
        v1::ErrorCode::NotFound,
    );
    let trash = h
        .client
        .entries_trash(request(v1::TrashParams {
            profile_id: Some(destination.id.clone()),
        }))
        .await
        .unwrap()
        .into_inner();
    let tombstone = trash
        .items
        .iter()
        .find(|item| item.id == entry_id)
        .expect("deleted entry in trash");
    h.client
        .entries_restore(request(v1::RestoreParams {
            id: entry_id.clone(),
            commit: tombstone.commit.clone(),
        }))
        .await
        .unwrap();
    assert_eq!(
        h.client
            .entries_get(request(v1::GetEntryParams {
                id: entry_id,
                reveal: false
            }))
            .await
            .unwrap()
            .into_inner()
            .profile_id,
        destination.id
    );
}
