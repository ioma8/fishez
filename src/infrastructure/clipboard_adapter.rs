//! Clipboard adapter - emits OSC 52 clipboard escape codes.

use crate::application::ports::ClipboardPort;
use base64::Engine;
use std::io::{self, Write};

/// System clipboard adapter.
pub struct SystemClipboard;

impl Default for SystemClipboard {
    fn default() -> Self {
        Self::new()
    }
}

impl SystemClipboard {
    pub fn new() -> Self {
        Self
    }
}

impl ClipboardPort for SystemClipboard {
    fn copy(&mut self, text: &str) -> Result<(), String> {
        let mut stdout = io::stdout();
        stdout
            .write_all(osc52_escape(text).as_bytes())
            .map_err(|e| e.to_string())?;
        stdout.flush().map_err(|e| e.to_string())
    }
}

fn osc52_escape(text: &str) -> String {
    format!(
        "\x1b]52;c;{}\x07",
        base64::engine::general_purpose::STANDARD.encode(text.as_bytes())
    )
}

#[cfg(test)]
mod tests {
    use super::osc52_escape;

    #[test]
    fn encodes_osc52_clipboard_sequence() {
        assert_eq!(osc52_escape("file.txt"), "\u{1b}]52;c;ZmlsZS50eHQ=\u{7}");
    }
}
