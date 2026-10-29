# Changelog

## 0.9.0 (unreleased)

### Added

- `render_image::ImageStore`: one type that holds an app's inline images and
  does everything after the download: layout (`prepare`), drawing and
  click areas (`render`), loading and failed state (`start` / `finish`; failed images stay
  failed until `retry_failed`, so `start` is safe to call every frame),
  re-fitting images when the width changes, and freeing memory (`retain` /
  `clear`).
- Opt-in placeholders while images load or fail: `with_spinner()`,
  `with_loading_line()`, `with_failed_line()`. Without them, images look
  exactly as before.
- `keep_placeholders()`: keep each image's placeholder line (alt text, link
  hint labels from `image_formatter`) above the image once it is drawn.
- `ImagePlacement::row`: the visual row an image starts on, after wrapping.

### Changed

- `compute_image_signed_positions` works out all positions in one pass over
  the lines instead of re-wrapping the text above every image; long documents
  with several images lay out much faster. Results are the same.

### Fixed

- `render_markdown_with_extra`: extra images used to have no line of their own
  and overwrote the first lines of text once loaded. Each now gets a placeholder
  line at the top, so they show while loading and never cover text.

### Breaking changes

Most apps only need to recompile. Check these if you use them:

- **Removed functions deprecated since 0.5:** `make_clipped_protocol`,
  `make_scrolled_protocol`, `compute_image_render_rects` and `ImageRenderRect`.
  Draw with `ImageStore::render`, or with `make_sliced_protocol` +
  `compute_image_signed_positions` + `SlicedImage` as recommended since 0.5.
- **`render_markdown_with_extra`** now adds one placeholder line per extra image
  at the top (see Fixed), so the other lines, images and links move down by that
  many lines. Indices in the result are already shifted; only code that assumed
  the first text line is line 0 is affected.
- **`ImagePlacement`** has a new `row` field: code that builds one by hand must
  set it (placements returned by limner already have it).

### Unchanged

- `prepare_inline_images()` and `compute_image_signed_positions()` keep their
  signatures and results. `ImageStore` is the recommended way from now on; the
  old functions will be deprecated in a later release, with the guide below as
  the migration path.

### Migrating to `ImageStore`

Optional, and it can be done in one go. Your two caches, your "downloading"
set and the picker move into the store; downloading and decoding stays in your
code.

| Before (0.8) | After (0.9) |
|---|---|
| `picker: Picker` | `store: ImageStore::new(picker)` |
| `image_cache: HashMap<String, DynamicImage>` | (inside the store) |
| `protocol_cache: HashMap<String, SlicedProtocol>` | (inside the store) |
| your own "downloading" set | (inside the store) |
| `if !cache.contains_key(url) && pending.insert(url) { download }` | `if store.start(url) { download }` |
| `image_cache.insert(url, image)` | `store.finish(url, Some(image))` |
| download/decode failed | `store.finish(url, None)` (stays failed; `store.retry_failed()` to try again) |
| `prepare_inline_images(&mut lines, &images, &mut image_cache, &mut protocol_cache, &picker, &picker.font_size(), width, 10)` | `store.prepare(&mut lines, &images, width, 10)` |
| `compute_image_signed_positions` + `protocol_cache.get` + `SlicedImage` loop | `store.render(frame.buffer_mut(), area, &placements, scroll)` |
| `protocol_cache.get(url)` | `store.get(url)` |
| `&picker` elsewhere | `store.picker()` |

Before:

```rust,ignore
for img in &result.images {
    if !image_cache.contains_key(&img.url) && pending.insert(img.url.clone()) {
        spawn_download(img.url.clone()); // later: pending.remove(&url); image_cache.insert(url, image)
    }
}
let placements = prepare_inline_images(
    &mut lines, &result.images, &mut image_cache, &mut protocol_cache,
    &picker, &picker.font_size(), area.width, 10,
);
// inside terminal.draw:
let viewport = ImageViewport { content: area, scroll };
for r in compute_image_signed_positions(&placements, &lines, &viewport) {
    if let Some(sliced) = protocol_cache.get(&r.url) {
        frame.render_widget(SlicedImage::new(sliced, r.position), area);
    }
}
```

After:

```rust,ignore
for img in &result.images {
    if store.start(&img.url) {
        spawn_download(img.url.clone()); // later: store.finish(&url, decoded)
    }
}
let placements = store.prepare(&mut lines, &result.images, area.width, 10);
// inside terminal.draw:
let hits = store.render(frame.buffer_mut(), area, &placements, scroll);
```

Notes:

- `prepare()` uses the picker's own font size. If you passed a different
  `FontSize` to `prepare_inline_images`, keep using that function for now.
- `width` is the width the lines are drawn at; images now re-fit when it
  changes (before, they kept the size of their first layout).
- If you cache layouts, add `store.version()` and the width to the cache key:
  the version changes whenever `prepare()` would return something different
  (an image loaded or failed, or a spinner frame while images are loading).
- To animate `with_spinner()`, call `store.tick()` on a timer (about 10 times a
  second) and redraw.
- The store keeps loaded images until you call `retain` or `clear`, e.g.
  `store.retain(|url| on_screen.contains(url))` when the user leaves a page.
