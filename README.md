# limner

A [ratatui] markdown renderer with image placeholders, styled headings, tables,
bordered code blocks with optional syntax highlighting,
**per-section alignment**, and more.

```toml
[dependencies]
limner = "0.6"
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

let result = render_markdown(content, &style, 80);
let paragraph = Paragraph::new(result.lines).wrap(Wrap { trim: false });
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
| Blockquote | `quote_alignment` | `Left`, `Center`, `Right`, `Justify` |

`Left` is the default for all elements (backward-compatible).

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

## Features

- **Headings** (levels 1–3) with configurable styles and alignment
- **Bold, italic, strikethrough** with proper nesting
- **Inline code** and **code blocks** with borders, a language label, and optional syntax highlighting
- **Tables** with borders, column alignment, and cell wrapping
- **Links** with styled text and prefix
- **Images** rendered as placeholder text (`🖼 alt-text`)
- **Blockquotes** with the indicator repeated on every wrapped line
- **Ordered and unordered lists**, with nested indentation
- **Horizontal rules**
- **Per-section alignment** — center, right, or justify individual elements
- **Lightweight** — by default only depends on `ratatui`, `pulldown-cmark`, and `unicode-width`
- **Terminal image rendering** (optional) — via `image-protocol` feature
- **Syntax highlighting** (optional) — via `syntax-highlighting` feature

## Terminal image rendering

Enable the `image-protocol` feature to render images via the terminal's native
graphics protocol (Kitty, Sixel, or iTerm2):

```toml
[dependencies]
limner = { version = "0.6", features = ["image-protocol"] }
```

Images in markdown (`![alt](url)`) are tracked as `ImageInfo` metadata.
Before drawing each frame, call `prepare_inline_images()`: it reserves rows in
the rendered lines for each image and returns `ImagePlacement`s telling you
where to draw them inside `terminal.draw()`:

```rust
use std::collections::HashMap;

use limner::render_image::{self, img_crate::DynamicImage, ImagePlacement, Picker, SlicedProtocol};
use limner::{render_markdown, MarkdownStyle};
use ratatui::text::Line;

fn layout_with_images(
    content: &str,
    width: u16,
    picker: &Picker,
    // URL → decoded image; populating it (downloading, decoding) is up to you.
    image_cache: &mut HashMap<String, DynamicImage>,
    protocol_cache: &mut HashMap<String, SlicedProtocol>,
) -> (Vec<Line<'static>>, Vec<ImagePlacement>) {
    let mut result = render_markdown(content, &MarkdownStyle::default(), width);
    let placements = render_image::prepare_inline_images(
        &mut result.lines,
        &result.images,
        image_cache,
        protocol_cache,
        picker,
        &picker.font_size(),
        width,
        10, // maximum image height, in rows
    );
    (result.lines, placements)
}
```

Create the `Picker` once at startup with `Picker::from_query_stdio()` (fall
back to `render_image::halfblock_picker()` if the query fails). See
`examples/demo.rs` for a complete scrolling viewer.

## Demo

```sh
# Text-only (no image rendering required)
cargo run --example demo

# With terminal image rendering (Kitty/Sixel/iTerm2)
cargo run --example demo --features image-protocol

# With syntax highlighting
cargo run --example demo --features syntax-highlighting
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

Enable the `syntax-highlighting` feature to colour fenced code blocks that
declare a language, using [syntect]'s bundled grammars and themes:

```toml
[dependencies]
limner = { version = "0.6", features = ["syntax-highlighting"] }
```

Pick a theme with `syntax_theme` (default `"base16-ocean.dark"`; list them with
`limner::highlight::theme_names()`), or set it to `None` to disable. Unknown
languages fall back to the plain `code_block` style.

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

## License

MIT

[ratatui]: https://github.com/ratatui/ratatui
[syntect]: https://github.com/trishume/syntect
