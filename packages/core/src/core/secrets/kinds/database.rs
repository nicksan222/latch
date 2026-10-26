//! Database secret strategy.

use super::super::*;

secret_kind! {
    id: "database",
    ui: (4, "Database", Some("password")),
    defaults: serde_json::json!({"engine": "postgres", "host": "", "port": 5432, "database": "", "username": "", "password": "", "options": ""}),
    wire_secrets: &[("password", true)],
    variant: Database, data: DatabaseData, doc: DatabaseDoc,
    diff_fields: &["host", "port", "database", "username", "password", "options"],
    validate_ui: validate_ui,
    merge: from_input,
    payload: to_payload,
    paths: secret_paths,
    reveal: reveal_field,
    subtitle: subtitle,
    digests: field_digests,
}

fn from_input(d: &DatabaseData, s: Option<&DatabaseDoc>) -> Result<DatabaseDoc> {
    let s = s.map(|s| &s.password);
    Ok(DatabaseDoc {
        engine: d.engine.trim().to_ascii_lowercase(),
        host: d.host.trim().to_owned(),
        port: d.port,
        database: d.database.clone(),
        username: d.username.clone(),
        password: keep_opt(&d.password, s),
        options: d.options.trim().trim_start_matches('?').to_owned(),
    })
}

fn to_payload(d: &DatabaseDoc, reveal: bool) -> DatabaseData {
    DatabaseData {
        engine: d.engine.clone(),
        host: d.host.clone(),
        port: d.port,
        database: d.database.clone(),
        username: d.username.clone(),
        password: if reveal { d.password.clone() } else { None },
        options: d.options.clone(),
    }
}

fn secret_paths(d: &DatabaseDoc) -> Vec<String> {
    let mut v = Vec::new();
    if d.password.is_some() {
        v.push("password".into());
    }
    v
}

fn reveal_field(d: &DatabaseDoc, field: &str) -> Result<Zeroizing<String>> {
    let nf = || Error::NotFound(format!("field `{field}`"));
    let z = |s: &str| Ok(Zeroizing::new(s.to_owned()));
    match field {
        "password" => d.password.as_deref().map_or_else(|| Err(nf()), z),
        "username" => z(&d.username),
        "url" => Ok(Zeroizing::new(database_url(d))),
        _ => Err(nf()),
    }
}

fn subtitle(d: &DatabaseDoc) -> String {
    let mut s = format!("{} · {}", d.engine, d.host);
    if !d.database.is_empty() {
        s.push('/');
        s.push_str(&d.database);
    }
    s
}

fn field_digests(d: &DatabaseDoc, put: &mut dyn FnMut(&str, [u8; 32])) {
    put("engine", digest(&d.engine));
    put("host", digest(&d.host));
    put("port", digest(&d.port));
    put("database", digest(&d.database));
    put("username", digest(&d.username));
    put("password", digest(&d.password));
    put("options", digest(&d.options));
}

fn pct(s: &str) -> String {
    url::form_urlencoded::byte_serialize(s.as_bytes())
        .collect::<String>()
        .replace('+', "%20")
}

/// Derived connection string for a database entry.
pub fn database_url(d: &DatabaseDoc) -> String {
    let scheme = match d.engine.as_str() {
        "postgres" | "postgresql" => "postgres",
        "" | "other" => "db",
        e => e,
    };
    if scheme == "sqlite" {
        return format!("sqlite://{}", d.database);
    }
    let mut s = format!("{scheme}://");
    if !d.username.is_empty() {
        s.push_str(&pct(&d.username));
        if let Some(p) = d.password.as_deref().filter(|p| !p.is_empty()) {
            s.push(':');
            s.push_str(&pct(p));
        }
        s.push('@');
    }
    if d.host.contains(':') && !d.host.starts_with('[') {
        s.push_str(&format!("[{}]", d.host));
    } else {
        s.push_str(&d.host);
    }
    if let Some(p) = d.port {
        s.push_str(&format!(":{p}"));
    }
    if !d.database.is_empty() {
        s.push('/');
        s.push_str(&pct(&d.database));
    }
    if !d.options.is_empty() {
        s.push('?');
        s.push_str(&d.options);
    }
    s
}

fn validate_ui(d: &DatabaseData, errors: &mut BTreeMap<String, String>) {
    #[allow(unused_imports)]
    use crate::core::domain::validation::{issue, keys, urlish};
    if d.engine.is_empty() {
        issue(errors, "engine", "Engine is required");
    }
    if d.engine != "sqlite" && d.host.trim().is_empty() {
        issue(errors, "host", "Host is required");
    }
    if d.port == Some(0) {
        issue(errors, "port", "Port must be 1–65535");
    }
}
