use std::io;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use crate::card::{Card, CardContent};
use crate::cloze_utils::mask_cloze_text;
use crate::crud::DB;
use crate::fsrs::{LEARN_AHEAD_THRESHOLD_MINS, ReviewStatus};
use crate::llm::drill_preprocessor::{AIStatus, DrillPreprocessor};
use crate::notes::register_apple_notes_cards;
use crate::palette::Palette;
use crate::parser::register_all_cards;
use crate::parser::render_markdown;
use crate::parser::{Media, extract_media};
use crate::tui::{CardImage, ImageRenderer, InlineImages, Theme, image_widget, split_card_area};
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

pub struct DrillOptions {
    pub paths: Vec<PathBuf>,
    pub card_limit: Option<usize>,
    pub new_card_limit: Option<usize>,
    pub rephrase_questions: bool,
    pub shuffle: bool,
    pub retention: f32,
    pub apple_notes: bool,
    pub inline_images: InlineImages,
}

pub async fn run(db: &DB, opts: DrillOptions) -> Result<()> {
    validate_retention(opts.retention)?;
    let (hash_cards, _) = if opts.apple_notes {
        register_apple_notes_cards(db).await?
    } else {
        register_all_cards(db, opts.paths).await?
    };
    let mut cards_due_today = db
        .due_today(&hash_cards, opts.card_limit, opts.new_card_limit)
        .await?;

    if opts.shuffle {
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

    let drill_preprocessor =
        DrillPreprocessor::new(&cards_due_today, opts.rephrase_questions).await?;
    drill_preprocessor.initialize_card_status(&mut cards_due_today);
    start_drill_session(
        db,
        cards_due_today,
        drill_preprocessor,
        opts.retention,
        opts.shuffle,
        opts.inline_images,
    )
    .await?;

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
    visible_key: Option<(String, Option<PathBuf>)>,
    card_image: CardImage,
    zoom_image: bool,
    retention: f32,
    shuffle: bool,
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
    fn new(db: &'a DB, cards: Vec<Card>, retention: f32, shuffle: bool) -> Self {
        Self {
            db,
            cards,
            redo_cards: Vec::new(),
            current_idx: 0,
            show_answer: false,
            last_action: None,
            current_medias: Vec::new(),
            visible_key: None,
            card_image: CardImage::Absent,
            zoom_image: false,
            retention,
            shuffle,
        }
    }

    fn sync_visible(&mut self, renderer: Option<&ImageRenderer>) -> (Card, String) {
        let card = self
            .current_card()
            .expect("card should exist while session is active");

        let content = if self.current_ai_pending() {
            "Enhancing this card with AI...\n\nPlease wait.".to_string()
        } else {
            format_card_text(&card, self.show_answer)
        };
        let base_dir = card.file_path.parent().map(|dir| dir.to_path_buf());

        let key = (content.clone(), base_dir.clone());
        if self.visible_key.as_ref() == Some(&key) {
            return (card, content);
        }

        self.current_medias = extract_media(&content, base_dir.as_deref());
        self.zoom_image = false;
        self.card_image = match (renderer, self.current_medias.iter().find(|m| m.is_image())) {
            (Some(renderer), Some(media)) => match renderer.load(media.path()) {
                Ok(protocol) => CardImage::Ready(Box::new(protocol)),
                Err(reason) => CardImage::Failed(reason),
            },
            _ => CardImage::Absent,
        };
        self.visible_key = Some(key);

        (card, content)
    }

    fn media_to_open(&self) -> Option<&Media> {
        if self.card_image.is_ready() {
            self.current_medias.iter().find(|media| media.is_image())
        } else {
            self.current_medias.first()
        }
    }

    fn current_card(&mut self) -> Option<Card> {
        if self.current_idx >= self.cards.len() {
            if self.redo_cards.is_empty() {
                return None;
            }
            if self.shuffle {
                use rand::seq::SliceRandom;
                self.redo_cards.shuffle(&mut rand::rng());
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

async fn start_drill_session(
    db: &DB,
    cards: Vec<Card>,
    drill_preprocessor: DrillPreprocessor,
    retention: f32,
    shuffle: bool,
    inline_images: InlineImages,
) -> Result<()> {
    enable_raw_mode().context("failed to enable raw mode")?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen).context("failed to configure terminal")?;

    let image_renderer = ImageRenderer::detect(inline_images);

    execute!(
        stdout,
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

    let mut state = DrillState::new(db, cards, retention, shuffle);

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

            let (card, content) = state.sync_visible(image_renderer.as_ref());
            let header_line = header_line(&state, &card);
            let body = render_markdown(&content);
            let instructions = instructions_text(&state, image_renderer.as_ref());
            let zoom = state.zoom_image;

            let card_image = &mut state.card_image;
            terminal
                .draw(|frame| {
                    let area = frame.area();
                    frame.render_widget(Theme::backdrop(), area);
                    let chunks = Layout::default()
                        .direction(Direction::Vertical)
                        .constraints([Constraint::Min(5), Constraint::Length(5)])
                        .split(area);

                    let block = Theme::panel_with_line(header_line);
                    let inner = block.inner(chunks[0]);
                    frame.render_widget(block, chunks[0]);

                    let (text_area, image_area) =
                        split_card_area(inner, card_image.is_ready(), zoom);
                    frame.render_widget(Paragraph::new(body).wrap(Wrap { trim: false }), text_area);
                    if let (Some(image_area), Some(protocol)) =
                        (image_area, card_image.protocol_mut())
                    {
                        frame.render_stateful_widget(image_widget(), image_area, protocol);
                    }

                    let footer = Paragraph::new(instructions)
                        .block(Theme::panel_with_line(Theme::section_header("Controls")));
                    frame.render_widget(footer, chunks[1]);
                })
                .context("failed to render frame")?;

            if let Some(protocol) = state.card_image.protocol_mut()
                && let Some(Err(err)) = protocol.last_encoding_result()
            {
                state.card_image = CardImage::Failed(format!("{err}"));
            }

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
                    KeyCode::Char('O') | KeyCode::Char('o') if !ai_pending => {
                        if let Some(media) = state.media_to_open() {
                            media.play()?;
                        }
                    }
                    KeyCode::Char('I') | KeyCode::Char('i')
                        if !ai_pending && state.card_image.is_ready() =>
                    {
                        state.zoom_image = !state.zoom_image;
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

fn header_line(state: &DrillState<'_>, card: &Card) -> Line<'static> {
    let mut spans = vec![
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
        spans.push(Theme::bullet());
        spans.push(Theme::key_chip("AI enhanced"));
    }
    Line::from(spans)
}

fn media_hint(state: &DrillState<'_>, renderer: Option<&ImageRenderer>) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    if state.current_medias.is_empty() {
        return spans;
    }

    spans.push(Theme::span(format!(
        "{} found in card ",
        pluralize("media file", state.current_medias.len())
    )));
    spans.push(Theme::key_chip("O"));
    spans.push(Theme::span(" open"));

    if state.card_image.is_ready() {
        spans.push(Theme::bullet());
        spans.push(Theme::key_chip("i"));
        spans.push(Theme::span(if state.zoom_image { " fit" } else { " zoom" }));
        if let Some(renderer) = renderer {
            spans.push(Theme::span(format!(" ({})", renderer.protocol_name())));
        }
    } else if let Some(reason) = state.card_image.failure() {
        spans.push(Theme::bullet());
        spans.push(Span::styled(
            format!("image not shown: {reason}"),
            Theme::danger(),
        ));
    }

    spans
}

fn instructions_text(
    state: &DrillState<'_>,
    renderer: Option<&ImageRenderer>,
) -> Vec<Line<'static>> {
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
            Theme::key_chip("Esc"),
            Theme::span(" / "),
            Theme::key_chip("Ctrl+C"),
            Theme::span(" exit"),
        ]));
    } else {
        lines.push(Line::from(vec![
            Theme::key_chip("Space"),
            Theme::span(" or "),
            Theme::key_chip("Enter"),
            Theme::span(" show answer"),
            Theme::bullet(),
            Theme::key_chip("Esc"),
            Theme::span(" / "),
            Theme::key_chip("Ctrl+C"),
            Theme::span(" exit"),
        ]));
    }

    if !state.current_medias.is_empty() {
        lines.push(Line::from(media_hint(state, renderer)));
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
    use crate::cloze_utils::find_cloze_ranges;

    use super::*;
    use std::path::PathBuf;
    use std::time::Instant;

    fn basic_card(question: &str, answer: &str) -> Card {
        basic_card_with_hash(question, answer, "hash")
    }

    fn basic_card_with_hash(question: &str, answer: &str, hash: &str) -> Card {
        let content = CardContent::Basic {
            question: question.into(),
            answer: answer.into(),
        };
        Card::new(PathBuf::from("test.md"), (0, 1), content, hash.into())
    }

    fn cloze_card(text: &str, blank_index: usize) -> Card {
        let (start, end) = find_cloze_ranges(text)[blank_index];
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
        let card = cloze_card("Value [東京]", 0);

        let masked = format_card_text(&card, false);
        let placeholder = extract_placeholder(&masked);
        assert!(placeholder.chars().all(|c| c == '_'));
        assert!(placeholder.chars().count() >= 3);

        let revealed = format_card_text(&card, true);
        assert!(revealed.contains("[東京]"));
    }

    #[test]
    fn cloze_card_masks_second_blank_and_leaves_first_visible() {
        let card = cloze_card(
            "The [order] of a group is [the cardinality of its underlying set].",
            1,
        );

        let masked = format_card_text(&card, false);
        assert!(masked.contains("[order]"));
        assert!(!masked.contains("the cardinality of its underlying set"));

        let second_blank_start = masked.rfind('[').unwrap();
        let second_blank_end = masked[second_blank_start..].find(']').unwrap() + second_blank_start;
        let placeholder = &masked[second_blank_start + 1..second_blank_end];
        assert!(placeholder.chars().all(|c| c == '_'));

        let revealed = format_card_text(&card, true);
        assert!(revealed.contains("[order]"));
        assert!(revealed.contains("[the cardinality of its underlying set]"));
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
        let mut state = DrillState::new(&db, vec![basic_card("Q", "A")], 0.9, false);
        state.show_answer = true;

        let lines = instructions_text(&state, None);
        let commands = flatten_line(&lines[0]);

        assert!(commands.contains("Pass"));
        assert!(commands.contains("Fail"));
    }

    #[test]
    fn recent_last_action_is_displayed_in_instructions() {
        let db = in_memory_db();
        let mut state = DrillState::new(&db, vec![basic_card("Q", "A")], 0.9, false);
        state.show_answer = true;
        state.last_action = Some(LastAction {
            action: ReviewStatus::Fail,
            show_again_duration: 0.0,
            last_reviewed_at: Instant::now(),
        });

        let lines = instructions_text(&state, None);
        assert!(lines.len() >= 2);

        let last_line = flatten_line(lines.last().unwrap());
        assert!(last_line.contains("Last:"));
        assert!(last_line.contains("Fail"));
    }

    #[test]
    fn sync_visible_resolves_media_once_per_card_side() {
        let db = in_memory_db();
        let card = basic_card(
            "What is this? ![diagram](wave.png)",
            "A wave [sound](t.mp3)",
        );
        let mut state = DrillState::new(&db, vec![card], 0.9, false);

        let (_, question) = state.sync_visible(None);
        assert!(question.contains("wave.png"));
        assert_eq!(
            state.current_medias.len(),
            1,
            "answer media not yet visible"
        );

        // Re-running on an unchanged card must be a no-op, not a re-parse: this is what
        // keeps the work out of the ~60fps render path.
        let key_before = state.visible_key.clone();
        state.sync_visible(None);
        assert_eq!(state.visible_key, key_before);

        state.reveal_answer();
        state.sync_visible(None);
        assert_ne!(
            state.visible_key, key_before,
            "reveal must re-resolve media"
        );
        assert_eq!(
            state.current_medias.len(),
            2,
            "answer-side media should now be found"
        );
    }

    #[test]
    fn media_hint_appears_in_both_question_and_answer_states() {
        let db = in_memory_db();
        let card = basic_card("Q ![diagram](wave.png)", "A");
        let mut state = DrillState::new(&db, vec![card], 0.9, false);

        state.sync_visible(None);
        let question_footer = flatten_footer(&state, None);
        assert!(
            question_footer.contains("media file found in card"),
            "{question_footer}"
        );

        state.reveal_answer();
        state.sync_visible(None);
        let answer_footer = flatten_footer(&state, None);
        assert!(
            answer_footer.contains("media file found in card"),
            "media must stay openable after the answer is revealed: {answer_footer}"
        );
    }

    #[test]
    fn footer_omits_the_media_line_when_a_card_has_no_attachments() {
        let db = in_memory_db();
        let mut state = DrillState::new(&db, vec![basic_card("Q", "A")], 0.9, false);
        state.sync_visible(None);

        assert_eq!(
            instructions_text(&state, None).len(),
            1,
            "a plain card should not gain an empty media line"
        );
    }

    /// The media chips were once appended to the review-keys line, which overran the panel
    /// and silently truncated them. Render the real footer widget and read the buffer back,
    /// so a regression shows up as missing text rather than as a passing span assertion.
    #[test]
    fn footer_chips_survive_rendering_at_a_normal_terminal_width() {
        use ratatui::{Terminal, backend::TestBackend, layout::Rect, widgets::Paragraph};

        let db = in_memory_db();
        let card = basic_card("Q ![d](test_data/synaptic_vessel.jpg)", "A");
        let mut state = DrillState::new(&db, vec![card], 0.9, false);
        let renderer = ImageRenderer::halfblocks();
        state.sync_visible(Some(&renderer));
        assert!(state.card_image.is_ready());

        let instructions = instructions_text(&state, Some(&renderer));
        let mut terminal = Terminal::new(TestBackend::new(100, 8)).unwrap();
        terminal
            .draw(|frame| {
                // Exactly how the drill loop builds the footer — no wrapping, so an
                // over-long line truncates at the panel edge instead of flowing.
                let footer = Paragraph::new(instructions)
                    .block(Theme::panel_with_line(Theme::section_header("Controls")));
                frame.render_widget(footer, Rect::new(0, 0, 100, 5));
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let rendered: String = (0..5)
            .flat_map(|y| (0..100).map(move |x| (x, y)))
            .map(|(x, y)| buffer[(x, y)].symbol().to_string())
            .collect();

        for needle in ["show answer", "media file found in card", "O", "i", "zoom"] {
            assert!(
                rendered.contains(needle),
                "footer lost {needle:?} when rendered at 100 columns:\n{rendered}"
            );
        }
    }

    #[test]
    fn footer_reports_why_an_image_could_not_be_shown() {
        let db = in_memory_db();
        let card = basic_card("Q ![diagram](wave.png)", "A");
        let mut state = DrillState::new(&db, vec![card], 0.9, false);
        state.sync_visible(None);
        state.card_image = CardImage::Failed("unreadable".to_string());

        let footer = flatten_footer(&state, None);
        assert!(footer.contains("image not shown: unreadable"), "{footer}");
    }

    #[test]
    fn open_target_prefers_the_displayed_image_over_a_leading_audio_file() {
        let db = in_memory_db();
        // Audio first, image second, so the first media and the first *image* differ.
        let card = basic_card(
            "Q [clip](a.mp3) ![diagram](test_data/synaptic_vessel.jpg)",
            "A",
        );
        let mut state = DrillState::new(&db, vec![card], 0.9, false);

        state.sync_visible(None);
        assert!(
            state.media_to_open().unwrap().path().ends_with("a.mp3"),
            "with no image displayed, O opens the first attachment"
        );

        // Now with a real decoded image on screen, `O` must open what is displayed.
        let renderer = ImageRenderer::halfblocks();
        state.visible_key = None;
        state.sync_visible(Some(&renderer));
        assert!(
            state.card_image.is_ready(),
            "fixture should decode via halfblocks"
        );
        assert!(
            state
                .media_to_open()
                .unwrap()
                .path()
                .ends_with("synaptic_vessel.jpg"),
            "O must open the picture the user can see"
        );
    }

    #[test]
    fn footer_offers_the_zoom_key_only_when_an_image_is_displayed() {
        let db = in_memory_db();
        let card = basic_card("Q ![d](test_data/synaptic_vessel.jpg)", "A");
        let mut state = DrillState::new(&db, vec![card], 0.9, false);
        let renderer = ImageRenderer::halfblocks();

        state.sync_visible(None);
        assert!(!flatten_footer(&state, None).contains("zoom"));

        state.visible_key = None;
        state.sync_visible(Some(&renderer));
        let footer = flatten_footer(&state, Some(&renderer));
        assert!(footer.contains("zoom"), "{footer}");
        assert!(footer.contains("halfblocks"), "{footer}");

        state.zoom_image = true;
        let footer = flatten_footer(&state, Some(&renderer));
        assert!(footer.contains("fit"), "{footer}");
    }

    #[tokio::test]
    async fn redo_queue_keeps_insertion_order_when_shuffle_disabled() {
        let db = DB::new_in_memory().await.unwrap();
        let cards = vec![
            basic_card_with_hash("A?", "A", "hash-a"),
            basic_card_with_hash("B?", "B", "hash-b"),
            basic_card_with_hash("C?", "C", "hash-c"),
        ];
        db.add_cards_batch(&cards).await.unwrap();

        let mut state = DrillState::new(&db, cards.clone(), 0.9, false);
        for _ in 0..cards.len() {
            state.handle_review(ReviewStatus::Fail).await.unwrap();
        }

        let current = state.current_card().unwrap();
        assert_eq!(current.card_hash, "hash-a");

        let order = state
            .cards
            .iter()
            .map(|card| card.card_hash.as_str())
            .collect::<Vec<_>>();
        assert_eq!(order, vec!["hash-a", "hash-b", "hash-c"]);
        assert!(state.redo_cards.is_empty());
    }

    fn extract_placeholder(text: &str) -> String {
        let start = text.find('[').unwrap();
        let end = text[start..].find(']').unwrap() + start;
        text[start + 1..end].to_string()
    }

    /// The whole footer as one string, so tests do not depend on which line a chip sits on.
    fn flatten_footer(state: &DrillState<'_>, renderer: Option<&ImageRenderer>) -> String {
        instructions_text(state, renderer)
            .iter()
            .map(flatten_line)
            .collect::<Vec<_>>()
            .join("\n")
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
