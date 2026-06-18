mod image_handler;

pub use image_handler::ImageData;

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_decode_image() {
        let image_data = fs::read("test.png").expect("Failed to read test image");
        let img = ImageData::decode(&image_data).expect("Failed to decode");

        println!("Image size: {}x{}", img.width(), img.height());
        println!("Total bytes: {}", img.pixels().len());
    }
}
