//! Build script for l402-ffi: generates C headers via cbindgen.

fn main() {
    let crate_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let output_dir = std::path::Path::new(&crate_dir).join("include");
    std::fs::create_dir_all(&output_dir).ok();

    if let Ok(bindings) = cbindgen::Builder::new()
        .with_crate(&crate_dir)
        .with_config(cbindgen::Config::from_file(format!("{crate_dir}/cbindgen.toml")).unwrap())
        .generate()
    {
        bindings.write_to_file(output_dir.join("l402.h"));
    }
}
