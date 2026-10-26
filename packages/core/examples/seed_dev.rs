//! Real encrypted development data. Run through tooling/seeder/seed-dev.py.
use anyhow::{Result, ensure};
use latch::{
    core::{paths, vault::Vault},
    protocol::*,
};
use serde_json::json;
use std::path::{Path, PathBuf};

const PASSPHRASE: &str = "correct horse battery staple";

fn reject_symlinks(root: &Path) -> Result<()> {
    if !root.exists() {
        return Ok(());
    }
    for item in std::fs::read_dir(root)? {
        let item = item?;
        let kind = item.file_type()?;
        ensure!(!kind.is_symlink(), "seed tree contains a symlink");
        if kind.is_dir() {
            reject_symlinks(&item.path())?;
        }
    }
    Ok(())
}

fn prepare(root: &Path) -> Result<()> {
    for (key, _) in std::env::vars_os() {
        if key.to_string_lossy().starts_with("GIT_") {
            ensure!(
                key == "GIT_CONFIG_GLOBAL" || key == "GIT_CONFIG_NOSYSTEM",
                "inherited Git overrides are unsafe; use tooling/seeder/seed-dev.py"
            );
        }
    }
    ensure!(
        std::env::var("GIT_CONFIG_GLOBAL").as_deref() == Ok("/dev/null")
            && std::env::var("GIT_CONFIG_NOSYSTEM").as_deref() == Ok("1"),
        "use tooling/seeder/seed-dev.py to isolate Git configuration"
    );
    // Check every component before core APIs can write trust/config state.
    for ancestor in root.ancestors() {
        ensure!(!ancestor.is_symlink(), "seed path contains a symlink");
    }
    for name in ["config", "data", "runtime", "ram", "vault"] {
        ensure!(!root.join(name).is_symlink(), "seed directory is a symlink");
    }
    for (name, actual) in [
        ("config", paths::config_dir()),
        ("data", paths::data_dir()),
        ("runtime", paths::runtime_dir()),
        ("ram", paths::ram_dir()),
    ] {
        ensure!(
            actual == root.join(name),
            "run using tooling/seeder/seed-dev.py (isolated paths required)"
        );
    }
    reject_symlinks(root)?;
    paths::ensure_secure_dir(root)?;
    for name in ["config", "data", "runtime", "ram"] {
        paths::ensure_secure_dir(&root.join(name))?;
    }
    Ok(())
}

fn seed(root: &Path) -> Result<()> {
    prepare(root)?;
    let lock = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(root.join("seed.lock"))?;
    fs2::FileExt::try_lock_exclusive(&lock)?;
    // Share the real agent's single-instance lock. A container may have been
    // killed without unlinking its socket; only remove that stale socket after
    // proving there is no live agent, and prevent a new one binding mid-seed.
    let agent_lock = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(root.join("runtime/agent.lock"))?;
    fs2::FileExt::try_lock_exclusive(&agent_lock)
        .map_err(|_| anyhow::anyhow!("stop the development agent before seeding"))?;
    let socket = root.join("runtime/agent.sock");
    if socket.exists() {
        std::fs::remove_file(socket)?;
    }
    let path = root.join("vault");
    let mut vault = if path.join(".latch/version").exists() {
        Vault::open(&path)?.unlock_passphrase(PASSPHRASE)?
    } else {
        Vault::init(&InitParams {
            path: path.display().to_string(),
            passphrase: PASSPHRASE.into(),
            kdf: Some(KdfParams {
                memory_kib: 65536,
                iterations: 3,
                parallelism: 1,
            }),
            git_name: Some("Latch Demo".into()),
            git_email: Some("demo@example.invalid".into()),
            remote_url: None,
            recovery_kit: false,
        })?
        .0
    };
    ensure!(
        vault.config().kdf.params().memory_kib >= 65536,
        "seed vault has a weak KDF"
    );
    ensure!(
        vault.integrity_problems().is_empty(),
        "seed vault integrity check failed"
    );
    let marker_path = root.join("seed.json");
    let marker = json!({ "version": 1, "vaultId": vault.id() });
    if std::fs::read(&marker_path)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
        .as_ref()
        == Some(&marker)
    {
        register(&path, vault.id())?;
        return Ok(());
    }
    for (index, (name, color, description)) in [
        (
            "Acme Studio",
            "#6366f1",
            "Client storefront and staging infrastructure",
        ),
        ("Orbit Labs", "#14b8a6", "Team analytics platform"),
        ("Personal", "#f59e0b", "Side projects and personal accounts"),
    ]
    .into_iter()
    .enumerate()
    {
        let profile = match vault.list_profiles()?.into_iter().find(|p| p.name == name) {
            Some(p) => p.id,
            None => {
                vault
                    .create_profile(&ProfileInput {
                        name: name.into(),
                        color: color.into(),
                        icon: "briefcase".into(),
                        description: description.into(),
                        pinned: index == 0,
                        browser: BrowserSettings {
                            start_pages: vec!["https://example.com".into()],
                            bookmarks: vec![Bookmark {
                                title: "Project dashboard".into(),
                                url: "https://example.com/dashboard".into(),
                            }],
                            autofill: AutofillScope::All,
                            autofill_entry_ids: vec![],
                        },
                    })?
                    .id
            }
        };
        let payloads = [
            (
                "Team console",
                json!({"type":"login","data":{"username":"alex@example.invalid","password":"demo-console-password-v1","urls":["https://example.com"],"totp":{"secret":"JBSWY3DPEHPK3PXP","algorithm":"SHA1","digits":6,"period":30,"issuer":"Latch Demo"},"custom":[]}}),
            ),
            (
                "Webhook signing secret",
                json!({"type":"secret","data":{"value":"demo-webhook-signing-secret"}}),
            ),
            (
                "Development environment",
                json!({"type":"env","data":{"vars":[{"key":"DATABASE_URL","value":"postgres://demo:demo-db-password@localhost:5432/app","hidden":true,"comment":"Local development only"},{"key":"NODE_ENV","value":"development","hidden":false,"comment":null}]}}),
            ),
            (
                "Deployment checklist",
                json!({"type":"note","data":{"body":"# Release checklist\n\n- Run migrations\n- Verify health checks\n\nDemo recovery code: demo-recovery-1234"}}),
            ),
            (
                "Local certificate",
                json!({"type":"file","data":{"filename":"demo-certificate.txt","mime":"text/plain","contentBase64":"ZGVtby1jZXJ0aWZpY2F0ZQo=","size":17}}),
            ),
            (
                "Staging SSH key",
                json!({"type":"ssh_key","data":{"privateKey":"DEMO ONLY - not a usable SSH private key","publicKey":"DEMO ONLY - not a usable SSH public key","passphrase":"demo-ssh-passphrase","comment":"Nonfunctional demo key"}}),
            ),
            (
                "Staging PostgreSQL",
                json!({"type":"database","data":{"engine":"postgres","host":"localhost","port":5432,"database":"app_staging","username":"demo","password":"demo-db-password","options":"sslmode=prefer"}}),
            ),
            (
                "Payments sandbox",
                json!({"type":"api_key","data":{"key":"sk_test_demo_latch_not_valid","secret":"demo-payments-secret","endpoint":"https://api.example.com","expiresAt":null}}),
            ),
        ];
        for (name, payload) in payloads {
            if vault
                .list_entries(&ListEntriesParams {
                    profile_id: Some(profile.clone()),
                    query: None,
                    entry_type: None,
                })?
                .iter()
                .any(|e| e.name == name)
            {
                continue;
            }
            let input = EntryInput {
                profile_id: profile.clone(),
                name: name.into(),
                tags: vec!["demo".into(), "development".into()],
                notes: "Fictional development data; never use in production.".into(),
                favorite: name == "Team console",
                payload: serde_json::from_value(payload)?,
            };
            let entry = vault.create_entry(&input)?;
            if name == "Team console" {
                let mut updated = input;
                if let EntryPayload::Login(login) = &mut updated.payload {
                    login.password = Some("demo-console-password-v2".into());
                }
                vault.update_entry(&EntryUpdateParams {
                    id: entry.id,
                    input: updated,
                })?;
            }
        }
        let trash_name = "Retired staging token";
        if !vault
            .trash(&TrashParams {
                profile_id: Some(profile.clone()),
            })?
            .iter()
            .any(|e| e.name == trash_name)
        {
            let existing = vault
                .list_entries(&ListEntriesParams {
                    profile_id: Some(profile.clone()),
                    query: None,
                    entry_type: None,
                })?
                .into_iter()
                .find(|e| e.name == trash_name);
            let id = match existing {
                Some(e) => e.id,
                None => {
                    vault
                        .create_entry(&EntryInput {
                            profile_id: profile,
                            name: trash_name.into(),
                            tags: vec!["demo".into()],
                            notes: String::new(),
                            favorite: false,
                            payload: EntryPayload::Secret(SecretData {
                                value: Some("demo-retired-token".into()),
                            }),
                        })?
                        .id
                }
            };
            vault.delete_entry(&id)?;
        }
    }
    register(&path, vault.id())?;
    // Completion marker stays outside the encrypted repository. Subsequent runs
    // preserve renames and deletions as well as value edits. A failed first run
    // can still resume using the name checks above.
    std::fs::write(marker_path, serde_json::to_vec_pretty(&marker)?)?;
    Ok(())
}

fn register(path: &Path, id: &str) -> Result<()> {
    let mut registry = latch::agent::state::VaultsFile::load(&paths::config_dir());
    // A bind mount changes absolute paths between host and container. Replace
    // the old registration for this vault rather than adding a broken duplicate.
    registry.vaults.retain(|entry| entry.vault_id != id);
    registry.activate(path, id);
    registry.save(&paths::config_dir())?;
    Ok(())
}

fn main() -> Result<()> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join(".dev/latch");
    seed(&root)?;
    println!(
        "Encrypted development vault ready at {}",
        root.join("vault").display()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    #[test]
    fn encrypted_idempotent_seed() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join(".dev/latch");
        let mut child = Command::new(std::env::current_exe().unwrap());
        child.args(["--ignored", "--exact", "tests::seed_worker"]);
        child.env("LATCH_SEED_TEST_ROOT", &root);
        for name in ["config", "data", "runtime", "ram"] {
            child.env(
                format!("LATCH_{}_DIR", name.to_uppercase()),
                root.join(name),
            );
        }
        for (key, _) in std::env::vars().filter(|(k, _)| k.starts_with("GIT_")) {
            child.env_remove(key);
        }
        child
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_NOSYSTEM", "1");
        child.env_remove("LATCH_ALLOW_WEAK_KDF");
        assert!(child.status().unwrap().success());
    }

    #[test]
    #[ignore = "subprocess worker with isolated environment"]
    fn seed_worker() -> Result<()> {
        let root = PathBuf::from(std::env::var_os("LATCH_SEED_TEST_ROOT").unwrap());
        seed(&root)?;
        let vault = Vault::open(root.join("vault"))?;
        assert!(vault.config().kdf.params().memory_kib >= 65536);
        assert!(vault.unlock_passphrase("wrong passphrase").is_err());
        let mut unlocked = vault.unlock_passphrase(PASSPHRASE)?;
        assert!(unlocked.integrity_problems().is_empty());
        let profiles = unlocked.list_profiles()?;
        assert_eq!(profiles.len(), 3);
        for profile in &profiles {
            let entries = unlocked.list_entries(&ListEntriesParams {
                profile_id: Some(profile.id.clone()),
                query: None,
                entry_type: None,
            })?;
            assert_eq!(entries.len(), 8);
            let kinds: std::collections::HashSet<_> = entries
                .iter()
                .map(|e| format!("{:?}", e.entry_type))
                .collect();
            assert_eq!(kinds.len(), 8);
            let login = entries.iter().find(|e| e.name == "Team console").unwrap();
            assert_eq!(unlocked.history(&login.id)?.len(), 2);
            assert_eq!(unlocked.totp(&login.id)?.code.len(), 6);
            assert_eq!(
                unlocked
                    .trash(&TrashParams {
                        profile_id: Some(profile.id.clone())
                    })?
                    .len(),
                1
            );
        }
        // A rerun must preserve an explicit developer edit and create no commits.
        let entry = unlocked.find_entry(&profiles[0].id, "Webhook signing secret")?;
        let mut input = latch::core::secrets::to_input(&unlocked.read_entry_doc(&entry.id)?);
        input.payload = EntryPayload::Secret(SecretData {
            value: Some("developer-edited-value".into()),
        });
        unlocked.update_entry(&EntryUpdateParams {
            id: entry.id.clone(),
            input,
        })?;
        let profile = unlocked.get_profile(&profiles[0].id)?;
        unlocked.update_profile(&ProfileUpdateParams {
            id: profile.id.clone(),
            input: ProfileInput {
                name: "Renamed demo".into(),
                color: profile.color,
                icon: profile.icon,
                description: profile.description,
                pinned: profile.pinned,
                browser: profile.browser,
            },
        })?;
        let deleted = unlocked.find_entry(&profiles[0].id, "Deployment checklist")?;
        unlocked.delete_entry(&deleted.id)?;
        // Simulate the same checkout mounted at another absolute path.
        let mut registry = latch::agent::state::VaultsFile::load(&root.join("config"));
        registry.vaults[0].path = "/old/workspace/vault".into();
        registry.active = Some("/old/workspace/vault".into());
        registry.save(&root.join("config"))?;
        let agent_lock = std::fs::OpenOptions::new()
            .write(true)
            .open(root.join("runtime/agent.lock"))?;
        fs2::FileExt::try_lock_exclusive(&agent_lock)?;
        assert!(seed(&root).is_err());
        drop(agent_lock);
        // Model an unclean stop which left a stale socket behind.
        #[cfg(unix)]
        let stale_socket = std::os::unix::net::UnixListener::bind(root.join("runtime/agent.sock"))?;
        #[cfg(unix)]
        drop(stale_socket);
        let head = unlocked.git().head()?;
        drop(unlocked);
        seed(&root)?;
        assert!(!root.join("runtime/agent.sock").exists());
        let unlocked = vault.unlock_passphrase(PASSPHRASE)?;
        assert_eq!(unlocked.git().head()?, head);
        assert_eq!(unlocked.list_profiles()?.len(), 3);
        assert_eq!(unlocked.get_profile(&profiles[0].id)?.name, "Renamed demo");
        assert!(
            unlocked
                .find_entry(&profiles[0].id, "Deployment checklist")
                .is_err()
        );
        assert_eq!(
            unlocked
                .reveal(&RevealParams {
                    id: entry.id,
                    field: "value".into()
                })?
                .value,
            "developer-edited-value"
        );
        let registry = latch::agent::state::VaultsFile::load(&root.join("config"));
        assert_eq!(registry.vaults.len(), 1);
        assert_eq!(registry.active_path(), Some(root.join("vault")));
        // Scan all reachable Git blobs, including deleted/previous versions.
        let git = unlocked.git();
        let objects = git.run(&["rev-list", "--objects", "--all"])?;
        let markers = [
            PASSPHRASE,
            "demo-console-password",
            "demo-webhook",
            "demo-db-password",
            "demo-retired-token",
            "Acme Studio",
            "developer-edited-value",
            "demo-recovery-1234",
            "JBSWY3DPEHPK3PXP",
        ];
        for line in objects.lines() {
            let oid = line.split_whitespace().next().unwrap();
            if git.run(&["cat-file", "-t", oid])?.trim() == "blob" {
                let bytes = git.run_bytes(&["cat-file", "blob", oid])?;
                for marker in markers {
                    assert!(
                        !bytes.windows(marker.len()).any(|w| w == marker.as_bytes()),
                        "plaintext in Git blob"
                    );
                }
            }
        }
        fn scan(path: &Path, markers: &[&str]) -> Result<()> {
            for item in std::fs::read_dir(path)? {
                let path = item?.path();
                if path.is_dir() {
                    scan(&path, markers)?;
                } else {
                    let bytes = std::fs::read(&path)?;
                    for marker in markers {
                        assert!(
                            !bytes.windows(marker.len()).any(|w| w == marker.as_bytes()),
                            "plaintext in vault"
                        );
                    }
                }
            }
            Ok(())
        }
        scan(&root.join("vault"), &markers)?;
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn refuses_nested_symlinks() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("seed");
        let outside = temp.path().join("outside");
        std::fs::create_dir_all(root.join("config")).unwrap();
        std::fs::create_dir(&outside).unwrap();
        std::fs::write(outside.join("sentinel"), "untouched").unwrap();
        std::os::unix::fs::symlink(&outside, root.join("config/trust")).unwrap();
        assert!(reject_symlinks(&root).is_err());
        assert_eq!(
            std::fs::read_to_string(outside.join("sentinel")).unwrap(),
            "untouched"
        );
    }

    #[test]
    fn refuses_unisolated_environment_before_writing() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("seed");
        assert!(seed(&root).is_err());
        assert!(!root.exists());
    }
}
