//! [`ImageStore`]: everything an app needs to show inline markdown images —
//! decoded images, terminal protocols, and which images are still loading or
//! failed — behind one type.

use std::collections::{HashMap, HashSet};

use ratatui::text::{Line, Span};

use crate::render_image::{
    fit_cell_size, img_crate::DynamicImage, make_sliced_protocol, place_images, ImagePlacement,
    Picker, SlicedProtocol,
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

/// Holds the images of rendered markdown and lays them out.
///
/// The app downloads and decodes images itself and reports to the store:
///
/// ```rust,ignore
/// let mut store = ImageStore::new(picker).with_spinner();
///
/// // When an article is shown:
/// for img in &result.images {
///     if store.start(&img.url) {
///         spawn_download(img.url.clone()); // ends with `store.finish(url, decoded)`
///     }
/// }
///
/// // Every frame, on a fresh copy of the rendered lines:
/// let mut lines = result.lines.clone();
/// let placements = store.prepare(&mut lines, &result.images, width, 10);
/// // draw `lines`, then `SlicedImage::new(store.get(&p.url)?, ..)` per placement
///
/// // On a timer, to animate the spinner:
/// store.tick();
/// ```
///
/// Without [`with_spinner`](Self::with_spinner) / the line hooks, images that
/// are loading or failed keep the placeholder `render_markdown` produced.
pub struct ImageStore {
    picker: Picker,
    decoded: HashMap<String, DynamicImage>,
    protocols: HashMap<String, SlicedProtocol>,
    loading: HashSet<String>,
    failed: HashSet<String>,
    loading_line: Option<Box<LineHook>>,
    failed_line: Option<Box<LineHook>>,
    frame: usize,
    version: u64,
}

impl ImageStore {
    /// An empty store drawing with `picker` (see
    /// [`halfblock_picker`](crate::render_image::halfblock_picker)).
    pub fn new(picker: Picker) -> Self {
        Self {
            picker,
            decoded: HashMap::new(),
            protocols: HashMap::new(),
            loading: HashSet::new(),
            failed: HashSet::new(),
            loading_line: None,
            failed_line: None,
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
            let mut spans = vec![Span::styled(
                format!("{} ", SPINNER[p.frame % SPINNER.len()]),
                style,
            )];
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

    /// The picker the store draws with (e.g. to build other protocols).
    pub fn picker(&self) -> &Picker {
        &self.picker
    }

    /// Mark `url` as loading. Returns `true` when the caller should download it:
    /// it is neither loaded nor already loading (failed images are retried).
    pub fn start(&mut self, url: &str) -> bool {
        if self.loading.contains(url)
            || self.decoded.contains_key(url)
            || self.protocols.contains_key(url)
        {
            return false;
        }
        self.failed.remove(url);
        self.loading.insert(url.to_string());
        self.changed_if(self.loading_line.is_some() || self.failed_line.is_some());
        true
    }

    /// Report the result of a download: the decoded image, or `None` if it failed.
    pub fn finish(&mut self, url: &str, image: Option<DynamicImage>) {
        self.loading.remove(url);
        match image {
            Some(image) => {
                self.decoded.insert(url.to_string(), image);
                self.changed_if(true);
            }
            None => {
                self.failed.insert(url.to_string());
                self.changed_if(self.loading_line.is_some() || self.failed_line.is_some());
            }
        }
    }

    /// Advance the loading animation by one frame.
    pub fn tick(&mut self) {
        self.frame = self.frame.wrapping_add(1);
        self.changed_if(self.loading_line.is_some() && !self.loading.is_empty());
    }

    /// Changes whenever [`prepare`](Self::prepare) would give a different
    /// result for the same input; use it in the key of a layout cache.
    pub fn version(&self) -> u64 {
        self.version
    }

    /// The protocol to draw `url` with, once [`prepare`](Self::prepare) placed it.
    pub fn get(&self, url: &str) -> Option<&SlicedProtocol> {
        self.protocols.get(url)
    }

    /// Lay out `images` (from the same render as `lines`): each loaded image's
    /// placeholder line becomes `cell_rows` empty lines to draw the image over,
    /// described by the returned placements. Loading and failed images get
    /// their hook's line, or keep the placeholder.
    ///
    /// Images are sized on first layout to fit `max_cols × max_rows` cells.
    pub fn prepare(
        &mut self,
        lines: &mut Vec<Line<'static>>,
        images: &[ImageInfo],
        max_cols: u16,
        max_rows: u16,
    ) -> Vec<ImagePlacement> {
        let font_size = self.picker.font_size();
        for img in images {
            if self.protocols.contains_key(&img.url) {
                continue;
            }
            let Some(decoded) = self.decoded.remove(&img.url) else {
                continue;
            };
            let (cols, rows) = fit_cell_size(&decoded, &font_size, max_cols, max_rows);
            match make_sliced_protocol(&self.picker, decoded, cols, rows) {
                Some(sliced) => {
                    self.protocols.insert(img.url.clone(), sliced);
                }
                None => {
                    self.failed.insert(img.url.clone());
                }
            }
        }

        let Self {
            protocols,
            loading,
            failed,
            loading_line,
            failed_line,
            frame,
            ..
        } = &*self;
        place_images(lines, images, protocols, |info, placeholder| {
            let hook = if loading.contains(&info.url) {
                loading_line
            } else if failed.contains(&info.url) {
                failed_line
            } else {
                &None
            };
            hook.as_ref().map(|f| {
                f(&PendingImage {
                    info,
                    placeholder,
                    frame: *frame,
                })
            })
        })
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

    fn prepare(store: &mut ImageStore) -> (Vec<String>, Vec<ImagePlacement>) {
        let result = render_markdown(DOC, &MarkdownStyle::default(), 40);
        let mut lines = result.lines;
        let placements = store.prepare(&mut lines, &result.images, 40, 10);
        (lines.iter().map(|l| l.to_string()).collect(), placements)
    }

    fn image() -> DynamicImage {
        DynamicImage::new_rgb8(40, 40)
    }

    #[test]
    fn placeholders_stay_without_hooks() {
        let mut store = ImageStore::new(halfblock_picker());
        assert!(store.start("a.png"));
        store.finish("b.png", None);
        let (plain, _) = prepare(&mut ImageStore::new(halfblock_picker()));
        let (lines, placements) = prepare(&mut store);
        assert_eq!(lines, plain);
        assert!(placements.is_empty());
    }

    #[test]
    fn spinner_animates_loading_images_only() {
        let mut store = ImageStore::new(halfblock_picker()).with_spinner();
        store.start("a.png");
        let (lines, _) = prepare(&mut store);
        assert!(lines
            .iter()
            .any(|l| l.starts_with("⠋ ") && l.contains("cat")));
        assert!(lines
            .iter()
            .any(|l| l.contains("dog") && !l.starts_with('⠋')));
        let version = store.version();
        store.tick();
        assert_ne!(store.version(), version);
        let (lines, _) = prepare(&mut store);
        assert!(lines
            .iter()
            .any(|l| l.starts_with("⠙ ") && l.contains("cat")));
    }

    #[test]
    fn failed_hook_and_retry() {
        let mut store = ImageStore::new(halfblock_picker())
            .with_failed_line(|p| Line::from(format!("⚠ {}", p.info.alt)));
        assert!(store.start("a.png"));
        assert!(!store.start("a.png"), "already loading");
        store.finish("a.png", None);
        let (lines, _) = prepare(&mut store);
        assert!(lines.iter().any(|l| l == "⚠ cat"));
        assert!(store.start("a.png"), "failed images are retried");
    }

    #[test]
    fn loaded_image_is_placed_and_not_downloaded_again() {
        let mut store = ImageStore::new(halfblock_picker()).with_spinner();
        store.start("a.png");
        let version = store.version();
        store.finish("a.png", Some(image()));
        assert_ne!(store.version(), version);
        let (lines, placements) = prepare(&mut store);
        assert_eq!(placements.len(), 1);
        assert!(store.get("a.png").is_some());
        assert!(!lines.iter().any(|l| l.contains("cat")));
        assert!(!store.start("a.png"));
        // Ticks with nothing loading don't invalidate layout caches.
        let version = store.version();
        store.tick();
        assert_eq!(store.version(), version);
    }
}
