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
        .unwrap_or_else(|| default_editor().to_string())
}

/// Fallback when neither EDITOR nor VISUAL is set.
///
/// `vi` is a safe assumption on Unix but does not exist on a stock Windows
/// install, where `notepad` is always present.
const fn default_editor() -> &'static str {
    if cfg!(windows) {
        "notepad"
    } else {
        "vi"
    }
}

/// Launch the user's editor on `path` and wait for it to exit.
pub fn run_editor(path: &Path) -> Result<()> {
    let editor = resolve_editor();
    let status = editor_command(&editor, path)
        .status()
        .with_context(|| format!("failed to launch editor `{editor}`"))?;
    if !status.success() {
        bail!("editor `{editor}` exited with {status}");
    }
    Ok(())
}

/// Build the command that opens `editor` on `path`.
///
/// On Unix this goes through `sh -c` so EDITOR values carrying arguments
/// ("code --wait") work. Windows has no `sh` on a stock install — the
/// documented PowerShell installer does not provide one — so going through a
/// shell there meant no editor could ever launch. Instead the editor string is
/// split into program + arguments and spawned directly, which supports the
/// same "code --wait" shape without needing a shell at all.
#[cfg(not(windows))]
fn editor_command(editor: &str, path: &Path) -> Command {
    let mut cmd = Command::new("sh");
    cmd.arg("-c")
        .arg(format!("{editor} \"$1\""))
        .arg("pp-editor") // $0 for the sh script
        .arg(path);
    cmd
}

#[cfg(windows)]
fn editor_command(editor: &str, path: &Path) -> Command {
    let (program, args) = split_editor(editor);
    let mut cmd = Command::new(program);
    cmd.args(args).arg(path);
    cmd
}

/// Split an editor string into its program and arguments.
///
/// Handles the quoted-path case (`"C:\Program Files\Editor\ed.exe" --wait`)
/// that a plain whitespace split would break apart.
#[cfg_attr(not(windows), allow(dead_code))]
fn split_editor(editor: &str) -> (String, Vec<String>) {
    let mut tokens: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let mut has_token = false;

    for c in editor.chars() {
        match c {
            '"' => {
                in_quotes = !in_quotes;
                has_token = true;
            }
            c if c.is_whitespace() && !in_quotes => {
                if has_token {
                    tokens.push(std::mem::take(&mut current));
                    has_token = false;
                }
            }
            c => {
                current.push(c);
                has_token = true;
            }
        }
    }
    if has_token {
        tokens.push(current);
    }

    let mut iter = tokens.into_iter();
    let program = iter.next().unwrap_or_else(|| default_editor().to_string());
    (program, iter.collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn editor_resolution_prefers_editor_then_visual_then_default() {
        assert_eq!(
            resolve_from(Some("nano".into()), Some("code".into())),
            "nano"
        );
        assert_eq!(
            resolve_from(None, Some("code --wait".into())),
            "code --wait"
        );
        assert_eq!(resolve_from(Some(String::new()), None), default_editor());
    }

    #[test]
    fn default_editor_exists_on_the_target_platform() {
        // `vi` is not on a stock Windows install; `notepad` always is.
        if cfg!(windows) {
            assert_eq!(default_editor(), "notepad");
        } else {
            assert_eq!(default_editor(), "vi");
        }
    }

    #[test]
    fn split_editor_handles_bare_program() {
        assert_eq!(split_editor("notepad"), ("notepad".into(), vec![]));
    }

    #[test]
    fn split_editor_keeps_arguments() {
        assert_eq!(
            split_editor("code --wait"),
            ("code".into(), vec!["--wait".to_string()])
        );
    }

    #[test]
    fn split_editor_respects_quoted_program_path() {
        assert_eq!(
            split_editor(r#""C:\Program Files\Editor\ed.exe" --wait"#),
            (
                r"C:\Program Files\Editor\ed.exe".into(),
                vec!["--wait".to_string()]
            )
        );
    }

    #[test]
    fn split_editor_falls_back_when_empty() {
        assert_eq!(split_editor("   "), (default_editor().to_string(), vec![]));
    }
}
