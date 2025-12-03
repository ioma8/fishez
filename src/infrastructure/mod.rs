//! Infrastructure layer - implementations of application ports.

pub mod clipboard_adapter;
pub mod fs_adapter;
pub mod open_adapter;
pub mod search_adapter;

pub use clipboard_adapter::SystemClipboard;
pub use fs_adapter::StdFileSystem;
pub use open_adapter::{SystemOpenAdapter, VsCodeAdapter};
pub use search_adapter::{FdSearchAdapter, RipGrepAdapter};
