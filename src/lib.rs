// Clean Architecture Layers
pub mod application;
pub mod domain;
pub mod infrastructure;
pub mod presentation;

// Legacy modules (kept for backward compatibility during migration)
pub mod features;
pub mod files_view;
pub mod logger;
pub mod terminal_ui;
