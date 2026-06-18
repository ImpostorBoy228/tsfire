use std::path::PathBuf;

fn main() {
    let src_dir = PathBuf::from("src/cmod");
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").unwrap());

    // freetype2 includes/libs через pkg-config
    let ft = pkg_config::Config::new().probe("freetype2").unwrap();

    // --- image_handler ---
    cc::Build::new()
        .file(src_dir.join("image_handler.c"))
        .include(&src_dir)
        .compile("image_handler");
    println!("cargo:rustc-link-arg={}", out_dir.join("libimage_handler.a").display());
    println!("cargo:rerun-if-changed=src/cmod/image_handler.c");
    println!("cargo:rerun-if-changed=src/cmod/stb_image.h");

    // --- font_handler ---
    let mut font_build = cc::Build::new();
    font_build
        .file(src_dir.join("font_handler.c"))
        .include(&src_dir);
    // freetype2 include paths
    for path in &ft.include_paths {
        font_build.include(path);
    }
    font_build.compile("font_handler");

    // Линкуем font_handler.a (workaround для -fuse-ld=lld)
    println!("cargo:rustc-link-arg={}", out_dir.join("libfont_handler.a").display());
    // Линкуем freetype2 (libfreetype.so/.a)
    println!("cargo:rustc-link-arg=-lfreetype");
    println!("cargo:rerun-if-changed=src/cmod/font_handler.c");
    println!("cargo:rerun-if-changed=src/cmod/font_handler.h");
}
