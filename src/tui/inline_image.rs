//! Inline image rendering for terminals that support a graphics protocol.
//!
//! Wraps `ratatui-image` so the rest of the codebase never depends on it directly. A
//! terminal that cannot draw pixels simply gets no [`ImageRenderer`], and the drill UI
//! falls back to the `O` key and the OS viewer exactly as it did before.
//!
//! Named `inline_image` rather than `image` on purpose: a module called `image` would
//! shadow the `image` crate for every path inside it.

use std::path::Path;

use clap::ValueEnum;
use image::Limits;
use ratatui::layout::Rect;
use ratatui_image::{
    FilterType, Resize, StatefulImage,
    picker::{Picker, ProtocolType},
    protocol::StatefulProtocol,
};

/// Skip files this large rather than spend seconds decoding them mid-session.
const MAX_IMAGE_BYTES: u64 = 32 * 1024 * 1024;

/// Refuse images with either dimension beyond this. A file-size cap alone does not bound
/// the *decoded* size — a few hundred KB of PNG can expand to gigabytes — and
/// `panic = "abort"` in the dist profile means a decoder blowup cannot be caught.
const MAX_DECODED_EDGE: u32 = 16_384;

/// Ceiling on what the decoder may allocate at once.
const MAX_DECODE_ALLOC: u64 = 256 * 1024 * 1024;

/// Downscale to this longest edge before handing the image to the protocol. Terminal
/// cells are coarse enough that more pixels buy nothing, and it bounds the cost of the
/// resize/encode that happens on the render thread when the area changes.
const MAX_EDGE: u32 = 2048;

/// Below this the card panel is too small to split usefully; show text only.
const MIN_PANEL_HEIGHT: u16 = 8;
const MIN_PANEL_WIDTH: u16 = 12;

/// Rows always left to the question/answer text when an image shares the panel.
const MIN_TEXT_ROWS: u16 = 3;

/// Rows the text will never exceed when an image shares the panel. On a tall terminal a
/// percentage split leaves the text area mostly empty whitespace while cramping the picture,
/// so past this point the image takes everything else. Twelve rows is far more than any
/// card's question or answer needs.
const MAX_TEXT_ROWS: u16 = 12;

/// Share of the card panel the image gets on shorter terminals, as a percentage of height.
const IMAGE_HEIGHT_PERCENT: u16 = 45;

/// When to draw card images directly in the terminal.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, ValueEnum)]
pub enum InlineImages {
    /// Render inline only when the terminal supports a real graphics protocol.
    #[default]
    Auto,
    /// Never render inline; rely on `O` and the OS viewer.
    Off,
    /// Render inline even when that means chunky Unicode half-blocks.
    Always,
}

/// The image state of the card currently on screen.
pub enum CardImage {
    /// No image on this side of the card, or inline rendering is unavailable.
    Absent,
    /// There is an image but it cannot be shown; holds the reason for the footer.
    Failed(String),
    /// Decoded and ready to draw.
    Ready(Box<StatefulProtocol>),
}

impl CardImage {
    pub fn is_ready(&self) -> bool {
        matches!(self, Self::Ready(_))
    }

    /// Why the image could not be shown, if it could not.
    pub fn failure(&self) -> Option<&str> {
        match self {
            Self::Failed(reason) => Some(reason),
            _ => None,
        }
    }

    pub fn protocol_mut(&mut self) -> Option<&mut StatefulProtocol> {
        match self {
            Self::Ready(protocol) => Some(protocol),
            _ => None,
        }
    }
}

/// Decodes card images into a protocol the terminal can draw.
pub struct ImageRenderer {
    picker: Picker,
}

impl ImageRenderer {
    /// Detect terminal graphics support, returning `None` when inline rendering is
    /// unavailable or switched off.
    ///
    /// Must be called *after* entering the alternate screen and enabling raw mode, but
    /// *before* the keyboard enhancement flags are pushed and before the event loop reads
    /// its first key. The underlying query writes an escape sequence to stdout and reads
    /// the reply from stdin; a reader that starts earlier would swallow that reply as
    /// keystrokes, and the kitty keyboard protocol would change how it arrives.
    pub fn detect(mode: InlineImages) -> Option<Self> {
        match mode {
            InlineImages::Off => None,
            InlineImages::Auto => {
                let mut picker = Picker::from_query_stdio().ok()?;
                prefer_iterm2_over_kitty(&mut picker);
                // Half-blocks turn diagrams and any text inside the image into mush, so
                // under `Auto` we would rather show nothing and let the user press `O`.
                if picker.protocol_type() == ProtocolType::Halfblocks {
                    return None;
                }
                Some(Self { picker })
            }
            // `Always` exists precisely for terminals that fail the query, so fall back to
            // half-blocks rather than giving up.
            InlineImages::Always => {
                let mut picker =
                    Picker::from_query_stdio().unwrap_or_else(|_| Picker::halfblocks());
                prefer_iterm2_over_kitty(&mut picker);
                Some(Self { picker })
            }
        }
    }

    /// Name of the negotiated protocol, for the drill footer.
    pub fn protocol_name(&self) -> &'static str {
        match self.picker.protocol_type() {
            ProtocolType::Halfblocks => "halfblocks",
            ProtocolType::Sixel => "sixel",
            ProtocolType::Kitty => "kitty",
            ProtocolType::Iterm2 => "iterm2",
        }
    }

    /// Decode an image once, ready to be drawn by [`image_widget`].
    ///
    /// The `Err` string is shown in the drill footer, so a bad path degrades to the `O`
    /// fallback with an explanation instead of ending the session.
    pub fn load(&self, path: &Path) -> Result<StatefulProtocol, String> {
        let metadata = std::fs::metadata(path).map_err(|err| format!("cannot read: {err}"))?;
        if !metadata.is_file() {
            return Err("not a file".to_string());
        }
        if metadata.len() > MAX_IMAGE_BYTES {
            return Err(format!("too large ({} MB)", metadata.len() / (1024 * 1024)));
        }

        // Guess the format from the contents rather than the extension, so a mislabeled
        // file still renders and a non-image file fails cleanly.
        let mut reader = image::ImageReader::open(path)
            .map_err(|err| format!("cannot open: {err}"))?
            .with_guessed_format()
            .map_err(|err| format!("unreadable: {err}"))?;
        reader.limits(decode_limits());
        let decoded = reader.decode().map_err(|err| format!("{err}"))?;

        // `thumbnail` is the fast downscale path; quality beyond this is invisible once
        // the picture is mapped onto terminal cells.
        let decoded = if decoded.width() > MAX_EDGE || decoded.height() > MAX_EDGE {
            decoded.thumbnail(MAX_EDGE, MAX_EDGE)
        } else {
            decoded
        };

        Ok(self.picker.new_resize_protocol(decoded))
    }

    /// A renderer that needs no terminal, so tests elsewhere in the crate can exercise the
    /// image paths without a tty. `from_query_stdio` must never be called in tests: CI has
    /// no terminal to answer it.
    #[cfg(test)]
    pub(crate) fn halfblocks() -> Self {
        Self {
            picker: Picker::halfblocks(),
        }
    }
}

/// Does this environment look like iTerm2?
///
/// Deliberately keyed on `TERM_PROGRAM`/`LC_TERMINAL` rather than anything kitty-ish: real
/// Kitty leaves `TERM_PROGRAM` unset (it sets `TERM=xterm-kitty` and `KITTY_WINDOW_ID`), and
/// WezTerm reports `TERM_PROGRAM=WezTerm`, so neither can match here.
fn looks_like_iterm2(var: impl Fn(&str) -> Option<String>) -> bool {
    ["TERM_PROGRAM", "LC_TERMINAL"]
        .iter()
        .any(|key| var(key).is_some_and(|value| value.contains("iTerm")))
}

/// Work around iTerm2 answering the kitty graphics query without supporting kitty's
/// placement mechanism.
///
/// `ratatui-image` treats the terminal's query response as authoritative, and iTerm2 3.5+
/// replies that it speaks the kitty graphics protocol. But it does not implement kitty's
/// Unicode-placeholder placement, which is how `ratatui-image` positions kitty images — so
/// the picture gets transmitted and then silently never appears. iTerm2's own inline-image
/// protocol works fine (it is what `imgcat` uses), so prefer it.
///
/// This is the same correction `ratatui-image` already applies to WezTerm and Konsole, which
/// it blacklists from kitty for this exact reason; iTerm2 only started answering the query
/// recently and has not been added upstream.
fn prefer_iterm2_over_kitty(picker: &mut Picker) {
    if picker.protocol_type() == ProtocolType::Kitty
        && looks_like_iterm2(|key| std::env::var(key).ok())
    {
        picker.set_protocol_type(ProtocolType::Iterm2);
    }
}

fn decode_limits() -> Limits {
    // `Limits` is `#[non_exhaustive]`, so start from the default (which already caps
    // allocation at 512 MiB) and tighten it.
    let mut limits = Limits::default();
    limits.max_image_width = Some(MAX_DECODED_EDGE);
    limits.max_image_height = Some(MAX_DECODED_EDGE);
    limits.max_alloc = Some(MAX_DECODE_ALLOC);
    limits
}

/// The widget used to draw a loaded image, letterboxed to fit its area.
///
/// `Resize::Fit(None)` would default to `FilterType::Nearest`, which aliases badly when a
/// photo is squeezed into a few dozen terminal cells, so pick a real filter.
pub fn image_widget() -> StatefulImage<StatefulProtocol> {
    StatefulImage::default().resize(Resize::Fit(Some(FilterType::Triangle)))
}

/// Split a card panel into its text area and, when there is room, an image area.
///
/// Kept pure so the geometry is unit-testable without a terminal. Sizes the *image*
/// deterministically and gives the remainder to the text, because the wrapped height of
/// the text is not knowable here — `Text::height` counts logical lines, and the card
/// paragraph wraps.
pub fn split_card_area(inner: Rect, has_image: bool, zoom: bool) -> (Rect, Option<Rect>) {
    if !has_image || inner.height < MIN_PANEL_HEIGHT || inner.width < MIN_PANEL_WIDTH {
        return (inner, None);
    }
    if zoom {
        // No text at all; a zero-height Paragraph renders as a no-op.
        return (Rect { height: 0, ..inner }, Some(inner));
    }

    let image_height = (inner.height * IMAGE_HEIGHT_PERCENT / 100)
        .max(inner.height.saturating_sub(MAX_TEXT_ROWS))
        .clamp(4, inner.height.saturating_sub(MIN_TEXT_ROWS));
    let text_height = inner.height - image_height;

    let text = Rect {
        height: text_height,
        ..inner
    };
    let image = Rect {
        y: inner.y + text_height,
        height: image_height,
        ..inner
    };
    (text, Some(image))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `StatefulProtocol` is not `Debug`, so `unwrap_err` is unavailable on `load`'s result.
    fn load_err(renderer: &ImageRenderer, path: &str) -> String {
        match renderer.load(Path::new(path)) {
            Ok(_) => panic!("expected {path} to fail to load"),
            Err(reason) => reason,
        }
    }

    fn panel(width: u16, height: u16) -> Rect {
        Rect {
            x: 1,
            y: 1,
            width,
            height,
        }
    }

    #[test]
    fn detect_off_never_queries_the_terminal() {
        assert!(ImageRenderer::detect(InlineImages::Off).is_none());
    }

    #[test]
    fn inline_images_defaults_to_auto() {
        assert_eq!(InlineImages::default(), InlineImages::Auto);
    }

    #[test]
    fn loads_a_real_image() {
        let renderer = ImageRenderer::halfblocks();
        let path = Path::new("test_data/synaptic_vessel.jpg");
        assert!(path.is_file(), "fixture missing: {}", path.display());
        assert!(renderer.load(path).is_ok());
    }

    /// End-to-end check of the pipeline that cannot otherwise be exercised without a
    /// graphics terminal: decode a real file, hand it to the widget, and confirm the widget
    /// actually painted into the buffer. Uses half-blocks because they land in ordinary
    /// cells; the kitty/sixel/iTerm2 protocols emit escape sequences a test backend cannot
    /// observe, but they share this code path.
    #[test]
    fn rendering_a_loaded_image_paints_the_buffer() {
        use ratatui::{Terminal, backend::TestBackend};

        let renderer = ImageRenderer::halfblocks();
        let mut protocol = renderer
            .load(Path::new("test_data/synaptic_vessel.jpg"))
            .expect("fixture should decode");

        let mut terminal = Terminal::new(TestBackend::new(40, 20)).unwrap();
        let (_, image_area) = split_card_area(panel(38, 18), true, false);
        let image_area = image_area.expect("image area expected");

        terminal
            .draw(|frame| frame.render_stateful_widget(image_widget(), image_area, &mut protocol))
            .unwrap();

        assert!(
            protocol.last_encoding_result().is_none_or(|r| r.is_ok()),
            "encoding the image failed"
        );

        let buffer = terminal.backend().buffer();
        let painted = image_area
            .rows()
            .flat_map(|row| row.columns())
            .filter(|cell| {
                let cell = &buffer[*cell];
                cell.symbol() != " " || cell.bg != ratatui::style::Color::Reset
            })
            .count();
        assert!(painted > 0, "image widget painted nothing into its area");
    }

    #[test]
    fn missing_file_reports_a_reason() {
        let renderer = ImageRenderer::halfblocks();
        let err = load_err(&renderer, "test_data/does_not_exist.png");
        assert!(err.contains("cannot read"), "unexpected reason: {err}");
    }

    #[test]
    fn directory_is_rejected() {
        let renderer = ImageRenderer::halfblocks();
        assert_eq!(load_err(&renderer, "test_data"), "not a file");
    }

    #[test]
    fn non_image_bytes_are_rejected() {
        // An existing Markdown fixture stands in for "not an image".
        let renderer = ImageRenderer::halfblocks();
        let err = load_err(&renderer, "test_data/physics.md");
        assert!(!err.is_empty());
    }

    #[test]
    fn oversized_file_is_rejected_before_decoding() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("huge.png");
        let file = std::fs::File::create(&path).unwrap();
        file.set_len(MAX_IMAGE_BYTES + 1).unwrap();
        drop(file);

        let renderer = ImageRenderer::halfblocks();
        let err = load_err(&renderer, path.to_str().unwrap());
        assert!(err.contains("too large"), "unexpected reason: {err}");
    }

    #[test]
    fn protocol_name_is_reported() {
        assert_eq!(ImageRenderer::halfblocks().protocol_name(), "halfblocks");
    }

    #[test]
    fn iterm2_is_recognised_from_either_env_var() {
        let cases = [
            (vec![("TERM_PROGRAM", "iTerm.app")], true),
            (vec![("LC_TERMINAL", "iTerm2")], true),
            (
                vec![("TERM_PROGRAM", "iTerm.app"), ("LC_TERMINAL", "iTerm2")],
                true,
            ),
            // Real Kitty leaves TERM_PROGRAM unset; must not be mistaken for iTerm2.
            (
                vec![("TERM", "xterm-kitty"), ("KITTY_WINDOW_ID", "1")],
                false,
            ),
            // WezTerm also answers the kitty query but is not iTerm2.
            (vec![("TERM_PROGRAM", "WezTerm")], false),
            // Ghostty speaks real kitty graphics and must keep it.
            (vec![("TERM_PROGRAM", "ghostty")], false),
            (vec![("TERM_PROGRAM", "Apple_Terminal")], false),
            (vec![], false),
        ];
        for (env, expected) in cases {
            let lookup = |key: &str| {
                env.iter()
                    .find(|(k, _)| *k == key)
                    .map(|(_, v)| (*v).to_string())
            };
            assert_eq!(
                looks_like_iterm2(lookup),
                expected,
                "wrong verdict for {env:?}"
            );
        }
    }

    /// Guards the bug this works around: on iTerm2 the kitty protocol transmits the image
    /// and then never displays it, because iTerm2 does not do kitty's Unicode-placeholder
    /// placement. Assert the two protocols really do emit different, recognisable output.
    #[test]
    fn iterm2_protocol_emits_inline_image_sequences_not_kitty_placeholders() {
        use ratatui::{Terminal, backend::TestBackend, layout::Rect};

        fn render(protocol: ProtocolType) -> String {
            let mut picker = Picker::halfblocks();
            picker.set_protocol_type(protocol);
            let renderer = ImageRenderer { picker };
            let mut proto = renderer
                .load(Path::new("test_data/synaptic_vessel.jpg"))
                .expect("fixture should decode");

            let mut terminal = Terminal::new(TestBackend::new(40, 20)).unwrap();
            terminal
                .draw(|frame| {
                    frame.render_stateful_widget(
                        image_widget(),
                        Rect::new(0, 0, 30, 12),
                        &mut proto,
                    )
                })
                .unwrap();
            let buffer = terminal.backend().buffer();
            (0..20)
                .flat_map(|y| (0..40).map(move |x| (x, y)))
                .map(|(x, y)| buffer[(x, y)].symbol().to_string())
                .collect()
        }

        let kitty = render(ProtocolType::Kitty);
        assert!(
            kitty.contains('\u{10EEEE}'),
            "kitty protocol should place via Unicode placeholders"
        );

        let iterm2 = render(ProtocolType::Iterm2);
        assert!(
            iterm2.contains("1337;File="),
            "iterm2 protocol should emit its own inline-image sequence"
        );
        assert!(
            !iterm2.contains('\u{10EEEE}'),
            "iterm2 output must not rely on kitty placeholders"
        );
    }

    #[test]
    fn card_image_states() {
        assert!(!CardImage::Absent.is_ready());
        assert_eq!(CardImage::Absent.failure(), None);
        let failed = CardImage::Failed("nope".to_string());
        assert!(!failed.is_ready());
        assert_eq!(failed.failure(), Some("nope"));
    }

    #[test]
    fn no_image_takes_the_whole_panel() {
        let inner = panel(40, 20);
        let (text, image) = split_card_area(inner, false, false);
        assert_eq!(text, inner);
        assert!(image.is_none());
    }

    #[test]
    fn split_reserves_text_and_image_without_overlap() {
        let inner = panel(40, 20);
        let (text, image) = split_card_area(inner, true, false);
        let image = image.expect("image area expected");

        assert!(text.height >= MIN_TEXT_ROWS);
        assert_eq!(text.y, inner.y);
        assert_eq!(image.y, text.y + text.height, "areas must be adjacent");
        assert_eq!(text.height + image.height, inner.height, "must fill panel");
    }

    #[test]
    fn tiny_panels_fall_back_to_text_only() {
        for inner in [
            panel(40, MIN_PANEL_HEIGHT - 1),
            panel(MIN_PANEL_WIDTH - 1, 20),
        ] {
            let (text, image) = split_card_area(inner, true, false);
            assert_eq!(text, inner);
            assert!(image.is_none(), "panel {inner:?} is too small for an image");
        }
    }

    #[test]
    fn tall_panels_stop_wasting_rows_on_the_text() {
        // A 60-row panel under a plain 45% split would leave 33 rows for three lines of
        // text; the picture should get the space instead.
        let (text, image) = split_card_area(panel(100, 60), true, false);
        let image = image.expect("image area expected");
        assert!(
            text.height <= MAX_TEXT_ROWS,
            "text kept {} rows on a tall panel",
            text.height
        );
        assert!(image.height >= 60 - MAX_TEXT_ROWS);
        assert_eq!(text.height + image.height, 60);
    }

    #[test]
    fn short_panels_keep_the_percentage_split() {
        let (text, image) = split_card_area(panel(100, 20), true, false);
        let image = image.expect("image area expected");
        assert_eq!(image.height, 9, "45% of 20 rows");
        assert_eq!(text.height, 11);
    }

    #[test]
    fn zoom_gives_the_whole_panel_to_the_image() {
        let inner = panel(40, 20);
        let (text, image) = split_card_area(inner, true, true);
        assert_eq!(text.height, 0);
        assert_eq!(image, Some(inner));
    }

    #[test]
    fn split_never_panics_across_panel_sizes() {
        for height in 0..60u16 {
            for width in [0u16, 1, 11, 12, 80] {
                for zoom in [false, true] {
                    let inner = panel(width, height);
                    let (text, image) = split_card_area(inner, true, zoom);
                    if let Some(image) = image {
                        assert!(text.height + image.height <= inner.height);
                        assert!(image.height > 0);
                    }
                }
            }
        }
    }
}
