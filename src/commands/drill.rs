use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crate::card::{Card, CardContent, CardType};
use crate::cloze_utils::mask_cloze_text;
use crate::crud::DB;
use crate::fsrs::{LEARN_AHEAD_THRESHOLD_MINS, ReviewStatus};
use crate::llm::drill_preprocessor::{AIStatus, DrillPreprocessor};
use crate::palette::Palette;
use crate::parser::{cards_from_md, content_to_card, extract_media, register_all_cards, render_markdown, Media};
use crate::tui::{Editor, Theme};
use crate::utils::pluralize;

use anyhow::{Context, Result, anyhow, bail};
use crossterm::event::KeyModifiers;
use crossterm::{
    event::{
        self, Event, KeyCode, KeyEventKind, KeyboardEnhancementFlags, PopKeyboardEnhancementFlags,
        PushKeyboardEnhancementFlags,
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
use tokio::sync::mpsc;

const MINUTES_PER_DAY: f64 = 24.0 * 60.0;
const FLASH_SECS: f64 = 2.0;

pub async fn run(
    db: &DB,
    paths: Vec<PathBuf>,
    card_limit: Option<usize>,
    new_card_limit: Option<usize>,
    rephrase_questions: bool,
    shuffle: bool,
    retention: f32,
) -> Result<()> {
    validate_retention(retention)?;
    let (hash_cards, _) = register_all_cards(db, paths).await?;
    let mut cards_due_today = db
        .due_today(&hash_cards, card_limit, new_card_limit)
        .await?;

    if shuffle {
        use rand::seq::SliceRandom;
        cards_due_today.shuffle(&mut rand::rng());
    }

    if cards_due_today.is_empty() {
        println!(
            "{}",
            Palette::paint(Palette::SUCCESS, "All caught up—no cards due today.")
        );
        return Ok(());
    }

    let drill_preprocessor = DrillPreprocessor::new(&cards_due_today, rephrase_questions).await?;
    drill_preprocessor.initialize_card_status(&mut cards_due_today);
    start_drill_session(db, cards_due_today, drill_preprocessor, retention).await?;

    Ok(())
}

fn validate_retention(retention: f32) -> Result<()> {
    if retention > 1.0 {
        bail!("Retention must be less than or equal to 1.0")
    }
    if retention < 0.65 {
        bail!("Retention must be greater than 0.65")
    }
    Ok(())
}
struct DrillState<'a> {
    db: &'a DB,
    cards: Vec<Card>,
    redo_cards: Vec<Card>,
    current_idx: usize,
    show_answer: bool,
    last_action: Option<LastAction>,
    current_medias: Vec<Media>,
    retention: f32,
}
struct LastAction {
    action: ReviewStatus,
    show_again_duration: f64,
    last_reviewed_at: Instant,
}
impl LastAction {
    fn print(&self) -> String {
        let mut show_again = String::new();
        if self.show_again_duration <= 15.0 / MINUTES_PER_DAY {
            show_again.push_str("<15 mins");
        } else if self.show_again_duration <= 30.0 / MINUTES_PER_DAY {
            show_again.push_str("<30 mins");
        } else if self.show_again_duration <= 0.5 {
            show_again.push_str("<12 hours");
        } else if self.show_again_duration <= 1.0 {
            show_again.push_str("<1 day");
        } else {
            show_again.push_str(format!("{} days", self.show_again_duration as i64).as_str());
        }
        format!(" {} (See again in {})", self.action.label(), show_again,)
    }
}

impl<'a> DrillState<'a> {
    fn new(db: &'a DB, cards: Vec<Card>, retention: f32) -> Self {
        Self {
            db,
            cards,
            redo_cards: Vec::new(),
            current_idx: 0,
            show_answer: false,
            last_action: None,
            current_medias: Vec::new(),
            retention,
        }
    }

    fn current_card(&mut self) -> Option<Card> {
        if self.current_idx >= self.cards.len() {
            if self.redo_cards.is_empty() {
                return None;
            }
            self.cards = std::mem::take(&mut self.redo_cards);
            self.current_idx = 0;
        }
        self.cards.get(self.current_idx).cloned()
    }

    fn reveal_answer(&mut self) {
        self.show_answer = true;
    }

    async fn handle_review(&mut self, action: ReviewStatus) -> Result<()> {
        let current_card = self
            .current_card()
            .expect("card should exist when handling review");
        let show_again_duration = self
            .db
            .update_card_performance(&current_card, action, None, self.retention)
            .await?;
        if action == ReviewStatus::Fail
            || show_again_duration
                < (LEARN_AHEAD_THRESHOLD_MINS.num_minutes() as f64 / MINUTES_PER_DAY)
        {
            self.redo_cards.push(current_card.clone());
        }

        self.last_action = Some(LastAction {
            action,
            show_again_duration,
            last_reviewed_at: std::time::Instant::now(),
        });
        self.current_idx += 1;
        self.show_answer = false;
        Ok(())
    }

    fn is_complete(&self) -> bool {
        self.current_idx >= self.cards.len() && self.redo_cards.is_empty()
    }

    fn apply_ai_update(&mut self, update: AiUpdate) {
        for card in self.cards.iter_mut().chain(self.redo_cards.iter_mut()) {
            if card.card_hash == update.card_hash {
                *card = update.card.clone();
                card.ai_status = AIStatus::AiEnhanced;
            }
        }
    }

    fn apply_edit_update(&mut self, update: CardEditUpdate) {
        let file_path = update.updated_card.file_path.clone();
        let old_range = update.old_range;
        let delta = update.line_delta;

        for card in self.cards.iter_mut().chain(self.redo_cards.iter_mut()) {
            if card.file_path == file_path && card.file_card_range == old_range {
                *card = update.updated_card.clone();
                continue;
            }

            if delta != 0
                && card.file_path == file_path
                && card.file_card_range.0 >= old_range.1
            {
                let start = (card.file_card_range.0 as isize + delta) as usize;
                let end = (card.file_card_range.1 as isize + delta) as usize;
                card.file_card_range = (start, end);
            }
        }
    }

    fn current_ai_pending(&self) -> bool {
        matches!(
            self.cards
                .get(self.current_idx)
                .map(|card| card.ai_status.clone()),
            Some(AIStatus::ClozeNeedDeletion | AIStatus::QuestionNeedRephrasing)
        )
    }
}

#[derive(Clone, Debug)]
struct AiUpdate {
    card_hash: String,
    card: Card,
}

#[derive(Clone, Debug)]
struct CardEditUpdate {
    updated_card: Card,
    old_range: (usize, usize),
    line_delta: isize,
}

async fn start_drill_session(
    db: &DB,
    cards: Vec<Card>,
    drill_preprocessor: DrillPreprocessor,
    retention: f32,
) -> Result<()> {
    enable_raw_mode().context("failed to enable raw mode")?;
    let mut stdout = io::stdout();
    execute!(
        stdout,
        EnterAlternateScreen,
        PushKeyboardEnhancementFlags(
            KeyboardEnhancementFlags::REPORT_ALL_KEYS_AS_ESCAPE_CODES
                | KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES
        )
    )
    .context("failed to configure terminal")?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend).context("failed to start terminal")?;
    terminal.hide_cursor().context("failed to hide cursor")?;

    let (ai_updates_tx, mut ai_updates_rx) = mpsc::unbounded_channel();
    let mut ai_preprocess_handle = if drill_preprocessor.llm_required() {
        let ai_cards = cards.clone();
        Some(tokio::spawn(async move {
            preprocess_cards_in_order(drill_preprocessor, ai_cards, ai_updates_tx).await
        }))
    } else {
        None
    };

    let mut state = DrillState::new(db, cards, retention);

    let loop_result: Result<()> = async {
        loop {
            if state.is_complete() {
                break Ok(());
            }

            while let Ok(update) = ai_updates_rx.try_recv() {
                state.apply_ai_update(update);
            }

            if let Some(handle) = &mut ai_preprocess_handle
                && handle.is_finished()
            {
                let result = handle
                    .await
                    .map_err(|err| anyhow!("AI preprocessing task failed: {err}"))?;
                if let Err(err) = result {
                    break Err(err);
                }
                ai_preprocess_handle = None;
            }

            terminal
                .draw(|frame| {
                    let card = state
                        .current_card()
                        .expect("card should exist while session is active");
                    let area = frame.area();
                    frame.render_widget(Theme::backdrop(), area);
                    let chunks = Layout::default()
                        .direction(Direction::Vertical)
                        .constraints([Constraint::Min(5), Constraint::Length(5)])
                        .split(area);

                    let mut header_vec = vec![
                        Theme::label_span(format!(
                            "Card {}/{}",
                            state.current_idx + 1,
                            state.cards.len()
                        )),
                        Theme::bullet(),
                        Theme::span(format!("{} coming again", state.redo_cards.len())),
                        Theme::bullet(),
                        Theme::span(card.file_path.display().to_string()),
                    ];
                    if card.ai_status == AIStatus::AiEnhanced {
                        header_vec.push(Theme::bullet());
                        header_vec.push(Theme::key_chip("AI enhanced"));
                    }
                    let header_line = Line::from(header_vec);

                    let ai_pending = state.current_ai_pending();
                    let content = if ai_pending {
                        "Enhancing this card with AI...\n\nPlease wait.".to_string()
                    } else {
                        format_card_text(&card, state.show_answer)
                    };
                    let markdown = render_markdown(&content);
                    state.current_medias = extract_media(&content, card.file_path.parent());

                    let card_widget = Paragraph::new(markdown)
                        .block(Theme::panel_with_line(header_line))
                        .wrap(Wrap { trim: false });
                    frame.render_widget(card_widget, chunks[0]);

                    let instructions = instructions_text(&state);
                    let footer = Paragraph::new(instructions)
                        .block(Theme::panel_with_line(Theme::section_header("Controls")));
                    frame.render_widget(footer, chunks[1]);
                })
                .context("failed to render frame")?;

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
                    break Ok(());
                }
                let ai_pending = state.current_ai_pending();
                match key.code {
                    KeyCode::Char(' ') | KeyCode::Enter if !ai_pending => {
                        if !state.show_answer {
                            state.reveal_answer();
                        } else {
                            state.handle_review(ReviewStatus::Pass).await?;
                        }
                    }
                    KeyCode::Char('F') | KeyCode::Char('f') if state.show_answer && !ai_pending => {
                        state.handle_review(ReviewStatus::Fail).await?;
                    }
                    KeyCode::Char('E') | KeyCode::Char('e') if !ai_pending => {
                        let edited = edit_current_card(&mut state, &mut terminal).await?;
                        if edited {
                            state.show_answer = false;
                        }
                    }
                    KeyCode::Char('O') | KeyCode::Char('o')
                        if !ai_pending
                            && !state.show_answer
                            && !state.current_medias.is_empty() =>
                    {
                        state.current_medias[0].play()?;
                    }

                    _ => {}
                }
            }
        }
    }
    .await;

    teardown_terminal(&mut terminal)?;

    loop_result
}

fn teardown_terminal(terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>) -> Result<()> {
    disable_raw_mode().context("failed to disable raw mode")?;
    execute!(
        terminal.backend_mut(),
        PopKeyboardEnhancementFlags,
        LeaveAlternateScreen
    )
    .context("failed to restore terminal")?;
    terminal.show_cursor().context("failed to show cursor")?;
    Ok(())
}

fn instructions_text(state: &DrillState<'_>) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    if state.current_ai_pending() {
        lines.push(Line::from(vec![
            Theme::span("Enhancing card with AI"),
            Theme::bullet(),
            Theme::key_chip("Esc"),
            Theme::span(" / "),
            Theme::key_chip("Ctrl+C"),
            Theme::span(" exit"),
        ]));
    } else if state.show_answer {
        lines.push(Line::from(vec![
            Theme::key_chip("Space"),
            Theme::span(" or "),
            Theme::key_chip("Enter"),
            Span::styled(" Pass", Theme::success()),
            Theme::bullet(),
            Theme::key_chip("F"),
            Span::styled(" Fail", Theme::danger()),
            Theme::bullet(),
            Theme::key_chip("E"),
            Theme::span(" edit"),
            Theme::bullet(),
            Theme::key_chip("Esc"),
            Theme::span(" / "),
            Theme::key_chip("Ctrl+C"),
            Theme::span(" exit"),
        ]));
    } else {
        let mut line = vec![
            Theme::key_chip("Space"),
            Theme::span(" or "),
            Theme::key_chip("Enter"),
            Theme::span(" show answer"),
            Theme::bullet(),
            Theme::key_chip("E"),
            Theme::span(" edit"),
            Theme::bullet(),
            Theme::key_chip("Esc"),
            Theme::span(" / "),
            Theme::key_chip("Ctrl+C"),
            Theme::span(" exit"),
        ];
        if !state.current_medias.is_empty() {
            let num_media = state.current_medias.len();
            line.push(Theme::bullet());
            line.push(Theme::span(format!(
                "{} found in card ",
                pluralize("media file", num_media)
            )));
            line.push(Theme::key_chip("O"));
            line.push(Theme::span(" open"));
        }
        lines.push(Line::from(line));
    }

    if let Some(action) = &state.last_action
        && action.last_reviewed_at.elapsed().as_secs_f64() < FLASH_SECS
    {
        let style = match action.action {
            ReviewStatus::Pass => Theme::success(),
            ReviewStatus::Fail => Theme::danger(),
        };
        lines.push(Line::from(vec![
            Theme::span("Last:"),
            Span::styled(action.print(), style),
        ]));
    }

    lines
}

fn format_card_text(card: &Card, show_answer: bool) -> String {
    match &card.content {
        CardContent::Basic { question, answer } => {
            let mut text = format!("Q:\n{}\n\nA:\n", question);
            if show_answer {
                text.push_str(answer);
            }
            text
        }
        CardContent::Cloze { text, cloze_range } => {
            let body = match (cloze_range, show_answer) {
                (Some(range), false) => mask_cloze_text(text, range),
                _ => text.clone(),
            };
            format!("C:\n{}", body)
        }
    }
}

async fn edit_current_card(
    state: &mut DrillState<'_>,
    terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
) -> Result<bool> {
    let Some(original) = state.current_card() else {
        return Ok(false);
    };
    let (card_type, editor_content) = card_to_edit_content(&original);
    let mut editor = Editor::from_content(card_type, &editor_content);
    let mut status: Option<String> = None;
    let mut status_time: Option<Instant> = None;
    let mut view_height = 0usize;

    terminal.show_cursor().context("failed to show cursor")?;

    let edit_result: Result<bool> = async {
        loop {
            terminal
                .draw(|frame| {
                    let area = frame.area();
                    frame.render_widget(Theme::backdrop(), area);
                    let chunks = Layout::default()
                        .direction(Direction::Vertical)
                        .constraints([Constraint::Min(5), Constraint::Length(5)])
                        .split(area);

                    view_height = chunks[0].height.saturating_sub(2) as usize;
                    editor.ensure_cursor_visible(view_height.max(1));

                    let editor_block = Theme::panel(original.file_path.display().to_string());
                    let editor_widget = Paragraph::new(editor.content())
                        .block(editor_block)
                        .wrap(Wrap { trim: false })
                        .scroll((editor.scroll_top() as u16, 0));
                    frame.render_widget(editor_widget, chunks[0]);

                    let mut help_lines = vec![Line::from(vec![
                        Theme::key_chip("Ctrl+S"),
                        Theme::span(" save"),
                        Theme::bullet(),
                        Theme::key_chip("Esc"),
                        Theme::span(" / "),
                        Theme::key_chip("Ctrl+C"),
                        Theme::span(" cancel"),
                    ])];

                    help_lines.push(Line::from(vec![
                        Theme::span("Editing current card"),
                        Theme::bullet(),
                        Theme::span(original.file_path.display().to_string()),
                    ]));

                    if let Some(time) = status_time
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
                        .block(Theme::panel_with_line(Theme::section_header("Edit")));
                    frame.render_widget(instructions, chunks[1]);

                    let (cursor_row, cursor_col) = editor.cursor();
                    let visible_row = cursor_row.saturating_sub(editor.scroll_top());
                    let cursor_x =
                        chunks[0].x + 1 + (cursor_col as u16).min(chunks[0].width.saturating_sub(2));
                    let cursor_y =
                        chunks[0].y + 1 + (visible_row as u16).min(chunks[0].height.saturating_sub(2));
                    frame.set_cursor_position((cursor_x, cursor_y));
                })
                .context("failed to render edit frame")?;

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
                    break Ok(false);
                }

                if key.code == KeyCode::Char('s') && key.modifiers.contains(KeyModifiers::CONTROL) {
                    let contents = editor.content();
                    match save_edited_card(state.db, &original, &contents).await {
                        Ok(update) => {
                            state.apply_edit_update(update);
                            break Ok(true);
                        }
                        Err(err) => {
                            status_time = Some(Instant::now());
                            let flat_error = err
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
    }
    .await;

    terminal.hide_cursor().context("failed to hide cursor")?;

    edit_result
}

async fn save_edited_card(db: &DB, original: &Card, contents: &str) -> Result<CardEditUpdate> {
    let mut updated_card = content_to_card(
        &original.file_path,
        contents,
        original.file_card_range.0,
        original.file_card_range.1,
    )?;

    let original_contents = fs::read_to_string(&original.file_path).with_context(|| {
        format!(
            "Failed to read card file at {}",
            original.file_path.display()
        )
    })?;

    let ends_with_newline = original_contents.ends_with('\n');
    let mut lines: Vec<String> = original_contents
        .lines()
        .map(|line| line.to_string())
        .collect();

    let mut range = original.file_card_range;
    let needs_resolve = range.0 >= range.1
        || range.1 > lines.len()
        || {
            let slice = lines[range.0.min(lines.len())..range.1.min(lines.len())].join("\n");
            match content_to_card(&original.file_path, &slice, range.0, range.1) {
                Ok(card) => card.card_hash != original.card_hash,
                Err(_) => true,
            }
        };
    if needs_resolve {
        range = resolve_card_range(&original.file_path, &original.card_hash)?;
    }

    let new_lines: Vec<String> = contents.split('\n').map(|line| line.to_string()).collect();
    let old_line_count = range.1.saturating_sub(range.0);
    let new_line_count = new_lines.len();

    if updated_card.card_hash != original.card_hash && db.card_exists(&updated_card).await? {
        bail!("A card with the same content already exists.");
    }

    lines.splice(range.0..range.1, new_lines);
    let mut updated_contents = lines.join("\n");
    if ends_with_newline {
        updated_contents.push('\n');
    }
    fs::write(&original.file_path, updated_contents).with_context(|| {
        format!(
            "Failed to write card file at {}",
            original.file_path.display()
        )
    })?;

    if updated_card.card_hash != original.card_hash {
        if let Err(err) = db
            .update_card_hash(&original.card_hash, &updated_card.card_hash)
            .await
        {
            let _ = fs::write(&original.file_path, original_contents);
            return Err(err);
        }
    }

    let new_range = (range.0, range.0 + new_line_count);
    updated_card.file_card_range = new_range;
    updated_card.ai_status = AIStatus::NoNeed;

    Ok(CardEditUpdate {
        updated_card,
        old_range: range,
        line_delta: new_line_count as isize - old_line_count as isize,
    })
}

fn resolve_card_range(path: &Path, card_hash: &str) -> Result<(usize, usize)> {
    let cards = cards_from_md(path)?;
    let mut matches = cards.into_iter().filter(|card| card.card_hash == card_hash);
    let Some(card) = matches.next() else {
        bail!("Unable to locate the current card in {}", path.display());
    };
    if matches.next().is_some() {
        bail!(
            "Multiple cards in {} share the same hash; edit is ambiguous.",
            path.display()
        );
    }
    Ok(card.file_card_range)
}

fn card_to_edit_content(card: &Card) -> (CardType, String) {
    match &card.content {
        CardContent::Basic { question, answer } => {
            let mut lines = prefixed_lines("Q: ", question);
            lines.extend(prefixed_lines("A: ", answer));
            (CardType::Basic, lines.join("\n"))
        }
        CardContent::Cloze { text, .. } => {
            let lines = prefixed_lines("C: ", text);
            (CardType::Cloze, lines.join("\n"))
        }
    }
}

fn prefixed_lines(prefix: &str, text: &str) -> Vec<String> {
    let mut iter = text.split('\n');
    let first = iter.next().unwrap_or("");
    let mut lines = Vec::new();
    lines.push(format!("{prefix}{first}"));
    lines.extend(iter.map(|line| line.to_string()));
    lines
}

async fn preprocess_cards_in_order(
    drill_preprocessor: DrillPreprocessor,
    cards: Vec<Card>,
    updates: mpsc::UnboundedSender<AiUpdate>,
) -> Result<()> {
    for card in cards.into_iter() {
        let needs_ai = matches!(
            card.ai_status,
            AIStatus::ClozeNeedDeletion | AIStatus::QuestionNeedRephrasing
        );
        if !needs_ai {
            continue;
        }

        let mut updated_card = card.clone();
        drill_preprocessor
            .preprocess_cards(std::slice::from_mut(&mut updated_card))
            .await?;

        let _ = updates.send(AiUpdate {
            card_hash: updated_card.card_hash.clone(),
            card: updated_card,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::card::ClozeRange;

    use super::*;
    use std::path::PathBuf;
    use std::time::Instant;

    fn basic_card(question: &str, answer: &str) -> Card {
        let content = CardContent::Basic {
            question: question.into(),
            answer: answer.into(),
        };
        Card::new(PathBuf::from("test.md"), (0, 1), content, "hash".into())
    }

    fn cloze_card(text: &str) -> Card {
        let start = text.find('[').unwrap();
        let end = text[start..].find(']').unwrap() + start + 1;
        Card::new(
            PathBuf::from("test.md"),
            (0, 1),
            CardContent::Cloze {
                text: text.into(),
                cloze_range: Some(ClozeRange::new(start, end).unwrap()),
            },
            "hash".into(),
        )
    }

    #[test]
    fn basic_card_hides_answer_until_revealed() {
        let card = basic_card("What?", "Answer");

        let hidden = format_card_text(&card, false);
        assert!(!hidden.contains("Answer"));

        let shown = format_card_text(&card, true);
        assert!(shown.contains("Answer"));
    }

    #[test]
    fn cloze_card_masks_until_answer_shown() {
        let card = cloze_card("Value [東京]");

        let masked = format_card_text(&card, false);
        let placeholder = extract_placeholder(&masked);
        assert!(placeholder.chars().all(|c| c == '_'));
        assert!(placeholder.chars().count() >= 3);

        let revealed = format_card_text(&card, true);
        assert!(revealed.contains("[東京]"));
    }

    #[test]
    fn last_action_prints_human_friendly_intervals() {
        fn formatted(minutes: f64, status: ReviewStatus) -> String {
            let action = LastAction {
                action: status,
                show_again_duration: minutes / MINUTES_PER_DAY,
                last_reviewed_at: Instant::now(),
            };
            action.print()
        }

        assert_eq!(
            formatted(10.0, ReviewStatus::Pass),
            " Pass (See again in <15 mins)"
        );
        assert_eq!(
            formatted(20.0, ReviewStatus::Pass),
            " Pass (See again in <30 mins)"
        );
        assert_eq!(
            formatted(60.0, ReviewStatus::Pass),
            " Pass (See again in <12 hours)"
        );
        assert_eq!(
            formatted(22.0 * 60.0, ReviewStatus::Pass),
            " Pass (See again in <1 day)"
        );
        assert_eq!(
            formatted(3.0 * MINUTES_PER_DAY, ReviewStatus::Fail),
            " Fail (See again in 3 days)"
        );
    }

    #[test]
    fn instructions_show_answer_branch_includes_pass_and_fail() {
        let db = in_memory_db();
        let mut state = DrillState::new(&db, vec![basic_card("Q", "A")], 0.9);
        state.show_answer = true;

        let lines = instructions_text(&state);
        let commands = flatten_line(&lines[0]);

        assert!(commands.contains("Pass"));
        assert!(commands.contains("Fail"));
    }

    #[test]
    fn recent_last_action_is_displayed_in_instructions() {
        let db = in_memory_db();
        let mut state = DrillState::new(&db, vec![basic_card("Q", "A")], 0.9);
        state.show_answer = true;
        state.last_action = Some(LastAction {
            action: ReviewStatus::Fail,
            show_again_duration: 0.0,
            last_reviewed_at: Instant::now(),
        });

        let lines = instructions_text(&state);
        assert!(lines.len() >= 2);

        let last_line = flatten_line(lines.last().unwrap());
        assert!(last_line.contains("Last:"));
        assert!(last_line.contains("Fail"));
    }

    fn extract_placeholder(text: &str) -> String {
        let start = text.find('[').unwrap();
        let end = text[start..].find(']').unwrap() + start;
        text[start + 1..end].to_string()
    }

    fn flatten_line(line: &Line<'_>) -> String {
        line.spans
            .iter()
            .map(|span| span.content.to_string())
            .collect::<String>()
    }

    fn in_memory_db() -> DB {
        tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(DB::new_in_memory())
            .unwrap()
    }
}
