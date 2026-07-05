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
    for sub in ["init", "list", "show", "copy", "new", "sync"] {
        assert!(output.contains(sub), "missing subcommand {sub} in --help");
    }
}

#[test]
fn help_shows_examples_for_common_workflows() {
    pp(Path::new("."))
        .arg("--help")
        .assert()
        .success()
        .stdout(predicates::str::contains("Examples:"))
        .stdout(predicates::str::contains("pp init"))
        .stdout(predicates::str::contains("pp list"))
        .stdout(predicates::str::contains("pp show bug report"))
        .stdout(predicates::str::contains(
            "pp show --id bug-report-template --raw",
        ))
        .stdout(predicates::str::contains(
            "pp copy bug report --var ticket=ABC-123",
        ));
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

#[test]
fn list_accepts_multi_word_query_without_shell_quotes() {
    let tmp = tempfile::TempDir::new().unwrap();
    let lib = tmp.path().join("pantry");
    pp(&lib).arg("init").assert().success();
    pp(&lib)
        .args(["list", "bug", "report"])
        .assert()
        .success()
        .stdout(predicates::str::contains(
            "bug-report-template\tBug Report Template",
        ));
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
fn copy_accepts_multi_word_query_without_shell_quotes() {
    let tmp = tempfile::TempDir::new().unwrap();
    let lib = seeded_lib(&tmp);
    pp(&lib)
        .args([
            "copy",
            "bug",
            "report",
            "--stdout",
            "--var",
            "ticket=ABC-123",
        ])
        .assert()
        .success()
        .stdout(predicates::str::contains("Ticket: ABC-123"));
}

#[test]
fn show_raw_prints_prompt_body_by_id() {
    let tmp = tempfile::TempDir::new().unwrap();
    let lib = seeded_lib(&tmp);
    pp(&lib)
        .args(["show", "--id", "bug-report-template", "--raw"])
        .assert()
        .success()
        .stdout(predicates::str::contains("Repo: {{repo}}"))
        .stdout(predicates::str::contains("Ticket: {{ticket}}"));
}

#[test]
fn show_accepts_multi_word_query_without_shell_quotes() {
    let tmp = tempfile::TempDir::new().unwrap();
    let lib = seeded_lib(&tmp);
    pp(&lib)
        .args(["show", "bug", "report", "--raw"])
        .assert()
        .success()
        .stdout(predicates::str::contains("Ticket: {{ticket}}"));
}

#[test]
fn show_without_vars_prints_raw_prompt_body() {
    let tmp = tempfile::TempDir::new().unwrap();
    let lib = seeded_lib(&tmp);
    pp(&lib)
        .args(["show", "bug", "report"])
        .assert()
        .success()
        .stdout(predicates::str::contains("Ticket: {{ticket}}"));
}

#[test]
fn view_alias_renders_variables() {
    let tmp = tempfile::TempDir::new().unwrap();
    let lib = seeded_lib(&tmp);
    pp(&lib)
        .args(["view", "bug report", "--var", "ticket=ABC-123"])
        .assert()
        .success()
        .stdout(predicates::str::contains("Ticket: ABC-123"));
}

#[test]
fn view_alias_accepts_multi_word_query_without_shell_quotes() {
    let tmp = tempfile::TempDir::new().unwrap();
    let lib = seeded_lib(&tmp);
    pp(&lib)
        .args(["view", "bug", "report", "--var", "ticket=ABC-123"])
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

#[test]
fn new_creates_file_in_subdir_and_commits() {
    let tmp = tempfile::TempDir::new().unwrap();
    let lib = seeded_lib(&tmp);
    pp(&lib)
        .args(["new", "evals/Rubric Writer", "--tags", "evals,writing"])
        .assert()
        .success();
    assert!(lib.join("evals/rubric-writer.md").is_file());
    let log = std::process::Command::new("git")
        .args(["log", "--oneline"])
        .current_dir(&lib)
        .output()
        .unwrap();
    assert!(String::from_utf8_lossy(&log.stdout).contains("pp: add evals/rubric-writer"));
}

#[test]
fn new_collision_errors() {
    let tmp = tempfile::TempDir::new().unwrap();
    let lib = seeded_lib(&tmp);
    pp(&lib).args(["new", "Retro"]).assert().success();
    pp(&lib)
        .args(["new", "Retro"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("already exists"));
}

#[test]
fn sync_commits_external_edits_and_pushes_to_a_remote() {
    let tmp = tempfile::TempDir::new().unwrap();
    let lib = seeded_lib(&tmp);
    let git = |dir: &std::path::Path, args: &[&str]| {
        let out = std::process::Command::new("git")
            .args(args)
            .current_dir(dir)
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_SYSTEM", "/dev/null")
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8_lossy(&out.stdout).to_string()
    };
    // wire the library to a local bare repo as `origin` with an upstream branch
    let remote = tmp.path().join("remote.git");
    git(tmp.path(), &["init", "--bare", remote.to_str().unwrap()]);
    git(&lib, &["remote", "add", "origin", remote.to_str().unwrap()]);
    git(&lib, &["push", "-u", "origin", "HEAD"]);
    // an edit made outside pp, never committed through the app
    std::fs::write(lib.join("external.md"), "---\ntitle: External\n---\nhi\n").unwrap();
    pp(&lib).arg("sync").assert().success();
    let log = git(&remote, &["log", "--oneline", "--all"]);
    assert!(log.contains("pp: sync external edits"));
}

#[test]
fn copy_var_without_equals_errors() {
    let tmp = tempfile::TempDir::new().unwrap();
    let lib = seeded_lib(&tmp);
    pp(&lib)
        .args([
            "copy",
            "--id",
            "bug-report-template",
            "--stdout",
            "--var",
            "novalue",
        ])
        .assert()
        .failure()
        .stderr(predicates::str::contains("--var must be KEY=VALUE"));
}

#[test]
fn missing_library_suggests_init() {
    let tmp = tempfile::TempDir::new().unwrap();
    let lib = tmp.path().join("does-not-exist");
    pp(&lib)
        .arg("list")
        .assert()
        .failure()
        .stderr(predicates::str::contains("pp init"));
}

#[test]
fn copy_stdout_renders_builtins() {
    let tmp = tempfile::TempDir::new().unwrap();
    let lib = seeded_lib(&tmp);
    std::fs::write(
        lib.join("dated.md"),
        "---\ntitle: Dated\n---\nD: {{date}}\n",
    )
    .unwrap();
    let expected = prompt_pantry::core::context::builtin_values(&std::env::current_dir().unwrap())
        ["date"]
        .clone();
    pp(&lib)
        .args(["copy", "--id", "dated", "--stdout"])
        .assert()
        .success()
        .stdout(predicates::str::contains(format!("D: {expected}")));
}
