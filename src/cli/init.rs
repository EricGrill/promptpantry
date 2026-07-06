use crate::core::gitops;
use anyhow::Result;
use std::fs;
use std::path::Path;

const README: &str = "# Prompt Pantry library\n\nEach `.md` file is a prompt card: YAML frontmatter (title, tags, description), then the prompt body.\nManage it with `pp`, or edit files directly — it's just a git repo.\n";

const EXAMPLE: &str = "---\ntitle: Bug Report Template\ntags: [bugs, templates]\ndescription: Structured repro report\n---\nRepo: {{repo}}\nBranch: {{branch}}\nDate: {{date}}\n\n## Steps to reproduce\n1.\n\n## Expected\n\n## Actual\n";

pub fn run(dir: &Path) -> Result<()> {
    fs::create_dir_all(dir)?;
    if !gitops::is_repo(dir) {
        gitops::init_repo(dir)?;
    }
    for (name, content) in [("README.md", README), ("bug-report-template.md", EXAMPLE)] {
        let p = dir.join(name);
        if !p.exists() {
            fs::write(&p, content)?;
        }
    }
    // skip the commit on a clean re-run so idempotent init stays warning-free
    if gitops::has_changes(dir, ".").unwrap_or(true) {
        if let Err(e) = gitops::commit_all(dir, "pp: init library") {
            eprintln!("warning: initial commit failed: {e}");
        }
    }
    println!("library ready at {}", dir.display());
    Ok(())
}
