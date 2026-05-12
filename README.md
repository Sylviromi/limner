# limner

A [ratatui] markdown renderer with image placeholders, styled headings, code blocks,
**per-section alignment**, and more.

```toml
[dependencies]
limner = "0.1"
```

## Usage

```rust
use limner::{render_markdown, MarkdownStyle};
use ratatui::widgets::{Paragraph, Wrap};

let content = "# Hello\n\nThis is **markdown** with `code`.";

let style = MarkdownStyle {
    heading_1: ratatui::style::Style::new().green().bold(),
    ..MarkdownStyle::default()
};

let lines = render_markdown(content, &style, 80);
let paragraph = Paragraph::new(lines).wrap(Wrap { trim: false });
```

Output lines work directly with ratatui's `Paragraph` widget — scrolling,
line counting, and word-wrapping are all handled by the existing widget.

## Per-section alignment

Every block-level element has a corresponding `*_alignment` field:

| Element | Alignment field | Supported values |
|---------|----------------|-----------------|
| Paragraph | `paragraph_alignment` | `Left`, `Center`, `Right`, `Justify` |
| Heading 1 | `heading_1_alignment` | `Left`, `Center`, `Right` |
| Heading 2 | `heading_2_alignment` | `Left`, `Center`, `Right` |
| Heading 3 | `heading_3_alignment` | `Left`, `Center`, `Right` |
| Code block | `code_block_alignment` | `Left`, `Center`, `Right` |
| Blockquote | `quote_alignment` | `Left`, `Center`, `Right`, `Justify` |

`Left` is the default for all elements (backward-compatible).

```rust
use limner::{render_markdown, Alignment, MarkdownStyle};

let style = MarkdownStyle {
    heading_1_alignment: Alignment::Center,
    paragraph_alignment: Alignment::Justify,
    code_block_alignment: Alignment::Right,
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

## Features

- **Headings** (levels 1–3) with configurable styles and alignment
- **Bold, italic, strikethrough** with proper nesting
- **Inline code** and **code blocks** with full-width background and alignment
- **Links** with styled text and prefix
- **Images** rendered as placeholder text (`🖼 alt-text`)
- **Blockquotes** with per-line indicators and justified continuation lines
- **Ordered and unordered lists**
- **Horizontal rules**
- **Per-section alignment** — center, right, or justify individual elements
- **Lightweight** — only depends on `ratatui`, `pulldown-cmark`, and `unicode-width`
- **Terminal image rendering** (optional) — via `image-protocol` feature

## Terminal image rendering

Enable the `image-protocol` feature to render images via the terminal's native
graphics protocol (Kitty, Sixel, or iTerm2):

```toml
[dependencies]
limner = { version = "0.1", features = ["image-protocol"] }
```

Images in markdown (`![alt](url)`) are tracked as `ImageInfo` metadata.
After drawing the ratatui frame, call `prepare_inline_images()` to insert
`Image` widgets at the correct positions:

```rust
use limner::{render_markdown, render_image};
use std::collections::HashMap;

let result = render_markdown(&content, &style, width);
let mut protocol_cache = HashMap::new();
let picker = render_image::Picker::from_query_stdio()?;
let font_size = picker.font_size();
let placements = render_image::prepare_inline_images(
    &mut result.lines,
    &result.images,
    &image_cache,
    &mut protocol_cache,
    &picker,
    &font_size,
    content_width,
    6,
);

// Then render Image widgets inside terminal.draw() using the placements.
```

The caller is responsible for populating the `image_cache`
(`HashMap<String, DynamicImage>`).

## Demo

```sh
# Text-only (no image rendering required)
cargo run --example demo

# With terminal image rendering (Kitty/Sixel/iTerm2)
cargo run --example demo --features image-protocol
```

Controls: `Up`/`Down` to scroll, `PageUp`/`PageDown` to scroll faster,
`Home` to jump to the top, `End` to jump to the bottom, `Q`/`Esc` to quit.

## Theming

`MarkdownStyle` exposes every element's [`Style`], alignment, and text prefix.
Set only the fields you want to override:

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

## License

MIT

[ratatui]: https://github.com/ratatui/ratatui
