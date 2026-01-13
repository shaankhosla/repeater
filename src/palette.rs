use std::fmt;

use ratatui::style::Color;

#[derive(Clone, Copy, Debug)]
pub struct PaletteColor {
    tui: Color,
    ansi: &'static str,
}

impl PaletteColor {
    pub const fn new(tui: Color, ansi: &'static str) -> Self {
        Self { tui, ansi }
    }

    pub const fn tui(self) -> Color {
        self.tui
    }

    pub const fn ansi(self) -> &'static str {
        self.ansi
    }
}

pub struct Palette;

impl Palette {
    pub const RESET: &'static str = "\x1b[0m";
    pub const DIM: &'static str = "\x1b[2m";

    pub const ACCENT: PaletteColor = PaletteColor::new(Color::Blue, "\x1b[34m");
    pub const INFO: PaletteColor = PaletteColor::new(Color::Cyan, "\x1b[36m");
    pub const SUCCESS: PaletteColor = PaletteColor::new(Color::Green, "\x1b[32m");
    pub const WARNING: PaletteColor = PaletteColor::new(Color::Yellow, "\x1b[33m");
    pub const DANGER: PaletteColor = PaletteColor::new(Color::Red, "\x1b[31m");
    pub const BORDER: PaletteColor = PaletteColor::new(Color::Gray, "\x1b[90m");

    pub fn paint(color: PaletteColor, value: impl fmt::Display) -> String {
        format!("{}{}{}", color.ansi(), value, Self::RESET)
    }

    pub fn dim(value: impl fmt::Display) -> String {
        format!("{}{}{}", Self::DIM, value, Self::RESET)
    }
}
