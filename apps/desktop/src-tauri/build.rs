fn main() {
    // The app command `rpc` gets its own permission (`allow-rpc`) so the
    // capability file lists exactly what the webview may call.
    tauri_build::try_build(
        tauri_build::Attributes::new()
            .app_manifest(tauri_build::AppManifest::new().commands(&["rpc"])),
    )
    .expect("tauri build script failed");
}
