use std::env;
use std::path::PathBuf;

fn main() {
    // 1. Configure pkg-config to find the library,
    // BUT disable automatic Cargo emission so we can fix the flags manually.
    let mut config = pkg_config::Config::new();
    config.cargo_metadata(false);

    let library = match config.probe("libraw") {
        Ok(lib) => lib,
        Err(e) => panic!("Could not find libraw via pkg-config: {}", e),
    };

    // 2. Manually emit the link paths
    for path in &library.link_paths {
        println!("cargo:rustc-link-search=native={}", path.display());
    }

    // 3. Manually emit the libraries (THE FIX)
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();

    for lib in &library.libs {
        if lib == "stdc++" && target_os == "macos" {
            // REPLACE: On macOS, use libc++ instead of libstdc++
            println!("cargo:rustc-link-lib=c++");
        } else {
            // KEEP: All other libs (like 'raw', 'lcms2') stay the same
            println!("cargo:rustc-link-lib={}", lib);
        }
    }

    // Fallback: If pkg-config didn't mention C++ at all, force it for macOS
    if target_os == "macos" && !library.libs.contains(&"stdc++".to_string()) && !library.libs.contains(&"c++".to_string()) {
        println!("cargo:rustc-link-lib=c++");
    }

    // 4. Generate Bindings
    let bindings = bindgen::Builder::default()
        .header_contents("wrapper.h", "#include <libraw.h>")
        .clang_args(
            library.include_paths
                .iter()
                .map(|path| format!("-I{}", path.to_string_lossy())),
        )
        .allowlist_function("libraw_.*")
        .allowlist_type("libraw_.*")
        .allowlist_var("LIBRAW_.*")
        .generate()
        .expect("Unable to generate bindings");

    // 5. Write to file
    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("Couldn't write bindings!");
}