//! Terminal image rendering via [`ratatui-image`].
//!
//! Provides helpers for creating terminal image protocols from decoded images,
//! and for placing inline images within rendered markdown content.
//!
//! Uses `ratatui-image` which auto-detects Kitty protocol, Sixel, or half-block
//! fallback.  Images are rendered as native ratatui widgets inside the frame
//! buffer — they scroll, clear, and clip automatically.

use std::collections::HashMap;

use ratatui::layout::{Alignment, Rect};
use ratatui::text::Line;
use ratatui::widgets::{Paragraph, Wrap};

use crate::ImageInfo;

/// Re-export the `image` crate for callers that need to decode images.
pub use image as img_crate;

/// Re-export key `ratatui-image` types for convenience.
pub use ratatui_image::{protocol::Protocol, FontSize, Image, Resize};

/// Re-export the picker and protocol type.
pub use ratatui_image::picker::{Picker, ProtocolType};

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

/// Create a [`Protocol`] for a clipped / partially-visible image.
///
/// The original image is first scaled to `full_size` (preserving aspect ratio),
/// then `hidden_top` cell‑rows and `hidden_left` cell‑columns are sliced off,
/// yielding a protocol that matches `visible_size`.
///
/// Use this to render images that are partially off‑screen — the visible
/// portion of the image is sent to the terminal instead of the full image.
pub fn make_clipped_protocol(
    picker: &Picker,
    img: &img_crate::DynamicImage,
    full_size: ratatui::layout::Size,
    visible_size: ratatui::layout::Size,
    hidden_top: u16,
    hidden_left: u16,
) -> Option<Protocol> {
    let (fw, fh) = (
        picker.font_size().width as u32,
        picker.font_size().height as u32,
    );

    // Pixel dimensions the image would occupy at `full_size`.
    let fit_w = full_size.width as u32 * fw;
    let fit_h = full_size.height as u32 * fh;

    // Uniform scale so the image fits into (fit_w × fit_h), never upscale.
    let scale = (fit_w as f64 / img.width() as f64)
        .min(fit_h as f64 / img.height() as f64)
        .min(1.0);
    let sw = (img.width() as f64 * scale).round() as u32;
    let sh = (img.height() as f64 * scale).round() as u32;

    let scaled = img.resize_exact(sw, sh, image::imageops::FilterType::Nearest);

    // Pad to the full cell grid with transparency.
    let mut padded = image::RgbaImage::from_pixel(fit_w, fit_h, image::Rgba([0, 0, 0, 0]));
    image::imageops::overlay(&mut padded, &scaled, 0, 0);

    // Slice off hidden rows and columns in pixel space.
    let vis_pix_w = visible_size.width as u32 * fw;
    let vis_pix_h = visible_size.height as u32 * fh;
    let x_off = (hidden_left as u32 * fw).min(padded.width().saturating_sub(vis_pix_w));
    let y_off = (hidden_top as u32 * fh).min(padded.height().saturating_sub(vis_pix_h));

    let padded_dyn: img_crate::DynamicImage = padded.into();
    let cropped = padded_dyn.crop_imm(x_off, y_off, vis_pix_w, vis_pix_h);

    picker
        .new_protocol(cropped, visible_size, Resize::Fit(None))
        .ok()
}

/// Create a [`Protocol`] for a vertically-scrolled / partially-visible image.
///
/// Convenience wrapper around [`make_clipped_protocol`] with `hidden_left = 0`.
pub fn make_scrolled_protocol(
    picker: &Picker,
    img: &img_crate::DynamicImage,
    full_size: ratatui::layout::Size,
    visible_size: ratatui::layout::Size,
    hidden_top: u16,
) -> Option<Protocol> {
    make_clipped_protocol(picker, img, full_size, visible_size, hidden_top, 0)
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
/// Pass this to [`compute_image_render_rects`] along with the placements and
/// line buffer so the library can calculate where each image should appear on
/// screen — including any clipping needed when the image is partially off‑screen.
pub struct ImageViewport {
    /// The area where content is rendered (typically `block.inner(terminal_area)`).
    pub content: Rect,
    /// Current scroll offset in lines.
    pub scroll: u16,
}

/// Describes how to render a single image, including any clipping parameters.
///
/// Returned by [`compute_image_render_rects`].  The caller should:
///
/// 1. Look up the decoded image from their cache.
/// 2. Build a [`Protocol`] via [`make_clipped_protocol`] if clipping is needed
///    (`hidden_top > 0` or `hidden_left > 0`), or use the standard cached
///    protocol otherwise.
/// 3. Render an [`Image`] widget at `render_rect`.
pub struct ImageRenderRect {
    /// The image URL (key into the caller's image and protocol caches).
    pub url: String,
    /// Where to render the visible portion (position + dimensions in terminal cells).
    pub render_rect: Rect,
    /// The original full cell dimensions (unclipped).
    pub full_cols: u16,
    pub full_rows: u16,
    /// How many cell-rows are hidden from the top of the original image.
    pub hidden_top: u16,
    /// How many cell-columns are hidden from the left of the original image.
    pub hidden_left: u16,
    /// Horizontal alignment (from the markdown source).
    pub alignment: Option<Alignment>,
}

/// Compute where to render each image given a viewport and scroll state.
///
/// Takes the output of [`prepare_inline_images`] and the current render state
/// (`lines` and viewport) and returns a list of [`ImageRenderRect`] values.
///
/// Images that are fully off‑screen (above, below, left, or right) are
/// excluded from the result.  Images that are partially visible get clipping
/// parameters (`hidden_top`, `hidden_left`) that the caller passes to
/// [`make_clipped_protocol`].
///
/// `lines` is the full line buffer after `prepare_inline_images` has replaced
/// image placeholders — its length is used to safely clamp line indices.
pub fn compute_image_render_rects(
    placements: &[ImagePlacement],
    lines: &[Line],
    viewport: &ImageViewport,
) -> Vec<ImageRenderRect> {
    let content_top = viewport.content.y as i32;
    let content_bottom = (viewport.content.y + viewport.content.height) as i32;
    let content_left = viewport.content.x as i32;
    let content_right = (viewport.content.x + viewport.content.width) as i32;

    let mut render_rects = Vec::new();

    for p in placements {
        // Compute visual Y (accounting for Paragraph word-wrap).
        let visual_y: u16 = if p.line_start == 0 {
            0
        } else {
            let end = p.line_start.min(lines.len());
            Paragraph::new(lines[..end].to_vec())
                .wrap(Wrap { trim: false })
                .line_count(viewport.content.width)
                .max(1) as u16
        };

        // Unclipped terminal-space Y coordinates.
        let unclipped_y0 = content_top + visual_y as i32 - viewport.scroll as i32;
        let unclipped_y1 = unclipped_y0 + p.cell_rows as i32;

        // Clamp Y to content area.
        let y0 = unclipped_y0.max(content_top);
        let y1 = unclipped_y1.min(content_bottom);
        if y0 >= y1 {
            continue;
        }
        let visible_rows = (y1 - y0) as u16;

        // Compute logical horizontal position (may be negative for overflow).
        let logical_x = match p.alignment {
            Some(Alignment::Center) => {
                content_left + (viewport.content.width as i32 / 2) - (p.cell_cols as i32 / 2)
            }
            Some(Alignment::Right) => {
                content_left + viewport.content.width as i32 - p.cell_cols as i32
            }
            _ => content_left,
        };

        // Clamp X to content area.
        let x0 = logical_x.max(content_left);
        let x1 = (logical_x + p.cell_cols as i32).min(content_right);
        if x0 >= x1 {
            continue;
        }
        let visible_cols = (x1 - x0) as u16;

        let hidden_top = if unclipped_y0 < content_top {
            (content_top - unclipped_y0) as u16
        } else {
            0
        };

        let hidden_left = if logical_x < content_left {
            (content_left - logical_x) as u16
        } else {
            0
        };

        render_rects.push(ImageRenderRect {
            url: p.url.clone(),
            render_rect: Rect {
                x: x0 as u16,
                y: y0 as u16,
                width: visible_cols,
                height: visible_rows,
            },
            full_cols: p.cell_cols,
            full_rows: p.cell_rows,
            hidden_top,
            hidden_left,
            alignment: p.alignment,
        });
    }

    render_rects
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

    // Pass 1 — lazily create fixed-size protocols for newly cached images.
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

        let alignment = lines[insert_at].alignment;
        let empty: Vec<Line<'static>> = (0..rows)
            .map(|_| {
                let mut l = Line::from("");
                l.alignment = alignment;
                l
            })
            .collect();
        lines.splice(insert_at..=insert_at, empty);

        placements.push(ImagePlacement {
            url: img.url.clone(),
            line_start: insert_at,
            cell_cols: cols,
            cell_rows: rows,
            alignment,
        });

        offset += rows as isize - 1;
        cursor = insert_at as isize + rows as isize;
    }

    placements
}
