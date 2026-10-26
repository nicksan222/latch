use super::super::{ArchiveDoc, EncodedExport};
use crate::protocol::{EntryPayload, EnvFormat};
use zeroize::Zeroizing;

export_kind! {
    Dotenv {
        id: "dotenv",
        encode(doc, _params) {
            let (text, entries) = dotenv_text(doc);
            Ok(EncodedExport { bytes: Zeroizing::new(text.as_bytes().to_vec()), entries })
        }
    }
}

/// Dotenv text for every env entry (`# <profile>/<entry>` headers).
pub fn dotenv_text(doc: &ArchiveDoc) -> (Zeroizing<String>, u32) {
    let mut out = Zeroizing::new(String::new());
    let mut n = 0;
    for p in &doc.profiles {
        for e in doc.entries.iter().filter(|e| e.profile_id == p.id) {
            let EntryPayload::Env(env) = &e.payload else {
                continue;
            };
            if n > 0 {
                out.push('\n');
            }
            out.push_str(&format!("# {}/{}\n", p.name, e.name));
            out.push_str(&crate::core::environment::format(
                &env.vars,
                EnvFormat::Dotenv,
            ));
            n += 1;
        }
    }
    (out, n)
}
