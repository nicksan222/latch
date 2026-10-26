//! KeePass 2.x XML export (KeePass "KeePass XML (2.x)", KeePassXC "XML").
//!
//! - First-level groups under the database root → profiles; deeper groups →
//!   tags; entries in the root group → `imported`. The recycle bin is skipped.
//! - Entries → `login` (Title, UserName, Password, URL, Notes, TOTP from
//!   `otp` / `TimeOtp-*` / `TOTP Seed`; other strings → custom fields, hidden
//!   when `ProtectInMemory`), or `note` when only Notes is set.
//! - Attachments → one `file` entry each (`<title> - <filename>`).
//! - Entry `History` is ignored.

use std::collections::HashMap;
use std::io::Read;
use std::path::Path;

use base64::Engine;
use quick_xml::events::Event;
use zeroize::Zeroizing;

use crate::core::import::{CustomFields, ImportedEntry, Parsed, guess_mime, parse_totp, read_text};
use crate::core::{Error, Result};
use crate::protocol::{EntryPayload, FileData, LoginData, NoteData, TotpAlgorithm, TotpConfig};

#[derive(Debug, Default)]
struct Node {
    name: String,
    attrs: Vec<(String, String)>,
    children: Vec<Node>,
    text: Zeroizing<String>,
}

impl Node {
    fn named(name: String) -> Self {
        Node {
            name,
            attrs: vec![],
            children: vec![],
            text: Zeroizing::new(String::new()),
        }
    }
}

impl Node {
    fn child(&self, name: &str) -> Option<&Node> {
        self.children.iter().find(|c| c.name == name)
    }
    fn children<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a Node> + 'a {
        self.children.iter().filter(move |c| c.name == name)
    }
    fn attr(&self, name: &str) -> Option<&str> {
        self.attrs
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.as_str())
    }
    fn text_of(&self, name: &str) -> String {
        self.child(name)
            .map(|c| c.text.to_string())
            .unwrap_or_default()
    }
}

/// Element nesting depth accepted (KeePass groups rarely exceed ~10).
const MAX_DEPTH: usize = 128;
/// Elements accepted in one file.
const MAX_NODES: usize = 1_000_000;
/// Decompressed size of one attachment (entries are limited to 1 MiB).
const MAX_ATTACHMENT_BYTES: u64 = 4 * 1024 * 1024;
/// Decompressed size of all attachments together.
const MAX_TOTAL_ATTACHMENT_BYTES: u64 = 128 * 1024 * 1024;

fn malformed(pos: u64) -> Error {
    Error::invalid(format!("malformed KeePass XML near byte {pos}"))
}

fn parse_dom(xml: &str) -> Result<Node> {
    let mut reader = quick_xml::Reader::from_str(xml);
    reader.config_mut().trim_text(false);
    let mut stack: Vec<Node> = vec![Node::named("#document".into())];
    let mut nodes = 0usize;
    let mut count = |pos: u64| -> Result<()> {
        nodes += 1;
        if nodes > MAX_NODES {
            return Err(Error::invalid(format!(
                "KeePass XML has too many elements (near byte {pos})"
            )));
        }
        Ok(())
    };
    let make = |e: &quick_xml::events::BytesStart<'_>, pos: u64| -> Result<Node> {
        let mut n = Node::named(String::from_utf8_lossy(e.local_name().as_ref()).into_owned());
        for a in e.attributes() {
            let a = a.map_err(|_| malformed(pos))?;
            let v = a.unescape_value().map_err(|_| malformed(pos))?;
            n.attrs.push((
                String::from_utf8_lossy(a.key.local_name().as_ref()).into_owned(),
                v.into_owned(),
            ));
        }
        Ok(n)
    };
    loop {
        let pos = reader.buffer_position();
        match reader.read_event().map_err(|_| malformed(pos))? {
            Event::Start(e) => {
                count(pos)?;
                if stack.len() > MAX_DEPTH {
                    return Err(Error::invalid(format!(
                        "KeePass XML is nested too deeply (near byte {pos})"
                    )));
                }
                let n = make(&e, pos)?;
                stack.push(n);
            }
            Event::Empty(e) => {
                count(pos)?;
                let n = make(&e, pos)?;
                stack.last_mut().expect("document node").children.push(n);
            }
            Event::End(_) => {
                if stack.len() < 2 {
                    return Err(malformed(pos));
                }
                let n = stack.pop().expect("checked");
                stack.last_mut().expect("checked").children.push(n);
            }
            Event::Text(t) => {
                let s = t.unescape().map_err(|_| malformed(pos))?;
                stack.last_mut().expect("document node").text.push_str(&s);
            }
            Event::CData(c) => {
                let s = String::from_utf8_lossy(&c.into_inner()).into_owned();
                stack.last_mut().expect("document node").text.push_str(&s);
            }
            Event::Eof => break,
            _ => {}
        }
    }
    if stack.len() != 1 {
        return Err(malformed(reader.buffer_position()));
    }
    let mut doc = stack.pop().expect("document node");
    let pos = doc
        .children
        .iter()
        .position(|c| c.name == "KeePassFile")
        .ok_or_else(|| Error::invalid("not a KeePass XML export (no KeePassFile element)"))?;
    Ok(doc.children.swap_remove(pos))
}

fn decode_binary(node: &Node, budget: &mut u64) -> Option<Zeroizing<Vec<u8>>> {
    let clean: String = node.text.chars().filter(|c| !c.is_whitespace()).collect();
    let raw = Zeroizing::new(
        base64::engine::general_purpose::STANDARD
            .decode(clean.as_bytes())
            .ok()?,
    );
    let limit = MAX_ATTACHMENT_BYTES.min(*budget);
    let out = if node
        .attr("Compressed")
        .is_some_and(|v| v.eq_ignore_ascii_case("true"))
    {
        // Bounded streaming decompression (gzip bombs).
        let mut out = Zeroizing::new(Vec::new());
        flate2::read::GzDecoder::new(&raw[..])
            .take(limit + 1)
            .read_to_end(&mut out)
            .ok()?;
        out
    } else {
        raw
    };
    if out.len() as u64 > limit {
        return None;
    }
    *budget -= out.len() as u64;
    Some(out)
}

struct Ctx<'a> {
    binaries: HashMap<String, &'a Node>,
    /// Remaining decompressed attachment bytes.
    budget: u64,
    recycle: String,
    out: Parsed,
}

fn parse(path: &Path, _passphrase: Option<&str>) -> Result<Parsed> {
    let text = read_text(path)?;
    let file = parse_dom(&text)?;
    let meta = file.child("Meta");
    let mut binaries = HashMap::new();
    if let Some(b) = meta.and_then(|m| m.child("Binaries")) {
        for bin in b.children("Binary") {
            if let Some(id) = bin.attr("ID") {
                binaries.insert(id.to_owned(), bin);
            }
        }
    }
    let recycle = meta
        .map(|m| m.text_of("RecycleBinUUID").trim().to_owned())
        .unwrap_or_default();
    let root_group = file
        .child("Root")
        .and_then(|r| r.child("Group"))
        .ok_or_else(|| Error::invalid("KeePass XML has no root group"))?;
    let mut ctx = Ctx {
        binaries,
        budget: MAX_TOTAL_ATTACHMENT_BYTES,
        recycle,
        out: Parsed::default(),
    };
    walk(&mut ctx, root_group, &[]);
    Ok(ctx.out)
}

/// Depth-first, pre-order (entries of a group, then its subgroups in
/// document order), iterative.
fn walk<'a>(ctx: &mut Ctx<'_>, root: &'a Node, path: &[String]) {
    let mut stack: Vec<(&'a Node, Vec<String>)> = vec![(root, path.to_vec())];
    while let Some((group, path)) = stack.pop() {
        let uuid = group.text_of("UUID").trim().to_owned();
        if !path.is_empty() && !ctx.recycle.is_empty() && uuid == ctx.recycle {
            continue;
        }
        for e in group.children("Entry") {
            entry(ctx, e, &path);
        }
        let subs: Vec<&Node> = group.children("Group").collect();
        for g in subs.into_iter().rev() {
            let mut p = path.clone();
            p.push(g.text_of("Name").trim().to_owned());
            stack.push((g, p));
        }
    }
}

const STANDARD: &[&str] = &["Title", "UserName", "Password", "URL", "Notes"];

fn entry(ctx: &mut Ctx<'_>, e: &Node, path: &[String]) {
    let mut strings: Vec<(String, String, bool)> = vec![];
    let mut title = String::new();
    for s in e.children("String") {
        let key = s.text_of("Key");
        let Some(v) = s.child("Value") else { continue };
        if v.attr("Protected")
            .is_some_and(|p| p.eq_ignore_ascii_case("true"))
        {
            ctx.out.warnings.push(format!(
                "field `{key}` is encrypted with the database stream key; export as KeePass XML (unencrypted) to include it"
            ));
            continue;
        }
        let hidden = v
            .attr("ProtectInMemory")
            .is_some_and(|p| p.eq_ignore_ascii_case("true"));
        if key == "Title" {
            title = v.text.trim().to_owned();
        }
        strings.push((key, v.text.to_string(), hidden));
    }
    let get = |k: &str| -> String {
        strings
            .iter()
            .find(|(key, _, _)| key == k)
            .map(|(_, v, _)| v.clone())
            .unwrap_or_default()
    };
    let username = get("UserName");
    let password = get("Password");
    let url = get("URL").trim().to_owned();
    let notes = get("Notes");

    // TOTP
    let mut consumed: Vec<&str> = vec![];
    let mut totp = None;
    let otp = get("otp");
    if !otp.trim().is_empty() {
        totp = parse_totp(&otp);
        consumed.push("otp");
    }
    if totp.is_none() {
        let seed = get("TimeOtp-Secret-Base32");
        if !seed.trim().is_empty() {
            let algorithm = match get("TimeOtp-Algorithm").to_ascii_uppercase().as_str() {
                "HMAC-SHA-256" => TotpAlgorithm::Sha256,
                "HMAC-SHA-512" => TotpAlgorithm::Sha512,
                _ => TotpAlgorithm::Sha1,
            };
            totp = crate::core::totp::normalize_secret(&seed)
                .ok()
                .map(|secret| TotpConfig {
                    secret: Some(secret),
                    algorithm,
                    digits: get("TimeOtp-Length").trim().parse().unwrap_or(6),
                    period: get("TimeOtp-Period").trim().parse().unwrap_or(30),
                    issuer: None,
                });
            consumed.extend([
                "TimeOtp-Secret-Base32",
                "TimeOtp-Algorithm",
                "TimeOtp-Length",
                "TimeOtp-Period",
            ]);
        }
    }
    if totp.is_none() {
        let seed = get("TOTP Seed");
        if !seed.trim().is_empty() {
            let settings = get("TOTP Settings");
            let mut it = settings.split(';');
            let period = it.next().and_then(|p| p.trim().parse().ok()).unwrap_or(30);
            let digits = it.next().and_then(|d| d.trim().parse().ok()).unwrap_or(6);
            totp = parse_totp(&seed).map(|mut t| {
                t.period = period;
                t.digits = digits;
                t
            });
            consumed.extend(["TOTP Seed", "TOTP Settings"]);
        }
    }
    if totp.is_none() && !consumed.is_empty() {
        // Keep an unparseable seed as a hidden field rather than losing it.
        consumed.clear();
    }

    let mut custom = CustomFields::default();
    for (k, v, hidden) in &strings {
        if STANDARD.contains(&k.as_str()) || consumed.contains(&k.as_str()) {
            continue;
        }
        custom.push(k, v, *hidden || k.to_ascii_lowercase().contains("otp"));
    }

    let (folder, mut tags) = match path.split_first() {
        Some((first, rest)) => (Some(first.clone()), rest.to_vec()),
        None => (None, vec![]),
    };
    for t in e.text_of("Tags").split([';', ',']) {
        let t = t.trim();
        if !t.is_empty() {
            tags.push(t.to_owned());
        }
    }
    let name = if title.is_empty() {
        crate::core::import::host_of(&url).unwrap_or_else(|| "Untitled".into())
    } else {
        title.clone()
    };

    let only_notes = username.is_empty()
        && password.is_empty()
        && url.is_empty()
        && totp.is_none()
        && custom.0.is_empty();
    let (payload, env_notes) = if only_notes && !notes.is_empty() {
        (
            EntryPayload::Note(NoteData { body: Some(notes) }),
            String::new(),
        )
    } else {
        (
            EntryPayload::Login(LoginData {
                username,
                password: Some(password),
                urls: if url.is_empty() { vec![] } else { vec![url] },
                totp,
                custom: custom.0,
            }),
            notes,
        )
    };
    // An entry that only carries attachments becomes just the file entries.
    let empty = only_notes && matches!(&payload, EntryPayload::Login(_));
    let has_binaries = e.children("Binary").next().is_some();
    if !(empty && has_binaries) {
        ctx.out.entries.push(ImportedEntry {
            folder: folder.clone(),
            name: name.clone(),
            tags: tags.clone(),
            notes: env_notes,
            favorite: false,
            payload,
            source_id: None,
        });
    }

    for b in e.children("Binary") {
        let filename = b.text_of("Key").trim().to_owned();
        let Some(v) = b.child("Value") else { continue };
        let data = match v.attr("Ref") {
            Some(r) => match ctx.binaries.get(r).copied() {
                Some(n) => decode_binary(n, &mut ctx.budget),
                None => None,
            },
            None => decode_binary(v, &mut ctx.budget),
        };
        let Some(data) = data else {
            ctx.out.warnings.push(format!(
                "`{name}`: attachment `{filename}` could not be read"
            ));
            continue;
        };
        let content = Zeroizing::new(base64::engine::general_purpose::STANDARD.encode(&data[..]));
        ctx.out.entries.push(ImportedEntry {
            folder: folder.clone(),
            name: format!("{name} - {filename}"),
            tags: tags.clone(),
            notes: String::new(),
            favorite: false,
            payload: EntryPayload::File(FileData {
                mime: guess_mime(&filename).into(),
                filename,
                content_base64: Some(content.to_string()),
                size: data.len() as u64,
            }),
            source_id: None,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dom_parses_attrs_and_text() {
        let d = parse_dom(
            r#"<?xml version="1.0"?><KeePassFile><Meta><X a="1&amp;2">t&lt;x</X><E/></Meta></KeePassFile>"#,
        )
        .unwrap();
        let x = d.child("Meta").unwrap().child("X").unwrap();
        assert_eq!(x.attr("a"), Some("1&2"));
        assert_eq!(x.text.as_str(), "t<x");
        assert!(d.child("Meta").unwrap().child("E").is_some());
    }

    #[test]
    fn limits_depth_and_bombs() {
        let deep = format!(
            "<KeePassFile>{}{}</KeePassFile>",
            "<a>".repeat(10_000),
            "</a>".repeat(10_000)
        );
        let e = parse_dom(&deep).unwrap_err().to_string();
        assert!(e.contains("too deeply"), "{e}");
        // 64 MiB of zeros gzip to a few KiB: refused, not expanded.
        let mut gz = flate2::write::GzEncoder::new(vec![], flate2::Compression::best());
        std::io::Write::write_all(&mut gz, &vec![0u8; 64 * 1024 * 1024]).unwrap();
        let b64 = base64::engine::general_purpose::STANDARD.encode(gz.finish().unwrap());
        let mut n = Node::named("Binary".into());
        n.attrs.push(("Compressed".into(), "True".into()));
        n.text = Zeroizing::new(b64);
        let mut budget = MAX_TOTAL_ATTACHMENT_BYTES;
        assert!(decode_binary(&n, &mut budget).is_none());
        assert_eq!(budget, MAX_TOTAL_ATTACHMENT_BYTES);
    }

    #[test]
    fn rejects_other_xml() {
        assert!(parse_dom("<html></html>").is_err());
        assert!(parse_dom("<KeePassFile><a></KeePassFile>").is_err());
    }
}

import_kind! { format: KeepassXml, id: "keepass_xml", parse: parse }
