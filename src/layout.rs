use ratatui::style::{Color, Style};
use ratatui::symbols::border;
use ratatui::text::{Line, Span};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

/// Number of columns a tab advances to (next multiple of this).
const TAB_WIDTH: usize = 4;

// ── Code blocks ──────────────────────────────────────────────────────────────

/// Split a code block into pre-wrapped lines, each with a full-width
/// background span so the background colour fills the entire terminal
/// width (for the code-block look).
pub fn wrap_code_block(code: &str, width: usize, style: Style, bg: Color) -> Vec<Line<'static>> {
    let w = width.max(1);
    let style = style.bg(bg);
    wrap_code_rows(plain_code(code, style), w)
        .into_iter()
        .map(|row| Line::from(pad_row(row, w, style)))
        .collect()
}

/// Options for [`code_block_lines`].
#[derive(Debug, Clone, Copy)]
pub struct CodeBlockOptions<'a> {
    /// Language label (e.g. `"rust"`), shown in the top border.
    pub language: Option<&'a str>,
    /// Border characters, or `None` for a borderless block.
    pub border: Option<border::Set<'static>>,
    pub code_style: Style,
    pub border_style: Style,
    pub language_style: Style,
    pub bg: Color,
}

/// Render a code block, filling `width`, optionally framed by a border with
/// the language label embedded in the top edge:
///
/// ```text
/// ╭─ rust ───────────╮
/// │ let x = 1;       │
/// ╰──────────────────╯
/// ```
///
/// Without a border, the language label (if any) is shown on its own header
/// line above the code.
pub fn code_block_lines(code: &str, width: usize, opts: &CodeBlockOptions) -> Vec<Line<'static>> {
    let width = width.max(1);
    let language = opts.language.filter(|l| !l.is_empty());
    let label_width = language.map_or(0, |l| UnicodeWidthStr::width(l) + 2);

    // Border + one column of padding on each side.
    const FRAME: usize = 4;
    let border = opts.border.filter(|_| width > FRAME);
    let inner = if border.is_some() {
        width - FRAME
    } else {
        width
    };

    let code_style = opts.code_style.bg(opts.bg);
    let border_style = opts.border_style.bg(opts.bg);
    let language_style = opts.language_style.bg(opts.bg);

    let code = expand_tabs(code);
    let logical = plain_code(&code, code_style);
    let rows = wrap_code_rows(logical, inner);

    let Some(b) = border else {
        let mut out = Vec::with_capacity(rows.len() + 1);
        if let Some(lang) = language.filter(|_| label_width <= inner) {
            out.push(Line::from(Span::styled(
                pad(&format!(" {lang} "), inner),
                language_style,
            )));
        }
        out.extend(
            rows.into_iter()
                .map(|r| Line::from(pad_row(r, inner, code_style))),
        );
        return out;
    };

    let mut out = Vec::with_capacity(rows.len() + 2);

    // Top edge: `╭─ lang ───╮`, or a plain edge if the label doesn't fit.
    let mut top = vec![Span::styled(b.top_left.to_string(), border_style)];
    let mut used = 2;
    if let Some(lang) = language.filter(|_| label_width + 4 <= width) {
        top.push(Span::styled(b.horizontal_top.to_string(), border_style));
        top.push(Span::styled(format!(" {lang} "), language_style));
        used += 1 + label_width;
    }
    top.push(Span::styled(
        b.horizontal_top.repeat(width - used),
        border_style,
    ));
    top.push(Span::styled(b.top_right.to_string(), border_style));
    out.push(Line::from(top));

    for row in rows {
        let mut spans = vec![Span::styled(format!("{} ", b.vertical_left), border_style)];
        spans.extend(pad_row(row, inner, code_style));
        spans.push(Span::styled(format!(" {}", b.vertical_right), border_style));
        out.push(Line::from(spans));
    }

    out.push(Line::from(Span::styled(
        format!(
            "{}{}{}",
            b.bottom_left,
            b.horizontal_bottom.repeat(width - 2),
            b.bottom_right
        ),
        border_style,
    )));
    out
}

fn plain_code(code: &str, style: Style) -> Vec<Vec<Span<'static>>> {
    code.lines()
        .map(|l| vec![Span::styled(expand_tabs(l), style)])
        .collect()
}

/// Hard-wrap styled code lines so no row is wider than `width` columns.
fn wrap_code_rows(lines: Vec<Vec<Span<'static>>>, width: usize) -> Vec<Vec<Span<'static>>> {
    let width = width.max(1);
    let mut out = Vec::new();
    for line in lines {
        let mut row: Vec<Span<'static>> = Vec::new();
        let mut col = 0;
        for span in line {
            let mut text = String::new();
            for c in span.content.chars() {
                let cw = c.width().unwrap_or(0);
                // Always place at least one char per row so characters wider
                // than `width` can't stall the loop.
                if col + cw > width && col > 0 {
                    if !text.is_empty() {
                        row.push(Span::styled(std::mem::take(&mut text), span.style));
                    }
                    out.push(std::mem::take(&mut row));
                    col = 0;
                }
                text.push(c);
                col += cw;
            }
            if !text.is_empty() {
                row.push(Span::styled(text, span.style));
            }
        }
        out.push(row);
    }
    out
}

fn expand_tabs(line: &str) -> String {
    if !line.contains('\t') {
        return line.to_string();
    }
    let mut out = String::with_capacity(line.len() + TAB_WIDTH);
    let mut col = 0;
    for c in line.chars() {
        match c {
            '\t' => {
                let n = TAB_WIDTH - col % TAB_WIDTH;
                out.push_str(&" ".repeat(n));
                col += n;
            }
            '\n' => {
                out.push(c);
                col = 0;
            }
            _ => {
                out.push(c);
                col += c.width().unwrap_or(0);
            }
        }
    }
    out
}

/// Right-pad `text` with spaces to `width` columns.
fn pad(text: &str, width: usize) -> String {
    let padding = width.saturating_sub(UnicodeWidthStr::width(text));
    let mut s = String::with_capacity(text.len() + padding);
    s.push_str(text);
    s.push_str(&" ".repeat(padding));
    s
}

/// Right-pad a row of spans with a `style`d filler span to `width` columns.
fn pad_row(mut row: Vec<Span<'static>>, width: usize, style: Style) -> Vec<Span<'static>> {
    let used: usize = row.iter().map(Span::width).sum();
    if used < width {
        row.push(Span::styled(" ".repeat(width - used), style));
    }
    row
}

// ── Word wrapping ────────────────────────────────────────────────────────────

/// A whitespace-delimited word made of one or more styled pieces
/// (e.g. `**bold**rest` is one word with two pieces).
#[derive(Debug, Clone)]
pub(crate) struct Word {
    pub pieces: Vec<Span<'static>>,
    pub width: usize,
}

#[cfg(test)]
impl Word {
    pub fn text(&self) -> String {
        self.pieces.iter().map(|p| p.content.as_ref()).collect()
    }
}

/// Split styled spans into words on ASCII whitespace, keeping each piece's
/// style.  Runs of whitespace collapse to a single word boundary.
pub(crate) fn split_words(spans: &[Span<'static>]) -> Vec<Word> {
    let mut words = Vec::new();
    let mut pieces: Vec<Span<'static>> = Vec::new();
    let mut text = String::new();
    let mut width = 0;

    for span in spans {
        for ch in span.content.chars() {
            if ch.is_ascii_whitespace() {
                if !text.is_empty() {
                    pieces.push(Span::styled(std::mem::take(&mut text), span.style));
                }
                if !pieces.is_empty() {
                    words.push(Word {
                        pieces: std::mem::take(&mut pieces),
                        width: std::mem::take(&mut width),
                    });
                }
            } else {
                text.push(ch);
                width += ch.width().unwrap_or(0);
            }
        }
        if !text.is_empty() {
            pieces.push(Span::styled(std::mem::take(&mut text), span.style));
        }
    }
    if !pieces.is_empty() {
        words.push(Word { pieces, width });
    }
    words
}

/// Greedy word wrap: the first line may be `first` columns wide, the rest
/// `cont`.  Words longer than a line are hard-split.
pub(crate) fn wrap_words(words: Vec<Word>, first: usize, cont: usize) -> Vec<Vec<Word>> {
    let mut lines: Vec<Vec<Word>> = Vec::new();
    let mut current: Vec<Word> = Vec::new();
    let mut current_width = 0;

    for word in words {
        let avail = if lines.is_empty() { first } else { cont }.max(1);
        let needed = if current.is_empty() {
            word.width
        } else {
            current_width + 1 + word.width
        };
        if needed <= avail {
            current_width = needed;
            current.push(word);
            continue;
        }
        if !current.is_empty() {
            lines.push(std::mem::take(&mut current));
        }
        let avail = if lines.is_empty() { first } else { cont }.max(1);
        let mut chunks = split_word(word, avail, cont.max(1));
        let last = chunks.pop().expect("split_word returns at least one chunk");
        lines.extend(chunks.into_iter().map(|c| vec![c]));
        current_width = last.width;
        current.push(last);
    }
    if !current.is_empty() {
        lines.push(current);
    }
    lines
}

/// Hard-split a word into chunks: the first at most `first` columns, the
/// rest at most `rest`.  Always places at least one char per chunk.
fn split_word(word: Word, first: usize, rest: usize) -> Vec<Word> {
    if word.width <= first {
        return vec![word];
    }
    let mut chunks = Vec::new();
    let mut cur = Word {
        pieces: Vec::new(),
        width: 0,
    };
    for piece in word.pieces {
        let mut text = String::new();
        for c in piece.content.chars() {
            let cw = c.width().unwrap_or(0);
            let limit = if chunks.is_empty() { first } else { rest };
            if cur.width + cw > limit && cur.width > 0 {
                if !text.is_empty() {
                    cur.pieces
                        .push(Span::styled(std::mem::take(&mut text), piece.style));
                }
                chunks.push(std::mem::replace(
                    &mut cur,
                    Word {
                        pieces: Vec::new(),
                        width: 0,
                    },
                ));
            }
            text.push(c);
            cur.width += cw;
        }
        if !text.is_empty() {
            cur.pieces.push(Span::styled(text, piece.style));
        }
    }
    chunks.push(cur);
    chunks
}

/// Style for the gap between two words: if they share a style at the
/// boundary (e.g. both inside one link), the gap keeps it so underlines and
/// backgrounds stay continuous.
pub(crate) fn gap_style(prev: &Word, next: &Word) -> Style {
    match (prev.pieces.last(), next.pieces.first()) {
        (Some(a), Some(b)) if a.style == b.style => a.style,
        _ => Style::default(),
    }
}

/// Join words with single spaces into spans.
pub(crate) fn join_words(words: Vec<Word>) -> Vec<Span<'static>> {
    let mut out = Vec::new();
    let mut prev: Option<Word> = None;
    for word in words {
        if let Some(p) = &prev {
            out.push(Span::styled(" ", gap_style(p, &word)));
        }
        out.extend(word.pieces.iter().cloned());
        prev = Some(word);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts<'a>(
        language: Option<&'a str>,
        border: Option<border::Set<'static>>,
    ) -> CodeBlockOptions<'a> {
        CodeBlockOptions {
            language,
            border,
            code_style: Style::default(),
            border_style: Style::default(),
            language_style: Style::default(),
            bg: Color::Black,
        }
    }

    fn text(lines: &[Line]) -> Vec<String> {
        lines.iter().map(|l| l.to_string()).collect()
    }

    fn cell(s: &str) -> Vec<Span<'static>> {
        vec![Span::raw(s.to_string())]
    }

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
        assert_eq!(expand_tabs("ab\n\tx"), "ab\n    x");
    }

    #[test]
    fn bordered_with_language() {
        let lines = code_block_lines("let x = 1;", 20, &opts(Some("rust"), Some(border::ROUNDED)));
        assert_eq!(
            text(&lines),
            [
                "╭─ rust ───────────╮",
                "│ let x = 1;       │",
                "╰──────────────────╯",
            ]
        );
    }

    #[test]
    fn bordered_wraps_inside_frame() {
        let lines = code_block_lines("abcdefgh", 8, &opts(None, Some(border::PLAIN)));
        assert_eq!(
            text(&lines),
            ["┌──────┐", "│ abcd │", "│ efgh │", "└──────┘"]
        );
    }

    #[test]
    fn label_dropped_when_too_narrow() {
        let lines = code_block_lines("x", 8, &opts(Some("typescript"), Some(border::PLAIN)));
        assert_eq!(lines[0].to_string(), "┌──────┐");
    }

    #[test]
    fn borderless_language_header() {
        let lines = code_block_lines("x", 6, &opts(Some("py"), None));
        assert_eq!(text(&lines), [" py   ", "x     "]);
    }

    #[test]
    fn split_words_keeps_piece_styles() {
        let bold = Style::new().bold();
        let words = split_words(&[Span::styled("bold", bold), Span::raw("rest and")]);
        assert_eq!(words.len(), 2);
        assert_eq!(words[0].text(), "boldrest");
        assert_eq!(words[0].pieces[0].style, bold);
        assert_eq!(words[0].pieces[1].style, Style::default());
    }

    #[test]
    fn split_words_contractions_and_quotes() {
        let spans = [
            Span::raw("it"),
            Span::raw("\u{2019}"),
            Span::raw("s a \u{201C}test\u{201D}"),
        ];
        let texts: Vec<_> = split_words(&spans).iter().map(Word::text).collect();
        assert_eq!(texts, ["it\u{2019}s", "a", "\u{201C}test\u{201D}"]);
    }

    #[test]
    fn wrap_words_hard_splits_long_words() {
        let lines = wrap_words(split_words(&cell("ab abcdefgh")), 4, 4);
        let texts: Vec<Vec<String>> = lines
            .iter()
            .map(|l| l.iter().map(Word::text).collect())
            .collect();
        assert_eq!(texts, [vec!["ab"], vec!["abcd"], vec!["efgh"]]);
    }
}
