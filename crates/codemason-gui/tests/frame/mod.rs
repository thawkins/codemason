//! One GUI frame over an App, for tests ported from the terminal UI.
//!
//! The terminal renderer published state on the App while it drew: the
//! Agents and Teams row maps the cursor actions resolve against, the sidebar
//! rows and page height, and the memory rows. The GUI panels publish the same
//! state while they draw, so [`publish`] draws, once each, the panels the App
//! currently shows (the sessions sidebar, the Agents or Teams window, the
//! Memory panel) in an egui_kittest harness and returns. Nothing is
//! asserted about the drawing itself.
//!
//! Depends on `codemason-gui` (panels, palette), `codemason-app-core` and
//! `egui_kittest`.

#![allow(dead_code)]

use codemason_app_core::app::App;
use codemason_app_core::theme_mode::ThemeMode;
use codemason_gui::palette::palette_for;
use codemason_gui::panels::agents::agents_ui;
use codemason_gui::panels::memory::memory_ui;
use codemason_gui::panels::sessions::SessionsView;
use codemason_gui::panels::teams::teams_ui;
use egui_kittest::Harness;

/// Window size the panels are drawn at, in points.
const SIZE: egui::Vec2 = egui::vec2(1200.0, 640.0);

/// Draw one panel with `draw` in a fresh harness over `app`.
fn frame(app: &mut App, mut draw: impl FnMut(&mut egui::Ui, &mut App)) {
    let mut harness = Harness::builder()
        .with_size(SIZE)
        .build_ui(|ui| draw(ui, app));
    harness.step();
}

/// Draw the panels `app` shows, publishing the state they publish.
///
/// # Arguments
///
/// * `app` - the App to draw; its sidebar, Agents/Teams window and Memory
///   panel flags pick the panels.
pub fn publish(app: &mut App) {
    let palette = palette_for(ThemeMode::Default);
    if app.sidebar.visible {
        let mut view = SessionsView::default();
        frame(app, |ui, app| view.show(ui, app, &palette));
    }
    if app.show_agents_window {
        frame(app, |ui, app| agents_ui(ui, app, &palette));
    }
    if app.show_teams_window {
        frame(app, |ui, app| teams_ui(ui, app, &palette));
    }
    if app.show_memory {
        frame(app, |ui, app| memory_ui(ui, app, &palette));
    }
}
