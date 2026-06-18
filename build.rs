use std::path::PathBuf;

fn main() {
    let src_dir = PathBuf::from("src/cmod");
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").unwrap());

    // --- image_handler (always) ---
    cc::Build::new()
        .file(src_dir.join("image_handler.c"))
        .include(&src_dir)
        .compile("image_handler");
    println!("cargo:rustc-link-arg={}", out_dir.join("libimage_handler.a").display());
    println!("cargo:rerun-if-changed=src/cmod/image_handler.c");
    println!("cargo:rerun-if-changed=src/cmod/stb_image.h");

    // --- font_handler (optional, needs freetype2) ---
    #[cfg(feature = "freetype")]
    {
        if let Ok(ft) = pkg_config::Config::new().probe("freetype2") {
            let mut font_build = cc::Build::new();
            font_build
                .file(src_dir.join("font_handler.c"))
                .include(&src_dir);
            for path in &ft.include_paths {
                font_build.include(path);
            }
            font_build.compile("font_handler");

            println!("cargo:rustc-link-arg={}", out_dir.join("libfont_handler.a").display());
            println!("cargo:rustc-link-arg=-lfreetype");
            println!("cargo:rerun-if-changed=src/cmod/font_handler.c");
            println!("cargo:rerun-if-changed=src/cmod/font_handler.h");
        } else {
            println!("cargo:warning=freetype2 not found, font support disabled");
        }
    }
}
