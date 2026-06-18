# tsfire

experimental browser engine in rust. fetches pages, parses html/css, builds render tree, computes layout.

## project structure

```
src/
├── main.rs      entry: fetch → dom → css → render tree → layout → dump
├── network.rs   user-agent builder
├── parse.rs     html parser (html5ever), css parser & selector matching (cssparser + selectors)
├── render.rs    render tree builder & style integration
├── style.rs     css value types, computedvalues, cascade with specificity
└── layout.rs    layoutengine trait, box tree with positioned/sized rects
```

dependencies: tokio, reqwest, html5ever, markup5ever_rcdom, cssparser, selectors, precomputed-hash

## current state

- html → dom → css collection → cascade (specificity sort + merge) → styled render tree
- block + inline layout → positioned `LayoutBox` tree with computed styles
- `LayoutEngine` trait for swappable layout backends
- dump shows render tree (with style attrs) and layout tree (with coordinates + sizes)
- workflow: `cargo run` fetches wikipedia.org, prints both trees

## ai code rules

1. **confirm important decisions** — ask before wiring up major changes
2. **never change existing comments** — leave them as-is
3. **comment style** — minimalist english lowercase, only where clarity requires it. no decorative or redundant comments
4. **no new documentation files** unless explicitly asked (no readme, no docs/). exceptions: this file, agnets.md
5. **minimal diffs** — change only what is needed for the task
6. **language** — instructions in russian are fine; code comments in english
