//! Terminal image rendering via [`ratatui-image`].
//!
//! Provides helpers for creating terminal image protocols from decoded images,
//! and for placing inline images within rendered markdown content.
//!
//! Uses `ratatui-image` which auto-detects Kitty protocol, Sixel, or half-block
//! fallback.  Images are rendered as native ratatui widgets inside the frame
//! buffer — they scroll, clear, and clip automatically.

use std::collections::HashMap;

use ratatui::text::Line;

use crate::ImageInfo;

/// Re-export the `image` crate for callers that need to decode images.
pub use image as img_crate;

/// Re-export key `ratatui-image` types for convenience.
pub use ratatui_image::{protocol::Protocol, FontSize, Image, Resize};

/// Re-export the picker.
pub use ratatui_image::picker::Picker;

/// Result of preparing images for inline rendering within markdown content.
///
/// Returned by [`prepare_inline_images`]; the caller uses the fields to position
/// [`Image`] widgets in the frame.
pub struct ImagePlacement {
    /// The image URL (used as key into the protocol cache).
    pub url: String,
    /// 0-based row index within the rendered line buffer where the image starts.
    pub line_start: usize,
    /// Number of terminal columns (character cells) the image occupies.
    pub cell_cols: u16,
    /// Number of terminal rows the image occupies.
    pub cell_rows: u16,
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

/// Create a halfblock-only [`Picker`] (works on every terminal).
///
/// Use this instead of [`Picker::from_query_stdio`] when you want reliable
/// cross-terminal image rendering without Kitty/Sixel protocol detection.
pub fn halfblock_picker() -> Picker {
    Picker::halfblocks()
}

/// Prepare images for inline rendering within markdown content.
///
/// **Pass 1** — For every image whose URL is in `cache` but not yet in
/// `protocol_cache`, a new [`Protocol`] is created and inserted.
///
/// **Pass 2** — Each image placeholder (1 line) in `lines` is replaced with
/// `cell_rows` empty lines to reserve space.  An [`ImagePlacement`] is returned
/// describing where the caller should position the [`Image`] widget.
///
/// Images whose URL is NOT in `cache` are skipped — the placeholder text
/// (e.g. `🖼 alt`) remains visible in the rendered content.
///
/// `max_rows` defaults to 6 (user preference).  `max_cols` should be the
/// content area width in terminal columns.
#[allow(clippy::too_many_arguments)]
pub fn prepare_inline_images(
    lines: &mut Vec<Line<'static>>,
    images: &[ImageInfo],
    cache: &HashMap<String, img_crate::DynamicImage>,
    protocol_cache: &mut HashMap<String, Protocol>,
    picker: &Picker,
    font_size: &FontSize,
    max_cols: u16,
    max_rows: u16,
) -> Vec<ImagePlacement> {
    let mut indexed: Vec<(usize, &ImageInfo)> = images.iter().enumerate().collect();
    indexed.sort_by_key(|a| a.1.line_index);

    // Pass 1 — lazily create protocols for newly cached images.
    for (_, img) in &indexed {
        if cache.contains_key(&img.url) && !protocol_cache.contains_key(&img.url) {
            if let Some(dyn_img) = cache.get(&img.url) {
                let (cols, rows) = fit_cell_size(dyn_img, font_size, max_cols, max_rows);
                if cols > 0 && rows > 0 {
                    if let Some(protocol) = make_protocol(picker, dyn_img, cols, rows) {
                        protocol_cache.insert(img.url.clone(), protocol);
                    }
                }
            }
        }
    }

    // Pass 2 — replace placeholder lines with empty space, record placements.
    // `cursor` tracks the next free line so images sharing the same original
    // `line_index` (e.g. multiple extra images at line 0) don't overlap.
    let mut placements = Vec::new();
    let mut offset: isize = 0;
    let mut cursor: isize = 0;

    for (_, img) in &indexed {
        let adjusted_line = (img.line_index as isize + offset) as usize;

        if !protocol_cache.contains_key(&img.url) {
            continue;
        }

        let Some(dyn_img) = cache.get(&img.url) else {
            continue;
        };

        let (cols, rows) = fit_cell_size(dyn_img, font_size, max_cols, max_rows);
        if cols == 0 || rows == 0 {
            continue;
        }

        let insert_at = (adjusted_line as isize).max(cursor) as usize;
        if insert_at >= lines.len() {
            continue;
        }

        let empty: Vec<Line<'static>> = (0..rows).map(|_| Line::from("")).collect();
        lines.splice(insert_at..=insert_at, empty);

        placements.push(ImagePlacement {
            url: img.url.clone(),
            line_start: insert_at,
            cell_cols: cols,
            cell_rows: rows,
        });

        offset += rows as isize - 1;
        cursor = insert_at as isize + rows as isize;
    }

    placements
}
