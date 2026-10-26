//! File secret strategy.

use super::super::*;
use base64::Engine;

secret_kind! {
    id: "file",
    ui: (7, "File", None),
    defaults: serde_json::json!({"filename": "", "mime": "application/octet-stream", "contentBase64": null, "size": 0}),
    wire_secrets: &[("content", false)],
    variant: File, data: FileData, doc: FileDoc,
    diff_fields: &["file"],
    validate_ui: validate_ui,
    merge: from_input,
    payload: to_payload,
    paths: secret_paths,
    reveal: reveal_field,
    subtitle: subtitle,
    digests: field_digests,
}

fn from_input(d: &FileData, s: Option<&FileDoc>) -> Result<FileDoc> {
    let s = s.map(|s| &s.content_base64);
    let content = keep(&d.content_base64, s);
    let bytes = Zeroizing::new(
        base64::engine::general_purpose::STANDARD
            .decode(content.trim())
            .map_err(|_| Error::invalid("file content is not valid base64"))?,
    );
    if d.filename.trim().is_empty() {
        return Err(Error::invalid("file name is required"));
    }
    Ok(FileDoc {
        filename: d.filename.trim().to_owned(),
        mime: if d.mime.trim().is_empty() {
            "application/octet-stream".into()
        } else {
            d.mime.trim().to_owned()
        },
        content_base64: content.trim().to_owned(),
        size: bytes.len() as u64,
    })
}

fn to_payload(d: &FileDoc, reveal: bool) -> FileData {
    FileData {
        filename: d.filename.clone(),
        mime: d.mime.clone(),
        content_base64: opt(reveal, &d.content_base64),
        size: d.size,
    }
}

fn secret_paths(_d: &FileDoc) -> Vec<String> {
    vec!["content".into()]
}

fn reveal_field(d: &FileDoc, field: &str) -> Result<Zeroizing<String>> {
    let nf = || Error::NotFound(format!("field `{field}`"));
    let z = |s: &str| Ok(Zeroizing::new(s.to_owned()));
    match field {
        "content" | "contentBase64" => z(&d.content_base64),
        "filename" => z(&d.filename),
        _ => Err(nf()),
    }
}

fn subtitle(d: &FileDoc) -> String {
    format!("{} · {}", d.filename, human_size(d.size))
}

fn field_digests(d: &FileDoc, put: &mut dyn FnMut(&str, [u8; 32])) {
    put("filename", digest(&d.filename));
    put("mime", digest(&d.mime));
    put("content", digest(&d.content_base64));
}

fn validate_ui(d: &FileData, errors: &mut BTreeMap<String, String>) {
    #[allow(unused_imports)]
    use crate::core::domain::validation::{issue, keys, urlish};
    if d.filename.trim().is_empty() {
        issue(errors, "filename", "Filename is required");
    }
    if d.size > 1024 * 1024 {
        issue(errors, "content", "Files are limited to 1 MiB");
    }
}
