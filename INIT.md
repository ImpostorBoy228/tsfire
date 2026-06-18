# tsfire

experimental browser engine in rust. fetches pages, parses html/css, builds render tree.

## project structure

```
src/
├── main.rs      entry: fetch page → parse dom → build render tree → dump
├── network.rs   user-agent builder
├── parse.rs     html parser (html5ever), css parser & selector matching (cssparser + selectors)
└── render.rs    render tree builder & dumper
```

dependencies: tokio, reqwest, html5ever, markup5ever_rcdom, cssparser, selectors, precomputed-hash

## current state

- html → dom → render tree → `dump()` (tree print)
- css collection & parsing exists, but style matching is **not** yet integrated into render tree
- workflow: `cargo run` fetches wikipedia.org and prints tree

## ai code rules

1. **confirm important decisions** — ask before wiring up major changes
2. **never change existing comments** — leave them as-is
3. **comment style** — minimalist english lowercase, only where clarity requires it. no decorative or redundant comments
4. **no new documentation files** unless explicitly asked (no readme, no docs/). exceptions: this file, agnets.md
5. **minimal diffs** — change only what is needed for the task
6. **language** — instructions in russian are fine; code comments in english
