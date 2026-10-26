//! Prepare the exact product runtime for real-browser verification. No host discovery.
use latch::core::{
    browser::{credentials, runtime},
    paths,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = std::env::args_os()
        .nth(1)
        .ok_or("expected an isolated browser cache directory")?;
    let root = std::path::Path::new(&root);
    paths::ensure_secure_dir(root)?;
    let executable = runtime::ensure(root)?;
    credentials::install_bootstrap(&executable)?;
    println!("{}", executable.display());
    Ok(())
}
