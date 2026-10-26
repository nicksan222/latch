// One contract, exercised through both the embedded and authenticated socket transports.
#[path = "../../../tests/agent_support/mod.rs"]
mod agent_support;

use std::sync::Mutex;

use agent_support::{PASS, TestAgent, profile_input};
use latch::core::crypto;
use latch::rpc::*;

static SOCKET_SERIAL: Mutex<()> = Mutex::new(());
const SECRET: &str = "fictional-password-one";
const SECRET_TWO: &str = "fictional-password-two";

fn id(id: &str) -> IdParams {
    IdParams { id: id.into() }
}

fn login(profile_id: &str, name: &str, password: Option<&str>) -> EntryInput {
    EntryInput {
        profile_id: profile_id.into(),
        name: name.into(),
        tags: vec![],
        notes: String::new(),
        favorite: false,
        payload: EntryPayload::Login(LoginData {
            username: "fictional-user".into(),
            password: password.map(str::to_owned),
            urls: vec!["https://example.test".into()],
            totp: None,
            custom: vec![],
        }),
    }
}

fn revealed_password(entry: &Entry) -> Option<&str> {
    match &entry.payload {
        EntryPayload::Login(data) => data.password.as_deref(),
        _ => panic!("expected login entry"),
    }
}

fn init<C: RpcSdk<Error = RpcError>>(client: &mut C, a: &TestAgent) {
    let initial = client.vault_status().unwrap();
    assert!(!initial.initialized && initial.locked);
    assert_eq!(
        client.profiles_list().err().unwrap().code,
        ErrorCode::NoVault
    );
    let result = client
        .vault_init(InitParams {
            path: a.vault_path().display().to_string(),
            passphrase: PASS.into(),
            kdf: Some(crypto::WEAK_KDF),
            git_name: Some("Alice".into()),
            git_email: Some("alice@example.test".into()),
            remote_url: None,
            recovery_kit: false,
        })
        .unwrap();
    assert!(result.status.initialized && !result.status.locked);
}

fn scenario<C: RpcSdk<Error = RpcError>>(client: &mut C, a: &TestAgent) -> (String, String) {
    init(client, a);
    let malformed = client
        .request::<_, serde_json::Value>(
            "profiles.get",
            serde_json::json!({ "id": 42, "secretish": SECRET }),
        )
        .err()
        .unwrap();
    assert_eq!(malformed.code, ErrorCode::InvalidParams);
    assert!(!malformed.message.contains(SECRET));
    let unknown = client
        .request::<_, serde_json::Value>("unknown.method", ())
        .err()
        .unwrap();
    assert_eq!(unknown.code, ErrorCode::InvalidParams);

    // Profile validation, uniqueness, rename, and deletion use the same typed API.
    let mut invalid = profile_input("");
    assert_eq!(
        client.profiles_create(invalid.clone()).err().unwrap().code,
        ErrorCode::InvalidParams
    );
    invalid.name = "work/project".into();
    assert_eq!(
        client.profiles_create(invalid).err().unwrap().code,
        ErrorCode::InvalidParams
    );
    assert!(client.profiles_list().unwrap().is_empty());
    let profile = client.profiles_create(profile_input("work")).unwrap();
    assert_eq!(
        client
            .profiles_create(profile_input("work"))
            .err()
            .unwrap()
            .code,
        ErrorCode::AlreadyExists
    );
    let mut renamed = profile_input("renamed");
    renamed.pinned = true;
    let updated = client
        .profiles_update(ProfileUpdateParams {
            id: profile.id.clone(),
            input: renamed,
        })
        .unwrap();
    assert_eq!(updated.id, profile.id);
    assert_eq!(updated.name, "renamed");
    assert!(client.profiles_get(id(&profile.id)).unwrap().pinned);
    assert_eq!(client.profiles_list().unwrap().len(), 1);

    // Secret values remain masked until a caller explicitly reveals them.
    let entry = client
        .entries_create(login(&profile.id, "Account", Some(SECRET)))
        .unwrap();
    assert!(revealed_password(&entry).is_none());
    assert!(entry.redacted_fields.iter().any(|f| f == "password"));
    let masked = client
        .entries_get(GetEntryParams {
            id: entry.id.clone(),
            reveal: false,
        })
        .unwrap();
    assert!(revealed_password(&masked).is_none());
    assert_eq!(
        client
            .entries_list(ListEntriesParams {
                profile_id: Some(profile.id.clone()),
                query: None,
                entry_type: None
            })
            .unwrap()
            .len(),
        1
    );
    assert!(
        client
            .entries_reveal(RevealParams {
                id: entry.id.clone(),
                field: "password".into()
            })
            .unwrap()
            .value
            == SECRET
    );
    assert!(
        revealed_password(
            &client
                .entries_get(GetEntryParams {
                    id: entry.id.clone(),
                    reveal: true
                })
                .unwrap()
        ) == Some(SECRET)
    );

    // A masked editor draft preserves omitted secret fields.
    let mut draft = login(&profile.id, "Account renamed", None);
    draft.notes = "non-secret edit".into();
    let changed = client
        .entries_update(EntryUpdateParams {
            id: entry.id.clone(),
            input: draft.clone(),
        })
        .unwrap();
    assert_eq!(changed.name, "Account renamed");
    assert!(revealed_password(&changed).is_none());
    assert!(
        client
            .entries_reveal(RevealParams {
                id: entry.id.clone(),
                field: "password".into()
            })
            .unwrap()
            .value
            == SECRET
    );

    // A rejected write leaves both metadata and secret value intact.
    let mut invalid_write = draft.clone();
    invalid_write.name = "invalid/name".into();
    invalid_write.payload = login(&profile.id, "x", Some(SECRET_TWO)).payload;
    assert_eq!(
        client
            .entries_update(EntryUpdateParams {
                id: entry.id.clone(),
                input: invalid_write
            })
            .err()
            .unwrap()
            .code,
        ErrorCode::InvalidParams
    );
    let after_rejection = client
        .entries_get(GetEntryParams {
            id: entry.id.clone(),
            reveal: true,
        })
        .unwrap();
    assert_eq!(after_rejection.name, "Account renamed");
    assert!(revealed_password(&after_rejection) == Some(SECRET));

    // Empty string explicitly clears an optional secret. History restores it.
    draft.payload = login(&profile.id, "x", Some("")).payload;
    client
        .entries_update(EntryUpdateParams {
            id: entry.id.clone(),
            input: draft,
        })
        .unwrap();
    let cleared = client
        .entries_get(GetEntryParams {
            id: entry.id.clone(),
            reveal: true,
        })
        .unwrap();
    assert!(revealed_password(&cleared).is_none());
    assert!(!cleared.redacted_fields.iter().any(|f| f == "password"));
    assert_eq!(
        client
            .entries_reveal(RevealParams {
                id: entry.id.clone(),
                field: "password".into()
            })
            .err()
            .unwrap()
            .code,
        ErrorCode::NotFound
    );
    let history = client.entries_history(id(&entry.id)).unwrap();
    assert!(history.len() >= 3);
    assert!(
        history
            .iter()
            .any(|h| h.changed_fields.iter().any(|f| f == "password"))
    );
    client
        .entries_restore(RestoreParams {
            id: entry.id.clone(),
            commit: history[1].commit.clone(),
        })
        .unwrap();
    assert!(
        client
            .entries_reveal(RevealParams {
                id: entry.id.clone(),
                field: "password".into()
            })
            .unwrap()
            .value
            == SECRET
    );

    client.entries_delete(id(&entry.id)).unwrap();
    assert_eq!(
        client
            .entries_get(GetEntryParams {
                id: entry.id.clone(),
                reveal: false
            })
            .err()
            .unwrap()
            .code,
        ErrorCode::NotFound
    );
    let trash = client
        .entries_trash(TrashParams {
            profile_id: Some(profile.id.clone()),
        })
        .unwrap();
    let item = trash
        .iter()
        .find(|item| item.id == entry.id)
        .expect("deleted entry in trash");
    client
        .entries_restore(RestoreParams {
            id: entry.id.clone(),
            commit: item.commit.clone(),
        })
        .unwrap();
    assert_eq!(
        client
            .entries_get(GetEntryParams {
                id: entry.id.clone(),
                reveal: false
            })
            .unwrap()
            .id,
        entry.id
    );

    // Authentication errors have stable codes and never quote attempted secrets.
    assert!(client.vault_lock().unwrap().locked);
    assert_eq!(
        client.profiles_list().err().unwrap().code,
        ErrorCode::Locked
    );
    let wrong = "fictional-wrong-passphrase";
    let error = client
        .vault_unlock(UnlockParams {
            passphrase: Some(wrong.into()),
            identity_file: None,
            timeout_secs: None,
        })
        .err()
        .unwrap();
    assert_eq!(error.code, ErrorCode::BadCredentials);
    assert!(!error.message.contains(wrong));
    assert!(client.vault_status().unwrap().locked);
    assert!(
        !client
            .vault_unlock(UnlockParams {
                passphrase: Some(PASS.into()),
                identity_file: None,
                timeout_secs: None
            })
            .unwrap()
            .locked
    );
    assert_eq!(
        client.profiles_get(id(&profile.id)).unwrap().name,
        "renamed"
    );

    client.entries_delete(id(&entry.id)).unwrap();
    client.profiles_delete(id(&profile.id)).unwrap();
    assert!(client.profiles_list().unwrap().is_empty());
    let persisted = client.profiles_create(profile_input("persisted")).unwrap();
    let entry = client
        .entries_create(login(&persisted.id, "persisted entry", Some(SECRET)))
        .unwrap();
    (persisted.id, entry.id)
}

fn check_reopen<C: RpcSdk<Error = RpcError>>(client: &mut C, profile_id: &str, entry_id: &str) {
    let status = client.vault_status().unwrap();
    assert!(status.initialized && status.locked);
    assert_eq!(
        client.profiles_list().err().unwrap().code,
        ErrorCode::Locked
    );
    assert!(
        !client
            .vault_unlock(UnlockParams {
                passphrase: Some(PASS.into()),
                identity_file: None,
                timeout_secs: None
            })
            .unwrap()
            .locked
    );
    assert_eq!(
        client.profiles_get(id(profile_id)).unwrap().name,
        "persisted"
    );
    assert!(
        client
            .entries_reveal(RevealParams {
                id: entry_id.into(),
                field: "password".into()
            })
            .unwrap()
            .value
            == SECRET
    );
}

#[test]
fn embedded_typed_rpc_contract() {
    let a = TestAgent::new();
    let (profile_id, entry_id) = {
        let mut client = LocalClient::new(&a.agent, ClientCtx::desktop());
        scenario(&mut client, &a)
    };
    let config = a.dir.path().join("config");
    a.agent.shutdown();
    let reopened = TestAgent::with(|options| options.config_dir = Some(config));
    let mut client = LocalClient::new(&reopened.agent, ClientCtx::desktop());
    check_reopen(&mut client, &profile_id, &entry_id);
}

#[cfg(unix)]
#[test]
fn socket_typed_rpc_contract() {
    let _guard = SOCKET_SERIAL
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let a = TestAgent::new();
    a.agent.serve_socket().unwrap();
    let (profile_id, entry_id) = {
        let (mut client, hello) = AgentClient::connect(HelloParams {
            client: ClientKind::Cli,
            version: "rpc-contract-test".into(),
        })
        .unwrap();
        assert_eq!(hello.protocol, PROTOCOL_VERSION);
        scenario(&mut client, &a)
    };
    let config = a.dir.path().join("config");
    a.agent.shutdown();
    let reopened = TestAgent::with(|options| options.config_dir = Some(config));
    reopened.agent.serve_socket().unwrap();
    {
        let (mut client, _) = AgentClient::connect(HelloParams {
            client: ClientKind::Cli,
            version: "rpc-contract-test".into(),
        })
        .unwrap();
        check_reopen(&mut client, &profile_id, &entry_id);
    }
    reopened.agent.shutdown();
}
