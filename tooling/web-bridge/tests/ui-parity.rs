//! UI-domain cases migrated from TypeScript. Every operation executes Rust policy.
use latch::{core::domain, protocol::DomainRequest};
use serde_json::{Value, json};

fn run(operation: &str, input: Value) -> Value {
    let request: DomainRequest =
        serde_json::from_value(json!({"operation":operation,"input":input})).unwrap();
    let response = serde_json::to_value(domain::call(request).unwrap()).unwrap();
    assert_eq!(response["operation"], operation);
    response["output"].clone()
}
fn entry(payload: Value) -> Value {
    json!({"profileId":"p","name":"Example","tags":[],"notes":"mine","favorite":false,"payload":payload})
}
fn login() -> Value {
    json!({"type":"login","data":{"username":"nick","password":"fictional-password","urls":["https://github.com"],"totp":{"secret":"JBSWY3DPEHPK3PXP","algorithm":"SHA1","digits":6,"period":30,"issuer":null},"custom":[{"key":"Recovery","value":"fictional-recovery","hidden":true}]}})
}

#[test]
fn login_redaction_merge_and_field_selection() {
    let stored = login();
    let paths = run("secret_paths", stored.clone());
    assert_eq!(paths, json!(["password", "totp.secret", "custom.Recovery"]));
    let redacted = run("redact_payload", stored.clone());
    assert_eq!(redacted["redacted"], paths);
    assert_eq!(run("secret_paths", redacted["payload"].clone()), json!([]));
    let mut input = redacted["payload"].clone();
    input["data"]["username"] = json!("changed");
    let merged = run("merge_keep_on_none", json!({"stored":stored,"input":input}));
    assert_eq!(merged["data"]["username"], "changed");
    assert_eq!(merged["data"]["password"], "fictional-password");
    let mine = entry(login());
    let mut theirs = mine.clone();
    theirs["name"] = json!("Organization");
    theirs["notes"] = json!("theirs");
    theirs["payload"]["data"]["password"] = json!("replacement");
    let flat = run("flatten_entry", theirs.clone());
    assert_eq!(flat["custom.Recovery"], "fictional-recovery");
    assert_eq!(flat["password"], "replacement");
    let merged = run(
        "merge_entries",
        json!({"mine":mine,"theirs":theirs,"fromTheirs":["name","password"]}),
    );
    assert_eq!(merged["name"], "Organization");
    assert_eq!(merged["notes"], "mine");
    assert_eq!(merged["payload"]["data"]["password"], "replacement");
}

#[test]
fn optional_clears_and_payload_type_changes() {
    let cleared = run(
        "apply_optional_clears",
        json!({"type":"api_key","data":{"key":"","secret":"","endpoint":null,"expiresAt":null}}),
    );
    assert_eq!(cleared["data"]["key"], "");
    assert!(cleared["data"]["secret"].is_null());
    let mut input = login();
    input["data"]["totp"]["secret"] = json!("");
    input["data"]["password"] = Value::Null;
    let merged = run(
        "merge_keep_on_none",
        json!({"stored":login(),"input":input}),
    );
    assert!(merged["data"]["totp"].is_null());
    assert_eq!(merged["data"]["password"], "fictional-password");
    let changed = json!({"type":"secret","data":{"value":null}});
    assert_eq!(
        run(
            "merge_keep_on_none",
            json!({"stored":login(),"input":changed})
        ),
        changed
    );
}

#[test]
fn validation_keeps_ui_paths_and_accepts_redacted_secrets() {
    let mut input = entry(login());
    input["name"] = json!("");
    input["payload"]["data"]["urls"] = json!(["https://ok.dev", "not a url"]);
    input["payload"]["data"]["totp"]["secret"] = json!("nope!");
    input["payload"]["data"]["totp"]["period"] = json!(0);
    input["payload"]["data"]["custom"] = json!([
        {"key":"","value":"","hidden":false},
        {"key":"A","value":"","hidden":false},
        {"key":"A","value":"","hidden":false}
    ]);
    let errors = run("validate_entry", input);
    for key in [
        "name",
        "urls.1",
        "totp.secret",
        "totp.period",
        "custom.0",
        "custom.2",
    ] {
        assert!(errors.get(key).is_some(), "missing UI path {key}");
    }
    for payload in [
        json!({"type":"secret","data":{"value":null}}),
        json!({"type":"api_key","data":{"key":null,"secret":null,"endpoint":null,"expiresAt":null}}),
    ] {
        assert_eq!(run("validate_entry", entry(payload)), json!({}));
    }
}

#[test]
fn dotenv_formats_and_canonical_parser_policy() {
    // Rust fails closed on malformed lines; the removed TS parser silently skipped them.
    for text in ["BAD LINE", "1BAD=x", "KEY=\"unterminated"] {
        let request: DomainRequest =
            serde_json::from_value(json!({"operation":"parse_dotenv","input":text})).unwrap();
        assert!(domain::call(request).is_err());
    }
    let vars = run(
        "parse_dotenv",
        json!(
            "# app url\r\nexport APP_URL=http://localhost:3000 # dev\r\nSINGLE='keep $raw'\r\nEMPTY=\r\nA=1\r\nA=2\r\nMULTI=\"line1\nline2\"\r\n"
        ),
    );
    assert_eq!(vars[0]["value"], "http://localhost:3000");
    assert_eq!(vars[1]["value"], "keep $raw");
    assert_eq!(vars[2]["value"], "");
    assert_eq!(vars[3]["value"], "2");
    assert_eq!(vars[4]["value"], "line1\nline2");
    let pairs = json!([{"key":"A","value":"simple"},{"key":"B","value":"has space"},{"key":"C","value":"it's"}]);
    assert_eq!(
        run("format_env", json!({"vars":pairs,"format":"dotenv"})),
        "A=simple\nB=\"has space\"\nC=\"it's\"\n"
    );
    assert_eq!(
        run("format_env", json!({"vars":pairs,"format":"export"})),
        "export A='simple'\nexport B='has space'\nexport C='it'\\''s'\n"
    );
    let text = run("format_env", json!({"vars":pairs,"format":"json"}));
    let parsed: Value = serde_json::from_str(text.as_str().unwrap()).unwrap();
    assert_eq!(parsed, json!({"A":"simple","B":"has space","C":"it's"}));
}

#[test]
fn otpauth_full_uri_defaults_and_invalid_inputs() {
    let parsed = run(
        "parse_otpauth",
        json!(
            "otpauth://totp/ACME%20Co:john.doe@email.com?secret=HXDMVJECJJWSRB3HWIZR4IFUGFTMXBOZ&issuer=ACME%20Co&algorithm=SHA256&digits=8&period=60"
        ),
    );
    assert_eq!(parsed["account"], "john.doe@email.com");
    assert_eq!(parsed["totp"]["algorithm"], "SHA256");
    assert_eq!(parsed["totp"]["digits"], 8);
    assert_eq!(parsed["totp"]["period"], 60);
    assert_eq!(parsed["totp"]["issuer"], "ACME Co");
    let defaults = run(
        "parse_otpauth",
        json!("otpauth://totp/GitHub:nick?secret=jbswy3dpehpk3pxp"),
    );
    assert_eq!(defaults["totp"]["secret"], "JBSWY3DPEHPK3PXP");
    assert_eq!(defaults["totp"]["algorithm"], "SHA1");
    assert_eq!(defaults["totp"]["digits"], 6);
    assert_eq!(defaults["totp"]["period"], 30);
    for uri in [
        "otpauth://hotp/x?secret=JBSWY3DPEHPK3PXP&counter=1",
        "otpauth://totp/x",
        "otpauth://totp/x?secret=not!base32",
        "https://example.com",
        "nonsense",
    ] {
        assert!(run("parse_otpauth", json!(uri)).is_null());
    }
}

#[test]
fn database_urls_encode_credentials_and_support_sqlite() {
    assert_eq!(
        run(
            "database_url",
            json!({"engine":"mysql","host":"h","port":3306,"database":"d","username":"u@x","password":"p:w","options":""})
        ),
        "mysql://u%40x:p%3Aw@h:3306/d"
    );
    assert_eq!(
        run(
            "database_url",
            json!({"engine":"sqlite","host":"","port":null,"database":"/tmp/a.db","username":"","password":null,"options":""})
        ),
        "sqlite:///tmp/a.db"
    );
}

#[test]
fn strength_ranking_comes_from_rust() {
    for text in ["", "password1", "aaaaaaa"] {
        assert!(
            run("estimate_strength", json!(text))["score"]
                .as_u64()
                .unwrap()
                <= 1
        );
    }
    assert!(
        run("estimate_strength", json!("correct horse battery staple"))["score"]
            .as_u64()
            .unwrap()
            >= 3
    );
    assert!(
        run("estimate_strength", json!("Tr0ub4dor&3xyzQ!"))["bits"]
            .as_u64()
            .unwrap()
            > run("estimate_strength", json!("Tr0ub4"))["bits"]
                .as_u64()
                .unwrap()
    );
}

#[test]
fn secret_kind_validation_paths_cover_database_api_ssh_and_file() {
    for (payload, fields) in [
        (
            json!({"type":"database","data":{"engine":"postgres","host":"","port":0,"database":"","username":"","password":"","options":""}}),
            vec!["host", "port"],
        ),
        (
            json!({"type":"api_key","data":{"key":"","secret":null,"endpoint":"bad url","expiresAt":"nope"}}),
            vec!["key", "endpoint", "expiresAt"],
        ),
        (
            json!({"type":"ssh_key","data":{"privateKey":"hello","publicKey":"nope","passphrase":null,"comment":""}}),
            vec!["privateKey", "publicKey"],
        ),
        (
            json!({"type":"file","data":{"filename":"","mime":"","contentBase64":null,"size":2097152}}),
            vec!["filename", "content"],
        ),
    ] {
        let errors = run("validate_entry", entry(payload));
        for field in fields {
            assert!(errors.get(field).is_some(), "missing UI path {field}");
        }
    }
}

#[test]
fn hotp_counter_vectors_use_the_rust_totp_primitive() {
    for (counter, expected) in [(0, "755224"), (1, "287082"), (2, "359152"), (9, "520489")] {
        assert_eq!(
            latch::core::totp::totp_raw(
                b"12345678901234567890",
                counter * 30,
                30,
                latch::protocol::TotpAlgorithm::Sha1,
                6
            ),
            expected
        );
    }
}
