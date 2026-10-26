//! ApiKey secret strategy.

use super::super::*;

secret_kind! {
    id: "api_key",
    ui: (3, "API key", Some("key")),
    defaults: serde_json::json!({"key": "", "secret": null, "endpoint": null, "expiresAt": null}),
    wire_secrets: &[("key", false), ("secret", true)],
    variant: ApiKey, data: ApiKeyData, doc: ApiKeyDoc,
    diff_fields: &["key", "secret", "endpoint", "expiresAt"],
    validate_ui: validate_ui,
    merge: from_input,
    payload: to_payload,
    paths: secret_paths,
    reveal: reveal_field,
    subtitle: subtitle,
    digests: field_digests,
}

fn from_input(d: &ApiKeyData, s: Option<&ApiKeyDoc>) -> Result<ApiKeyDoc> {
    Ok(ApiKeyDoc {
        key: keep(&d.key, s.map(|s| &s.key)),
        secret: keep_opt(&d.secret, s.map(|s| &s.secret)),
        endpoint: d.endpoint.clone().filter(|e| !e.trim().is_empty()),
        expires_at: d.expires_at.clone().filter(|e| !e.trim().is_empty()),
    })
}

fn to_payload(d: &ApiKeyDoc, reveal: bool) -> ApiKeyData {
    ApiKeyData {
        key: opt(reveal, &d.key),
        secret: if reveal { d.secret.clone() } else { None },
        endpoint: d.endpoint.clone(),
        expires_at: d.expires_at.clone(),
    }
}

fn secret_paths(d: &ApiKeyDoc) -> Vec<String> {
    let mut v = Vec::new();
    v.push("key".into());
    if d.secret.is_some() {
        v.push("secret".into());
    }
    v
}

fn reveal_field(d: &ApiKeyDoc, field: &str) -> Result<Zeroizing<String>> {
    let nf = || Error::NotFound(format!("field `{field}`"));
    let z = |s: &str| Ok(Zeroizing::new(s.to_owned()));
    match field {
        "key" => z(&d.key),
        "secret" => d.secret.as_deref().map_or_else(|| Err(nf()), z),
        _ => Err(nf()),
    }
}

fn subtitle(d: &ApiKeyDoc) -> String {
    d.endpoint
        .as_deref()
        .map(|e| {
            url::Url::parse(e)
                .ok()
                .and_then(|u| u.host_str().map(str::to_owned))
                .unwrap_or_else(|| e.to_owned())
        })
        .unwrap_or_default()
}

fn field_digests(d: &ApiKeyDoc, put: &mut dyn FnMut(&str, [u8; 32])) {
    put("key", digest(&d.key));
    put("secret", digest(&d.secret));
    put("endpoint", digest(&d.endpoint));
    put("expiresAt", digest(&d.expires_at));
}

fn validate_ui(d: &ApiKeyData, errors: &mut BTreeMap<String, String>) {
    #[allow(unused_imports)]
    use crate::core::domain::validation::{issue, keys, urlish};
    if d.key.as_deref() == Some("") {
        issue(errors, "key", "Key is required");
    }
    if d.endpoint
        .as_ref()
        .is_some_and(|s| !s.is_empty() && !urlish(s))
    {
        issue(errors, "endpoint", "Not a valid URL");
    }
    if d.expires_at
        .as_ref()
        .is_some_and(|s| !s.is_empty() && crate::core::format::parse_time(s).is_none())
    {
        issue(errors, "expiresAt", "Not a valid date");
    }
}
