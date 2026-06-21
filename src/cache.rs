use std::collections::HashMap;
use std::hash::{Hash, Hasher};

use crate::font::GlyphInfo;

/// Caches glyph metrics (`Vec<GlyphInfo>`) per text run to avoid
/// calling freetype `fill_glyphs` every frame.  The bitmap data is
/// **not** cached — it is only needed once for the first GPU atlas
/// upload.
pub struct GlyphMetricsCache {
    entries: HashMap<u64, Vec<GlyphInfo>>,
}

impl GlyphMetricsCache {
    pub fn new() -> Self {
        Self { entries: HashMap::new() }
    }

    pub fn get(&self, font: &crate::font::FontHandle, font_size: f32, text: &str) -> Option<&Vec<GlyphInfo>> {
        let key = Self::key(font, font_size, text);
        self.entries.get(&key)
    }

    pub fn insert(&mut self, font: &crate::font::FontHandle, font_size: f32, text: &str, glyphs: Vec<GlyphInfo>) {
        let key = Self::key(font, font_size, text);
        self.entries.insert(key, glyphs);
    }

    fn key(font: &crate::font::FontHandle, font_size: f32, text: &str) -> u64 {
        let mut s = std::collections::hash_map::DefaultHasher::new();
        std::ptr::from_ref(font).hash(&mut s);
        font_size.to_bits().hash(&mut s);
        text.hash(&mut s);
        s.finish()
    }
}
