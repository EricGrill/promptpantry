pub mod app;
pub mod ui;

use crate::core::{editor, gitops, store::Store};
use anyhow::Result;
use app::{Action, App};
use ratatui::crossterm::event::{self, Event, KeyEventKind};
use ratatui::DefaultTerminal;
use std::path::PathBuf;

pub fn run(dir: PathBuf) -> Result<()> {
    let store = Store::open(dir)?; // fails with the `pp init` hint before touching the terminal
    let mut app = App::new(store);
    let mut terminal = ratatui::init();
    let result = event_loop(&mut terminal, &mut app);
    ratatui::restore();
    result
}

fn event_loop(terminal: &mut DefaultTerminal, app: &mut App) -> Result<()> {
    loop {
        terminal.draw(|f| ui::draw(f, app))?;
        let Event::Key(key) = event::read()? else {
            continue;
        };
        if key.kind != KeyEventKind::Press {
            continue;
        }
        match app.handle_key(key) {
            Action::None => {}
            Action::Quit => return Ok(()),
            Action::Edit {
                path,
                commit_rel,
                commit_msg,
            } => {
                // hand the terminal to $EDITOR, then take it back
                ratatui::restore();
                let edit_result = editor::run_editor(&path);
                *terminal = ratatui::init();
                terminal.clear()?;
                match edit_result {
                    Ok(()) if !app.git_ok => {
                        // not a git repo: nothing to commit, no misleading git errors
                        app.set_status("saved".to_string());
                        app.reload();
                    }
                    Ok(()) => {
                        // commit only when the editor actually changed something —
                        // quit-without-save must not show a spurious git error
                        match gitops::has_changes(&app.store.root, &commit_rel) {
                            Ok(true) => {
                                if let Err(e) =
                                    gitops::commit_path(&app.store.root, &commit_rel, &commit_msg)
                                {
                                    app.set_status(format!("saved, but git commit failed: {e}"));
                                } else {
                                    app.set_status(commit_msg);
                                }
                            }
                            Ok(false) => app.set_status("no changes".to_string()),
                            Err(e) => app.set_status(format!("saved, but git check failed: {e}")),
                        }
                        app.reload();
                    }
                    Err(e) => {
                        app.set_status(format!("editor failed: {e}"));
                        app.reload(); // the file may still have been created/changed
                    }
                }
            }
        }
    }
}
