use std::path::PathBuf;

fn main() {
    let src_dir = PathBuf::from("src/cmod");
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").unwrap());

    cc::Build::new()
        .file(src_dir.join("image_handler.c"))
        .include(src_dir)
        .flag("-O3")
        .compile("image_handler");

    // static lib declaration
    println!("cargo:rustc-link-search=native={}", out_dir.display());
    println!("cargo:rustc-link-lib=static=image_handler");

    // recompile if stb_image changed
    println!("cargo:rerun-if-changed=src/c/image_handler.c");
    println!("cargo:rerun-if-changed=src/c/stb_image.h");
}
