use ratatui::prelude::{Color, Modifier, Style};
use ratatui::symbols::{border, line};

/// Per-section text alignment.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Alignment {
    #[default]
    Left,
    Center,
    Right,
    /// Word-wrap and distribute extra spaces so every line fills the full width
    /// (except the last line of a paragraph, which stays left-aligned).
    Justify,
}

/// Styling configuration for every element that `limner` can render.
///
/// Each block-level element has both a `Style` field and an `Alignment` field
/// (see [`Alignment`]) – set the latter to control how the element is laid
/// out on screen.
///
///
/// # Example – left-aligned paragraphs with centered headings
///
/// ```rust
/// # use limner::{Alignment, MarkdownStyle};
/// # use ratatui::prelude::*;
/// let style = MarkdownStyle {
///     heading_1: Style::new().green().bold(),
///     heading_1_alignment: Alignment::Center,
///     paragraph: Style::new().fg(Color::Rgb(220, 220, 220)),
///     paragraph_alignment: Alignment::Left,
///     ..MarkdownStyle::default()
/// };
/// ```
///
/// # Example – justified paragraphs with square code-block borders
///
/// ```rust
/// # use limner::{Alignment, MarkdownStyle};
/// # use ratatui::symbols::border;
/// let style = MarkdownStyle {
///     paragraph_alignment: Alignment::Justify,
///     code_block_border: Some(border::PLAIN),
///     ..MarkdownStyle::default()
/// };
/// ```
///
/// All alignment fields default to [`Alignment::Left`], matching the original
/// behaviour before alignment was introduced.
#[derive(Debug, Clone)]
pub struct MarkdownStyle {
    // ── Block elements ──────────────────────────────────────────
    /// Style applied to paragraph text.
    pub paragraph: Style,
    /// Alignment for paragraph lines.
    pub paragraph_alignment: Alignment,

    /// Style applied to level-1 heading text.
    pub heading_1: Style,
    /// Alignment for level-1 headings.
    pub heading_1_alignment: Alignment,
    /// Style applied to level-2 heading text.
    pub heading_2: Style,
    /// Alignment for level-2 headings.
    pub heading_2_alignment: Alignment,
    /// Style applied to level-3 heading text.
    pub heading_3: Style,
    /// Alignment for level-3 headings.
    pub heading_3_alignment: Alignment,

    // ── Inline elements ─────────────────────────────────────────
    pub bold: Style,
    pub italic: Style,
    pub strikethrough: Style,
    pub inline_code: Style,

    // ── Code blocks ─────────────────────────────────────────────
    /// Style applied to code-block text (commonly a subtle foreground).
    pub code_block: Style,
    /// Full-width background colour for code blocks.
    pub code_block_bg: Color,
    /// Border drawn around code blocks (e.g. `border::ROUNDED`,
    /// `border::PLAIN`), or `None` for a borderless block.
    pub code_block_border: Option<border::Set<'static>>,
    /// Style applied to the code-block border characters.
    pub code_block_border_style: Style,
    /// Style applied to the language label (e.g. `rust` from ` ```rust `),
    /// shown in the top border or on a header line when borderless.
    pub code_block_language: Style,
    /// Syntax-highlighting theme for fenced code blocks with a language, or
    /// `None` to disable.  Requires the `syntax-highlighting` feature
    /// (ignored otherwise); see `highlight::theme_names()` for valid names.
    pub syntax_theme: Option<&'static str>,

    // ── Tables ──────────────────────────────────────────────────
    /// Line characters for table borders (e.g. `line::ROUNDED`, `line::NORMAL`).
    pub table_border: line::Set<'static>,
    /// Style applied to table border characters.
    pub table_border_style: Style,
    /// Style patched onto header-row cells.
    pub table_header: Style,

    // ── Links ───────────────────────────────────────────────────
    /// Style applied to link text.
    pub link: Style,
    /// Static string prepended to every link (e.g. `"🔗 "` or `""`).
    pub link_prefix: &'static str,

    // ── Blockquotes ─────────────────────────────────────────────
    /// Style applied to blockquote text.
    pub quote: Style,
    /// Alignment for blockquote lines.
    pub quote_alignment: Alignment,
    /// Character(s) drawn at the start of each wrapped quote line.
    pub quote_indicator: &'static str,

    // ── Images ──────────────────────────────────────────────────
    /// Style applied to the image placeholder.
    pub image: Style,
    /// Static string prepended to every image (e.g. `"🖼 "`).
    pub image_prefix: &'static str,

    // ── Lists ───────────────────────────────────────────────────
    /// Bullet character for unordered lists.
    pub list_bullet: &'static str,
    /// Template for ordered-list numbers. Use `{}` as the number placeholder.
    pub ordered_template: &'static str,

    // ── Horizontal rules ────────────────────────────────────────
    pub hr_char: char,
    pub hr_style: Style,
}

impl Default for MarkdownStyle {
    /// Reasonable dark-theme defaults.
    fn default() -> Self {
        let white = Color::Rgb(220, 220, 220);
        let accent = Color::Rgb(98, 175, 239);
        let dim = Color::Rgb(140, 140, 140);
        let warm = Color::Rgb(229, 185, 115);
        let green = Color::Rgb(140, 200, 140);

        Self {
            paragraph: Style::new().fg(white),
            paragraph_alignment: Alignment::Left,

            heading_1: Style::new().fg(accent).add_modifier(Modifier::BOLD),
            heading_1_alignment: Alignment::Left,
            heading_2: Style::new().fg(accent).add_modifier(Modifier::BOLD),
            heading_2_alignment: Alignment::Left,
            heading_3: Style::new().fg(accent).add_modifier(Modifier::BOLD),
            heading_3_alignment: Alignment::Left,

            bold: Style::new().add_modifier(Modifier::BOLD),
            italic: Style::new().add_modifier(Modifier::ITALIC),
            strikethrough: Style::new().add_modifier(Modifier::CROSSED_OUT),
            inline_code: Style::new().fg(warm).bg(Color::Rgb(40, 40, 40)),

            code_block: Style::new().fg(warm),
            code_block_bg: Color::Rgb(25, 25, 25),
            code_block_border: Some(border::ROUNDED),
            code_block_border_style: Style::new().fg(Color::Rgb(80, 80, 80)),
            code_block_language: Style::new().fg(accent).add_modifier(Modifier::BOLD),
            syntax_theme: Some("base16-ocean.dark"),

            table_border: line::ROUNDED,
            table_border_style: Style::new().fg(Color::Rgb(80, 80, 80)),
            table_header: Style::new().fg(accent).add_modifier(Modifier::BOLD),

            link: Style::new().fg(accent).add_modifier(Modifier::UNDERLINED),
            link_prefix: "🔗 ",

            quote: Style::new().fg(dim),
            quote_alignment: Alignment::Left,
            quote_indicator: "▍ ",

            image: Style::new().fg(green),
            image_prefix: "🖼 ",

            list_bullet: "• ",
            ordered_template: "{}. ",

            hr_char: '─',
            hr_style: Style::new().fg(dim),
        }
    }
}
