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
├── main.rs               entry point, cli arg
├── network.rs            http / user-agent
├── cache.rs              glyph metrics cache (eliminates per-frame freetype)
├── parsing/
│   ├── mod.rs            re-exports
│   ├── parse.rs          html + css parsing
│   ├── style.rs          ComputedValues, all CSS enums
│   ├── stylo_integration.rs  stylo bridge (property extraction)
│   └── tree.rs           render tree builder
├── ui_shit/
│   ├── mod.rs            module root
│   ├── layout.rs         block/inline/positioned/float layout
│   ├── paint.rs          DisplayList builder
│   ├── display_renderer.rs  wgpu renderer (solid + textured pipelines)
│   ├── cpu_renderer.rs   minifb cpu fallback
│   ├── window.rs         winit + wgpu window loop
│   └── shaders/
│       └── pipeline.wgsl WGSL vertex + fragment shaders
├── image_handler.rs      stb_image FFI
├── font.rs               freetype FFI
├── lib.rs                crate root
└── cmod/
    ├── image_handler.c   stb_image wrapper
    ├── stb_image.h       image decoder
    ├── font_handler.c    freetype wrapper (load, measure, fill_glyphs)
    └── font_handler.h    GlyphInfo struct
```

dependencies: tokio, reqwest, html5ever, markup5ever_rcdom, cssparser, selectors, precomputed-hash, euclid, app_units, string_cache, web_atoms, stylo_atoms, stylo, dom, style_traits, url, winit, wgpu, freetype2 (optional), stb_image (bundled)

## current state

- http fetch + html5ever dom → css collection → stylo css engine (all major properties bridged)
- external `<link rel="stylesheet">` fetched and parsed
- cli argument: `cargo run -- https://example.com`
- block/inline/inline-block layout with absolute/fixed/relative positioning, float left/right, overflow clip, auto margins
- percentage lengths (`Length::Percent`)
- baseline alignment + line wrapping for mixed text/inline-block content
- flat DisplayList: FillRect, FillGradient, DrawImage, TextRun, Border, DrawBoxShadow, SetClip/PopClip, SetOpacity/PopOpacity
- wgpu renderer with solid + gradient + textured pipelines (clip rect, opacity alpha, border 4-rect, box-shadow multi-rect blur)
- cpu renderer fallback (minifb) — gradient lerp + DrawImage blit (clip/opacity still missing)
- image decoding via stb_image (png/jpeg/webp → rgba)
- font decoding via freetype2 (ttf → glyph metrics + grayscale bitmap)
- glyph atlas for wgpu text rendering (Nearest filter + pixel snapping → sharp text)
- glyph metrics cache (freetype not called after 1st frame)
- dynamic vertex buffers (no `create_buffer_init` per frame)
- frame rate limiter (60 FPS via `ControlFlow::WaitUntil`)
- viewport resize → re-layout + rebuild DisplayList
- `<img>` tag fetch + decode + DrawImage pipeline
- word-level line wrapping with whitespace handling
- margin collapse between block siblings (vertical max reduction)
- 35 tests, all passing (run with `RUST_TEST_THREADS=1` to avoid race on exit)

## TODO

- **background-repeat/position** — tiling and offset for background images
- **incremental re-style** — handle viewport-relative CSS units on resize
- **relative z-index** — proper stacking context ordering
- **cpu_renderer clip/opacity** — currently unimplemented

## ai code rules

1. **confirm important decisions** — ask before wiring up major changes
2. **never change existing comments** — leave them as-is
3. **comment style** — minimalist english lowercase, only where clarity requires it. no decorative or redundant comments
4. **no new documentation files** unless explicitly asked (no readme, no docs/). exceptions: this file, agents.md
5. **minimal diffs** — change only what is needed for the task
6. **language** — instructions in russian are fine; code comments in english
7. **never commit automatically** — i will tell you when to commit/push. do not commit on your own.
