//! Interactive demo: renders a comprehensive markdown fixture with limner.
//!
//! Run: `cargo run --example demo`
//! With image rendering: `cargo run --example demo --features image-protocol`
//!   - Uses ratatui-image: auto-detects Kitty / Sixel / half-block support.
//!
//! Controls: Up/Down to scroll, Q/Esc to quit.

use std::io::stdout;

use crossterm::event::{self, Event, KeyCode};
use ratatui::layout::Rect;
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use ratatui::Terminal;

use limner::{render_markdown, MarkdownStyle};

fn main() -> std::io::Result<()> {
    crossterm::terminal::enable_raw_mode()?;
    crossterm::execute!(stdout(), crossterm::terminal::EnterAlternateScreen)?;
    let mut terminal = Terminal::new(ratatui::backend::CrosstermBackend::new(stdout()))?;

    let mut scroll: u16 = 0;

    #[cfg(feature = "image-protocol")]
    let mut state = {
        use limner::render_image::Picker;
        use std::collections::HashMap;

        ImageDemoState {
            image_cache: HashMap::new(),
            protocol_cache: HashMap::new(),
            // Auto-detect Kitty / Sixel / halfblock.
            picker: Picker::from_query_stdio()
                .unwrap_or_else(|_| limner::render_image::halfblock_picker()),
        }
    };
    // Draw an empty frame to flush any probe escape sequences.
    #[cfg(feature = "image-protocol")]
    terminal.draw(|_| {})?;

    loop {
        let size = terminal.size()?;
        let area: Rect = size.into();
        let content_width = area.width.saturating_sub(2);
        let style = MarkdownStyle::default();
        #[allow(unused_mut)]
        let mut result = render_markdown(FIXTURE, &style, content_width);
        let line_count = result.lines.len();
        let img_count = result.images.len();
        let link_count = result.links.len();

        #[cfg(feature = "image-protocol")]
        let placements = {
            // Blocking download of images not yet in cache.
            for img in &result.images {
                if !state.image_cache.contains_key(&img.url) {
                    let url = img.url.clone();
                    if let Some(bytes) = fetch_image(&url) {
                        if let Ok(dyn_img) =
                            limner::render_image::img_crate::load_from_memory(&bytes)
                        {
                            state.image_cache.insert(url, dyn_img);
                        }
                    }
                }
            }

            let font_size = state.picker.font_size();
            limner::render_image::prepare_inline_images(
                &mut result.lines,
                &result.images,
                &state.image_cache,
                &mut state.protocol_cache,
                &state.picker,
                &font_size,
                content_width,
                10,
            )
        };

        let block = Block::default()
            .title(" limner demo ")
            .borders(Borders::ALL)
            .title_bottom(format!(
                " {scroll}/{line_count} lines · {img_count} images · {link_count} links ",
            ));
        let inner = block.inner(area);

        terminal.draw(|f| {
            f.render_widget(block, area);
            f.render_widget(
                Paragraph::new(result.lines.clone())
                    .wrap(Wrap { trim: false })
                    .scroll((scroll, 0)),
                inner,
            );

            #[cfg(feature = "image-protocol")]
            {
                let content_top = inner.y as i32;
                let content_bottom = (inner.y + inner.height) as i32;
                for p in &placements {
                    let Some(protocol) = state.protocol_cache.get(&p.url) else {
                        continue;
                    };

                    // Visual line index (accounts for word-wrap).
                    let visual_y = if p.line_start == 0 {
                        0
                    } else {
                        let end = p.line_start.min(result.lines.len());
                        Paragraph::new(result.lines[..end].to_vec())
                            .wrap(Wrap { trim: false })
                            .line_count(inner.width)
                            .max(1) as u16
                    };

                    let y0 = content_top + visual_y as i32 - scroll as i32;
                    let y1 = y0 + p.cell_rows as i32;

                    if y0 < content_top || y1 > content_bottom {
                        continue;
                    }

                    f.render_widget(
                        limner::render_image::Image::new(protocol),
                        Rect {
                            x: inner.x,
                            y: y0 as u16,
                            width: p.cell_cols,
                            height: p.cell_rows,
                        },
                    );
                }
            }
        })?;

        if let Event::Key(key) = event::read()? {
            match key.code {
                KeyCode::Char('q') | KeyCode::Esc => break,
                KeyCode::Up => scroll = scroll.saturating_sub(1),
                KeyCode::Down => scroll = scroll.saturating_add(1),
                KeyCode::PageUp => scroll = scroll.saturating_sub(10),
                KeyCode::PageDown => scroll = scroll.saturating_add(10),
                _ => {}
            }
        }
    }

    crossterm::terminal::disable_raw_mode()?;
    crossterm::execute!(std::io::stdout(), crossterm::terminal::LeaveAlternateScreen)?;
    Ok(())
}

#[cfg(feature = "image-protocol")]
struct ImageDemoState {
    image_cache: std::collections::HashMap<String, limner::render_image::img_crate::DynamicImage>,
    protocol_cache: std::collections::HashMap<String, limner::render_image::Protocol>,
    picker: limner::render_image::Picker,
}

#[cfg(feature = "image-protocol")]
fn fetch_image(url: &str) -> Option<Vec<u8>> {
    use std::io::Read;
    let resp = ureq::get(url)
        .set("User-Agent", "limner-demo/0.1")
        .timeout(std::time::Duration::from_secs(15))
        .call()
        .ok()?;
    let mut reader = resp.into_reader();
    let mut bytes = Vec::new();
    reader.read_to_end(&mut bytes).ok()?;
    Some(bytes)
}

const FIXTURE: &str = r#"# Heading 1

This paragraph contains **bold**, *italic*, ~~strikethrough~~, and `inline code`.

> A blockquote with multiple lines.
> It can span several paragraphs.

## Heading 2

Here is a [link to the stars](https://example.com).

An inline image: ![Rust logo](https://rust-lang.org/logos/rust-logo-512x512.png)

### Heading 3

Ordered list:

1. First item
2. Second item
3. Third item with **bold inside**

Unordered list:

- Bullet one
- Bullet two
  - Nested bullet
  - Another nested
- Bullet three

Code block with syntax hint:

```rust
fn main() {
    println!("Hello, world!");
    let x = vec![1, 2, 3];
    println!("{x:?}");
}
```

Plain code block:

```
$ cargo run --example demo
  Compiling limner v0.1.0
```

Horizontal rule:

---

Mix of inline elements: *italic and **bold** together*, ~~strikethrough with `code`~~.

Another image inline: ![Ferris](https://rustacean.net/assets/rustacean-flat-happy.png)

Final paragraph with a hard break at the end.  
This should appear on a new line.
"#;
