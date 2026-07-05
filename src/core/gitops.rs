use anyhow::{bail, Context, Result};
use std::path::Path;
use std::process::Command;

fn git(dir: &Path, args: &[&str]) -> Result<String> {
    let out = Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .context("failed to run git — is it installed?")?;
    if !out.status.success() {
        bail!("git {} failed: {}", args.join(" "), String::from_utf8_lossy(&out.stderr).trim());
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

pub fn is_repo(dir: &Path) -> bool {
    dir.join(".git").exists()
}

pub fn init_repo(dir: &Path) -> Result<()> {
    git(dir, &["init"]).map(|_| ())
}

/// Stage one path (handles adds, edits and deletions) and commit just it.
pub fn commit_path(dir: &Path, rel: &str, message: &str) -> Result<()> {
    git(dir, &["add", "-A", "--", rel])?;
    git(dir, &["commit", "-m", message, "--", rel]).map(|_| ())
}

/// Stage and commit everything (used by `pp init` and `pp sync`).
pub fn commit_all(dir: &Path, message: &str) -> Result<()> {
    git(dir, &["add", "-A"])?;
    git(dir, &["commit", "-m", message]).map(|_| ())
}

/// True when the path has uncommitted changes (including untracked).
pub fn has_changes(dir: &Path, rel: &str) -> Result<bool> {
    Ok(!git(dir, &["status", "--porcelain", "--", rel])?.trim().is_empty())
}

/// Commit pending external edits, then pull --rebase and push.
pub fn sync(dir: &Path) -> Result<String> {
    if has_changes(dir, ".")? {
        git(dir, &["add", "-A"])?;
        git(dir, &["commit", "-m", "pp: sync external edits"])?;
    }
    let pull = git(dir, &["pull", "--rebase"])?;
    let push = git(dir, &["push"])?;
    Ok(format!("{}\n{}", pull.trim(), push.trim()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    /// Throwaway repo with identity + signing configured locally, so commits
    /// succeed regardless of this machine's global git config.
    fn repo() -> TempDir {
        let tmp = TempDir::new().unwrap();
        init_repo(tmp.path()).unwrap();
        for cfg in [["user.email", "t@t"], ["user.name", "t"], ["commit.gpgsign", "false"]] {
            git(tmp.path(), &["config", cfg[0], cfg[1]]).unwrap();
        }
        tmp
    }

    fn log_count(dir: &Path) -> usize {
        git(dir, &["log", "--oneline"]).unwrap().lines().count()
    }

    #[test]
    fn commit_path_commits_adds_and_deletions() {
        let tmp = repo();
        fs::write(tmp.path().join("a.md"), "hi").unwrap();
        commit_path(tmp.path(), "a.md", "pp: add a").unwrap();
        assert_eq!(log_count(tmp.path()), 1);
        fs::remove_file(tmp.path().join("a.md")).unwrap();
        commit_path(tmp.path(), "a.md", "pp: delete a").unwrap();
        assert_eq!(log_count(tmp.path()), 2);
    }

    #[test]
    fn is_repo_detects() {
        let tmp = TempDir::new().unwrap();
        assert!(!is_repo(tmp.path()));
        init_repo(tmp.path()).unwrap();
        assert!(is_repo(tmp.path()));
    }

    #[test]
    fn commit_fails_cleanly_outside_a_repo() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("a.md"), "hi").unwrap();
        assert!(commit_path(tmp.path(), "a.md", "m").is_err());
    }

    #[test]
    fn has_changes_tracks_dirty_state() {
        let tmp = repo();
        fs::write(tmp.path().join("a.md"), "hi").unwrap();
        assert!(has_changes(tmp.path(), "a.md").unwrap());
        commit_path(tmp.path(), "a.md", "m").unwrap();
        assert!(!has_changes(tmp.path(), "a.md").unwrap());
    }
}
