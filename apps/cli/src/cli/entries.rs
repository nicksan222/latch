//! Shared entry addressing, display, and input conversion.

use std::io::IsTerminal;

use super::session::Session;
use super::{CliError, CliResult};
use latch_rpc::{sdk::RpcSdk, types::*};

pub const MASK: &str = "••••••";

pub fn stdout_tty() -> bool {
    std::io::stdout().is_terminal()
}

pub fn print_json<T: serde::Serialize>(v: &T) -> CliResult {
    outln!(
        "{}",
        serde_json::to_string_pretty(v).map_err(|e| CliError::user(e.to_string()))?
    );
    Ok(())
}

pub fn type_name(t: EntryType) -> String {
    serde_json::to_value(t)
        .ok()
        .and_then(|v| v.as_str().map(str::to_owned))
        .unwrap_or_default()
}

pub fn parse_type(s: &str) -> CliResult<EntryType> {
    Ok(match s.to_ascii_lowercase().replace('-', "_").as_str() {
        "login" => EntryType::Login,
        "secret" => EntryType::Secret,
        "env" => EntryType::Env,
        "file" => EntryType::File,
        "note" => EntryType::Note,
        "ssh_key" | "ssh" | "sshkey" => EntryType::SshKey,
        "database" | "db" => EntryType::Database,
        "api_key" | "apikey" | "api" => EntryType::ApiKey,
        _ => {
            return Err(CliError::user(format!(
                "unknown type `{s}` (login, secret, env, file, note, ssh_key, database, api_key)"
            )));
        }
    })
}

/// `<profile>/<entry>`; the entry part may itself contain `/`.
pub fn split_target(t: &str) -> CliResult<(&str, &str)> {
    match t.split_once('/') {
        Some((p, e)) if !p.is_empty() => Ok((p, e)),
        _ => Err(CliError::user(format!(
            "expected <profile>/<entry>, got `{t}`"
        ))),
    }
}

pub fn resolve_profile(s: &mut Session, name: &str) -> CliResult<ProfileSummary> {
    let list: Vec<ProfileSummary> = s.profiles_list()?;
    list.iter()
        .find(|p| p.id == name)
        .or_else(|| list.iter().find(|p| p.name.eq_ignore_ascii_case(name)))
        .cloned()
        .ok_or_else(|| CliError::user(format!("no profile `{name}`")))
}

pub fn list_entries(s: &mut Session, profile_id: &str) -> CliResult<Vec<EntrySummary>> {
    s.entries_list(ListEntriesParams {
        profile_id: Some(profile_id.to_owned()),
        query: None,
        entry_type: None,
    })
}

pub fn find_in(list: &[EntrySummary], name: &str) -> Option<EntrySummary> {
    list.iter()
        .find(|e| e.id == name)
        .or_else(|| list.iter().find(|e| e.name.eq_ignore_ascii_case(name)))
        .cloned()
}

pub fn resolve_entry(s: &mut Session, target: &str) -> CliResult<(ProfileSummary, EntrySummary)> {
    let (p, e) = split_target(target)?;
    let prof = resolve_profile(s, p)?;
    let list = list_entries(s, &prof.id)?;
    let ent = find_in(&list, e)
        .ok_or_else(|| CliError::user(format!("no entry `{e}` in profile `{}`", prof.name)))?;
    Ok((prof, ent))
}

pub fn get_entry(s: &mut Session, id: &str, reveal: bool) -> CliResult<Entry> {
    s.entries_get(GetEntryParams {
        id: id.to_owned(),
        reveal,
    })
}

/// Default field for `exec`/`copy` by entry type.
pub fn default_field(t: EntryType) -> CliResult<&'static str> {
    Ok(match t {
        EntryType::Login | EntryType::Database => {
            if t == EntryType::Database {
                "url"
            } else {
                "password"
            }
        }
        EntryType::Secret => "value",
        EntryType::ApiKey => "key",
        EntryType::Note => "body",
        EntryType::SshKey => "privateKey",
        EntryType::File => "content",
        EntryType::Env => {
            return Err(CliError::user(
                "env entries need --field vars.<KEY> (or use `latch env`)",
            ));
        }
    })
}

/// Fields that are never secret (printable without --reveal).
pub fn is_public_field(f: &str) -> bool {
    matches!(f, "username" | "filename" | "publicKey") || f.starts_with("urls.")
}

// ---------------------------------------------------------------------------
// Display
// ---------------------------------------------------------------------------

fn show(v: &Option<String>) -> String {
    v.clone().unwrap_or_else(|| MASK.to_owned())
}

pub fn print_entry(e: &Entry, profile_name: &str) {
    let ty = match &e.payload {
        EntryPayload::Login(_) => EntryType::Login,
        EntryPayload::Secret(_) => EntryType::Secret,
        EntryPayload::Env(_) => EntryType::Env,
        EntryPayload::File(_) => EntryType::File,
        EntryPayload::Note(_) => EntryType::Note,
        EntryPayload::SshKey(_) => EntryType::SshKey,
        EntryPayload::Database(_) => EntryType::Database,
        EntryPayload::ApiKey(_) => EntryType::ApiKey,
    };
    outln!("name:     {}", e.name);
    outln!("profile:  {profile_name}");
    outln!("type:     {}", type_name(ty));
    outln!("id:       {}", e.id);
    if !e.tags.is_empty() {
        outln!("tags:     {}", e.tags.join(", "));
    }
    match &e.payload {
        EntryPayload::Login(d) => {
            outln!("username: {}", d.username);
            outln!("password: {}", show(&d.password));
            for u in &d.urls {
                outln!("url:      {u}");
            }
            if let Some(t) = &d.totp {
                outln!("totp:     {}", show(&t.secret));
            }
            for c in &d.custom {
                outln!("{}: {}", c.key, show(&c.value));
            }
        }
        EntryPayload::Secret(d) => outln!("value:    {}", show(&d.value)),
        EntryPayload::Env(d) => {
            for v in &d.vars {
                outln!("{}={}", v.key, show(&v.value));
            }
        }
        EntryPayload::File(d) => {
            outln!("filename: {}", d.filename);
            outln!("mime:     {}", d.mime);
            outln!("size:     {}", d.size);
            if d.content_base64.is_some() {
                outln!("content:  {}", show(&d.content_base64));
            }
        }
        EntryPayload::Note(d) => outln!("body:\n{}", show(&d.body)),
        EntryPayload::SshKey(d) => {
            outln!("public:   {}", d.public_key);
            outln!("comment:  {}", d.comment);
            outln!("private:  {}", show(&d.private_key));
            if d.passphrase.is_some() || e.redacted_fields.iter().any(|f| f == "passphrase") {
                outln!("passphrase: {}", show(&d.passphrase));
            }
        }
        EntryPayload::Database(d) => {
            outln!("engine:   {}", d.engine);
            outln!("host:     {}", d.host);
            if let Some(p) = d.port {
                outln!("port:     {p}");
            }
            outln!("database: {}", d.database);
            outln!("username: {}", d.username);
            outln!("password: {}", show(&d.password));
            if !d.options.is_empty() {
                outln!("options:  {}", d.options);
            }
        }
        EntryPayload::ApiKey(d) => {
            outln!("key:      {}", show(&d.key));
            if d.secret.is_some() || e.redacted_fields.iter().any(|f| f == "secret") {
                outln!("secret:   {}", show(&d.secret));
            }
            if let Some(x) = &d.endpoint {
                outln!("endpoint: {x}");
            }
            if let Some(x) = &d.expires_at {
                outln!("expires:  {x}");
            }
        }
    }
    if !e.notes.is_empty() {
        outln!("notes:    {}", e.notes);
    }
}

pub fn input_from_entry(e: &Entry) -> EntryInput {
    EntryInput {
        profile_id: e.profile_id.clone(),
        name: e.name.clone(),
        tags: e.tags.clone(),
        notes: e.notes.clone(),
        favorite: e.favorite,
        payload: e.payload.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nested_entry_names_are_preserved_and_missing_profiles_are_rejected() {
        assert_eq!(
            split_target("work/service/token").unwrap(),
            ("work", "service/token")
        );
        assert!(split_target("/token").is_err());
        assert!(split_target("token").is_err());
    }

    #[test]
    fn environment_entries_require_an_explicit_field_for_copy_or_exec() {
        let error = default_field(EntryType::Env).unwrap_err();
        assert_eq!(error.code, 1);
        assert!(error.message.contains("--field vars.<KEY>"));
        assert_eq!(default_field(EntryType::Database).unwrap(), "url");
    }

    #[test]
    fn secret_fields_are_not_classified_as_public() {
        for field in ["password", "value", "privateKey", "totp", "vars.TOKEN"] {
            assert!(!is_public_field(field), "{field}");
        }
        for field in ["username", "filename", "publicKey", "urls.0"] {
            assert!(is_public_field(field), "{field}");
        }
    }
}
