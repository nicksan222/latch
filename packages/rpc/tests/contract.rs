use latch_rpc::{
    client,
    errors::{self, RpcError},
    sdk::{Call, Reply, RpcSdk, RpcTransport},
    v1,
};
use prost::Message;

#[test]
fn every_unary_request_uses_its_generated_protobuf_message() {
    // The generated service descriptor supplies every method, not a parallel list.
    macro_rules! check_methods {
        ($(($method:ident, $name:ident, $input:ident, $output:ident);)*) => { $(
            let bytes = v1::$input::default().encode_to_vec();
            let call = Call::decode(stringify!($name), &bytes).unwrap();
            assert_eq!(call.method(), stringify!($name));
            assert_eq!(call.encode(), bytes);
        )* };
    }
    latch_rpc::for_each_rpc!(check_methods);
}

#[test]
fn requests_reject_unknown_methods_and_invalid_protobuf_without_echoing_input() {
    assert_eq!(
        Call::decode("vault.unlock", b"sensitive").unwrap_err().code,
        v1::ErrorCode::InvalidParams
    );
    let error = Call::decode("VaultUnlock", &[0x0a, 0xff]).unwrap_err();
    assert_eq!(error.code, v1::ErrorCode::InvalidParams);
    assert_eq!(error.message, "invalid protobuf request");
}

#[test]
fn protobuf_preserves_secret_presence_and_redacts_messages() {
    let absent = v1::UnlockParams::default();
    let present = v1::UnlockParams {
        passphrase: Some(String::new()),
        ..Default::default()
    };
    assert_ne!(absent.encode_to_vec(), present.encode_to_vec());
    assert_eq!(
        v1::UnlockParams::decode(present.encode_to_vec().as_slice())
            .unwrap()
            .passphrase,
        Some(String::new())
    );
    let secret = "private-test-value";
    let call = Call::VaultUnlock(v1::UnlockParams {
        passphrase: Some(secret.into()),
        ..Default::default()
    });
    assert!(!format!("{call:?}").contains(secret));
}

#[test]
fn grpc_errors_round_trip_codes_without_leaking_transport_messages() {
    for code in 1..=13 {
        let error = RpcError {
            code: v1::ErrorCode::try_from(code).unwrap(),
            message: "safe explanation".into(),
        };
        let decoded = errors::rpc_error(errors::status(&error));
        assert_eq!(decoded, error);
    }
    let fallback = errors::rpc_error(tonic::Status::unavailable("private transport detail"));
    assert_eq!(fallback.code, v1::ErrorCode::Io);
    assert_eq!(fallback.message, "agent unavailable");
    let error = RpcError {
        code: v1::ErrorCode::Internal,
        message: "private-test-value".into(),
    };
    assert!(!format!("{error:?}").contains("private-test-value"));
}

#[test]
fn client_metadata_requires_a_known_identity_and_a_valid_version() {
    let identity = v1::HelloParams {
        client: v1::ClientKind::Cli as i32,
        version: "test/1".into(),
    };
    let request = client::request(v1::Empty {}, &identity).unwrap();
    assert_eq!(
        request
            .metadata()
            .get("x-latch-client")
            .unwrap()
            .to_str()
            .unwrap(),
        "cli"
    );
    assert_eq!(
        request
            .metadata()
            .get("x-latch-version")
            .unwrap()
            .to_str()
            .unwrap(),
        "test/1"
    );
    assert!(client::request(v1::Empty {}, &v1::HelloParams::default()).is_err());
    assert!(
        client::request(
            v1::Empty {},
            &v1::HelloParams {
                version: "bad\nvalue".into(),
                ..identity
            }
        )
        .is_err()
    );
}

#[test]
fn blocking_sdk_keeps_typed_results_and_fails_on_a_mismatched_reply() {
    struct Transport {
        wrong_reply: bool,
    }
    impl RpcTransport for Transport {
        type Error = RpcError;
        fn execute(&mut self, request: Call) -> Result<Reply, Self::Error> {
            assert!(matches!(request, Call::VaultStatus(_)));
            if self.wrong_reply {
                Ok(Reply::VaultLock(v1::VaultStatus::default()))
            } else {
                Ok(Reply::VaultStatus(v1::VaultStatus {
                    locked: true,
                    ..Default::default()
                }))
            }
        }
    }
    assert!(
        Transport { wrong_reply: false }
            .vault_status()
            .unwrap()
            .locked
    );
    assert_eq!(
        Transport { wrong_reply: true }
            .vault_status()
            .unwrap_err()
            .code,
        v1::ErrorCode::Internal
    );
}
