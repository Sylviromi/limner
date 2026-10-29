<div align="center">

# limner

**Markdown rendering for [ratatui] — styled, aligned, and ready for a `Paragraph`.**

[![Crates.io Version](https://img.shields.io/crates/v/limner?style=flat-square&color=f5c2e7)](https://crates.io/crates/limner)
[![Crates.io Downloads](https://img.shields.io/crates/d/limner?style=flat-square&color=cba6f7)](https://crates.io/crates/limner)
[![docs.rs](https://img.shields.io/docsrs/limner?style=flat-square&color=a6e3a1)](https://docs.rs/limner)
[![License: MIT](https://img.shields.io/badge/license-MIT-89dceb?style=flat-square)](LICENSE)
[![Rust: 1.86+](https://img.shields.io/badge/rust-1.86+-fab387?style=flat-square)](https://www.rust-lang.org)

Tables · bordered code blocks · optional syntax highlighting · per-section alignment · inline terminal images

</div>

---

## Features

- **Headings** (levels 1–3) with configurable styles and alignment
- **Bold, italic, strikethrough** with proper nesting
- **Inline code** and **code blocks** with borders, a language label, and optional syntax highlighting
- **Tables** with borders, column alignment, and cell wrapping
- **Links** with styled text and prefix, reported with their line for click handling
- **Images** rendered as placeholder text (`🖼 alt-text`), or as real images with `image-protocol`
- **Blockquotes** with the indicator repeated on every wrapped line
- **Ordered and unordered lists**, with nested indentation
- **Horizontal rules**
- **Per-section alignment** — center, right, or justify individual elements
- **Lightweight** — by default only depends on `ratatui`, `pulldown-cmark`, and `unicode-width`

## Installation

```toml
[dependencies]
limner = "0.9"
```

Optional features:

| Feature | Adds |
|---------|------|
| `syntax-highlighting` | Coloured code blocks via [syntect] (pure Rust, no C toolchain needed) |
| `image-protocol` | Real images via Kitty, Sixel, or iTerm2 graphics |

```toml
limner = { version = "0.9", features = ["syntax-highlighting", "image-protocol"] }
```

## Quick start

```rust
use limner::{render_markdown, MarkdownStyle};
use ratatui::widgets::{Paragraph, Wrap};

let content = "# Hello\n\nThis is **markdown** with `code`.";

let style = MarkdownStyle {
    heading_1: ratatui::style::Style::new().green().bold(),
    ..MarkdownStyle::default()
};

let result = render_markdown(content, &style, 80);
let paragraph = Paragraph::new(result.lines).wrap(Wrap { trim: false });
```

The output lines go straight into ratatui's `Paragraph` — scrolling, line
counting, and word-wrapping are all handled by the existing widget.

## Using it in an app

[`examples/viewer.rs`](examples/viewer.rs) is a complete scrollable viewer in
about 50 lines — start there. The things worth knowing:

- **Render at the width you display at.** Pass the inner width of the area the
  `Paragraph` is drawn in (after borders/padding). Code blocks, tables, and
  rules are laid out to fill exactly that width; re-render when it changes.
- **Use `Wrap { trim: false }`.** Trimming would eat code indentation and the
  padding inside tables.
- **Scroll with `Paragraph::scroll`**, clamped with `Paragraph::line_count`
  (ratatui's `unstable-rendered-line-info` feature).
- **Links and images come back as metadata.** `RenderResult::links` and
  `RenderResult::images` carry the URL and the index of the line they're on,
  for click handling or placing images.
- **Customise links and images with `MarkdownStyle::link_formatter` /
  `image_formatter`.** A `SpanFormatter` closure gets each link or image
  placeholder (its index in `RenderResult::links` / `images`, URL, text and
  spans) and returns the spans to draw instead, e.g. to put a hint label in
  front of each one.

```sh
cargo run --example viewer -- README.md
cargo run --example viewer --features syntax-highlighting -- README.md
```

There's also a bigger demo that exercises every alignment mode and edge case,
including inline images:

```sh
cargo run --example demo
cargo run --example demo --features image-protocol,syntax-highlighting
```

Demo controls: `Up`/`Down` scroll, `PageUp`/`PageDown` scroll faster,
`Home`/`End` jump, `Q`/`Esc` quit.

## Styling

`MarkdownStyle` exposes every element's ratatui `Style`, alignment, and text
prefix. Set only the fields you want to override:

```rust
use limner::{Alignment, MarkdownStyle};
use ratatui::prelude::*;

let my_style = MarkdownStyle {
    heading_1: Style::new().fg(Color::Rgb(255, 200, 100)).bold(),
    heading_1_alignment: Alignment::Center,
    code_block: Style::new().fg(Color::Rgb(200, 200, 100)),
    code_block_bg: Color::Rgb(30, 30, 30),
    quote_alignment: Alignment::Justify,
    link: Style::new().cyan().underlined(),
    ..MarkdownStyle::default()
};
```

### Alignment

Every text block has a corresponding `*_alignment` field:

| Element | Alignment field | Supported values |
|---------|----------------|-----------------|
| Paragraph | `paragraph_alignment` | `Left`, `Center`, `Right`, `Justify` |
| Heading 1 | `heading_1_alignment` | `Left`, `Center`, `Right` |
| Heading 2 | `heading_2_alignment` | `Left`, `Center`, `Right` |
| Heading 3 | `heading_3_alignment` | `Left`, `Center`, `Right` |
| Blockquote | `quote_alignment` | `Left`, `Center`, `Right`, `Justify` |

`Left` is the default for all elements.

```rust
use limner::{render_markdown, Alignment, MarkdownStyle};

let style = MarkdownStyle {
    heading_1_alignment: Alignment::Center,
    paragraph_alignment: Alignment::Justify,
    ..MarkdownStyle::default()
};

let result = render_markdown("# Title\n\nBody text...", &style, 60);
```

**Justify** — limner word-wraps paragraphs itself and distributes extra
spaces between words so every line (except the last) fills the full width.
Works correctly with inline styles (bold, italic, code) and images.

Render multiple sections with different alignments by calling
`render_markdown` separately for each section and combining the lines:

```rust
use limner::{render_markdown, Alignment, MarkdownStyle};

let title = render_markdown("# Centered Title", &MarkdownStyle {
    heading_1_alignment: Alignment::Center,
    ..MarkdownStyle::default()
}, 80);

let body = render_markdown("Justified body text...", &MarkdownStyle {
    paragraph_alignment: Alignment::Justify,
    ..MarkdownStyle::default()
}, 80);

let mut all_lines = title.lines;
all_lines.extend(body.lines);
```

### Code blocks

Fenced code blocks are framed with a border, and the language from the info
string (` ```rust `) is shown in the top edge:

```text
╭─ rust ───────────────────╮
│ fn main() {              │
│     println!("hello");   │
│ }                        │
╰──────────────────────────╯
```

Pick any ratatui border set, or set `code_block_border: None` for a
borderless block (the language then appears on a header line):

```rust
use limner::MarkdownStyle;
use ratatui::{prelude::*, symbols::border};

let style = MarkdownStyle {
    code_block_border: Some(border::PLAIN),
    code_block_border_style: Style::new().dark_gray(),
    code_block_language: Style::new().yellow().bold(),
    ..MarkdownStyle::default()
};
```

Code blocks always fill the available width (like GitHub, glow, or bat) —
code relies on its left edge for indentation, so it isn't centred.

### Syntax highlighting

With the `syntax-highlighting` feature, fenced code blocks that declare a
language are coloured using [syntect]'s bundled grammars and themes.

Pick a theme with `syntax_theme` (default `"base16-ocean.dark"`; list them with
`limner::highlight::theme_names()`), or set it to `None` to disable. Unknown
languages fall back to the plain `code_block` style. If your app has light and
dark themes, choose the syntax theme to match the code background — e.g.
`"InspiredGitHub"` on light backgrounds.

### Tables

GFM tables are drawn with box borders at their natural width, honouring
column alignment (`:--`, `:-:`, `--:`). Wide tables shrink to fit and wrap
cell text. Style them with `table_border` (any `ratatui::symbols::line::Set`),
`table_border_style`, and `table_header`.

```text
╭───────────┬────────┬───────╮
│ Feature   │ Status │ Notes │
├───────────┼────────┼───────┤
│ Tables    │  done  │    ok │
╰───────────┴────────┴───────╯
```

## Terminal images

With the `image-protocol` feature, images in markdown (`![alt](url)`) can be
drawn with the terminal's native graphics protocol (Kitty, Sixel, or iTerm2).
They're tracked as `ImageInfo` metadata, and an `ImageStore` does the rest:
you download and decode images and report them to the store; each frame it lays
them out between the text lines and draws them.

```rust
use limner::render_image::{halfblock_picker, img_crate::DynamicImage, ImageStore};
use limner::{render_markdown, MarkdownStyle};
use ratatui::{buffer::Buffer, layout::Rect};

// Once, at startup (`Picker::from_query_stdio()` picks the best protocol).
let mut store = ImageStore::new(halfblock_picker()).with_spinner();

let result = render_markdown("![a cat](cat.png)", &MarkdownStyle::default(), 80);
for img in &result.images {
    if store.start(&img.url) {
        // Download + decode in the background, then report the result
        // (`None` if it failed):
        store.finish(&img.url, Some(DynamicImage::new_rgb8(64, 64)));
    }
}

// Every frame: lay out a fresh copy of the lines, draw them as a wrapped
// `Paragraph` scrolled by `scroll` rows, then draw the images on top.
let area = Rect::new(0, 0, 80, 24);
let scroll = 0;
let mut lines = result.lines.clone();
let placements = store.prepare(&mut lines, &result.images, area.width, 10);
// frame.render_widget(Paragraph::new(lines).wrap(..).scroll((scroll, 0)), area);
let mut buf = Buffer::empty(area); // inside `terminal.draw`: `frame.buffer_mut()`
let hits = store.render(&mut buf, area, &placements, scroll);
assert_eq!(hits.len(), 1); // where each image landed, e.g. to make it clickable
```

Options, all opt-in (without them images look as they always did):

- `with_spinner()` puts an animated spinner in front of the placeholder of
  loading images; call `store.tick()` on a timer to animate it.
- `with_loading_line(|p| ...)` and `with_failed_line(|p| ...)` return your own
  line, from `p.info` (url, alt), `p.placeholder` and `p.frame`.
- `keep_placeholders()` keeps each placeholder line (alt text, link hint
  labels from `image_formatter`) above its image.

Images re-fit when the width changes. `store.retain(|url| ...)` / `clear()` free
images you no longer show, and `store.version()` changes whenever `prepare()`
would give a different result, so it can go into a layout cache key.

The lower-level `prepare_inline_images()` still works with your own caches;
[`CHANGELOG.md`](CHANGELOG.md#migrating-to-imagestore) covers moving to
`ImageStore`. See [`examples/demo.rs`](examples/demo.rs) for background loading
with a spinner.

## License

MIT

[ratatui]: https://github.com/ratatui/ratatui
[syntect]: https://github.com/trishume/syntect
