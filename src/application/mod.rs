//! Application layer - use cases and ports (business logic).

pub mod ports;
pub mod state;
pub mod use_cases;

pub use state::{ActivePane, AppState, PanelMode, PanelState, QuickViewMode, SizeFigure};
