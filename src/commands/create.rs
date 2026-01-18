use crate::{
    card::CardType,
    crud::DB,
    parser::{cards_from_md, content_to_card},
    tui::Editor,
    tui::Theme,
    utils::ask_yn,
    utils::is_markdown,
};

use std::{
    collections::HashSet,
    fs::{self, File, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
    time::Duration,
};

use anyhow::{Context, Result, bail};
use crossterm::{
    event::{
        self, Event, KeyCode, KeyEventKind, KeyModifiers, KeyboardEnhancementFlags,
        PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags,
    },
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    text::{Line, Span},
    widgets::{Paragraph, Wrap},
};

const FLASH_SECS: f64 = 1.5;

pub async fn run(db: &DB, card_path: PathBuf) -> Result<()> {
    if !is_markdown(&card_path) {
        bail!("Card path must be a markdown file: {}", card_path.display());
    }

    let file_exists = card_path.is_file();
    if !file_exists {
        let should_create = ask_yn(
            format!("Card '{}' does not exist. Create it?", card_path.display()),
            false,
        );
        if !should_create {
            println!("Aborting; card not created.");
            return Ok(());
        }
        create_file(&card_path)?;
    }

    capture_cards(db, &card_path).await?;
    Ok(())
}

fn create_file(path: &Path) -> Result<File> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)?;
    }
    let file = OpenOptions::new().create(true).append(true).open(path)?;
    Ok(file)
}

async fn create_card_append_file(db: &DB, path: &Path, contents: &str) -> Result<()> {
    let existing_len = fs::metadata(path).map(|m| m.len()).unwrap_or(0);
    let start_idx = existing_len as usize;
    let end_idx = start_idx + contents.len();

    let card = content_to_card(path, contents, start_idx, end_idx).context("Invalid card")?;
    let card_exists = db.card_exists(&card).await?;
    if card_exists {
        bail!("This card already exists in the database.");
    }

    let mut file = create_file(path)?;
    if start_idx > 0 {
        writeln!(file)?;
    }
    writeln!(file, "{}", contents)?;

    db.add_card(&card).await?;

    Ok(())
}

async fn capture_cards(db: &DB, card_path: &Path) -> Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(
        stdout,
        EnterAlternateScreen,
        PushKeyboardEnhancementFlags(
            KeyboardEnhancementFlags::REPORT_ALL_KEYS_AS_ESCAPE_CODES
                | KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES
        )
    )?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.show_cursor()?;
    let editor_result: Result<()> = async {
        let mut editor = Editor::new();
        let mut status: Option<String> = None;
        let existing_cards = cards_from_md(card_path)?;
        let unique_hashes: HashSet<_> = existing_cards.into_iter().map(|c| c.card_hash).collect();

        let mut num_cards_in_collection = unique_hashes.len();
        let mut card_created_count = 0;
        let mut card_last_save_attempt: Option<std::time::Instant> = None;
        let mut view_height = 0usize;
        loop {
            terminal.draw(|frame| {
                let area = frame.area();
                frame.render_widget(Theme::backdrop(), area);
                let chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Min(5), Constraint::Length(5)])
                    .split(area);

                view_height = chunks[0].height.saturating_sub(2) as usize;
                editor.ensure_cursor_visible(view_height.max(1));

                let editor_block = Theme::panel(card_path.display().to_string());
                let editor_widget = Paragraph::new(editor.content())
                    .block(editor_block)
                    .wrap(Wrap { trim: false })
                    .scroll((editor.scroll_top() as u16, 0));
                frame.render_widget(editor_widget, chunks[0]);

                let mut help_lines = vec![Line::from(vec![
                    Theme::key_chip("Ctrl+B"),
                    Theme::span(" basic"),
                    Theme::bullet(),
                    Theme::key_chip("Ctrl+K"),
                    Theme::span(" cloze"),
                    Theme::bullet(),
                    Theme::key_chip("Ctrl+S"),
                    Theme::span(" save"),
                    Theme::bullet(),
                    Theme::key_chip("Esc"),
                    Theme::span(" / "),
                    Theme::key_chip("Ctrl+C"),
                    Theme::span(" exit"),
                ])];
                help_lines.push(Line::from(vec![
                    Theme::span("Cards in collection:"),
                    Theme::label_span(format!(" {}", num_cards_in_collection)),
                    Theme::bullet(),
                    Theme::span("Created this session:"),
                    Theme::label_span(format!(" {}", card_created_count)),
                ]));
                if let Some(time) = card_last_save_attempt
                    && time.elapsed().as_secs_f64() < FLASH_SECS
                    && status.is_some()
                {
                    let message = status.clone().unwrap();
                    let style = if message.starts_with("Unable") {
                        Theme::danger()
                    } else {
                        Theme::success()
                    };
                    help_lines.push(Line::from(vec![Span::styled(message, style)]));
                }

                let instructions = Paragraph::new(help_lines)
                    .block(Theme::panel_with_line(Theme::section_header("Help")))
                    .wrap(Wrap { trim: true });
                frame.render_widget(instructions, chunks[1]);

                let (cursor_row, cursor_col) = editor.cursor();
                let visible_row = cursor_row.saturating_sub(editor.scroll_top());
                let cursor_x =
                    chunks[0].x + 1 + (cursor_col as u16).min(chunks[0].width.saturating_sub(2));
                let cursor_y =
                    chunks[0].y + 1 + (visible_row as u16).min(chunks[0].height.saturating_sub(2));
                frame.set_cursor_position((cursor_x, cursor_y));
            })?;

            if event::poll(Duration::from_millis(16))?
                && let Event::Key(key) = event::read()?
            {
                if key.kind != KeyEventKind::Press {
                    continue;
                }
                if key.code == KeyCode::Esc
                    || (key.code == KeyCode::Char('c')
                        && key.modifiers.contains(KeyModifiers::CONTROL))
                {
                    break;
                }
                if key.code == KeyCode::Char('b') && key.modifiers.contains(KeyModifiers::CONTROL) {
                    editor.card_type = CardType::Basic;
                    editor.clear();
                    continue;
                }
                if key.code == KeyCode::Char('k') && key.modifiers.contains(KeyModifiers::CONTROL) {
                    editor.card_type = CardType::Cloze;
                    editor.clear();
                    continue;
                }

                if key.code == KeyCode::Char('s') && key.modifiers.contains(KeyModifiers::CONTROL) {
                    let contents = editor.content();
                    let save_status = create_card_append_file(db, card_path, &contents).await;
                    match save_status {
                        Ok(_) => {
                            editor.clear();
                            card_created_count += 1;
                            num_cards_in_collection += 1;
                            card_last_save_attempt = Some(std::time::Instant::now());
                            status = Some(String::from("Card saved."))
                        }
                        Err(e) => {
                            card_last_save_attempt = Some(std::time::Instant::now());
                            let flat_error = e
                                .chain()
                                .map(|cause| cause.to_string().replace('\n', " "))
                                .collect::<Vec<_>>()
                                .join(": ");
                            status = Some(format!("Unable to save card: {}", flat_error));
                        }
                    }
                    continue;
                }

                match key.code {
                    KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                        editor.insert_char(c);
                    }
                    KeyCode::Enter => editor.insert_newline(),
                    KeyCode::Tab => editor.insert_tab(),
                    KeyCode::Backspace => editor.backspace(),
                    KeyCode::Delete => editor.delete(),
                    KeyCode::Left => editor.move_left(),
                    KeyCode::Right => editor.move_right(),
                    KeyCode::Up => editor.move_up(),
                    KeyCode::Down => editor.move_down(),
                    KeyCode::Home => editor.move_home(),
                    KeyCode::End => editor.move_end(),
                    KeyCode::PageUp => {
                        for _ in 0..view_height.max(1) {
                            editor.move_up();
                        }
                    }
                    KeyCode::PageDown => {
                        for _ in 0..view_height.max(1) {
                            editor.move_down();
                        }
                    }
                    _ => {}
                }
            }
        }
        Ok(())
    }
    .await;

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        PopKeyboardEnhancementFlags,
        LeaveAlternateScreen
    )?;
    terminal.show_cursor()?;

    editor_result
}

#[cfg(test)]
mod tests {

    use std::env::temp_dir;

    use super::*;

    #[tokio::test]
    async fn test_card_create() {
        let db = DB::new_in_memory().await.unwrap();
        let content = "Q: what?\nA: yes\n\n";
        let card_path = temp_dir().join("test.md");
        let result = create_card_append_file(&db, &card_path, content).await;
        assert!(result.is_ok());
    }
}
