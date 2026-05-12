//! A [ratatui] markdown renderer with image placeholders, code blocks,
//! styled headings, lists, blockquotes, links, and more.
//!
//! # Quick start
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
//! # Per-section alignment
//!
//! Every block-level element has a corresponding `*_alignment` field.
//! Set them to `Alignment::Center`, `Alignment::Right`, or `Alignment::Justify`
//! to control how that element is laid out:
//!
//! ```rust
//! use limner::{render_markdown, Alignment, MarkdownStyle};
//!
//! let style = MarkdownStyle {
//!     paragraph_alignment: Alignment::Justify,
//!     heading_1_alignment: Alignment::Center,
//!     code_block_alignment: Alignment::Right,
//!     ..MarkdownStyle::default()
//! };
//!
//! let _result = render_markdown("# Title\n\nLong paragraph here...", &style, 60);
//! ```
//!
//! See the [`MarkdownStyle`] struct and [`Alignment`] enum for all available
//! options.  The [`render_markdown`] function and [`MarkdownStyle`] struct
//! for details.

pub mod layout;
pub mod render;
pub mod style;

#[cfg(feature = "image-protocol")]
pub mod render_image;

pub use pulldown_cmark;
pub use render::{render_markdown, render_markdown_with_extra, ImageInfo, LinkInfo, RenderResult};
pub use style::{Alignment, MarkdownStyle};
