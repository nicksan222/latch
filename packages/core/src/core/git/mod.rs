//! Git feature interface over the system `git` binary.
//!
//! Repository, history, index and remote operations share one non-interactive
//! command policy. Callers use this facade; implementation modules stay private.

mod command;
mod history;
mod index;
mod remote;
mod repository;

use std::path::{Path, PathBuf};

use super::error::{Error, Result};
use crate::protocol::CommitInfo;
pub use command::available;
use command::{base_command, git_err};
pub use remote::{check_remote_url, clone, redact_url};

#[derive(Debug, Clone)]
pub struct Git {
    root: PathBuf,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GitStatus {
    pub branch: String,
    pub upstream: Option<String>,
    pub ahead: u32,
    pub behind: u32,
    pub dirty: bool,
}

/// One file touched by a commit (`git log --name-status`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileChange {
    /// A, M, D, T …
    pub status: char,
    pub path: String,
}

#[derive(Debug, Clone)]
pub struct LogItem {
    pub info: CommitInfo,
    pub changes: Vec<FileChange>,
}

impl Git {
    pub fn new(root: &Path) -> Self {
        Git {
            root: root.to_path_buf(),
        }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }
}

/// `2026-10-22T12:00:00+02:00` → `2026-10-22T10:00:00Z`.
pub fn normalize_git_time(s: &str) -> String {
    super::format::parse_time(s)
        .map(|t| {
            t.to_offset(time::UtcOffset::UTC)
                .format(&time::format_description::well_known::Rfc3339)
                .unwrap_or_else(|_| s.to_owned())
        })
        .unwrap_or_else(|| s.to_owned())
}

#[cfg(test)]
mod tests {
    #[test]
    fn remote_url_rules() {
        use super::{check_remote_url, redact_url};
        assert!(check_remote_url("https://user:tok@example.test/r.git").is_err());
        assert!(check_remote_url("--upload-pack=x").is_err());
        check_remote_url("git@github.com:a/b.git").unwrap();
        check_remote_url("https://example.test/r.git").unwrap();
        assert_eq!(
            redact_url("https://user:tok@example.test/r.git"),
            "https://***@example.test/r.git"
        );
        assert_eq!(
            redact_url("https://tok@example.test/r"),
            "https://***@example.test/r"
        );
        assert_eq!(redact_url("ssh://git@h/r"), "ssh://git@h/r");
        assert_eq!(
            redact_url("git@github.com:a/b.git"),
            "git@github.com:a/b.git"
        );
    }

    use super::*;

    fn repo() -> (tempfile::TempDir, Git) {
        let t = tempfile::tempdir().unwrap();
        let g = Git::new(t.path());
        g.init(Some("Test"), Some("test@example.com")).unwrap();
        (t, g)
    }

    #[test]
    fn init_commit_log_status() {
        let (t, g) = repo();
        assert!(g.is_repo());
        assert_eq!(g.head().unwrap(), None);
        assert_eq!(g.commit("nothing").unwrap(), None);
        std::fs::write(t.path().join("a.txt"), "1").unwrap();
        assert!(g.status().unwrap().dirty);
        g.add(&["a.txt"]).unwrap();
        let c = g.commit("first").unwrap().unwrap();
        assert_eq!(g.head().unwrap().as_deref(), Some(c.as_str()));
        let st = g.status().unwrap();
        assert!(!st.dirty);
        assert_eq!(st.branch, "main");
        let log = g.log(10).unwrap();
        assert_eq!(log.len(), 1);
        assert_eq!(log[0].message, "first");
        assert_eq!(log[0].author, "Test");
        assert!(log[0].at.ends_with('Z'));
        assert_eq!(g.show(&c, "a.txt").unwrap().unwrap(), b"1");
        assert_eq!(g.show(&c, "missing").unwrap(), None);
        std::fs::remove_file(t.path().join("a.txt")).unwrap();
        g.add(&["a.txt"]).unwrap();
        g.commit("rm").unwrap();
        let items = g.log_paths(&["a.txt"], &[]).unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].changes[0].status, 'D');
        assert_eq!(items[1].changes[0].status, 'A');
    }

    #[test]
    fn not_a_repo() {
        let t = tempfile::tempdir().unwrap();
        let g = Git::new(t.path());
        assert!(!g.is_repo());
    }

    #[test]
    fn remote_push_fetch() {
        let bare = tempfile::tempdir().unwrap();
        Git::new(bare.path())
            .run(&["init", "--bare", "-q", "-b", "main"])
            .unwrap();
        let (t, g) = repo();
        let url = bare.path().to_string_lossy().to_string();
        g.set_remote(&url).unwrap();
        assert_eq!(g.remote_url().as_deref(), Some(url.as_str()));
        std::fs::write(t.path().join("a"), "1").unwrap();
        g.add(&["a"]).unwrap();
        g.commit("one").unwrap();
        g.push("main").unwrap();
        g.fetch().unwrap();
        let st = g.status().unwrap();
        assert_eq!((st.ahead, st.behind), (0, 0));
        assert_eq!(st.upstream.as_deref(), Some("origin/main"));

        let other = tempfile::tempdir().unwrap();
        let g2 = clone(&url, &other.path().join("c")).unwrap();
        assert!(g2.is_repo());
        g.set_remote("").unwrap();
        assert_eq!(g.remote_url(), None);
    }

    #[test]
    fn time_normalization() {
        assert_eq!(
            normalize_git_time("2026-10-22T12:00:00+02:00"),
            "2026-10-22T10:00:00Z"
        );
        assert_eq!(normalize_git_time("garbage"), "garbage");
    }
}
