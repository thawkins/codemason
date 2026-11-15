//! Platform services the window uses beside drawing: native file dialogs
//! ([`files`]), the system clipboard ([`clipboard`]), the macOS app menu
//! ([`menu`]) and the window lifecycle ([`window`]: the close confirmation
//! and the quit steps).

pub mod clipboard;
pub mod files;
pub mod menu;
pub mod window;
