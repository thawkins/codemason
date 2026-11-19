//! The app's title strip as the window's real title bar.
//!
//! craft-window decides at startup who draws the window frame
//! ([`decide_frame`]): on most desktops the app does
//! ([`Frame::Integrated`]: decorations off, the app draws the caption
//! buttons), on macOS the traffic lights sit over the app's strip
//! ([`Frame::Inset`]), and on tiling window managers, or when the user sets
//! `CODEMASON_TITLEBAR=system`, the system keeps its own
//! ([`Frame::Native`]). The decision holds for every theme, because the
//! window's decorations are fixed when it opens.
//!
//! When the app draws the frame, the title strip ([`crate::ribbon`]) calls
//! craft-window's [`craft_window::title_bar::empty_area`] so its empty part
//! moves the window and follows the system's double-click setting, draws
//! [`caption_buttons`] where the desktop puts them, and the shell lets the
//! window edges resize it ([`resize_edges`]), which an undecorated window
//! otherwise cannot.
//!
//! Dependencies: `egui`, `craft-window` (`policy`, `title_bar`), and this
//! crate's material icon buttons.

use craft_window::Button;
use craft_window::policy::{Backend, Decision, Frame, TitleBarPref, decide_for};
use egui::{CursorIcon, Pos2, Rect, ResizeDirection, ViewportCommand};

use crate::icons::Icon;
use crate::material::icon_button::{IconButtonKind, icon_button};
use crate::palette::Palette;

/// The prefix of the override variable: `CODEMASON_TITLEBAR=app|system`.
pub const PREFIX: craft_window::Prefix = craft_window::Prefix::new("CODEMASON");

/// Width of the band along each window edge that resizes the window, in
/// points.
pub const RESIZE_BORDER: f32 = 6.0;

/// Accessible name of the close caption button.
pub const CLOSE_LABEL: &str = "Close window";

/// Accessible name of the minimise caption button.
pub const MINIMIZE_LABEL: &str = "Minimise";

/// Accessible name of the maximise caption button.
pub const MAXIMIZE_LABEL: &str = "Maximise";

/// Accessible name of the maximise caption button while maximised.
pub const RESTORE_LABEL: &str = "Restore";

/// Decide who draws the window frame, once, before the window opens.
///
/// # Returns
///
/// craft-window's decision for this OS, desktop and display backend with
/// the app's bar preferred; `CODEMASON_TITLEBAR` (or `CRAFT_TITLEBAR`)
/// overrides it.
#[must_use]
pub fn decide_frame() -> Decision {
    decide_for(PREFIX, Backend::from_env(), TitleBarPref::Integrated)
}

/// Whether the app draws over the window's title bar with `frame`.
#[must_use]
pub const fn app_drawn(frame: &Frame) -> bool {
    !matches!(frame, Frame::Native)
}

/// Draw caption buttons for `buttons`, in the order given.
///
/// # Arguments
///
/// * `ui` - the title strip's ui, laid out in the direction the buttons go.
/// * `palette` - the colours.
/// * `buttons` - the window buttons to draw.
pub fn caption_buttons(ui: &mut egui::Ui, palette: &Palette, buttons: &[Button]) {
    let maximized = ui.input(|i| i.viewport().maximized).unwrap_or(false);
    for button in buttons {
        let (icon, label, command) = match button {
            Button::Minimize => (
                Icon::Minimize,
                MINIMIZE_LABEL,
                ViewportCommand::Minimized(true),
            ),
            Button::Maximize if maximized => (
                Icon::Restore,
                RESTORE_LABEL,
                ViewportCommand::Maximized(false),
            ),
            Button::Maximize => (
                Icon::Maximize,
                MAXIMIZE_LABEL,
                ViewportCommand::Maximized(true),
            ),
            // A close request goes through the shell's close guard, which
            // asks first while a turn runs.
            Button::Close => (Icon::Cross, CLOSE_LABEL, ViewportCommand::Close),
        };
        if icon_button(ui, palette, IconButtonKind::Standard, icon, label, None).clicked() {
            ui.ctx().send_viewport_cmd(command);
        }
    }
}

/// The resize direction of `pos` on `window`'s edges.
///
/// # Arguments
///
/// * `window` - the window's content rect.
/// * `pos` - the pointer position.
/// * `border` - the width of the resize band along each edge.
///
/// # Returns
///
/// The edge or corner `pos` is within `border` of, or `None` inside the
/// window (or outside it).
#[must_use]
pub fn resize_direction(window: Rect, pos: Pos2, border: f32) -> Option<ResizeDirection> {
    if !window.contains(pos) {
        return None;
    }
    let west = pos.x < window.left() + border;
    let east = pos.x > window.right() - border;
    let north = pos.y < window.top() + border;
    let south = pos.y > window.bottom() - border;
    match (north, south, west, east) {
        (true, _, true, _) => Some(ResizeDirection::NorthWest),
        (true, _, _, true) => Some(ResizeDirection::NorthEast),
        (_, true, true, _) => Some(ResizeDirection::SouthWest),
        (_, true, _, true) => Some(ResizeDirection::SouthEast),
        (true, _, _, _) => Some(ResizeDirection::North),
        (_, true, _, _) => Some(ResizeDirection::South),
        (_, _, true, _) => Some(ResizeDirection::West),
        (_, _, _, true) => Some(ResizeDirection::East),
        _ => None,
    }
}

/// The pointer cursor for resizing in `direction`.
const fn resize_cursor(direction: ResizeDirection) -> CursorIcon {
    match direction {
        ResizeDirection::North => CursorIcon::ResizeNorth,
        ResizeDirection::South => CursorIcon::ResizeSouth,
        ResizeDirection::East => CursorIcon::ResizeEast,
        ResizeDirection::West => CursorIcon::ResizeWest,
        ResizeDirection::NorthEast => CursorIcon::ResizeNorthEast,
        ResizeDirection::SouthEast => CursorIcon::ResizeSouthEast,
        ResizeDirection::NorthWest => CursorIcon::ResizeNorthWest,
        ResizeDirection::SouthWest => CursorIcon::ResizeSouthWest,
    }
}

/// Let the window's edges resize an undecorated window: the resize cursor
/// over an edge, and a press there starts the system's resize.
///
/// Does nothing while the window is maximised or full screen.
///
/// # Arguments
///
/// * `ctx` - the window's context; call once per frame, after the UI so the
///   cursor wins over the widgets at the edge.
pub fn resize_edges(ctx: &egui::Context) {
    let (fixed, window, pos, pressed) = ctx.input(|i| {
        let viewport = i.viewport();
        (
            viewport.maximized.unwrap_or(false) || viewport.fullscreen.unwrap_or(false),
            i.content_rect(),
            i.pointer.hover_pos(),
            i.pointer.primary_pressed(),
        )
    });
    if fixed {
        return;
    }
    let Some(direction) = pos.and_then(|pos| resize_direction(window, pos, RESIZE_BORDER)) else {
        return;
    };
    ctx.set_cursor_icon(resize_cursor(direction));
    if pressed {
        ctx.send_viewport_cmd(ViewportCommand::BeginResize(direction));
    }
}
