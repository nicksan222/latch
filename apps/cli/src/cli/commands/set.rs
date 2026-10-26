use super::*;
use base64::Engine;
use std::io::Read;

fn norm(k: &str) -> String {
    k.to_ascii_lowercase().replace(['_', '-'], "")
}

fn parse_pairs(pairs: &[String]) -> CliResult<Vec<(String, Zeroizing<String>)>> {
    pairs
        .iter()
        .map(|p| match p.split_once('=') {
            Some((k, v)) if !k.is_empty() => Ok((k.to_owned(), Zeroizing::new(v.to_owned()))),
            _ => Err(CliError::user("arguments must be key=value")),
        })
        .collect()
}

const LOGIN_KEYS: &[&str] = &["username", "user", "password", "url", "urls", "totp"];

fn env_name_candidate(key: &str) -> bool {
    let mut chars = key.chars();
    matches!(chars.next(), Some(c) if c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-'))
}

fn infer_type(pairs: &[(String, Zeroizing<String>)]) -> CliResult<EntryType> {
    if pairs.is_empty() {
        return Ok(EntryType::Secret);
    }
    if pairs
        .iter()
        .all(|(k, _)| env_name_candidate(k) && !k.chars().any(|c| c.is_ascii_lowercase()))
    {
        return Ok(EntryType::Env);
    }
    if pairs
        .iter()
        .all(|(k, _)| LOGIN_KEYS.contains(&norm(k).as_str()))
    {
        return Ok(EntryType::Login);
    }
    if pairs.len() == 1 && norm(&pairs[0].0) == "value" {
        return Ok(EntryType::Secret);
    }
    Err(CliError::user("cannot infer the entry type; pass --type"))
}

fn totp_config(s: &mut Session, value: &str) -> CliResult<TotpConfig> {
    s.totp_parse(EnvParseParams { text: value.into() })
}

fn unknown(k: &str, t: EntryType) -> CliError {
    CliError::user(format!("unknown field `{k}` for {} entries", type_name(t)))
}

/// Apply key=value pairs and the stdin/file value to a payload.
fn apply(
    payload: &mut EntryPayload,
    pairs: Vec<(String, Zeroizing<String>)>,
    main: Option<(Zeroizing<Vec<u8>>, Option<String>)>,
    s: &mut Session,
) -> CliResult {
    let text = |b: &Zeroizing<Vec<u8>>| -> CliResult<String> {
        String::from_utf8(b.to_vec()).map_err(|_| CliError::user("input is not valid UTF-8"))
    };
    match payload
        .data
        .as_mut()
        .ok_or_else(|| CliError::user("entry payload is missing"))?
    {
        entry_payload::Data::Login(d) => {
            let mut urls_set = false;
            for (k, v) in pairs {
                match norm(&k).as_str() {
                    "username" | "user" => d.username = v.to_string(),
                    "password" => d.password = Some(v.to_string()),
                    "url" | "urls" => {
                        if !urls_set {
                            d.urls.clear();
                            urls_set = true;
                        }
                        d.urls.push(v.to_string());
                    }
                    "totp" => d.totp = Some(totp_config(s, &v)?),
                    _ => {
                        if let Some(c) = d.custom.iter_mut().find(|c| c.key == k) {
                            c.value = Some(v.to_string());
                        } else {
                            d.custom.push(CustomField {
                                key: k,
                                value: Some(v.to_string()),
                                hidden: true,
                            });
                        }
                    }
                }
            }
            if let Some((b, _)) = main {
                d.password = Some(text(&b)?);
            }
        }
        entry_payload::Data::Secret(d) => {
            for (k, v) in pairs {
                match norm(&k).as_str() {
                    "value" => d.value = Some(v.to_string()),
                    _ => return Err(unknown(&k, EntryType::Secret)),
                }
            }
            if let Some((b, _)) = main {
                d.value = Some(text(&b)?);
            }
        }
        entry_payload::Data::Env(d) => {
            let set_var = |key: String, value: String, d: &mut EnvData| {
                if let Some(x) = d.vars.iter_mut().find(|x| x.key == key) {
                    x.value = Some(value);
                } else {
                    d.vars.push(EnvVar {
                        key,
                        value: Some(value),
                        hidden: true,
                        comment: None,
                    });
                }
            };
            if let Some((b, _)) = main {
                let parsed = s.env_parse(EnvParseParams { text: text(&b)? })?.items;
                for v in parsed {
                    set_var(v.key, v.value.unwrap_or_default(), d);
                }
            }
            for (k, v) in pairs {
                set_var(k, v.to_string(), d);
            }
        }
        entry_payload::Data::Note(d) => {
            for (k, v) in pairs {
                match norm(&k).as_str() {
                    "body" | "value" => d.body = Some(v.to_string()),
                    _ => return Err(unknown(&k, EntryType::Note)),
                }
            }
            if let Some((b, _)) = main {
                d.body = Some(text(&b)?);
            }
        }
        entry_payload::Data::File(d) => {
            for (k, v) in pairs {
                match norm(&k).as_str() {
                    "filename" | "name" => d.filename = v.to_string(),
                    "mime" => d.mime = v.to_string(),
                    _ => return Err(unknown(&k, EntryType::File)),
                }
            }
            if let Some((b, fname)) = main {
                d.size = b.len() as u64;
                d.content_base64 = Some(base64::engine::general_purpose::STANDARD.encode(&b[..]));
                if d.filename.is_empty() {
                    d.filename = fname.unwrap_or_else(|| "file".into());
                }
            }
            if d.mime.is_empty() {
                d.mime = "application/octet-stream".into();
            }
        }
        entry_payload::Data::SshKey(d) => {
            for (k, v) in pairs {
                match norm(&k).as_str() {
                    "privatekey" | "private" => d.private_key = Some(v.to_string()),
                    "publickey" | "public" => d.public_key = v.to_string(),
                    "passphrase" => d.passphrase = Some(v.to_string()),
                    "comment" => d.comment = v.to_string(),
                    _ => return Err(unknown(&k, EntryType::SshKey)),
                }
            }
            if let Some((b, _)) = main {
                d.private_key = Some(text(&b)?);
            }
        }
        entry_payload::Data::Database(d) => {
            for (k, v) in pairs {
                match norm(&k).as_str() {
                    "engine" => d.engine = v.to_string(),
                    "host" => d.host = v.to_string(),
                    "port" => d.port = Some(v.parse().map_err(|_| CliError::user("invalid port"))?),
                    "database" | "db" => d.database = v.to_string(),
                    "username" | "user" => d.username = v.to_string(),
                    "password" => d.password = Some(v.to_string()),
                    "options" => d.options = v.to_string(),
                    _ => return Err(unknown(&k, EntryType::Database)),
                }
            }
            if let Some((b, _)) = main {
                d.password = Some(text(&b)?);
            }
        }
        entry_payload::Data::ApiKey(d) => {
            for (k, v) in pairs {
                match norm(&k).as_str() {
                    "key" => d.key = Some(v.to_string()),
                    "secret" => d.secret = Some(v.to_string()),
                    "endpoint" => d.endpoint = Some(v.to_string()),
                    "expiresat" | "expires" => d.expires_at = Some(v.to_string()),
                    _ => return Err(unknown(&k, EntryType::ApiKey)),
                }
            }
            if let Some((b, _)) = main {
                d.key = Some(text(&b)?);
            }
        }
    }
    Ok(())
}

fn empty_payload(t: EntryType) -> EntryPayload {
    match t {
        EntryType::Login => EntryPayload {
            data: Some(entry_payload::Data::Login(LoginData {
                username: String::new(),
                password: None,
                urls: vec![],
                totp: None,
                custom: vec![],
            })),
        },
        EntryType::Secret => EntryPayload {
            data: Some(entry_payload::Data::Secret(SecretData { value: None })),
        },
        EntryType::Env => EntryPayload {
            data: Some(entry_payload::Data::Env(EnvData { vars: vec![] })),
        },
        EntryType::File => EntryPayload {
            data: Some(entry_payload::Data::File(FileData {
                filename: String::new(),
                mime: String::new(),
                content_base64: None,
                size: 0,
            })),
        },
        EntryType::Note => EntryPayload {
            data: Some(entry_payload::Data::Note(NoteData { body: None })),
        },
        EntryType::SshKey => EntryPayload {
            data: Some(entry_payload::Data::SshKey(SshKeyData {
                private_key: None,
                public_key: String::new(),
                passphrase: None,
                comment: String::new(),
            })),
        },
        EntryType::Database => EntryPayload {
            data: Some(entry_payload::Data::Database(DatabaseData {
                engine: "postgres".into(),
                host: String::new(),
                port: None,
                database: String::new(),
                username: String::new(),
                password: None,
                options: String::new(),
            })),
        },
        EntryType::ApiKey => EntryPayload {
            data: Some(entry_payload::Data::ApiKey(ApiKeyData {
                key: None,
                secret: None,
                endpoint: None,
                expires_at: None,
            })),
        },
        EntryType::Unspecified => EntryPayload::default(),
    }
}

pub(super) fn set(s: &mut Session, a: SetArgs) -> CliResult {
    let (pname, ename) = split_target(&a.target)?;
    if ename.trim().is_empty() {
        return Err(CliError::user("entry name is empty"));
    }
    let prof = resolve_profile(s, pname)?;
    let list = list_entries(s, &prof.id)?;
    let existing = find_in(&list, ename);
    let pairs = parse_pairs(&a.pairs)?;
    let main: Option<(Zeroizing<Vec<u8>>, Option<String>)> = if a.stdin {
        let mut b = Zeroizing::new(Vec::new());
        std::io::stdin()
            .read_to_end(&mut b)
            .map_err(|e| CliError::user(format!("read stdin: {e}")))?;
        // A single trailing newline (from `echo`) is not part of the value.
        if b.ends_with(b"\n") {
            b.pop();
            if b.ends_with(b"\r") {
                b.pop();
            }
        }
        Some((b, None))
    } else if let Some(f) = &a.from_file {
        let b = Zeroizing::new(
            std::fs::read(f).map_err(|e| CliError::user(format!("read {}: {e}", f.display())))?,
        );
        let name = f.file_name().map(|n| n.to_string_lossy().into_owned());
        Some((b, name))
    } else {
        None
    };
    let wanted = a.entry_type.as_deref().map(parse_type).transpose()?;
    let (mut input, id) = match &existing {
        Some(sum) => {
            if let Some(t) = wanted
                && t != EntryType::try_from(sum.r#type).unwrap_or(EntryType::Unspecified)
            {
                return Err(CliError::user(format!(
                    "entry is of type {}, not {}",
                    type_name(EntryType::try_from(sum.r#type).unwrap_or(EntryType::Unspecified)),
                    type_name(t)
                )));
            }
            let e = get_entry(s, &sum.id, false)?;
            (input_from_entry(&e), Some(sum.id.clone()))
        }
        None => {
            let t = match wanted {
                Some(t) => t,
                None if main.is_some() && pairs.is_empty() => EntryType::Secret,
                None => infer_type(&pairs)?,
            };
            (
                EntryInput {
                    profile_id: prof.id.clone(),
                    name: ename.trim().to_owned(),
                    tags: vec![],
                    notes: String::new(),
                    favorite: false,
                    payload: Some(empty_payload(t)),
                },
                None,
            )
        }
    };
    if !a.tags.is_empty() {
        input.tags = a.tags.clone();
    }
    if let Some(n) = &a.notes {
        input.notes = n.clone();
    }
    apply(
        input
            .payload
            .as_mut()
            .ok_or_else(|| CliError::user("entry payload is missing"))?,
        pairs,
        main,
        s,
    )?;
    let e: Entry = match id {
        Some(id) => s.entries_update(EntryUpdateParams {
            id,
            input: Some(input),
        })?,
        None => s.entries_create(input)?,
    };
    eprintln!(
        "{} {}/{}",
        if existing.is_some() {
            "updated"
        } else {
            "created"
        },
        prof.name,
        e.name
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn empty_entry_name_is_rejected_before_resolving_a_profile() {
        let mut fixture = crate::cli::commands::test_support::EmptySession::new();
        let error = set(
            &mut fixture.session,
            SetArgs {
                target: "work/ ".into(),
                pairs: vec![],
                entry_type: None,
                from_file: None,
                stdin: false,
                tags: vec![],
                notes: None,
            },
        )
        .unwrap_err();
        assert_eq!(error.code, 1);
        assert_eq!(error.message, "entry name is empty");
    }

    #[test]
    fn pair_parser_preserves_equals_in_values_and_rejects_missing_keys() {
        let pairs = parse_pairs(&["value=a=b".into()]).unwrap();
        assert_eq!(pairs[0].1.as_str(), "a=b");
        let error = parse_pairs(&["=private-input".into()]).unwrap_err();
        assert!(!error.message.contains("private-input"));
        assert!(parse_pairs(&["missing-separator".into()]).is_err());
    }

    #[test]
    fn inference_distinguishes_environment_login_and_ambiguous_fields() {
        assert_eq!(
            infer_type(&parse_pairs(&["TOKEN=secret".into()]).unwrap()).unwrap(),
            EntryType::Env
        );
        assert_eq!(
            infer_type(&parse_pairs(&["username=alice".into(), "password=secret".into()]).unwrap())
                .unwrap(),
            EntryType::Login
        );
        assert!(infer_type(&parse_pairs(&["host=example".into()]).unwrap()).is_err());
    }

    #[test]
    fn updating_login_urls_keeps_custom_fields_hidden_and_unrelated_fields_intact() {
        let mut fixture = crate::cli::commands::test_support::EmptySession::new();
        let mut payload = EntryPayload {
            data: Some(entry_payload::Data::Login(LoginData {
                username: "alice".into(),
                password: None,
                urls: vec!["https://old.example".into()],
                totp: None,
                custom: vec![],
            })),
        };
        apply(
            &mut payload,
            parse_pairs(&[
                "url=https://one.example".into(),
                "url=https://two.example".into(),
                "custom=private-input".into(),
            ])
            .unwrap(),
            None,
            &mut fixture.session,
        )
        .unwrap();
        let Some(entry_payload::Data::Login(login)) = payload.data else {
            panic!("wrong payload type")
        };
        assert_eq!(login.username, "alice");
        assert!(login.password.is_none());
        assert_eq!(login.urls, ["https://one.example", "https://two.example"]);
        assert!(login.custom[0].hidden);
    }

    #[test]
    fn unknown_fields_and_invalid_utf8_never_echo_secret_values() {
        let mut fixture = crate::cli::commands::test_support::EmptySession::new();
        let mut payload = empty_payload(EntryType::Secret);
        let error = apply(
            &mut payload,
            parse_pairs(&["unsupported=private-input".into()]).unwrap(),
            None,
            &mut fixture.session,
        )
        .unwrap_err();
        assert!(!error.message.contains("private-input"));
        let error = apply(
            &mut payload,
            vec![],
            Some((Zeroizing::new(vec![0xff]), None)),
            &mut fixture.session,
        )
        .unwrap_err();
        assert_eq!(error.message, "input is not valid UTF-8");
    }

    #[test]
    fn file_input_is_base64_encoded_without_utf8_conversion() {
        let mut fixture = crate::cli::commands::test_support::EmptySession::new();
        let mut payload = empty_payload(EntryType::File);
        apply(
            &mut payload,
            vec![],
            Some((Zeroizing::new(vec![0xff, 0]), Some("key.bin".into()))),
            &mut fixture.session,
        )
        .unwrap();
        let Some(entry_payload::Data::File(file)) = payload.data else {
            panic!("wrong payload type")
        };
        assert_eq!(file.size, 2);
        assert_eq!(file.filename, "key.bin");
        assert_eq!(file.content_base64.as_deref(), Some("/wA="));
    }

    #[test]
    fn totp_input_uses_core_normalization_and_validation() {
        let mut fixture = crate::cli::commands::test_support::EmptySession::new();
        assert_eq!(
            totp_config(&mut fixture.session, "jbsw y3dp ehpk 3pxp")
                .unwrap()
                .secret
                .as_deref(),
            Some("JBSWY3DPEHPK3PXP")
        );
        assert!(totp_config(&mut fixture.session, "not a base32 secret!").is_err());
    }
}
