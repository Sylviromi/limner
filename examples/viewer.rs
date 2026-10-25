//! Minimal scrollable markdown viewer — the smallest complete way to use limner.
//!
//! Run: `cargo run --example viewer -- path/to/file.md` (defaults to README.md)
//! Add `--features syntax-highlighting` for coloured code blocks.
//!
//! Keys: Up/Down or j/k scroll, PageUp/PageDown scroll faster, q/Esc quit.

use std::{env, fs, io};

use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use limner::{render_markdown, MarkdownStyle};
use ratatui::widgets::{Block, Paragraph, Wrap};

fn main() -> io::Result<()> {
    let path = env::args().nth(1).unwrap_or_else(|| "README.md".into());
    let markdown = fs::read_to_string(&path)?;
    let style = MarkdownStyle::default();

    let mut terminal = ratatui::init();
    let mut scroll: u16 = 0;
    let result = loop {
        let drawn = terminal.draw(|frame| {
            let block = Block::bordered().title(path.as_str());
            let area = block.inner(frame.area());

            // Render at the width you will display at: code blocks, tables and
            // rules are laid out to fit it exactly.  Re-render on resize.
            let lines = render_markdown(&markdown, &style, area.width).lines;
            let paragraph = Paragraph::new(lines).wrap(Wrap { trim: false });

            let max_scroll = paragraph
                .line_count(area.width)
                .saturating_sub(area.height as usize);
            scroll = scroll.min(max_scroll as u16);
            frame.render_widget(paragraph.block(block).scroll((scroll, 0)), frame.area());
        });
        if let Err(e) = drawn {
            break Err(e);
        }

        match event::read() {
            Ok(Event::Key(key)) if key.kind == KeyEventKind::Press => match key.code {
                KeyCode::Char('q') | KeyCode::Esc => break Ok(()),
                KeyCode::Down | KeyCode::Char('j') => scroll = scroll.saturating_add(1),
                KeyCode::Up | KeyCode::Char('k') => scroll = scroll.saturating_sub(1),
                KeyCode::PageDown => scroll = scroll.saturating_add(10),
                KeyCode::PageUp => scroll = scroll.saturating_sub(10),
                _ => {}
            },
            Ok(_) => {}
            Err(e) => break Err(e),
        }
    };
    ratatui::restore();
    result
}
