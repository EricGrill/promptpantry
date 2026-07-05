use crate::core::{editor, gitops, store::Store};
use anyhow::Result;
use std::path::Path;

pub fn run(dir: &Path, title: &str, tags: &[String]) -> Result<()> {
    let store = Store::open(dir.to_path_buf())?;
    let (id, path) = store.create_card(title, tags)?;
    editor::run_editor(&path)?;
    if let Err(e) = gitops::commit_path(dir, &format!("{id}.md"), &format!("pp: add {id}")) {
        eprintln!("warning: git commit failed: {e}");
    }
    println!("created {}", path.display());
    Ok(())
}
