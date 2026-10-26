//! Commit history and repository object reads.

use super::{CommitInfo, Error, FileChange, Git, LogItem, Result, normalize_git_time};

const SEP: char = '\u{1f}';

impl Git {
    /// Contents of many objects (`<rev>:<path>` specs) via one
    /// `git cat-file --batch`. `None` for missing objects.
    pub fn read_blobs(&self, specs: &[String]) -> Result<Vec<Option<Vec<u8>>>> {
        if specs.is_empty() {
            return Ok(vec![]);
        }
        let mut input = Vec::new();
        for s in specs {
            if s.contains('\n') {
                return Err(Error::invalid("object spec contains a newline"));
            }
            input.extend_from_slice(s.as_bytes());
            input.push(b'\n');
        }
        let out = self.run_env(&["cat-file", "--batch"], &[], Some(&input))?;
        let mut res = Vec::with_capacity(specs.len());
        let mut pos = 0usize;
        for _ in specs {
            let nl = out[pos..]
                .iter()
                .position(|b| *b == b'\n')
                .ok_or_else(|| Error::Git("truncated cat-file output".into()))?;
            let header = String::from_utf8_lossy(&out[pos..pos + nl]).into_owned();
            pos += nl + 1;
            let mut parts = header.split(' ');
            let (_oid, kind, size) = (parts.next(), parts.next(), parts.next());
            match (kind, size.and_then(|s| s.parse::<usize>().ok())) {
                (Some("blob"), Some(n)) if pos + n < out.len() + 1 => {
                    res.push(Some(out[pos..pos + n].to_vec()));
                    pos += n + 1;
                }
                (Some(_), Some(n)) if !header.ends_with(" missing") => {
                    // Not a blob (tree/commit): skip its content.
                    pos += n + 1;
                    res.push(None);
                }
                _ => res.push(None),
            }
        }
        Ok(res)
    }

    /// Files tracked at `rev` (recursive, all paths).
    pub fn tree_paths(&self, rev: &str) -> Result<Vec<String>> {
        let s = self.run(&["ls-tree", "-r", "-z", "--name-only", rev])?;
        Ok(s.split('\0')
            .filter(|s| !s.is_empty())
            .map(str::to_owned)
            .collect())
    }

    fn parse_log(s: &str, with_changes: bool) -> Vec<LogItem> {
        let mut items: Vec<LogItem> = vec![];
        for line in s.lines() {
            if let Some(rest) = line.strip_prefix("\u{1e}") {
                let mut f = rest.split(SEP);
                let commit = f.next().unwrap_or_default().to_owned();
                let at = f.next().unwrap_or_default().to_owned();
                let author = f.next().unwrap_or_default().to_owned();
                let message = f.next().unwrap_or_default().to_owned();
                items.push(LogItem {
                    info: CommitInfo {
                        commit,
                        at: normalize_git_time(&at),
                        author,
                        message,
                    },
                    changes: vec![],
                });
            } else if with_changes && !line.trim().is_empty() {
                let mut parts = line.split('\t');
                let status = parts.next().and_then(|s| s.chars().next()).unwrap_or('M');
                if let (Some(path), Some(item)) = (parts.next_back(), items.last_mut()) {
                    item.changes.push(FileChange {
                        status,
                        path: path.to_owned(),
                    });
                }
            }
        }
        items
    }

    const LOG_FORMAT: &'static str = "--format=%x1e%H%x1f%aI%x1f%an%x1f%s";

    /// Recent commits, newest first.
    pub fn log(&self, max: usize) -> Result<Vec<CommitInfo>> {
        if self.head()?.is_none() {
            return Ok(vec![]);
        }
        let n = format!("-n{max}");
        let s = self.run(&["log", Self::LOG_FORMAT, &n])?;
        Ok(Self::parse_log(&s, false)
            .into_iter()
            .map(|i| i.info)
            .collect())
    }

    /// Commits touching `pathspecs` with per-file status, newest first.
    pub fn log_paths(&self, pathspecs: &[&str], extra: &[&str]) -> Result<Vec<LogItem>> {
        if self.head()?.is_none() {
            return Ok(vec![]);
        }
        let mut args = vec!["log", Self::LOG_FORMAT, "--name-status", "--no-renames"];
        args.extend_from_slice(extra);
        args.push("--");
        args.extend_from_slice(pathspecs);
        let s = self.run(&args)?;
        Ok(Self::parse_log(&s, true))
    }

    /// Contents of `path` at `rev` (`rev` may be `:2` style index stages
    /// when passed as `:2:path` via `spec`). None when absent.
    pub fn show(&self, rev: &str, path: &str) -> Result<Option<Vec<u8>>> {
        self.show_spec(&format!("{rev}:{path}"))
    }

    pub fn show_spec(&self, spec: &str) -> Result<Option<Vec<u8>>> {
        let out = self.output(&["show", spec])?;
        if out.status.success() {
            Ok(Some(out.stdout))
        } else {
            Ok(None)
        }
    }

    /// Files tracked at `rev` under `prefix`.
    pub fn ls_tree(&self, rev: &str, prefix: &str) -> Result<Vec<String>> {
        let s = self.run(&["ls-tree", "-r", "--name-only", rev, "--", prefix])?;
        Ok(s.lines().map(str::to_owned).collect())
    }
}
