//! Login secret strategy.

use super::super::*;

secret_kind! {
    id: "login",
    ui: (0, "Login", Some("password")),
    defaults: serde_json::json!({"username": "", "password": "", "urls": [], "totp": null, "custom": []}),
    wire_secrets: &[("password", true), ("totp.secret", true), ("custom.*", false)],
    variant: Login, data: LoginData, doc: LoginDoc,
    diff_fields: &["username", "password", "urls", "totp.secret", "custom.*"],
    validate_ui: validate_ui,
    merge: from_input,
    payload: to_payload,
    paths: secret_paths,
    reveal: reveal_field,
    subtitle: subtitle,
    digests: field_digests,
}

fn from_input(d: &LoginData, s: Option<&LoginDoc>) -> Result<LoginDoc> {
    unique_keys(d.custom.iter().map(|c| c.key.as_str()), "custom field")?;
    let totp = match &d.totp {
        // `Some("")` clears the optional TOTP secret: no TOTP at all.
        Some(t) if t.secret.as_deref() == Some("") => None,
        Some(t) => Some(totp_from_input(t, s.and_then(|s| s.totp.as_ref()))?),
        None => None,
    };
    Ok(LoginDoc {
        username: d.username.clone(),
        password: keep_opt(&d.password, s.map(|s| &s.password)),
        urls: d
            .urls
            .iter()
            .map(|u| u.trim().to_owned())
            .filter(|u| !u.is_empty())
            .collect(),
        totp,
        custom: d
            .custom
            .iter()
            .map(|c| CustomFieldDoc {
                key: c.key.clone(),
                value: keep(
                    &c.value,
                    s.and_then(|s| s.custom.iter().find(|x| x.key == c.key))
                        .map(|x| &x.value),
                ),
                hidden: c.hidden,
            })
            .collect(),
    })
}

fn to_payload(d: &LoginDoc, reveal: bool) -> LoginData {
    LoginData {
        username: d.username.clone(),
        password: if reveal { d.password.clone() } else { None },
        urls: d.urls.clone(),
        totp: d.totp.as_ref().map(|t| TotpConfig {
            secret: opt(reveal, &t.secret),
            algorithm: t.algorithm,
            digits: t.digits,
            period: t.period,
            issuer: t.issuer.clone(),
        }),
        custom: d
            .custom
            .iter()
            .map(|c| CustomField {
                key: c.key.clone(),
                value: opt(reveal || !c.hidden, &c.value),
                hidden: c.hidden,
            })
            .collect(),
    }
}

fn secret_paths(d: &LoginDoc) -> Vec<String> {
    let mut v = Vec::new();
    if d.password.is_some() {
        v.push("password".into());
    }
    if d.totp.is_some() {
        v.push("totp.secret".into());
    }
    v.extend(
        d.custom
            .iter()
            .filter(|c| c.hidden)
            .map(|c| format!("custom.{}", c.key)),
    );
    v
}

fn reveal_field(d: &LoginDoc, field: &str) -> Result<Zeroizing<String>> {
    let nf = || Error::NotFound(format!("field `{field}`"));
    let z = |s: &str| Ok(Zeroizing::new(s.to_owned()));
    match field {
        "password" => d.password.as_deref().map_or_else(|| Err(nf()), z),
        "username" => z(&d.username),
        "totp.secret" => d.totp.as_ref().map_or_else(|| Err(nf()), |t| z(&t.secret)),
        f => match f.strip_prefix("custom.") {
            Some(k) => d
                .custom
                .iter()
                .find(|c| c.key == k)
                .map_or_else(|| Err(nf()), |c| z(&c.value)),
            None => match f
                .strip_prefix("urls.")
                .and_then(|i| i.parse::<usize>().ok())
            {
                Some(i) => d.urls.get(i).map_or_else(|| Err(nf()), |u| z(u)),
                None => Err(nf()),
            },
        },
    }
}

fn subtitle(d: &LoginDoc) -> String {
    d.username.clone()
}

fn field_digests(d: &LoginDoc, put: &mut dyn FnMut(&str, [u8; 32])) {
    put("username", digest(&d.username));
    put("password", digest(&d.password));
    put("urls", digest(&d.urls));
    if let Some(t) = &d.totp {
        put("totp.secret", digest(&t.secret));
        put(
            "totp",
            digest(&(t.algorithm, t.digits, t.period, &t.issuer)),
        );
    }
    for c in &d.custom {
        put(&format!("custom.{}", c.key), digest(&(&c.value, c.hidden)));
    }
}

fn totp_from_input(t: &TotpConfig, stored: Option<&TotpDoc>) -> Result<TotpDoc> {
    let doc = match t.secret.as_deref() {
        Some(s) if s.trim_start().starts_with("otpauth://") => {
            let mut d = totp::parse_otpauth(s)?;
            if d.issuer.is_none() {
                d.issuer = t.issuer.clone();
            }
            d
        }
        Some(s) => TotpDoc {
            secret: totp::normalize_secret(s)?,
            algorithm: t.algorithm,
            digits: t.digits,
            period: t.period,
            issuer: t.issuer.clone(),
        },
        None => {
            let s = stored.ok_or_else(|| Error::invalid("TOTP secret is required"))?;
            TotpDoc {
                secret: s.secret.clone(),
                algorithm: t.algorithm,
                digits: t.digits,
                period: t.period,
                issuer: t.issuer.clone(),
            }
        }
    };
    totp::validate(&doc)?;
    Ok(doc)
}

fn validate_ui(d: &LoginData, errors: &mut BTreeMap<String, String>) {
    #[allow(unused_imports)]
    use crate::core::domain::validation::{issue, keys, urlish};
    for (i, url) in d.urls.iter().enumerate() {
        if !url.trim().is_empty() && !urlish(url) {
            issue(errors, format!("urls.{i}"), "Not a valid URL");
        }
    }
    if let Some(t) = &d.totp {
        if t.secret
            .as_ref()
            .is_some_and(|s| totp::normalize_secret(s).map(Zeroizing::new).is_err())
        {
            issue(errors, "totp.secret", "TOTP secret must be base32");
        }
        if ![6, 8].contains(&t.digits) {
            issue(errors, "totp.digits", "Digits must be 6 or 8");
        }
        if t.period == 0 || t.period > 86_400 {
            issue(errors, "totp.period", "Period must be 1–86400 s");
        }
    }
    keys(
        d.custom.iter().map(|f| f.key.as_str()),
        "custom",
        false,
        errors,
    );
}
