use crate::{
    check_version::{check_version, prompt_for_new_version},
    crud::DB,
    palette::Palette,
    parser::{FileSearchStats, register_all_cards},
    stats::{CardLifeCycle, CardStats, Histogram},
    tui::Theme,
    utils::pluralize,
};

use std::{
    cmp,
    io::{self},
    path::PathBuf,
    time::Duration,
};

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

pub async fn run(db: &DB, paths: Vec<PathBuf>, plain: bool) -> Result<usize> {
    let version_check = tokio::spawn(check_version(db.clone()));

    let (card_hashes, file_traversal_stats) = register_all_cards(db, paths).await?;
    let count = card_hashes.len();
    let crud_stats = db.collection_stats(&card_hashes).await?;
    if let Some(notification) = version_check.await.ok().flatten() {
        prompt_for_new_version(db, &notification).await;
    }

    if plain {
        render_plain_summary(&crud_stats, &file_traversal_stats);
    } else {
        render_dashboard(&crud_stats, &file_traversal_stats)?;
    }
    Ok(count)
}

fn render_dashboard(crud_stats: &CardStats, file_traversal_stats: &FileSearchStats) -> Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.hide_cursor()?;

    let draw_result = dashboard_loop(&mut terminal, crud_stats, file_traversal_stats);

    terminal.show_cursor()?;
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;

    draw_result
}

fn render_plain_summary(crud_stats: &CardStats, file_traversal_stats: &FileSearchStats) {
    println!("{}", Palette::paint(Palette::ACCENT, "Collection Summary"));
    println!(
        "{} {}",
        Palette::dim("Cards found:"),
        Palette::paint(Palette::INFO, crud_stats.num_cards)
    );
    println!(
        "{} {} {} {} {} {}",
        Palette::dim("New:"),
        Palette::paint(
            Palette::INFO,
            *crud_stats
                .card_lifecycles
                .get(&CardLifeCycle::New)
                .unwrap_or(&0)
        ),
        Palette::dim("Young:"),
        Palette::paint(
            Palette::INFO,
            *crud_stats
                .card_lifecycles
                .get(&CardLifeCycle::Young)
                .unwrap_or(&0)
        ),
        Palette::dim("Mature:"),
        Palette::paint(
            Palette::INFO,
            *crud_stats
                .card_lifecycles
                .get(&CardLifeCycle::Mature)
                .unwrap_or(&0)
        )
    );
    println!(
        "{} {}",
        Palette::dim("Files containing cards:"),
        Palette::paint(Palette::INFO, crud_stats.file_paths.len())
    );
    println!(
        "{} {}",
        Palette::dim("Markdowns parsed:"),
        Palette::paint(Palette::INFO, file_traversal_stats.markdown_files)
    );
    println!(
        "{} {}",
        Palette::dim("Files searched:"),
        Palette::paint(Palette::INFO, file_traversal_stats.files_searched)
    );
    println!(
        "{} {}",
        Palette::dim("Total cards indexed in DB:"),
        Palette::paint(Palette::INFO, crud_stats.total_cards_in_db)
    );

    println!("\n{}", Palette::paint(Palette::ACCENT, "Due Status"));
    let load_factor = if crud_stats.num_cards == 0 {
        0.0
    } else {
        crud_stats.due_cards as f32 / crud_stats.num_cards as f32
    };
    let due_color = if crud_stats.due_cards > 0 {
        Palette::WARNING
    } else {
        Palette::SUCCESS
    };
    let upcoming_week_total: usize = crud_stats.upcoming_week.values().sum();
    println!(
        "{} {}",
        Palette::dim("Due load:"),
        Palette::paint(due_color, format!("{:.0}%", load_factor * 100.0))
    );
    println!(
        "{} {}",
        Palette::dim("Due now:"),
        Palette::paint(due_color, crud_stats.due_cards)
    );
    println!(
        "{} {}",
        Palette::dim("Next 7 days:"),
        Palette::paint(Palette::INFO, upcoming_week_total)
    );
    println!(
        "{} {}",
        Palette::dim("Next 30 days:"),
        Palette::paint(Palette::INFO, crud_stats.upcoming_month)
    );

    println!(
        "\n{}",
        Palette::paint(Palette::ACCENT, "Next 7 Days Histogram")
    );
    if crud_stats.upcoming_week.is_empty() {
        println!("{}", Palette::dim("You're clear for the next 7 days."));
    } else {
        let max_count = crud_stats
            .upcoming_week
            .values()
            .max()
            .copied()
            .unwrap_or(0);
        for (day, count) in &crud_stats.upcoming_week {
            let label = format_upcoming_label(day);
            println!(
                "{} {}",
                Palette::dim(format!("{label}:")),
                format_bar(*count, max_count)
            );
        }
    }

    println!(
        "\n{}",
        Palette::paint(Palette::ACCENT, "FSRS Memory Health")
    );
    if crud_stats.retrievability_histogram.mean().is_none()
        || crud_stats.difficulty_histogram.mean().is_none()
    {
        println!("{}", Palette::dim("No FSRS statistics to display"));
    } else {
        render_plain_histogram(
            "Difficulty",
            "The higher the difficulty, the slower stability will increase.",
            &crud_stats.difficulty_histogram,
        );
        render_plain_histogram(
            "Retrievability",
            "The probability of recalling a card today.",
            &crud_stats.retrievability_histogram,
        );
    }
    println!(
        "\n{} {}",
        Palette::dim("Snapshot covers"),
        Palette::paint(
            Palette::INFO,
            pluralize("card", crud_stats.num_cards as usize)
        )
    );
    println!("{}", Palette::dim("Rerun command anytime to refresh data"));
}

fn render_plain_histogram(label: &str, description: &str, stats: &Histogram<5>) {
    println!(
        "{} {}",
        Palette::dim("Card"),
        Palette::paint(Palette::ACCENT, label)
    );
    println!("{}", Palette::dim(description));
    let average = stats
        .mean()
        .map(|v| format!("{}%", (v * 100.0).round()))
        .unwrap_or_else(|| String::from("NA - No cards reviewed"));
    println!(
        "{} {}",
        Palette::dim("Average:"),
        Palette::paint(Palette::INFO, average)
    );

    let max_bin = stats.bins.iter().copied().max().unwrap_or(0);
    let step_size = 100 / stats.bins.len().max(1);
    for (idx, count) in stats.bins.iter().enumerate() {
        let min_thresh = step_size * idx;
        let label = format!("{}%-{}%", min_thresh, min_thresh + step_size);
        println!(
            "{} {}",
            Palette::dim(format!("{label}:")),
            format_bar(*count as usize, max_bin as usize)
        );
    }
}

fn format_bar(count: usize, max: usize) -> String {
    let width = 20usize;
    let filled = if max == 0 {
        0
    } else {
        ((count as f64 / max as f64) * width as f64).round() as usize
    };
    let clamped = filled.min(width);
    let bar = "#".repeat(clamped);
    let remainder = "-".repeat(width - clamped);
    format!(
        "{} {}",
        Palette::paint(Palette::INFO, bar + &remainder),
        Palette::dim(pluralize("card", count))
    )
}

fn dashboard_loop(
    terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
    crud_stats: &CardStats,
    file_traversal_stats: &FileSearchStats,
) -> Result<()> {
    loop {
        terminal.draw(|frame| draw_dashboard(frame, crud_stats, file_traversal_stats))?;

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

fn draw_dashboard(
    frame: &mut Frame<'_>,
    crud_stats: &CardStats,
    file_traversal_stats: &FileSearchStats,
) {
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

    frame.render_widget(
        collection_panel(crud_stats, file_traversal_stats),
        summary[0],
    );
    frame.render_widget(due_panel(crud_stats), summary[1]);

    let mid = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(45), Constraint::Percentage(55)])
        .split(rows[1]);

    render_upcoming_histogram(frame, mid[0], crud_stats);

    render_fsrs_panel(frame, mid[1], crud_stats);

    frame.render_widget(help_panel(crud_stats), rows[2]);
}

fn collection_panel(
    crud_stats: &CardStats,
    file_traversal_stats: &FileSearchStats,
) -> Paragraph<'static> {
    let lines = vec![
        Line::from(vec![
            Theme::span("Cards Found"),
            Theme::bullet(),
            Theme::label_span(format!("{}", crud_stats.num_cards)),
        ]),
        Line::from(vec![
            Theme::span("New"),
            Theme::bullet(),
            Theme::label_span(format!(
                "{}",
                *crud_stats
                    .card_lifecycles
                    .get(&CardLifeCycle::New)
                    .unwrap_or(&0)
            )),
            Theme::bullet(),
            Theme::span("Young"),
            Theme::bullet(),
            Theme::label_span(format!(
                "{}",
                *crud_stats
                    .card_lifecycles
                    .get(&CardLifeCycle::Young)
                    .unwrap_or(&0)
            )),
            Theme::bullet(),
            Theme::span("Mature"),
            Theme::bullet(),
            Theme::label_span(format!(
                "{}",
                *crud_stats
                    .card_lifecycles
                    .get(&CardLifeCycle::Mature)
                    .unwrap_or(&0)
            )),
        ]),
        Line::from(vec![
            Theme::span("Files Containing Cards"),
            Theme::bullet(),
            Theme::label_span(format!("{}", crud_stats.file_paths.len())),
            Theme::bullet(),
            Theme::span("Markdowns Parsed"),
            Theme::bullet(),
            Theme::label_span(format!("{}", file_traversal_stats.markdown_files)),
            Theme::bullet(),
            Theme::span("Files Searched"),
            Theme::bullet(),
            Theme::label_span(format!("{}", file_traversal_stats.files_searched)),
        ]),
        Line::from(vec![
            Theme::span("Total Cards Indexed in DB"),
            Theme::bullet(),
            Theme::label_span(format!("{}", crud_stats.total_cards_in_db)),
        ]),
    ];
    Paragraph::new(lines).block(Theme::panel("Collection"))
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
            Theme::span("Due load"),
            Theme::bullet(),
            Theme::label_span(format!("{:.0}%", load_factor * 100.0)),
            Theme::bullet(),
            Theme::span("Due now"),
            Theme::bullet(),
            Theme::label_span(format!("{}", stats.due_cards)),
            Theme::span("  "),
        ]),
        Line::from(vec![
            Theme::span("Next 7 days"),
            Theme::bullet(),
            Theme::label_span(format!("{}", upcoming_week_total)),
            Theme::bullet(),
            Theme::span("Next 30 days"),
            Theme::bullet(),
            Theme::label_span(format!("{}", stats.upcoming_month)),
        ]),
    ];
    Paragraph::new(lines).block(Theme::panel("Due Status"))
}

fn render_upcoming_histogram(frame: &mut Frame<'_>, area: Rect, stats: &CardStats) {
    let block = Theme::panel_with_line(Theme::title_line("Next 7 days histogram"));
    if stats.upcoming_week.is_empty() {
        let empty = Paragraph::new(vec![Line::from(vec![Theme::span(
            "You're clear for the next 7 days.",
        )])])
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
                .label(Line::from(vec![Theme::span(label)]))
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
        .bar_style(Theme::label())
        .bar_gap(bar_gap);

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
            Theme::span("Average"),
            Theme::bullet(),
            Theme::label_span(histogram_stats.mean().map_or_else(
                || "NA - No cards reviewed".to_string(),
                |v| format!("{}%", (v * 100.0).round()),
            )),
        ]),
        Line::from(Theme::span(description)),
    ]);
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
                .label(Line::from(vec![Theme::span(label)]))
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
        .bar_style(Theme::label())
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
    if stats.retrievability_histogram.mean().is_none()
        || stats.difficulty_histogram.mean().is_none()
    {
        let empty = Paragraph::new(vec![Line::from(vec![Theme::span(
            "No FSRS statistics to display",
        )])])
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
            Theme::span("/ "),
            Theme::key_chip("Ctrl+C"),
            Theme::span(" exit"),
        ]),
        Line::from(vec![
            Theme::span("Snapshot covers"),
            Theme::bullet(),
            Theme::label_span(format!("{} cards", stats.num_cards)),
            Theme::bullet(),
            Theme::span("Rerun command anytime to refresh data"),
        ]),
    ];

    Paragraph::new(lines)
        .block(Theme::panel_with_line(Theme::section_header("Controls")))
        .wrap(Wrap { trim: true })
}

#[cfg(test)]
mod tests {
    use super::format_upcoming_label;

    #[test]
    fn format_upcoming_label_pretty_prints_dates() {
        assert_eq!(format_upcoming_label("2024-12-25"), "Wed 25");
    }

    #[test]
    fn format_upcoming_label_falls_back_to_original_input() {
        assert_eq!(
            format_upcoming_label("not-a-date"),
            "not-a-date".to_string()
        );
    }
}
