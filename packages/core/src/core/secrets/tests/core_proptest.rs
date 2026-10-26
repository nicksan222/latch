// Property tests: .env format/parse round trip, on-disk entry document
// serialization, and "update with a redacted payload changes nothing".

use latch::core::{environment as env, format::*, secrets};
use latch::protocol::{EnvFormat, EnvVar, TotpAlgorithm};
use proptest::prelude::*;

fn key() -> impl Strategy<Value = String> {
    "[A-Za-z_][A-Za-z0-9_]{0,15}"
}

fn vars() -> impl Strategy<Value = Vec<EnvVar>> {
    prop::collection::vec(
        (
            key(),
            any::<String>(),
            any::<bool>(),
            prop::option::of("[^\n\r]{1,20}"),
        ),
        0..8,
    )
    .prop_map(|v| {
        let mut seen = std::collections::HashSet::new();
        v.into_iter()
            .filter(|(k, ..)| seen.insert(k.clone()))
            .map(|(key, value, hidden, comment)| EnvVar {
                key,
                value: Some(value),
                hidden,
                comment,
            })
            .collect()
    })
}

fn s() -> impl Strategy<Value = String> {
    any::<String>()
}

fn opt() -> impl Strategy<Value = Option<String>> {
    prop::option::of(any::<String>())
}

fn body() -> impl Strategy<Value = EntryBody> {
    let totp = (
        "[A-Z2-7]{16,32}",
        prop_oneof![
            Just(TotpAlgorithm::Sha1),
            Just(TotpAlgorithm::Sha256),
            Just(TotpAlgorithm::Sha512)
        ],
        prop_oneof![Just(6u8), Just(8u8)],
        1u32..120,
        opt(),
    )
        .prop_map(|(secret, algorithm, digits, period, issuer)| TotpDoc {
            secret,
            algorithm,
            digits,
            period,
            issuer,
        });
    let custom = prop::collection::vec((key(), s(), any::<bool>()), 0..4).prop_map(|v| {
        let mut seen = std::collections::HashSet::new();
        v.into_iter()
            .filter(|(k, ..)| seen.insert(k.clone()))
            .map(|(key, value, hidden)| CustomFieldDoc { key, value, hidden })
            .collect::<Vec<_>>()
    });
    let env_vars = prop::collection::vec(
        (key(), s(), any::<bool>(), prop::option::of("[^\n]{1,10}")),
        0..5,
    )
    .prop_map(|v| {
        let mut seen = std::collections::HashSet::new();
        v.into_iter()
            .filter(|(k, ..)| seen.insert(k.clone()))
            .map(|(key, value, hidden, comment)| EnvVarDoc {
                key,
                value,
                hidden,
                comment,
            })
            .collect::<Vec<_>>()
    });
    prop_oneof![
        (
            s(),
            opt(),
            prop::collection::vec("[a-z]{1,8}://[a-z.]{1,10}", 0..3),
            prop::option::of(totp),
            custom
        )
            .prop_map(|(username, password, urls, totp, custom)| EntryBody::Login(
                LoginDoc {
                    username,
                    password,
                    urls,
                    totp,
                    custom
                }
            )),
        s().prop_map(|value| EntryBody::Secret(SecretDoc {
            value,
            hidden: true
        })),
        env_vars.prop_map(|vars| EntryBody::Env(EnvDoc { vars })),
        (
            "[a-z]{1,8}\\.[a-z]{1,3}",
            prop::collection::vec(any::<u8>(), 0..64)
        )
            .prop_map(|(filename, bytes)| {
                use base64::Engine;
                EntryBody::File(FileDoc {
                    filename,
                    mime: "application/octet-stream".into(),
                    size: bytes.len() as u64,
                    content_base64: base64::engine::general_purpose::STANDARD.encode(bytes),
                })
            }),
        s().prop_map(|body| EntryBody::Note(NoteDoc { body })),
        (s(), s(), opt(), s()).prop_map(|(private_key, public_key, passphrase, comment)| {
            EntryBody::SshKey(SshKeyDoc {
                private_key,
                public_key,
                passphrase,
                comment,
            })
        }),
        (
            "[a-z]{1,8}",
            s(),
            prop::option::of(any::<u16>()),
            s(),
            s(),
            opt(),
            s()
        )
            .prop_map(
                |(engine, host, port, database, username, password, options)| {
                    EntryBody::Database(DatabaseDoc {
                        engine,
                        host,
                        port,
                        database,
                        username,
                        password,
                        options,
                    })
                }
            ),
        (s(), opt(), opt(), opt()).prop_map(|(key, secret, endpoint, expires_at)| {
            EntryBody::ApiKey(ApiKeyDoc {
                key,
                secret,
                endpoint,
                expires_at,
            })
        }),
    ]
}

fn doc() -> impl Strategy<Value = EntryDoc> {
    (
        s(),
        prop::collection::vec(s(), 0..3),
        s(),
        any::<bool>(),
        body(),
    )
        .prop_map(|(name, tags, notes, favorite, body)| EntryDoc {
            version: 1,
            id: latch::core::ids::new_id(),
            profile_id: latch::core::ids::new_id(),
            name,
            tags,
            notes,
            favorite,
            created_at: now(),
            updated_at: now(),
            body,
        })
}

proptest! {
    #![proptest_config(ProptestConfig {
        // Keep replaying the existing regression corpus after moving this source.
        failure_persistence: Some(Box::new(proptest::test_runner::FileFailurePersistence::Direct(
            concat!(env!("CARGO_MANIFEST_DIR"), "/tests/core_proptest.proptest-regressions"),
        ))),
        ..ProptestConfig::with_cases(256)
    })]

    #[test]
    fn dotenv_round_trip(vars in vars()) {
        let text = env::format(&vars, EnvFormat::Dotenv);
        let back = env::parse(&text).unwrap();
        prop_assert_eq!(back.len(), vars.len());
        for (a, b) in vars.iter().zip(back.iter()) {
            prop_assert_eq!(&a.key, &b.key);
            prop_assert_eq!(&a.value, &b.value);
            prop_assert_eq!(&a.comment, &b.comment);
        }
    }

    #[test]
    fn json_format_is_valid(vars in vars()) {
        let text = env::format(&vars, EnvFormat::Json);
        let v: serde_json::Value = serde_json::from_str(&text).unwrap();
        for var in &vars {
            prop_assert_eq!(v[&var.key].as_str(), var.value.as_deref());
        }
    }

    #[test]
    fn entry_doc_serialization_round_trip(d in doc()) {
        let json = d.to_json();
        let back = EntryDoc::from_json(&json).unwrap();
        prop_assert!(back == d);
    }

    /// Stored documents are always normalized by `body_from_payload`;
    /// normalization is idempotent, and updating with the redacted payload
    /// (secrets `None`) or the revealed payload changes nothing.
    #[test]
    fn redacted_update_is_identity(raw in doc()) {
        let Ok(body) = secrets::body_from_payload(&secrets::payload(&raw.body, true), None) else {
            return Ok(()); // invalid input (e.g. bad env key) is rejected, fine
        };
        let mut d = raw.clone();
        d.body = body;
        let again = secrets::body_from_payload(&secrets::payload(&d.body, true), None).unwrap();
        prop_assert!(again == d.body, "normalization not idempotent");

        let redacted = secrets::to_entry(&d, false);
        let merged = secrets::body_from_payload(&redacted.payload, Some(&d.body)).unwrap();
        let mut d2 = d.clone();
        d2.body = merged;
        prop_assert!(secrets::changed_fields(Some(&d), &d2).is_empty(),
            "changed: {:?}", secrets::changed_fields(Some(&d), &d2));
        // Redacted JSON never contains a secret path's value.
        for path in &redacted.redacted_fields {
            let v = secrets::reveal_field(&d.body, path).unwrap();
            if v.len() >= 12 {
                let json = serde_json::to_string(&redacted.payload).unwrap();
                let enc = serde_json::to_string(v.as_str()).unwrap();
                prop_assert!(!json.contains(&enc), "{} leaked", path);
            }
        }
    }
}
