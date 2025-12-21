use crate::{
    crud::{CardStats, DB},
    drill::register_all_cards,
    theme::Theme,
};

use std::{io, time::Duration};

use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Frame, Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    text::{Line, Span},
    widgets::{List, ListItem, Paragraph, Wrap},
};

pub async fn run(db: &DB, paths: Vec<String>) -> Result<usize> {
    let card_hashes = register_all_cards(db, paths).await?;
    let count = card_hashes.len();
    let stats = db.collection_stats(&card_hashes).await?;
    render_dashboard(&stats)?;
    Ok(count)
}

fn render_dashboard(stats: &CardStats) -> Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.hide_cursor()?;

    let draw_result = dashboard_loop(&mut terminal, stats);

    terminal.show_cursor()?;
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;

    draw_result
}

fn dashboard_loop<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    stats: &CardStats,
) -> Result<()> {
    loop {
        terminal.draw(|frame| draw_dashboard(frame, stats))?;

        if event::poll(Duration::from_millis(200))?
            && let Event::Key(key) = event::read()?
        {
            if key.kind != KeyEventKind::Press {
                continue;
            }
            let exit_ctrl_c =
                key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL);
            if key.code == KeyCode::Esc || exit_ctrl_c {
                break;
            }
        }
    }
    Ok(())
}

fn draw_dashboard(frame: &mut Frame<'_>, stats: &CardStats) {
    let area = frame.area();
    frame.render_widget(Theme::backdrop(), area);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(7),
            Constraint::Min(6),
            Constraint::Length(3),
        ])
        .split(area);

    let summary = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
        .split(rows[0]);

    frame.render_widget(collection_panel(stats), summary[0]);
    frame.render_widget(due_panel(stats), summary[1]);

    let mid = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(rows[1]);

    frame.render_widget(upcoming_panel(stats), mid[0]);
    frame.render_widget(highlights_panel(stats), mid[1]);

    frame.render_widget(help_panel(stats), rows[2]);
}

fn collection_panel(stats: &CardStats) -> Paragraph<'static> {
    let lines = vec![
        Line::from(vec![
            Theme::muted_span("Tracked cards"),
            Theme::bullet(),
            Theme::label_span(format!("{}", stats.num_cards)),
        ]),
        Line::from(vec![
            Theme::muted_span("New"),
            Theme::bullet(),
            Theme::label_span(format!("{}", stats.new_cards)),
        ]),
        Line::from(vec![
            Theme::muted_span("Mature"),
            Theme::bullet(),
            Theme::label_span(format!("{}", stats.reviewed_cards)),
        ]),
        Line::from(vec![
            Theme::muted_span("Indexed in DB"),
            Theme::bullet(),
            Theme::label_span(format!("{}", stats.total_cards_in_db)),
        ]),
    ];
    Paragraph::new(lines)
        .style(Theme::body())
        .block(Theme::panel("Collection"))
}

fn due_panel(stats: &CardStats) -> Paragraph<'static> {
    let upcoming_week_total: i64 = stats.upcoming_week.iter().map(|b| b.count).sum();
    let lines = vec![
        Line::from(vec![
            Theme::muted_span("Due now"),
            Theme::bullet(),
            Theme::label_span(format!("{}", stats.due_cards)),
            Span::raw("  "),
            Theme::muted_span("Overdue"),
            Theme::bullet(),
            Theme::label_span(format!("{}", stats.overdue_cards)),
        ]),
        Line::from(vec![
            Theme::muted_span("Next 7 days"),
            Theme::bullet(),
            Theme::label_span(format!("{}", upcoming_week_total)),
        ]),
        Line::from(vec![
            Theme::muted_span("Next 30 days"),
            Theme::bullet(),
            Theme::label_span(format!("{}", stats.upcoming_month)),
        ]),
    ];
    Paragraph::new(lines)
        .style(Theme::body())
        .block(Theme::panel("Due Status"))
}

fn upcoming_panel(stats: &CardStats) -> List<'static> {
    let mut items: Vec<ListItem> = stats
        .upcoming_week
        .iter()
        .map(|bucket| {
            ListItem::new(Line::from(vec![
                Theme::label_span(bucket.day.clone()),
                Span::raw("  "),
                Theme::muted_span("cards"),
                Theme::bullet(),
                Theme::label_span(format!("{}", bucket.count)),
            ]))
        })
        .collect();

    if items.is_empty() {
        items.push(ListItem::new(Line::from(vec![Theme::muted_span(
            "You're clear for the next 7 days.",
        )])));
    }

    List::new(items)
        .style(Theme::body())
        .block(Theme::panel_with_line(Theme::title_line(
            "Next 7 days schedule",
        )))
}

fn highlights_panel(stats: &CardStats) -> Paragraph<'static> {
    let load_factor = if stats.num_cards == 0 {
        0.0
    } else {
        stats.due_cards as f32 / stats.num_cards as f32
    };
    let emphasis = if stats.overdue_cards > 0 {
        Theme::danger()
    } else if stats.due_cards == 0 {
        Theme::success()
    } else {
        Theme::emphasis()
    };
    let lines = vec![
        Line::from(vec![
            Span::styled("Focus", emphasis),
            Theme::bullet(),
            Theme::muted_span("Keep pace with today's queue"),
        ]),
        Line::from(vec![
            Theme::muted_span("Due load"),
            Theme::bullet(),
            Theme::label_span(format!("{:.0}%", load_factor * 100.0)),
        ]),
        Line::from(vec![
            Theme::muted_span("Overdue pressure"),
            Theme::bullet(),
            Theme::label_span(format!("{}", stats.overdue_cards)),
        ]),
        Line::from(vec![
            Theme::muted_span("Momentum"),
            Theme::bullet(),
            Theme::label_span(format!(
                "{} new / {} reviewed",
                stats.new_cards, stats.reviewed_cards
            )),
        ]),
    ];

    Paragraph::new(lines)
        .block(Theme::panel("Highlights"))
        .style(Theme::body())
        .wrap(Wrap { trim: true })
}

fn help_panel(stats: &CardStats) -> Paragraph<'static> {
    let lines = vec![
        Line::from(vec![
            Theme::key_chip("Esc"),
            Span::styled(" / ", Theme::muted()),
            Theme::key_chip("Ctrl+C"),
            Span::styled(" exit", Theme::muted()),
        ]),
        Line::from(vec![
            Theme::muted_span("Snapshot covers"),
            Theme::bullet(),
            Theme::label_span(format!("{} cards", stats.num_cards)),
            Theme::bullet(),
            Theme::muted_span("Rerun command anytime to refresh data"),
        ]),
    ];

    Paragraph::new(lines)
        .style(Theme::body())
        .block(Theme::panel_with_line(Theme::section_header("Controls")))
        .wrap(Wrap { trim: true })
}
