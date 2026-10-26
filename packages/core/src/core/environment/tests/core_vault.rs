// Environment acceptance cases; shared imports/helpers come from core_vault_support.rs.

#[test]
fn env_resolve_and_format() {
    let t = tempfile::tempdir().unwrap();
    let mut v = new_vault(&t.path().join("v"));
    let p = v.create_profile(&profile_input("acme")).unwrap();
    let base = v
        .create_entry(&entry_input(
            &p.id,
            "base",
            EntryPayload::Env(EnvData {
                vars: latch::core::environment::parse("A=1\nB=\"two words\"\n").unwrap(),
            }),
        ))
        .unwrap();
    let over = v
        .create_entry(&entry_input(
            &p.id,
            "override",
            EntryPayload::Env(EnvData {
                vars: latch::core::environment::parse("B=override\nC='its'\n").unwrap(),
            }),
        ))
        .unwrap();
    v.create_entry(&entry_input(
        &p.id,
        "not env",
        EntryPayload::Secret(SecretData {
            value: Some("x".into()),
        }),
    ))
    .unwrap();
    let all = v
        .env_resolve(&EnvResolveParams {
            profile_id: p.id.clone(),
            entry_ids: None,
        })
        .unwrap();
    let pairs: Vec<(String, String)> = all
        .vars
        .iter()
        .map(|p| (p.key.clone(), p.value.clone()))
        .collect();
    assert_eq!(
        pairs,
        vec![
            ("A".into(), "1".into()),
            ("B".into(), "override".into()),
            ("C".into(), "its".into())
        ]
    );
    // Named order: later wins.
    let rev = v
        .env_resolve(&EnvResolveParams {
            profile_id: p.id.clone(),
            entry_ids: Some(vec![over.id.clone(), base.id.clone()]),
        })
        .unwrap();
    assert_eq!(
        rev.vars.iter().find(|p| p.key == "B").unwrap().value,
        "two words"
    );
    let text = v
        .env_format(&EnvFormatParams {
            profile_id: p.id.clone(),
            entry_ids: None,
            format: EnvFormat::Dotenv,
        })
        .unwrap()
        .text;
    assert_eq!(text, "A=1\nB=override\nC=its\n");
    let json = v
        .env_format(&EnvFormatParams {
            profile_id: p.id.clone(),
            entry_ids: None,
            format: EnvFormat::Json,
        })
        .unwrap()
        .text;
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&json).unwrap()["B"],
        "override"
    );
    let exp = v
        .env_format(&EnvFormatParams {
            profile_id: p.id.clone(),
            entry_ids: None,
            format: EnvFormat::Export,
        })
        .unwrap()
        .text;
    assert!(exp.starts_with("export A='1'\n"));
    let secret_id = v.find_entry(&p.id, "not env").unwrap().id;
    assert_eq!(
        v.env_resolve(&EnvResolveParams {
            profile_id: p.id.clone(),
            entry_ids: Some(vec![secret_id])
        })
        .unwrap_err()
        .code(),
        ErrorCode::InvalidParams
    );
}
