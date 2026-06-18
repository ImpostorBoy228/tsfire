# tsfire

experimental browser engine in rust. fetches pages, parses html/css, builds render tree, computes layout.

**primary goal:** lower memory (ram) footprint than chrome/firefox

every design decision prioritizes this. no multi-process per tab, no gecko-level bloat, minimal copies.

## ram-critical design rules

1. **no pixel buffers per element** — display list stores commands (~32 bytes each), not rasterized buffers (240KB for 300×200px element)
2. **no shadow dom allocations** — no deep copies of style structs per node
3. **single process** — one global stylist, one lock, one arena. chrome pays 100+ MB per tab process
4. **zero-copy where possible** — css text → parser → property values, no intermediate string maps
5. **lazy decode** — images decoded on first paint, freed when not visible
6. **no precomputed layout cache** — layout is cheap, storing it for 50k nodes is not
7. **glyph atlas, not per-text allocations** — font textures shared, not duplicated

these decisions are non-negotiable. anything that adds per-element heap allocations is a design error.

## project structure

```
src/
├── main.rs      entry: fetch → dom → css → render tree → layout → display list
├── network.rs   user-agent builder
├── parse.rs     html parser (html5ever), css collection
├── render.rs    render tree builder
├── layout.rs    layoutengine trait, box tree with font-based text measurement
├── paint.rs     display list builder
├── style.rs     old cascade (preserved for reference)
├── stylo_integration.rs   mozilla stylo css engine integration
├── image_handler.rs   stb_image FFI (png/jpeg/webp → rgba)
├── font.rs            freetype FFI (ttf → glyph metrics + bitmap)
├── cache.rs           (reserved)
├── lib.rs             crate root, re-exports
└── cmod/
    ├── image_handler.c   stb_image wrapper
    ├── stb_image.h       single-file image decoder
    ├── font_handler.c    freetype wrapper (load, measure, fill_glyphs)
    └── font_handler.h    GlyphInfo struct
```

dependencies: tokio, reqwest, html5ever, markup5ever_rcdom, cssparser, selectors, precomputed-hash, euclid, app_units, string_cache, web_atoms, stylo_atoms, stylo, dom, style_traits, url

## current state

- html → dom → css collection → stylo css engine (bridge architecture)
- css selector matching via `selectors` crate (kept from old system)
- css property parsing + computation via mozilla's stylo engine
- bridge converts stylo's `ComputedValues` → custom `style::ComputedValues` (used by layout/paint)
- block layout → positioned `LayoutBox` tree with computed styles
- `LayoutEngine` trait for swappable layout backends
- paint system with display list rendering
- dump shows render tree (with style attrs) and layout tree (with coordinates + sizes)
- workflow: `cargo run` fetches wikipedia.org, prints both trees
- unused code in `style.rs` (old cascade) preserved for reference

## TODO

### done
- [x] deutf8() validation — continuation bytes, overlong, surrogates, >U+10FFFF
- [x] freetype2 optional (`feature = "freetype"`, default on). CI builds with `--no-default-features`

### plan: webrender integration
1. **window + gpu context** — add `winit` + `wgpu` (or `glutin`) to deps. create window, init wgpu surface/device.
2. **webrender** — add to deps. init `webrender::Renderer` with wgpu backend.
3. **display list bridge** — rewrite `paint::build_display_list` → webrender `Transaction`:
   - `FillRect` → `webrender::api::PushStackingContext` + `PushRect`
   - `DrawImage` → `AddImage` + `PushImage`
   - `TextRun` → `AddGlyphFontInstance` + `PushText` (webrender does glyph rasterization itself, no freetype needed)
   - `Border` → `PushBorder`
   - `SetClip/PopClip` → `SetClipRect` / `PopClip`
4. **text** — webrender has built-in glyph cache (glyph atlas). upload font data once via `AddNativeFont`.
5. **layout** — update `font_cache()` to load font and register with webrender.
6. **main loop** — `winit` event loop: fetch → parse → style → layout → build wr display list → render.
7. **stretch** — scrolling, resize, `<img>` elements via `DrawImage`.


## ai code rules

1. **confirm important decisions** — ask before wiring up major changes
2. **never change existing comments** — leave them as-is
3. **comment style** — minimalist english lowercase, only where clarity requires it. no decorative or redundant comments
4. **no new documentation files** unless explicitly asked (no readme, no docs/). exceptions: this file, agents.md
5. **minimal diffs** — change only what is needed for the task
6. **language** — instructions in russian are fine; code comments in english
