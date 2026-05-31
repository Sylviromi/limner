use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag, TagEnd};
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
        list_counters: Vec::new(),
        item_prefix: None,
        has_content: false,
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
    list_counters: Vec<ListCounter>,
    item_prefix: Option<String>,
    has_content: bool,
}

#[derive(Clone, Copy)]
enum ListCounter {
    Bullet,
    Ordered(u64),
}

struct StyledWord {
    text: String,
    style: Style,
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
                let mut l = Line::from(Span::styled(line, self.style.hr_style));
                l.alignment = Self::map_alignment(self.block_alignment);
                self.output.push(l);
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
            Tag::CodeBlock(_) => {
                self.flush_paragraph(self.style.paragraph);
                self.block_alignment = self.style.code_block_alignment;
                self.code_buf = Some(String::new());
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
                self.has_content = false;
                if let Some(counter) = self.list_counters.last_mut() {
                    match counter {
                        ListCounter::Bullet => {
                            self.item_prefix = Some(self.style.list_bullet.to_string());
                        }
                        ListCounter::Ordered(n) => {
                            let label = self.style.ordered_template.replace("{}", &n.to_string());
                            self.item_prefix = Some(label);
                            *n += 1;
                        }
                    }
                }
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
                self.has_content = true;
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
                    let mut lines = layout::wrap_code_block(
                        &buf,
                        self.width as usize,
                        self.style.code_block,
                        self.style.code_block_bg,
                    );
                    self.apply_alignment(&mut lines);
                    self.output.extend(lines);
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
                self.item_prefix = None;
            }
            TagEnd::Item => {
                if !self.has_content {
                    self.flush_paragraph(self.style.paragraph);
                }
                self.item_prefix = None;
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

        // Flush any text accumulated before the image so the image placeholder
        // gets its own line.  This is required by `prepare_inline_images`
        // (image-protocol) which replaces the placeholder line with the actual
        // image — if the placeholder shared a line with surrounding text, that
        // text would be destroyed.
        let had_content = !self.spans.is_empty() || self.item_prefix.is_some();
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
        let spans = std::mem::take(&mut self.spans);
        if spans.is_empty() && self.item_prefix.is_none() {
            return;
        }

        match self.block_alignment {
            Alignment::Justify => self.flush_justified(spans, base_style),
            _ => self.flush_simple(spans, base_style),
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
        line.alignment = match self.block_alignment {
            Alignment::Center => Some(RatatuiAlignment::Center),
            Alignment::Right => Some(RatatuiAlignment::Right),
            _ => None,
        };
        self.push_line(line);
    }

    fn flush_justified(&mut self, spans: Vec<Span<'static>>, base_style: Style) {
        let first_prefix = self.build_first_prefix();
        let cont_prefix = self.build_cont_prefix();

        let first_pw = UnicodeWidthStr::width(first_prefix.as_str());
        let cont_pw = UnicodeWidthStr::width(cont_prefix.as_str());

        let words = Self::spans_to_words(&spans);
        if words.is_empty() {
            let mut line = Line::from(Span::styled(first_prefix, base_style));
            line.alignment = None;
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

        let word_lines = self.justify_word_wrap(&words, first_avail, cont_avail);
        let total = word_lines.len();

        for (i, wl) in word_lines.iter().enumerate() {
            let prefix = if i == 0 { &first_prefix } else { &cont_prefix };
            let avail = if i == 0 { first_avail } else { cont_avail };
            let is_last = i == total - 1;

            let mut line_spans: Vec<Span<'static>> = Vec::new();
            if !prefix.is_empty() {
                line_spans.push(Span::styled(prefix.to_string(), base_style));
            }

            if is_last {
                for (j, word) in wl.iter().enumerate() {
                    if j > 0 {
                        line_spans.push(Span::raw(" "));
                    }
                    line_spans.push(Span::styled(word.text.clone(), word.style));
                }
            } else {
                let gaps = wl.len().saturating_sub(1);
                if gaps == 0 {
                    line_spans.push(Span::styled(wl[0].text.clone(), wl[0].style));
                } else {
                    let word_widths: usize = wl
                        .iter()
                        .map(|w| UnicodeWidthStr::width(w.text.as_str()))
                        .sum();
                    let text_width = word_widths + gaps;
                    let deficit = avail.saturating_sub(text_width);
                    let extra_per = deficit / gaps;
                    let remainder = deficit % gaps;

                    for (j, word) in wl.iter().enumerate() {
                        line_spans.push(Span::styled(word.text.clone(), word.style));
                        if j < gaps {
                            let spaces = 1 + extra_per + if j < remainder { 1 } else { 0 };
                            line_spans.push(Span::raw(" ".repeat(spaces)));
                        }
                    }
                }
            }

            self.push_line(Line::from(line_spans));
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

    fn build_first_prefix(&self) -> String {
        let mut p = String::new();
        for _ in 0..self.in_blockquote {
            p.push_str(self.style.quote_indicator);
        }
        if let Some(ref item) = self.item_prefix {
            p.push_str(item);
        }
        p
    }

    fn build_cont_prefix(&self) -> String {
        let mut p = String::new();
        for _ in 0..self.in_blockquote {
            p.push_str(self.style.quote_indicator);
        }
        p
    }

    fn spans_to_words(spans: &[Span<'static>]) -> Vec<StyledWord> {
        let mut words: Vec<StyledWord> = Vec::new();
        let mut current = String::new();
        let mut current_style: Option<Style> = None;

        for span in spans {
            for ch in span.content.chars() {
                if ch.is_ascii_whitespace() {
                    if !current.is_empty() {
                        words.push(StyledWord {
                            text: std::mem::take(&mut current),
                            style: current_style.unwrap_or(span.style),
                        });
                        current_style = None;
                    }
                } else {
                    if current.is_empty() {
                        current_style = Some(span.style);
                    }
                    current.push(ch);
                }
            }
        }
        if !current.is_empty() {
            words.push(StyledWord {
                text: current,
                style: current_style.unwrap_or_default(),
            });
        }
        words
    }

    fn justify_word_wrap<'b>(
        &self,
        words: &'b [StyledWord],
        first_avail: usize,
        cont_avail: usize,
    ) -> Vec<Vec<&'b StyledWord>> {
        let mut lines: Vec<Vec<&StyledWord>> = Vec::new();
        let mut current: Vec<&StyledWord> = Vec::new();
        let mut current_width: usize = 0;

        for word in words {
            let w = UnicodeWidthStr::width(word.text.as_str());
            let avail = if lines.is_empty() {
                first_avail
            } else {
                cont_avail
            };

            if current.is_empty() {
                current.push(word);
                current_width = w;
            } else if current_width + 1 + w <= avail {
                current.push(word);
                current_width += 1 + w;
            } else {
                lines.push(current);
                current = vec![word];
                current_width = w;
            }
        }
        if !current.is_empty() {
            lines.push(current);
        }
        lines
    }

    fn map_alignment(aln: Alignment) -> Option<RatatuiAlignment> {
        match aln {
            Alignment::Center => Some(RatatuiAlignment::Center),
            Alignment::Right => Some(RatatuiAlignment::Right),
            _ => None,
        }
    }

    fn apply_alignment(&self, lines: &mut [Line<'static>]) {
        let aln = Self::map_alignment(self.block_alignment);
        for line in lines.iter_mut() {
            line.alignment = aln;
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

    #[test]
    fn code_block() {
        let result = render_markdown("```\nlet x = 1;\n```", &default_style(), 80);
        assert_eq!(result.lines.len(), 1);
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
    fn spans_to_words_contractions() {
        let spans = vec![
            Span::raw("it"),
            Span::raw("\u{2019}"),
            Span::raw("s a test"),
        ];
        let words = Renderer::spans_to_words(&spans);
        let texts: Vec<_> = words.iter().map(|w| w.text.as_str()).collect();
        assert_eq!(texts, ["it\u{2019}s", "a", "test"]);
    }

    #[test]
    fn spans_to_words_quotes() {
        let spans = vec![
            Span::raw("\u{201C}"),
            Span::raw("hello"),
            Span::raw("\u{201D}"),
        ];
        let words = Renderer::spans_to_words(&spans);
        let texts: Vec<_> = words.iter().map(|w| w.text.as_str()).collect();
        assert_eq!(texts, ["\u{201C}hello\u{201D}"]);
    }
}
