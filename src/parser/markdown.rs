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
            Event::Text(text) => push_text(
                text.as_ref(),
                current_style(&styles),
                in_code_block,
                &mut lines,
                &mut current_line,
                &mut pending_prefix,
            ),
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
            Event::InlineMath(math) | Event::DisplayMath(math) => push_text(
                math.as_ref(),
                current_style(&styles).add_modifier(Modifier::ITALIC),
                in_code_block,
                &mut lines,
                &mut current_line,
                &mut pending_prefix,
            ),
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

#[cfg(test)]
mod tests {
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
}
