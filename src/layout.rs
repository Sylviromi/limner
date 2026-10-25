use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

/// Number of columns a tab advances to (next multiple of this).
const TAB_WIDTH: usize = 4;

/// Split a code block into pre-wrapped lines, each with a full-width
/// background span so the background colour fills the entire terminal
/// width (for the code-block look).
pub fn wrap_code_block(code: &str, width: usize, style: Style, bg: Color) -> Vec<Line<'static>> {
    let w = width.max(1);
    let mut out = Vec::new();

    for src_line in code.lines() {
        let src_line = expand_tabs(src_line);
        let src_line = src_line.as_str();
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
    let mut s = text.to_string();
    if padding > 0 {
        s.push_str(&" ".repeat(padding));
    }
    Line::from(Span::styled(s, style.bg(bg)))
}

/// Find the byte index to break a line so that its displayed width ≤ `max_width`.
///
/// Always keeps at least one char, so a char wider than `max_width` can't
/// stall the caller's loop.
fn find_wrap_col(line: &str, max_width: usize) -> usize {
    let mut col = 0;
    for (i, c) in line.char_indices() {
        let cw = c.width().unwrap_or(0);
        if col + cw > max_width && i > 0 {
            return i;
        }
        col += cw;
    }
    line.len()
}

fn expand_tabs(line: &str) -> String {
    if !line.contains('\t') {
        return line.to_string();
    }
    let mut out = String::with_capacity(line.len() + TAB_WIDTH);
    let mut col = 0;
    for c in line.chars() {
        if c == '\t' {
            let n = TAB_WIDTH - col % TAB_WIDTH;
            out.push_str(&" ".repeat(n));
            col += n;
        } else {
            out.push(c);
            col += c.width().unwrap_or(0);
        }
    }
    out
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

    #[test]
    fn wide_char_wider_than_width_terminates() {
        let lines = wrap_code_block("日本", 1, Style::default(), Color::Black);
        assert_eq!(lines.len(), 2);
    }

    #[test]
    fn tabs_expand_to_tab_stops() {
        assert_eq!(expand_tabs("\tx"), "    x");
        assert_eq!(expand_tabs("ab\tx"), "ab  x");
    }
}
