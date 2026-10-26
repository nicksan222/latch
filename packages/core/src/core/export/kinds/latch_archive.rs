use super::super::{EncodedExport, encrypt_archive};
use crate::core::Error;
use zeroize::Zeroizing;

export_kind! {
    LatchArchive {
        id: "latch_archive",
        encode(doc, params) {
            let passphrase = params.passphrase.as_deref().filter(|p| !p.is_empty())
                .ok_or_else(|| Error::invalid("a passphrase is required for Latch archives"))?;
            Ok(EncodedExport {
                bytes: Zeroizing::new(encrypt_archive(doc, passphrase)?),
                entries: doc.entries.len() as u32,
            })
        }
    }
}
