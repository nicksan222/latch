//! Secret secret strategy.

use super::super::*;

secret_kind! {
    id: "secret",
    ui: (1, "Secret", Some("value")),
    defaults: serde_json::json!({"value": ""}),
    wire_secrets: &[("value", false)],
    variant: Secret, data: SecretData, doc: SecretDoc,
    diff_fields: &["value"],
    validate_ui: validate_ui,
    merge: from_input,
    payload: to_payload,
    paths: secret_paths,
    reveal: reveal_field,
    subtitle: subtitle,
    digests: field_digests,
}

fn from_input(d: &SecretData, s: Option<&SecretDoc>) -> Result<SecretDoc> {
    let s = s.map(|s| &s.value);
    Ok(SecretDoc {
        value: keep(&d.value, s),
        hidden: true,
    })
}

fn to_payload(d: &SecretDoc, reveal: bool) -> SecretData {
    SecretData {
        value: opt(reveal, &d.value),
    }
}

fn secret_paths(_d: &SecretDoc) -> Vec<String> {
    vec!["value".into()]
}

fn reveal_field(d: &SecretDoc, field: &str) -> Result<Zeroizing<String>> {
    let nf = || Error::NotFound(format!("field `{field}`"));
    let z = |s: &str| Ok(Zeroizing::new(s.to_owned()));
    match field {
        "value" => z(&d.value),
        _ => Err(nf()),
    }
}

fn subtitle(_d: &SecretDoc) -> String {
    String::new()
}

fn field_digests(d: &SecretDoc, put: &mut dyn FnMut(&str, [u8; 32])) {
    put("value", digest(&d.value));
}

fn validate_ui(d: &SecretData, errors: &mut BTreeMap<String, String>) {
    #[allow(unused_imports)]
    use crate::core::domain::validation::{issue, keys, urlish};
    if d.value.as_deref() == Some("") {
        issue(errors, "value", "Value is required");
    }
}
