//! Rust automation and subprocess fixtures for the workspace.
#[cfg(feature = "containers")]
pub mod containers;
pub mod demo;
pub mod e2e;
pub mod fixtures;
pub mod native_browser;
pub mod release;
pub mod rust_checks;

pub fn repository() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("workspace root")
}
