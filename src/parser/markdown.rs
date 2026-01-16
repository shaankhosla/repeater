use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span, Text};

pub fn render_markdown(md: &str) -> Text<'static> {
    let parser = Parser::new_ext(md, Options::ENABLE_MATH | Options::ENABLE_TASKLISTS);
    let mut lines: Vec<Line> = Vec::new();
    let mut current_line: Vec<Span> = Vec::new();
    let mut styles = vec![Style::default()];
    let mut list_stack: Vec<ListKind> = Vec::new();
    let mut pending_prefix: Option<String> = None;
    let mut in_code_block = false;

    for event in parser {
        match event {
            Event::Start(tag) => match tag {
                Tag::Heading { level, .. } => {
                    flush_line(&mut lines, &mut current_line);
                    push_style(&mut styles, |_| heading_style(level));
                }
                Tag::Strong => push_style(&mut styles, |style| style.add_modifier(Modifier::BOLD)),
                Tag::Emphasis => {
                    push_style(&mut styles, |style| style.add_modifier(Modifier::ITALIC))
                }
                Tag::BlockQuote(_) => {
                    push_style(&mut styles, |style| style.add_modifier(Modifier::DIM))
                }
                Tag::Link { .. } => push_style(&mut styles, |style| {
                    style.add_modifier(Modifier::UNDERLINED)
                }),
                Tag::CodeBlock(_) => {
                    flush_line(&mut lines, &mut current_line);
                    in_code_block = true;
                    push_style(&mut styles, |_| {
                        Style::default().add_modifier(Modifier::DIM)
                    });
                }
                Tag::List(start) => list_stack.push(ListKind::from(start)),
                Tag::Item => {
                    flush_line(&mut lines, &mut current_line);
                    pending_prefix = Some(list_prefix(list_stack.as_mut_slice()));
                }
                _ => {}
            },
            Event::End(tag) => match tag {
                TagEnd::Paragraph => {
                    flush_line(&mut lines, &mut current_line);
                    if list_stack.is_empty() {
                        lines.push(Line::default());
                    }
                }
                TagEnd::Heading(_) => {
                    flush_line(&mut lines, &mut current_line);
                    lines.push(Line::default());
                    pop_style(&mut styles);
                }
                TagEnd::CodeBlock => {
                    flush_line(&mut lines, &mut current_line);
                    lines.push(Line::default());
                    in_code_block = false;
                    pop_style(&mut styles);
                }
                TagEnd::Strong | TagEnd::Emphasis | TagEnd::BlockQuote(_) | TagEnd::Link => {
                    pop_style(&mut styles);
                }
                TagEnd::List(_) => {
                    flush_line(&mut lines, &mut current_line);
                    list_stack.pop();
                    if list_stack.is_empty() {
                        lines.push(Line::default());
                    }
                }
                TagEnd::Item => {
                    flush_line(&mut lines, &mut current_line);
                    pending_prefix = None;
                }
                _ => {}
            },
            Event::Text(text) => {
                let processed = if in_code_block {
                    text.to_string()
                } else {
                    latex_to_unicode_math(text.as_ref())
                };
                push_text(
                    &processed,
                    current_style(&styles),
                    in_code_block,
                    &mut lines,
                    &mut current_line,
                    &mut pending_prefix,
                )
            }
            Event::Code(code) => {
                maybe_apply_prefix(&mut current_line, &mut pending_prefix);
                current_line.push(Span::styled(
                    code.to_string(),
                    Style::default().add_modifier(Modifier::REVERSED),
                ));
            }
            Event::Html(html) | Event::InlineHtml(html) => push_text(
                html.as_ref(),
                current_style(&styles),
                in_code_block,
                &mut lines,
                &mut current_line,
                &mut pending_prefix,
            ),
            Event::InlineMath(math) | Event::DisplayMath(math) => {
                let converted = latex_to_unicode_math(math.as_ref());
                push_text(
                    &converted,
                    current_style(&styles).add_modifier(Modifier::ITALIC),
                    in_code_block,
                    &mut lines,
                    &mut current_line,
                    &mut pending_prefix,
                );
            }
            Event::FootnoteReference(label) => {
                let rendered = format!("[^{}]", label);
                push_text(
                    &rendered,
                    current_style(&styles),
                    in_code_block,
                    &mut lines,
                    &mut current_line,
                    &mut pending_prefix,
                );
            }
            Event::SoftBreak => {
                if in_code_block {
                    flush_line(&mut lines, &mut current_line);
                } else {
                    maybe_apply_prefix(&mut current_line, &mut pending_prefix);
                    current_line.push(Span::raw(" "));
                }
            }
            Event::HardBreak => {
                flush_line(&mut lines, &mut current_line);
            }
            Event::Rule => {
                flush_line(&mut lines, &mut current_line);
                lines.push(Line::from(Span::styled(
                    "─".repeat(20),
                    Style::default().add_modifier(Modifier::DIM),
                )));
                lines.push(Line::default());
            }
            Event::TaskListMarker(done) => {
                maybe_apply_prefix(&mut current_line, &mut pending_prefix);
                current_line.push(Span::styled(
                    format!("[{}] ", if done { 'x' } else { ' ' }),
                    current_style(&styles),
                ));
            }
        }
    }

    flush_line(&mut lines, &mut current_line);
    Text::from(lines)
}

fn push_text(
    text: &str,
    style: Style,
    in_code_block: bool,
    lines: &mut Vec<Line<'static>>,
    current_line: &mut Vec<Span<'static>>,
    pending_prefix: &mut Option<String>,
) {
    if in_code_block {
        let mut segments = text.split('\n').peekable();
        let mut first = true;
        while let Some(segment) = segments.next() {
            if !first {
                flush_line(lines, current_line);
            }
            first = false;
            if segment.is_empty() {
                if segments.peek().is_some() {
                    lines.push(Line::default());
                }
                continue;
            }
            maybe_apply_prefix(current_line, pending_prefix);
            current_line.push(Span::styled(segment.to_string(), style));
        }
    } else {
        maybe_apply_prefix(current_line, pending_prefix);
        current_line.push(Span::styled(text.to_string(), style));
    }
}

fn flush_line(lines: &mut Vec<Line<'static>>, current_line: &mut Vec<Span<'static>>) {
    if current_line.is_empty() {
        return;
    }
    lines.push(Line::from(std::mem::take(current_line)));
}

fn push_style<F>(stack: &mut Vec<Style>, f: F)
where
    F: FnOnce(Style) -> Style,
{
    let base = stack.last().cloned().unwrap_or_default();
    stack.push(f(base));
}

fn pop_style(stack: &mut Vec<Style>) {
    if stack.len() > 1 {
        stack.pop();
    }
}

fn current_style(stack: &[Style]) -> Style {
    stack.last().cloned().unwrap_or_default()
}

fn maybe_apply_prefix(current_line: &mut Vec<Span<'static>>, pending_prefix: &mut Option<String>) {
    if current_line.is_empty()
        && let Some(prefix) = pending_prefix.take()
    {
        current_line.push(Span::raw(prefix));
    }
}

fn heading_style(level: HeadingLevel) -> Style {
    let mut style = Style::default().add_modifier(Modifier::BOLD);
    if matches!(level, HeadingLevel::H1 | HeadingLevel::H2) {
        style = style.add_modifier(Modifier::UNDERLINED);
    }
    style
}

#[derive(Debug)]
enum ListKind {
    Unordered,
    Ordered(u64),
}

impl From<Option<u64>> for ListKind {
    fn from(value: Option<u64>) -> Self {
        match value {
            Some(n) if n > 0 => ListKind::Ordered(n),
            Some(_) => ListKind::Ordered(1),
            None => ListKind::Unordered,
        }
    }
}

impl ListKind {
    fn next_marker(&mut self) -> String {
        match self {
            ListKind::Unordered => "- ".to_string(),
            ListKind::Ordered(n) => {
                let marker = format!("{}. ", *n);
                *n += 1;
                marker
            }
        }
    }
}

fn list_prefix(stack: &mut [ListKind]) -> String {
    let indent = "  ".repeat(stack.len().saturating_sub(1));
    if let Some(kind) = stack.last_mut() {
        format!("{indent}{}", kind.next_marker())
    } else {
        "- ".to_string()
    }
}

fn latex_to_unicode_math(input: &str) -> String {
    let mut out = String::new();
    let mut chars = input.chars().peekable();

    while let Some(ch) = chars.next() {
        match ch {
            '\\' => match read_command(&mut chars) {
                Some(CommandToken::Named(name)) => {
                    if name == "frac" {
                        if let (Some(numerator), Some(denominator)) =
                            (read_group(&mut chars), read_group(&mut chars))
                        {
                            let top = latex_to_unicode_math(&numerator);
                            let bottom = latex_to_unicode_math(&denominator);
                            out.push_str(&top);
                            out.push('⁄');
                            out.push_str(&bottom);
                        } else {
                            out.push('\\');
                            out.push_str(&name);
                        }
                    } else if name == "text" || name == "textbf" || name == "mathbf" {
                        if let Some(content) = read_group(&mut chars) {
                            out.push_str(&latex_to_unicode_math(&content));
                        } else {
                            out.push('\\');
                            out.push_str(&name);
                        }
                    } else if let Some(replacement) = latex_command_to_unicode(&name) {
                        out.push_str(replacement);
                    } else {
                        out.push('\\');
                        out.push_str(&name);
                    }
                }
                Some(CommandToken::Symbol(symbol)) => out.push(symbol),
                None => out.push('\\'),
            },
            '^' | '_' => {
                let kind = if ch == '^' {
                    ScriptKind::Superscript
                } else {
                    ScriptKind::Subscript
                };
                let script = read_script(&mut chars);
                if script.is_empty() {
                    continue;
                }
                let (converted, fully_mapped) = convert_script_content(&script, kind);
                if !fully_mapped {
                    out.push(ch);
                }
                out.push_str(&converted);
            }
            _ => out.push(ch),
        }
    }

    out
}

#[derive(Copy, Clone)]
enum ScriptKind {
    Superscript,
    Subscript,
}

fn convert_script_content(content: &str, kind: ScriptKind) -> (String, bool) {
    let mut out = String::new();
    let mut fully_mapped = true;
    let mut chars = content.chars().peekable();

    while let Some(ch) = chars.next() {
        match ch {
            '\\' => match read_command(&mut chars) {
                Some(CommandToken::Named(name)) => {
                    if name == "text" || name == "textbf" || name == "mathbf" {
                        if let Some(content) = read_group(&mut chars) {
                            let (mapped, ok) = map_script_text(&content, kind);
                            if !ok {
                                fully_mapped = false;
                            }
                            out.push_str(&mapped);
                        } else {
                            fully_mapped = false;
                            out.push('\\');
                            out.push_str(&name);
                        }
                    } else if let Some(replacement) = latex_command_to_unicode(&name) {
                        let (mapped, ok) = map_script_text(replacement, kind);
                        if !ok {
                            fully_mapped = false;
                        }
                        out.push_str(&mapped);
                    } else {
                        fully_mapped = false;
                        out.push('\\');
                        out.push_str(&name);
                    }
                }
                Some(CommandToken::Symbol(symbol)) => {
                    if !push_mapped_char(&mut out, symbol, kind) {
                        fully_mapped = false;
                    }
                }
                None => {
                    fully_mapped = false;
                    out.push('\\');
                }
            },
            '^' | '_' => {}
            _ => {
                if !push_mapped_char(&mut out, ch, kind) {
                    fully_mapped = false;
                }
            }
        }
    }

    (out, fully_mapped)
}

fn map_script_text(value: &str, kind: ScriptKind) -> (String, bool) {
    let mut out = String::new();
    let mut fully_mapped = true;
    for ch in value.chars() {
        if !push_mapped_char(&mut out, ch, kind) {
            fully_mapped = false;
        }
    }
    (out, fully_mapped)
}

fn push_mapped_char(out: &mut String, value: char, kind: ScriptKind) -> bool {
    if let Some(mapped) = map_script_char(value, kind) {
        out.push(mapped);
        true
    } else {
        out.push(value);
        false
    }
}

fn map_script_char(value: char, kind: ScriptKind) -> Option<char> {
    match kind {
        ScriptKind::Superscript => superscript_char(value),
        ScriptKind::Subscript => subscript_char(value),
    }
}

enum CommandToken {
    Named(String),
    Symbol(char),
}

fn read_command<I>(chars: &mut std::iter::Peekable<I>) -> Option<CommandToken>
where
    I: Iterator<Item = char>,
{
    let next = chars.peek().copied()?;
    if next.is_ascii_alphabetic() {
        let mut name = String::new();
        while let Some(ch) = chars.peek().copied() {
            if ch.is_ascii_alphabetic() {
                name.push(ch);
                chars.next();
            } else {
                break;
            }
        }
        Some(CommandToken::Named(name))
    } else {
        chars.next().map(CommandToken::Symbol)
    }
}

fn read_script<I>(chars: &mut std::iter::Peekable<I>) -> String
where
    I: Iterator<Item = char>,
{
    let Some(next) = chars.peek().copied() else {
        return String::new();
    };
    if next != '{' {
        if next == '\\' {
            chars.next();
            return match read_command(chars) {
                Some(CommandToken::Named(name)) => format!(r"\{name}"),
                Some(CommandToken::Symbol(symbol)) => format!(r"\{symbol}"),
                None => "\\".to_string(),
            };
        }
        return chars.next().map(|ch| ch.to_string()).unwrap_or_default();
    }

    chars.next();
    let mut depth = 1;
    let mut out = String::new();
    for ch in chars.by_ref() {
        match ch {
            '{' => {
                depth += 1;
                out.push(ch);
            }
            '}' => {
                depth -= 1;
                if depth == 0 {
                    break;
                }
                out.push(ch);
            }
            _ => out.push(ch),
        }
    }
    out
}

fn read_group<I>(chars: &mut std::iter::Peekable<I>) -> Option<String>
where
    I: Iterator<Item = char>,
{
    while matches!(chars.peek(), Some(ch) if ch.is_whitespace()) {
        chars.next();
    }

    let next = chars.peek().copied()?;

    if next != '{' {
        return chars.next().map(|ch| ch.to_string());
    }

    chars.next();
    let mut depth = 1;
    let mut out = String::new();
    for ch in chars.by_ref() {
        match ch {
            '{' => {
                depth += 1;
                out.push(ch);
            }
            '}' => {
                depth -= 1;
                if depth == 0 {
                    break;
                }
                out.push(ch);
            }
            _ => out.push(ch),
        }
    }
    Some(out)
}

fn latex_command_to_unicode(command: &str) -> Option<&'static str> {
    match command {
        "int" => Some("∫"),
        "infty" => Some("∞"),
        "sum" => Some("∑"),
        "times" => Some("×"),
        "cdot" => Some("·"),
        "pm" => Some("±"),
        "leq" => Some("≤"),
        "geq" => Some("≥"),
        "neq" => Some("≠"),
        "approx" => Some("≈"),
        "to" | "rightarrow" => Some("→"),
        "leftarrow" => Some("←"),
        "leftrightarrow" => Some("↔"),
        "partial" => Some("∂"),
        "nabla" => Some("∇"),
        "neg" => Some("¬"),
        "land" => Some("∧"),
        "lor" => Some("∨"),
        "equiv" => Some("≡"),
        "alpha" => Some("α"),
        "beta" => Some("β"),
        "gamma" => Some("γ"),
        "delta" => Some("δ"),
        "epsilon" => Some("ε"),
        "theta" => Some("θ"),
        "lambda" => Some("λ"),
        "mu" => Some("μ"),
        "pi" => Some("π"),
        "sigma" => Some("σ"),
        "phi" => Some("φ"),
        "omega" => Some("ω"),
        "cos" => Some("cos"),
        "sin" => Some("sin"),
        "tan" => Some("tan"),
        "csc" => Some("csc"),
        "sec" => Some("sec"),
        "cot" => Some("cot"),
        "log" => Some("log"),
        "ln" => Some("ln"),
        "left" | "right" => Some(""),
        _ => None,
    }
}

fn superscript_char(value: char) -> Option<char> {
    match value {
        '0' => Some('⁰'),
        '1' => Some('¹'),
        '2' => Some('²'),
        '3' => Some('³'),
        '4' => Some('⁴'),
        '5' => Some('⁵'),
        '6' => Some('⁶'),
        '7' => Some('⁷'),
        '8' => Some('⁸'),
        '9' => Some('⁹'),
        '+' => Some('⁺'),
        '-' => Some('⁻'),
        '=' => Some('⁼'),
        '(' => Some('⁽'),
        ')' => Some('⁾'),
        'a' => Some('ᵃ'),
        'b' => Some('ᵇ'),
        'c' => Some('ᶜ'),
        'd' => Some('ᵈ'),
        'e' => Some('ᵉ'),
        'f' => Some('ᶠ'),
        'g' => Some('ᵍ'),
        'h' => Some('ʰ'),
        'i' => Some('ⁱ'),
        'j' => Some('ʲ'),
        'k' => Some('ᵏ'),
        'l' => Some('ˡ'),
        'm' => Some('ᵐ'),
        'n' => Some('ⁿ'),
        'o' => Some('ᵒ'),
        'p' => Some('ᵖ'),
        'r' => Some('ʳ'),
        's' => Some('ˢ'),
        't' => Some('ᵗ'),
        'u' => Some('ᵘ'),
        'v' => Some('ᵛ'),
        'w' => Some('ʷ'),
        'x' => Some('ˣ'),
        'y' => Some('ʸ'),
        'z' => Some('ᶻ'),
        'A' => Some('ᴬ'),
        'B' => Some('ᴮ'),
        'D' => Some('ᴰ'),
        'E' => Some('ᴱ'),
        'G' => Some('ᴳ'),
        'H' => Some('ᴴ'),
        'I' => Some('ᴵ'),
        'J' => Some('ᴶ'),
        'K' => Some('ᴷ'),
        'L' => Some('ᴸ'),
        'M' => Some('ᴹ'),
        'N' => Some('ᴺ'),
        'O' => Some('ᴼ'),
        'P' => Some('ᴾ'),
        'R' => Some('ᴿ'),
        'T' => Some('ᵀ'),
        'U' => Some('ᵁ'),
        'V' => Some('ⱽ'),
        'W' => Some('ᵂ'),
        _ => None,
    }
}

fn subscript_char(value: char) -> Option<char> {
    match value {
        '0' => Some('₀'),
        '1' => Some('₁'),
        '2' => Some('₂'),
        '3' => Some('₃'),
        '4' => Some('₄'),
        '5' => Some('₅'),
        '6' => Some('₆'),
        '7' => Some('₇'),
        '8' => Some('₈'),
        '9' => Some('₉'),
        '+' => Some('₊'),
        '-' => Some('₋'),
        '=' => Some('₌'),
        '(' => Some('₍'),
        ')' => Some('₎'),
        'a' => Some('ₐ'),
        'e' => Some('ₑ'),
        'h' => Some('ₕ'),
        'i' => Some('ᵢ'),
        'j' => Some('ⱼ'),
        'k' => Some('ₖ'),
        'l' => Some('ₗ'),
        'm' => Some('ₘ'),
        'n' => Some('ₙ'),
        'o' => Some('ₒ'),
        'p' => Some('ₚ'),
        'r' => Some('ᵣ'),
        's' => Some('ₛ'),
        't' => Some('ₜ'),
        'u' => Some('ᵤ'),
        'v' => Some('ᵥ'),
        'x' => Some('ₓ'),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::latex_to_unicode_math;
    use super::render_markdown;
    use proptest::prelude::*;
    proptest! {
        #[test]
        fn test_markdown_render( content in "\\PC*") {
            render_markdown(&content);
        }
    }
    #[test]
    fn renders_heading_and_paragraph() {
        let text = render_markdown("# Title\n\nBody");

        // Expect:
        // Line 0: "Title"
        // Line 1: blank
        // Line 2: "Body"
        // Line 3: blank
        assert_eq!(text.lines.len(), 4);

        assert_eq!(text.lines[0].spans[0].content, "Title");
        assert!(text.lines[1].spans.is_empty());
        assert_eq!(text.lines[2].spans[0].content, "Body");
        assert!(text.lines[3].spans.is_empty());
    }

    #[test]
    fn converts_latex_math_to_unicode() {
        let rendered = latex_to_unicode_math(r"\int_0^\infty e^{-x^2} dx");
        assert_eq!(rendered, "∫₀^∞ e⁻ˣ² dx");
    }

    #[test]
    fn converts_frac_and_sum_to_unicode() {
        let rendered = latex_to_unicode_math(r"-\frac{1}{N}\sum_{i=1}^n y_i");
        assert_eq!(rendered, "-1⁄N∑ᵢ₌₁ⁿ yᵢ");
    }

    #[test]
    fn renders_plain_text_math_to_unicode() {
        let text = render_markdown("x^2 + y_1");
        assert_eq!(text.lines[0].spans[0].content, "x² + y₁");
    }

    #[test]
    fn renders_text_command_without_conversion() {
        let rendered = latex_to_unicode_math(r"\text{correctly predicted positives}");
        assert_eq!(rendered, "correctly predicted positives");
    }
}
