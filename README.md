# limner

A [ratatui] markdown renderer with image placeholders, styled headings, code blocks, and more.

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

## Features

- **Headings** (levels 1–3) with configurable styles
- **Bold, italic, strikethrough** with proper nesting
- **Inline code** and **code blocks** with full-width background
- **Links** with styled text and prefix
- **Images** rendered as placeholder text (`🖼 alt-text`)
- **Blockquotes** with per-line indicators
- **Ordered and unordered lists**
- **Horizontal rules**
- **Lightweight** — only depends on `ratatui`, `pulldown-cmark`, and `unicode-width`
- **Terminal image rendering** (optional) — via `image-protocol` feature; requires a terminal that supports Kitty graphics protocol, Sixel, or iTerm2

## Terminal image rendering

Enable the `image-protocol` feature to render images via the terminal's native
graphics protocol (Kitty, Sixel, or iTerm2):

```toml
[dependencies]
limner = { version = "0.1", features = ["image-protocol"] }
```

Images in markdown (`![alt](url)`) are tracked as `ImageInfo` metadata.
After drawing the ratatui frame, call `render_images()` to blit visible images:

```rust
use limner::{render_markdown, render_image::render_images};

let result = render_markdown(&content, &style, width);
terminal.draw(|f| { /* render result.lines */ })?;

// Must be called AFTER terminal.draw() — viuer writes directly to stdout.
render_images(&result.images, content_area, scroll, &image_cache);
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

## Theming

`MarkdownStyle` exposes every element's [`Style`] and text prefix.
Set only the fields you want to override:

```rust
let my_style = MarkdownStyle {
    heading_1: Style::new().fg(Color::Rgb(255, 200, 100)).bold(),
    code_block: Style::new().fg(Color::Rgb(200, 200, 100)),
    code_block_bg: Color::Rgb(30, 30, 30),
    link: Style::new().cyan().underlined(),
    ..MarkdownStyle::default()
};
```

## License

MIT

[ratatui]: https://github.com/ratatui/ratatui
