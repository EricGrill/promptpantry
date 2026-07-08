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
        .stdout(predicates::str::contains("pp copy bug report"));
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
        .args(["copy", "bug report", "--stdout"])
        .assert()
        .success()
        .stdout(predicates::str::contains("Repo: promptpantry"))
        .stdout(predicates::str::contains("Date: "));
}

#[test]
fn copy_accepts_multi_word_query_without_shell_quotes() {
    let tmp = tempfile::TempDir::new().unwrap();
    let lib = seeded_lib(&tmp);
    pp(&lib)
        .args(["copy", "bug", "report", "--stdout"])
        .assert()
        .success()
        .stdout(predicates::str::contains("Repo: promptpantry"))
        .stdout(predicates::str::contains("Date: "));
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
        .stdout(predicates::str::contains("Branch: {{branch}}"))
        .stdout(predicates::str::contains("Date: {{date}}"));
}

#[test]
fn show_accepts_multi_word_query_without_shell_quotes() {
    let tmp = tempfile::TempDir::new().unwrap();
    let lib = seeded_lib(&tmp);
    pp(&lib)
        .args(["show", "bug", "report", "--raw"])
        .assert()
        .success()
        .stdout(predicates::str::contains("Branch: {{branch}}"))
        .stdout(predicates::str::contains("Date: {{date}}"));
}

#[test]
fn show_without_vars_prints_raw_prompt_body() {
    let tmp = tempfile::TempDir::new().unwrap();
    let lib = seeded_lib(&tmp);
    pp(&lib)
        .args(["show", "bug", "report"])
        .assert()
        .success()
        .stdout(predicates::str::contains("Branch: {{branch}}"))
        .stdout(predicates::str::contains("Date: {{date}}"));
}

#[test]
fn view_alias_renders_variables() {
    let tmp = tempfile::TempDir::new().unwrap();
    let lib = seeded_lib(&tmp);
    pp(&lib)
        .args(["view", "bug report", "--var", "unused=value"])
        .assert()
        .success()
        .stdout(predicates::str::contains("Repo: promptpantry"))
        .stdout(predicates::str::contains("Date: "));
}

#[test]
fn view_alias_accepts_multi_word_query_without_shell_quotes() {
    let tmp = tempfile::TempDir::new().unwrap();
    let lib = seeded_lib(&tmp);
    pp(&lib)
        .args(["view", "bug", "report", "--var", "unused=value"])
        .assert()
        .success()
        .stdout(predicates::str::contains("Repo: promptpantry"))
        .stdout(predicates::str::contains("Date: "));
}

#[test]
fn copy_missing_variable_errors_with_names() {
    let tmp = tempfile::TempDir::new().unwrap();
    let lib = seeded_lib(&tmp);
    std::fs::write(
        lib.join("needs-ticket.md"),
        "---\ntitle: Needs Ticket\ntags: [tests]\n---\nTicket: {{ticket}}\n",
    )
    .unwrap();
    pp(&lib)
        .args(["copy", "--id", "needs-ticket", "--stdout"])
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
        .stdout(predicates::str::contains("{{branch}}"))
        .stdout(predicates::str::contains("{{date}}"));
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

#[test]
fn library_add_list_and_search_manage_catalog_entries() {
    let tmp = tempfile::TempDir::new().unwrap();
    let lib = seeded_lib(&tmp);
    let source_dir = tmp.path().join("sources/writer");
    std::fs::create_dir_all(&source_dir).unwrap();
    let source = source_dir.join("SKILL.md");
    std::fs::write(&source, "---\nname: writer\n---\nWrite prompts\n").unwrap();

    pp(&lib)
        .args([
            "library",
            "add",
            "skill",
            "writer",
            "--description",
            "Writes reusable prompts",
            "--source",
            source.to_str().unwrap(),
        ])
        .assert()
        .success();

    let catalog = std::fs::read_to_string(lib.join("library.yaml")).unwrap();
    assert!(catalog.contains("name: writer"));
    pp(&lib)
        .args(["library", "list"])
        .assert()
        .success()
        .stdout(predicates::str::contains(
            "skill\twriter\tWrites reusable prompts",
        ));
    pp(&lib)
        .args(["library", "search", "reusable"])
        .assert()
        .success()
        .stdout(predicates::str::contains("skill\twriter"));
}

#[test]
fn library_use_installs_dependencies_before_requested_item() {
    let tmp = tempfile::TempDir::new().unwrap();
    let lib = seeded_lib(&tmp);
    let work = tmp.path().join("work");
    std::fs::create_dir(&work).unwrap();
    let skill_dir = tmp.path().join("sources/writer");
    std::fs::create_dir_all(&skill_dir).unwrap();
    let skill_source = skill_dir.join("SKILL.md");
    std::fs::write(&skill_source, "skill body\n").unwrap();
    let prompt_source = tmp.path().join("sources/base-prompt.md");
    std::fs::write(&prompt_source, "prompt body\n").unwrap();

    pp(&lib)
        .args([
            "library",
            "add",
            "prompt",
            "base-prompt",
            "--description",
            "Base command",
            "--source",
            prompt_source.to_str().unwrap(),
        ])
        .assert()
        .success();
    pp(&lib)
        .args([
            "library",
            "add",
            "skill",
            "writer",
            "--description",
            "Writer skill",
            "--source",
            skill_source.to_str().unwrap(),
            "--requires",
            "prompt:base-prompt",
        ])
        .assert()
        .success();

    let mut cmd = pp(&lib);
    cmd.current_dir(&work);
    cmd.args(["library", "use", "writer"]).assert().success();

    assert!(work.join(".claude/commands/base-prompt.md").is_file());
    assert!(work.join(".claude/skills/writer/SKILL.md").is_file());
}

#[test]
fn library_sync_refreshes_installed_items_from_sources() {
    let tmp = tempfile::TempDir::new().unwrap();
    let lib = seeded_lib(&tmp);
    let work = tmp.path().join("work");
    std::fs::create_dir(&work).unwrap();
    let source_dir = tmp.path().join("sources/writer");
    std::fs::create_dir_all(&source_dir).unwrap();
    let source = source_dir.join("SKILL.md");
    std::fs::write(&source, "v1\n").unwrap();
    pp(&lib)
        .args([
            "library",
            "add",
            "skill",
            "writer",
            "--description",
            "Writer skill",
            "--source",
            source.to_str().unwrap(),
        ])
        .assert()
        .success();
    let mut use_cmd = pp(&lib);
    use_cmd.current_dir(&work);
    use_cmd
        .args(["library", "use", "writer"])
        .assert()
        .success();

    std::fs::write(&source, "v2\n").unwrap();
    let mut sync_cmd = pp(&lib);
    sync_cmd.current_dir(&work);
    sync_cmd.args(["library", "sync"]).assert().success();

    assert_eq!(
        std::fs::read_to_string(work.join(".claude/skills/writer/SKILL.md")).unwrap(),
        "v2\n"
    );
}

#[test]
fn library_push_copies_local_install_back_to_local_source() {
    let tmp = tempfile::TempDir::new().unwrap();
    let lib = seeded_lib(&tmp);
    let work = tmp.path().join("work");
    std::fs::create_dir(&work).unwrap();
    let source_dir = tmp.path().join("sources/writer");
    std::fs::create_dir_all(&source_dir).unwrap();
    let source = source_dir.join("SKILL.md");
    std::fs::write(&source, "source\n").unwrap();
    pp(&lib)
        .args([
            "library",
            "add",
            "skill",
            "writer",
            "--description",
            "Writer skill",
            "--source",
            source.to_str().unwrap(),
        ])
        .assert()
        .success();
    let mut use_cmd = pp(&lib);
    use_cmd.current_dir(&work);
    use_cmd
        .args(["library", "use", "writer"])
        .assert()
        .success();
    std::fs::write(work.join(".claude/skills/writer/SKILL.md"), "local edit\n").unwrap();

    let mut push_cmd = pp(&lib);
    push_cmd.current_dir(&work);
    push_cmd
        .args(["library", "push", "writer"])
        .assert()
        .success();

    assert_eq!(std::fs::read_to_string(source).unwrap(), "local edit\n");
}

#[test]
fn library_remove_can_delete_catalog_entry_and_local_install() {
    let tmp = tempfile::TempDir::new().unwrap();
    let lib = seeded_lib(&tmp);
    let work = tmp.path().join("work");
    std::fs::create_dir(&work).unwrap();
    let source_dir = tmp.path().join("sources/writer");
    std::fs::create_dir_all(&source_dir).unwrap();
    let source = source_dir.join("SKILL.md");
    std::fs::write(&source, "source\n").unwrap();
    pp(&lib)
        .args([
            "library",
            "add",
            "skill",
            "writer",
            "--description",
            "Writer skill",
            "--source",
            source.to_str().unwrap(),
        ])
        .assert()
        .success();
    let mut use_cmd = pp(&lib);
    use_cmd.current_dir(&work);
    use_cmd
        .args(["library", "use", "writer"])
        .assert()
        .success();

    let mut remove_cmd = pp(&lib);
    remove_cmd.current_dir(&work);
    remove_cmd
        .args(["library", "remove", "writer", "--delete-local"])
        .assert()
        .success();

    assert!(!std::fs::read_to_string(lib.join("library.yaml"))
        .unwrap()
        .contains("writer"));
    assert!(!work.join(".claude/skills/writer").exists());
}

#[test]
fn library_import_merges_existing_library_yaml() {
    let tmp = tempfile::TempDir::new().unwrap();
    let lib = seeded_lib(&tmp);
    let source = tmp.path().join("agent.md");
    std::fs::write(&source, "agent prompt\n").unwrap();
    let external = tmp.path().join("external-library.yaml");
    std::fs::write(
        &external,
        format!(
            "default_dirs:\n  skills:\n    - default: .claude/skills/\n    - global: ~/.claude/skills/\n  agents:\n    - default: .claude/agents/\n    - global: ~/.claude/agents/\n  prompts:\n    - default: .claude/commands/\n    - global: ~/.claude/commands/\nlibrary:\n  skills: []\n  agents:\n    - name: reviewer\n      description: Review code\n      source: {}\n  prompts: []\n",
            source.display()
        ),
    )
    .unwrap();

    pp(&lib)
        .args(["library", "import", external.to_str().unwrap()])
        .assert()
        .success();

    pp(&lib)
        .args(["library", "list"])
        .assert()
        .success()
        .stdout(predicates::str::contains("agent\treviewer\tReview code"));
}
