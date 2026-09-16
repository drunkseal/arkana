use std::path::PathBuf;

fn main() {
    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let config = slint_build::CompilerConfiguration::new().with_include_paths(vec![
        manifest_dir.join("ui"),
        manifest_dir.join("assets"),
    ]);
    slint_build::compile_with_config("ui/main_window.slint", config).unwrap();
}
