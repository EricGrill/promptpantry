use std::collections::HashMap;
use std::path::Path;
use std::process::Command;

fn git_out(dir: &Path, args: &[&str]) -> Option<String> {
    let out = Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    (!s.is_empty()).then_some(s)
}

/// Builtin {{variable}} values detected from `cwd` (where pp was launched).
/// Keys are always present; values are empty strings when not applicable.
pub fn builtin_values(cwd: &Path) -> HashMap<String, String> {
    let repo = git_out(cwd, &["rev-parse", "--show-toplevel"])
        .and_then(|p| {
            Path::new(&p)
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
        })
        .unwrap_or_default();
    // `branch --show-current` works even on an unborn branch (fresh init)
    let branch = git_out(cwd, &["branch", "--show-current"]).unwrap_or_default();
    HashMap::from([
        ("repo".to_string(), repo),
        ("branch".to_string(), branch),
        ("cwd".to_string(), cwd.to_string_lossy().into_owned()),
        (
            "date".to_string(),
            chrono::Local::now().format("%Y-%m-%d").to_string(),
        ),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn git(dir: &Path, args: &[&str]) {
        assert!(Command::new("git")
            .args(args)
            .current_dir(dir)
            .status()
            .unwrap()
            .success());
    }

    #[test]
    fn detects_repo_branch_cwd_date_inside_a_repo() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = tmp.path().join("myrepo");
        std::fs::create_dir(&repo).unwrap();
        git(&repo, &["init", "-b", "main"]);
        let v = builtin_values(&repo);
        assert_eq!(v["repo"], "myrepo");
        assert_eq!(v["branch"], "main");
        assert_eq!(v["cwd"], repo.to_string_lossy());
        assert_eq!(v["date"].len(), 10); // YYYY-MM-DD
    }

    #[test]
    fn empty_repo_and_branch_outside_a_repo() {
        let tmp = tempfile::tempdir().unwrap();
        let v = builtin_values(tmp.path());
        assert_eq!(v["repo"], "");
        assert_eq!(v["branch"], "");
    }
}
