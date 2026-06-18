use std::ptr;

// FFI shit
unsafe extern "C" {
    fn idecode(
        data: *const u8,
        len: std::os::raw::c_ulong,
        out_rgba: *mut *mut u8,
        out_w: *mut i32,
        out_h: *mut i32,
    ) -> i32;

    fn ifree(pixels: *mut u8);
}

// RAII struct
pub struct ImageData {
    pixels: *mut u8,
    width: u32,
    height: u32,
}

impl ImageData {
    pub fn decode(data: &[u8]) -> Result<Self, &'static str> {
        let mut pixels: *mut u8 = ptr::null_mut();
        let mut width: i32 = 0;
        let mut height: i32 = 0;

        let result = unsafe {
            idecode(
                data.as_ptr(),
                data.len() as std::os::raw::c_ulong,
                &mut pixels,
                &mut width,
                &mut height,
            )
        };

        if result != 0 || pixels.is_null() {
            return Err("Failed to decode image");
        }

        Ok(ImageData {
            pixels,
            width: width as u32,
            height: height as u32,
        })
    }

    pub fn width(&self) -> u32 { self.width }
    pub fn height(&self) -> u32 { self.height }

    // Zero-copy доступ к пикселям
    pub fn pixels(&self) -> &[u8] {
        let len = (self.width * self.height * 4) as usize; // RGBA = 4 байта на пиксель
        unsafe { std::slice::from_raw_parts(self.pixels, len) }
    }
}

impl Drop for ImageData {
    fn drop(&mut self) {
        if !self.pixels.is_null() {
            unsafe { ifree(self.pixels) };
            self.pixels = ptr::null_mut();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_PNG: &[u8] = include_bytes!("../test.png");

    #[test]
    fn test_decode_png() {
        let img = ImageData::decode(TEST_PNG).expect("decode failed");
        assert!(img.width > 0, "width should be > 0, got {}", img.width);
        assert!(img.height > 0, "height should be > 0, got {}", img.height);
        let px = img.pixels();
        assert_eq!(px.len(), (img.width * img.height * 4) as usize, "pixel buffer size mismatch");
        // check first pixel is valid RGBA (non-zero in at least one channel)
        let non_zero = px.iter().any(|&b| b != 0);
        assert!(non_zero, "image has no non-zero pixels");
        println!("OK: {}x{} RGBA ({} bytes)", img.width, img.height, px.len());
    }
}
