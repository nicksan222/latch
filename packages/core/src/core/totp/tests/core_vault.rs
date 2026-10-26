// Totp acceptance cases; shared imports/helpers come from core_vault_support.rs.

#[test]
fn totp_and_pinned() {
    let t = tempfile::tempdir().unwrap();
    let mut v = new_vault(&t.path().join("v"));
    let p = v.create_profile(&profile_input("acme")).unwrap();
    let (name, payload) = all_payloads().remove(0);
    let mut input = entry_input(&p.id, name, payload);
    input.favorite = true;
    let e = v.create_entry(&input).unwrap();
    let code = v.totp(&e.id).unwrap();
    assert_eq!(code.code.len(), 6);
    assert!(code.expires_in >= 1 && code.expires_in <= 30);
    let pinned = v.pinned_totp().unwrap();
    assert_eq!(pinned.len(), 1);
    assert_eq!(pinned[0].profile_name, "acme");
    let s = v
        .create_entry(&entry_input(
            &p.id,
            "s",
            EntryPayload::Secret(SecretData {
                value: Some("x".into()),
            }),
        ))
        .unwrap();
    assert_eq!(v.totp(&s.id).unwrap_err().code(), ErrorCode::NotFound);
    // otpauth URI accepted on create.
    let uri = EntryPayload::Login(LoginData {
        username: "u".into(),
        password: None,
        urls: vec![],
        totp: Some(TotpConfig {
            secret: Some(format!(
                "otpauth://totp/ACME:u?secret={TOTP_SECRET}&digits=8&period=60&algorithm=SHA256"
            )),
            algorithm: TotpAlgorithm::Sha1,
            digits: 6,
            period: 30,
            issuer: None,
        }),
        custom: vec![],
    });
    let e2 = v.create_entry(&entry_input(&p.id, "otp", uri)).unwrap();
    let full = v
        .get_entry(&GetEntryParams {
            id: e2.id.clone(),
            reveal: true,
        })
        .unwrap();
    let EntryPayload::Login(l) = full.payload else {
        panic!()
    };
    let t = l.totp.unwrap();
    assert_eq!(
        (t.digits, t.period, t.algorithm),
        (8, 60, TotpAlgorithm::Sha256)
    );
    assert_eq!(t.secret.as_deref(), Some(TOTP_SECRET));
    assert_eq!(t.issuer.as_deref(), Some("ACME"));
    assert_eq!(v.totp(&e2.id).unwrap().code.len(), 8);
}
