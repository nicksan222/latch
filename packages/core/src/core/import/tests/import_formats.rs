// Importers: parse, preview, apply, duplicates, profile mapping.
// Fixtures live in `fixtures/import/` (all secrets fake).

#[path = "../../../../tests/core_support/mod.rs"]
mod support;

use std::path::{Path, PathBuf};

use latch::core::UnlockedVault;
use latch::core::import;
use latch::protocol::*;
use support::*;

fn fx(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures/import")
        .join(name)
}

fn params(format: ImportFormat, path: &Path) -> ImportParams {
    ImportParams {
        format,
        path: path.display().to_string(),
        profile_id: None,
        skip_duplicates: true,
        passphrase: None,
    }
}

fn reveal(v: &UnlockedVault, profile: &str, name: &str) -> Entry {
    let p = v.find_profile(profile).unwrap();
    let s = v.find_entry(&p.id, name).unwrap();
    v.get_entry(&GetEntryParams {
        id: s.id,
        reveal: true,
    })
    .unwrap()
}

fn login(e: &Entry) -> &LoginData {
    match &e.payload {
        EntryPayload::Login(l) => l,
        other => panic!("not a login: {:?}", payload_kind(other)),
    }
}

fn payload_kind(p: &EntryPayload) -> EntryType {
    import::payload_type(p)
}

#[test]
fn dotenv_import() {
    let t = tempfile::tempdir().unwrap();
    let mut v = new_vault(&t.path().join("v"));
    let p = params(ImportFormat::Dotenv, &fx("sample.env"));
    let pv = import::preview(&v, &p).unwrap();
    assert_eq!(pv.items.len(), 1);
    assert_eq!(pv.items[0].entry_type, EntryType::Env);
    assert_eq!(pv.items[0].profile_name, "imported");
    assert!(!pv.items[0].duplicate);
    let r = import::apply(&mut v, &p).unwrap();
    assert_eq!((r.created, r.skipped, r.profiles_created), (1, 0, 1));
    let e = reveal(&v, "imported", "sample.env");
    let EntryPayload::Env(env) = &e.payload else {
        panic!()
    };
    let keys: Vec<_> = env.vars.iter().map(|v| v.key.as_str()).collect();
    assert_eq!(keys, ["DATABASE_URL", "PORT", "API_TOKEN"]);
    assert_eq!(
        env.vars[2].value.as_deref(),
        Some("fixture token with spaces")
    );
    // Second run: everything is a duplicate.
    let pv = import::preview(&v, &p).unwrap();
    assert!(pv.items.iter().all(|i| i.duplicate));
    let r = import::apply(&mut v, &p).unwrap();
    assert_eq!((r.created, r.skipped), (0, 1));
    // Without skip_duplicates the copy gets a unique name.
    let mut p2 = p.clone();
    p2.skip_duplicates = false;
    let r = import::apply(&mut v, &p2).unwrap();
    assert_eq!(r.created, 1);
    reveal(&v, "imported", "sample.env (2)");
    assert!(v.integrity_problems().is_empty());
}

#[test]
fn bitwarden_import() {
    let t = tempfile::tempdir().unwrap();
    let mut v = new_vault(&t.path().join("v"));
    let p = params(ImportFormat::Bitwarden, &fx("bitwarden.json"));
    let pv = import::preview(&v, &p).unwrap();
    let names: Vec<_> = pv
        .items
        .iter()
        .map(|i| (i.name.as_str(), i.profile_name.as_str(), i.entry_type))
        .collect();
    assert!(names.contains(&("GitHub", "Work", EntryType::Login)));
    assert!(names.contains(&("Acme Portal", "Work", EntryType::Login)));
    assert!(names.contains(&("Wifi", "imported", EntryType::Note)));
    assert!(names.contains(&("Visa", "imported", EntryType::Note)));
    assert!(names.contains(&("Passport", "imported", EntryType::Note)));
    assert!(names.contains(&("Deploy key", "Work", EntryType::SshKey)));
    assert!(!names.iter().any(|n| n.0 == "Deleted"));
    let r = import::apply(&mut v, &p).unwrap();
    assert_eq!(r.created, 6);
    assert_eq!(r.profiles_created, 2);

    let gh = reveal(&v, "Work", "GitHub");
    let l = login(&gh);
    assert_eq!(l.username, "alice");
    assert_eq!(l.password.as_deref(), Some("fixture-gh-pass"));
    assert_eq!(l.urls, ["https://github.com/login"]);
    assert_eq!(
        l.totp.as_ref().unwrap().secret.as_deref(),
        Some("JBSWY3DPEHPK3PXP")
    );
    assert_eq!(l.custom.len(), 2);
    assert!(l.custom.iter().any(|c| c.key == "Recovery" && c.hidden));
    assert!(l.custom.iter().any(|c| c.key == "Org" && !c.hidden));
    assert!(gh.favorite);
    assert_eq!(gh.notes, "Org owner");

    let acme = reveal(&v, "Work", "Acme Portal");
    assert_eq!(acme.tags, ["Clients"]);
    assert!(login(&acme).totp.is_some());

    let visa = reveal(&v, "imported", "Visa");
    let EntryPayload::Note(n) = &visa.payload else {
        panic!()
    };
    let body = n.body.as_deref().unwrap();
    assert!(body.contains("4111111111111111") && body.contains("backup card"));

    // Re-import: all duplicates.
    let pv = import::preview(&v, &p).unwrap();
    assert!(pv.items.iter().all(|i| i.duplicate));
}

#[test]
fn bitwarden_encrypted_rejected() {
    let t = tempfile::tempdir().unwrap();
    let f = t.path().join("enc.json");
    std::fs::write(
        &f,
        r#"{"encrypted": true, "encKeyValidation_DO_NOT_EDIT": "x", "items": []}"#,
    )
    .unwrap();
    let e = import::parse(ImportFormat::Bitwarden, &f, None).unwrap_err();
    assert!(e.to_string().contains("encrypted"));
    std::fs::write(
        &f,
        r#"{"items": [ {"type": 1, "name": "x", "login": {"password": "LEAKME" "#,
    )
    .unwrap();
    let e = import::parse(ImportFormat::Bitwarden, &f, None).unwrap_err();
    assert!(!e.to_string().contains("LEAKME"));
}

#[test]
fn onepassword_import() {
    let parsed = import::parse(ImportFormat::OnePasswordCsv, &fx("1password.csv"), None).unwrap();
    assert_eq!(parsed.entries.len(), 3);
    let aws = &parsed.entries[0];
    assert_eq!(aws.name, "AWS Console");
    assert!(aws.favorite);
    assert_eq!(aws.tags, ["cloud", "prod"]);
    let EntryPayload::Login(l) = &aws.payload else {
        panic!()
    };
    assert_eq!(l.username, "root@acme.test");
    assert!(l.totp.is_some());
    assert_eq!(parsed.entries[1].entry_type(), EntryType::Note);
    assert_eq!(parsed.entries[2].tags, ["archived"]);

    let t = tempfile::tempdir().unwrap();
    let mut v = new_vault(&t.path().join("v"));
    let pid = v.create_profile(&profile_input("acme")).unwrap().id;
    let mut p = params(ImportFormat::OnePasswordCsv, &fx("1password.csv"));
    p.profile_id = Some(pid.clone());
    let pv = import::preview(&v, &p).unwrap();
    assert!(pv.items.iter().all(|i| i.profile_name == "acme"));
    let r = import::apply(&mut v, &p).unwrap();
    assert_eq!((r.created, r.profiles_created), (3, 0));
    assert_eq!(v.list_profiles().unwrap().len(), 1);
}

#[test]
fn browser_csv_import_and_login_duplicates() {
    let chrome = import::parse(ImportFormat::BrowserCsv, &fx("chrome.csv"), None).unwrap();
    assert_eq!(chrome.entries.len(), 2);
    assert_eq!(chrome.entries[1].notes, "loyalty account");
    let ff = import::parse(ImportFormat::BrowserCsv, &fx("firefox.csv"), None).unwrap();
    let names: Vec<_> = ff.entries.iter().map(|e| e.name.as_str()).collect();
    assert_eq!(names, ["mozilla.test", "forum.example.test"]);

    let t = tempfile::tempdir().unwrap();
    let mut v = new_vault(&t.path().join("v"));
    let pid = v.create_profile(&profile_input("imported")).unwrap().id;
    // Same username + host under a different name → duplicate.
    v.create_entry(&entry_input(
        &pid,
        "My Mozilla",
        EntryPayload::Login(LoginData {
            username: "alice".into(),
            password: Some("other".into()),
            urls: vec!["https://mozilla.test/account".into()],
            totp: None,
            custom: vec![],
        }),
    ))
    .unwrap();
    let p = params(ImportFormat::BrowserCsv, &fx("firefox.csv"));
    let pv = import::preview(&v, &p).unwrap();
    assert!(pv.items[0].duplicate, "username+host match");
    assert!(!pv.items[1].duplicate);
    let r = import::apply(&mut v, &p).unwrap();
    assert_eq!((r.created, r.skipped, r.profiles_created), (1, 1, 0));
}

#[test]
fn keepass_import() {
    let parsed = import::parse(ImportFormat::KeepassXml, &fx("keepass.xml"), None).unwrap();
    let names: Vec<_> = parsed.entries.iter().map(|e| e.name.as_str()).collect();
    assert!(!names.contains(&"Trashed"), "recycle bin skipped");
    assert!(!names.contains(&"Mail (old)"), "history skipped");
    assert!(
        !names.contains(&"Files"),
        "attachment-only entry becomes files"
    );

    let t = tempfile::tempdir().unwrap();
    let mut v = new_vault(&t.path().join("v"));
    let p = params(ImportFormat::KeepassXml, &fx("keepass.xml"));
    let r = import::apply(&mut v, &p).unwrap();
    assert_eq!(r.created, 5, "{names:?}");
    assert_eq!(r.profiles_created, 2); // imported (root) + Work

    let mail = reveal(&v, "imported", "Mail");
    let l = login(&mail);
    assert_eq!(l.password.as_deref(), Some("fixture-mail-pass"));
    assert_eq!(l.totp.as_ref().unwrap().digits, 8);
    assert!(l.custom.iter().any(|c| c.key == "PIN" && c.hidden));
    assert!(l.custom.iter().any(|c| c.key == "Department" && !c.hidden));
    assert!(!l.custom.iter().any(|c| c.key == "otp"));
    assert_eq!(mail.notes, "main inbox");
    assert_eq!(mail.tags, ["personal", "mail"]);

    let vpn = reveal(&v, "Work", "VPN");
    let totp = login(&vpn).totp.as_ref().unwrap();
    assert_eq!(totp.algorithm, TotpAlgorithm::Sha256);
    assert!(login(&vpn).custom.is_empty());

    let crt = reveal(&v, "Work", "VPN - client.crt");
    let EntryPayload::File(f) = &crt.payload else {
        panic!()
    };
    use base64::Engine;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(f.content_base64.as_deref().unwrap())
        .unwrap();
    assert!(
        String::from_utf8(bytes)
            .unwrap()
            .contains("fixture attachment")
    );
    assert_eq!(f.mime, "application/x-pem-file");

    let runbook = reveal(&v, "Work", "Runbook");
    assert_eq!(runbook.tags, ["Servers"]);
    assert!(matches!(runbook.payload, EntryPayload::Note(_)));
    let files = reveal(&v, "Work", "Files - notes.txt");
    assert!(matches!(files.payload, EntryPayload::File(_)));
}

#[test]
fn generic_json_yaml_import() {
    let j = import::parse(ImportFormat::Json, &fx("generic.json"), None).unwrap();
    let names: Vec<_> = j.entries.iter().map(|e| e.name.as_str()).collect();
    assert_eq!(names, ["generic", "database", "stripe"]);
    let EntryPayload::Env(db) = &j.entries[1].payload else {
        panic!()
    };
    let keys: Vec<_> = db.vars.iter().map(|v| v.key.as_str()).collect();
    assert_eq!(keys, ["pool_max", "url"]);
    let EntryPayload::Env(top) = &j.entries[0].payload else {
        panic!()
    };
    assert!(
        top.vars
            .iter()
            .any(|v| v.key == "DEBUG" && v.value.as_deref() == Some("false"))
    );

    let y = import::parse(ImportFormat::Yaml, &fx("generic.yaml"), None).unwrap();
    let names: Vec<_> = y.entries.iter().map(|e| e.name.as_str()).collect();
    assert_eq!(names, ["generic", "hosts", "redis"]);

    let e = import::parse(ImportFormat::Yaml, &fx("sops.yaml"), None).unwrap_err();
    assert!(e.to_string().contains("sops -d"), "{e}");

    let t = tempfile::tempdir().unwrap();
    let mut v = new_vault(&t.path().join("v"));
    let r = import::apply(&mut v, &params(ImportFormat::Yaml, &fx("generic.yaml"))).unwrap();
    assert_eq!(r.created, 3);
}

#[test]
fn invalid_items_are_warned_not_fatal() {
    let t = tempfile::tempdir().unwrap();
    let f = t.path().join("bad.json");
    // A file entry with invalid base64 inside a Latch JSON export.
    std::fs::write(
        &f,
        r#"{"format":"latch-archive","version":1,"exportedAt":"","vaultId":"x","profiles":[],
            "entries":[{"id":"a","profileId":"p","name":"bad","payload":{"type":"file","data":{"filename":"f","mime":"","contentBase64":"!!!","size":0}}},
                       {"id":"b","profileId":"p","name":"ok","payload":{"type":"secret","data":{"value":"v"}}}]}"#,
    )
    .unwrap();
    let mut v = new_vault(&t.path().join("v"));
    let p = params(ImportFormat::Json, &f);
    let pv = import::preview(&v, &p).unwrap();
    assert_eq!(pv.items.len(), 1);
    assert!(pv.warnings.iter().any(|w| w.contains("bad")));
    let r = import::apply(&mut v, &p).unwrap();
    assert_eq!((r.created, r.skipped), (1, 1));
}

#[test]
fn unknown_profile_id_is_not_found() {
    let t = tempfile::tempdir().unwrap();
    let v = new_vault(&t.path().join("v"));
    let mut p = params(ImportFormat::Dotenv, &fx("sample.env"));
    p.profile_id = Some("aaaaaaaaaaaaaaaaaaaaaaaaaa".into());
    assert_eq!(
        import::preview(&v, &p).unwrap_err().code(),
        ErrorCode::NotFound
    );
}

#[test]
fn import_commits_once() {
    let t = tempfile::tempdir().unwrap();
    let path = t.path().join("v");
    let mut v = new_vault(&path);
    let before = git(&path, &["rev-list", "--count", "HEAD"]);
    import::apply(
        &mut v,
        &params(ImportFormat::Bitwarden, &fx("bitwarden.json")),
    )
    .unwrap();
    let after = git(&path, &["rev-list", "--count", "HEAD"]);
    assert_eq!(
        after.trim().parse::<u32>().unwrap(),
        before.trim().parse::<u32>().unwrap() + 1
    );
    let msg = git(&path, &["log", "-1", "--format=%s"]);
    assert!(msg.starts_with("import:") && !msg.contains("GitHub"));
    assert!(git(&path, &["status", "--porcelain"]).trim().is_empty());
}
