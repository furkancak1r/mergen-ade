use std::path::{Path, PathBuf};

/// Information about a single git worktree entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitWorktreeInfo {
    /// Absolute or relative path to the worktree directory.
    pub path: PathBuf,
    /// Current HEAD commit SHA, if available.
    pub head: Option<String>,
    /// Full branch ref (e.g. `refs/heads/main`) when not detached.
    pub branch: Option<String>,
    /// True when the worktree is in detached HEAD state.
    pub detached: bool,
    /// True when the worktree is locked.
    pub locked: bool,
    /// Optional lock reason text.
    pub locked_reason: Option<String>,
    /// True when the worktree is marked prunable.
    pub prunable: bool,
    /// Optional prune reason text.
    pub prunable_reason: Option<String>,
}

impl GitWorktreeInfo {
    /// Extract the short branch name from a full ref, if present.
    pub fn branch_name(&self) -> Option<&str> {
        self.branch
            .as_deref()
            .and_then(|r| r.strip_prefix("refs/heads/"))
    }

    /// Display label: branch name when available, otherwise a shortened path.
    pub fn display_label(&self) -> String {
        if let Some(name) = self.branch_name() {
            name.to_owned()
        } else if self.detached {
            let short = self
                .head
                .as_deref()
                .map(|h| &h[h.len().saturating_sub(8)..])
                .unwrap_or("detached");
            format!("detached@{short}")
        } else {
            self.path
                .file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| self.path.display().to_string())
        }
    }
}

/// Parse `git worktree list --porcelain` output into worktree records.
pub fn parse_git_worktree_list_porcelain(output: &str) -> Vec<GitWorktreeInfo> {
    let mut results = Vec::new();
    let mut current: Option<GitWorktreeInfo> = None;

    for line in output.lines() {
        if line.is_empty() {
            if let Some(info) = current.take() {
                results.push(info);
            }
            continue;
        }

        if let Some(path_str) = line.strip_prefix("worktree ") {
            if let Some(info) = current.take() {
                results.push(info);
            }
            current = Some(GitWorktreeInfo {
                path: PathBuf::from(path_str.trim()),
                head: None,
                branch: None,
                detached: false,
                locked: false,
                locked_reason: None,
                prunable: false,
                prunable_reason: None,
            });
        } else if let Some(head) = line.strip_prefix("HEAD ") {
            if let Some(ref mut info) = current {
                info.head = Some(head.trim().to_owned());
            }
        } else if let Some(branch) = line.strip_prefix("branch ") {
            if let Some(ref mut info) = current {
                info.branch = Some(branch.trim().to_owned());
            }
        } else if line.trim() == "detached" {
            if let Some(ref mut info) = current {
                info.detached = true;
            }
        } else if let Some(reason) = line.strip_prefix("locked ") {
            if let Some(ref mut info) = current {
                info.locked = true;
                info.locked_reason = Some(reason.trim().to_owned());
            }
        } else if line.trim() == "locked" {
            if let Some(ref mut info) = current {
                info.locked = true;
            }
        } else if let Some(reason) = line.strip_prefix("prunable ") {
            if let Some(ref mut info) = current {
                info.prunable = true;
                info.prunable_reason = Some(reason.trim().to_owned());
            }
        } else if line.trim() == "prunable" {
            if let Some(ref mut info) = current {
                info.prunable = true;
            }
        }
    }

    if let Some(info) = current.take() {
        results.push(info);
    }

    results
}

/// Run `git worktree list --porcelain` in the given directory and parse the output.
pub fn discover_worktrees(project_path: &Path) -> std::io::Result<Vec<GitWorktreeInfo>> {
    let mut command = std::process::Command::new("git");
    command.arg("-C").arg(project_path);
    command.args(["worktree", "list", "--porcelain"]);
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000); // CREATE_NO_WINDOW
    }

    let output = command.output()?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            format!("git worktree list failed: {stderr}"),
        ));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut results = parse_git_worktree_list_porcelain(&stdout);
    for wt in &mut results {
        if let Some(s) = wt.path.to_str() {
            let r = crate::mojibake::repair_mojibake(s);
            if r != s && std::path::Path::new(&r).exists() {
                wt.path = std::path::PathBuf::from(r);
            }
        }
    }
    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_two_worktrees_with_branches() {
        let input = r#"worktree /repo/main
HEAD f6432285388293f772c5e0d08383ca449df613ba
branch refs/heads/main

worktree /repo/worktrees/feature-x
HEAD 2a9316b3d381a6715ac7f56714eea590b9e8e9f9
branch refs/heads/feature-x
"#;
        let list = parse_git_worktree_list_porcelain(input);
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].path, PathBuf::from("/repo/main"));
        assert_eq!(
            list[0].head,
            Some("f6432285388293f772c5e0d08383ca449df613ba".to_owned())
        );
        assert_eq!(list[0].branch, Some("refs/heads/main".to_owned()));
        assert!(!list[0].detached);
        assert_eq!(list[1].path, PathBuf::from("/repo/worktrees/feature-x"));
        assert_eq!(list[1].branch_name(), Some("feature-x"));
    }

    #[test]
    fn parse_detached_worktree() {
        let input = r#"worktree /repo/detached-wt
HEAD abcdef1234567890abcdef1234567890abcdef12
detached
"#;
        let list = parse_git_worktree_list_porcelain(input);
        assert_eq!(list.len(), 1);
        assert!(list[0].detached);
        assert_eq!(list[0].branch, None);
        assert_eq!(list[0].display_label(), "detached@abcdef12");
    }

    #[test]
    fn parse_locked_worktree_with_reason() {
        let input = r#"worktree /repo/locked-wt
HEAD abcdef1234567890abcdef1234567890abcdef12
branch refs/heads/old-feature
locked running build
"#;
        let list = parse_git_worktree_list_porcelain(input);
        assert_eq!(list.len(), 1);
        assert!(list[0].locked);
        assert_eq!(list[0].locked_reason, Some("running build".to_owned()));
    }

    #[test]
    fn parse_prunable_worktree() {
        let input = r#"worktree /repo/stale-wt
HEAD abcdef1234567890abcdef1234567890abcdef12
branch refs/heads/stale
prunable gitdir file points to non-existent location
"#;
        let list = parse_git_worktree_list_porcelain(input);
        assert_eq!(list.len(), 1);
        assert!(list[0].prunable);
        assert_eq!(
            list[0].prunable_reason,
            Some("gitdir file points to non-existent location".to_owned())
        );
    }

    #[test]
    fn parse_empty_git_output_returns_empty() {
        let list = parse_git_worktree_list_porcelain("");
        assert!(list.is_empty());
    }

    #[test]
    fn branch_name_strips_refs_heads() {
        let info = GitWorktreeInfo {
            path: PathBuf::from("/a"),
            head: None,
            branch: Some("refs/heads/feature-x".to_owned()),
            detached: false,
            locked: false,
            locked_reason: None,
            prunable: false,
            prunable_reason: None,
        };
        assert_eq!(info.branch_name(), Some("feature-x"));
    }

    #[test]
    fn display_label_uses_branch_when_available() {
        let info = GitWorktreeInfo {
            path: PathBuf::from("/repo/wt"),
            head: None,
            branch: Some("refs/heads/feature-foo".to_owned()),
            detached: false,
            locked: false,
            locked_reason: None,
            prunable: false,
            prunable_reason: None,
        };
        assert_eq!(info.display_label(), "feature-foo");
    }
}
