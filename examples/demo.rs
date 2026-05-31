//! Interactive demo: tests all per-section alignment modes with edge cases.
//!
//! Run: `cargo run --example demo`
//!
//! Controls: Up/Down to scroll, Q/Esc to quit.

use std::io::stdout;

use crossterm::event::{self, Event, KeyCode};
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use ratatui::Terminal;

use limner::{render_markdown, Alignment, MarkdownStyle};

// ── section runner ──────────────────────────────────────────────────────────

struct Section {
    label: &'static str,
    text: &'static str,
    style: MarkdownStyle,
}

fn run(
    sections: &[Section],
    width: u16,
) -> (
    Vec<Line<'static>>,
    Vec<limner::ImageInfo>,
    Vec<limner::LinkInfo>,
) {
    let mut out = Vec::new();
    let mut images = Vec::new();
    let mut links = Vec::new();

    for (i, s) in sections.iter().enumerate() {
        if i > 0 {
            // separator
            let sep = format!("── {} ", s.label);
            out.push(Line::from(Span::styled(
                sep,
                Style::new().fg(Color::Rgb(100, 100, 100)),
            )));
        }

        let result = render_markdown(s.text, &s.style, width);
        let offset = out.len();
        out.extend(result.lines);
        for img in result.images {
            images.push(limner::ImageInfo {
                line_index: img.line_index + offset,
                ..img
            });
        }
        for link in result.links {
            links.push(limner::LinkInfo {
                line_index: link.line_index + offset,
                ..link
            });
        }
    }

    (out, images, links)
}

// ── test sections ───────────────────────────────────────────────────────────

fn sections(
    width: u16,
) -> (
    Vec<Line<'static>>,
    Vec<limner::ImageInfo>,
    Vec<limner::LinkInfo>,
) {
    let base = MarkdownStyle::default();

    fn txt(s: &str) -> String {
        s.to_string()
    }

    let sections = vec![
        // 1 ── Left (default baseline) ──────────────────────────
        Section {
            label: "Left (default)",
            text: txt(
                "This paragraph is **left-aligned** (the default). It contains *italic*, \
                 `inline code`, and ~~strikethrough~~. This is just the baseline to confirm \
                 nothing broke.\n\n\
                 Short para.",
            )
            .leak(),
            style: MarkdownStyle { ..base.clone() },
        },
        // 2 ── Center ──────────────────────────────────────────
        Section {
            label: "Center",
            text: txt("# Centered Title\n\n\
                 This paragraph is **centered**. Multiple sentences should all appear \
                 centered on screen.")
            .leak(),
            style: MarkdownStyle {
                heading_1_alignment: Alignment::Center,
                paragraph_alignment: Alignment::Center,
                ..base.clone()
            },
        },
        // 3 ── Right ───────────────────────────────────────────
        Section {
            label: "Right",
            text: txt("## Right-Aligned Heading\n\n\
                 This whole paragraph hugs the right edge of the terminal. \
                 Every line should be flush right.")
            .leak(),
            style: MarkdownStyle {
                heading_2_alignment: Alignment::Right,
                paragraph_alignment: Alignment::Right,
                ..base.clone()
            },
        },
        // 4 ── Justify (multi-line) ────────────────────────────
        Section {
            label: "Justify",
            text: txt(
                "This long paragraph is justified. Every line except the last is padded with \
                 extra spaces so that both the left and right edges are perfectly aligned. \
                 This is the classic newspaper-style typesetting. The last line stays \
                 left-aligned as is conventional. Make sure this wraps to at least three or \
                 four lines so the space distribution is clearly visible.",
            )
            .leak(),
            style: MarkdownStyle {
                paragraph_alignment: Alignment::Justify,
                ..base.clone()
            },
        },
        // 5 ── Justify + inline styles ─────────────────────────
        Section {
            label: "Justify + inline styles",
            text: txt(
                "This **justified** paragraph contains *inline* styling like `bold`, \
                 *italic*, and `inline code`. The words themselves carry their own styling \
                 while the extra padding spaces between them use the base paragraph style. \
                 This sentence is specifically written so that it wraps across multiple lines.",
            )
            .leak(),
            style: MarkdownStyle {
                paragraph_alignment: Alignment::Justify,
                ..base.clone()
            },
        },
        // 6 ── Justify + edge: single word ─────────────────────
        Section {
            label: "Justify — single word (edge case)",
            text: txt("Hello").leak(),
            style: MarkdownStyle {
                paragraph_alignment: Alignment::Justify,
                ..base.clone()
            },
        },
        // 7 ── Justify + edge: single short line ────────────────
        Section {
            label: "Justify — single short line (edge case)",
            text: txt("Hello world, this fits on one line.").leak(),
            style: MarkdownStyle {
                paragraph_alignment: Alignment::Justify,
                ..base.clone()
            },
        },
        // 8 ── Justify + hard break ─────────────────────────────
        Section {
            label: "Justify — hard break",
            text: txt("This line has a hard break.  \
                 This is the line after the hard break. Both should be handled properly.")
            .leak(),
            style: MarkdownStyle {
                paragraph_alignment: Alignment::Justify,
                ..base.clone()
            },
        },
        // 9 ── Blockquote centered ──────────────────────────────
        Section {
            label: "Blockquote — Center",
            text: txt(
                "> This blockquote is centered. The quote indicator should appear at the \
                 start of each line.",
            )
            .leak(),
            style: MarkdownStyle {
                quote_alignment: Alignment::Center,
                ..base.clone()
            },
        },
        // 10 ── Blockquote justified + continuation indicators ──
        Section {
            label: "Blockquote — Justify",
            text: txt(
                "> This blockquote uses justified alignment. Every line except the last is \
                 padded with extra spaces to fill the full width. The quote indicator \
                 should appear at the start of every wrapped line, showing proper \
                 blockquote continuation rendering. This sentence adds more length.",
            )
            .leak(),
            style: MarkdownStyle {
                quote_alignment: Alignment::Justify,
                ..base.clone()
            },
        },
        // 11 ── Nested blockquote ───────────────────────────────
        Section {
            label: "Blockquote — nested",
            text: txt("> Outer level\n\
                 >> Inner level with a bit more text so we can see the double indicator\n\
                 > Back to outer.")
            .leak(),
            style: MarkdownStyle { ..base.clone() },
        },
        // 12 ── Code block centered ─────────────────────────────
        Section {
            label: "Code block — Center",
            text: txt("```rust\nfn greet() {\n    println!(\"hello\");\n}\n```").leak(),
            style: MarkdownStyle {
                code_block_alignment: Alignment::Center,
                ..base.clone()
            },
        },
        // 13 ── Code block right ────────────────────────────────
        Section {
            label: "Code block — Right",
            text: txt("```\nfn greet() {\n    println!(\"hello\");\n}\n```").leak(),
            style: MarkdownStyle {
                code_block_alignment: Alignment::Right,
                ..base.clone()
            },
        },
        // 14 ── HR centered ─────────────────────────────────────
        Section {
            label: "HR — Center",
            text: txt("---").leak(),
            style: MarkdownStyle { ..base.clone() },
        },
        // 15 ── HR right ────────────────────────────────────────
        Section {
            label: "HR — Right",
            text: txt("---").leak(),
            style: MarkdownStyle {
                hr_style: Style::new().fg(Color::Rgb(140, 140, 140)),
                ..base.clone()
            },
        },
        // 16 ── List justified ──────────────────────────────────
        Section {
            label: "List — Justify",
            text: txt(
                "1. First item with extra explanatory text so this wraps across multiple \
                 lines and shows justification behavior in list items.\n\
                 2. Second item that also wraps to demonstrate continuation alignment.",
            )
            .leak(),
            style: MarkdownStyle {
                paragraph_alignment: Alignment::Justify,
                ..base.clone()
            },
        },
        // 17 ── Unordered list center ───────────────────────────
        Section {
            label: "List unordered — Center",
            text: txt("- First bullet that has some text to make it wrap.\n\
                 - Second bullet with even more content to fill the available width.")
            .leak(),
            style: MarkdownStyle {
                paragraph_alignment: Alignment::Center,
                ..base.clone()
            },
        },
        // 18 ── Mixed: link + image inside justified ────────────
        Section {
            label: "Justify — with link and image",
            text: txt(
                "This justified paragraph contains a [link](https://example.com) and an \
                 inline image: ![Rust logo](https://rust-lang.org/logos/rust-logo-512x512.png). \
                 Both should appear inline with proper alignment. The placeholder image \
                 text is part of the justified flow.",
            )
            .leak(),
            style: MarkdownStyle {
                paragraph_alignment: Alignment::Justify,
                ..base.clone()
            },
        },
        // 19 ── Image in centered paragraph ──────────────────────
        Section {
            label: "Image — Center",
            text: txt("# Centered Image\n\n\
                 This paragraph is centered and contains an inline image: \
                 ![Rust logo](https://rust-lang.org/logos/rust-logo-512x512.png). \
                 The image should also be centered horizontally.")
            .leak(),
            style: MarkdownStyle {
                heading_1_alignment: Alignment::Center,
                paragraph_alignment: Alignment::Center,
                ..base.clone()
            },
        },
        // 20 ── Image in right-aligned paragraph ─────────────────
        Section {
            label: "Image — Right",
            text: txt("## Right-Aligned Image\n\n\
                 This paragraph is right-aligned with an image: \
                 ![Rust logo](https://rust-lang.org/logos/rust-logo-512x512.png). \
                 The image should hug the right edge of the terminal.")
            .leak(),
            style: MarkdownStyle {
                heading_2_alignment: Alignment::Right,
                paragraph_alignment: Alignment::Right,
                ..base.clone()
            },
        },
        // 21 ── Image in left-aligned paragraph (baseline) ───────
        Section {
            label: "Image — Left (baseline)",
            text: txt("Left-aligned paragraph with an image: \
                 ![Rust logo](https://rust-lang.org/logos/rust-logo-512x512.png). \
                 The image should stay at the left edge.")
            .leak(),
            style: MarkdownStyle { ..base.clone() },
        },
        // 22 ── Standalone centered image, no surrounding text ───
        Section {
            label: "Image — standalone center",
            text: txt("Text above the centered image.\n\n\
                 ![Rust logo](https://rust-lang.org/logos/rust-logo-512x512.png)\n\n\
                 Text below the centered image.")
            .leak(),
            style: MarkdownStyle {
                paragraph_alignment: Alignment::Center,
                ..base.clone()
            },
        },
    ];

    run(&sections, width)
}

// ── main ────────────────────────────────────────────────────────────────────

fn main() -> std::io::Result<()> {
    crossterm::terminal::enable_raw_mode()?;
    crossterm::execute!(stdout(), crossterm::terminal::EnterAlternateScreen)?;
    let mut terminal = Terminal::new(ratatui::backend::CrosstermBackend::new(stdout()))?;

    let mut scroll: u16 = 0;

    #[cfg(feature = "image-protocol")]
    let mut state = {
        use limner::render_image::ProtocolType;
        use std::collections::HashMap;

        let mut picker = limner::render_image::Picker::from_query_stdio()
            .unwrap_or_else(|_| limner::render_image::halfblock_picker());
        if std::env::var("TERM").is_ok_and(|t| t.contains("kitty"))
            && picker.protocol_type() == ProtocolType::Halfblocks
        {
            picker.set_protocol_type(ProtocolType::Kitty);
        }

        ImageDemoState {
            image_cache: HashMap::new(),
            sliced_protocol_cache: HashMap::new(),
            picker,
        }
    };
    #[cfg(feature = "image-protocol")]
    terminal.draw(|_| {})?;

    loop {
        let size = terminal.size()?;
        let area: Rect = size.into();
        let content_width = area.width.saturating_sub(2);

        #[allow(unused_mut)]
        let (mut lines, images, links) = sections(content_width);
        let img_count = images.len();
        let link_count = links.len();

        #[cfg(feature = "image-protocol")]
        let placements = {
            for img in &images {
                if !state.image_cache.contains_key(&img.url) {
                    let url = img.url.clone();
                    if let Some(bytes) = fetch_image(&url) {
                        use limner::render_image::img_crate;
                        if let Ok(dyn_img) = img_crate::load_from_memory(&bytes) {
                            state.image_cache.insert(url, dyn_img);
                        }
                    }
                }
            }
            let font_size = state.picker.font_size();
            limner::render_image::prepare_inline_images(
                &mut lines,
                &images,
                &mut state.image_cache,
                &mut state.sliced_protocol_cache,
                &state.picker,
                &font_size,
                content_width,
                10,
            )
        };

        let line_count = lines.len();

        let block = Block::default()
            .title(" limner alignment demo ")
            .borders(Borders::ALL)
            .title_bottom(format!(
                " {scroll}/{line_count} lines · {img_count} images · {link_count} links ",
            ));
        let inner = block.inner(area);
        scroll = scroll.min(line_count.saturating_sub(inner.height as usize) as u16);

        terminal.draw(|f| {
            f.render_widget(block, area);
            f.render_widget(
                Paragraph::new(lines.clone())
                    .wrap(Wrap { trim: false })
                    .scroll((scroll, 0)),
                inner,
            );

            #[cfg(feature = "image-protocol")]
            {
                use limner::render_image::{
                    compute_image_signed_positions, ImageViewport, SlicedImage,
                };

                let viewport = ImageViewport {
                    content: inner,
                    scroll,
                };
                let render_positions =
                    compute_image_signed_positions(&placements, &lines, &viewport);

                for r in &render_positions {
                    let Some(sliced) = state.sliced_protocol_cache.get(&r.url) else {
                        continue;
                    };
                    f.render_widget(SlicedImage::new(sliced, r.position), inner);
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
                KeyCode::Home => scroll = 0,
                KeyCode::End => scroll = line_count.saturating_sub(1) as u16,
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
    sliced_protocol_cache: std::collections::HashMap<String, limner::render_image::SlicedProtocol>,
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
