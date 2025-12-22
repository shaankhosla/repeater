use crate::{
    crud::DB,
    drill::register_all_cards,
    stats::{CardLifeCycle, CardStats, Histogram},
    theme::Theme,
};

use std::{cmp, io, time::Duration};

use anyhow::Result;
use chrono::NaiveDate;
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Frame, Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    text::{Line, Span},
    widgets::{Bar, BarChart, BarGroup, Paragraph, Wrap},
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
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(rows[0]);

    frame.render_widget(collection_panel(stats), summary[0]);
    frame.render_widget(due_panel(stats), summary[1]);

    let mid = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(45), Constraint::Percentage(55)])
        .split(rows[1]);

    render_upcoming_histogram(frame, mid[0], stats);

    render_fsrs_panel(frame, mid[1], stats);

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
            Theme::label_span(format!(
                "{}",
                *stats.card_lifecycles.get(&CardLifeCycle::New).unwrap_or(&0)
            )),
            Theme::bullet(),
            Theme::muted_span("Young"),
            Theme::bullet(),
            Theme::label_span(format!(
                "{}",
                *stats
                    .card_lifecycles
                    .get(&CardLifeCycle::Young)
                    .unwrap_or(&0)
            )),
            Theme::bullet(),
            Theme::muted_span("Mature"),
            Theme::bullet(),
            Theme::label_span(format!(
                "{}",
                *stats
                    .card_lifecycles
                    .get(&CardLifeCycle::Mature)
                    .unwrap_or(&0)
            )),
        ]),
        Line::from(vec![
            Theme::muted_span("Files in Collection"),
            Theme::bullet(),
            Theme::label_span(format!("{}", stats.file_paths.len())),
        ]),
        Line::from(vec![
            Theme::muted_span("Total Cards Indexed in DB"),
            Theme::bullet(),
            Theme::label_span(format!("{}", stats.total_cards_in_db)),
        ]),
    ];
    Paragraph::new(lines)
        .style(Theme::body())
        .block(Theme::panel("Collection"))
}

fn due_panel(stats: &CardStats) -> Paragraph<'static> {
    let load_factor = if stats.num_cards == 0 {
        0.0
    } else {
        stats.due_cards as f32 / stats.num_cards as f32
    };
    let emphasis = if stats.due_cards > 0 {
        Theme::danger()
    } else if stats.due_cards == 0 {
        Theme::success()
    } else {
        Theme::emphasis()
    };
    let upcoming_week_total: usize = stats.upcoming_week.values().sum();
    let lines = vec![
        Line::from(vec![Span::styled("Focus", emphasis)]),
        Line::from(vec![
            Theme::muted_span("Due load"),
            Theme::bullet(),
            Theme::label_span(format!("{:.0}%", load_factor * 100.0)),
            Theme::bullet(),
            Theme::muted_span("Due now"),
            Theme::bullet(),
            Theme::label_span(format!("{}", stats.due_cards)),
            Span::raw("  "),
        ]),
        Line::from(vec![
            Theme::muted_span("Next 7 days"),
            Theme::bullet(),
            Theme::label_span(format!("{}", upcoming_week_total)),
            Theme::bullet(),
            Theme::muted_span("Next 30 days"),
            Theme::bullet(),
            Theme::label_span(format!("{}", stats.upcoming_month)),
        ]),
    ];
    Paragraph::new(lines)
        .style(Theme::body())
        .block(Theme::panel("Due Status"))
}

fn render_upcoming_histogram(frame: &mut Frame<'_>, area: Rect, stats: &CardStats) {
    let block = Theme::panel_with_line(Theme::title_line("Next 7 days histogram"));
    if stats.upcoming_week.is_empty() {
        let empty = Paragraph::new(vec![Line::from(vec![Theme::muted_span(
            "You're clear for the next 7 days.",
        )])])
        .style(Theme::body())
        .block(block);
        frame.render_widget(empty, area);
        return;
    }

    frame.render_widget(block.clone(), area);
    let mut inner = block.inner(area);
    if inner.width == 0 || inner.height == 0 {
        inner = area;
    }
    let mut chart_area = inner;
    let top_pad = cmp::min(3, chart_area.height);
    chart_area.y = chart_area.y.saturating_add(top_pad);
    chart_area.height = chart_area.height.saturating_sub(top_pad);

    let right_pad = cmp::min(2, chart_area.width);
    chart_area.width = chart_area.width.saturating_sub(right_pad);

    if chart_area.width == 0 || chart_area.height == 0 {
        chart_area = inner;
    }

    let bars: Vec<Bar<'static>> = stats
        .upcoming_week
        .iter()
        .map(|(day, count)| {
            let label = format_upcoming_label(day);
            Bar::default()
                .value(*count as u64)
                .text_value(count.to_string())
                .label(Line::from(vec![Theme::muted_span(label)]))
                .style(Theme::label())
        })
        .collect();

    let len = bars.len() as u16;
    let denom = cmp::max(len, 1);
    let mut available = chart_area.width.saturating_sub(1).max(1);
    let mut bar_gap: u16 = if len > 1 { 1 } else { 0 };
    let required_with_gap = len.saturating_add(bar_gap.saturating_mul(len.saturating_sub(1)));
    if required_with_gap > available {
        bar_gap = 0;
    }
    let total_gap = bar_gap.saturating_mul(len.saturating_sub(1));
    available = available.saturating_sub(total_gap);
    let bar_width = cmp::max(1, cmp::min(available / denom, available));

    let chart = BarChart::default()
        .data(BarGroup::default().bars(&bars))
        .bar_width(bar_width)
        .bar_gap(bar_gap)
        .value_style(Theme::body())
        .label_style(Theme::muted())
        .bar_style(Theme::label())
        .style(Theme::body());

    frame.render_widget(chart, chart_area);
}

fn format_upcoming_label(day: &str) -> String {
    NaiveDate::parse_from_str(day, "%Y-%m-%d")
        .map(|date| date.format("%a %d").to_string())
        .unwrap_or_else(|_| day.to_string())
}

fn render_fsrs_histogram(
    frame: &mut Frame<'_>,
    chart_area: Rect,
    histogram_stats: &Histogram<5>,
    title: &str,
    description: &str,
) {
    let section_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(6)])
        .split(chart_area);
    let difficulty_header = Paragraph::new(vec![
        Line::from(vec![
            Span::styled(format!("Card {}:", title), Theme::emphasis()),
            Theme::bullet(),
            Theme::muted_span("Average"),
            Theme::bullet(),
            Theme::label_span(format!("{}%", (histogram_stats.mean() * 100.0).round())),
        ]),
        Line::from(Theme::muted_span(description)),
    ])
    .style(Theme::body());
    frame.render_widget(difficulty_header, section_chunks[0]);
    let step_size = 100 / histogram_stats.bins.len().max(1);
    let bars: Vec<Bar> = histogram_stats
        .bins
        .iter()
        .enumerate()
        .map(|(i, count)| {
            let min_thresh = step_size * i;
            let label = format!("{}%-{}%", min_thresh, min_thresh + step_size);
            Bar::default()
                .value(*count as u64)
                .text_value(count.to_string())
                .label(Line::from(vec![Theme::muted_span(label)]))
                .style(Theme::label())
        })
        .collect();

    let len = bars.len() as u16;
    let available = chart_area.height.saturating_sub(1).max(1);
    let denom = cmp::max(len, 1);
    let raw_height = available / denom;
    let bar_height = cmp::max(1, cmp::min(cmp::max(raw_height, 1), available));

    let chart = BarChart::default()
        .data(BarGroup::default().bars(&bars))
        .bar_width(bar_height)
        .bar_gap(0)
        .value_style(Theme::body())
        .label_style(Theme::muted())
        .bar_style(Theme::label())
        .style(Theme::body())
        .direction(Direction::Horizontal);

    let mut chart_area = section_chunks[1];
    // let right_pad = cmp::min(3, chart_area.width);
    // chart_area.x = chart_area.x.saturating_add(right_pad);

    let right_pad = cmp::min(2, chart_area.width);
    chart_area.width = chart_area.width.saturating_sub(right_pad);

    frame.render_widget(chart, chart_area);
}

fn render_fsrs_panel(frame: &mut Frame<'_>, area: Rect, stats: &CardStats) {
    let block = Theme::panel_with_line(Theme::title_line("FSRS Memory Health"));
    if stats.upcoming_week.is_empty() {
        let empty = Paragraph::new(vec![Line::from(vec![Theme::muted_span(
            "No FSRS statistics to display",
        )])])
        .style(Theme::body())
        .block(block);
        frame.render_widget(empty, area);
        return;
    }
    frame.render_widget(block.clone(), area);
    let inner = block.inner(area);
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(inner);

    render_fsrs_histogram(
        frame,
        chunks[0],
        &stats.difficulty_histogram,
        "Difficulty",
        "The higher the difficulty, the slower stability will increase.",
    );
    render_fsrs_histogram(
        frame,
        chunks[1],
        &stats.retrievability_histogram,
        "Retrievability",
        "The probability of recalling a card today.",
    );
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
