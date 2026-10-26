//! Pluggable clipboard used by `clipboard.copy`.
//!
//! The agent resolves the value itself and writes it through a
//! [`ClipboardBackend`]; a clear is scheduled that only fires when the
//! clipboard still holds that value. The default backend is
//! [`ArboardClipboard`] (OS clipboard via `arboard`); the desktop shell can
//! pass its own (e.g. wrapping `tauri-plugin-clipboard-manager`) through
//! `AgentOptions::clipboard`, and tests use [`MemoryClipboard`].

use std::sync::Mutex;

/// Errors are short static-ish strings and never contain clipboard content.
pub trait ClipboardBackend: Send + Sync {
    fn set_text(&self, text: &str) -> Result<(), String>;
    /// Current text content, `None` when empty/unavailable/non-text.
    fn get_text(&self) -> Option<String>;
    fn clear(&self) -> Result<(), String>;
}

/// OS clipboard through `arboard`. The handle is created lazily and kept
/// alive, which matters on X11 where the owning process serves the content.
#[derive(Default)]
pub struct ArboardClipboard {
    inner: Mutex<Option<arboard::Clipboard>>,
}

impl ArboardClipboard {
    pub fn new() -> Self {
        Self::default()
    }

    fn with<T>(
        &self,
        f: impl FnOnce(&mut arboard::Clipboard) -> Result<T, arboard::Error>,
    ) -> Result<T, String> {
        let mut g = self.inner.lock().unwrap_or_else(|p| p.into_inner());
        if g.is_none() {
            *g = Some(arboard::Clipboard::new().map_err(|_| "clipboard unavailable".to_owned())?);
        }
        let cb = g.as_mut().expect("initialized above");
        f(cb).map_err(|e| match e {
            arboard::Error::ContentNotAvailable => "clipboard empty".to_owned(),
            _ => "clipboard operation failed".to_owned(),
        })
    }
}

impl ClipboardBackend for ArboardClipboard {
    fn set_text(&self, text: &str) -> Result<(), String> {
        self.with(|c| c.set_text(text.to_owned()))
    }
    fn get_text(&self) -> Option<String> {
        self.with(|c| c.get_text()).ok()
    }
    fn clear(&self) -> Result<(), String> {
        self.with(|c| c.clear())
    }
}

/// In-memory clipboard for tests and headless environments.
#[derive(Default)]
pub struct MemoryClipboard {
    content: Mutex<Option<String>>,
}

impl MemoryClipboard {
    pub fn new() -> Self {
        Self::default()
    }
}

impl ClipboardBackend for MemoryClipboard {
    fn set_text(&self, text: &str) -> Result<(), String> {
        *self.content.lock().unwrap_or_else(|p| p.into_inner()) = Some(text.to_owned());
        Ok(())
    }
    fn get_text(&self) -> Option<String> {
        self.content
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone()
    }
    fn clear(&self) -> Result<(), String> {
        *self.content.lock().unwrap_or_else(|p| p.into_inner()) = None;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_backend() {
        let c = MemoryClipboard::new();
        assert_eq!(c.get_text(), None);
        c.set_text("x").unwrap();
        assert_eq!(c.get_text().as_deref(), Some("x"));
        c.clear().unwrap();
        assert_eq!(c.get_text(), None);
    }
}
