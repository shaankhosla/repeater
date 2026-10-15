pub mod editor;
pub mod theme;

pub use editor::Editor;
pub use theme::Theme;

use std::io::{self, Write};

use crossterm::event::{
    KeyboardEnhancementFlags, PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags,
};
use crossterm::execute;

/// Report every key as an escape code and disambiguate the ones a terminal
/// would otherwise collapse into the same sequence.
fn keyboard_enhancement_flags() -> KeyboardEnhancementFlags {
    KeyboardEnhancementFlags::REPORT_ALL_KEYS_AS_ESCAPE_CODES
        | KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES
}

/// crossterm's keyboard enhancement commands always take the WinAPI path on
/// Windows (`is_ansi_code_supported` is false for both) and every WinAPI call
/// returns `Unsupported`. The flags only change which escape codes arrive, so
/// the app runs without them.
fn tolerate_unsupported(result: io::Result<()>) -> io::Result<()> {
    match result {
        Err(error) if error.kind() == io::ErrorKind::Unsupported => Ok(()),
        result => result,
    }
}

pub fn push_keyboard_enhancement(stdout: &mut impl Write) -> io::Result<()> {
    tolerate_unsupported(execute!(
        stdout,
        PushKeyboardEnhancementFlags(keyboard_enhancement_flags())
    ))
}

pub fn pop_keyboard_enhancement(stdout: &mut impl Write) -> io::Result<()> {
    tolerate_unsupported(execute!(stdout, PopKeyboardEnhancementFlags))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unsupported_is_not_fatal() {
        let error = io::Error::new(io::ErrorKind::Unsupported, "not implemented");
        assert!(tolerate_unsupported(Err(error)).is_ok());
    }

    #[test]
    fn other_errors_propagate() {
        for kind in [io::ErrorKind::BrokenPipe, io::ErrorKind::Other] {
            let error = tolerate_unsupported(Err(io::Error::new(kind, "boom")))
                .expect_err("non-Unsupported errors must propagate");
            assert_eq!(error.kind(), kind);
        }
        assert!(tolerate_unsupported(Ok(())).is_ok());
    }

    // The two commands answer Unsupported through execute_winapi, which returns
    // before touching a console handle, so this runs in CI without a terminal.
    #[cfg(windows)]
    #[test]
    fn the_enhancement_is_optional_on_windows() {
        assert!(push_keyboard_enhancement(&mut io::sink()).is_ok());
        assert!(pop_keyboard_enhancement(&mut io::sink()).is_ok());
    }
}
