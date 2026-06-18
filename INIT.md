# tsfire

experimental browser engine in rust. fetches pages, parses html/css, builds render tree, computes layout.

## project structure

```
src/
├── main.rs      entry: fetch → dom → css → render tree → layout → dump
├── network.rs   user-agent builder
├── parse.rs     html parser (html5ever), css collection
├── render.rs    render tree builder
├── layout.rs    layoutengine trait, box tree with positioned/sized rects
├── paint.rs     display list builder and renderer
└── stylo_integration.rs   mozilla stylo css engine integration
```

dependencies: tokio, reqwest, html5ever, markup5ever_rcdom, cssparser, selectors, precomputed-hash, euclid, app_units, string_cache, web_atoms, stylo_atoms, stylo, dom, style_traits

## current state

- html → dom → css collection → stylo css engine integration
- full css cascade, selector matching, and style computation via mozilla's stylo engine
- block layout → positioned `LayoutBox` tree with computed styles
- `LayoutEngine` trait for swappable layout backends
- paint system with display list rendering
- dump shows render tree (with style attrs) and layout tree (with coordinates + sizes)
- workflow: `cargo run` fetches wikipedia.org, prints both trees

## ai code rules

1. **confirm important decisions** — ask before wiring up major changes
2. **never change existing comments** — leave them as-is
3. **comment style** — minimalist english lowercase, only where clarity requires it. no decorative or redundant comments
4. **no new documentation files** unless explicitly asked (no readme, no docs/). exceptions: this file, agents.md
5. **minimal diffs** — change only what is needed for the task
6. **language** — instructions in russian are fine; code comments in english
