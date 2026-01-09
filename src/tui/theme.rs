use ratatui::{
    layout::Alignment,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders},
};

/// Centralized color palette and helpers for a consistent TUI look.
pub struct Theme;

impl Theme {
    pub const KEY_FG: Color = Color::Rgb(255, 255, 255);
    pub const ACCENT: Color = Color::Blue;
    pub const BORDER: Color = Color::Gray;
    pub const WARNING: Color = Color::Yellow;
    pub const SUCCESS: Color = Color::Green;

    pub fn default_style() -> Style {
        Style::default()
    }

    pub fn label() -> Style {
        Style::default()
            .fg(Self::ACCENT)
            .add_modifier(Modifier::BOLD)
    }

    pub fn success() -> Style {
        Style::default()
            .fg(Self::SUCCESS)
            .add_modifier(Modifier::BOLD)
    }

    pub fn danger() -> Style {
        Style::default()
            .fg(Self::WARNING)
            .add_modifier(Modifier::BOLD)
    }

    pub fn emphasis() -> Style {
        Style::default().add_modifier(Modifier::BOLD)
    }

    pub fn panel<'a>(title: impl Into<String>) -> Block<'a> {
        Self::panel_with_line(Self::title_line(title))
    }

    pub fn backdrop<'a>() -> Block<'a> {
        Block::default()
    }

    pub fn panel_with_line<'a>(title: Line<'a>) -> Block<'a> {
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Self::BORDER))
            .title(title)
            .title_alignment(Alignment::Left)
    }

    pub fn title_line(title: impl Into<String>) -> Line<'static> {
        Line::from(vec![Span::styled(
            format!(" {} ", title.into()),
            Self::label(),
        )])
    }

    pub fn label_span(text: impl Into<String>) -> Span<'static> {
        Span::styled(text.into(), Self::label())
    }

    pub fn span(text: impl Into<String>) -> Span<'static> {
        Span::raw(text.into())
    }

    pub fn key_chip(text: impl Into<String>) -> Span<'static> {
        Span::styled(
            format!(" {} ", text.into()),
            Style::default()
                .fg(Self::KEY_FG)
                .bg(Self::ACCENT)
                .add_modifier(Modifier::BOLD),
        )
    }

    pub fn bullet() -> Span<'static> {
        Self::span(" • ")
    }

    pub fn section_header(text: impl Into<String>) -> Line<'static> {
        Line::from(vec![Span::styled(text.into(), Self::emphasis())])
    }
}
