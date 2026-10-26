//! Work-tree discovery, configuration, references and status.

use std::path::PathBuf;
use std::process::Output;

use super::{Error, Git, GitStatus, Result, git_err};

const FALLBACK_NAME: &str = "Latch";
const FALLBACK_EMAIL: &str = "latch@localhost";

impl Git {
    /// Discover the containing work tree from this directory. Adapters should
    /// use this method rather than starting their own Git subprocesses.
    pub fn work_tree_root(&self) -> Result<PathBuf> {
        let top = self.run(&["rev-parse", "--show-toplevel"])?;
        Ok(PathBuf::from(top.trim_end_matches(['\r', '\n'])))
    }

    /// True when `root` is the top level of a git work tree.
    pub fn is_repo(&self) -> bool {
        let Ok(top) = self.work_tree_root() else {
            return false;
        };
        match (top.canonicalize(), self.root.canonicalize()) {
            (Ok(a), Ok(b)) => a == b,
            _ => false,
        }
    }

    pub fn init(&self, name: Option<&str>, email: Option<&str>) -> Result<()> {
        std::fs::create_dir_all(&self.root)
            .map_err(|e| Error::io(format!("create {}", self.root.display()), e))?;
        self.run(&["init", "--quiet", "-b", "main"])?;
        if let Some(n) = name.filter(|n| !n.is_empty()) {
            self.run(&["config", "user.name", n])?;
        }
        if let Some(e) = email.filter(|e| !e.is_empty()) {
            self.run(&["config", "user.email", e])?;
        }
        Ok(())
    }

    pub fn set_identity(&self, name: &str, email: &str) -> Result<()> {
        self.run(&["config", "user.name", name])?;
        self.run(&["config", "user.email", email])?;
        Ok(())
    }

    pub(super) fn config_get(&self, key: &str) -> Option<String> {
        self.run(&["config", "--get", key])
            .ok()
            .map(|s| s.trim().to_owned())
            .filter(|s| !s.is_empty())
    }

    /// Identity overrides for commits when the user has none configured.
    pub(super) fn identity_args(&self) -> Vec<String> {
        let mut v = vec![];
        if self.config_get("user.name").is_none() {
            v.extend(["-c".to_owned(), format!("user.name={FALLBACK_NAME}")]);
        }
        if self.config_get("user.email").is_none() {
            v.extend(["-c".to_owned(), format!("user.email={FALLBACK_EMAIL}")]);
        }
        v
    }

    /// Run with commit identity fallbacks (commit, rebase --continue …).
    pub fn run_with_identity(&self, args: &[&str]) -> Result<Output> {
        let id = self.identity_args();
        let mut all: Vec<&str> = id.iter().map(String::as_str).collect();
        all.extend_from_slice(args);
        self.output(&all)
    }

    /// The current commit, or `None` for an unborn branch. Repository and
    /// execution failures remain errors rather than masquerading as empty history.
    pub fn head(&self) -> Result<Option<String>> {
        let args = ["rev-parse", "--verify", "-q", "HEAD"];
        let out = self.output(&args)?;
        match out.status.code() {
            Some(0) => Ok(Some(String::from_utf8_lossy(&out.stdout).trim().to_owned())),
            Some(1) => Ok(None),
            _ => Err(git_err(&args, &out)),
        }
    }

    pub fn current_branch(&self) -> String {
        self.run(&["symbolic-ref", "--short", "-q", "HEAD"])
            .map(|s| s.trim().to_owned())
            .ok()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "main".into())
    }

    pub fn status(&self) -> Result<GitStatus> {
        let s = self.run(&[
            "status",
            "--porcelain=v2",
            "--branch",
            "--untracked-files=all",
        ])?;
        let mut st = GitStatus::default();
        for line in s.lines() {
            if let Some(h) = line.strip_prefix("# branch.head ") {
                st.branch = h.trim().to_owned();
            } else if let Some(u) = line.strip_prefix("# branch.upstream ") {
                st.upstream = Some(u.trim().to_owned());
            } else if let Some(ab) = line.strip_prefix("# branch.ab ") {
                for part in ab.split_whitespace() {
                    if let Some(a) = part.strip_prefix('+') {
                        st.ahead = a.parse().unwrap_or(0);
                    } else if let Some(b) = part.strip_prefix('-') {
                        st.behind = b.parse().unwrap_or(0);
                    }
                }
            } else if !line.starts_with('#') && !line.trim().is_empty() {
                st.dirty = true;
            }
        }
        if st.branch.is_empty() || st.branch == "(detached)" {
            st.branch = self.current_branch();
        }
        // No upstream configured but origin/<branch> exists: compute anyway.
        if st.upstream.is_none() {
            let r = format!("origin/{}", st.branch);
            if self.ref_exists(&format!("refs/remotes/{r}")) {
                let (a, b) = self.ahead_behind("HEAD", &r)?;
                st.ahead = a;
                st.behind = b;
                st.upstream = Some(r);
            }
        }
        Ok(st)
    }

    pub fn ref_exists(&self, r: &str) -> bool {
        self.ok(&["rev-parse", "--verify", "-q", r])
    }

    /// (ahead, behind) of `a` relative to `b`.
    pub fn ahead_behind(&self, a: &str, b: &str) -> Result<(u32, u32)> {
        if self.head()?.is_none() {
            let behind = self
                .run(&["rev-list", "--count", b])
                .ok()
                .and_then(|s| s.trim().parse().ok())
                .unwrap_or(0);
            return Ok((0, behind));
        }
        let s = self.run(&["rev-list", "--left-right", "--count", &format!("{a}...{b}")])?;
        let mut it = s.split_whitespace().map(|x| x.parse::<u32>().unwrap_or(0));
        Ok((it.next().unwrap_or(0), it.next().unwrap_or(0)))
    }

    pub fn git_path(&self, name: &str) -> Result<PathBuf> {
        let p = self.run(&["rev-parse", "--git-path", name])?;
        let p = PathBuf::from(p.trim());
        Ok(if p.is_absolute() {
            p
        } else {
            self.root.join(p)
        })
    }

    pub fn rebase_in_progress(&self) -> bool {
        ["rebase-merge", "rebase-apply"]
            .iter()
            .any(|n| self.git_path(n).is_ok_and(|p| p.exists()))
    }

    pub fn hooks_dir(&self) -> Result<PathBuf> {
        self.git_path("hooks")
    }
}
