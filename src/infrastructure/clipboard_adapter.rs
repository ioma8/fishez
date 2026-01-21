//! Clipboard adapter - implements ClipboardPort using clipboard crate.

use crate::application::ports::ClipboardPort;
use clipboard::{ClipboardContext, ClipboardProvider};

/// System clipboard adapter.
pub struct SystemClipboard {
    ctx: Option<ClipboardContext>,
}

impl Default for SystemClipboard {
    fn default() -> Self {
        Self::new()
    }
}

impl SystemClipboard {
    pub fn new() -> Self {
        Self {
            ctx: ClipboardProvider::new().ok(),
        }
    }
}

impl ClipboardPort for SystemClipboard {
    fn copy(&mut self, text: &str) -> Result<(), String> {
        let Some(ctx) = &mut self.ctx else {
            return Err("Clipboard unavailable".to_string());
        };
        ctx.set_contents(text.to_string())
            .map_err(|e| e.to_string())
    }
}
