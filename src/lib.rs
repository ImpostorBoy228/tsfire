#![allow(dead_code)]
mod image_handler;

pub use image_handler::ImageData;

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_PNG: &[u8] = include_bytes!("../test.png");

    #[test]
    fn test_decode_image() {
        let img = ImageData::decode(TEST_PNG).expect("Failed to decode");
        println!("Image size: {}x{}", img.width(), img.height());
        println!("Total bytes: {}", img.pixels().len());
    }
}
