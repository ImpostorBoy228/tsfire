# Agents Configuration

This file documents the commands and tools used in this project.

## Build Commands
- `cargo build` - Build the project
- `cargo run` - Run the browser engine
- `cargo test` - Run tests
- `cargo check` - Check compilation without building

## Code Formatting
- `cargo fmt` - Format code
- `cargo clippy` - Lint code

## Dependencies
- tokio, reqwest - Networking
- html5ever, markup5ever_rcdom - HTML parsing
- cssparser, selectors - CSS parsing and selector matching
- stylo, style, style_traits - Mozilla's CSS engine
- euclid, app_units - Geometry and units
- string_cache, web_atoms, stylo_atoms - String handling