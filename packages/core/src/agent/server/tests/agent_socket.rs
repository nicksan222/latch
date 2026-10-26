// Socket server: permissions, peer check, single instance, stale socket,
// lifecycle, concurrent clients, and authenticated gRPC AgentClient.
// Tests share one runtime dir, so they are serialized.

#[path = "../../../../tests/agent_support/mod.rs"]
mod agent_support;

use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixStream;
use std::sync::{Arc, Mutex};

use agent_support::*;
use latch::agent::client::AgentClient;
use latch::agent::socket_path;
use latch::protocol::methods as m;
use latch::protocol::*;
use serde_json::{Value, json};

static SERIAL: Mutex<()> = Mutex::new(());

fn serial() -> std::sync::MutexGuard<'static, ()> {
    SERIAL.lock().unwrap_or_else(|p| p.into_inner())
}

fn cli_hello() -> HelloParams {
    HelloParams {
        client: ClientKind::Cli,
        version: "test".into(),
    }
}

#[test]
fn permissions_peer_and_lifecycle() {
    let _g = serial();
    let a = TestAgent::new();
    let path = a.agent.serve_socket().unwrap();
    assert_eq!(path, socket_path());
    let mode = std::fs::metadata(&path).unwrap().permissions().mode();
    assert_eq!(mode & 0o777, 0o600, "socket mode");
    let dir_mode = std::fs::metadata(path.parent().unwrap())
        .unwrap()
        .permissions()
        .mode();
    assert_eq!(dir_mode & 0o777, 0o700, "runtime dir mode");
    assert!(latch::agent::pid_path().exists());

    let s = UnixStream::connect(&path).unwrap();
    // SAFETY: getuid cannot fail.
    assert_eq!(latch::agent::peer_uid(&s).unwrap(), unsafe {
        libc::getuid()
    });
    drop(s);

    // Single instance.
    let b = TestAgent::new();
    let e = b.agent.serve_socket().unwrap_err();
    assert_eq!(e.code(), ErrorCode::AlreadyExists);

    let (mut client, hello) = AgentClient::connect(cli_hello()).unwrap();
    assert_eq!(hello.protocol, PROTOCOL_VERSION);
    let value = client.call(m::VAULT_STATUS, Value::Null).unwrap();
    let status: VaultStatus = serde_json::from_value(value).unwrap();
    assert!(!status.initialized);
    // Shutdown must close an active HTTP/2 connection without waiting for its
    // client to voluntarily disconnect.
    a.agent.shutdown();
    assert!(!path.exists(), "socket removed on shutdown");
    assert!(client.call(m::VAULT_STATUS, Value::Null).is_err());
    drop(client);
    let e = AgentClient::connect(cli_hello()).err().unwrap();
    assert_eq!(e.code, ErrorCode::Io);

    // After shutdown another agent can take over.
    b.agent.serve_socket().unwrap();
    b.agent.shutdown();
}

#[test]
fn stale_socket_is_replaced() {
    let _g = serial();
    setup();
    let path = socket_path();
    latch::core::paths::ensure_private_dir(path.parent().unwrap()).unwrap();
    let _ = std::fs::remove_file(&path);
    // A socket file nobody listens on (crashed agent).
    {
        let l = std::os::unix::net::UnixListener::bind(&path).unwrap();
        drop(l);
    }
    assert!(path.exists());
    let a = TestAgent::new();
    a.agent.serve_socket().unwrap();
    let (mut c, h) = AgentClient::connect(cli_hello()).unwrap();
    assert_eq!(h.protocol, PROTOCOL_VERSION);
    let v = c.call(m::VAULT_STATUS, Value::Null).unwrap();
    let st: VaultStatus = serde_json::from_value(v).unwrap();
    assert!(!st.initialized);
    a.agent.shutdown();
}

#[test]
fn concurrent_clients() {
    let _g = serial();
    let a = TestAgent::new();
    a.init();
    let p = a.create_profile("acme");
    a.agent.serve_socket().unwrap();
    let mut handles = vec![];
    for t in 0..8 {
        let pid = p.id.clone();
        handles.push(std::thread::spawn(move || {
            let (mut c, _) = AgentClient::connect(cli_hello()).unwrap();
            for i in 0..5 {
                let v = c.call(m::PROFILES_LIST, Value::Null).unwrap();
                let l: Vec<ProfileSummary> = serde_json::from_value(v).unwrap();
                assert!(!l.is_empty());
                if i == 0 {
                    // Concurrent writes are serialized.
                    let input = EntryInput {
                        profile_id: pid.clone(),
                        name: format!("secret-{t}"),
                        tags: vec![],
                        notes: String::new(),
                        favorite: false,
                        payload: EntryPayload::Secret(SecretData {
                            value: Some(format!("v{t}")),
                        }),
                    };
                    c.call(m::ENTRIES_CREATE, serde_json::to_value(input).unwrap())
                        .unwrap();
                }
            }
        }));
    }
    for h in handles {
        h.join().unwrap();
    }
    let l: Vec<EntrySummary> = a.ok(m::ENTRIES_LIST, json!({"profileId": p.id}));
    assert_eq!(l.len(), 8);
    let st: VaultStatus = a.ok(m::VAULT_STATUS, Value::Null);
    assert!(!st.dirty, "every write committed");
    a.agent.shutdown();
}

#[test]
fn headless_idle_exit() {
    let _g = serial();
    let a = TestAgent::with(|o| o.headless_idle_exit = Some(std::time::Duration::from_millis(500)));
    a.agent.serve_socket().unwrap();
    // A connected client keeps it alive.
    let (c, _) = AgentClient::connect(cli_hello()).unwrap();
    std::thread::sleep(std::time::Duration::from_millis(900));
    assert!(!a.agent.is_shutting_down());
    drop(c);
    assert!(wait_until(std::time::Duration::from_secs(5), || a
        .agent
        .is_shutting_down()));
    assert!(wait_until(std::time::Duration::from_secs(2), || {
        !socket_path().exists()
    }));
}

#[tokio::test(flavor = "current_thread")]
async fn embedded_blocking_client_works_inside_a_tokio_runtime() {
    let a = TestAgent::new();
    let (mut client, hello) =
        AgentClient::connect_embedded(Arc::downgrade(&a.agent), cli_hello()).unwrap();
    assert_eq!(hello.protocol, PROTOCOL_VERSION);
    let value = client.call(m::VAULT_STATUS, Value::Null).unwrap();
    let status: VaultStatus = serde_json::from_value(value).unwrap();
    assert!(!status.initialized);
    drop(client);
    a.agent.shutdown();
}

#[test]
fn generated_client_cannot_skip_metadata_or_mismatch_hello_identity() {
    use latch_rpc::v1::{self, latch_agent_client::LatchAgentClient};
    use tonic::{Code, Request, transport::Endpoint};
    use tower::service_fn;

    let _g = serial();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(async {
        let a = TestAgent::new();
        let path = a.agent.serve_socket().unwrap();
        let channel = Endpoint::from_static("http://[::]:50051")
            .connect_with_connector(service_fn(move |_| {
                let path = path.clone();
                async move {
                    let stream = tokio::net::UnixStream::connect(path).await?;
                    Ok::<_, std::io::Error>(hyper_util::rt::TokioIo::new(stream))
                }
            }))
            .await
            .unwrap();
        let mut client = LatchAgentClient::new(channel);
        let error = client.vault_status(v1::Empty {}).await.unwrap_err();
        assert_eq!(error.code(), Code::PermissionDenied);
        let error = client.watch_events(v1::Empty {}).await.unwrap_err();
        assert_eq!(error.code(), Code::PermissionDenied);

        let mut status = Request::new(v1::Empty {});
        status
            .metadata_mut()
            .insert("x-latch-client", "cli".parse().unwrap());
        status
            .metadata_mut()
            .insert("x-latch-version", "test".parse().unwrap());
        assert!(
            !client
                .vault_status(status)
                .await
                .unwrap()
                .into_inner()
                .initialized
        );

        let mut hello = Request::new(v1::HelloParams {
            client: v1::ClientKind::Desktop as i32,
            version: "test".into(),
        });
        hello
            .metadata_mut()
            .insert("x-latch-client", "cli".parse().unwrap());
        hello
            .metadata_mut()
            .insert("x-latch-version", "test".parse().unwrap());
        let error = client.hello(hello).await.unwrap_err();
        assert_eq!(latch_rpc::codec::rpc_error(error).code, ErrorCode::Denied);
        a.agent.shutdown();
    });
}
