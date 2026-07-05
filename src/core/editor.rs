use anyhow::{bail, Context, Result};
use std::path::Path;
use std::process::Command;

pub fn resolve_editor() -> String {
    resolve_from(std::env::var("EDITOR").ok(), std::env::var("VISUAL").ok())
}

fn resolve_from(editor: Option<String>, visual: Option<String>) -> String {
    editor
        .filter(|s| !s.is_empty())
        .or(visual.filter(|s| !s.is_empty()))
        .unwrap_or_else(|| "vi".to_string())
}

/// Launch the user's editor on `path` and wait for it to exit.
/// Runs through `sh -c` so EDITOR values with arguments ("code --wait") work.
pub fn run_editor(path: &Path) -> Result<()> {
    let editor = resolve_editor();
    let status = Command::new("sh")
        .arg("-c")
        .arg(format!("{editor} \"$1\""))
        .arg("pp-editor") // $0 for the sh script
        .arg(path)
        .status()
        .with_context(|| format!("failed to launch editor `{editor}`"))?;
    if !status.success() {
        bail!("editor `{editor}` exited with {status}");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn editor_resolution_prefers_editor_then_visual_then_vi() {
        assert_eq!(
            resolve_from(Some("nano".into()), Some("code".into())),
            "nano"
        );
        assert_eq!(
            resolve_from(None, Some("code --wait".into())),
            "code --wait"
        );
        assert_eq!(resolve_from(Some(String::new()), None), "vi");
    }
}
