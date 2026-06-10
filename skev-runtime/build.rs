use std::env;
use std::path::PathBuf;

fn main() {
    let crate_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let out_path = PathBuf::from(&crate_dir)
        .join("..")
        .join("target")
        .join("include")
        .join("skev_runtime.h");

    std::fs::create_dir_all(out_path.parent().unwrap()).unwrap();

    // The programmatic Builder does NOT auto-load cbindgen.toml — it must
    // be passed explicitly, or item_types/exclude/guards are silently ignored.
    let config = cbindgen::Config::from_root_or_default(&crate_dir);

    cbindgen::Builder::new()
        .with_crate(&crate_dir)
        .with_config(config)
        .generate()
        .expect("cbindgen failed — check pub use items resolve and cbindgen >= 0.29 is in [build-dependencies]")
        .write_to_file(out_path);

    // Regenerate when the surface or generator config changes
    // (prevents the stale-header problem that masked the 0.27 bug).
    println!("cargo:rerun-if-changed=src/lib.rs");
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=cbindgen.toml");
}
