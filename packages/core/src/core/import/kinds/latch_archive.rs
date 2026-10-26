//! Encrypted Latch archive import.

use std::path::Path;

use crate::core::import::{Parsed, from_archive, read_bytes};
use crate::core::{Error, Result};

fn parse(path: &Path, passphrase: Option<&str>) -> Result<Parsed> {
    let pw = passphrase
        .filter(|p| !p.is_empty())
        .ok_or_else(|| Error::invalid("a passphrase is required for Latch archives"))?;
    let bytes = read_bytes(path)?;
    let doc = crate::core::export::decrypt_archive(&bytes, pw)?;
    Ok(from_archive(doc))
}

import_kind! { format: LatchArchive, id: "latch_archive", parse: parse }
