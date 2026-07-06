use crate::core::card::Card;
use crate::core::catalog::{self, CatalogRow, EntryKind, InstallScope};
use crate::core::context::builtin_values;
use crate::core::search::search_indices;
use crate::core::store::Store;
use crate::core::template::{extract_vars, render};
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum View {
    Cards,
    Library,
}

pub enum Mode {
    Browse,
    VarForm(VarForm),
    NewCard(NewCardForm),
    ConfirmDelete,
    CatalogAdd(CatalogAddForm),
    CatalogImport(CatalogImportForm),
    ConfirmCatalogPush,
    ConfirmCatalogRemove,
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

#[derive(Default)]
pub struct CatalogAddForm {
    pub kind: String,
    pub name: String,
    pub description: String,
    pub source: String,
    pub requires: String,
    pub focus: usize,
}

impl CatalogAddForm {
    fn field_mut(&mut self) -> &mut String {
        match self.focus {
            0 => &mut self.kind,
            1 => &mut self.name,
            2 => &mut self.description,
            3 => &mut self.source,
            _ => &mut self.requires,
        }
    }
}

#[derive(Default)]
pub struct CatalogImportForm {
    pub source: String,
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
    pub cwd: PathBuf,
    pub view: View,
    pub cards: Vec<Card>,
    pub query: String,
    pub filtered: Vec<usize>, // indices into cards
    pub selected: usize,      // index into filtered
    pub preview_scroll: u16,
    pub catalog_query: String,
    pub catalog_rows: Vec<CatalogRow>,
    pub catalog_selected: usize,
    pub catalog_preview_scroll: u16,
    pub mode: Mode,
    pub status: Option<String>,
    pub git_ok: bool,
}

impl App {
    pub fn new(store: Store) -> App {
        let cwd = std::env::current_dir().unwrap_or_default();
        App::new_with_cwd(store, cwd)
    }

    pub fn new_with_cwd(store: Store, cwd: PathBuf) -> App {
        let git_ok = crate::core::gitops::is_repo(&store.root);
        let mut app = App {
            cwd,
            view: View::Cards,
            cards: vec![],
            query: String::new(),
            filtered: vec![],
            selected: 0,
            preview_scroll: 0,
            catalog_query: String::new(),
            catalog_rows: vec![],
            catalog_selected: 0,
            catalog_preview_scroll: 0,
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
        self.reload_catalog();
    }

    pub fn refresh_filter(&mut self) {
        self.filtered = search_indices(&self.cards, &self.query);
        if self.selected >= self.filtered.len() {
            self.selected = self.filtered.len().saturating_sub(1);
        }
        self.preview_scroll = 0;
    }

    pub fn reload_catalog(&mut self) {
        let query = (!self.catalog_query.is_empty()).then_some(self.catalog_query.as_str());
        match catalog::rows(&self.store.root, query, &self.cwd) {
            Ok(rows) => {
                self.catalog_rows = rows;
                if self.catalog_selected >= self.catalog_rows.len() {
                    self.catalog_selected = self.catalog_rows.len().saturating_sub(1);
                }
                self.catalog_preview_scroll = 0;
            }
            Err(e) => {
                self.catalog_rows.clear();
                self.catalog_selected = 0;
                self.set_status(format!("catalog error: {e}"));
            }
        }
    }

    pub fn selected_card(&self) -> Option<&Card> {
        self.filtered.get(self.selected).map(|&i| &self.cards[i])
    }

    pub fn selected_catalog_row(&self) -> Option<&CatalogRow> {
        self.catalog_rows.get(self.catalog_selected)
    }

    fn move_selection(&mut self, delta: i64) {
        if self.filtered.is_empty() {
            return;
        }
        let last = (self.filtered.len() - 1) as i64;
        self.selected = (self.selected as i64 + delta).clamp(0, last) as usize;
        self.preview_scroll = 0;
    }

    fn move_catalog_selection(&mut self, delta: i64) {
        if self.catalog_rows.is_empty() {
            return;
        }
        let last = (self.catalog_rows.len() - 1) as i64;
        self.catalog_selected = (self.catalog_selected as i64 + delta).clamp(0, last) as usize;
        self.catalog_preview_scroll = 0;
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
            Mode::CatalogAdd(_) => self.handle_catalog_add(key),
            Mode::CatalogImport(_) => self.handle_catalog_import(key),
            Mode::ConfirmCatalogPush => self.handle_confirm_catalog_push(key),
            Mode::ConfirmCatalogRemove => self.handle_confirm_catalog_remove(key),
        }
    }

    fn handle_browse(&mut self, key: KeyEvent) -> Action {
        if key.code == KeyCode::Tab {
            self.view = match self.view {
                View::Cards => View::Library,
                View::Library => View::Cards,
            };
            return Action::None;
        }
        match self.view {
            View::Cards => self.handle_card_browse(key),
            View::Library => self.handle_library_browse(key),
        }
    }

    fn handle_card_browse(&mut self, key: KeyEvent) -> Action {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        match (key.code, ctrl) {
            (KeyCode::Char('c'), true) => return Action::Quit,
            (KeyCode::Char('s'), true) => self.sync_prompt_library(),
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

    fn handle_library_browse(&mut self, key: KeyEvent) -> Action {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        match (key.code, ctrl) {
            (KeyCode::Char('c'), true) => return Action::Quit,
            (KeyCode::Char('s'), true) => self.sync_catalog(),
            (KeyCode::Char('p'), true) => {
                if self.selected_catalog_row().is_some() {
                    self.mode = Mode::ConfirmCatalogPush;
                }
            }
            (KeyCode::Char('d'), true) => {
                if self.selected_catalog_row().is_some() {
                    self.mode = Mode::ConfirmCatalogRemove;
                }
            }
            (KeyCode::Esc, _) => {
                if self.catalog_query.is_empty() {
                    return Action::Quit;
                }
                self.catalog_query.clear();
                self.catalog_selected = 0;
                self.reload_catalog();
            }
            (KeyCode::Up, _) | (KeyCode::Char('k'), true) => self.move_catalog_selection(-1),
            (KeyCode::Down, _) | (KeyCode::Char('j'), true) => self.move_catalog_selection(1),
            (KeyCode::PageUp, _) => {
                self.catalog_preview_scroll = self.catalog_preview_scroll.saturating_sub(10)
            }
            (KeyCode::PageDown, _) => {
                self.catalog_preview_scroll = self.catalog_preview_scroll.saturating_add(10)
            }
            (KeyCode::Enter, _) => self.use_selected_catalog_entry(),
            (KeyCode::Backspace, _) => {
                self.catalog_query.pop();
                self.catalog_selected = 0;
                self.reload_catalog();
            }
            (KeyCode::Char('a'), false) if self.catalog_query.is_empty() => {
                self.mode = Mode::CatalogAdd(CatalogAddForm::default());
            }
            (KeyCode::Char('i'), false) if self.catalog_query.is_empty() => {
                self.mode = Mode::CatalogImport(CatalogImportForm::default());
            }
            (KeyCode::Char(c), false) => {
                self.catalog_query.push(c);
                self.catalog_selected = 0;
                self.reload_catalog();
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
        if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
            return Action::Quit;
        }
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

    fn handle_catalog_add(&mut self, key: KeyEvent) -> Action {
        if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
            return Action::Quit;
        }
        let mut submit = None;
        let Mode::CatalogAdd(form) = &mut self.mode else {
            return Action::None;
        };
        match key.code {
            KeyCode::Esc => self.mode = Mode::Browse,
            KeyCode::Tab | KeyCode::Down => form.focus = (form.focus + 1) % 5,
            KeyCode::BackTab | KeyCode::Up => form.focus = (form.focus + 4) % 5,
            KeyCode::Backspace => {
                form.field_mut().pop();
            }
            KeyCode::Enter => {
                if form.focus < 4 {
                    form.focus += 1;
                } else {
                    submit = Some((
                        form.kind.clone(),
                        form.name.clone(),
                        form.description.clone(),
                        form.source.clone(),
                        form.requires.clone(),
                    ));
                }
            }
            KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                form.field_mut().push(c);
            }
            _ => {}
        }
        if let Some((kind, name, description, source, requires)) = submit {
            self.add_catalog_entry(kind, name, description, source, requires);
        }
        Action::None
    }

    fn handle_catalog_import(&mut self, key: KeyEvent) -> Action {
        if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
            return Action::Quit;
        }
        let mut submit = None;
        let Mode::CatalogImport(form) = &mut self.mode else {
            return Action::None;
        };
        match key.code {
            KeyCode::Esc => self.mode = Mode::Browse,
            KeyCode::Backspace => {
                form.source.pop();
            }
            KeyCode::Enter => submit = Some(form.source.clone()),
            KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                form.source.push(c);
            }
            _ => {}
        }
        if let Some(source) = submit {
            self.import_catalog(source);
        }
        Action::None
    }

    fn handle_confirm_catalog_push(&mut self, key: KeyEvent) -> Action {
        if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
            return Action::Quit;
        }
        match key.code {
            KeyCode::Char('y') | KeyCode::Enter => {
                self.mode = Mode::Browse;
                self.push_selected_catalog_entry();
            }
            _ => self.mode = Mode::Browse,
        }
        Action::None
    }

    fn handle_confirm_catalog_remove(&mut self, key: KeyEvent) -> Action {
        if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
            return Action::Quit;
        }
        match key.code {
            KeyCode::Char('y') | KeyCode::Enter => {
                self.mode = Mode::Browse;
                self.remove_selected_catalog_entry();
            }
            _ => self.mode = Mode::Browse,
        }
        Action::None
    }

    fn add_catalog_entry(
        &mut self,
        kind: String,
        name: String,
        description: String,
        source: String,
        requires: String,
    ) {
        let kind = match kind.trim().parse::<EntryKind>() {
            Ok(kind) => kind,
            Err(e) => {
                self.set_status(e.to_string());
                return;
            }
        };
        let requires = split_requires(&requires);
        match catalog::add(
            &self.store.root,
            kind,
            name.trim().to_string(),
            description.trim().to_string(),
            source.trim().to_string(),
            requires,
        ) {
            Ok(()) => {
                self.mode = Mode::Browse;
                self.reload_catalog();
                self.set_status(format!("added {} catalog entry", kind.as_str()));
            }
            Err(e) => self.set_status(e.to_string()),
        }
    }

    fn import_catalog(&mut self, source: String) {
        match catalog::import(&self.store.root, PathBuf::from(source.trim()).as_path()) {
            Ok(count) => {
                self.mode = Mode::Browse;
                self.reload_catalog();
                self.set_status(format!("imported {count} library entries"));
            }
            Err(e) => self.set_status(e.to_string()),
        }
    }

    fn use_selected_catalog_entry(&mut self) {
        let Some(row) = self.selected_catalog_row().cloned() else {
            return;
        };
        match catalog::use_entry(
            &self.store.root,
            &row.entry.name,
            InstallScope::Default,
            &self.cwd,
        ) {
            Ok(reports) => {
                self.reload_catalog();
                self.set_status(report_status(&reports));
            }
            Err(e) => self.set_status(e.to_string()),
        }
    }

    fn sync_catalog(&mut self) {
        match catalog::sync_installed(&self.store.root, &self.cwd) {
            Ok(reports) => {
                self.reload_catalog();
                self.set_status(report_status(&reports));
            }
            Err(e) => self.set_status(e.to_string()),
        }
    }

    fn push_selected_catalog_entry(&mut self) {
        let Some(row) = self.selected_catalog_row().cloned() else {
            return;
        };
        match catalog::push_entry(&self.store.root, &row.entry.name, &self.cwd) {
            Ok(report) => {
                self.reload_catalog();
                self.set_status(format!(
                    "{} `{}` {}",
                    report.kind.as_str(),
                    report.name,
                    report.status
                ));
            }
            Err(e) => self.set_status(e.to_string()),
        }
    }

    fn remove_selected_catalog_entry(&mut self) {
        let Some(row) = self.selected_catalog_row().cloned() else {
            return;
        };
        match catalog::remove(&self.store.root, &row.entry.name, true, &self.cwd) {
            Ok(report) => {
                self.reload_catalog();
                self.set_status(format!(
                    "{} `{}` {}",
                    report.kind.as_str(),
                    report.name,
                    report.status
                ));
            }
            Err(e) => self.set_status(e.to_string()),
        }
    }

    fn sync_prompt_library(&mut self) {
        match crate::core::gitops::sync(&self.store.root) {
            Ok(out) => {
                self.reload();
                self.set_status(out.trim().lines().next().unwrap_or("synced").to_string());
            }
            Err(e) => self.set_status(e.to_string()),
        }
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

fn split_requires(raw: &str) -> Vec<String> {
    raw.split(',')
        .map(|part| part.trim().to_string())
        .filter(|part| !part.is_empty())
        .collect()
}

fn report_status(reports: &[catalog::ActionReport]) -> String {
    if reports.is_empty() {
        return "no catalog changes".to_string();
    }
    reports
        .iter()
        .map(|report| {
            format!(
                "{} `{}` {}",
                report.kind.as_str(),
                report.name,
                report.status
            )
        })
        .collect::<Vec<_>>()
        .join("; ")
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

    fn type_text(app: &mut App, text: &str) {
        for c in text.chars() {
            app.handle_key(key(KeyCode::Char(c)));
        }
    }

    fn catalog_app() -> (App, TempDir, PathBuf, PathBuf) {
        let tmp = TempDir::new().unwrap();
        std::fs::write(
            tmp.path().join("standup.md"),
            "---\ntitle: Standup\n---\nbody\n",
        )
        .unwrap();
        let source_dir = tmp.path().join("sources/writer");
        std::fs::create_dir_all(&source_dir).unwrap();
        let source = source_dir.join("SKILL.md");
        std::fs::write(&source, "v1\n").unwrap();
        let install_base = tmp.path().join("installed/.claude/skills");
        std::fs::write(
            tmp.path().join("library.yaml"),
            format!(
                "default_dirs:\n  skills:\n    - default: {}/\n    - global: {}/\n  agents:\n    - default: {}/agents/\n    - global: {}/agents/\n  prompts:\n    - default: {}/commands/\n    - global: {}/commands/\nlibrary:\n  skills:\n    - name: writer\n      description: Writes reusable prompts\n      source: {}\n  agents: []\n  prompts: []\n",
                install_base.display(),
                install_base.display(),
                tmp.path().join("installed/.claude").display(),
                tmp.path().join("installed/.claude").display(),
                tmp.path().join("installed/.claude").display(),
                tmp.path().join("installed/.claude").display(),
                source.display()
            ),
        )
        .unwrap();
        let app = App::new(Store::open(tmp.path().to_path_buf()).unwrap());
        (app, tmp, source, install_base)
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
    fn ctrl_c_quits_confirm_delete() {
        let (mut app, t) = sample_app();
        for c in "standup".chars() {
            app.handle_key(key(KeyCode::Char(c)));
        }
        app.handle_key(ctrl('d'));
        assert!(matches!(app.mode, Mode::ConfirmDelete));
        assert_eq!(app.handle_key(ctrl('c')), Action::Quit);
        assert!(t.path().join("standup.md").exists()); // not deleted
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

    #[test]
    fn tab_switches_to_library_catalog_and_filters_entries() {
        let (mut app, _t, _source, install_base) = catalog_app();
        let installed = install_base.join("writer/SKILL.md");

        app.handle_key(key(KeyCode::Tab));
        type_text(&mut app, "missing");
        app.handle_key(key(KeyCode::Enter));
        assert!(!installed.exists());

        app.handle_key(key(KeyCode::Esc));
        type_text(&mut app, "writer");
        app.handle_key(key(KeyCode::Enter));
        assert_eq!(std::fs::read_to_string(&installed).unwrap(), "v1\n");
    }

    #[test]
    fn library_view_installs_syncs_pushes_and_removes_selected_entry() {
        let (mut app, tmp, source, install_base) = catalog_app();
        let installed = install_base.join("writer/SKILL.md");

        app.handle_key(key(KeyCode::Tab));
        app.handle_key(key(KeyCode::Enter));
        assert_eq!(std::fs::read_to_string(&installed).unwrap(), "v1\n");

        std::fs::write(&source, "v2\n").unwrap();
        app.handle_key(ctrl('s'));
        assert_eq!(std::fs::read_to_string(&installed).unwrap(), "v2\n");

        std::fs::write(&installed, "local edit\n").unwrap();
        app.handle_key(ctrl('p'));
        assert_eq!(std::fs::read_to_string(&source).unwrap(), "v2\n");
        assert!(matches!(app.mode, Mode::ConfirmCatalogPush));
        app.handle_key(key(KeyCode::Char('y')));
        assert_eq!(std::fs::read_to_string(&source).unwrap(), "local edit\n");

        app.handle_key(ctrl('d'));
        app.handle_key(key(KeyCode::Char('y')));
        assert!(!std::fs::read_to_string(tmp.path().join("library.yaml"))
            .unwrap()
            .contains("writer"));
        assert!(!install_base.join("writer").exists());
    }

    #[test]
    fn library_add_form_registers_catalog_entries() {
        let (mut app, tmp) = sample_app();
        let source_dir = tmp.path().join("sources/reviewer");
        std::fs::create_dir_all(&source_dir).unwrap();
        let source = source_dir.join("SKILL.md");
        std::fs::write(&source, "review skill\n").unwrap();

        app.handle_key(key(KeyCode::Tab));
        app.handle_key(key(KeyCode::Char('a')));
        type_text(&mut app, "skill");
        app.handle_key(key(KeyCode::Enter));
        type_text(&mut app, "reviewer");
        app.handle_key(key(KeyCode::Enter));
        type_text(&mut app, "Reviews code");
        app.handle_key(key(KeyCode::Enter));
        type_text(&mut app, source.to_str().unwrap());
        app.handle_key(key(KeyCode::Enter));
        app.handle_key(key(KeyCode::Enter));

        let catalog = std::fs::read_to_string(tmp.path().join("library.yaml")).unwrap();
        assert!(catalog.contains("name: reviewer"));
        assert!(catalog.contains("description: Reviews code"));
    }

    #[test]
    fn library_import_form_merges_catalog_entries() {
        let (mut app, tmp) = sample_app();
        let source = tmp.path().join("agent.md");
        std::fs::write(&source, "agent body\n").unwrap();
        let external = tmp.path().join("external-library.yaml");
        std::fs::write(
            &external,
            format!(
                "library:\n  skills: []\n  agents:\n    - name: reviewer\n      description: Review code\n      source: {}\n  prompts: []\n",
                source.display()
            ),
        )
        .unwrap();

        app.handle_key(key(KeyCode::Tab));
        app.handle_key(key(KeyCode::Char('i')));
        type_text(&mut app, external.to_str().unwrap());
        app.handle_key(key(KeyCode::Enter));

        assert!(std::fs::read_to_string(tmp.path().join("library.yaml"))
            .unwrap()
            .contains("name: reviewer"));
    }
}
