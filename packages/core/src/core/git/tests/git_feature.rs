// Acceptance contracts for the system-Git feature, using local repositories only.
use latch::core::error::Error;
use latch::core::git::{self, Git};
use std::path::Path;
use std::process::Command;

const CHILD_TEST: &str = "LATCH_GIT_FEATURE_CHILD";

fn exact_test(name: &str) -> Command {
    let mut command = Command::new(std::env::current_exe().unwrap());
    command.args(["--exact", name, "--nocapture"]);
    command
}

fn assert_child_passes(command: &mut Command) {
    let output = command.output().unwrap();
    assert!(
        output.status.success(),
        "child test failed:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("1 passed"),
        "isolated Git child must execute the exact scenario"
    );
}

/// Reexecute before any real Git operation, without mutating the environment
/// shared by the parallel test harness. The marker matches only this exact test.
fn isolated(name: &str) -> bool {
    if std::env::var(CHILD_TEST).as_deref() == Ok(name) {
        return true;
    }
    let sandbox = tempfile::tempdir().unwrap();
    let config = sandbox.path().join("empty.gitconfig");
    let templates = sandbox.path().join("empty-templates");
    std::fs::write(&config, "").unwrap();
    std::fs::create_dir(&templates).unwrap();
    let mut command = exact_test(name);
    // Drop repository redirects, injected config, identities, alternate object
    // stores, transport overrides, and every other inherited GIT_* setting.
    for (key, _) in std::env::vars_os() {
        if key.to_string_lossy().starts_with("GIT_") {
            command.env_remove(key);
        }
    }
    for key in [
        "EMAIL",
        "SSH_ASKPASS",
        "SSH_ASKPASS_REQUIRE",
        "SSH_AUTH_SOCK",
        "SSH_AGENT_PID",
    ] {
        command.env_remove(key);
    }
    command
        .env(CHILD_TEST, name)
        .env("GIT_CONFIG_GLOBAL", config)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_TEMPLATE_DIR", templates);
    // Keep the sandbox alive until every Git subprocess in the child exits.
    assert_child_passes(&mut command);
    false
}

fn repo(path: &Path) -> Git {
    let git = Git::new(path);
    git.init(Some("Git feature"), Some("git-feature@example.test"))
        .unwrap();
    git.run(&["config", "commit.gpgsign", "false"]).unwrap();
    git
}

fn write_commit(git: &Git, path: &str, contents: &[u8], message: &str) -> String {
    std::fs::write(git.root().join(path), contents).unwrap();
    git.commit_paths(message, &[path.into()], &[], &|_| Ok(()))
        .unwrap()
        .unwrap()
}

#[test]
fn credential_urls_are_refused_before_repository_mutation() {
    if !isolated("credential_urls_are_refused_before_repository_mutation") {
        return;
    }
    let temp = tempfile::tempdir().unwrap();
    let repository = repo(&temp.path().join("vault"));
    repository.set_remote("git@example.test:vault.git").unwrap();
    for url in [
        "https://user:sentinel-secret@example.test/vault.git",
        "https://sentinel-secret@example.test/vault.git",
        "http://sentinel-secret@example.test/vault.git",
        "--upload-pack=sentinel-secret",
        "https://example.test/vault\nsentinel-secret",
    ] {
        let error = repository.set_remote(url).unwrap_err();
        assert!(matches!(error, Error::InvalidParams(_)));
        assert!(!error.to_string().contains("sentinel-secret"));
        assert_eq!(
            repository.remote_url().as_deref(),
            Some("git@example.test:vault.git")
        );
        let destination = temp.path().join("refused-clone");
        let error = git::clone(url, &destination).unwrap_err();
        assert!(matches!(error, Error::InvalidParams(_)));
        assert!(!error.to_string().contains("sentinel-secret"));
        assert!(!destination.exists());
    }
    repository
        .set_remote("ssh://git@example.test/vault.git")
        .unwrap();
    // Older configs may already contain credentials. Never send them to Git.
    repository
        .run(&[
            "config",
            "remote.origin.url",
            "https://sentinel-secret@example.test/vault.git",
        ])
        .unwrap();
    for result in [repository.fetch(), repository.push("main")] {
        let error = result.unwrap_err();
        assert!(matches!(error, Error::InvalidParams(_)));
        assert!(!error.to_string().contains("sentinel-secret"));
    }
}

#[test]
fn repository_failures_are_not_empty_history() {
    if !isolated("repository_failures_are_not_empty_history") {
        return;
    }
    let temp = tempfile::tempdir().unwrap();
    let invalid = Git::new(temp.path());
    assert!(matches!(invalid.head(), Err(Error::Git(_))));
    assert!(matches!(invalid.log(10), Err(Error::Git(_))));
    assert!(matches!(invalid.status(), Err(Error::Git(_))));
    let empty = repo(&temp.path().join("empty"));
    assert_eq!(empty.head().unwrap(), None);
    assert!(empty.log(10).unwrap().is_empty());
}

#[test]
fn selected_commits_preserve_user_staging_and_read_binary_history() {
    if !isolated("selected_commits_preserve_user_staging_and_read_binary_history") {
        return;
    }
    let temp = tempfile::tempdir().unwrap();
    let repository = repo(&temp.path().join("vault with spaces"));
    assert!(repository.is_repo());
    assert_eq!(repository.status().unwrap().branch, "main");
    std::fs::write(repository.root().join("personal.txt"), "not vault data").unwrap();
    repository.add(&["personal.txt"]).unwrap();
    let path = "ciphertext with spaces.age";
    let first = write_commit(&repository, path, b"\0first\xff", "encrypted snapshot one");
    assert_eq!(repository.tree_paths(&first).unwrap(), [path]);
    assert_eq!(repository.staged_files().unwrap(), ["personal.txt"]);
    assert!(repository.status().unwrap().dirty);
    let second = write_commit(&repository, path, b"\0second\xfe", "encrypted snapshot two");
    assert_eq!(
        repository.show(&first, path).unwrap().unwrap(),
        b"\0first\xff"
    );
    assert_eq!(
        repository
            .read_blobs(&[format!("{second}:{path}"), format!("{first}:missing")])
            .unwrap(),
        [Some(b"\0second\xfe".to_vec()), None]
    );
    let history = repository.log_paths(&[path], &[]).unwrap();
    assert_eq!(history.len(), 2);
    assert_eq!(history[0].info.message, "encrypted snapshot two");
    assert_eq!(history[0].changes[0].status, 'M');
    assert_eq!(history[1].changes[0].status, 'A');
    assert_eq!(
        repository
            .commit_paths("unchanged", &[path.into()], &[], &|_| Ok(()))
            .unwrap(),
        None
    );
    let before = repository.head().unwrap();
    std::fs::write(repository.root().join(path), b"rejected").unwrap();
    assert!(
        repository
            .commit_paths("rejected", &[path.into()], &[], &|_| Err(
                Error::InvalidParams("rejected tree".into())
            ))
            .is_err()
    );
    assert_eq!(repository.head().unwrap(), before);
    assert_eq!(repository.staged_files().unwrap(), ["personal.txt"]);
    let metadata = repository.git_path("").unwrap();
    assert!(!std::fs::read_dir(metadata).unwrap().any(|entry| {
        entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with("latch-index-")
    }));
}

#[test]
fn local_remote_clone_fetch_and_divergence() {
    if !isolated("local_remote_clone_fetch_and_divergence") {
        return;
    }
    let temp = tempfile::tempdir().unwrap();
    let bare = temp.path().join("remote with spaces.git");
    std::fs::create_dir(&bare).unwrap();
    Git::new(&bare)
        .run(&["init", "--bare", "--quiet", "-b", "main"])
        .unwrap();
    let writer = repo(&temp.path().join("writer"));
    let first = write_commit(&writer, "one.age", b"encrypted one", "one");
    writer.set_remote(bare.to_str().unwrap()).unwrap();
    writer.push("main").unwrap();
    let reader = git::clone(bare.to_str().unwrap(), &temp.path().join("reader")).unwrap();
    reader
        .set_identity("Reader", "reader@example.test")
        .unwrap();
    reader.run(&["config", "commit.gpgsign", "false"]).unwrap();
    assert_eq!(reader.head().unwrap().as_deref(), Some(first.as_str()));
    write_commit(&writer, "two.age", b"encrypted two", "two");
    writer.push("main").unwrap();
    reader.fetch().unwrap();
    let status = reader.status().unwrap();
    assert_eq!((status.ahead, status.behind), (0, 1));
    assert_eq!(status.upstream.as_deref(), Some("origin/main"));
    write_commit(&reader, "three.age", b"encrypted three", "three");
    let status = reader.status().unwrap();
    assert_eq!((status.ahead, status.behind), (1, 1));
    assert!(!status.dirty);
    assert!(matches!(reader.push("main"), Err(Error::Git(_))));
    reader.set_remote("").unwrap();
    assert_eq!(reader.remote_url(), None);
    assert!(matches!(reader.fetch(), Err(Error::Git(_))));
    assert!(matches!(
        git::clone(
            temp.path().join("missing").to_str().unwrap(),
            &temp.path().join("failed")
        ),
        Err(Error::Git(_))
    ));
}

#[test]
fn discovery_finds_work_tree_from_nested_directories() {
    if !isolated("discovery_finds_work_tree_from_nested_directories") {
        return;
    }
    let temp = tempfile::tempdir().unwrap();
    let repository = repo(&temp.path().join("vault with spaces"));
    let nested = repository.root().join("nested/directory");
    std::fs::create_dir_all(&nested).unwrap();
    let discovered = Git::new(&nested).work_tree_root().unwrap();
    assert_eq!(
        discovered.canonicalize().unwrap(),
        repository.root().canonicalize().unwrap()
    );
    assert!(!Git::new(&nested).is_repo());
    assert!(Git::new(temp.path()).work_tree_root().is_err());
}

#[test]
fn every_configured_remote_url_is_checked_before_transport() {
    if !isolated("every_configured_remote_url_is_checked_before_transport") {
        return;
    }
    let temp = tempfile::tempdir().unwrap();
    let repository = repo(&temp.path().join("vault"));
    let safe = temp.path().join("missing-local-remote");
    let safe = safe.to_str().unwrap();
    for key in ["remote.origin.url", "remote.origin.pushurl"] {
        repository.set_remote(safe).unwrap();
        repository
            .run(&[
                "config",
                "--replace-all",
                key,
                "http://user:sentinel-secret@127.0.0.1:1/nonexistent-latch-repo",
            ])
            .unwrap();
        repository.run(&["config", "--add", key, safe]).unwrap();
        let error = repository.push("main").unwrap_err();
        assert!(matches!(error, Error::InvalidParams(_)), "{error}");
        assert!(!error.to_string().contains("sentinel-secret"));
        repository.set_remote("").unwrap();
    }
}

#[cfg(unix)]
#[test]
fn system_git_runs_without_terminal_or_editor_prompts() {
    if !isolated("system_git_runs_without_terminal_or_editor_prompts") {
        return;
    }
    let temp = tempfile::tempdir().unwrap();
    let repository = repo(&temp.path().join("vault"));
    // A real Git alias observes the subprocess environment without a network
    // request, credential prompt, editor, or dependency on user configuration.
    let output = repository
        .run(&[
            "-c",
            "alias.latch-prompt-contract=!printf '%s/%s/%s' \"$GIT_TERMINAL_PROMPT\" \"$GIT_EDITOR\" \"$GIT_MERGE_AUTOEDIT\"",
            "latch-prompt-contract",
        ])
        .unwrap();
    assert_eq!(output, "0/true/no");
}

#[cfg(unix)]
#[test]
fn hostile_inherited_git_configuration_is_ignored() {
    use latch_automation::fixtures::{self, FixtureConfig, GitCallbackConfig};
    const NAME: &str = "hostile_inherited_git_configuration_is_ignored";
    const FIXTURE: &str = "LATCH_GIT_FEATURE_HOSTILE_FIXTURE";
    let Ok(root) = std::env::var(FIXTURE) else {
        // Only fixture files and child environment are changed here. In
        // particular, never edit the user's actual Git config or environment.
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        let hooks = root.join("hostile-hooks");
        let templates = root.join("hostile-templates");
        std::fs::create_dir_all(&hooks).unwrap();
        std::fs::create_dir_all(templates.join("hooks")).unwrap();
        std::fs::write(templates.join("template-sentinel"), "untrusted template").unwrap();
        for path in [
            hooks.join("post-checkout"),
            hooks.join("signer"),
            templates.join("hooks/post-checkout"),
        ] {
            fixtures::install(
                FixtureConfig::GitCallback(GitCallbackConfig {
                    marker_path: root.join("executed"),
                    exit_status: 73,
                }),
                &path,
            )
            .unwrap();
        }
        // Git config accepts quoted strings with backslash/quote escaping.
        let quote = |path: &Path| serde_json::to_string(path.to_str().unwrap()).unwrap();
        let global = root.join("global.gitconfig");
        let system = root.join("system.gitconfig");
        std::fs::write(&global, format!(
            "[core]\n hooksPath = {}\n[gpg]\n program = {}\n[commit]\n gpgsign = true\n[init]\n templateDir = {}\n[isolation]\n global = hostile\n",
            quote(&hooks), quote(&hooks.join("signer")), quote(&templates)
        )).unwrap();
        std::fs::write(&system, "[isolation]\n system = hostile\n").unwrap();
        let mut command = exact_test(NAME);
        command
            .env_remove(CHILD_TEST)
            .env(FIXTURE, root)
            .env("GIT_CONFIG_GLOBAL", global)
            .env("GIT_CONFIG_SYSTEM", system)
            .env("GIT_CONFIG_NOSYSTEM", "0")
            .env("GIT_TEMPLATE_DIR", templates)
            .env("GIT_CONFIG_COUNT", "1")
            .env("GIT_CONFIG_KEY_0", "isolation.injected")
            .env("GIT_CONFIG_VALUE_0", "hostile")
            .env("GIT_CONFIG_PARAMETERS", "'isolation.parameters=hostile'")
            .env("GIT_DIR", root.join("redirected-git-dir"))
            .env("GIT_WORK_TREE", root.join("redirected-work-tree"))
            .env("GIT_INDEX_FILE", root.join("redirected-index"));
        assert_child_passes(&mut command);
        assert!(!root.join("executed").exists());
        assert!(!root.join("redirected-git-dir").exists());
        assert!(!root.join("redirected-work-tree").exists());
        assert!(!root.join("redirected-index").exists());
        return;
    };
    if !isolated(NAME) {
        return;
    }
    let root = Path::new(&root);
    let source = Git::new(&root.join("source"));
    source
        .init(Some("Isolated"), Some("isolated@example.test"))
        .unwrap();
    for key in [
        "isolation.global",
        "isolation.system",
        "isolation.injected",
        "isolation.parameters",
        "core.hooksPath",
        "commit.gpgsign",
        "gpg.program",
    ] {
        assert!(
            !source
                .output(&["config", "--get", key])
                .unwrap()
                .status
                .success(),
            "inherited {key}"
        );
    }
    std::fs::write(source.root().join("ciphertext.age"), b"encrypted fixture").unwrap();
    source.add(&["ciphertext.age"]).unwrap();
    let commit = source.commit("isolated commit").unwrap().unwrap();
    let bare = root.join("bare.git");
    std::fs::create_dir(&bare).unwrap();
    let remote = Git::new(&bare);
    remote
        .run(&["init", "--bare", "--quiet", "-b", "main"])
        .unwrap();
    source.set_remote(bare.to_str().unwrap()).unwrap();
    source.push("main").unwrap();
    let cloned = git::clone(bare.to_str().unwrap(), &root.join("clone")).unwrap();
    assert_eq!(cloned.head().unwrap().as_deref(), Some(commit.as_str()));
    for repository in [&source, &remote, &cloned] {
        assert!(!repository.git_path("template-sentinel").unwrap().exists());
        assert!(!repository.git_path("hooks/post-checkout").unwrap().exists());
    }
    assert!(!root.join("executed").exists());
}
