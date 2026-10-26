//! Work-tree selection, isolated index commits and staging.

use std::path::PathBuf;

use super::{Git, Result, git_err};

impl Git {
    /// Paths whose work-tree state differs from HEAD (modified, deleted,
    /// added) plus every untracked path (ignored ones included: callers
    /// filter by their own allowlist). Without HEAD: every work-tree file.
    pub fn changed_paths(&self) -> Result<Vec<String>> {
        let split = |s: String| -> Vec<String> {
            s.split('\0')
                .filter(|s| !s.is_empty())
                .map(str::to_owned)
                .collect()
        };
        let mut v = split(self.run(&["ls-files", "-z", "--others"])?);
        if self.head()?.is_some() {
            v.extend(split(self.run(&[
                "diff",
                "--name-only",
                "-z",
                "--no-renames",
                "HEAD",
                "--",
            ])?));
        } else {
            v.extend(split(self.run(&["ls-files", "-z"])?));
        }
        v.sort();
        v.dedup();
        Ok(v)
    }

    /// Commit exactly `update` (staged from the work tree; missing files are
    /// removed) and `remove` on top of HEAD, using a temporary index so the
    /// user's index and untracked files never enter the commit. `check`
    /// sees the complete path list of the resulting tree BEFORE any object
    /// is written and may reject it. Hooks do not run (callers scan the
    /// content themselves). Afterwards the real index is synced for the
    /// committed paths only. Returns the new commit, or None when the tree
    /// is unchanged.
    pub fn commit_paths(
        &self,
        message: &str,
        update: &[String],
        remove: &[String],
        check: &dyn Fn(&[String]) -> Result<()>,
    ) -> Result<Option<String>> {
        let head = self.head()?;
        let tmp_path = self.git_path(&format!(
            "latch-index-{}-{}",
            std::process::id(),
            crate::core::ids::new_id()
        ))?;
        struct Cleanup(PathBuf);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                let _ = std::fs::remove_file(&self.0);
                let mut lock = self.0.clone().into_os_string();
                lock.push(".lock");
                let _ = std::fs::remove_file(lock);
            }
        }
        let _cleanup = Cleanup(tmp_path.clone());
        let env: [(&str, &std::ffi::OsStr); 1] = [("GIT_INDEX_FILE", tmp_path.as_os_str())];
        match &head {
            Some(h) => self.run_env(&["read-tree", h.as_str()], &env, None)?,
            None => self.run_env(&["read-tree", "--empty"], &env, None)?,
        };
        let listed = self.run_env(&["ls-files", "-z"], &env, None)?;
        let mut paths: std::collections::BTreeSet<String> = String::from_utf8_lossy(&listed)
            .split('\0')
            .filter(|s| !s.is_empty())
            .map(str::to_owned)
            .collect();
        let mut adds = vec![];
        let mut dels = vec![];
        for p in update {
            if self.root.join(p).is_file() {
                paths.insert(p.clone());
                adds.push(p.clone());
            } else if paths.remove(p) {
                dels.push(p.clone());
            }
        }
        for p in remove {
            if paths.remove(p) {
                dels.push(p.clone());
            }
        }
        let final_paths: Vec<String> = paths.into_iter().collect();
        check(&final_paths)?;
        let feed = |list: &[String]| {
            let mut v = Vec::new();
            for p in list {
                v.extend_from_slice(p.as_bytes());
                v.push(0);
            }
            v
        };
        if !adds.is_empty() {
            self.run_env(
                &["update-index", "--add", "-z", "--stdin"],
                &env,
                Some(&feed(&adds)),
            )?;
        }
        if !dels.is_empty() {
            self.run_env(
                &["update-index", "--force-remove", "-z", "--stdin"],
                &env,
                Some(&feed(&dels)),
            )?;
        }
        let tree = String::from_utf8_lossy(&self.run_env(&["write-tree"], &env, None)?)
            .trim()
            .to_owned();
        if let Some(h) = &head {
            let old_tree = self.run(&["rev-parse", &format!("{h}^{{tree}}")])?;
            if old_tree.trim() == tree {
                return Ok(None);
            }
        }
        let mut args: Vec<String> = self.identity_args();
        args.extend(["commit-tree".into(), tree.clone()]);
        if let Some(h) = &head {
            args.extend(["-p".into(), h.clone()]);
        }
        args.extend(["-m".into(), message.to_owned()]);
        let argv: Vec<&str> = args.iter().map(String::as_str).collect();
        let commit = String::from_utf8_lossy(&self.run_env(&argv, &[], None)?)
            .trim()
            .to_owned();
        let reflog = format!("latch: {}", message.lines().next().unwrap_or(""));
        match &head {
            Some(h) => self.run(&["update-ref", "-m", &reflog, "HEAD", &commit, h])?,
            None => self.run(&["update-ref", "-m", &reflog, "HEAD", &commit])?,
        };
        // Sync the user's index for the committed paths only.
        let mut touched: Vec<&str> = adds.iter().chain(dels.iter()).map(String::as_str).collect();
        touched.sort_unstable();
        touched.dedup();
        for chunk in touched.chunks(200) {
            let mut a = vec!["reset", "-q", "HEAD", "--"];
            a.extend_from_slice(chunk);
            if let Err(e) = self.run(&a) {
                tracing::warn!("could not sync the git index after a Latch commit: {e}");
            }
        }
        Ok(Some(commit))
    }

    /// Stage paths (additions, modifications and deletions).
    pub fn add(&self, paths: &[&str]) -> Result<()> {
        let mut args = vec!["add", "-A", "--"];
        args.extend_from_slice(paths);
        self.run(&args).map(|_| ())
    }

    pub fn has_staged_changes(&self) -> Result<bool> {
        let out = self.output(&["diff", "--cached", "--quiet"])?;
        match out.status.code() {
            Some(0) => Ok(false),
            Some(1) => Ok(true),
            _ => {
                // No HEAD yet: anything staged counts.
                Ok(!self.run(&["ls-files"])?.trim().is_empty())
            }
        }
    }

    /// Commit staged changes (hooks skipped: Latch is the trusted writer).
    /// Returns the new commit id, or None when nothing was staged.
    pub fn commit(&self, message: &str) -> Result<Option<String>> {
        if !self.has_staged_changes()? {
            return Ok(None);
        }
        let args = ["commit", "--quiet", "--no-verify", "-m", message];
        let out = self.run_with_identity(&args)?;
        if !out.status.success() {
            return Err(git_err(&args, &out));
        }
        self.head()
    }

    /// Tracked files (index).
    pub fn ls_files(&self) -> Result<Vec<String>> {
        let s = self.run(&["ls-files", "-z"])?;
        Ok(s.split('\0')
            .filter(|s| !s.is_empty())
            .map(str::to_owned)
            .collect())
    }

    /// Untracked, non-ignored files.
    pub fn untracked_files(&self) -> Result<Vec<String>> {
        let s = self.run(&["ls-files", "-z", "--others", "--exclude-standard"])?;
        Ok(s.split('\0')
            .filter(|s| !s.is_empty())
            .map(str::to_owned)
            .collect())
    }

    /// Staged additions/modifications (excluding deletions).
    pub fn staged_files(&self) -> Result<Vec<String>> {
        let args: &[&str] = if self.head()?.is_some() {
            &[
                "diff",
                "--cached",
                "--name-only",
                "-z",
                "--diff-filter=ACMRT",
            ]
        } else {
            &["ls-files", "-z"]
        };
        let s = self.run(args)?;
        Ok(s.split('\0')
            .filter(|s| !s.is_empty())
            .map(str::to_owned)
            .collect())
    }

    /// Paths with unmerged index entries.
    pub fn unmerged_files(&self) -> Result<Vec<String>> {
        let s = self.run(&["diff", "--name-only", "-z", "--diff-filter=U"])?;
        let mut v: Vec<String> = s
            .split('\0')
            .filter(|s| !s.is_empty())
            .map(str::to_owned)
            .collect();
        v.sort();
        v.dedup();
        Ok(v)
    }
}
