use std::env;
use std::path::PathBuf;

fn main() {
    let crate_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());

    // workspace target/ is one level up from the runtime crate dir
    let include_dir = crate_dir.join("..").join("target").join("include");
    std::fs::create_dir_all(&include_dir).expect("create target/include");

    // Tolerate empty lib in Step 1 — cbindgen returns Err when there
    // are no extern fns to emit. Real header lands once Step 8 adds
    // the public C-ABI symbols.
    if let Ok(bindings) = cbindgen::Builder::new()
        .with_crate(&crate_dir)
        .with_language(cbindgen::Language::C)
        .with_include_guard("SKEV_RUNTIME_H")
        .generate()
    {
        bindings.write_to_file(include_dir.join("skev_runtime.h"));
    }

    println!("cargo:rerun-if-changed=src/lib.rs");
    println!("cargo:rerun-if-changed=build.rs");
}
