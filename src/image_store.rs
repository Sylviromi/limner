//! [`ImageStore`]: everything an app needs to show inline markdown images —
//! decoded images, terminal protocols, loading and failed state, layout and
//! drawing — behind one type.

use std::collections::HashMap;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::Widget;

use crate::render_image::{
    fit_cell_size, image_position, img_crate::DynamicImage, make_sliced_protocol, place_images,
    ImagePlacement, Picker, SlicedImage, SlicedProtocol,
};
use crate::ImageInfo;

/// Braille spinner used by [`ImageStore::with_spinner`].
const SPINNER: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

type LineHook = dyn Fn(&PendingImage<'_>) -> Line<'static> + Send + Sync;

/// An image that isn't drawn yet, as handed to the hooks of
/// [`ImageStore::with_loading_line`] and [`ImageStore::with_failed_line`].
#[derive(Debug, Clone, Copy)]
pub struct PendingImage<'a> {
    /// The image as found in the markdown.
    pub info: &'a ImageInfo,
    /// The placeholder line limner rendered for it (including any
    /// `image_formatter` output), to reuse or extend.
    pub placeholder: &'a Line<'static>,
    /// Animation frame: how many times [`ImageStore::tick`] was called.
    pub frame: usize,
}

/// The on-screen part of an image drawn by [`ImageStore::render`], e.g. to
/// make it clickable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageHit {
    pub url: String,
    pub area: Rect,
}

enum Entry {
    Loading,
    Failed,
    Ready {
        source: DynamicImage,
        fitted: Option<Fitted>,
    },
}

/// A protocol encoded for one layout size; re-encoded when the size changes.
struct Fitted {
    protocol: SlicedProtocol,
    width: u16,
    max_rows: u16,
}

/// Holds the images of rendered markdown, lays them out and draws them.
///
/// The app downloads and decodes images itself and reports to the store:
///
/// ```rust,ignore
/// let mut store = ImageStore::new(picker).with_spinner();
///
/// // When an article is shown:
/// for img in &result.images {
///     if store.start(&img.url) {
///         spawn_download(img.url.clone()); // ends with `store.finish(&url, decoded)`
///     }
/// }
///
/// // Every frame, on a fresh copy of the rendered lines:
/// let mut lines = result.lines.clone();
/// let placements = store.prepare(&mut lines, &result.images, area.width, 10);
/// frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }).scroll((scroll, 0)), area);
/// let hits = store.render(frame.buffer_mut(), area, &placements, scroll);
///
/// // On a timer, to animate the spinner:
/// store.tick();
/// ```
///
/// Without [`with_spinner`](Self::with_spinner) / the line hooks, images that
/// are loading or failed keep the placeholder `render_markdown` produced.
pub struct ImageStore {
    picker: Picker,
    entries: HashMap<String, Entry>,
    loading_line: Option<Box<LineHook>>,
    failed_line: Option<Box<LineHook>>,
    keep_placeholders: bool,
    frame: usize,
    version: u64,
}

impl ImageStore {
    /// An empty store drawing with `picker` (see
    /// [`halfblock_picker`](crate::render_image::halfblock_picker)).
    pub fn new(picker: Picker) -> Self {
        Self {
            picker,
            entries: HashMap::new(),
            loading_line: None,
            failed_line: None,
            keep_placeholders: false,
            frame: 0,
            version: 0,
        }
    }

    /// Show a spinner in front of the placeholder of images that are loading.
    /// Call [`tick`](Self::tick) to animate it.
    pub fn with_spinner(self) -> Self {
        self.with_loading_line(|p| {
            let style = p
                .placeholder
                .spans
                .first()
                .map(|s| s.style)
                .unwrap_or_default();
            let frame = SPINNER[p.frame % SPINNER.len()];
            let mut spans = vec![Span::styled(format!("{frame} "), style)];
            spans.extend(p.placeholder.spans.iter().cloned());
            Line::from(spans)
        })
    }

    /// Draw this line instead of the placeholder while an image is loading.
    pub fn with_loading_line(
        mut self,
        f: impl Fn(&PendingImage<'_>) -> Line<'static> + Send + Sync + 'static,
    ) -> Self {
        self.loading_line = Some(Box::new(f));
        self
    }

    /// Draw this line instead of the placeholder when an image failed to load.
    pub fn with_failed_line(
        mut self,
        f: impl Fn(&PendingImage<'_>) -> Line<'static> + Send + Sync + 'static,
    ) -> Self {
        self.failed_line = Some(Box::new(f));
        self
    }

    /// Keep each image's placeholder line (alt text, `image_formatter` labels
    /// such as link hints) above the image once it is drawn, instead of the
    /// image taking its place.
    pub fn keep_placeholders(mut self) -> Self {
        self.keep_placeholders = true;
        self
    }

    /// The picker the store draws with (e.g. to build other protocols).
    pub fn picker(&self) -> &Picker {
        &self.picker
    }

    /// Mark `url` as loading. Returns `true` when the caller should download it:
    /// it is neither loaded nor already loading (failed images are retried).
    pub fn start(&mut self, url: &str) -> bool {
        if matches!(
            self.entries.get(url),
            Some(Entry::Loading | Entry::Ready { .. })
        ) {
            return false;
        }
        self.entries.insert(url.to_string(), Entry::Loading);
        self.changed_if(self.has_hooks());
        true
    }

    /// Report the result of a download: the decoded image, or `None` if it failed.
    pub fn finish(&mut self, url: &str, image: Option<DynamicImage>) {
        let entry = match image {
            Some(source) => Entry::Ready {
                source,
                fitted: None,
            },
            None => Entry::Failed,
        };
        let ready = matches!(entry, Entry::Ready { .. });
        self.entries.insert(url.to_string(), entry);
        self.changed_if(ready || self.has_hooks());
    }

    /// Advance the loading animation by one frame.
    pub fn tick(&mut self) {
        self.frame = self.frame.wrapping_add(1);
        let loading = self.entries.values().any(|e| matches!(e, Entry::Loading));
        self.changed_if(self.loading_line.is_some() && loading);
    }

    /// Forget the images whose URL `keep` rejects, freeing their memory (e.g.
    /// keep only the article on screen). They are downloaded again if needed.
    pub fn retain(&mut self, mut keep: impl FnMut(&str) -> bool) {
        let before = self.entries.len();
        self.entries.retain(|url, _| keep(url));
        self.changed_if(self.entries.len() != before);
    }

    /// Forget every image.
    pub fn clear(&mut self) {
        self.retain(|_| false);
    }

    /// Changes whenever [`prepare`](Self::prepare) would give a different
    /// result for the same input; use it, with the width, in the key of a
    /// layout cache.
    pub fn version(&self) -> u64 {
        self.version
    }

    /// The protocol to draw `url` with, once [`prepare`](Self::prepare) placed it.
    pub fn get(&self, url: &str) -> Option<&SlicedProtocol> {
        match self.entries.get(url) {
            Some(Entry::Ready {
                fitted: Some(f), ..
            }) => Some(&f.protocol),
            _ => None,
        }
    }

    /// Lay out `images` (from the same render as `lines`): each loaded image
    /// gets `cell_rows` empty lines to be drawn over, described by the
    /// returned placements. Loading and failed images get their hook's line,
    /// or keep the placeholder.
    ///
    /// `width` is the width the lines will be drawn at; images fit within
    /// `width × max_rows` cells and are re-fitted when either changes.
    pub fn prepare(
        &mut self,
        lines: &mut Vec<Line<'static>>,
        images: &[ImageInfo],
        width: u16,
        max_rows: u16,
    ) -> Vec<ImagePlacement> {
        for img in images {
            self.fit(&img.url, width, max_rows);
        }

        let this = &*self;
        place_images(
            lines,
            images,
            |url| this.get(url).map(SlicedProtocol::size),
            |info, placeholder| {
                let hook = match this.entries.get(&info.url) {
                    Some(Entry::Loading) => &this.loading_line,
                    Some(Entry::Failed) => &this.failed_line,
                    _ => &None,
                };
                hook.as_ref().map(|f| {
                    f(&PendingImage {
                        info,
                        placeholder,
                        frame: this.frame,
                    })
                })
            },
            self.keep_placeholders,
            width,
        )
    }

    /// Draw the visible images of `placements` into `area`, the area the
    /// lines from [`prepare`](Self::prepare) are drawn in, scrolled down by
    /// `scroll` rows. Returns the on-screen part of each image drawn.
    pub fn render(
        &self,
        buf: &mut Buffer,
        area: Rect,
        placements: &[ImagePlacement],
        scroll: u16,
    ) -> Vec<ImageHit> {
        let mut hits = Vec::new();
        for p in placements {
            let (Some(position), Some(protocol)) =
                (image_position(p, p.row, area, scroll), self.get(&p.url))
            else {
                continue;
            };
            SlicedImage::new(protocol, position).render(area, buf);

            let x = i32::from(area.x) + i32::from(position.x);
            let y = i32::from(area.y) + i32::from(position.y);
            let x0 = x.max(i32::from(area.x));
            let y0 = y.max(i32::from(area.y));
            let x1 = (x + i32::from(p.cell_cols)).min(i32::from(area.right()));
            let y1 = (y + i32::from(p.cell_rows)).min(i32::from(area.bottom()));
            if x0 < x1 && y0 < y1 {
                hits.push(ImageHit {
                    url: p.url.clone(),
                    area: Rect::new(x0 as u16, y0 as u16, (x1 - x0) as u16, (y1 - y0) as u16),
                });
            }
        }
        hits
    }

    /// Encode `url` for `width × max_rows` if it is loaded and not encoded at
    /// that size yet.
    fn fit(&mut self, url: &str, width: u16, max_rows: u16) {
        let Some(Entry::Ready { source, fitted }) = self.entries.get_mut(url) else {
            return;
        };
        if fitted
            .as_ref()
            .is_some_and(|f| (f.width, f.max_rows) == (width, max_rows))
        {
            return;
        }
        let font = self.picker.font_size();
        // On first fit only, shrink the original to 2× its fitted size: later
        // re-fits (other widths) start from this copy.
        // shortcut: a terminal more than 2× wider than at first sight shows the
        // image a bit soft; keep the original if that ever matters.
        if fitted.is_none() {
            let max_w = u32::from(width) * u32::from(font.width) * 2;
            let max_h = u32::from(max_rows) * u32::from(font.height) * 2;
            if source.width() > max_w || source.height() > max_h {
                *source = source.resize(max_w, max_h, image::imageops::FilterType::Triangle);
            }
        }
        let (cols, rows) = fit_cell_size(source, &font, width, max_rows);
        match make_sliced_protocol(&self.picker, source.clone(), cols, rows) {
            Some(protocol) => {
                *fitted = Some(Fitted {
                    protocol,
                    width,
                    max_rows,
                })
            }
            None => {
                self.entries.insert(url.to_string(), Entry::Failed);
            }
        }
    }

    fn has_hooks(&self) -> bool {
        self.loading_line.is_some() || self.failed_line.is_some()
    }

    fn changed_if(&mut self, changed: bool) {
        if changed {
            self.version = self.version.wrapping_add(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render_image::halfblock_picker;
    use crate::{render_markdown, MarkdownStyle};

    const DOC: &str = "before\n\n![cat](a.png)\n\n![dog](b.png)\n\nafter";

    fn prepare_at(store: &mut ImageStore, width: u16) -> (Vec<String>, Vec<ImagePlacement>) {
        let result = render_markdown(DOC, &MarkdownStyle::default(), width);
        let mut lines = result.lines;
        let placements = store.prepare(&mut lines, &result.images, width, 10);
        (lines.iter().map(|l| l.to_string()).collect(), placements)
    }

    fn prepare(store: &mut ImageStore) -> (Vec<String>, Vec<ImagePlacement>) {
        prepare_at(store, 40)
    }

    fn image() -> DynamicImage {
        DynamicImage::new_rgb8(400, 400)
    }

    fn store() -> ImageStore {
        ImageStore::new(halfblock_picker())
    }

    #[test]
    fn placeholders_stay_without_hooks() {
        let mut s = store();
        assert!(s.start("a.png"));
        s.finish("b.png", None);
        let (plain, _) = prepare(&mut store());
        let (lines, placements) = prepare(&mut s);
        assert_eq!(lines, plain);
        assert!(placements.is_empty());
    }

    #[test]
    fn spinner_animates_loading_images_only() {
        let mut s = store().with_spinner();
        s.start("a.png");
        let (lines, _) = prepare(&mut s);
        assert!(lines
            .iter()
            .any(|l| l.starts_with("⠋ ") && l.contains("cat")));
        assert!(lines
            .iter()
            .any(|l| l.contains("dog") && !l.starts_with('⠋')));
        let version = s.version();
        s.tick();
        assert_ne!(s.version(), version);
        let (lines, _) = prepare(&mut s);
        assert!(lines
            .iter()
            .any(|l| l.starts_with("⠙ ") && l.contains("cat")));
    }

    #[test]
    fn failed_hook_and_retry() {
        let mut s = store().with_failed_line(|p| Line::from(format!("⚠ {}", p.info.alt)));
        assert!(s.start("a.png"));
        assert!(!s.start("a.png"), "already loading");
        s.finish("a.png", None);
        let (lines, _) = prepare(&mut s);
        assert!(lines.iter().any(|l| l == "⚠ cat"));
        assert!(s.start("a.png"), "failed images are retried");
    }

    #[test]
    fn loaded_image_replaces_its_placeholder() {
        let mut s = store().with_spinner();
        s.start("a.png");
        let version = s.version();
        s.finish("a.png", Some(image()));
        assert_ne!(s.version(), version);
        let (lines, placements) = prepare(&mut s);
        assert_eq!(placements.len(), 1);
        let p = &placements[0];
        assert_eq!((p.line_start, p.row), (2, 2));
        assert!(lines[p.line_start..p.line_start + p.cell_rows as usize]
            .iter()
            .all(String::is_empty));
        assert!(!lines.iter().any(|l| l.contains("cat")));
        assert!(!s.start("a.png"));
        // Ticks with nothing loading don't invalidate layout caches.
        let version = s.version();
        s.tick();
        assert_eq!(s.version(), version);
    }

    #[test]
    fn kept_placeholder_sits_above_the_image() {
        let mut s = store().keep_placeholders();
        s.finish("a.png", Some(image()));
        s.finish("b.png", Some(image()));
        let (lines, placements) = prepare(&mut s);
        let [cat, dog] = &placements[..] else {
            panic!("two placements");
        };
        assert!(lines[cat.line_start - 1].contains("cat"));
        assert!(lines[dog.line_start - 1].contains("dog"));
        assert!(dog.line_start > cat.line_start + cat.cell_rows as usize);
        assert_eq!(lines.last().map(String::as_str), Some("after"));
    }

    #[test]
    fn images_refit_when_the_width_changes() {
        let mut s = store();
        s.finish("a.png", Some(image()));
        let (_, wide) = prepare_at(&mut s, 40);
        let (_, narrow) = prepare_at(&mut s, 5);
        assert!(narrow[0].cell_cols <= 5 && narrow[0].cell_cols < wide[0].cell_cols);
        let (_, wide_again) = prepare_at(&mut s, 40);
        assert_eq!(wide_again[0].cell_cols, wide[0].cell_cols);
    }

    #[test]
    fn render_draws_visible_images_and_reports_where() {
        let mut s = store();
        s.finish("a.png", Some(image()));
        let (_, placements) = prepare(&mut s);
        let p = &placements[0];
        let area = Rect::new(2, 1, 40, 20);
        let mut buf = Buffer::empty(Rect::new(0, 0, 50, 24));

        let hits = s.render(&mut buf, area, &placements, 0);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].url, "a.png");
        assert_eq!((hits[0].area.x, hits[0].area.y), (2, 1 + p.row as u16));
        assert!(area.contains(hits[0].area.as_position()));

        // Scrolled past it: clipped at the top, then gone.
        let hits = s.render(&mut buf, area, &placements, p.row as u16 + 1);
        assert_eq!(hits[0].area.y, 1);
        assert_eq!(hits[0].area.height, p.cell_rows - 1);
        let hits = s.render(&mut buf, area, &placements, 100);
        assert!(hits.is_empty());
    }

    #[test]
    fn retain_forgets_images() {
        let mut s = store();
        s.finish("a.png", Some(image()));
        s.finish("b.png", Some(image()));
        prepare(&mut s);
        let version = s.version();
        s.retain(|url| url == "b.png");
        assert_ne!(s.version(), version);
        assert!(s.get("a.png").is_none() && s.get("b.png").is_some());
        assert!(s.start("a.png"), "forgotten images can be loaded again");
        s.clear();
        assert!(s.get("b.png").is_none());
    }
}
