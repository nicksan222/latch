//! Note secret strategy.

use super::super::*;

secret_kind! {
    id: "note",
    ui: (6, "Note", Some("body")),
    defaults: serde_json::json!({"body": ""}),
    wire_secrets: &[("body", false)],
    variant: Note, data: NoteData, doc: NoteDoc,
    diff_fields: &["body"],
    validate_ui: validate_ui,
    merge: from_input,
    payload: to_payload,
    paths: secret_paths,
    reveal: reveal_field,
    subtitle: subtitle,
    digests: field_digests,
}

fn from_input(d: &NoteData, s: Option<&NoteDoc>) -> Result<NoteDoc> {
    let s = s.map(|s| &s.body);
    Ok(NoteDoc {
        body: keep(&d.body, s),
    })
}

fn to_payload(d: &NoteDoc, reveal: bool) -> NoteData {
    NoteData {
        body: opt(reveal, &d.body),
    }
}

fn secret_paths(_d: &NoteDoc) -> Vec<String> {
    vec!["body".into()]
}

fn reveal_field(d: &NoteDoc, field: &str) -> Result<Zeroizing<String>> {
    let nf = || Error::NotFound(format!("field `{field}`"));
    let z = |s: &str| Ok(Zeroizing::new(s.to_owned()));
    match field {
        "body" => z(&d.body),
        _ => Err(nf()),
    }
}

fn subtitle(_d: &NoteDoc) -> String {
    String::new()
}

fn field_digests(d: &NoteDoc, put: &mut dyn FnMut(&str, [u8; 32])) {
    put("body", digest(&d.body));
}

fn validate_ui(d: &NoteData, errors: &mut BTreeMap<String, String>) {
    #[allow(unused_imports)]
    use crate::core::domain::validation::{issue, keys, urlish};
    let _ = (d, errors);
}
