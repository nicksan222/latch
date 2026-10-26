//! `ClipboardBackend` over `tauri-plugin-clipboard-manager`.
//!
//! Values are written with `write_secret`, which sets the hints clipboard
//! managers use to keep the text out of their history. Reads happen only on
//! the agent timer thread (clear-if-unchanged), never on the main thread
//! (the plugin warns that would deadlock on Linux).

use tauri::{AppHandle, Runtime};
use tauri_plugin_clipboard_manager::ClipboardExt;

use latch::agent::clipboard::ClipboardBackend;

pub struct TauriClipboard<R: Runtime> {
    app: AppHandle<R>,
}

impl<R: Runtime> TauriClipboard<R> {
    pub fn new(app: AppHandle<R>) -> Self {
        Self { app }
    }
}

impl<R: Runtime> ClipboardBackend for TauriClipboard<R> {
    fn set_text(&self, text: &str) -> Result<(), String> {
        self.app
            .clipboard()
            .write_secret(text)
            .map_err(|_| "clipboard operation failed".to_owned())
    }

    fn get_text(&self) -> Option<String> {
        self.app.clipboard().read_text().ok()
    }

    fn clear(&self) -> Result<(), String> {
        self.app
            .clipboard()
            .clear()
            .map_err(|_| "clipboard operation failed".to_owned())
    }
}
