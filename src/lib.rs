//! A [ratatui] markdown renderer with image placeholders, code blocks,
//! styled headings, lists, blockquotes, links, and more.
//!
//! Parse markdown into ratatui text lines ready for a `Paragraph` widget:
//!
//! ```rust
//! use limner::{render_markdown, MarkdownStyle};
//!
//! let lines = render_markdown("# Hello **world**", &MarkdownStyle::default(), 80);
//! assert_eq!(lines.lines.len(), 1);
//! ```
//!
//! See the [`render_markdown`] function and [`MarkdownStyle`] struct for details.

pub mod layout;
pub mod render;
pub mod style;

#[cfg(feature = "image-protocol")]
pub mod render_image;

pub use pulldown_cmark;
pub use render::{render_markdown, render_markdown_with_extra, ImageInfo, LinkInfo, RenderResult};
pub use style::MarkdownStyle;
