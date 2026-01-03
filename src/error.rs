use crate::theme::Theme;
use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyEventKind},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::prelude::{Color, Style};
use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    text::Line,
    widgets::Paragraph,
};
use std::io;
use std::time::Duration;

/// Formats an error and its context into a vector of error message lines
pub fn format_error_lines(context: &str, error: &anyhow::Error) -> Vec<String> {
    let mut lines = vec![context.to_string()];
    lines.extend(error.to_string().lines().map(String::from));
    lines
}

pub fn error_display(msgs: Vec<&str>) -> Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.hide_cursor()?;
    loop {
        terminal.draw(|frame| {
            let area = frame.area();
            frame.render_widget(Theme::backdrop(), area);

            let rows = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(7)])
                .split(area);
            let section = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(100)])
                .split(rows[0]);
            let lines = msgs
                .iter()
                .map(|msg| Line::styled(*msg, Style::default().bg(Color::Red)))
                .chain(std::iter::once(Line::styled(
                    "Press any key to continue.",
                    Style::default(),
                )))
                .collect::<Vec<_>>();
            let paragraph = Paragraph::new(lines)
                .style(Theme::body())
                .block(Theme::panel("Error"));
            frame.render_widget(paragraph, section[0]);
        })?;

        if event::poll(Duration::from_millis(50))?
            && let Event::Key(key) = event::read()?
            && key.kind == KeyEventKind::Press
        {
            break;
        }
    }
    terminal.show_cursor()?;
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::anyhow;

    #[test]
    fn format_error_lines_with_single_line_error() {
        let error = anyhow!("Something went wrong");
        let lines = format_error_lines("Error context", &error);

        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0], "Error context");
        assert_eq!(lines[1], "Something went wrong");
    }

    #[test]
    fn format_error_lines_with_multiline_error() {
        let error = anyhow!("Error occurred:\nLine 1\nLine 2\nLine 3");
        let lines = format_error_lines("Failed to process", &error);

        assert_eq!(lines.len(), 5);
        assert_eq!(lines[0], "Failed to process");
        assert_eq!(lines[1], "Error occurred:");
        assert_eq!(lines[2], "Line 1");
        assert_eq!(lines[3], "Line 2");
        assert_eq!(lines[4], "Line 3");
    }

    #[test]
    fn format_error_lines_preserves_context() {
        let error = anyhow!("Parse error");
        let context = "Failed to parse cards.md";
        let lines = format_error_lines(context, &error);

        assert_eq!(lines[0], context);
        assert!(lines[1].contains("Parse error"));
    }

    #[test]
    fn format_error_lines_handles_empty_error_message() {
        let error = anyhow!("");
        let lines = format_error_lines("Context message", &error);

        // Empty error message results in no additional lines
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0], "Context message");
    }

    #[test]
    fn format_error_lines_with_special_characters() {
        let error =
            anyhow!("Error: Invalid card format\n  Expected: Q: question\n  Found: X: something");
        let lines = format_error_lines("Card parsing failed", &error);

        assert_eq!(lines[0], "Card parsing failed");
        assert!(lines[1].contains("Invalid card format"));
        assert!(lines[2].contains("Expected"));
        assert!(lines[3].contains("Found"));
    }
}
