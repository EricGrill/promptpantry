use assert_cmd::Command;
use std::path::Path;

/// `pp` pointed at a library dir, isolated from this machine's git config,
/// with a no-op editor and deterministic commit identity.
pub fn pp(dir: &Path) -> Command {
    let mut cmd = Command::cargo_bin("pp").unwrap();
    cmd.arg("--dir")
        .arg(dir)
        .env("EDITOR", "true")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_SYSTEM", "/dev/null")
        .env("GIT_AUTHOR_NAME", "pp-test")
        .env("GIT_AUTHOR_EMAIL", "pp@test")
        .env("GIT_COMMITTER_NAME", "pp-test")
        .env("GIT_COMMITTER_EMAIL", "pp@test");
    cmd
}

#[test]
fn help_lists_all_subcommands() {
    let assert = pp(Path::new(".")).arg("--help").assert().success();
    let output = String::from_utf8_lossy(&assert.get_output().stdout).to_string();
    for sub in ["init", "list", "copy", "new", "sync"] {
        assert!(output.contains(sub), "missing subcommand {sub} in --help");
    }
}

#[test]
fn init_creates_repo_readme_and_example_card() {
    let tmp = tempfile::TempDir::new().unwrap();
    let lib = tmp.path().join("pantry");
    pp(&lib).arg("init").assert().success();
    assert!(lib.join(".git").is_dir());
    assert!(lib.join("README.md").is_file());
    assert!(lib.join("bug-report-template.md").is_file());
    // idempotent: re-run succeeds and doesn't clobber existing files
    std::fs::write(lib.join("README.md"), "customized").unwrap();
    pp(&lib).arg("init").assert().success();
    assert_eq!(
        std::fs::read_to_string(lib.join("README.md")).unwrap(),
        "customized"
    );
}

#[test]
fn list_outputs_tab_separated_and_filters() {
    let tmp = tempfile::TempDir::new().unwrap();
    let lib = tmp.path().join("pantry");
    pp(&lib).arg("init").assert().success();
    pp(&lib)
        .arg("list")
        .assert()
        .success()
        .stdout(predicates::str::contains(
            "bug-report-template\tBug Report Template\tbugs,templates",
        ));
    pp(&lib)
        .args(["list", "#nosuchtag"])
        .assert()
        .success()
        .stdout(predicates::str::is_empty());
}

fn seeded_lib(tmp: &tempfile::TempDir) -> std::path::PathBuf {
    let lib = tmp.path().join("pantry");
    pp(&lib).arg("init").assert().success();
    lib
}

#[test]
fn copy_stdout_renders_variables() {
    let tmp = tempfile::TempDir::new().unwrap();
    let lib = seeded_lib(&tmp);
    pp(&lib)
        .args(["copy", "bug report", "--stdout", "--var", "ticket=ABC-123"])
        .assert()
        .success()
        .stdout(predicates::str::contains("Ticket: ABC-123"));
}

#[test]
fn copy_missing_variable_errors_with_names() {
    let tmp = tempfile::TempDir::new().unwrap();
    let lib = seeded_lib(&tmp);
    pp(&lib)
        .args(["copy", "--id", "bug-report-template", "--stdout"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("missing variables: ticket"));
}

#[test]
fn copy_raw_keeps_placeholders() {
    let tmp = tempfile::TempDir::new().unwrap();
    let lib = seeded_lib(&tmp);
    pp(&lib)
        .args(["copy", "--id", "bug-report-template", "--raw", "--stdout"])
        .assert()
        .success()
        .stdout(predicates::str::contains("{{ticket}}"));
}

#[test]
fn copy_no_match_errors_with_hint() {
    let tmp = tempfile::TempDir::new().unwrap();
    let lib = seeded_lib(&tmp);
    pp(&lib)
        .args(["copy", "zzzzqq", "--stdout"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("no card matches"));
}
