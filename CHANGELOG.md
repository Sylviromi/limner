# Changelog

## 0.9.0 (unreleased)

### Added

- `render_image::ImageStore`: one type that holds an app's inline images
  (decoded images, terminal protocols, loading and failed state) and lays them
  out. Opt-in loading placeholders: `with_spinner()`, `with_loading_line()`,
  `with_failed_line()`. Without them, placeholders look exactly as before.

### Unchanged

- `prepare_inline_images()` and the other image functions work as before.
  `ImageStore` is the recommended way from now on; the old functions will be
  deprecated in a later release, with this guide as the migration path.

### Migrating to `ImageStore`

Optional, and it can be done in one go. Your two caches and the picker move into
the store; downloading and decoding stays in your code.

| Before (0.8) | After (0.9) |
|---|---|
| `picker: Picker` | `store: ImageStore::new(picker)` |
| `image_cache: HashMap<String, DynamicImage>` | (inside the store) |
| `protocol_cache: HashMap<String, SlicedProtocol>` | (inside the store) |
| your own "downloading" set | (inside the store) |
| `if !cache.contains_key(url) && pending.insert(url) { download }` | `if store.start(url) { download }` |
| `image_cache.insert(url, image)` | `store.finish(url, Some(image))` |
| download/decode failed | `store.finish(url, None)` (retried on the next `start`) |
| `prepare_inline_images(&mut lines, &images, &mut image_cache, &mut protocol_cache, &picker, &picker.font_size(), width, 10)` | `store.prepare(&mut lines, &images, width, 10)` |
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
    &picker, &picker.font_size(), width, 10,
);
let sliced = protocol_cache.get(&placement.url);
```

After:

```rust,ignore
for img in &result.images {
    if store.start(&img.url) {
        spawn_download(img.url.clone()); // later: store.finish(&url, decoded)
    }
}
let placements = store.prepare(&mut lines, &result.images, width, 10);
let sliced = store.get(&placement.url);
```

Notes:

- `prepare()` uses the picker's own font size. If you passed a different
  `FontSize` to `prepare_inline_images`, keep using that function for now.
- If you cache layouts, add `store.version()` to the cache key: it changes
  whenever `prepare()` would return something different (an image loaded or
  failed, or a spinner frame while images are loading).
- To animate `with_spinner()`, call `store.tick()` on a timer (about 10 times a
  second) and redraw.
