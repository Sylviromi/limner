//! Terminal image rendering via [`ratatui-image`](ratatui_image).
//!
//! Provides helpers for creating terminal image protocols from decoded images,
//! and for placing inline images within rendered markdown content.
//!
//! Uses `ratatui-image` which auto-detects Kitty protocol, Sixel, or half-block
//! fallback.  Images are rendered as native ratatui widgets inside the frame
//! buffer — they scroll, clear, and clip automatically.

use std::collections::HashMap;

use ratatui::layout::{Alignment, Rect, Size};
use ratatui::text::Line;
use ratatui::widgets::{Paragraph, Wrap};

use crate::ImageInfo;

pub use crate::image_store::{ImageHit, ImageStore, PendingImage};

/// Re-export the `image` crate for callers that need to decode images.
pub use image as img_crate;

/// Re-export key `ratatui-image` types for convenience.
pub use ratatui_image::{protocol::Protocol, FontSize, Image, Resize};

/// Re-export the picker and protocol type.
pub use ratatui_image::picker::{Picker, ProtocolType};

/// Re-export sliced image types for scrollable/clipped image rendering.
pub use ratatui_image::sliced::{SignedPosition, SlicedImage, SlicedProtocol};

/// Result of preparing images for inline rendering within markdown content.
///
/// Returned by [`ImageStore::prepare`] and [`prepare_inline_images`];
/// [`ImageStore::render`] draws them.
pub struct ImagePlacement {
    /// The image URL (used as key into the protocol cache).
    pub url: String,
    /// 0-based index within the rendered line buffer where the image starts.
    pub line_start: usize,
    /// Visual row the image starts on, with the lines wrapped at the width
    /// passed to `prepare` (equal to `line_start` when no line wraps).
    pub row: usize,
    /// Number of terminal columns (character cells) the image occupies.
    pub cell_cols: u16,
    /// Number of terminal rows the image occupies.
    pub cell_rows: u16,
    /// Horizontal alignment within the content area (None = left-aligned).
    pub alignment: Option<Alignment>,
}

/// Compute the optimal cell dimensions for an image, fitting within
/// `max_cols × max_rows` while preserving aspect ratio.
///
/// Never upscales — the image is only ever shrunk to fit.
/// Returns `(cols, rows)` both at least 1.
pub fn fit_cell_size(
    img: &img_crate::DynamicImage,
    font_size: &FontSize,
    max_cols: u16,
    max_rows: u16,
) -> (u16, u16) {
    if max_cols == 0 || max_rows == 0 || img.width() == 0 || img.height() == 0 {
        return (max_cols.max(1), max_rows.max(1));
    }

    let cell_w = font_size.width as f64;
    let cell_h = font_size.height as f64;

    let img_cols = (img.width() as f64 / cell_w).ceil();
    let img_rows = (img.height() as f64 / cell_h).ceil();

    let scale_x = max_cols as f64 / img_cols;
    let scale_y = max_rows as f64 / img_rows;
    let scale = scale_x.min(scale_y).min(1.0);

    let cols = (img_cols * scale).ceil().max(1.0) as u16;
    let rows = (img_rows * scale).ceil().max(1.0) as u16;

    (cols.min(max_cols), rows.min(max_rows))
}

/// Create a fixed-size [`Protocol`] from a decoded image.
///
/// The image is scaled to fit within `cell_cols × cell_rows` terminal cells
/// while preserving aspect ratio (via `Resize::Fit`).
///
/// Note: the `Size` passed to `picker.new_protocol` is in **cell units** (not
/// pixels).  For halfblocks this is the number of `▀` characters to use.
pub fn make_protocol(
    picker: &Picker,
    img: &img_crate::DynamicImage,
    cell_cols: u16,
    cell_rows: u16,
) -> Option<Protocol> {
    let size = ratatui::layout::Size::new(cell_cols, cell_rows);
    picker
        .new_protocol(img.clone(), size, Resize::Fit(None))
        .ok()
}

/// Create a [`SlicedProtocol`] from a decoded image.
///
/// The image is scaled to fit within `cell_cols × cell_rows` terminal cells
/// while preserving aspect ratio, then encoded into a protocol that supports
/// efficient row-level clipping for scrolling via [`SlicedImage`].
///
/// Unlike [`make_protocol`], this consumes the `DynamicImage` and sets up
/// internal row slicing so that scrolling/clipping costs zero per-frame work.
pub fn make_sliced_protocol(
    picker: &Picker,
    img: img_crate::DynamicImage,
    cell_cols: u16,
    cell_rows: u16,
) -> Option<SlicedProtocol> {
    let size = ratatui::layout::Size::new(cell_cols, cell_rows);
    SlicedProtocol::new(picker, img, Some(size)).ok()
}

/// Create a halfblock-only [`Picker`] (works on every terminal).
///
/// Use this instead of [`Picker::from_query_stdio`] when you want reliable
/// cross-terminal image rendering without Kitty/Sixel protocol detection.
pub fn halfblock_picker() -> Picker {
    Picker::halfblocks()
}

/// Describes the visible viewport for computing image render positions.
///
/// Pass this to [`compute_image_signed_positions`] along with the placements
/// and line buffer.
pub struct ImageViewport {
    /// The area where content is rendered (typically `block.inner(terminal_area)`).
    pub content: Rect,
    /// Current scroll offset in rows.
    pub scroll: u16,
}

/// Describes where to render a [`SlicedImage`] widget.
///
/// Returned by [`compute_image_signed_positions`].  The caller should
/// look up the [`SlicedProtocol`] from `sliced_protocol_cache` and render
/// via `SlicedImage::new(&sliced, position)` at the content area.
///
/// The [`SignedPosition`] is relative to the content area passed to
/// `f.render_widget()` — the [`SlicedImage`] widget handles all row-level
/// skip/drop and column-level clamping automatically.
pub struct ImageSlicedRender {
    /// The image URL (key into `sliced_protocol_cache`).
    pub url: String,
    /// Position relative to the content area (may be negative for off-screen start).
    pub position: SignedPosition,
}

/// Compute signed positions for sliced image rendering.
///
/// Takes the output of [`prepare_inline_images`] and the current viewport,
/// returning a signed position for each visible or partially-visible image.
/// Fully off-screen images are left out.
///
/// `lines` is the full line buffer after `prepare_inline_images` has replaced
/// image placeholders. With an [`ImageStore`], use [`ImageStore::render`]
/// instead, which also draws the images.
pub fn compute_image_signed_positions(
    placements: &[ImagePlacement],
    lines: &[Line],
    viewport: &ImageViewport,
) -> Vec<ImageSlicedRender> {
    let rows = row_starts(lines, viewport.content.width);
    placements
        .iter()
        .filter_map(|p| {
            let row = rows[p.line_start.min(lines.len())];
            let position = image_position(p, row, viewport.content, viewport.scroll)?;
            Some(ImageSlicedRender {
                url: p.url.clone(),
                position,
            })
        })
        .collect()
}

/// Prepare images for inline rendering within markdown content.
///
/// **Pass 1** — For every image whose URL is in `image_cache` but not yet in
/// `sliced_protocol_cache`, a new [`SlicedProtocol`] is created and inserted.
/// The [`DynamicImage`](img_crate::DynamicImage) is consumed (removed from
/// `image_cache`) on success.
///
/// **Pass 2** — Each image placeholder (1 line) in `lines` is replaced with
/// `cell_rows` empty lines to reserve space.  An [`ImagePlacement`] is returned
/// describing where the caller should position the [`SlicedImage`] widget.
///
/// Images whose URL is NOT in either cache are skipped — the placeholder text
/// (e.g. `🖼 alt`) remains visible in the rendered content.
///
/// `max_cols` should be the content area width in terminal columns.
///
/// [`ImageStore`] does all of this (plus loading state and drawing) without
/// caches of your own; see the changelog for how to move to it.
#[allow(clippy::too_many_arguments)]
pub fn prepare_inline_images(
    lines: &mut Vec<Line<'static>>,
    images: &[ImageInfo],
    image_cache: &mut HashMap<String, img_crate::DynamicImage>,
    sliced_protocol_cache: &mut HashMap<String, SlicedProtocol>,
    picker: &Picker,
    font_size: &FontSize,
    max_cols: u16,
    max_rows: u16,
) -> Vec<ImagePlacement> {
    // Pass 1 — lazily create SlicedProtocols for newly cached images.
    for img in images {
        if sliced_protocol_cache.contains_key(&img.url) {
            continue;
        }
        if let Some(dyn_img) = image_cache.remove(&img.url) {
            let (cols, rows) = fit_cell_size(&dyn_img, font_size, max_cols, max_rows);
            if let Some(sliced) = make_sliced_protocol(picker, dyn_img, cols, rows) {
                sliced_protocol_cache.insert(img.url.clone(), sliced);
            }
        }
    }

    place_images(
        lines,
        images,
        |url| sliced_protocol_cache.get(url).map(SlicedProtocol::size),
        |_, _| None,
        false,
        max_cols,
    )
}

/// Pass 2 of image layout, shared with [`ImageStore`]: reserve `size_of(url)`
/// rows for each drawable image — in place of its placeholder line, or below
/// it with `keep_placeholder` — and record its placement. Placeholders of the
/// other images get `pending`'s line, if it returns one. `width` is the width
/// the lines are drawn at, used for [`ImagePlacement::row`].
pub(crate) fn place_images(
    lines: &mut Vec<Line<'static>>,
    images: &[ImageInfo],
    size_of: impl Fn(&str) -> Option<Size>,
    pending: impl Fn(&ImageInfo, &Line<'static>) -> Option<Line<'static>>,
    keep_placeholder: bool,
    width: u16,
) -> Vec<ImagePlacement> {
    let mut sorted: Vec<&ImageInfo> = images.iter().collect();
    sorted.sort_by_key(|img| img.line_index);

    // `offset` is how far earlier images pushed this one's line down.
    let mut placements = Vec::new();
    let mut offset = 0;
    for img in sorted {
        let at = img.line_index + offset;
        let Some(slot) = lines.get_mut(at) else {
            continue;
        };
        let size = size_of(&img.url).filter(|s| s.width > 0 && s.height > 0);
        let Some(size) = size else {
            if let Some(mut line) = pending(img, slot) {
                line.alignment = line.alignment.or(slot.alignment);
                *slot = line;
            }
            continue;
        };

        let alignment = slot.alignment;
        let empty = (0..size.height).map(|_| {
            let mut l = Line::from("");
            l.alignment = alignment;
            l
        });
        let line_start = if keep_placeholder { at + 1 } else { at };
        lines.splice(line_start..at + 1, empty);
        offset += line_start - at + size.height as usize - 1;

        placements.push(ImagePlacement {
            url: img.url.clone(),
            line_start,
            row: 0,
            cell_cols: size.width,
            cell_rows: size.height,
            alignment,
        });
    }

    if !placements.is_empty() {
        let rows = row_starts(lines, width);
        for p in &mut placements {
            p.row = rows[p.line_start];
        }
    }
    placements
}

/// Visual row each line starts on once wrapped at `width`, plus the total
/// row count as the last entry (`lines.len() + 1` entries).
pub(crate) fn row_starts(lines: &[Line], width: u16) -> Vec<usize> {
    let mut rows = Vec::with_capacity(lines.len() + 1);
    let mut row = 0;
    rows.push(0);
    for line in lines {
        row += if line.width() <= width as usize {
            1
        } else {
            Paragraph::new(line.clone())
                .wrap(Wrap { trim: false })
                .line_count(width)
        };
        rows.push(row);
    }
    rows
}

/// Where an image starting at visual `row` sits in `area` once scrolled, or
/// `None` when it is entirely outside.
pub(crate) fn image_position(
    p: &ImagePlacement,
    row: usize,
    area: Rect,
    scroll: u16,
) -> Option<SignedPosition> {
    let (cols, width) = (i32::from(p.cell_cols), i32::from(area.width));
    let x = match p.alignment {
        Some(Alignment::Center) => (width - cols) / 2,
        Some(Alignment::Right) => width - cols,
        _ => 0,
    };
    let y = row as i64 - i64::from(scroll);
    let visible =
        y + i64::from(p.cell_rows) > 0 && y < i64::from(area.height) && x + cols > 0 && x < width;
    visible.then(|| SignedPosition {
        x: x as i16,
        y: y.clamp(i64::from(i16::MIN), i64::from(i16::MAX)) as i16,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{render_markdown, render_markdown_with_extra, MarkdownStyle};

    fn texts(lines: &[Line]) -> Vec<String> {
        lines.iter().map(|l| l.to_string()).collect()
    }

    #[test]
    fn prepare_inline_images_still_replaces_loaded_placeholders() {
        let result = render_markdown("a\n\n![cat](a.png)\n\nb", &MarkdownStyle::default(), 40);
        let mut lines = result.lines;
        let mut decoded = HashMap::from([(
            "a.png".to_string(),
            img_crate::DynamicImage::new_rgb8(100, 100),
        )]);
        let mut protocols = HashMap::new();
        let picker = halfblock_picker();
        let placements = prepare_inline_images(
            &mut lines,
            &result.images,
            &mut decoded,
            &mut protocols,
            &picker,
            &picker.font_size(),
            40,
            10,
        );
        assert!(decoded.is_empty() && protocols.contains_key("a.png"));
        let [p] = &placements[..] else {
            panic!("one placement");
        };
        assert_eq!((p.line_start, p.row), (2, 2));
        let laid_out = texts(&lines);
        assert_eq!(laid_out.first().map(String::as_str), Some("a"));
        assert_eq!(laid_out.last().map(String::as_str), Some("b"));
        assert!(!laid_out.iter().any(|l| l.contains("cat")));

        let viewport = ImageViewport {
            content: Rect::new(0, 0, 40, 5),
            scroll: 1,
        };
        let positions = compute_image_signed_positions(&placements, &lines, &viewport);
        let pos = &positions[0].position;
        assert_eq!((pos.x, pos.y), (0, 1));
    }

    #[test]
    fn extra_images_get_their_own_lines_and_keep_the_text() {
        let result = render_markdown_with_extra(
            "first line\n\n![cat](a.png)",
            &MarkdownStyle::default(),
            40,
            &[
                "x.png".into(),
                "a.png".into(),
                "y.png".into(),
                "x.png".into(),
            ],
        );
        let urls: Vec<&str> = result.images.iter().map(|i| i.url.as_str()).collect();
        assert_eq!(urls, ["a.png", "x.png", "y.png"]);
        let rendered = texts(&result.lines);
        assert_eq!(rendered[2], "first line");
        for img in &result.images {
            assert!(rendered[img.line_index].contains("image") || img.alt == "cat");
        }

        let mut decoded: HashMap<_, _> = ["x.png", "y.png", "a.png"]
            .map(|u| (u.to_string(), img_crate::DynamicImage::new_rgb8(50, 50)))
            .into();
        let mut lines = result.lines.clone();
        let picker = halfblock_picker();
        let placements = prepare_inline_images(
            &mut lines,
            &result.images,
            &mut decoded,
            &mut HashMap::new(),
            &picker,
            &picker.font_size(),
            40,
            10,
        );
        assert_eq!(placements.len(), 3);
        assert!(texts(&lines).iter().any(|l| l == "first line"));
    }
}
