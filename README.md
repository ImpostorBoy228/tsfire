# tsfire

experimental low-ram browser engine in rust.

![test](test.png)
fetches pages → parses html/css (via stylo) → builds render tree →
computes layout → produces display list → (planned) renders via webrender.

**primary goal:** lower memory footprint than chrome/firefox.
every design decision prioritizes this. no multi-process, no per-element pixel buffers, no shadow dom copies.

## current state

- http fetch + html5ever dom → css collection → stylo css engine
- block/inline layout with freetype-based text measurement (kerning-aware)
- flat DisplayList with FillRect, FillGradient, DrawImage, TextRun, Border, Clip, Opacity
- image decoding via stb_image (png/jpeg/webp → rgba)
- font decoding via freetype2 (ttf → glyph metrics + grayscale bitmap)
- output: display list dump to stdout

## architecture

```
fetch → parse(html) → collect(css) → stylo(compute) → render tree
→ layout → display list → [webrender → window]
```

```
src/
├── main.rs              entry point
├── network.rs           http / user-agent
├── parse.rs             html + css parsing
├── stylo_integration.rs stylo bridge (property parsing + computation)
├── render.rs            render tree builder
├── layout.rs            block/inline layout
├── paint.rs             DisplayList builder
├── image_handler.rs     stb_image FFI
├── font.rs              freetype FFI
├── style.rs             old cascade (reference)
├── cache.rs             (reserved)
├── lib.rs               crate root
└── cmod/
    ├── image_handler.c  stb_image wrapper
    ├── stb_image.h      image decoder
    ├── font_handler.c   freetype wrapper
    └── font_handler.h   GlyphInfo struct
```

## design rules

1. no pixel buffers per element — flat display list (~32 bytes/command)
2. no shadow dom allocations — no deep style copies per node
3. single process — one stylist, one arena
4. zero-copy where possible — css text → parser → values, no string maps
5. lazy image decode — decoded on first paint, freed when not visible
6. no precomputed layout cache — layout is cheap
7. glyph atlas, not per-text allocations — shared font textures

## building

```sh
cargo build
cargo run
```

freetype2 is auto-detected. if missing, font measurement falls back to `chars*0.6` estimate.

## dependencies

tokio, reqwest, html5ever, markup5ever_rcdom, cssparser, selectors,
euclid, app_units, string_cache, stylo (servo), freetype2 (optional)
