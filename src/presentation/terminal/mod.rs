//! Terminal presentation module.

pub mod image_protocol;
pub mod overlays;
pub mod renderer;

pub use renderer::{FOOTER_ROWS, HEADER_ROWS, TerminalRenderer};
