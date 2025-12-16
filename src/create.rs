use crate::{
    card::CardType,
    crud::DB,
    editor::Editor,
    utils::{cards_from_md, content_to_card, validate_file_can_be_card},
};

use std::{
    fs::{self, OpenOptions},
    io::{self, Write},
    path::Path,
    time::Duration,
};

use anyhow::{Context, Result};
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
    style::Stylize,
    widgets::{Block, Borders, Paragraph, Wrap},
};

pub async fn run(db: &DB, card_path: String) -> Result<()> {
    let card_path = validate_file_can_be_card(card_path)?;
    let file_exists = card_path.is_file();
    if !file_exists && !prompt_create(&card_path)? {
        println!("Aborting; card not created.");
        return Ok(());
    }

    capture_cards(db, &card_path).await?;
    Ok(())
}

fn prompt_create(path: &Path) -> io::Result<bool> {
    print!(
        "Card '{}' does not exist. Create it? [y/N]: ",
        path.display()
    );
    io::stdout().flush()?;
    let mut answer = String::new();
    io::stdin().read_line(&mut answer)?;
    let trimmed = answer.trim().to_lowercase();
    Ok(trimmed == "y" || trimmed == "yes")
}

async fn create_card_append_file(db: &DB, path: &Path, contents: &str) -> Result<()> {
    let existing_len = fs::metadata(path).map(|m| m.len()).unwrap_or(0);
    let start_idx = existing_len as usize;
    let end_idx = start_idx + contents.len();

    let card = content_to_card(path, contents, start_idx, end_idx).context("Invalid card")?;
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)?;
    }
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;

    if start_idx > 0 {
        writeln!(file)?;
    }
    writeln!(file, "{}", contents)?;

    db.add_card(&card).await?;

    Ok(())
}

async fn capture_cards(db: &DB, card_path: &Path) -> io::Result<()> {
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

    let editor_result: io::Result<()> = async {
        let mut editor = Editor::new();
        let mut status: Option<String> = None;
        let mut num_cards_in_collection = cards_from_md(card_path).unwrap_or_default().len();
        let mut card_created_count = 0;
        let mut card_last_save_attempt: Option<std::time::Instant> = None;
        let mut view_height = 0usize;
        loop {
            terminal.draw(|frame| {
                let area = frame.area();
                let chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Min(3), Constraint::Length(5)])
                    .split(area);

                view_height = chunks[0].height.saturating_sub(2) as usize;
                editor.ensure_cursor_visible(view_height.max(1));

                let editor_block = Block::default()
                    .title(format!(" {} ", card_path.display()).bold())
                    .borders(Borders::ALL);
                let editor_widget = Paragraph::new(editor.content())
                    .block(editor_block)
                    .wrap(Wrap { trim: false })
                    .scroll((editor.scroll_top() as u16, 0));
                frame.render_widget(editor_widget, chunks[0]);

                let mut help = String::from(
                    "Ctrl+B for basic card • Ctrl+K for cloze card • Ctrl+S save • Esc/Ctrl-C exit\n",
                );
                help.push_str(&format!("Cards in collection: {} • Cards created: {}", num_cards_in_collection, card_created_count));
                if let Some(time) = card_last_save_attempt &&  time.elapsed().as_secs_f32() < 1.0 && status.is_some(){
                            help.push_str(&format!(" | {}", status.clone().unwrap()));
                    }

                let instructions = Paragraph::new(help)
                    .block(Block::default().borders(Borders::ALL).title(" Help "));
                frame.render_widget(instructions, chunks[1]);

                let (cursor_row, cursor_col) = editor.cursor();
                let visible_row = cursor_row.saturating_sub(editor.scroll_top());
                let cursor_x =
                    chunks[0].x + 1 + (cursor_col as u16).min(chunks[0].width.saturating_sub(2));
                let cursor_y =
                    chunks[0].y + 1 + (visible_row as u16).min(chunks[0].height.saturating_sub(2));
                frame.set_cursor_position((cursor_x, cursor_y));
            })?;

            if event::poll(Duration::from_millis(250))?
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
                            status = Some(format!("Unable to save card: {}", e));
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
