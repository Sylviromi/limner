use ratatui::prelude::{Color, Modifier, Style};

/// Styling configuration for every element that `limner` can render.
///
/// Create an instance and override only the fields you care about:
///
/// ```rust
/// # use limner::MarkdownStyle;
/// # use ratatui::prelude::*;
/// let style = MarkdownStyle {
///     heading_1: Style::new().green().bold(),
///     link: Style::new().cyan().underlined(),
///     ..MarkdownStyle::default()
/// };
/// ```
#[derive(Debug, Clone)]
pub struct MarkdownStyle {
    // ── Block elements ──────────────────────────────────────────
    pub paragraph: Style,
    pub heading_1: Style,
    pub heading_2: Style,
    pub heading_3: Style,

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

    // ── Links ───────────────────────────────────────────────────
    /// Style applied to link text.
    pub link: Style,
    /// Static string prepended to every link (e.g. `"🔗 "` or `""`).
    pub link_prefix: &'static str,

    // ── Blockquotes ─────────────────────────────────────────────
    /// Style applied to blockquote text.
    pub quote: Style,
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

            heading_1: Style::new().fg(accent).add_modifier(Modifier::BOLD),
            heading_2: Style::new().fg(accent).add_modifier(Modifier::BOLD),
            heading_3: Style::new().fg(accent).add_modifier(Modifier::BOLD),

            bold: Style::new().add_modifier(Modifier::BOLD),
            italic: Style::new().add_modifier(Modifier::ITALIC),
            strikethrough: Style::new().add_modifier(Modifier::CROSSED_OUT),
            inline_code: Style::new().fg(warm).bg(Color::Rgb(40, 40, 40)),

            code_block: Style::new().fg(warm),
            code_block_bg: Color::Rgb(25, 25, 25),

            link: Style::new().fg(accent).add_modifier(Modifier::UNDERLINED),
            link_prefix: "🔗 ",

            quote: Style::new().fg(dim),
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
