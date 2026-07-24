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
        bail!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

fn git_success(dir: &Path, args: &[&str]) -> Result<bool> {
    Ok(Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .context("failed to run git — is it installed?")?
        .status
        .success())
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
    Ok(!git(dir, &["status", "--porcelain", "--", rel])?
        .trim()
        .is_empty())
}

/// Commit pending external edits, then pull --rebase and push.
pub fn sync(dir: &Path) -> Result<String> {
    if has_changes(dir, ".")? {
        git(dir, &["add", "-A"])?;
        git(dir, &["commit", "-m", "pp: sync external edits"])?;
    }
    if git(dir, &["remote"])?.trim().is_empty() {
        bail!(
            "No remote repository configured. Run `git remote add origin <url>` or use a library with a remote."
        );
    }
    if !git_success(
        dir,
        &["rev-parse", "--abbrev-ref", "--symbolic-full-name", "@{u}"],
    )? {
        let branch = git(dir, &["branch", "--show-current"])?.trim().to_string();
        let branch = if branch.is_empty() { "HEAD" } else { &branch };
        bail!(
            "No upstream branch configured. Run `git push -u origin {branch}` once, then retry `pp sync`."
        );
    }
    let pull = match git(dir, &["pull", "--rebase"]) {
        Ok(out) => out,
        Err(err) => {
            // A conflicting `pull --rebase` stops mid-rebase, leaving conflict
            // markers in the working tree and the repo stuck in a state `pp`
            // offers no way out of. Roll back so the library is usable again.
            //
            // If the pull failed for some other reason (no network, bad
            // credentials) no rebase is in progress, `rebase --abort` fails
            // harmlessly, and the original error is surfaced unchanged.
            if git_success(dir, &["rebase", "--abort"]).unwrap_or(false) {
                bail!(
                    "Sync hit a conflict with the remote and was rolled back — \
                     your library is back to the state it was in before the rebase, \
                     with your local commits intact.\n\n\
                     Resolve it yourself in {} with `git pull --rebase`, then run \
                     `pp sync` again.\n\n\
                     git said: {err}",
                    dir.display()
                );
            }
            return Err(err);
        }
    };
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
        for cfg in [
            ["user.email", "t@t"],
            ["user.name", "t"],
            ["commit.gpgsign", "false"],
        ] {
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

    #[test]
    fn sync_without_remote_explains_how_to_configure_one() {
        let tmp = repo();
        let err = sync(tmp.path()).unwrap_err();
        assert!(
            err.to_string().contains("No remote repository configured"),
            "{err:#}"
        );
    }

    /// Configure identity + signing locally so commits work regardless of
    /// this machine's global git config.
    fn configure(dir: &Path) {
        for cfg in [
            ["user.email", "t@t"],
            ["user.name", "t"],
            ["commit.gpgsign", "false"],
        ] {
            git(dir, &["config", cfg[0], cfg[1]]).unwrap();
        }
    }

    /// Two clones of one bare remote, each with a conflicting commit on the
    /// same file, with the first clone's version already pushed — exactly the
    /// reproduction in CHA-2543.
    fn conflicting_clones() -> (TempDir, TempDir, TempDir) {
        let remote = TempDir::new().unwrap();
        git(remote.path(), &["init", "--bare"]).unwrap();

        let first = TempDir::new().unwrap();
        git(
            first.path(),
            &["clone", &remote.path().to_string_lossy(), "."],
        )
        .unwrap();
        configure(first.path());
        fs::write(first.path().join("p.md"), "base\n").unwrap();
        commit_all(first.path(), "base").unwrap();
        git(first.path(), &["push", "-u", "origin", "HEAD"]).unwrap();

        let second = TempDir::new().unwrap();
        git(
            second.path(),
            &["clone", &remote.path().to_string_lossy(), "."],
        )
        .unwrap();
        configure(second.path());

        // Diverge: same file, different content, first clone publishes.
        fs::write(first.path().join("p.md"), "from first\n").unwrap();
        commit_all(first.path(), "first edit").unwrap();
        git(first.path(), &["push"]).unwrap();

        fs::write(second.path().join("p.md"), "from second\n").unwrap();

        (remote, first, second)
    }

    #[test]
    fn sync_conflict_rolls_back_instead_of_leaving_repo_mid_rebase() {
        let (_remote, _first, second) = conflicting_clones();

        let err = sync(second.path()).unwrap_err();
        assert!(
            err.to_string().contains("rolled back"),
            "expected a rollback message, got: {err:#}"
        );

        // The repo must be usable again: no rebase in progress, no conflict
        // markers left behind, and the local commit preserved.
        assert!(
            !second.path().join(".git/rebase-merge").exists()
                && !second.path().join(".git/rebase-apply").exists(),
            "repo left mid-rebase"
        );
        let content = fs::read_to_string(second.path().join("p.md")).unwrap();
        assert!(
            !content.contains("<<<<<<<"),
            "conflict markers left in working tree: {content}"
        );
        assert_eq!(content, "from second\n");
        assert!(!has_changes(second.path(), ".").unwrap(), "left dirty");
    }

    #[test]
    fn sync_succeeds_when_there_is_no_conflict() {
        let remote = TempDir::new().unwrap();
        git(remote.path(), &["init", "--bare"]).unwrap();

        let clone = TempDir::new().unwrap();
        git(
            clone.path(),
            &["clone", &remote.path().to_string_lossy(), "."],
        )
        .unwrap();
        configure(clone.path());
        fs::write(clone.path().join("p.md"), "base\n").unwrap();
        commit_all(clone.path(), "base").unwrap();
        git(clone.path(), &["push", "-u", "origin", "HEAD"]).unwrap();

        fs::write(clone.path().join("q.md"), "new card\n").unwrap();
        sync(clone.path()).expect("clean sync should succeed");
        assert!(!has_changes(clone.path(), ".").unwrap());
    }
}
