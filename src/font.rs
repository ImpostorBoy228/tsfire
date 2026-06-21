// real freetype backend — only compiled when freetype2 found at build time
#[cfg(freetype_avail)]
mod imp {
    use std::ffi::c_void;
    use std::ptr;

    #[repr(C)]
    pub struct GlyphInfo {
        pub codepoint: u32,
        pub adv_x: f32,
        pub br_x: f32,
        pub br_y: f32,
        pub bm_width: i32,
        pub bm_rows: i32,
        pub bm_pitch: i32,
        pub bm_offset: i32,
        pub ker_x: f32,
        pub ker_y: f32,
    }

    unsafe extern "C" {
        fn font_load(data: *const u8, len: std::ffi::c_ulong, pixel_size: f32) -> *mut c_void;
        fn font_free(font: *mut c_void);
        fn cock_measure(font: *mut c_void, utf8: *const u8, len: std::ffi::c_ulong) -> f32;
        fn font_fill_glyphs(
            font: *mut c_void,
            utf8: *const u8,
            len: std::ffi::c_ulong,
            out_infos: *mut GlyphInfo,
            max_glyphs: i32,
            out_bitmap: *mut *mut u8,
            out_bitmap_size: *mut std::ffi::c_ulong,
        ) -> i32;
        fn free_bitmap_buffer(ptr: *mut u8);
    }

    pub struct FontHandle {
        inner: *mut c_void,
        _data: Box<[u8]>,
    }

    unsafe impl Send for FontHandle {}
    unsafe impl Sync for FontHandle {}

    impl FontHandle {
        pub fn load(data: Box<[u8]>, pixel_size: f32) -> Option<Self> {
            let inner = unsafe {
                font_load(data.as_ptr(), data.len() as std::ffi::c_ulong, pixel_size)
            };
            if inner.is_null() {
                None
            } else {
                Some(FontHandle { inner, _data: data })
            }
        }

        pub fn measure(&self, text: &str) -> f32 {
            unsafe { cock_measure(self.inner, text.as_ptr(), text.len() as std::ffi::c_ulong) }
        }

        pub fn fill_glyphs(&self, text: &str) -> Option<(Vec<GlyphInfo>, Vec<u8>)> {
            let max = text.chars().count() as i32;
            if max == 0 {
                return None;
            }

            let mut infos = (0..max as usize)
                .map(|_| GlyphInfo {
                    codepoint: 0,
                    adv_x: 0.0,
                    br_x: 0.0,
                    br_y: 0.0,
                    bm_width: 0,
                    bm_rows: 0,
                    bm_pitch: 0,
                    bm_offset: 0,
                    ker_x: 0.0,
                    ker_y: 0.0,
                })
                .collect::<Vec<_>>();

            let mut bitmap: *mut u8 = ptr::null_mut();
            let mut bitmap_size: std::ffi::c_ulong = 0;

            let count = unsafe {
                font_fill_glyphs(
                    self.inner,
                    text.as_ptr(),
                    text.len() as std::ffi::c_ulong,
                    infos.as_mut_ptr(),
                    max,
                    &mut bitmap,
                    &mut bitmap_size,
                )
            };

            if count < 0 {
                return None;
            }

            infos.truncate(count as usize);

            let bm = if bitmap_size > 0 && !bitmap.is_null() {
                let slice = unsafe { std::slice::from_raw_parts(bitmap, bitmap_size as usize) };
                let owned = slice.to_vec();
                unsafe { free_bitmap_buffer(bitmap) };
                owned
            } else {
                Vec::new()
            };

            Some((infos, bm))
        }
    }

    impl Drop for FontHandle {
        fn drop(&mut self) {
            if !self.inner.is_null() {
                unsafe { font_free(self.inner) };
            }
        }
    }
}

// stub — used when freetype2 is not available
#[cfg(not(freetype_avail))]
mod imp {
    pub struct GlyphInfo;

    pub struct FontHandle;

    impl FontHandle {
        pub fn load(_data: Box<[u8]>, _pixel_size: f32) -> Option<Self> {
            None
        }

        pub fn measure(&self, _text: &str) -> f32 {
            0.0
        }

        pub fn fill_glyphs(&self, _text: &str) -> Option<(Vec<GlyphInfo>, Vec<u8>)> {
            None
        }
    }
}

#[cfg_attr(not(freetype_avail), allow(unused_imports))]
pub use imp::*;

#[cfg(freetype_avail)]
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cock_measure() {
        let font_data: &[u8] = include_bytes!("/usr/share/fonts/TTF/DejaVuSans.ttf");
        let boxed: Box<[u8]> = Box::from(font_data);
        let font = FontHandle::load(boxed, 16.0).expect("font_load failed");
        let w = font.measure("Hello World");
        assert!(w > 0.0, "width should be > 0, got {}", w);
        let w2 = font.measure("Привет мир");
        assert!(w2 > 0.0, "cyrillic width should be > 0, got {}", w2);
        println!("OK: 'Hello World' = {}px, 'Привет мир' = {}px", w, w2);
    }

    #[test]
    fn test_fill_glyphs() {
        let font_data: &[u8] = include_bytes!("/usr/share/fonts/TTF/DejaVuSans.ttf");
        let boxed: Box<[u8]> = Box::from(font_data);
        let font = FontHandle::load(boxed, 16.0).expect("font_load failed");
        let (infos, bitmap) = font.fill_glyphs("Hi").expect("fill_glyphs failed");
        assert_eq!(infos.len(), 2, "should have 2 glyphs");
        assert!(infos[0].adv_x > 0.0);
        assert!(infos[1].adv_x > 0.0);
        assert!(!bitmap.is_empty(), "should have bitmap data");
        println!("OK: {} glyphs, {} bytes bitmap", infos.len(), bitmap.len());
    }
}
