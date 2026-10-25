//! Syntax highlighting for fenced code blocks (`syntax-highlighting` feature),
//! powered by [syntect]'s bundled Sublime Text grammars and themes.

use std::sync::OnceLock;

use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Span;
use syntect::easy::HighlightLines;
use syntect::highlighting::{self, FontStyle, ThemeSet};
use syntect::parsing::SyntaxSet;
use syntect::util::LinesWithEndings;

fn syntaxes() -> &'static SyntaxSet {
    static SET: OnceLock<SyntaxSet> = OnceLock::new();
    SET.get_or_init(SyntaxSet::load_defaults_newlines)
}

fn themes() -> &'static ThemeSet {
    static SET: OnceLock<ThemeSet> = OnceLock::new();
    SET.get_or_init(ThemeSet::load_defaults)
}

/// Names of the bundled themes usable as [`MarkdownStyle::syntax_theme`].
///
/// [`MarkdownStyle::syntax_theme`]: crate::MarkdownStyle::syntax_theme
pub fn theme_names() -> impl Iterator<Item = &'static str> {
    themes().themes.keys().map(String::as_str)
}

/// Highlight `code` as `language` (a fence token like `rust`, `rs`, `py`).
///
/// Returns one `Vec<Span>` per source line, or `None` if the language or
/// theme is unknown.  The theme's background is replaced by `bg`.
pub(crate) fn highlight(
    code: &str,
    language: &str,
    theme: &str,
    bg: Color,
) -> Option<Vec<Vec<Span<'static>>>> {
    let ss = syntaxes();
    let syntax = ss.find_syntax_by_token(language)?;
    let theme = themes().themes.get(theme)?;
    let mut h = HighlightLines::new(syntax, theme);

    let mut out = Vec::new();
    for line in LinesWithEndings::from(code) {
        let regions = h.highlight_line(line, ss).ok()?;
        out.push(
            regions
                .into_iter()
                .filter_map(|(style, text)| {
                    let text = text.trim_end_matches(['\n', '\r']);
                    (!text.is_empty()).then(|| Span::styled(text.to_string(), convert(style, bg)))
                })
                .collect(),
        );
    }
    Some(out)
}

fn convert(s: highlighting::Style, bg: Color) -> Style {
    let fg = s.foreground;
    let mut style = Style::new().fg(Color::Rgb(fg.r, fg.g, fg.b)).bg(bg);
    if s.font_style.contains(FontStyle::BOLD) {
        style = style.add_modifier(Modifier::BOLD);
    }
    if s.font_style.contains(FontStyle::ITALIC) {
        style = style.add_modifier(Modifier::ITALIC);
    }
    if s.font_style.contains(FontStyle::UNDERLINE) {
        style = style.add_modifier(Modifier::UNDERLINED);
    }
    style
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn highlights_known_language() {
        let lines = highlight("fn main() {}\n", "rust", "base16-ocean.dark", Color::Black).unwrap();
        assert_eq!(lines.len(), 1);
        assert!(lines[0].len() > 1, "expected several coloured tokens");
        let text: String = lines[0].iter().map(|s| s.content.as_ref()).collect();
        assert_eq!(text, "fn main() {}");
    }

    #[test]
    fn unknown_language_or_theme_is_none() {
        assert!(highlight("x", "no-such-lang", "base16-ocean.dark", Color::Black).is_none());
        assert!(highlight("x", "rust", "no-such-theme", Color::Black).is_none());
    }

    #[test]
    fn default_theme_is_bundled() {
        let default = crate::MarkdownStyle::default().syntax_theme.unwrap();
        assert!(theme_names().any(|t| t == default));
    }
}
