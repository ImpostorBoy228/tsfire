# tsfire

experimental low-ram browser engine in rust.

fetches pages → parses html/css (via mozilla stylo) → builds render tree →
computes layout → produces flat display list → renders via wgpu (or cpu fallback).

**primary goal:** lower memory footprint than chrome/firefox.
every design decision prioritizes this. no multi-process, no per-element pixel buffers, no shadow dom copies.

## current state

- http fetch + html5ever dom → css collection → stylo css engine (all major properties)
- external `<link rel="stylesheet">` fetched and parsed
- cli argument: `cargo run -- https://example.com`
- block layout with absolute/fixed positioning, float left/right, overflow clip
- auto margins, relative positioning offset
- percentage lengths (`Length::Percent`)
- all display types mapped from stylo (block, inline, inline-block, flex, grid, table, etc.)
- flat DisplayList: FillRect, FillGradient, DrawImage, TextRun, Border, DrawBoxShadow, SetClip/PopClip, SetOpacity/PopOpacity
- wgpu renderer with solid + textured pipelines (clip rect, opacity alpha, border 4-rect, box-shadow)
- cpu renderer fallback (minifb)
- image decoding via stb_image (png/jpeg/webp → rgba)
- font decoding via freetype2 (ttf → glyph metrics + grayscale bitmap)
- glyph atlas for wgpu text rendering
- 30 tests, all passing

## architecture

```
fetch → parse(html) → collect(css) + external stylesheets → stylo(compute)
→ render tree → layout → display list → [wgpu | cpu] → window
```

```
src/
├── main.rs               entry point, cli arg
├── network.rs            http / user-agent
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
    ├── font_handler.c    freetype wrapper
    └── font_handler.h    GlyphInfo struct
```

## design rules

1. no pixel buffers per element — flat display list (~32-48 bytes/command)
2. no shadow dom allocations — no deep style copies per node
3. single process — one stylist, one arena
4. zero-copy where possible — css text → parser → values, no string maps
5. lazy image decode — decoded on first paint, freed when not visible
6. no precomputed layout cache — layout is cheap
7. glyph atlas, not per-text allocations — shared font textures

## building

```sh
cargo build
cargo test -- --test-threads=1
cargo run -- https://example.com
```

freetype2 is auto-detected. if missing, font measurement falls back to `chars*0.6` estimate.

## dependencies

tokio, reqwest, html5ever, markup5ever_rcdom, cssparser, selectors,
euclid, app_units, string_cache, stylo (servo), winit, wgpu, freetype2 (optional), stb_image (bundled)
