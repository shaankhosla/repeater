use std::io;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use crate::card::{Card, CardContent, ClozeRange};
use crate::crud::DB;
use crate::fsrs::{LEARN_AHEAD_THRESHOLD_MINS, ReviewStatus};
use crate::markdown::render_markdown;
use crate::theme::Theme;
use crate::utils::register_all_cards;

use anyhow::{Context, Result};
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

const MINUTES_PER_DAY: f64 = 24.0 * 60.0;
const FLASH_SECS: f64 = 2.0;

pub async fn run(
    db: &DB,
    paths: Vec<PathBuf>,
    card_limit: Option<usize>,
    new_card_limit: Option<usize>,
) -> Result<()> {
    let hash_cards = register_all_cards(db, paths).await?;
    let cards_due_today = db.due_today(hash_cards, card_limit, new_card_limit).await?;

    if cards_due_today.is_empty() {
        println!("All caught up—no cards due today.");
        return Ok(());
    }

    start_drill_session(db, cards_due_today).await?;

    Ok(())
}

struct DrillState<'a> {
    db: &'a DB,
    cards: Vec<Card>,
    redo_cards: Vec<Card>,
    current_idx: usize,
    show_answer: bool,
    last_action: Option<LastAction>,
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
    fn new(db: &'a DB, cards: Vec<Card>) -> Self {
        Self {
            db,
            cards,
            redo_cards: Vec::new(),
            current_idx: 0,
            show_answer: false,
            last_action: None,
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
            .update_card_performance(&current_card, action, None)
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
}

async fn start_drill_session(db: &DB, cards: Vec<Card>) -> Result<()> {
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

    let mut state = DrillState::new(db, cards);

    let loop_result: Result<()> = async {
        loop {
            if state.is_complete() {
                break Ok(());
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

                    let header_line = Line::from(vec![
                        Theme::label_span(format!(
                            "Card {}/{}",
                            state.current_idx + 1,
                            state.cards.len()
                        )),
                        Theme::bullet(),
                        Span::styled(
                            format!("{} coming again", state.redo_cards.len()),
                            Theme::muted(),
                        ),
                        Theme::bullet(),
                        Span::styled(card.file_path.display().to_string(), Theme::muted()),
                    ]);

                    let content = format_card_text(&card, state.show_answer);
                    let markdown = render_markdown(&content);
                    let card_widget = Paragraph::new(markdown)
                        .style(Theme::body())
                        .block(Theme::panel_with_line(header_line))
                        .wrap(Wrap { trim: false });
                    frame.render_widget(card_widget, chunks[0]);

                    let instructions = instructions_text(&state);
                    let footer = Paragraph::new(instructions)
                        .style(Theme::body())
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
                match key.code {
                    KeyCode::Char(' ') | KeyCode::Enter => {
                        if !state.show_answer {
                            state.reveal_answer();
                        }
                    }
                    KeyCode::Char('1') if state.show_answer => {
                        state.handle_review(ReviewStatus::Fail).await?;
                    }
                    KeyCode::Char('2') if state.show_answer => {
                        state.handle_review(ReviewStatus::Pass).await?;
                    }
                    _ => {}
                }
            }
        }
    }
    .await;

    disable_raw_mode().context("failed to disable raw mode")?;
    execute!(
        terminal.backend_mut(),
        PopKeyboardEnhancementFlags,
        LeaveAlternateScreen
    )
    .context("failed to restore terminal")?;
    terminal.show_cursor().context("failed to show cursor")?;

    loop_result
}

fn instructions_text(state: &DrillState<'_>) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    if state.show_answer {
        lines.push(Line::from(vec![
            Theme::key_chip("1"),
            Span::styled(" Fail", Theme::danger()),
            Theme::bullet(),
            Theme::key_chip("2"),
            Span::styled(" Pass", Theme::success()),
            Theme::bullet(),
            Theme::key_chip("Esc"),
            Span::styled(" / ", Theme::muted()),
            Theme::key_chip("Ctrl+C"),
            Span::styled(" exit", Theme::muted()),
        ]));
    } else {
        lines.push(Line::from(vec![
            Theme::key_chip("Space"),
            Span::styled(" or ", Theme::muted()),
            Theme::key_chip("Enter"),
            Span::styled(" show answer", Theme::muted()),
            Theme::bullet(),
            Theme::key_chip("Esc"),
            Span::styled(" / ", Theme::muted()),
            Theme::key_chip("Ctrl+C"),
            Span::styled(" exit", Theme::muted()),
        ]));
    }

    if let Some(action) = &state.last_action
        && action.last_reviewed_at.elapsed().as_secs_f64() < FLASH_SECS
    {
        let style = match action.action {
            ReviewStatus::Pass => Theme::success(),
            ReviewStatus::Fail => Theme::danger(),
        };
        lines.push(Line::from(vec![
            Theme::muted_span("Last:"),
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
            format!("Cloze:\n{}", body)
        }
    }
}

fn mask_cloze_text(text: &str, range: &ClozeRange) -> String {
    let start = range.start;
    let end = range.end;
    let hidden_section = &text[start..end];
    let core = hidden_section.trim_start_matches('[').trim_end_matches(']');
    let placeholder = "_".repeat(core.chars().count().max(3));

    let masked = format!("{}[{}]{}", &text[..start], placeholder, &text[end..]);
    masked
}

#[cfg(test)]
mod tests {
    use crate::utils::find_cloze_ranges;

    use super::*;
    use std::path::PathBuf;

    fn basic_card(question: &str, answer: &str) -> Card {
        Card {
            file_path: PathBuf::from("test.md"),
            file_card_range: (0, 1),
            content: CardContent::Basic {
                question: question.into(),
                answer: answer.into(),
            },
            card_hash: "hash".into(),
        }
    }

    fn cloze_card(text: &str) -> Card {
        let start = text.find('[').unwrap();
        let end = text[start..].find(']').unwrap() + start + 1;
        Card {
            file_path: PathBuf::from("test.md"),
            file_card_range: (0, 1),
            content: CardContent::Cloze {
                text: text.into(),
                cloze_range: Some(ClozeRange::new(start, end).unwrap()),
            },
            card_hash: "hash".into(),
        }
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
    fn mask_cloze_text_handles_unicode_and_bad_ranges() {
        let text = "Capital of 日本 is [東京]";

        let cloze_idxs = find_cloze_ranges(text);
        let range: ClozeRange = cloze_idxs
            .first()
            .map(|(start, end)| ClozeRange::new(*start, *end))
            .transpose()
            .unwrap()
            .unwrap();
        let masked = mask_cloze_text(text, &range);
        dbg!(&masked);
        assert_eq!(masked, "Capital of 日本 is [___]");
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

    fn extract_placeholder(text: &str) -> String {
        let start = text.find('[').unwrap();
        let end = text[start..].find(']').unwrap() + start;
        text[start + 1..end].to_string()
    }
}
