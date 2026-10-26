//! UI-domain cases use the public typed RPC interface and execute Rust policy.
use latch::{
    agent::{Agent, AgentOptions, ClientCtx},
    rpc::LocalClient,
};
use latch_rpc::{
    sdk::{Call, Reply},
    v1::{
        self, domain_request::Operation as Op, domain_response::Operation as Out,
        entry_payload::Data,
    },
};

fn run(operation: Op) -> Result<Out, latch_rpc::RpcError> {
    let dir = tempfile::tempdir().unwrap();
    let agent = Agent::new(AgentOptions {
        config_dir: Some(dir.path().into()),
        ..Default::default()
    });
    let result = LocalClient::new(&agent, ClientCtx::desktop()).execute(Call::DomainCall(
        v1::DomainRequest {
            operation: Some(operation),
        },
    ));
    agent.shutdown();
    match result? {
        Reply::DomainCall(r) => Ok(r.operation.unwrap()),
        _ => panic!("domain reply expected"),
    }
}
fn text(value: &str) -> v1::StringValue {
    v1::StringValue {
        value: value.into(),
    }
}
fn payload(data: Data) -> v1::EntryPayload {
    v1::EntryPayload { data: Some(data) }
}
fn entry(payload: v1::EntryPayload) -> v1::EntryInput {
    v1::EntryInput {
        profile_id: "p".into(),
        name: "Example".into(),
        notes: "mine".into(),
        payload: Some(payload),
        ..Default::default()
    }
}
fn login() -> v1::EntryPayload {
    payload(Data::Login(v1::LoginData {
        username: "nick".into(),
        password: Some("fictional-password".into()),
        urls: vec!["https://github.com".into()],
        totp: Some(v1::TotpConfig {
            secret: Some("JBSWY3DPEHPK3PXP".into()),
            algorithm: v1::TotpAlgorithm::Sha1 as i32,
            digits: 6,
            period: 30,
            issuer: None,
        }),
        custom: vec![v1::CustomField {
            key: "Recovery".into(),
            value: Some("fictional-recovery".into()),
            hidden: true,
        }],
    }))
}
fn login_data(p: &mut v1::EntryPayload) -> &mut v1::LoginData {
    match p.data.as_mut().unwrap() {
        Data::Login(v) => v,
        _ => panic!("login expected"),
    }
}
fn validate(e: v1::EntryInput) -> std::collections::HashMap<String, String> {
    match run(Op::ValidateEntry(e)).unwrap() {
        Out::ValidateEntry(m) => m.items,
        _ => panic!("validation expected"),
    }
}
#[test]
fn login_redaction_merge_and_field_selection() {
    let stored = login();
    let paths = match run(Op::SecretPaths(stored.clone())).unwrap() {
        Out::SecretPaths(p) => p.items,
        _ => panic!(),
    };
    assert_eq!(paths, vec!["password", "totp.secret", "custom.Recovery"]);
    let redacted = match run(Op::RedactPayload(stored.clone())).unwrap() {
        Out::RedactPayload(p) => p,
        _ => panic!(),
    };
    assert_eq!(redacted.redacted, paths);
    let mut input = redacted.payload.unwrap();
    match run(Op::SecretPaths(input.clone())).unwrap() {
        Out::SecretPaths(p) => assert!(p.items.is_empty()),
        _ => panic!(),
    };
    login_data(&mut input).username = "changed".into();
    let mut merged = match run(Op::MergeKeepOnNone(v1::PayloadMergeInput {
        stored: Some(stored),
        input: Some(input),
    }))
    .unwrap()
    {
        Out::MergeKeepOnNone(p) => p,
        _ => panic!(),
    };
    assert_eq!(login_data(&mut merged).username, "changed");
    assert_eq!(
        login_data(&mut merged).password.as_deref(),
        Some("fictional-password")
    );
    let mine = entry(login());
    let mut theirs = mine.clone();
    theirs.name = "Organization".into();
    theirs.notes = "theirs".into();
    login_data(theirs.payload.as_mut().unwrap()).password = Some("replacement".into());
    match run(Op::FlattenEntry(theirs.clone())).unwrap() {
        Out::FlattenEntry(m) => {
            assert_eq!(m.items["custom.Recovery"], "fictional-recovery");
            assert_eq!(m.items["password"], "replacement");
        }
        _ => panic!(),
    };
    let mut merged = match run(Op::MergeEntries(v1::MergeEntriesInput {
        mine: Some(mine),
        theirs: Some(theirs),
        from_theirs: vec!["name".into(), "password".into()],
    }))
    .unwrap()
    {
        Out::MergeEntries(e) => e,
        _ => panic!(),
    };
    assert_eq!(merged.name, "Organization");
    assert_eq!(merged.notes, "mine");
    assert_eq!(
        login_data(merged.payload.as_mut().unwrap())
            .password
            .as_deref(),
        Some("replacement")
    );
}
#[test]
fn optional_clears_and_payload_type_changes() {
    let api = payload(Data::ApiKey(v1::ApiKeyData {
        key: Some("".into()),
        secret: Some("".into()),
        ..Default::default()
    }));
    match run(Op::ApplyOptionalClears(api)).unwrap() {
        Out::ApplyOptionalClears(p) => match p.data.unwrap() {
            Data::ApiKey(a) => {
                assert_eq!(a.key.as_deref(), Some(""));
                assert!(a.secret.is_none());
            }
            _ => panic!(),
        },
        _ => panic!(),
    };
    let mut input = login();
    login_data(&mut input).totp.as_mut().unwrap().secret = Some("".into());
    login_data(&mut input).password = None;
    let mut merged = match run(Op::MergeKeepOnNone(v1::PayloadMergeInput {
        stored: Some(login()),
        input: Some(input),
    }))
    .unwrap()
    {
        Out::MergeKeepOnNone(p) => p,
        _ => panic!(),
    };
    assert!(login_data(&mut merged).totp.is_none());
    assert_eq!(
        login_data(&mut merged).password.as_deref(),
        Some("fictional-password")
    );
    let changed = payload(Data::Secret(v1::SecretData { value: None }));
    match run(Op::MergeKeepOnNone(v1::PayloadMergeInput {
        stored: Some(login()),
        input: Some(changed.clone()),
    }))
    .unwrap()
    {
        Out::MergeKeepOnNone(p) => assert_eq!(p, changed),
        _ => panic!(),
    };
}
#[test]
fn validation_keeps_ui_paths_and_accepts_redacted_secrets() {
    let mut input = entry(login());
    input.name.clear();
    let data = login_data(input.payload.as_mut().unwrap());
    data.urls = vec!["https://ok.dev".into(), "not a url".into()];
    let totp = data.totp.as_mut().unwrap();
    totp.secret = Some("nope!".into());
    totp.period = 0;
    data.custom = vec![
        v1::CustomField::default(),
        v1::CustomField {
            key: "A".into(),
            ..Default::default()
        },
        v1::CustomField {
            key: "A".into(),
            ..Default::default()
        },
    ];
    let errors = validate(input);
    for key in [
        "name",
        "urls.1",
        "totp.secret",
        "totp.period",
        "custom.0",
        "custom.2",
    ] {
        assert!(errors.contains_key(key), "missing UI path {key}");
    }
    for p in [
        payload(Data::Secret(v1::SecretData::default())),
        payload(Data::ApiKey(v1::ApiKeyData::default())),
    ] {
        assert!(validate(entry(p)).is_empty());
    }
}
#[test]
fn dotenv_formats_and_canonical_parser_policy() {
    for value in ["BAD LINE", "1BAD=x", "KEY=\"unterminated"] {
        assert!(run(Op::ParseDotenv(text(value))).is_err());
    }
    let vars=match run(Op::ParseDotenv(text("# app url\r\nexport APP_URL=http://localhost:3000 # dev\r\nSINGLE='keep $raw'\r\nEMPTY=\r\nA=1\r\nA=2\r\nMULTI=\"line1\nline2\"\r\n"))).unwrap(){Out::ParseDotenv(v)=>v.items,_=>panic!()};
    assert_eq!(
        vars.iter()
            .map(|v| v.value.as_deref().unwrap())
            .collect::<Vec<_>>(),
        vec![
            "http://localhost:3000",
            "keep $raw",
            "",
            "2",
            "line1\nline2"
        ]
    );
    let pairs = vec![
        v1::EnvPair {
            key: "A".into(),
            value: "simple".into(),
        },
        v1::EnvPair {
            key: "B".into(),
            value: "has space".into(),
        },
        v1::EnvPair {
            key: "C".into(),
            value: "it's".into(),
        },
    ];
    for (format, expected) in [
        (
            v1::EnvFormat::Dotenv,
            "A=simple\nB=\"has space\"\nC=\"it's\"\n",
        ),
        (
            v1::EnvFormat::Export,
            "export A='simple'\nexport B='has space'\nexport C='it'\\''s'\n",
        ),
    ] {
        match run(Op::FormatEnv(v1::DomainEnvFormatInput {
            vars: pairs.clone(),
            format: format as i32,
        }))
        .unwrap()
        {
            Out::FormatEnv(v) => assert_eq!(v.value, expected),
            _ => panic!(),
        };
    }
    match run(Op::FormatEnv(v1::DomainEnvFormatInput {
        vars: pairs,
        format: v1::EnvFormat::Json as i32,
    }))
    .unwrap()
    {
        Out::FormatEnv(v) => assert_eq!(
            serde_json::from_str::<serde_json::Value>(&v.value).unwrap(),
            serde_json::json!({"A":"simple","B":"has space","C":"it's"})
        ),
        _ => panic!(),
    };
}
#[test]
fn otpauth_full_uri_defaults_and_invalid_inputs() {
    let parsed=match run(Op::ParseOtpauth(text("otpauth://totp/ACME%20Co:john.doe@email.com?secret=HXDMVJECJJWSRB3HWIZR4IFUGFTMXBOZ&issuer=ACME%20Co&algorithm=SHA256&digits=8&period=60"))).unwrap(){Out::ParseOtpauth(v)=>v.value.unwrap(),_=>panic!()};
    assert_eq!(parsed.account.as_deref(), Some("john.doe@email.com"));
    let t = parsed.totp.unwrap();
    assert_eq!(t.algorithm, v1::TotpAlgorithm::Sha256 as i32);
    assert_eq!(t.digits, 8);
    assert_eq!(t.period, 60);
    assert_eq!(t.issuer.as_deref(), Some("ACME Co"));
    let defaults = match run(Op::ParseOtpauth(text(
        "otpauth://totp/GitHub:nick?secret=jbswy3dpehpk3pxp",
    )))
    .unwrap()
    {
        Out::ParseOtpauth(v) => v.value.unwrap().totp.unwrap(),
        _ => panic!(),
    };
    assert_eq!(defaults.secret.as_deref(), Some("JBSWY3DPEHPK3PXP"));
    assert_eq!(defaults.algorithm, v1::TotpAlgorithm::Sha1 as i32);
    assert_eq!(defaults.digits, 6);
    assert_eq!(defaults.period, 30);
    for uri in [
        "otpauth://hotp/x?secret=JBSWY3DPEHPK3PXP&counter=1",
        "otpauth://totp/x",
        "otpauth://totp/x?secret=not!base32",
        "https://example.com",
        "nonsense",
    ] {
        match run(Op::ParseOtpauth(text(uri))).unwrap() {
            Out::ParseOtpauth(v) => assert!(v.value.is_none()),
            _ => panic!(),
        };
    }
}
#[test]
fn database_urls_encode_credentials_and_support_sqlite() {
    for (db, expected) in [
        (
            v1::DatabaseData {
                engine: "mysql".into(),
                host: "h".into(),
                port: Some(3306),
                database: "d".into(),
                username: "u@x".into(),
                password: Some("p:w".into()),
                options: String::new(),
            },
            "mysql://u%40x:p%3Aw@h:3306/d",
        ),
        (
            v1::DatabaseData {
                engine: "sqlite".into(),
                database: "/tmp/a.db".into(),
                ..Default::default()
            },
            "sqlite:///tmp/a.db",
        ),
    ] {
        match run(Op::DatabaseUrl(db)).unwrap() {
            Out::DatabaseUrl(v) => assert_eq!(v.value, expected),
            _ => panic!(),
        };
    }
}
fn strength(s: &str) -> v1::PasswordStrength {
    match run(Op::EstimateStrength(text(s))).unwrap() {
        Out::EstimateStrength(v) => v,
        _ => panic!(),
    }
}
#[test]
fn strength_ranking_comes_from_rust() {
    for text in ["", "password1", "aaaaaaa"] {
        assert!(strength(text).score <= 1);
    }
    assert!(strength("correct horse battery staple").score >= 3);
    assert!(strength("Tr0ub4dor&3xyzQ!").bits > strength("Tr0ub4").bits);
}
#[test]
fn secret_kind_validation_paths_cover_database_api_ssh_and_file() {
    for (p, fields) in [
        (
            payload(Data::Database(v1::DatabaseData {
                engine: "postgres".into(),
                port: Some(0),
                password: Some("".into()),
                ..Default::default()
            })),
            vec!["host", "port"],
        ),
        (
            payload(Data::ApiKey(v1::ApiKeyData {
                key: Some("".into()),
                endpoint: Some("bad url".into()),
                expires_at: Some("nope".into()),
                ..Default::default()
            })),
            vec!["key", "endpoint", "expiresAt"],
        ),
        (
            payload(Data::SshKey(v1::SshKeyData {
                private_key: Some("hello".into()),
                public_key: "nope".into(),
                ..Default::default()
            })),
            vec!["privateKey", "publicKey"],
        ),
        (
            payload(Data::File(v1::FileData {
                size: 2097152,
                ..Default::default()
            })),
            vec!["filename", "content"],
        ),
    ] {
        let errors = validate(entry(p));
        for key in fields {
            assert!(errors.contains_key(key), "missing UI path {key}");
        }
    }
}
#[test]
fn public_otpauth_preserves_rfc_sha1_algorithm_defaults() {
    // Public RPC exposes the policy's configured SHA-1 algorithm and period;
    // deterministic RFC 4226 primitive vectors remain a core-owned test concern.
    let parsed = match run(Op::ParseOtpauth(text(
        "otpauth://totp/RFC?secret=GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ",
    )))
    .unwrap()
    {
        Out::ParseOtpauth(v) => v.value.unwrap().totp.unwrap(),
        _ => panic!(),
    };
    assert_eq!(parsed.algorithm, v1::TotpAlgorithm::Sha1 as i32);
    assert_eq!(parsed.period, 30);
    assert_eq!(parsed.digits, 6);
}
