use ratatui::{
    layout::Alignment,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders},
};

/// Centralized color palette and helpers for a consistent TUI look.
pub struct Theme;

impl Theme {
    pub const BG: Color = Color::Rgb(6, 9, 15);
    pub const SURFACE: Color = Color::Rgb(16, 22, 35);
    pub const FG: Color = Color::Rgb(235, 240, 246);
    pub const MUTED: Color = Color::Rgb(133, 142, 168);
    pub const ACCENT: Color = Color::Rgb(118, 182, 255);
    pub const BORDER: Color = Color::Rgb(66, 80, 120);
    pub const WARNING: Color = Color::Rgb(255, 145, 145);
    pub const SUCCESS: Color = Color::Rgb(132, 222, 182);
    pub const EMPHASIS: Color = Color::Rgb(255, 214, 165);

    pub fn body() -> Style {
        Style::default().fg(Self::FG)
    }

    pub fn screen() -> Style {
        Style::default().bg(Self::BG).fg(Self::FG)
    }

    pub fn surface() -> Style {
        Style::default().bg(Self::SURFACE).fg(Self::FG)
    }

    pub fn muted() -> Style {
        Style::default().fg(Self::MUTED)
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
        Style::default()
            .fg(Self::EMPHASIS)
            .add_modifier(Modifier::BOLD)
    }

    pub fn panel<'a>(title: impl Into<String>) -> Block<'a> {
        Self::panel_with_line(Self::title_line(title))
    }

    pub fn backdrop<'a>() -> Block<'a> {
        Block::default().style(Self::screen())
    }

    pub fn panel_with_line<'a>(title: Line<'a>) -> Block<'a> {
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Self::BORDER))
            .style(Self::surface())
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

    pub fn muted_span(text: impl Into<String>) -> Span<'static> {
        Span::styled(text.into(), Self::muted())
    }

    pub fn key_chip(text: impl Into<String>) -> Span<'static> {
        Span::styled(
            format!(" {} ", text.into()),
            Style::default()
                .fg(Self::BG)
                .bg(Self::ACCENT)
                .add_modifier(Modifier::BOLD),
        )
    }

    pub fn bullet() -> Span<'static> {
        Span::styled(" • ", Self::muted())
    }

    pub fn section_header(text: impl Into<String>) -> Line<'static> {
        Line::from(vec![Span::styled(text.into(), Self::emphasis())])
    }
}
