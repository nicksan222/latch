use super::*;
use latch_rpc::v1;
use std::collections::BTreeSet;
use std::time::Duration;

macro_rules! exercise_each_unary {
    ($(($method:ident, $variant:ident, $input:ident, $output:ident);)*) => {
        async fn invoke_all(client: &mut Client) {
            $(
                let response = client.$method(request(v1::$input::default())).await;
                if let Err(error) = response {
                    assert_ne!(error.code(), tonic::Code::Unimplemented, "{} has no service", stringify!($variant));
                    let details = <v1::RpcError as prost::Message>::decode(error.details())
                        .expect("each procedure must return a typed response from the external service");
                    assert!(!details.message.contains("unknown method"), "{} has no implementation", stringify!($variant));
                    assert_ne!(details.code, v1::ErrorCode::Internal as i32, "{} returned an internal failure", stringify!($variant));
                }
            )*
        }
    };
}
latch_rpc::for_each_rpc!(exercise_each_unary);

#[tokio::test]
async fn every_schema_method_crosses_the_external_grpc_boundary() {
    let mut host = Harness::start().await;
    invoke_all(&mut host.client).await;
    let stream = host
        .client
        .watch_events(request(v1::Empty {}))
        .await
        .unwrap()
        .into_inner();
    drop(stream);
    let pool = prost_reflect::DescriptorPool::decode(latch_rpc::DESCRIPTOR_SET).unwrap();
    let service = pool.get_service_by_name("latch.v1.LatchAgent").unwrap();
    let declared: BTreeSet<String> = service
        .methods()
        .map(|method| method.name().to_owned())
        .collect();
    let observed = host.seen.lock().unwrap().clone();
    assert_eq!(
        observed, declared,
        "every schema method must reach the external HTTP/2 transport"
    );
}

#[tokio::test]
async fn authentication_invalid_messages_and_size_limits_fail_closed() {
    let mut host = Harness::start().await;
    let error = host.client.vault_status(v1::Empty {}).await.unwrap_err();
    assert_eq!(error.code(), tonic::Code::PermissionDenied);
    let mut missing_version = request(v1::Empty {});
    missing_version.metadata_mut().remove("x-latch-version");
    assert_eq!(
        host.client
            .vault_status(missing_version)
            .await
            .unwrap_err()
            .code(),
        tonic::Code::PermissionDenied
    );
    let mut bad_client = request(v1::Empty {});
    bad_client
        .metadata_mut()
        .insert("x-latch-client", "native_host".parse().unwrap());
    assert_eq!(
        host.client
            .vault_status(bad_client)
            .await
            .unwrap_err()
            .code(),
        tonic::Code::PermissionDenied
    );
    let error = host
        .client
        .hello(request(v1::HelloParams {
            client: v1::ClientKind::Desktop as i32,
            version: "external-test/1".into(),
        }))
        .await
        .unwrap_err();
    expect_error(error, v1::ErrorCode::Denied);
    let error = host
        .client
        .hello(request(v1::HelloParams {
            client: 999,
            version: "external-test/1".into(),
        }))
        .await
        .unwrap_err();
    expect_error(error, v1::ErrorCode::InvalidParams);
    let error = host
        .client
        .domain_call(request(v1::DomainRequest::default()))
        .await
        .unwrap_err();
    expect_error(error, v1::ErrorCode::InvalidParams);
    let error = host
        .client
        .env_parse(request(v1::EnvParseParams {
            text: "x".repeat(17 * 1024 * 1024),
        }))
        .await
        .unwrap_err();
    assert!(matches!(
        error.code(),
        tonic::Code::OutOfRange | tonic::Code::ResourceExhausted
    ));
    assert!(
        !host
            .client
            .vault_status(request(v1::Empty {}))
            .await
            .unwrap()
            .into_inner()
            .initialized
    );
}

#[tokio::test]
async fn concurrent_mutations_and_lock_are_atomic_across_independent_clients() {
    let mut host = Harness::start().await;
    host.init().await;
    let profile = host.profile("parallel").await;
    // Prove successful parallel writes independently of lock scheduling.
    let mut committed = Vec::new();
    for index in 0..6 {
        let mut client = host.client.clone();
        let profile_id = profile.id.clone();
        committed.push(tokio::spawn(async move {
            client
                .entries_create(request(v1::EntryInput {
                    profile_id,
                    name: format!("committed-{index}"),
                    payload: Some(v1::EntryPayload {
                        data: Some(v1::entry_payload::Data::Secret(v1::SecretData {
                            value: Some(SECRET.into()),
                        })),
                    }),
                    ..Default::default()
                }))
                .await
                .unwrap()
                .into_inner()
                .id
        }));
    }
    let mut successes = BTreeSet::new();
    for task in committed {
        successes.insert(task.await.unwrap());
    }
    assert_eq!(successes.len(), 6);
    let mut tasks = Vec::new();
    for index in 0..12 {
        let mut client = host.client.clone();
        let profile_id = profile.id.clone();
        tasks.push(tokio::spawn(async move {
            client
                .entries_create(request(v1::EntryInput {
                    profile_id,
                    name: format!("parallel-{index}"),
                    payload: Some(v1::EntryPayload {
                        data: Some(v1::entry_payload::Data::Secret(v1::SecretData {
                            value: Some(SECRET.into()),
                        })),
                    }),
                    ..Default::default()
                }))
                .await
        }));
    }
    assert!(
        host.client
            .vault_lock(request(v1::Empty {}))
            .await
            .unwrap()
            .into_inner()
            .locked
    );
    for task in tasks {
        match task.await.unwrap() {
            Ok(response) => {
                successes.insert(response.into_inner().id);
            }
            Err(error) => expect_error(error, v1::ErrorCode::Locked),
        }
    }
    expect_error(
        host.client
            .profiles_list(request(v1::Empty {}))
            .await
            .unwrap_err(),
        v1::ErrorCode::Locked,
    );
    host.client
        .vault_unlock(request(v1::UnlockParams {
            passphrase: Some(PASS.into()),
            ..Default::default()
        }))
        .await
        .unwrap();
    let entries = host
        .client
        .entries_list(request(v1::ListEntriesParams {
            profile_id: Some(profile.id),
            ..Default::default()
        }))
        .await
        .unwrap()
        .into_inner();
    assert_eq!(
        entries
            .items
            .iter()
            .map(|entry| entry.id.clone())
            .collect::<BTreeSet<_>>(),
        successes
    );
    assert!(
        !host
            .client
            .vault_status(request(v1::Empty {}))
            .await
            .unwrap()
            .into_inner()
            .dirty
    );
}

async fn event(stream: &mut tonic::Streaming<v1::AgentEvent>, expected: v1::AgentEventKind) {
    tokio::time::timeout(Duration::from_secs(5), async {
        while let Some(event) = stream.message().await.unwrap() {
            if event.kind == expected as i32 {
                return;
            }
        }
        panic!("event stream ended before expected notification");
    })
    .await
    .expect("bounded event delivery");
}

#[tokio::test]
async fn streamed_notifications_follow_public_mutations_and_server_restart() {
    let mut host = Harness::start().await;
    let mut stream = host
        .client
        .watch_events(request(v1::Empty {}))
        .await
        .unwrap()
        .into_inner();
    host.init().await;
    event(&mut stream, v1::AgentEventKind::Unlocked).await;
    let profile = host.profile("events").await;
    event(&mut stream, v1::AgentEventKind::ProfilesChanged).await;
    host.client
        .entries_create(request(v1::EntryInput {
            profile_id: profile.id,
            name: "event note".into(),
            payload: Some(v1::EntryPayload {
                data: Some(v1::entry_payload::Data::Note(v1::NoteData {
                    body: Some(SECRET.into()),
                })),
            }),
            ..Default::default()
        }))
        .await
        .unwrap();
    event(&mut stream, v1::AgentEventKind::EntriesChanged).await;
    assert!(
        !host
            .client
            .vault_status(request(v1::Empty {}))
            .await
            .unwrap()
            .into_inner()
            .locked
    );
    host.client.vault_lock(request(v1::Empty {})).await.unwrap();
    event(&mut stream, v1::AgentEventKind::Locked).await;
    host.restart().await;
    let closed = tokio::time::timeout(Duration::from_secs(5), stream.message())
        .await
        .expect("stream closes with external host");
    assert!(closed.is_err() || closed.unwrap().is_none());
    assert!(
        host.client
            .vault_status(request(v1::Empty {}))
            .await
            .unwrap()
            .into_inner()
            .locked
    );
}
