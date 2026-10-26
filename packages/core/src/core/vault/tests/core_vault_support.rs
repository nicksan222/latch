// Imports and helpers shared by the feature-owned core_vault acceptance parts.
// Included once at the Cargo test crate root to preserve exact subprocess names.

#[path = "../../../../tests/core_support/mod.rs"]
mod core_support;

use core_support::*;
use latch::core::{Error, Vault, format};
use latch::protocol::*;

fn walk(p: &std::path::Path) -> Vec<std::path::PathBuf> {
    let mut v = vec![];
    for e in std::fs::read_dir(p).unwrap().flatten() {
        if e.path().is_dir() {
            v.extend(walk(&e.path()));
        } else {
            v.push(e.path());
        }
    }
    v
}
