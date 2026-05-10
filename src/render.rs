use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};

use crate::layout;
use crate::style::MarkdownStyle;

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
    mods: Vec<Modifier>,
    block_style: Style,

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
                for m in &self.mods {
                    style = style.add_modifier(*m);
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
                self.block_style = self.style.paragraph;
                self.mods.clear();
            }
            Tag::Heading { level, .. } => {
                self.block_style = match level {
                    HeadingLevel::H1 => self.style.heading_1,
                    HeadingLevel::H2 => self.style.heading_2,
                    HeadingLevel::H3 => self.style.heading_3,
                    _ => self.style.heading_3,
                };
                self.mods.clear();
            }
            Tag::BlockQuote(_) => {
                self.flush_paragraph(self.style.paragraph);
                self.in_blockquote += 1;
            }
            Tag::CodeBlock(_) => {
                self.flush_paragraph(self.style.paragraph);
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
            Tag::Emphasis => self.mods.push(Modifier::ITALIC),
            Tag::Strong => self.mods.push(Modifier::BOLD),
            Tag::Strikethrough => self.mods.push(Modifier::CROSSED_OUT),
            Tag::Link { dest_url, .. } => {
                self.link_url = Some(dest_url.into_string());
                self.link_text.clear();
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
                self.flush_paragraph(self.style.paragraph);
                self.has_content = true;
            }
            TagEnd::Heading(level) => {
                let style = match level {
                    HeadingLevel::H1 => self.style.heading_1,
                    HeadingLevel::H2 => self.style.heading_2,
                    _ => self.style.heading_3,
                };
                self.flush_paragraph(style);
            }
            TagEnd::BlockQuote(_) => {
                self.flush_paragraph(self.style.paragraph);
                self.in_blockquote = self.in_blockquote.saturating_sub(1);
            }
            TagEnd::CodeBlock => {
                if let Some(buf) = self.code_buf.take() {
                    let lines = layout::wrap_code_block(
                        &buf,
                        self.width as usize,
                        self.style.code_block,
                        self.style.code_block_bg,
                    );
                    self.output.extend(lines);
                }
            }
            TagEnd::List(_) => {
                self.flush_paragraph(self.style.paragraph);
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
            self.spans.push(Span::raw(text.to_string()));
            self.pending_img_alt.push_str(text);
            return;
        }
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
        for m in &self.mods {
            s = s.add_modifier(*m);
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
        self.images.push(ImageInfo {
            line_index: usize::MAX,
            url,
            alt,
        });
        self.spans.push(Span::styled(display, self.style.image));
        self.is_image = false;
    }

    // ── line flushing ──────────────────────────────────────────

    fn flush_paragraph(&mut self, base_style: Style) {
        if self.spans.is_empty() && self.item_prefix.is_none() {
            return;
        }

        let mut prefix = String::new();
        for _ in 0..self.in_blockquote {
            prefix.push_str(self.style.quote_indicator);
        }
        if let Some(ref item) = self.item_prefix {
            prefix.push_str(item);
        }

        let mut line_spans: Vec<Span<'static>> = Vec::new();
        if !prefix.is_empty() {
            line_spans.push(Span::styled(prefix, base_style));
        }
        line_spans.append(&mut self.spans);
        self.output.push(Line::from(line_spans));
        let current = self.output.len() - 1;
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
}
