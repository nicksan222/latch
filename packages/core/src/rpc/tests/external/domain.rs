use super::*;
use latch_rpc::v1::{
    self, domain_request::Operation as Q, domain_response::Operation as R, entry_payload::Data,
};

async fn call(h: &mut Harness, operation: Q) -> R {
    h.client
        .domain_call(request(v1::DomainRequest {
            operation: Some(operation),
        }))
        .await
        .unwrap()
        .into_inner()
        .operation
        .expect("typed domain response")
}

fn secret(value: Option<&str>) -> v1::EntryPayload {
    v1::EntryPayload {
        data: Some(Data::Secret(v1::SecretData {
            value: value.map(str::to_owned),
        })),
    }
}

fn entry(name: &str, value: Option<&str>) -> v1::EntryInput {
    v1::EntryInput {
        profile_id: "synthetic-profile".into(),
        name: name.into(),
        payload: Some(secret(value)),
        ..Default::default()
    }
}

#[tokio::test]
async fn domain_validation_parsing_passwords_and_formats_use_typed_variants() {
    let mut h = Harness::start().await;
    match call(&mut h, Q::ValidateEntry(entry("bad/name", Some(SECRET)))).await {
        R::ValidateEntry(v) => assert!(v.items.contains_key("name")),
        _ => panic!("wrong response variant"),
    }
    match call(&mut h, Q::ValidateProfile(profile_input("bad/name"))).await {
        R::ValidateProfile(v) => assert!(v.items.contains_key("name")),
        _ => panic!("wrong response variant"),
    }
    match call(
        &mut h,
        Q::ValidateRecipient(v1::ValidateRecipientInput {
            label: String::new(),
            key: "invalid key".into(),
            ..Default::default()
        }),
    )
    .await
    {
        R::ValidateRecipient(v) => {
            assert!(v.items.contains_key("label") && v.items.contains_key("key"))
        }
        _ => panic!("wrong response variant"),
    }
    match call(
        &mut h,
        Q::RecipientKeyKind(v1::StringValue {
            value: "invalid key".into(),
        }),
    )
    .await
    {
        R::RecipientKeyKind(v) => assert!(v.value.is_none()),
        _ => panic!("wrong response variant"),
    }
    match call(
        &mut h,
        Q::ParseDotenv(v1::StringValue {
            value: "A=fictional\nB=second".into(),
        }),
    )
    .await
    {
        R::ParseDotenv(v) => assert_eq!(v.items.len(), 2),
        _ => panic!("wrong response variant"),
    }
    match call(
        &mut h,
        Q::FormatEnv(v1::DomainEnvFormatInput {
            vars: vec![v1::EnvPair {
                key: "A".into(),
                value: "fictional".into(),
            }],
            format: v1::EnvFormat::Dotenv as i32,
        }),
    )
    .await
    {
        R::FormatEnv(v) => assert!(v.value.contains("A=fictional")),
        _ => panic!("wrong response variant"),
    }
    match call(
        &mut h,
        Q::ParseOtpauth(v1::StringValue {
            value: "otpauth://totp/Issuer:alice?secret=JBSWY3DPEHPK3PXP".into(),
        }),
    )
    .await
    {
        R::ParseOtpauth(v) => {
            let parsed = v.value.expect("valid otpauth");
            assert_eq!(parsed.account.as_deref(), Some("alice"));
            assert_eq!(parsed.totp.expect("TOTP").digits, 6);
        }
        _ => panic!("wrong response variant"),
    }
    match call(
        &mut h,
        Q::ParseOtpauth(v1::StringValue {
            value: "not an otpauth URI".into(),
        }),
    )
    .await
    {
        R::ParseOtpauth(v) => assert!(v.value.is_none()),
        _ => panic!("wrong response variant"),
    }
    match call(&mut h, Q::GeneratePassword(v1::UInt32Value { value: 24 })).await {
        R::GeneratePassword(v) => assert_eq!(v.value.chars().count(), 24),
        _ => panic!("wrong response variant"),
    }
    match call(
        &mut h,
        Q::EstimateStrength(v1::StringValue {
            value: "password1".into(),
        }),
    )
    .await
    {
        R::EstimateStrength(v) => assert!(v.score <= 1),
        _ => panic!("wrong response variant"),
    }
    for invalid in [
        Q::GeneratePassword(v1::UInt32Value { value: 4097 }),
        Q::EntryDefaults(v1::EntryDefaultsInput {
            profile_id: "p".into(),
            entry_type: 999,
        }),
    ] {
        expect_error(
            h.client
                .domain_call(request(v1::DomainRequest {
                    operation: Some(invalid),
                }))
                .await
                .unwrap_err(),
            v1::ErrorCode::InvalidParams,
        );
    }
    expect_error(
        h.client
            .domain_call(request(v1::DomainRequest { operation: None }))
            .await
            .unwrap_err(),
        v1::ErrorCode::InvalidParams,
    );
}

#[tokio::test]
async fn domain_payload_operations_preserve_optional_secret_semantics() {
    let mut h = Harness::start().await;
    match call(
        &mut h,
        Q::EntryDefaults(v1::EntryDefaultsInput {
            profile_id: "synthetic-profile".into(),
            entry_type: v1::EntryType::Secret as i32,
        }),
    )
    .await
    {
        R::EntryDefaults(v) => assert!(matches!(
            v.payload.and_then(|p| p.data),
            Some(Data::Secret(_))
        )),
        _ => panic!("wrong response variant"),
    }
    let db = v1::DatabaseData {
        engine: "postgres".into(),
        host: "example.test".into(),
        database: "sample".into(),
        username: "fictional-user".into(),
        password: Some(SECRET.into()),
        ..Default::default()
    };
    match call(&mut h, Q::DatabaseUrl(db)).await {
        R::DatabaseUrl(v) => assert!(v.value.starts_with("postgres")),
        _ => panic!("wrong response variant"),
    }
    match call(&mut h, Q::SecretPaths(secret(Some(SECRET)))).await {
        R::SecretPaths(v) => assert_eq!(v.items, ["value"]),
        _ => panic!("wrong response variant"),
    }
    match call(
        &mut h,
        Q::ReadField(v1::ReadFieldInput {
            payload: Some(secret(Some(SECRET))),
            path: "value".into(),
        }),
    )
    .await
    {
        R::ReadField(v) => assert_eq!(v.value.as_deref(), Some(SECRET)),
        _ => panic!("wrong response variant"),
    }
    match call(&mut h, Q::RedactPayload(secret(Some(SECRET)))).await {
        R::RedactPayload(v) => {
            assert_eq!(v.redacted, ["value"]);
            assert!(matches!(
                v.payload.and_then(|p| p.data),
                Some(Data::Secret(v1::SecretData { value: None }))
            ));
        }
        _ => panic!("wrong response variant"),
    }
    match call(
        &mut h,
        Q::MergeKeepOnNone(v1::PayloadMergeInput {
            stored: Some(secret(Some(SECRET))),
            input: Some(secret(None)),
        }),
    )
    .await
    {
        R::MergeKeepOnNone(v) => assert!(
            matches!(v.data, Some(Data::Secret(v1::SecretData { value: Some(s) })) if s == SECRET)
        ),
        _ => panic!("wrong response variant"),
    }
    match call(
        &mut h,
        Q::ApplyOptionalClears(v1::EntryPayload {
            data: Some(Data::ApiKey(v1::ApiKeyData {
                secret: Some(String::new()),
                ..Default::default()
            })),
        }),
    )
    .await
    {
        R::ApplyOptionalClears(v) => assert!(matches!(
            v.data,
            Some(Data::ApiKey(v1::ApiKeyData { secret: None, .. }))
        )),
        _ => panic!("wrong response variant"),
    }
    match call(&mut h, Q::FlattenEntry(entry("synthetic", Some(SECRET)))).await {
        R::FlattenEntry(v) => {
            assert_eq!(v.items.get("name").map(String::as_str), Some("synthetic"))
        }
        _ => panic!("wrong response variant"),
    }
    match call(
        &mut h,
        Q::MergeEntries(v1::MergeEntriesInput {
            mine: Some(entry("mine", Some(SECRET))),
            theirs: Some(entry("theirs", Some("fictional-other"))),
            from_theirs: vec!["name".into()],
        }),
    )
    .await
    {
        R::MergeEntries(v) => assert_eq!(v.name, "theirs"),
        _ => panic!("wrong response variant"),
    }
    match call(&mut h, Q::DefaultBrowserSettings(v1::Empty {})).await {
        R::DefaultBrowserSettings(v) => assert_ne!(v.engine, v1::BrowserEngine::Unspecified as i32),
        _ => panic!("wrong response variant"),
    }
}

#[tokio::test]
async fn domain_file_digest_and_diff_rows_are_typed() {
    let mut h = Harness::start().await;
    match call(
        &mut h,
        Q::FileDigest(v1::OptionalEntryInput { value: None }),
    )
    .await
    {
        R::FileDigest(v) => assert!(v.value.is_none()),
        _ => panic!("wrong response variant"),
    }
    let file = v1::EntryInput {
        profile_id: "p".into(),
        name: "file".into(),
        payload: Some(v1::EntryPayload {
            data: Some(Data::File(v1::FileData {
                filename: "sample.txt".into(),
                mime: "text/plain".into(),
                content_base64: Some("ZmljdGlvbmFs".into()),
                size: 9,
            })),
        }),
        ..Default::default()
    };
    match call(
        &mut h,
        Q::FileDigest(v1::OptionalEntryInput { value: Some(file) }),
    )
    .await
    {
        R::FileDigest(v) => assert!(v.value.is_some_and(|digest| !digest.is_empty())),
        _ => panic!("wrong response variant"),
    }
    match call(
        &mut h,
        Q::DiffRows(v1::DiffRowsInput {
            mine: Some(entry("mine", Some(SECRET))),
            theirs: Some(entry("theirs", Some(SECRET))),
        }),
    )
    .await
    {
        R::DiffRows(v) => assert!(v.items.iter().any(|row| row.field == "name" && row.differs)),
        _ => panic!("wrong response variant"),
    }
}
