use latch_rpc::{codec, types, v1};
use prost::Message;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

fn roundtrip<M: Message + Default, T: DeserializeOwned + PartialEq>(value: Value, name: &str) {
    let expected: T = serde_json::from_value(value.clone()).unwrap();
    let proto: M = codec::from_legacy(value, name).unwrap();
    let actual: T = serde_json::from_value(codec::to_legacy(&proto, name).unwrap()).unwrap();
    assert!(actual == expected, "{name} compatibility shape changed");
}

fn payload(kind: &str, data: Value) -> Value {
    json!({"type": kind, "data": data})
}

fn entry(payload: Value) -> Value {
    json!({"profileId":"p", "name":"n", "tags":[], "notes":"", "favorite":false, "payload":payload})
}

fn browser() -> Value {
    json!({"engine":"chromium", "binary":null, "startPages":[], "bookmarks":[], "extensionsAllowed":false, "autofill":"all", "autofillEntryIds":[]})
}

#[test]
fn all_entry_payloads_preserve_tagged_shape_and_optional_secret_presence() {
    let cases = [
        payload(
            "login",
            json!({"username":"u", "password":null, "urls":[], "totp":null, "custom":[]}),
        ),
        payload("secret", json!({"value":""})),
        payload(
            "env",
            json!({"vars":[{"key":"TOKEN", "value":null, "hidden":true, "comment":null}]}),
        ),
        payload(
            "file",
            json!({"filename":"f", "mime":"application/octet-stream", "contentBase64":null, "size":0}),
        ),
        payload("note", json!({"body":null})),
        payload(
            "ssh_key",
            json!({"privateKey":null, "publicKey":"", "passphrase":"", "comment":""}),
        ),
        payload(
            "database",
            json!({"engine":"postgres", "host":"h", "port":5432, "database":"d", "username":"u", "password":null, "options":""}),
        ),
        payload(
            "api_key",
            json!({"key":null, "secret":"", "endpoint":null, "expiresAt":null}),
        ),
    ];
    for case in cases {
        roundtrip::<v1::EntryPayload, types::EntryPayload>(case, "latch.v1.EntryPayload");
    }
    let absent: v1::SecretData =
        codec::from_legacy(json!({"value":null}), "latch.v1.SecretData").unwrap();
    let cleared: v1::SecretData =
        codec::from_legacy(json!({"value":""}), "latch.v1.SecretData").unwrap();
    assert!(absent.value.is_none());
    assert!(cleared.value.as_deref() == Some(""));
}

#[test]
fn every_domain_operation_preserves_request_and_response_variant() {
    let secret = payload("secret", json!({"value":null}));
    let input = entry(secret.clone());
    let profile = json!({"name":"p", "color":"#2563eb", "icon":"briefcase", "description":"", "pinned":false, "browser":browser()});
    let cases: [(&str, Value, Value); 21] = [
        ("validate_entry", input.clone(), json!({})),
        ("validate_profile", profile, json!({})),
        (
            "validate_recipient",
            json!({"label":"l", "key":"k", "existing":[]}),
            json!({}),
        ),
        ("recipient_key_kind", json!("age1example"), json!("age")),
        (
            "parse_dotenv",
            json!("TOKEN=x"),
            json!([{"key":"TOKEN", "value":"x", "hidden":true, "comment":null}]),
        ),
        (
            "format_env",
            json!({"vars":[{"key":"A", "value":"x"}], "format":"dotenv"}),
            json!("A=x"),
        ),
        ("parse_otpauth", json!("otpauth://totp/x"), Value::Null),
        ("generate_password", json!(24), json!("fictional")),
        (
            "estimate_strength",
            json!("fictional"),
            json!({"score":2, "label":"okay", "bits":40, "hints":[]}),
        ),
        (
            "entry_defaults",
            json!({"profileId":"p", "entryType":"secret"}),
            input.clone(),
        ),
        (
            "database_url",
            json!({"engine":"postgres", "host":"h", "port":null, "database":"d", "username":"u", "password":null, "options":""}),
            json!("postgres://h/d"),
        ),
        ("secret_paths", secret.clone(), json!(["value"])),
        (
            "read_field",
            json!({"payload":secret.clone(), "path":"value"}),
            Value::Null,
        ),
        (
            "redact_payload",
            secret.clone(),
            json!({"payload":secret.clone(), "redacted":[]}),
        ),
        (
            "merge_keep_on_none",
            json!({"stored":secret.clone(), "input":secret.clone()}),
            secret.clone(),
        ),
        ("apply_optional_clears", secret.clone(), secret.clone()),
        ("flatten_entry", input.clone(), json!({"name":"n"})),
        (
            "merge_entries",
            json!({"mine":input.clone(), "theirs":input.clone(), "fromTheirs":[]}),
            input.clone(),
        ),
        ("default_browser_settings", Value::Null, browser()),
        ("file_digest", Value::Null, Value::Null),
        ("diff_rows", json!({"mine":null, "theirs":null}), json!([])),
    ];
    for (operation, input, output) in cases {
        roundtrip::<v1::DomainRequest, types::DomainRequest>(
            json!({"operation":operation, "input":input}),
            "latch.v1.DomainRequest",
        );
        roundtrip::<v1::DomainResponse, types::DomainResponse>(
            json!({"operation":operation, "output":output}),
            "latch.v1.DomainResponse",
        );
    }
}

#[test]
fn wrappers_and_enum_validation_are_explicit() {
    roundtrip::<v1::ProfilesListResponse, Vec<types::ProfileSummary>>(
        json!([]),
        "latch.v1.ProfilesListResponse",
    );
    roundtrip::<v1::ActionInput, types::ActionInput>(
        json!({"name":"n", "command":"true", "envEntryIds":[], "envKeys":[], "urls":[]}),
        "latch.v1.ActionInput",
    );
    roundtrip::<v1::ActionInput, types::ActionInput>(
        json!({"name":"n", "command":"true", "envEntryIds":[], "envKeys":null, "urls":[]}),
        "latch.v1.ActionInput",
    );
    assert!(
        codec::from_legacy::<v1::BrowserSettings>(
            json!({"engine":"not_an_engine"}),
            "latch.v1.BrowserSettings"
        )
        .is_err()
    );
    assert_eq!(
        codec::from_legacy::<v1::TotpConfig>(
            json!({"secret":null,"algorithm":"SHA1","digits":256,"period":30,"issuer":null}),
            "latch.v1.TotpConfig"
        )
        .err()
        .unwrap()
        .code,
        types::ErrorCode::InvalidParams
    );
    assert_eq!(
        codec::from_legacy::<v1::DatabaseData>(
            json!({"engine":"postgres","host":"h","port":65536,"database":"d","username":"u","password":null,"options":""}),
            "latch.v1.DatabaseData"
        ).err().unwrap().code,
        types::ErrorCode::InvalidParams
    );
}

#[test]
fn grpc_status_details_preserve_codes_without_exposing_transport_text() {
    let error = types::RpcError {
        code: types::ErrorCode::BadCredentials,
        message: "credentials rejected".into(),
    };
    let status = codec::status(&error);
    assert_eq!(status.code(), tonic::Code::Unauthenticated);
    let actual = codec::rpc_error(status);
    assert_eq!(actual.code, error.code);
    assert_eq!(actual.message, error.message);
    let fallback = codec::rpc_error(tonic::Status::unavailable(
        "possibly sensitive internal text",
    ));
    assert_eq!(fallback.code, types::ErrorCode::Io);
    assert!(!fallback.message.contains("sensitive"));
}

#[test]
fn debug_output_never_contains_secret_payloads() {
    let marker = "fictional-private-value";
    let value = entry(payload("secret", json!({"value":marker})));
    let legacy: types::EntryInput = serde_json::from_value(value.clone()).unwrap();
    let proto: v1::EntryInput = codec::from_legacy(value, "latch.v1.EntryInput").unwrap();
    assert!(!format!("{legacy:?}").contains(marker));
    assert!(!format!("{proto:?}").contains(marker));
    assert!(!format!("{:?}", proto.payload).contains(marker));
    let error = types::RpcError {
        code: types::ErrorCode::InvalidParams,
        message: marker.into(),
    };
    assert!(!format!("{error:?}").contains(marker));
    assert!(!format!("{error}").contains(marker));
}

#[test]
fn omittable_legacy_fields_stay_absent() {
    let event: v1::AgentEvent =
        codec::from_legacy(json!({"kind":"locked"}), "latch.v1.AgentEvent").unwrap();
    let value = codec::to_legacy(&event, "latch.v1.AgentEvent").unwrap();
    assert_eq!(value, json!({"kind":"locked"}));

    let copy: v1::ClipboardCopyParams = codec::from_legacy(
        json!({"id":"entry", "field":"password"}),
        "latch.v1.ClipboardCopyParams",
    )
    .unwrap();
    let value = codec::to_legacy(&copy, "latch.v1.ClipboardCopyParams").unwrap();
    assert_eq!(value, json!({"id":"entry", "field":"password"}));
}

#[test]
fn absent_optional_env_fields_remain_explicit_nulls() {
    let expected = json!([{"key":"TOKEN", "value":null, "hidden":true, "comment":null}]);
    let response: v1::EnvParseResponse =
        codec::from_legacy(expected.clone(), "latch.v1.EnvParseResponse").unwrap();
    assert_eq!(
        codec::to_legacy(&response, "latch.v1.EnvParseResponse").unwrap(),
        expected
    );
}

#[test]
fn omitted_forget_trust_confirmation_uses_legacy_default() {
    let params: types::ForgetTrustParams = serde_json::from_value(json!({})).unwrap();
    assert!(!params.confirm);
    let wire: v1::ForgetTrustParams =
        codec::from_legacy(json!({}), "latch.v1.ForgetTrustParams").unwrap();
    assert!(!wire.confirm);
    let value = codec::to_legacy(&wire, "latch.v1.ForgetTrustParams").unwrap();
    assert_eq!(value, json!({"confirm":false}));
}
