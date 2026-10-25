use pulldown_cmark::{
    Alignment as ColumnAlignment, CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd,
};
use ratatui::layout::Alignment as RatatuiAlignment;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use unicode_width::UnicodeWidthStr;

use crate::layout;
use crate::style::{Alignment, MarkdownStyle};

/// Metadata about an image encountered during rendering.
#[derive(Debug, Clone)]
pub struct ImageInfo {
    pub url: String,
    pub alt: String,
    pub line_index: usize,
}

/// Metadata about a hyperlink encountered during rendering.
#[derive(Debug, Clone)]
pub struct LinkInfo {
    pub url: String,
    pub text: String,
    pub line_index: usize,
}

/// Combined result of rendering markdown.
#[derive(Debug, Clone)]
pub struct RenderResult {
    /// Text lines ready for a `Paragraph` widget.
    pub lines: Vec<Line<'static>>,
    /// Images found in the input (with their output line index).
    pub images: Vec<ImageInfo>,
    /// Hyperlinks found in the input (with their output line index).
    pub links: Vec<LinkInfo>,
}

/// Render markdown `input` into ratatui [`Line`]s.
///
/// Returns lines, image metadata, and link metadata.  Image lines show a
/// placeholder (`🖼 alt`).  The caller can use the [`ImageInfo`] entries
/// to render actual images via terminal graphic protocols, and the
/// [`LinkInfo`] entries to make links interactive.
///
/// Code blocks are pre-wrapped to `width` so that every line carries
/// the full-width background span.
pub fn render_markdown(input: &str, style: &MarkdownStyle, width: u16) -> RenderResult {
    let parser = Parser::new_ext(input, Options::all());
    let mut r = Renderer {
        style,
        width,
        output: Vec::new(),
        images: Vec::new(),
        links: Vec::new(),
        spans: Vec::new(),
        mods: Vec::new(),
        block_style: style.paragraph,
        block_alignment: Alignment::Left,
        pen_img_line: None,
        link_url: None,
        link_text: String::new(),
        is_image: false,
        pending_img_alt: String::new(),
        in_blockquote: 0,
        code_buf: None,
        code_lang: None,
        list_counters: Vec::new(),
        item_marker: None,
        item_indents: Vec::new(),
        table: None,
    };
    for event in parser {
        r.handle(event);
    }
    r.flush_paragraph(style.paragraph);
    while r.output.last().map(|l| l.spans.is_empty()).unwrap_or(false) {
        r.output.pop();
    }
    RenderResult {
        lines: r.output,
        images: r.images,
        links: r.links,
    }
}

// ── Internal state ───────────────────────────────────────────────────────────

struct Renderer<'a> {
    style: &'a MarkdownStyle,
    width: u16,
    output: Vec<Line<'static>>,
    images: Vec<ImageInfo>,
    links: Vec<LinkInfo>,

    spans: Vec<Span<'static>>,
    mods: Vec<Style>,
    block_style: Style,
    block_alignment: Alignment,
    pen_img_line: Option<usize>,

    link_url: Option<String>,
    link_text: String,
    is_image: bool,
    pending_img_alt: String,

    in_blockquote: u32,
    code_buf: Option<String>,
    code_lang: Option<String>,
    list_counters: Vec<ListCounter>,
    /// Marker (`• `, `1. `) for the current item, consumed by its first line.
    item_marker: Option<String>,
    /// Display width of each enclosing item's marker, for nested indentation.
    item_indents: Vec<usize>,
    table: Option<TableState>,
}

/// Cells collected while inside a table; rendered on `TagEnd::Table`.
struct TableState {
    alignments: Vec<ColumnAlignment>,
    /// `rows[0]` is the header row.
    rows: Vec<Vec<Vec<Span<'static>>>>,
    row: Vec<Vec<Span<'static>>>,
}

#[derive(Clone, Copy)]
enum ListCounter {
    Bullet,
    Ordered(u64),
}

// ── Event handling ───────────────────────────────────────────────────────────

impl<'a> Renderer<'a> {
    fn handle(&mut self, event: Event<'a>) {
        match event {
            Event::Start(tag) => self.start(tag),
            Event::End(tag_end) => self.end(tag_end),

            Event::Text(text) => self.on_text(&text),
            Event::Code(text) => {
                let s = text.to_string();
                let mut style = self.style.inline_code;
                for patch in &self.mods {
                    style = style.patch(*patch);
                }
                self.spans.push(Span::styled(s.clone(), style));
                if self.link_url.is_some() {
                    self.link_text.push_str(&s);
                }
            }
            Event::Html(text) | Event::InlineHtml(text) => {
                self.spans.push(Span::raw(text.into_string()));
            }
            Event::SoftBreak => {
                self.spans.push(Span::raw(" "));
                if self.link_url.is_some() {
                    self.link_text.push(' ');
                }
            }
            Event::HardBreak => {
                self.flush_paragraph(self.style.paragraph);
            }
            Event::Rule => {
                self.flush_paragraph(self.style.paragraph);
                let mut line = String::new();
                let w = self.width.max(1) as usize;
                for _ in 0..w {
                    line.push(self.style.hr_char);
                }
                self.output
                    .push(Line::from(Span::styled(line, self.style.hr_style)));
                if self.list_counters.is_empty() {
                    self.output.push(Line::default());
                }
            }
            Event::TaskListMarker(checked) => {
                let mark = if checked { "☑ " } else { "☐ " };
                self.spans.push(Span::raw(mark));
            }
            _ => {}
        }
    }

    // ── tag dispatching ────────────────────────────────────────

    fn start(&mut self, tag: Tag<'a>) {
        match tag {
            Tag::Paragraph => {
                self.block_style = if self.in_blockquote > 0 {
                    self.style.quote
                } else {
                    self.style.paragraph
                };
                self.block_alignment = if self.in_blockquote > 0 {
                    self.style.quote_alignment
                } else {
                    self.style.paragraph_alignment
                };
                self.mods.clear();
            }
            Tag::Heading { level, .. } => {
                let (sty, aln) = match level {
                    HeadingLevel::H1 => (self.style.heading_1, self.style.heading_1_alignment),
                    HeadingLevel::H2 => (self.style.heading_2, self.style.heading_2_alignment),
                    HeadingLevel::H3 => (self.style.heading_3, self.style.heading_3_alignment),
                    _ => (self.style.heading_3, self.style.heading_3_alignment),
                };
                self.block_style = sty;
                self.block_alignment = aln;
                self.mods.clear();
            }
            Tag::BlockQuote(_) => {
                self.flush_paragraph(self.style.paragraph);
                self.in_blockquote += 1;
                self.block_alignment = self.style.quote_alignment;
            }
            Tag::CodeBlock(kind) => {
                self.flush_paragraph(self.style.paragraph);
                self.code_buf = Some(String::new());
                self.code_lang = match kind {
                    // The info string may carry extra attributes
                    // (` ```rust,ignore ` or ` ```js {1,3} `); keep the language.
                    CodeBlockKind::Fenced(info) => info
                        .split(|c: char| c.is_whitespace() || c == ',' || c == '{')
                        .next()
                        .filter(|l| !l.is_empty())
                        .map(str::to_string),
                    CodeBlockKind::Indented => None,
                };
            }
            Tag::List(start) => {
                self.flush_paragraph(self.style.paragraph);
                let counter = match start {
                    Some(n) => ListCounter::Ordered(n),
                    None => ListCounter::Bullet,
                };
                self.list_counters.push(counter);
            }
            Tag::Item => {
                self.flush_paragraph(self.style.paragraph);
                let marker = match self.list_counters.last_mut() {
                    Some(ListCounter::Ordered(n)) => {
                        let label = self.style.ordered_template.replace("{}", &n.to_string());
                        *n += 1;
                        label
                    }
                    _ => self.style.list_bullet.to_string(),
                };
                self.item_indents
                    .push(UnicodeWidthStr::width(marker.as_str()));
                self.item_marker = Some(marker);
            }
            Tag::Table(alignments) => {
                self.flush_paragraph(self.style.paragraph);
                self.table = Some(TableState {
                    alignments,
                    rows: Vec::new(),
                    row: Vec::new(),
                });
            }
            Tag::TableCell => {
                self.spans.clear();
                self.block_style = self.style.paragraph;
                self.mods.clear();
            }
            Tag::Emphasis => self.mods.push(self.style.italic),
            Tag::Strong => self.mods.push(self.style.bold),
            Tag::Strikethrough => self.mods.push(self.style.strikethrough),
            Tag::Link { dest_url, .. } => {
                self.link_url = Some(dest_url.into_string());
                self.link_text.clear();
                if !self.style.link_prefix.is_empty() {
                    self.spans.push(Span::styled(
                        self.style.link_prefix.to_string(),
                        self.current_style(),
                    ));
                }
            }
            Tag::Image { dest_url, .. } => {
                self.is_image = true;
                self.link_url = Some(dest_url.into_string());
                self.pending_img_alt.clear();
            }
            _ => {}
        }
    }

    fn end(&mut self, tag: TagEnd) {
        match tag {
            TagEnd::Paragraph => {
                let before = self.output.len();
                self.flush_paragraph(self.style.paragraph);
                if self.output.len() > before && self.list_counters.is_empty() {
                    self.output.push(Line::default());
                }
            }
            TagEnd::Heading(level) => {
                let style = match level {
                    HeadingLevel::H1 => self.style.heading_1,
                    HeadingLevel::H2 => self.style.heading_2,
                    _ => self.style.heading_3,
                };
                let before = self.output.len();
                self.flush_paragraph(style);
                if self.output.len() > before && self.list_counters.is_empty() {
                    self.output.push(Line::default());
                }
            }
            TagEnd::BlockQuote(_) => {
                let before = self.output.len();
                self.flush_paragraph(self.style.paragraph);
                if self.output.len() > before && self.list_counters.is_empty() {
                    self.output.push(Line::default());
                }
                self.in_blockquote = self.in_blockquote.saturating_sub(1);
            }
            TagEnd::CodeBlock => {
                if let Some(buf) = self.code_buf.take() {
                    let before = self.output.len();
                    let lang = self.code_lang.take();
                    // Keep code inside lists / blockquotes visually nested.
                    let prefix = self.build_cont_prefix();
                    let prefix_w = UnicodeWidthStr::width(prefix.as_str());
                    let opts = layout::CodeBlockOptions {
                        language: lang.as_deref(),
                        border: self.style.code_block_border,
                        syntax_theme: self.style.syntax_theme,
                        code_style: self.style.code_block,
                        border_style: self.style.code_block_border_style,
                        language_style: self.style.code_block_language,
                        bg: self.style.code_block_bg,
                    };
                    let width = (self.width as usize).saturating_sub(prefix_w);
                    let lines = layout::code_block_lines(&buf, width, &opts);
                    self.push_nested_block(lines, &prefix);
                    if self.output.len() > before && self.list_counters.is_empty() {
                        self.output.push(Line::default());
                    }
                }
            }
            TagEnd::TableCell => {
                let cell = std::mem::take(&mut self.spans);
                if let Some(t) = &mut self.table {
                    t.row.push(cell);
                }
            }
            TagEnd::TableHead | TagEnd::TableRow => {
                if let Some(t) = &mut self.table {
                    let row = std::mem::take(&mut t.row);
                    t.rows.push(row);
                }
            }
            TagEnd::Table => {
                if let Some(t) = self.table.take() {
                    let before = self.output.len();
                    let prefix = self.build_cont_prefix();
                    let prefix_w = UnicodeWidthStr::width(prefix.as_str());
                    let opts = layout::TableOptions {
                        alignments: &t.alignments,
                        border: self.style.table_border,
                        border_style: self.style.table_border_style,
                        header_style: self.style.table_header,
                    };
                    let width = (self.width as usize).saturating_sub(prefix_w);
                    let lines = layout::table_lines(&t.rows, width, &opts);
                    self.push_nested_block(lines, &prefix);
                    if self.output.len() > before && self.list_counters.is_empty() {
                        self.output.push(Line::default());
                    }
                }
            }
            TagEnd::List(_) => {
                let before = self.output.len();
                self.flush_paragraph(self.style.paragraph);
                if self.output.len() > before && self.list_counters.len() <= 1 {
                    self.output.push(Line::default());
                }
                self.list_counters.pop();
            }
            TagEnd::Item => {
                self.flush_paragraph(self.style.paragraph);
                self.item_marker = None;
                self.item_indents.pop();
            }
            TagEnd::Emphasis => {
                self.mods.pop();
            }
            TagEnd::Strong => {
                self.mods.pop();
            }
            TagEnd::Strikethrough => {
                self.mods.pop();
            }
            TagEnd::Link => {
                self.finalize_link();
            }
            TagEnd::Image => {
                self.finalize_image();
            }
            _ => {}
        }
    }

    // ── inline helpers ─────────────────────────────────────────

    fn on_text(&mut self, text: &str) {
        if self.is_image {
            self.pending_img_alt.push_str(text);
            return;
        }

        // If the previous event was an image that got its own line,
        // absorb leading punctuation so it stays attached to the
        // image placeholder instead of starting the next paragraph.
        let text = if let Some(img_line) = self.pen_img_line.take() {
            let punct_count = text
                .chars()
                .take_while(|c| c.is_ascii_punctuation())
                .count();
            if punct_count > 0 {
                if let Some(line) = self.output.get_mut(img_line) {
                    line.spans.push(Span::raw(text[..punct_count].to_string()));
                }
                &text[punct_count..]
            } else {
                text
            }
        } else {
            text
        };

        if let Some(buf) = &mut self.code_buf {
            buf.push_str(text);
            return;
        }
        let style = self.current_style();
        self.spans.push(Span::styled(text.to_string(), style));
        if self.link_url.is_some() {
            self.link_text.push_str(text);
        }
    }

    fn current_style(&self) -> Style {
        let mut s = self.block_style;
        for patch in &self.mods {
            s = s.patch(*patch);
        }
        if self.link_url.is_some() {
            s = s.patch(self.style.link);
        }
        s
    }

    fn finalize_link(&mut self) {
        if let Some(url) = self.link_url.take() {
            let text = std::mem::take(&mut self.link_text);
            self.links.push(LinkInfo {
                line_index: usize::MAX,
                url,
                text,
            });
        }
    }

    fn finalize_image(&mut self) {
        let url = self.link_url.take().unwrap_or_default();
        let alt = std::mem::take(&mut self.pending_img_alt);

        let display = if alt.is_empty() {
            format!("{}image", self.style.image_prefix)
        } else {
            format!("{}{}", self.style.image_prefix, alt)
        };

        self.is_image = false;

        // Inside a table the placeholder stays inline in its cell; the image
        // isn't tracked since it can't be given a line of its own.
        if self.table.is_some() {
            self.spans.push(Span::styled(display, self.style.image));
            return;
        }

        // Flush any text accumulated before the image so the image placeholder
        // gets its own line.  This is required by `prepare_inline_images`
        // (image-protocol) which replaces the placeholder line with the actual
        // image — if the placeholder shared a line with surrounding text, that
        // text would be destroyed.
        let had_content = !self.spans.is_empty() || self.item_marker.is_some();
        if had_content {
            self.flush_paragraph(self.current_style());
        }

        let mut line = Line::from(vec![Span::styled(display, self.style.image)]);
        line.alignment = Self::map_alignment(self.block_alignment);
        self.output.push(line);
        let current = self.output.len() - 1;

        if had_content {
            // Text after the image will arrive in the next `on_text` call;
            // remember this line index so we can absorb leading punctuation
            // into the image line.
            self.pen_img_line = Some(current);
        }

        self.images.push(ImageInfo {
            line_index: current,
            url,
            alt,
        });
    }

    // ── line flushing ──────────────────────────────────────────

    fn flush_paragraph(&mut self, base_style: Style) {
        // Table cells collect their spans until `TagEnd::TableCell`.
        if self.table.is_some() {
            return;
        }
        let spans = std::mem::take(&mut self.spans);
        if spans.is_empty() && self.item_marker.is_none() {
            return;
        }

        // Plain top-level text is left to ratatui's own wrapping.  Justified
        // text, and text with a prefix (quotes, list items) that must repeat
        // on every wrapped line, is wrapped here.
        if self.block_alignment == Alignment::Justify || !self.build_cont_prefix().is_empty() {
            self.flush_wrapped(spans, base_style);
        } else {
            self.flush_simple(spans, base_style);
        }
    }

    fn flush_simple(&mut self, spans: Vec<Span<'static>>, base_style: Style) {
        let prefix = self.build_first_prefix();

        let mut line_spans: Vec<Span<'static>> = Vec::new();
        if !prefix.is_empty() {
            line_spans.push(Span::styled(prefix, base_style));
        }
        line_spans.extend(spans);

        let mut line = Line::from(line_spans);
        line.alignment = Self::map_alignment(self.block_alignment);
        self.push_line(line);
    }

    fn flush_wrapped(&mut self, spans: Vec<Span<'static>>, base_style: Style) {
        let justify = self.block_alignment == Alignment::Justify;
        let alignment = Self::map_alignment(self.block_alignment);
        let first_prefix = self.build_first_prefix();
        let cont_prefix = self.build_cont_prefix();

        let first_pw = UnicodeWidthStr::width(first_prefix.as_str());
        let cont_pw = UnicodeWidthStr::width(cont_prefix.as_str());

        let words = layout::split_words(&spans);
        if words.is_empty() {
            let mut line = Line::from(Span::styled(first_prefix, base_style));
            line.alignment = alignment;
            self.push_line(line);
            return;
        }

        // Subtract 1 as a safety margin: ratatui's WordWrapper uses `>=` when
        // checking whether a line overflows the widget width, so a line exactly
        // as wide as the widget would be re-wrapped (moving its last word to the
        // next visual line).  Keeping one column shorter avoids that.
        let first_avail = (self.width as usize)
            .saturating_sub(first_pw)
            .saturating_sub(1)
            .max(1);
        let cont_avail = (self.width as usize)
            .saturating_sub(cont_pw)
            .saturating_sub(1)
            .max(1);

        let word_lines = layout::wrap_words(words, first_avail, cont_avail);
        let total = word_lines.len();

        for (i, wl) in word_lines.into_iter().enumerate() {
            let prefix = if i == 0 { &first_prefix } else { &cont_prefix };
            let avail = if i == 0 { first_avail } else { cont_avail };

            let mut line_spans: Vec<Span<'static>> = Vec::new();
            if !prefix.is_empty() {
                line_spans.push(Span::styled(prefix.to_string(), base_style));
            }

            // The last line of a justified paragraph stays left-aligned.
            let gaps = wl.len().saturating_sub(1);
            if justify && i + 1 < total && gaps > 0 {
                let text_width: usize = wl.iter().map(|w| w.width).sum::<usize>() + gaps;
                let deficit = avail.saturating_sub(text_width);
                for (j, word) in wl.iter().enumerate() {
                    line_spans.extend(word.pieces.iter().cloned());
                    if let Some(next) = wl.get(j + 1) {
                        let n = 1 + deficit / gaps + usize::from(j < deficit % gaps);
                        line_spans.push(Span::styled(" ".repeat(n), layout::gap_style(word, next)));
                    }
                }
            } else {
                line_spans.extend(layout::join_words(wl));
            }

            let mut line = Line::from(line_spans);
            line.alignment = alignment;
            self.push_line(line);
        }
    }

    /// Push pre-rendered block lines (code, tables), prefixed so they stay
    /// nested inside quotes / list items.
    fn push_nested_block(&mut self, lines: Vec<Line<'static>>, prefix: &str) {
        for mut line in lines {
            if !prefix.is_empty() {
                line.spans
                    .insert(0, Span::styled(prefix.to_string(), self.style.quote));
            }
            self.push_line(line);
        }
    }

    fn push_line(&mut self, line: Line<'static>) {
        let current = self.output.len();
        self.output.push(line);
        for img in self
            .images
            .iter_mut()
            .rev()
            .take_while(|i| i.line_index == usize::MAX)
        {
            img.line_index = current;
        }
        for link in self
            .links
            .iter_mut()
            .rev()
            .take_while(|l| l.line_index == usize::MAX)
        {
            link.line_index = current;
        }
    }

    /// Prefix for the first line of a block: quote indicators, indentation
    /// for enclosing list items, then the current item's marker (consumed, so
    /// later lines of the same item are indented instead of re-bulleted).
    fn build_first_prefix(&mut self) -> String {
        let Some(marker) = self.item_marker.take() else {
            return self.build_cont_prefix();
        };
        let mut p = self
            .style
            .quote_indicator
            .repeat(self.in_blockquote as usize);
        let outer: usize = self.item_indents.iter().rev().skip(1).sum();
        p.push_str(&" ".repeat(outer));
        p.push_str(&marker);
        p
    }

    /// Prefix for continuation lines: quote indicators plus indentation for
    /// every enclosing list item.
    fn build_cont_prefix(&self) -> String {
        let mut p = self
            .style
            .quote_indicator
            .repeat(self.in_blockquote as usize);
        p.push_str(&" ".repeat(self.item_indents.iter().sum()));
        p
    }

    fn map_alignment(aln: Alignment) -> Option<RatatuiAlignment> {
        match aln {
            Alignment::Center => Some(RatatuiAlignment::Center),
            Alignment::Right => Some(RatatuiAlignment::Right),
            _ => None,
        }
    }
}

/// Like [`render_markdown`] but also accepts a list of external image URLs
/// to include in the result (e.g. images extracted from HTML or media
/// attachments that don't appear in the markdown itself).
pub fn render_markdown_with_extra(
    input: &str,
    style: &MarkdownStyle,
    width: u16,
    extra_images: &[String],
) -> RenderResult {
    let mut result = render_markdown(input, style, width);
    for url in extra_images {
        if !result.images.iter().any(|i| &i.url == url) {
            result.images.push(ImageInfo {
                url: url.clone(),
                alt: String::new(),
                line_index: 0,
            });
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_style() -> MarkdownStyle {
        MarkdownStyle::default()
    }

    #[test]
    fn empty_input() {
        let result = render_markdown("", &default_style(), 80);
        assert!(result.lines.is_empty());
        assert!(result.images.is_empty());
        assert!(result.links.is_empty());
    }

    #[test]
    fn plain_text() {
        let result = render_markdown("hello world", &default_style(), 80);
        assert_eq!(result.lines.len(), 1);
    }

    #[test]
    fn heading() {
        let result = render_markdown("# Title", &default_style(), 80);
        assert_eq!(result.lines.len(), 1);
        assert!(result.lines[0].to_string().contains("Title"));
    }

    #[test]
    fn heading_style_differs_from_paragraph() {
        let s = MarkdownStyle::default();
        let para = render_markdown("paragraph text", &s, 80);
        let head = render_markdown("# heading text", &s, 80);
        let p = format!("{:?}", para.lines[0]);
        let h = format!("{:?}", head.lines[0]);
        assert_ne!(p, h, "heading style should differ from paragraph");
    }

    fn texts(result: &RenderResult) -> Vec<String> {
        result.lines.iter().map(|l| l.to_string()).collect()
    }

    #[test]
    fn code_block() {
        let result = render_markdown("```\nlet x = 1;\n```", &default_style(), 80);
        // Top border, code line, bottom border.
        assert_eq!(result.lines.len(), 3);
    }

    #[test]
    fn code_block_language_in_border() {
        let result = render_markdown("```rust,ignore\nfn f() {}\n```", &default_style(), 20);
        assert_eq!(
            texts(&result),
            [
                "╭─ rust ───────────╮",
                "│ fn f() {}        │",
                "╰──────────────────╯",
            ]
        );
    }

    #[test]
    fn code_block_borderless() {
        let style = MarkdownStyle {
            code_block_border: None,
            ..default_style()
        };
        let result = render_markdown("```\nlet x = 1;\n```", &style, 80);
        assert_eq!(result.lines.len(), 1);
    }

    #[test]
    fn code_block_in_list_is_indented() {
        let result = render_markdown("- item\n\n  ```\n  x\n  ```", &default_style(), 12);
        assert_eq!(
            texts(&result),
            ["• item", "  ╭────────╮", "  │ x      │", "  ╰────────╯"]
        );
    }

    #[test]
    fn nested_lists_indent_without_stray_bullets() {
        let result = render_markdown("- a\n  - b\n    1. c\n- d", &default_style(), 80);
        assert_eq!(texts(&result), ["• a", "  • b", "    1. c", "• d"]);
    }

    #[test]
    fn hard_break_in_list_item_indents_continuation() {
        let result = render_markdown("- one  \n  two", &default_style(), 80);
        assert_eq!(texts(&result), ["• one", "  two"]);
    }

    #[test]
    fn image_tracked() {
        let result = render_markdown("![alt](img.png)", &default_style(), 80);
        assert_eq!(result.lines.len(), 1);
        assert_eq!(result.images.len(), 1);
        assert_eq!(result.images[0].url, "img.png");
        assert_eq!(result.images[0].alt, "alt");
    }

    #[test]
    fn link_tracked() {
        let result = render_markdown("[text](https://ex.com)", &default_style(), 80);
        assert_eq!(result.lines.len(), 1);
        assert_eq!(result.links.len(), 1);
        assert_eq!(result.links[0].url, "https://ex.com");
        assert_eq!(result.links[0].text, "text");
    }

    #[test]
    fn bold_and_italic() {
        let result = render_markdown("**bold** and *italic*", &default_style(), 80);
        assert_eq!(result.lines.len(), 1);
    }

    #[test]
    fn strikethrough_with_code() {
        let result = render_markdown("~~`code`~~", &default_style(), 80);
        assert_eq!(result.lines.len(), 1);
    }

    #[test]
    fn list_ordered() {
        let result = render_markdown("1. one\n2. two", &default_style(), 80);
        assert_eq!(result.lines.len(), 2);
    }

    #[test]
    fn list_unordered() {
        let result = render_markdown("- one\n- two", &default_style(), 80);
        assert_eq!(result.lines.len(), 2);
    }

    #[test]
    fn justify_paragraph_wraps_and_fits_width() {
        let style = MarkdownStyle {
            paragraph_alignment: Alignment::Justify,
            ..default_style()
        };
        let result = render_markdown(
            "This long paragraph is justified. Every line except the last is padded with \
             extra spaces so that both the left and right edges are perfectly aligned. \
             This is the classic newspaper-style typesetting. The last line stays \
             left-aligned as is conventional.",
            &style,
            60,
        );
        assert!(result.lines.len() >= 3, "should wrap to multiple lines");
        for (i, l) in result.lines.iter().enumerate() {
            let w = UnicodeWidthStr::width(l.to_string().as_str());
            assert!(w <= 60, "line {} width {} exceeds max 60", i, w);
        }
    }

    #[test]
    fn justify_with_image_no_duplicate_words() {
        let style = MarkdownStyle {
            paragraph_alignment: Alignment::Justify,
            ..default_style()
        };
        let text = "An image: ![alt](img.png). More text.";
        let result = render_markdown(text, &style, 80);
        let combined: String = result
            .lines
            .iter()
            .map(|l| l.to_string())
            .collect::<Vec<_>>()
            .join(" ");
        // "alt" should appear only ONCE (from the placeholder), not twice
        assert_eq!(
            combined.matches("alt").count(),
            1,
            "alt text should not be duplicated"
        );
    }

    #[test]
    fn justify_apostrophe_keeps_word_whole() {
        let style = MarkdownStyle {
            paragraph_alignment: Alignment::Justify,
            ..MarkdownStyle::default()
        };
        let result = render_markdown("my father's boots", &style, 80);
        assert_eq!(result.lines.len(), 1);
        let line = result.lines[0].to_string();
        assert!(
            line.contains("father\u{2019}s"),
            "expected apostrophe word intact, got: {line:?}"
        );
    }

    #[test]
    fn justify_double_quote_sticks_to_word() {
        let style = MarkdownStyle {
            paragraph_alignment: Alignment::Justify,
            ..MarkdownStyle::default()
        };
        let result = render_markdown("\"hello\" world", &style, 80);
        assert_eq!(result.lines.len(), 1);
        let line = result.lines[0].to_string();
        assert!(
            line.contains('\u{201C}'),
            "expected left curly quote, got: {line:?}"
        );
        assert!(
            line.contains('\u{201D}'),
            "expected right curly quote, got: {line:?}"
        );
    }

    #[test]
    fn two_paragraphs_separated_by_blank_line() {
        let result = render_markdown(
            "First paragraph.\n\nSecond paragraph.",
            &default_style(),
            80,
        );
        assert_eq!(
            result.lines.len(),
            3,
            "expected 2 paras + 1 blank separator"
        );
        assert_eq!(result.lines[0].to_string(), "First paragraph.");
        assert!(
            result.lines[1].to_string().is_empty(),
            "line 1 should be blank"
        );
        assert_eq!(result.lines[2].to_string(), "Second paragraph.");
    }

    #[test]
    fn heading_and_paragraph_separated() {
        let result = render_markdown("# Title\n\nBody text.", &default_style(), 80);
        assert_eq!(
            result.lines.len(),
            3,
            "expected heading + blank + paragraph"
        );
        assert!(result.lines[0].to_string().contains("Title"));
        assert!(
            result.lines[1].to_string().is_empty(),
            "separator should be blank"
        );
        assert_eq!(result.lines[2].to_string(), "Body text.");
    }

    #[test]
    fn multiple_blocks_no_trailing_blank() {
        let result = render_markdown("# A\n\na\n\n## B\n\nb\n\n---\n\nc", &default_style(), 80);
        assert!(!result.lines.is_empty(), "should have content");
        assert!(
            !result.lines.last().unwrap().to_string().is_empty(),
            "last line should not be blank"
        );
    }

    #[test]
    fn list_items_no_blank_lines_between() {
        let result = render_markdown("- one\n- two\n- three", &default_style(), 80);
        assert_eq!(result.lines.len(), 3, "no blank lines between list items");
    }

    #[test]
    fn table_renders_with_header_and_alignment() {
        let md = "| Name | Qty |\n|------|----:|\n| Pear | 3 |\n| **Fig** | 12 |\n\nafter";
        let result = render_markdown(md, &default_style(), 80);
        assert_eq!(
            texts(&result),
            [
                "╭──────┬─────╮",
                "│ Name │ Qty │",
                "├──────┼─────┤",
                "│ Pear │   3 │",
                "│ Fig  │  12 │",
                "╰──────┴─────╯",
                "",
                "after",
            ]
        );
    }

    #[test]
    fn table_image_stays_inline_in_cell() {
        let result = render_markdown("| a |\n|---|\n| ![x](y.png) |", &default_style(), 80);
        assert!(result.images.is_empty());
        assert!(result.lines.iter().any(|l| l.to_string().contains("🖼 x")));
    }

    #[test]
    fn wrapped_blockquote_keeps_indicator() {
        let result = render_markdown("> one two three four five six", &default_style(), 14);
        let lines = texts(&result);
        assert!(lines.len() > 1, "{lines:?}");
        assert!(lines.iter().all(|l| l.starts_with("▍ ")), "{lines:?}");
    }

    #[test]
    fn wrapped_list_item_is_indented() {
        let result = render_markdown("- one two three four five six", &default_style(), 14);
        let lines = texts(&result);
        assert!(lines[0].starts_with("• "));
        assert!(lines[1..].iter().all(|l| l.starts_with("  ")), "{lines:?}");
    }

    #[test]
    fn rule_ignores_previous_block_alignment() {
        let style = MarkdownStyle {
            heading_1_alignment: Alignment::Center,
            ..default_style()
        };
        let result = render_markdown("# T\n\n---", &style, 10);
        assert_eq!(result.lines.last().unwrap().alignment, None);
    }

    #[test]
    fn justify_keeps_mixed_styles_within_a_word() {
        let style = MarkdownStyle {
            paragraph_alignment: Alignment::Justify,
            ..default_style()
        };
        let result = render_markdown("**bold**plain", &style, 80);
        let spans = &result.lines[0].spans;
        assert_eq!(spans[0].content, "bold");
        assert_eq!(spans[1].content, "plain");
        assert_ne!(spans[0].style, spans[1].style);
    }
}
