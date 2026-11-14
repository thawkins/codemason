//! Crossterm-typed entry points into the frontend-neutral input handlers.
//!
//! The app core takes [`KeyInput`] / [`MouseInput`]; the terminal frontend and
//! the test-suite speak crossterm. These thin wrappers convert through
//! [`crate::adapters`] and apply the terminal loop's release filtering, so a
//! crossterm event reaches the core exactly as it does from `run_tui`.
//!
//! Depends on `crossterm`, `codemason-app-core` and [`crate::adapters`].

use codemason_app_core::input_event::{Key, KeyInput};
use crossterm::event::{KeyEvent, KeyEventKind, MouseEvent};

use crate::adapters::{from_crossterm_key, from_crossterm_modifiers, from_crossterm_mouse};
use crate::app::App;
use crate::input::InputAction;

/// Convert a crossterm key event into a [`KeyInput`].
///
/// # Arguments
/// * `ev` - the crossterm key event (its press/release kind is ignored).
///
/// # Returns
/// The neutral key; an unmappable key becomes [`Key::Other`].
pub fn key_input(ev: KeyEvent) -> KeyInput {
    match from_crossterm_key(ev) {
        Some(input) => input,
        None => KeyInput {
            key: Key::Other,
            mods: from_crossterm_modifiers(ev.modifiers),
        },
    }
}

/// Whether `ev` is a key release, which the app never handles.
fn is_release(ev: &KeyEvent) -> bool {
    ev.kind == KeyEventKind::Release
}

/// Crossterm-typed methods on [`App`], mirroring the terminal event loop.
#[allow(async_fn_in_trait)]
pub trait CrosstermAppExt {
    /// Feed a crossterm key event to [`App::handle_key`], ignoring releases.
    async fn handle_key_event(&mut self, key: KeyEvent);

    /// Feed a crossterm mouse event to [`App::handle_mouse`]; events with no
    /// neutral equivalent (horizontal scroll) are dropped.
    fn handle_mouse_event(&mut self, event: MouseEvent);
}

impl CrosstermAppExt for App {
    async fn handle_key_event(&mut self, key: KeyEvent) {
        if is_release(&key) {
            return;
        }
        self.handle_key(key_input(key)).await;
    }

    fn handle_mouse_event(&mut self, event: MouseEvent) {
        if let Some(input) = from_crossterm_mouse(event) {
            self.handle_mouse(input);
        }
    }
}

/// Crossterm-typed [`crate::input::handle_key`]: releases yield `None`.
///
/// # Arguments
/// * `app` - the application state.
/// * `key` - the crossterm key event.
///
/// # Returns
/// The [`InputAction`] the key maps to, if any.
pub async fn handle_key(app: &mut App, key: KeyEvent) -> Option<InputAction> {
    if is_release(&key) {
        return None;
    }
    crate::input::handle_key(app, key_input(key)).await
}

/// Crossterm-typed [`crate::input::is_global_quit_key`].
///
/// # Arguments
/// * `key` - the crossterm key event.
///
/// # Returns
/// `true` for `Ctrl+C` / `Ctrl+D`.
pub fn is_global_quit_key(key: KeyEvent) -> bool {
    crate::input::is_global_quit_key(key_input(key))
}

/// Crossterm-typed [`crate::app::handle_loop_setup_key`].
///
/// # Arguments
/// * `app` - the application state.
/// * `key` - the crossterm key event.
pub fn handle_loop_setup_key(app: &mut App, key: KeyEvent) {
    crate::app::handle_loop_setup_key(app, key_input(key));
}
