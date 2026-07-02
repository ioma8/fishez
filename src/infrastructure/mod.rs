//! Infrastructure layer - implementations of application ports.

pub mod clipboard_adapter;
pub mod disk_usage_adapter;
pub mod favorites_adapter;
pub mod fs_adapter;
pub mod open_adapter;
pub mod search_adapter;

pub use clipboard_adapter::SystemClipboard;
pub use disk_usage_adapter::disk_free_and_total;
pub use favorites_adapter::{add_favorite, is_onboarded, load_favorites, mark_onboarded};
pub use fs_adapter::StdFileSystem;
pub use open_adapter::{SystemOpenAdapter, VsCodeAdapter};
pub use search_adapter::{FdSearchAdapter, RipGrepAdapter};
