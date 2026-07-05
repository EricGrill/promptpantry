use crate::core::card::Card;
use crate::core::context::builtin_values;
use crate::core::search::search_indices;
use crate::core::store::Store;
use crate::core::template::{extract_vars, render};
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::collections::HashMap;
use std::path::PathBuf;

pub enum Mode {
    Browse,
    VarForm(VarForm),
    NewCard(NewCardForm),
    ConfirmDelete,
}

pub struct VarForm {
    pub card_idx: usize, // index into App::cards
    pub names: Vec<String>,
    pub values: Vec<String>,
    pub focus: usize,
}

#[derive(Default)]
pub struct NewCardForm {
    pub title: String,
    pub tags: String,
    pub focus: usize, // 0 = title, 1 = tags
}

/// What the event loop must do after a key press.
#[derive(Debug, PartialEq)]
pub enum Action {
    None,
    Quit,
    /// Suspend the TUI, open $EDITOR on `path`, then commit `commit_rel` with `commit_msg`.
    Edit {
        path: PathBuf,
        commit_rel: String,
        commit_msg: String,
    },
}

pub struct App {
    pub store: Store,
    pub cards: Vec<Card>,
    pub query: String,
    pub filtered: Vec<usize>, // indices into cards
    pub selected: usize,      // index into filtered
    pub preview_scroll: u16,
    pub mode: Mode,
    pub status: Option<String>,
    pub git_ok: bool,
}

impl App {
    pub fn new(store: Store) -> App {
        let git_ok = crate::core::gitops::is_repo(&store.root);
        let mut app = App {
            cards: vec![],
            query: String::new(),
            filtered: vec![],
            selected: 0,
            preview_scroll: 0,
            mode: Mode::Browse,
            status: (!git_ok)
                .then(|| "not a git repo — run `pp init` (changes won't be committed)".to_string()),
            git_ok,
            store,
        };
        app.reload();
        app
    }

    pub fn reload(&mut self) {
        self.cards = self.store.load_cards();
        self.refresh_filter();
    }

    pub fn refresh_filter(&mut self) {
        self.filtered = search_indices(&self.cards, &self.query);
        if self.selected >= self.filtered.len() {
            self.selected = self.filtered.len().saturating_sub(1);
        }
        self.preview_scroll = 0;
    }

    pub fn selected_card(&self) -> Option<&Card> {
        self.filtered.get(self.selected).map(|&i| &self.cards[i])
    }

    fn move_selection(&mut self, delta: i64) {
        if self.filtered.is_empty() {
            return;
        }
        let last = (self.filtered.len() - 1) as i64;
        self.selected = (self.selected as i64 + delta).clamp(0, last) as usize;
        self.preview_scroll = 0;
    }

    pub fn set_status(&mut self, msg: String) {
        self.status = Some(msg);
    }

    /// Route a key by mode. Status messages are transient: cleared on the next key.
    pub fn handle_key(&mut self, key: KeyEvent) -> Action {
        self.status = None;
        match self.mode {
            Mode::Browse => self.handle_browse(key),
            Mode::VarForm(_) => self.handle_var_form(key),
            Mode::NewCard(_) => self.handle_new_card(key),
            Mode::ConfirmDelete => self.handle_confirm_delete(key),
        }
    }

    fn handle_browse(&mut self, key: KeyEvent) -> Action {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        match (key.code, ctrl) {
            (KeyCode::Char('c'), true) => return Action::Quit,
            (KeyCode::Esc, _) => {
                if self.query.is_empty() {
                    return Action::Quit;
                }
                self.query.clear();
                self.selected = 0;
                self.refresh_filter();
            }
            (KeyCode::Up, _) | (KeyCode::Char('k'), true) => self.move_selection(-1),
            (KeyCode::Down, _) | (KeyCode::Char('j'), true) => self.move_selection(1),
            (KeyCode::PageUp, _) => self.preview_scroll = self.preview_scroll.saturating_sub(10),
            (KeyCode::PageDown, _) => self.preview_scroll = self.preview_scroll.saturating_add(10),
            (KeyCode::Char('n'), true) => self.mode = Mode::NewCard(NewCardForm::default()),
            (KeyCode::Char('e'), true) => {
                if let Some(card) = self.selected_card() {
                    return Action::Edit {
                        path: card.path.clone(),
                        commit_rel: format!("{}.md", card.id),
                        commit_msg: format!("pp: update {}", card.id),
                    };
                }
            }
            (KeyCode::Char('d'), true) => {
                if self.selected_card().is_some() {
                    self.mode = Mode::ConfirmDelete;
                }
            }
            (KeyCode::Enter, _) => self.start_copy(),
            (KeyCode::Backspace, _) => {
                self.query.pop();
                self.selected = 0;
                self.refresh_filter();
            }
            (KeyCode::Char(c), false) => {
                self.query.push(c);
                self.selected = 0;
                self.refresh_filter();
            }
            _ => {}
        }
        Action::None
    }

    /// Enter in Browse: copy directly when the card has no variables,
    /// otherwise open the variable form with builtins pre-filled.
    fn start_copy(&mut self) {
        let Some(idx) = self.filtered.get(self.selected).copied() else {
            return;
        };
        let names = extract_vars(&self.cards[idx].body);
        if names.is_empty() {
            let text = self.cards[idx].body.clone();
            let id = self.cards[idx].id.clone();
            self.finish_copy(&id, text);
        } else {
            let builtins = builtin_values(&std::env::current_dir().unwrap_or_default());
            let values = names
                .iter()
                .map(|n| builtins.get(n).cloned().unwrap_or_default())
                .collect();
            self.mode = Mode::VarForm(VarForm {
                card_idx: idx,
                names,
                values,
                focus: 0,
            });
        }
    }

    fn finish_copy(&mut self, id: &str, text: String) {
        match copy_to_clipboard(text) {
            Ok(()) => self.set_status(format!("copied `{id}` to clipboard")),
            Err(e) => self.set_status(format!("clipboard unavailable: {e}")),
        }
    }

    fn handle_var_form(&mut self, key: KeyEvent) -> Action {
        if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
            return Action::Quit;
        }
        let Mode::VarForm(form) = &mut self.mode else {
            return Action::None;
        };
        match key.code {
            KeyCode::Esc => self.mode = Mode::Browse,
            KeyCode::Tab | KeyCode::Down => form.focus = (form.focus + 1) % form.names.len(),
            KeyCode::BackTab | KeyCode::Up => {
                form.focus = (form.focus + form.names.len() - 1) % form.names.len()
            }
            KeyCode::Backspace => {
                form.values[form.focus].pop();
            }
            KeyCode::Enter => {
                if form.focus + 1 < form.names.len() {
                    form.focus += 1;
                } else {
                    let values: HashMap<String, String> = form
                        .names
                        .iter()
                        .cloned()
                        .zip(form.values.iter().cloned())
                        .collect();
                    let card_idx = form.card_idx;
                    let text = render(&self.cards[card_idx].body, &values);
                    let id = self.cards[card_idx].id.clone();
                    self.mode = Mode::Browse;
                    self.finish_copy(&id, text);
                }
            }
            KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                form.values[form.focus].push(c)
            }
            _ => {}
        }
        Action::None
    }

    fn handle_new_card(&mut self, key: KeyEvent) -> Action {
        if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
            return Action::Quit;
        }
        let Mode::NewCard(form) = &mut self.mode else {
            return Action::None;
        };
        match key.code {
            KeyCode::Esc => self.mode = Mode::Browse,
            KeyCode::Tab | KeyCode::BackTab | KeyCode::Up | KeyCode::Down => form.focus ^= 1,
            KeyCode::Backspace => {
                if form.focus == 0 {
                    form.title.pop();
                } else {
                    form.tags.pop();
                }
            }
            KeyCode::Enter => {
                if form.focus == 0 {
                    form.focus = 1;
                    return Action::None;
                }
                let title = form.title.trim().to_string();
                if title.is_empty() {
                    self.set_status("title required".to_string());
                    return Action::None;
                }
                let tags: Vec<String> = form
                    .tags
                    .split(',')
                    .map(|t| t.trim().to_string())
                    .filter(|t| !t.is_empty())
                    .collect();
                match self.store.create_card(&title, &tags) {
                    Ok((id, path)) => {
                        self.mode = Mode::Browse;
                        return Action::Edit {
                            path,
                            commit_rel: format!("{id}.md"),
                            commit_msg: format!("pp: add {id}"),
                        };
                    }
                    Err(e) => self.set_status(e.to_string()),
                }
            }
            KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                if form.focus == 0 {
                    form.title.push(c);
                } else {
                    form.tags.push(c);
                }
            }
            _ => {}
        }
        Action::None
    }

    fn handle_confirm_delete(&mut self, key: KeyEvent) -> Action {
        match key.code {
            KeyCode::Char('y') | KeyCode::Enter => {
                self.mode = Mode::Browse;
                if let Some(idx) = self.filtered.get(self.selected).copied() {
                    let card = self.cards[idx].clone();
                    match self.store.delete_card(&card) {
                        Ok(()) => {
                            self.reload();
                            self.set_status(format!("deleted `{}`", card.id));
                            // last, so a commit-failure warning replaces the happy-path status
                            self.commit_and_note(
                                &format!("{}.md", card.id),
                                &format!("pp: delete {}", card.id),
                            );
                        }
                        Err(e) => self.set_status(e.to_string()),
                    }
                }
            }
            _ => self.mode = Mode::Browse,
        }
        Action::None
    }

    /// Commit; on failure replace the current status with a warning.
    /// No-op when the library isn't a git repo — the startup warning already covers that.
    pub fn commit_and_note(&mut self, rel: &str, msg: &str) {
        if !self.git_ok {
            return;
        }
        if let Err(e) = crate::core::gitops::commit_path(&self.store.root, rel, msg) {
            self.set_status(format!("saved, but git commit failed: {e}"));
        }
    }
}

fn copy_to_clipboard(text: String) -> Result<(), arboard::Error> {
    arboard::Clipboard::new()?.set_text(text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }
    fn ctrl(c: char) -> KeyEvent {
        KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL)
    }

    fn sample_app() -> (App, TempDir) {
        let tmp = TempDir::new().unwrap();
        std::fs::write(
            tmp.path().join("bug-report.md"),
            "---\ntitle: Bug Report\ntags: [bugs]\n---\nTicket: {{ticket}}\n",
        )
        .unwrap();
        std::fs::write(
            tmp.path().join("standup.md"),
            "---\ntitle: Standup\n---\nplain body\n",
        )
        .unwrap();
        let app = App::new(Store::open(tmp.path().to_path_buf()).unwrap());
        (app, tmp)
    }

    #[test]
    fn typing_filters_and_esc_clears() {
        let (mut app, _t) = sample_app();
        assert_eq!(app.filtered.len(), 2);
        for c in "bug".chars() {
            app.handle_key(key(KeyCode::Char(c)));
        }
        assert_eq!(app.filtered.len(), 1);
        app.handle_key(key(KeyCode::Esc));
        assert_eq!(app.query, "");
        assert_eq!(app.filtered.len(), 2);
    }

    #[test]
    fn esc_on_empty_query_quits_and_ctrl_c_always_quits() {
        let (mut app, _t) = sample_app();
        assert_eq!(app.handle_key(key(KeyCode::Esc)), Action::Quit);
        assert_eq!(app.handle_key(ctrl('c')), Action::Quit);
    }

    #[test]
    fn enter_on_var_card_opens_form_with_vars() {
        let (mut app, _t) = sample_app();
        for c in "bug".chars() {
            app.handle_key(key(KeyCode::Char(c)));
        }
        app.handle_key(key(KeyCode::Enter));
        let Mode::VarForm(form) = &app.mode else {
            panic!("expected var form")
        };
        assert_eq!(form.names, vec!["ticket"]);
    }

    #[test]
    fn new_card_form_creates_file_and_requests_edit() {
        let (mut app, _t) = sample_app();
        app.handle_key(ctrl('n'));
        for c in "Retro Notes".chars() {
            app.handle_key(key(KeyCode::Char(c)));
        }
        app.handle_key(key(KeyCode::Enter)); // title -> tags field
        let action = app.handle_key(key(KeyCode::Enter)); // submit
        match action {
            Action::Edit { path, .. } => {
                assert!(path.to_string_lossy().ends_with("retro-notes.md"));
                assert!(path.exists());
            }
            other => panic!("expected Edit action, got {other:?}"),
        }
    }

    #[test]
    fn ctrl_c_quits_var_form_without_inserting() {
        let (mut app, _t) = sample_app();
        for c in "bug".chars() {
            app.handle_key(key(KeyCode::Char(c)));
        }
        app.handle_key(key(KeyCode::Enter));
        assert!(matches!(app.mode, Mode::VarForm(_)));
        assert_eq!(app.handle_key(ctrl('c')), Action::Quit);
        let Mode::VarForm(form) = &app.mode else {
            panic!("expected var form")
        };
        assert_eq!(form.values[form.focus], ""); // no literal 'c' inserted
    }

    #[test]
    fn ctrl_c_quits_new_card_form_without_inserting() {
        let (mut app, _t) = sample_app();
        app.handle_key(ctrl('n'));
        app.handle_key(key(KeyCode::Char('a')));
        assert_eq!(app.handle_key(ctrl('c')), Action::Quit);
        let Mode::NewCard(form) = &app.mode else {
            panic!("expected new card form")
        };
        assert_eq!(form.title, "a"); // no literal 'c' appended
    }

    #[test]
    fn delete_confirm_removes_the_file() {
        let (mut app, t) = sample_app();
        for c in "standup".chars() {
            app.handle_key(key(KeyCode::Char(c)));
        }
        app.handle_key(ctrl('d'));
        app.handle_key(key(KeyCode::Char('y')));
        assert!(!t.path().join("standup.md").exists());
        assert_eq!(app.filtered.len(), 0); // query "standup" no longer matches anything
    }
}
