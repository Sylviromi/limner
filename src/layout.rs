use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use unicode_width::UnicodeWidthStr;

/// Split a code block into pre-wrapped lines, each with a full-width
/// background span so the background colour fills the entire terminal
/// width (for the code-block look).
pub fn wrap_code_block(code: &str, width: usize, style: Style, bg: Color) -> Vec<Line<'static>> {
    let w = width.max(1);
    let mut out = Vec::new();

    for src_line in code.lines() {
        if src_line.is_empty() {
            // Preserve empty lines as blank code lines.
            out.push(make_code_line("", w, style, bg));
            continue;
        }

        let cols = UnicodeWidthStr::width(src_line);
        if cols <= w {
            out.push(make_code_line(src_line, w, style, bg));
        } else {
            // Hard-wrap long lines at width boundary.
            let mut remain = src_line;
            while !remain.is_empty() {
                let wrap_at = find_wrap_col(remain, w);
                let (part, rest) = remain.split_at(wrap_at);
                out.push(make_code_line(part, w, style, bg));
                remain = rest;
            }
        }
    }

    out
}

fn make_code_line(text: &str, width: usize, style: Style, bg: Color) -> Line<'static> {
    let padding = width.saturating_sub(UnicodeWidthStr::width(text));
    let mut spans = Vec::with_capacity(2);
    spans.push(Span::styled(text.to_string(), style.bg(bg)));
    if padding > 0 {
        let blank = " ".repeat(padding);
        spans.push(Span::styled(blank, Style::default().bg(bg)));
    }
    Line::from(spans)
}

/// Find the byte index to break a line so that its displayed width ≤ `max_width`.
fn find_wrap_col(line: &str, max_width: usize) -> usize {
    let mut col = 0;
    for (i, c) in line.char_indices() {
        let cw = UnicodeWidthStr::width(c.to_string().as_str());
        if col + cw > max_width {
            return i;
        }
        col += cw;
    }
    line.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_line_padded() {
        let lines = wrap_code_block("hi", 10, Style::default(), Color::Black);
        assert_eq!(lines.len(), 1);
        // "hi" = 2 cols, padding = 8 spaces.
        let rendered = lines[0].to_string();
        assert_eq!(rendered.chars().count(), 10);
    }

    #[test]
    fn hard_wrap_long_line() {
        let lines = wrap_code_block("abcdefghij", 5, Style::default(), Color::Black);
        assert_eq!(lines.len(), 2);
    }

    #[test]
    fn empty_lines_preserved() {
        let lines = wrap_code_block("a\n\nb", 5, Style::default(), Color::Black);
        assert_eq!(lines.len(), 3);
    }
}
