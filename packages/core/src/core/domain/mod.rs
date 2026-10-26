//! Pure domain operations shared by UI clients; no vault access or side effects.
use super::{
    environment,
    error::{Error, Result},
    format, secrets, totp,
};
use crate::protocol::*;
use std::collections::BTreeMap;
mod password;
mod payload;
pub(crate) mod validation;

pub use password::{estimate_strength, generate_password};
pub use payload::*;
pub use validation::*;

/// Dispatch the typed, closed set of domain operations.
pub fn call(request: DomainRequest) -> Result<DomainResponse> {
    execute(request)
}

fn execute(request: DomainRequest) -> Result<DomainResponse> {
    match request {
        DomainRequest::ValidateEntry(input) => {
            validate_entry(input).map(DomainResponse::ValidateEntry)
        }
        DomainRequest::ValidateProfile(input) => {
            validate_profile(input).map(DomainResponse::ValidateProfile)
        }
        DomainRequest::ValidateRecipient(input) => {
            validate_recipient(input).map(DomainResponse::ValidateRecipient)
        }
        DomainRequest::RecipientKeyKind(input) => {
            recipient_key_kind(input).map(DomainResponse::RecipientKeyKind)
        }
        DomainRequest::ParseDotenv(input) => parse_dotenv(input).map(DomainResponse::ParseDotenv),
        DomainRequest::FormatEnv(input) => format_env(input).map(DomainResponse::FormatEnv),
        DomainRequest::ParseOtpauth(input) => {
            parse_otpauth(input).map(DomainResponse::ParseOtpauth)
        }
        DomainRequest::GeneratePassword(input) => {
            generate_password(input).map(DomainResponse::GeneratePassword)
        }
        DomainRequest::EstimateStrength(input) => {
            estimate_strength(input).map(DomainResponse::EstimateStrength)
        }
        DomainRequest::EntryDefaults(input) => {
            entry_defaults(input).map(DomainResponse::EntryDefaults)
        }
        DomainRequest::DatabaseUrl(input) => database_url(input).map(DomainResponse::DatabaseUrl),
        DomainRequest::SecretPaths(input) => secret_paths(input).map(DomainResponse::SecretPaths),
        DomainRequest::ReadField(input) => read_field(input).map(DomainResponse::ReadField),
        DomainRequest::RedactPayload(input) => {
            redact_payload(input).map(DomainResponse::RedactPayload)
        }
        DomainRequest::MergeKeepOnNone(input) => {
            merge_keep_on_none(input).map(DomainResponse::MergeKeepOnNone)
        }
        DomainRequest::ApplyOptionalClears(input) => {
            apply_optional_clears(input).map(DomainResponse::ApplyOptionalClears)
        }
        DomainRequest::FlattenEntry(input) => {
            flatten_entry(input).map(DomainResponse::FlattenEntry)
        }
        DomainRequest::MergeEntries(input) => {
            merge_entries(input).map(DomainResponse::MergeEntries)
        }
        DomainRequest::DefaultBrowserSettings(input) => {
            default_browser_settings(input).map(DomainResponse::DefaultBrowserSettings)
        }
        DomainRequest::FileDigest(input) => file_digest(input).map(DomainResponse::FileDigest),
        DomainRequest::DiffRows(input) => diff_rows(input).map(DomainResponse::DiffRows),
    }
}

pub fn parse_dotenv(text: String) -> Result<Vec<EnvVar>> {
    environment::parse(&text).map_err(|_| Error::invalid("Invalid dotenv input"))
}
pub fn format_env(input: DomainEnvFormatInput) -> Result<String> {
    Ok(environment::format_pairs(&input.vars, input.format))
}
pub fn default_browser_settings(_: ()) -> Result<BrowserSettings> {
    Ok(format::BrowserDoc::default().to_protocol())
}
pub fn parse_otpauth(uri: String) -> Result<Option<ParsedOtpauth>> {
    let uri = zeroize::Zeroizing::new(uri);
    let Ok(t) = totp::parse_otpauth(&uri) else {
        return Ok(None);
    };
    let u = url::Url::parse(uri.trim()).map_err(|_| Error::invalid("Invalid otpauth URI"))?;
    let label = totp::percent_decode(u.path().trim_start_matches('/'));
    let account = label
        .split_once(':')
        .map_or(label.as_str(), |(_, a)| a)
        .trim();
    Ok(Some(ParsedOtpauth {
        account: (!account.is_empty()).then(|| account.to_owned()),
        totp: TotpConfig {
            secret: Some(t.secret.clone()),
            algorithm: t.algorithm,
            digits: t.digits,
            period: t.period,
            issuer: t.issuer.clone(),
        },
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn known_ui_strength_and_otpauth_cases() {
        assert_eq!(estimate_strength(String::new()).unwrap().score, 0);
        assert!(estimate_strength("password1".into()).unwrap().score <= 1);
        assert!(
            estimate_strength("correct horse battery staple".into())
                .unwrap()
                .score
                >= 3
        );
        let parsed = parse_otpauth("otpauth://totp/GitHub:nick?secret=jbswy3dpehpk3pxp".into())
            .unwrap()
            .unwrap();
        assert_eq!(parsed.account.as_deref(), Some("nick"));
        assert_eq!(parsed.totp.issuer.as_deref(), Some("GitHub"));
        assert!(
            parse_otpauth("otpauth://totp/x?secret=JBSWY3DPEHPK3PXP&algorithm=MD5".into())
                .unwrap()
                .is_none()
        );
    }
    #[test]
    fn otpauth_account_preserves_plus_in_email_label() {
        let parsed = parse_otpauth(
            "otpauth://totp/Acme:alice+tag@example.com?secret=JBSWY3DPEHPK3PXP".into(),
        )
        .unwrap()
        .unwrap();
        assert_eq!(parsed.account.as_deref(), Some("alice+tag@example.com"));
    }
    #[test]
    fn otpauth_account_and_issuer_preserve_path_characters() {
        for label in [
            "ACME&Co+Ops:alice&bob+tag@example.com",
            "ACME%26Co%2BOps:alice%26bob%2Btag%40example.com",
        ] {
            let parsed = parse_otpauth(format!("otpauth://totp/{label}?secret=JBSWY3DPEHPK3PXP"))
                .unwrap()
                .unwrap();
            assert_eq!(parsed.account.as_deref(), Some("alice&bob+tag@example.com"));
            assert_eq!(parsed.totp.issuer.as_deref(), Some("ACME&Co+Ops"));
        }
    }

    #[test]
    fn masked_and_optional_values() {
        let stored = EntryPayload::ApiKey(ApiKeyData {
            key: Some("keep".into()),
            secret: Some("clear".into()),
            endpoint: None,
            expires_at: None,
        });
        let input = EntryPayload::ApiKey(ApiKeyData {
            key: None,
            secret: Some(String::new()),
            endpoint: None,
            expires_at: None,
        });
        let merged = merge_keep_on_none(PayloadMergeInput { stored, input }).unwrap();
        let EntryPayload::ApiKey(d) = &merged else {
            panic!("kind")
        };
        assert_eq!(d.key.as_deref(), Some("keep"));
        assert_eq!(d.secret, None);
        let redacted = redact_payload(merged).unwrap();
        assert_eq!(redacted.redacted, vec!["key"]);
        assert_eq!(
            read_field(ReadFieldInput {
                payload: redacted.payload,
                path: "key".into()
            })
            .unwrap(),
            None
        );
    }
    #[test]
    fn validation_paths_and_canonical_name_limit() {
        let mut i = entry_defaults(EntryDefaultsInput {
            profile_id: "p".into(),
            entry_type: EntryType::Secret,
        })
        .unwrap();
        assert!(validate_entry(i.clone()).unwrap().contains_key("name"));
        i.name = "x".repeat(201);
        i.payload = EntryPayload::Secret(SecretData { value: None });
        assert!(validate_entry(i.clone()).unwrap().is_empty());
        i.name = "bad/name".into();
        assert!(validate_entry(i).unwrap().contains_key("name"));
    }
    #[test]
    fn safe_debug_and_wire_contract() {
        let req = DomainRequest::EstimateStrength("never-log-this".into());
        assert!(!format!("{req:?}").contains("never-log-this"));
        let v = serde_json::to_value(req).unwrap();
        assert_eq!(v["operation"], "estimate_strength");
        assert_eq!(v["input"], "never-log-this");
        assert_eq!(
            serde_json::to_value(call(DomainRequest::DefaultBrowserSettings(())).unwrap()).unwrap()
                ["operation"],
            "default_browser_settings"
        );
    }
}

#[cfg(test)]
mod regression_tests {
    use super::*;
    use base64::Engine;
    fn entry(payload: EntryPayload) -> EntryInput {
        EntryInput {
            profile_id: "p".into(),
            name: "x".into(),
            tags: vec![],
            notes: String::new(),
            favorite: false,
            payload,
        }
    }
    fn file(text: &str) -> EntryInput {
        entry(EntryPayload::File(FileData {
            filename: "same.txt".into(),
            mime: "text/plain".into(),
            content_base64: Some(base64::engine::general_purpose::STANDARD.encode(text)),
            size: text.len() as u64,
        }))
    }
    #[test]
    fn catalog_defaults_redact_without_validation() {
        let catalog = catalog().unwrap();
        assert_eq!(
            catalog
                .entry_types
                .iter()
                .map(|k| k.r#type)
                .collect::<Vec<_>>(),
            vec![
                EntryType::Login,
                EntryType::Secret,
                EntryType::Env,
                EntryType::ApiKey,
                EntryType::Database,
                EntryType::SshKey,
                EntryType::Note,
                EntryType::File
            ]
        );
        for kind in catalog.entry_types {
            let paths = secret_paths(kind.default_payload.clone()).unwrap();
            let redacted = redact_payload(kind.default_payload).unwrap();
            assert_eq!(paths, redacted.redacted);
            assert!(secret_paths(redacted.payload).unwrap().is_empty());
        }
        assert_eq!(catalog.default_totp.period, 30);
    }
    #[test]
    fn env_validation_paths_and_parser_round_trip() {
        let input = entry(EntryPayload::Env(EnvData {
            vars: vec![
                EnvVar {
                    key: "1X".into(),
                    value: None,
                    hidden: true,
                    comment: None,
                },
                EnvVar {
                    key: "A".into(),
                    value: None,
                    hidden: false,
                    comment: None,
                },
                EnvVar {
                    key: "A".into(),
                    value: None,
                    hidden: false,
                    comment: None,
                },
            ],
        }));
        assert_eq!(
            validate_entry(input).unwrap(),
            BTreeMap::from([
                ("vars.0".into(), "Invalid variable name".into()),
                ("vars.2".into(), "Duplicate variable".into())
            ])
        );
        let pairs = vec![EnvPair {
            key: "TOKEN".into(),
            value: "one\ntwo".into(),
        }];
        let text = format_env(DomainEnvFormatInput {
            vars: pairs,
            format: EnvFormat::Dotenv,
        })
        .unwrap();
        assert_eq!(
            parse_dotenv(text).unwrap()[0].value.as_deref(),
            Some("one\ntwo")
        );
    }
    #[test]
    fn file_diff_never_displays_content_and_merge_keeps_unit() {
        let mine = file("one!");
        let theirs = file("two!");
        assert_eq!(
            flatten_entry(mine.clone()).unwrap()["file"],
            "same.txt · text/plain · 4 B"
        );
        let rows = diff_rows(DiffRowsInput {
            mine: Some(mine.clone()),
            theirs: Some(theirs.clone()),
        })
        .unwrap();
        let row = rows.iter().find(|r| r.field == "file").unwrap();
        assert!(row.differs && row.content_differs);
        let wire = serde_json::to_string(&rows).unwrap();
        assert!(!wire.contains("one!") && !wire.contains("b25lIQ=="));
        let merged = merge_entries(MergeEntriesInput {
            mine: mine.clone(),
            theirs: theirs.clone(),
            from_theirs: vec!["file".into()],
        })
        .unwrap();
        assert_eq!(merged.payload, theirs.payload);
        assert_ne!(
            file_digest(Some(mine)).unwrap(),
            file_digest(Some(theirs)).unwrap()
        );
    }
    #[test]
    fn file_hash_accepts_browser_base64_draft_forms() {
        let canonical = file("one!");
        let mut draft = canonical.clone();
        let EntryPayload::File(f) = &mut draft.payload else {
            panic!("kind")
        };
        f.content_base64 = Some(" b25l\nIQ ".into());
        assert_eq!(
            file_digest(Some(canonical)).unwrap(),
            file_digest(Some(draft)).unwrap()
        );
    }
    #[test]
    fn change_kind_keeps_redacted_null() {
        let stored = EntryPayload::Note(NoteData {
            body: Some("secret".into()),
        });
        let input = EntryPayload::Secret(SecretData { value: None });
        assert_eq!(
            merge_keep_on_none(PayloadMergeInput {
                stored,
                input: input.clone()
            })
            .unwrap(),
            input
        );
    }
    #[test]
    fn valid_recipient_parser_rejects_old_fake_keys() {
        use std::str::FromStr;
        let identity = age::x25519::Identity::generate();
        assert_eq!(
            recipient_key_kind(identity.to_public().to_string()).unwrap(),
            Some(DomainRecipientKeyKind::Age)
        );
        assert!(
            recipient_key_kind(format!("age1{}", "q".repeat(58)))
                .unwrap()
                .is_none()
        );
        let _ = age::x25519::Recipient::from_str(&identity.to_public().to_string()).unwrap();
    }
    #[test]
    fn password_is_bounded_random_and_uses_allowed_alphabet() {
        let a = generate_password(64).unwrap();
        let b = generate_password(64).unwrap();
        assert_eq!(a.len(), 64);
        assert_ne!(a, b);
        assert!(a.bytes().all(|c| {
            b"ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz23456789!@#$%^&*-_=+".contains(&c)
        }));
        assert!(generate_password(4097).is_err());
    }
    #[test]
    fn profile_errors_use_stable_paths() {
        let input = ProfileInput {
            name: String::new(),
            color: "red".into(),
            icon: "folder".into(),
            description: String::new(),
            pinned: false,
            browser: default_browser_settings(()).unwrap(),
        };
        let errors = validate_profile(input).unwrap();
        assert!(errors.contains_key("name"));
        assert!(errors.contains_key("color"));
    }
}

#[cfg(test)]
mod draft_tests {
    use super::*;
    #[test]
    fn masked_totp_and_file_drafts_preserve_nulls_and_size() {
        let p = EntryPayload::Login(LoginData {
            username: "u".into(),
            password: None,
            urls: vec![],
            totp: Some(TotpConfig {
                secret: None,
                algorithm: TotpAlgorithm::Sha1,
                digits: 6,
                period: 30,
                issuer: None,
            }),
            custom: vec![],
        });
        assert_eq!(
            merge_keep_on_none(PayloadMergeInput {
                stored: p.clone(),
                input: p.clone()
            })
            .unwrap(),
            p
        );
        let p = EntryPayload::File(FileData {
            filename: "x".into(),
            mime: "text/plain".into(),
            content_base64: None,
            size: 256,
        });
        assert_eq!(
            merge_keep_on_none(PayloadMergeInput {
                stored: p.clone(),
                input: p.clone()
            })
            .unwrap(),
            p
        );
    }
    #[test]
    fn dotted_keys_and_optional_totp_clear_survive_draft_transforms() {
        let p = EntryPayload::Login(LoginData {
            username: String::new(),
            password: Some("keep".into()),
            urls: vec![],
            totp: Some(TotpConfig {
                secret: Some("".into()),
                algorithm: TotpAlgorithm::Sha1,
                digits: 6,
                period: 30,
                issuer: None,
            }),
            custom: vec![CustomField {
                key: "key.with.dot".into(),
                value: Some("value".into()),
                hidden: true,
            }],
        });
        assert_eq!(
            read_field(ReadFieldInput {
                payload: p.clone(),
                path: "custom.key.with.dot".into()
            })
            .unwrap()
            .as_deref(),
            Some("value")
        );
        let merged = merge_keep_on_none(PayloadMergeInput {
            stored: p.clone(),
            input: p,
        })
        .unwrap();
        let EntryPayload::Login(data) = merged else {
            panic!("kind")
        };
        assert!(data.totp.is_none());
    }
}
