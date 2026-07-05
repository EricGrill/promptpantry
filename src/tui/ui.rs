use crate::tui::app::{App, CatalogAddForm, CatalogImportForm, Mode, NewCardForm, VarForm, View};
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap};
use ratatui::Frame;

pub fn draw(f: &mut Frame, app: &App) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(1),
            Constraint::Length(1),
        ])
        .split(f.area());
    draw_search(f, app, rows[0]);
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
        .split(rows[1]);
    draw_list(f, app, cols[0]);
    draw_preview(f, app, cols[1]);
    draw_status(f, app, rows[2]);
    match &app.mode {
        Mode::VarForm(form) => draw_var_form(f, app, form),
        Mode::NewCard(form) => draw_new_card(f, form),
        Mode::ConfirmDelete => draw_confirm_delete(f, app),
        Mode::CatalogAdd(form) => draw_catalog_add(f, form),
        Mode::CatalogImport(form) => draw_catalog_import(f, form),
        Mode::ConfirmCatalogRemove => draw_confirm_catalog_remove(f, app),
        Mode::Browse => {}
    }
}

fn draw_search(f: &mut Frame, app: &App, area: Rect) {
    let (title, query, count) = match app.view {
        View::Cards => (
            " Prompt Pantry ",
            app.query.as_str(),
            format!(" {}/{} cards ", app.filtered.len(), app.cards.len()),
        ),
        View::Library => (
            " Library Catalog ",
            app.catalog_query.as_str(),
            format!(" {} entries ", app.catalog_rows.len()),
        ),
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .title(title)
        .title_top(Line::from(count).alignment(Alignment::Right));
    let inner = block.inner(area);
    f.render_widget(Paragraph::new(format!("> {query}")).block(block), area);
    if matches!(app.mode, Mode::Browse) {
        // chars, not bytes, so non-ASCII queries keep the cursor placed right
        let x = inner.x + 2 + query.chars().count() as u16;
        f.set_cursor_position((x.min(inner.right().saturating_sub(1)), inner.y));
    }
}

fn draw_list(f: &mut Frame, app: &App, area: Rect) {
    match app.view {
        View::Cards => draw_card_list(f, app, area),
        View::Library => draw_catalog_list(f, app, area),
    }
}

fn draw_card_list(f: &mut Frame, app: &App, area: Rect) {
    let items: Vec<ListItem> = app
        .filtered
        .iter()
        .map(|&i| {
            let c = &app.cards[i];
            let mut spans = vec![Span::raw(c.title.clone())];
            if c.parse_error.is_some() {
                spans.push(Span::styled(" !", Style::default().fg(Color::Red)));
            }
            ListItem::new(Line::from(spans))
        })
        .collect();
    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL))
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED))
        .highlight_symbol("▶ ");
    let mut state = ListState::default();
    state.select((!app.filtered.is_empty()).then_some(app.selected));
    f.render_stateful_widget(list, area, &mut state);
}

fn draw_catalog_list(f: &mut Frame, app: &App, area: Rect) {
    let items: Vec<ListItem> = app
        .catalog_rows
        .iter()
        .map(|row| {
            ListItem::new(Line::from(vec![
                Span::styled(
                    format!("{:<6}", row.kind.as_str()),
                    Style::default().fg(Color::Cyan),
                ),
                Span::raw(format!(" {}", row.entry.name)),
            ]))
        })
        .collect();
    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL))
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED))
        .highlight_symbol("▶ ");
    let mut state = ListState::default();
    state.select((!app.catalog_rows.is_empty()).then_some(app.catalog_selected));
    f.render_stateful_widget(list, area, &mut state);
}

fn draw_preview(f: &mut Frame, app: &App, area: Rect) {
    match app.view {
        View::Cards => draw_card_preview(f, app, area),
        View::Library => draw_catalog_preview(f, app, area),
    }
}

fn draw_card_preview(f: &mut Frame, app: &App, area: Rect) {
    let block = Block::default().borders(Borders::ALL);
    let Some(card) = app.selected_card() else {
        f.render_widget(Paragraph::new("no matching cards").block(block), area);
        return;
    };
    let mut lines = vec![Line::styled(
        card.title.clone(),
        Style::default().add_modifier(Modifier::BOLD),
    )];
    if !card.tags.is_empty() {
        lines.push(Line::styled(
            format!("tags: {}", card.tags.join(", ")),
            Style::default().fg(Color::Cyan),
        ));
    }
    if let Some(d) = &card.description {
        lines.push(Line::styled(
            d.clone(),
            Style::default().add_modifier(Modifier::ITALIC),
        ));
    }
    if let Some(err) = &card.parse_error {
        lines.push(Line::styled(err.clone(), Style::default().fg(Color::Red)));
    }
    lines.push(Line::from(
        "─".repeat(area.width.saturating_sub(2) as usize),
    ));
    lines.extend(card.body.lines().map(|l| Line::from(l.to_string())));
    let p = Paragraph::new(lines)
        .block(block)
        .wrap(Wrap { trim: false })
        .scroll((app.preview_scroll, 0));
    f.render_widget(p, area);
}

fn draw_catalog_preview(f: &mut Frame, app: &App, area: Rect) {
    let block = Block::default().borders(Borders::ALL);
    let Some(row) = app.selected_catalog_row() else {
        f.render_widget(
            Paragraph::new("no matching catalog entries").block(block),
            area,
        );
        return;
    };
    let mut lines = vec![
        Line::styled(
            format!("{}: {}", row.kind.as_str(), row.entry.name),
            Style::default().add_modifier(Modifier::BOLD),
        ),
        Line::from(row.entry.description.clone()),
        Line::styled(
            format!("status: {}", row.status),
            Style::default().fg(Color::Cyan),
        ),
        Line::from(format!("source: {}", row.entry.source)),
    ];
    if !row.entry.requires.is_empty() {
        lines.push(Line::from(format!(
            "requires: {}",
            row.entry.requires.join(", ")
        )));
    }
    lines.push(Line::from(
        "─".repeat(area.width.saturating_sub(2) as usize),
    ));
    lines.push(Line::from("Enter installs the selected entry."));
    lines.push(Line::from(
        "^s syncs installed entries; ^p pushes local edits.",
    ));
    lines.push(Line::from(
        "a adds; i imports; ^d removes catalog entry and local installs.",
    ));
    let p = Paragraph::new(lines)
        .block(block)
        .wrap(Wrap { trim: false })
        .scroll((app.catalog_preview_scroll, 0));
    f.render_widget(p, area);
}

fn draw_status(f: &mut Frame, app: &App, area: Rect) {
    let (text, style) = match &app.status {
        Some(msg) => (msg.clone(), Style::default().fg(Color::Yellow)),
        None => match app.view {
            View::Cards => (
                "tab library   ↵ copy   ^n new   ^e edit   ^d delete   ^s sync   esc quit"
                    .to_string(),
                Style::default().fg(Color::DarkGray),
            ),
            View::Library => (
                "tab cards   ↵ use   a add   i import   ^s sync   ^p push   ^d remove   esc quit"
                    .to_string(),
                Style::default().fg(Color::DarkGray),
            ),
        },
    };
    f.render_widget(Paragraph::new(text).style(style), area);
}

fn centered(width: u16, height: u16, area: Rect) -> Rect {
    let w = width.min(area.width);
    let h = height.min(area.height);
    Rect {
        x: area.x + (area.width - w) / 2,
        y: area.y + (area.height - h) / 2,
        width: w,
        height: h,
    }
}

fn draw_var_form(f: &mut Frame, app: &App, form: &VarForm) {
    let title = app.cards[form.card_idx].title.clone();
    let area = centered(50, form.names.len() as u16 + 2, f.area());
    f.render_widget(Clear, area);
    let lines: Vec<Line> = form
        .names
        .iter()
        .enumerate()
        .map(|(i, name)| {
            let style = if i == form.focus {
                Style::default().add_modifier(Modifier::REVERSED)
            } else {
                Style::default()
            };
            Line::styled(format!("{name}: {}", form.values[i]), style)
        })
        .collect();
    let block = Block::default()
        .borders(Borders::ALL)
        .title(format!(" fill variables — {title} "));
    f.render_widget(Paragraph::new(lines).block(block), area);
}

fn draw_new_card(f: &mut Frame, form: &NewCardForm) {
    let area = centered(50, 4, f.area());
    f.render_widget(Clear, area);
    let sel = |i: usize| {
        if form.focus == i {
            Style::default().add_modifier(Modifier::REVERSED)
        } else {
            Style::default()
        }
    };
    let lines = vec![
        Line::styled(format!("title: {}", form.title), sel(0)),
        Line::styled(format!("tags:  {}", form.tags), sel(1)),
    ];
    f.render_widget(
        Paragraph::new(lines).block(Block::default().borders(Borders::ALL).title(" new card ")),
        area,
    );
}

fn draw_confirm_delete(f: &mut Frame, app: &App) {
    let name = app
        .selected_card()
        .map(|c| c.id.clone())
        .unwrap_or_default();
    let area = centered(50, 3, f.area());
    f.render_widget(Clear, area);
    f.render_widget(
        Paragraph::new(format!("delete `{name}`? (y/n)"))
            .block(Block::default().borders(Borders::ALL).title(" confirm ")),
        area,
    );
}

fn draw_catalog_add(f: &mut Frame, form: &CatalogAddForm) {
    let area = centered(72, 7, f.area());
    f.render_widget(Clear, area);
    let sel = |i: usize| {
        if form.focus == i {
            Style::default().add_modifier(Modifier::REVERSED)
        } else {
            Style::default()
        }
    };
    let lines = vec![
        Line::styled(format!("kind:        {}", form.kind), sel(0)),
        Line::styled(format!("name:        {}", form.name), sel(1)),
        Line::styled(format!("description: {}", form.description), sel(2)),
        Line::styled(format!("source:      {}", form.source), sel(3)),
        Line::styled(format!("requires:    {}", form.requires), sel(4)),
    ];
    f.render_widget(
        Paragraph::new(lines).block(
            Block::default()
                .borders(Borders::ALL)
                .title(" add catalog entry "),
        ),
        area,
    );
}

fn draw_catalog_import(f: &mut Frame, form: &CatalogImportForm) {
    let area = centered(72, 3, f.area());
    f.render_widget(Clear, area);
    f.render_widget(
        Paragraph::new(format!("source: {}", form.source)).block(
            Block::default()
                .borders(Borders::ALL)
                .title(" import library.yaml "),
        ),
        area,
    );
}

fn draw_confirm_catalog_remove(f: &mut Frame, app: &App) {
    let name = app
        .selected_catalog_row()
        .map(|row| format!("{} `{}`", row.kind.as_str(), row.entry.name))
        .unwrap_or_default();
    let area = centered(60, 3, f.area());
    f.render_widget(Clear, area);
    f.render_widget(
        Paragraph::new(format!("remove {name} and local installs? (y/n)")).block(
            Block::default()
                .borders(Borders::ALL)
                .title(" confirm catalog remove "),
        ),
        area,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::store::Store;
    use crate::tui::app::App;
    use ratatui::backend::TestBackend;
    use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use ratatui::Terminal;
    use tempfile::TempDir;

    fn sample_app() -> (App, TempDir) {
        let tmp = TempDir::new().unwrap();
        // fake .git so is_repo() is true and no startup warning masks the help line
        std::fs::create_dir(tmp.path().join(".git")).unwrap();
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

    fn catalog_app() -> (App, TempDir) {
        let tmp = TempDir::new().unwrap();
        std::fs::write(
            tmp.path().join("standup.md"),
            "---\ntitle: Standup\n---\nbody\n",
        )
        .unwrap();
        let source_dir = tmp.path().join("sources/writer");
        std::fs::create_dir_all(&source_dir).unwrap();
        let source = source_dir.join("SKILL.md");
        std::fs::write(&source, "skill body\n").unwrap();
        std::fs::write(
            tmp.path().join("library.yaml"),
            format!(
                "library:\n  skills:\n    - name: writer\n      description: Writes reusable prompts\n      source: {}\n  agents: []\n  prompts: []\n",
                source.display()
            ),
        )
        .unwrap();
        let app = App::new(Store::open(tmp.path().to_path_buf()).unwrap());
        (app, tmp)
    }

    fn render_to_string(app: &App) -> String {
        let mut term = Terminal::new(TestBackend::new(80, 24)).unwrap();
        term.draw(|f| draw(f, app)).unwrap();
        let buf = term.backend().buffer().clone();
        let mut out = String::new();
        for y in 0..buf.area.height {
            for x in 0..buf.area.width {
                out.push_str(buf.cell((x, y)).unwrap().symbol());
            }
            out.push('\n');
        }
        out
    }

    #[test]
    fn main_screen_shows_title_count_cards_and_preview() {
        let (app, _t) = sample_app();
        let s = render_to_string(&app);
        assert!(s.contains("Prompt Pantry"));
        assert!(s.contains("2/2 cards"));
        assert!(s.contains("Bug Report"));
        assert!(s.contains("Standup"));
        assert!(s.contains("Ticket: {{ticket}}")); // preview of first (title-sorted) card
        assert!(s.contains("↵ copy"));
    }

    #[test]
    fn var_form_modal_renders_field_names() {
        let (mut app, _t) = sample_app();
        for c in "bug".chars() {
            app.handle_key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE));
        }
        app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        let s = render_to_string(&app);
        assert!(s.contains("fill variables"));
        assert!(s.contains("ticket:"));
    }

    #[test]
    fn new_card_modal_renders() {
        let (mut app, _t) = sample_app();
        app.handle_key(KeyEvent::new(KeyCode::Char('n'), KeyModifiers::CONTROL));
        let s = render_to_string(&app);
        assert!(s.contains("new card"));
        assert!(s.contains("title:"));
    }

    #[test]
    fn confirm_delete_modal_renders() {
        let (mut app, _t) = sample_app();
        app.handle_key(KeyEvent::new(KeyCode::Char('d'), KeyModifiers::CONTROL));
        let s = render_to_string(&app);
        assert!(s.contains("delete `bug-report`?")); // first title-sorted card is selected
    }

    #[test]
    fn library_view_renders_catalog_entries_and_help() {
        let (mut app, _t) = catalog_app();
        app.handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
        let s = render_to_string(&app);
        assert!(s.contains("Library Catalog"));
        assert!(s.contains("writer"));
        assert!(s.contains("Writes reusable prompts"));
        assert!(s.contains("↵ use"));
        assert!(s.contains("a add"));
    }
}
