fn main() {
    // The app command `grpc_request` gets its own permission (`allow-grpc-request`) so the
    // capability file lists exactly what the webview may call.
    tauri_build::try_build(
        tauri_build::Attributes::new()
            .app_manifest(tauri_build::AppManifest::new().commands(&["grpc_request"])),
    )
    .expect("tauri build script failed");
}
