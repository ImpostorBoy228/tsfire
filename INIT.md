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
├── main.rs      entry: fetch → dom → css → render tree → layout → display list → window
├── network.rs   user-agent builder
├── parse.rs     html parser (html5ever), css collection
├── render.rs    render tree builder
├── layout.rs    layoutengine trait, block/inline layout with font-based text measurement
├── paint.rs     display list builder
├── style.rs     old cascade (preserved for reference)
├── stylo_integration.rs   mozilla stylo css engine integration
├── image_handler.rs   stb_image FFI (png/jpeg/webp → rgba)
├── font.rs            freetype FFI (ttf → glyph metrics + bitmap)
├── cache.rs           (reserved)
├── lib.rs             crate root, re-exports
├── window.rs          winit window + wgpu surface/device
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

### main goal: complete display list generation for full html+css (with stylo)

1. **stylo bridge — support all computed css properties** — `stylo_integration.rs`:
   - extract border-* (width, style, color per side) from stylo `ComputedValues`
   - extract background-image (gradients, urls), background-repeat, background-position
   - extract opacity, overflow, text-decoration, box-shadow
   - extract float, clear, vertical-align
   - ensure every property used by layout/paint is bridged

2. **layout — full coordinate generation for all css modes** — `layout.rs`:
   - positioned elements (absolute, fixed, relative) with proper containing block
   - floats (left/right) with clear
   - inline-block, inline formatting with baseline alignment
   - overflow → clip rects, scroll offsets
   - proper box model: content-box vs border-box sizing
   - margin collapse (adjacent siblings, parent-child)

3. **paint — generate all display commands from computed style** — `paint.rs`:
   - `FillGradient` from background-image linear-gradient / radial-gradient
   - `DrawImage` from `<img>` tags and background-image urls
   - `Border` from border-{top,right,bottom,left}-{width,style,color} (solid, dashed, dotted)
   - `SetClip/PopClip` from overflow:hidden / border-radius
   - `SetOpacity/PopOpacity` from opacity property
   - `TextRun` with proper font-family fallback, text-decoration
   - stacked backgrounds, multiple background layers

4. **display renderer — pipelines for every `DisplayCommand`** — `display_renderer.rs`:
   - solid fill pipeline (FillRect) — batched quads, solid color shader
   - textured quad pipeline (TextRun + DrawImage) — glyph atlas / stb_image → wgpu texture → quads with UV
   - gradient pipeline (FillGradient) — shader interpolates between N color stops
   - clip/opacity — SetClip/PopClip → wgpu scissor rect; SetOpacity/PopOpacity → separate blend pipeline
   - border — Border → 4 solid rects (top, right, bottom, left) with per-side style
   - glyph atlas — freetype fill_glyphs() → wgpu texture atlas, lazy rasterize per (codepoint, font_size)

5. **full html page pipeline** — `main.rs`:
   - fetch real page → parse → stylo style → render tree → layout → display list → render
   - dump display list with all command types, verify coordinates visually correct
   - handle external stylesheets (&lt;link rel="stylesheet"&gt;)
   - incremental / re-style on viewport resize


## ai code rules

1. **confirm important decisions** — ask before wiring up major changes
2. **never change existing comments** — leave them as-is
3. **comment style** — minimalist english lowercase, only where clarity requires it. no decorative or redundant comments
4. **no new documentation files** unless explicitly asked (no readme, no docs/). exceptions: this file, agents.md
5. **minimal diffs** — change only what is needed for the task
6. **language** — instructions in russian are fine; code comments in english
7. **never commit automatically** — i will tell you when to commit/push. do not commit on your own.
