//! Presentation layer - UI rendering and input handling.

pub mod event_loop;
pub mod input_handler;
pub mod shortcuts;
pub mod terminal;

pub use event_loop::run;
pub use terminal::{FOOTER_ROWS, HEADER_ROWS, TerminalRenderer, overlays};
