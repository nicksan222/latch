use super::super::EncodedExport;
use crate::core::Error;
use zeroize::Zeroizing;

export_kind! {
    Json {
        id: "json",
        encode(doc, _params) {
            let mut bytes = Zeroizing::new(serde_json::to_vec_pretty(doc)
                .map_err(|_| Error::Internal("serialize export".into()))?);
            bytes.push(b'\n');
            Ok(EncodedExport { bytes, entries: doc.entries.len() as u32 })
        }
    }
}
