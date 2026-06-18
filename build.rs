use std::path::PathBuf;
use std::process::Command;

fn ensure_stb_image_h(src_dir: &std::path::Path) {
    let path = src_dir.join("stb_image.h");
    if path.exists() {
        return;
    }
    println!("cargo:warning=stb_image.h not found, downloading...");
    let url = "https://raw.githubusercontent.com/nothings/stb/refs/heads/master/stb_image.h";
    let status = Command::new("curl")
        .args(["-fsSL", "-o", &path.to_string_lossy(), url])
        .status()
        .expect("failed to run curl");
    if !status.success() {
        panic!("failed to download stb_image.h from {}", url);
    }
    println!("cargo:warning=stb_image.h downloaded");
}

fn main() {
    let src_dir = PathBuf::from("src/cmod");
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").unwrap());

    ensure_stb_image_h(&src_dir);

    // --- image_handler (always) ---
    cc::Build::new()
        .file(src_dir.join("image_handler.c"))
        .include(&src_dir)
        .compile("image_handler");
    println!("cargo:rustc-link-arg={}", out_dir.join("libimage_handler.a").display());
    println!("cargo:rerun-if-changed=src/cmod/image_handler.c");

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
