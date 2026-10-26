use super::*;
use std::str::FromStr;

pub(crate) fn issue(errors: &mut BTreeMap<String, String>, path: impl Into<String>, message: &str) {
    errors.insert(path.into(), message.into());
}
pub(crate) fn urlish(value: &str) -> bool {
    if value.trim().is_empty() || value.chars().any(char::is_whitespace) {
        return false;
    }
    let input = if value.contains("://") {
        value.to_owned()
    } else {
        format!("https://{value}")
    };
    url::Url::parse(&input).is_ok_and(|u| u.host_str().is_some())
}
pub(crate) fn keys<'a>(
    values: impl Iterator<Item = &'a str>,
    prefix: &str,
    env: bool,
    errors: &mut BTreeMap<String, String>,
) {
    let mut seen = std::collections::HashSet::new();
    for (i, key) in values.enumerate() {
        let invalid = if env {
            !super::environment::is_valid_key(key)
        } else {
            key.trim().is_empty()
        };
        if invalid {
            issue(
                errors,
                format!("{prefix}.{i}"),
                if env {
                    "Invalid variable name"
                } else {
                    "Field name is required"
                },
            );
        } else if !seen.insert(key) {
            issue(
                errors,
                format!("{prefix}.{i}"),
                if env {
                    "Duplicate variable"
                } else {
                    "Duplicate field name"
                },
            );
        }
    }
}
pub fn validate_entry(input: EntryInput) -> Result<BTreeMap<String, String>> {
    let mut errors = BTreeMap::new();
    if secrets::validate_input(&input).is_err() {
        issue(
            &mut errors,
            "name",
            if input.name.trim().is_empty() {
                "Name is required"
            } else if input.name.trim().len() > 256 {
                "Name is too long"
            } else {
                "Name must not contain `/` or control characters"
            },
        );
    }
    secrets::validate_ui(&input.payload, &mut errors)?;
    Ok(errors)
}
pub fn validate_profile(input: ProfileInput) -> Result<BTreeMap<String, String>> {
    let mut errors = BTreeMap::new();
    // Keep the canonical validator as the authority; field projections do not
    // impose new restrictions on direct backend writes.
    if format::validate_profile_input(&input).is_ok() {
        return Ok(errors);
    }
    let name = input.name.trim();
    if name.is_empty() {
        issue(&mut errors, "name", "Name is required");
    } else if name.len() > 128 {
        issue(&mut errors, "name", "Name is too long");
    } else if name.contains('/') || name.chars().any(char::is_control) {
        issue(
            &mut errors,
            "name",
            "Name must not contain `/` or control characters",
        );
    }
    let color = input.color.trim();
    if !(color.len() == 7
        && color.starts_with('#')
        && color[1..].bytes().all(|b| b.is_ascii_hexdigit()))
    {
        issue(&mut errors, "color", "Expected a #rrggbb color");
    }
    if input.browser.start_pages.len() > 64 {
        issue(&mut errors, "browser.startPages", "Too many start pages");
    }
    for (i, page) in format::profile_start_page_rows(&input) {
        if super::super::browser::normalize_start_url(page).is_err() {
            issue(
                &mut errors,
                format!("browser.startPages.{i}"),
                "Not a valid HTTP(S) URL",
            );
        }
    }
    if input
        .browser
        .autofill_entry_ids
        .iter()
        .any(|id| !super::super::ids::is_valid_id(id))
    {
        issue(&mut errors, "browser.autofillEntryIds", "Invalid entry ids");
    }
    Ok(errors)
}
pub fn recipient_key_kind(key: String) -> Result<Option<DomainRecipientKeyKind>> {
    let key = key.trim();
    // Parse only: identifying an age plugin must not launch a plugin binary.
    Ok(if age::x25519::Recipient::from_str(key).is_ok() {
        Some(DomainRecipientKeyKind::Age)
    } else if age::ssh::Recipient::from_str(key).is_ok() {
        Some(DomainRecipientKeyKind::Ssh)
    } else if age::plugin::Recipient::from_str(key).is_ok() {
        Some(DomainRecipientKeyKind::Plugin)
    } else {
        None
    })
}
pub fn validate_recipient(input: ValidateRecipientInput) -> Result<BTreeMap<String, String>> {
    let mut errors = BTreeMap::new();
    let label = input.label.trim();
    let key = input.key.trim();
    if label.is_empty() {
        issue(&mut errors, "label", "Label is required");
    } else if label.len() > 128 || label.chars().any(char::is_control) {
        issue(
            &mut errors,
            "label",
            "Label must be 1–128 printable characters",
        );
    } else if input.existing.iter().any(|r| r.label == label) {
        issue(&mut errors, "label", "A recipient with this label exists");
    }
    if key.is_empty() {
        issue(&mut errors, "key", "Public key is required");
    } else if recipient_key_kind(key.to_owned())?.is_none() {
        issue(
            &mut errors,
            "key",
            "Expected an age1… key or an ssh-ed25519 / ssh-rsa public key",
        );
    } else if input.existing.iter().any(|r| r.public_key == key) {
        issue(&mut errors, "key", "This key is already a recipient");
    }
    Ok(errors)
}

#[cfg(test)]
mod profile_placeholder_tests {
    use super::*;
    fn profile(pages: &[&str]) -> ProfileInput {
        ProfileInput {
            name: "profile".into(),
            color: "#2563eb".into(),
            icon: "folder".into(),
            description: String::new(),
            pinned: false,
            browser: BrowserSettings {
                start_pages: pages.iter().map(|s| (*s).into()).collect(),
                ..format::BrowserDoc::default().to_protocol()
            },
        }
    }
    #[test]
    fn profile_placeholder_errors_preserve_original_indices() {
        let errors = validate_profile(profile(&[
            " ",
            "not a URL",
            "",
            "https://valid.example",
            "file:///etc/passwd",
        ]))
        .unwrap();
        assert_eq!(
            errors.keys().map(String::as_str).collect::<Vec<_>>(),
            vec!["browser.startPages.1", "browser.startPages.4"]
        );
        assert!(
            validate_profile(profile(&["", " \t", "https://valid.example"]))
                .unwrap()
                .is_empty()
        );
    }
    #[test]
    fn profile_placeholder_rows_cannot_bypass_limit_or_unsafe_url_policy() {
        let mut input = profile(&[]);
        input.browser.start_pages = vec![String::new(); 64];
        assert!(validate_profile(input.clone()).unwrap().is_empty());
        assert!(format::validate_profile_input(&input).is_ok());
        input.browser.start_pages.push(String::new());
        assert!(
            validate_profile(input.clone())
                .unwrap()
                .contains_key("browser.startPages")
        );
        assert!(format::validate_profile_input(&input).is_err());
        for page in [
            "javascript:alert(1)",
            "file:///etc/passwd",
            "ftp://example.com",
            "--load-extension=/bad",
        ] {
            let errors = validate_profile(profile(&["", page])).unwrap();
            assert_eq!(errors.len(), 1);
            assert!(errors.contains_key("browser.startPages.1"));
        }
    }
}
