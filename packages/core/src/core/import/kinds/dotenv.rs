//! `.env` file → one `env` entry named after the file.

use std::path::Path;

use crate::core::Result;
use crate::core::import::{ImportedEntry, Parsed, read_text};
use crate::protocol::{EntryPayload, EnvData};

fn parse(path: &Path, _passphrase: Option<&str>) -> Result<Parsed> {
    let text = read_text(path)?;
    let vars = crate::core::environment::parse(&text)?;
    let mut out = Parsed::default();
    if vars.is_empty() {
        out.warnings.push("no variables found".into());
        return Ok(out);
    }
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| ".env".into());
    out.entries.push(ImportedEntry::new(
        name,
        EntryPayload::Env(EnvData { vars }),
    ));
    Ok(out)
}

import_kind! { format: Dotenv, id: "dotenv", parse: parse }
