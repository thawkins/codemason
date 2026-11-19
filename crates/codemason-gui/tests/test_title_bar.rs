//! Tests for `codemason_gui::title_bar`: the resize bands along the window
//! edges, and the title strip with caption buttons drawn on every theme when
//! the app draws the window frame, where the desktop's layout puts them.

#[path = "support/mod.rs"]
mod support;

use codemason_app_core::theme_tokens::{ThemeCatalog, ThemeSelection};
use codemason_gui::Shell;
use codemason_gui::title_bar::{
    CLOSE_LABEL, MAXIMIZE_LABEL, MINIMIZE_LABEL, RESIZE_BORDER, app_drawn, resize_direction,
};
use craft_window::policy::{CaptionFamily, Captions, Frame};
use craft_window::{Button, Layout};
use egui::{ResizeDirection, pos2};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;

/// A window in theme `theme` drawing `frame`.
fn harness(theme: &str, frame: Frame) -> (tokio::runtime::Runtime, Harness<'static, Shell>) {
    let runtime = tokio::runtime::Runtime::new().expect("tokio runtime");
    let mut app = {
        let _enter = runtime.enter();
        support::make_app()
    };
    app.themes = ThemeCatalog::builtin();
    app.theme = ThemeSelection::Named(theme.to_string());
    let shell = Shell::new(app).with_title_bar(frame);
    let mut harness = Harness::builder()
        .with_size(egui::vec2(1200.0, 800.0))
        .allow_missing_glyphs()
        .build_ui_state(|ui, shell: &mut Shell| shell.update(ui), shell);
    harness.run();
    (runtime, harness)
}

/// An app-drawn frame with `layout`'s caption buttons.
fn integrated(layout: Layout) -> Frame {
    Frame::Integrated(Captions {
        family: CaptionFamily::Generic,
        layout,
    })
}

#[test]
fn test_title_bar_resize_direction_follows_edges_and_corners() {
    let window = egui::Rect::from_min_size(pos2(0.0, 0.0), egui::vec2(800.0, 600.0));
    let b = RESIZE_BORDER;
    let cases = [
        (pos2(1.0, 1.0), Some(ResizeDirection::NorthWest)),
        (pos2(799.0, 1.0), Some(ResizeDirection::NorthEast)),
        (pos2(1.0, 599.0), Some(ResizeDirection::SouthWest)),
        (pos2(799.0, 599.0), Some(ResizeDirection::SouthEast)),
        (pos2(400.0, 1.0), Some(ResizeDirection::North)),
        (pos2(400.0, 599.0), Some(ResizeDirection::South)),
        (pos2(1.0, 300.0), Some(ResizeDirection::West)),
        (pos2(799.0, 300.0), Some(ResizeDirection::East)),
        (pos2(400.0, 300.0), None),
        (pos2(b + 1.0, 300.0), None),
        (pos2(-5.0, 300.0), None),
    ];
    for (pos, expected) in cases {
        assert_eq!(resize_direction(window, pos, b), expected, "{pos:?}");
    }
}

#[test]
fn test_title_bar_captions_drawn_on_every_theme_with_an_app_frame() {
    assert!(!app_drawn(&Frame::Native));
    for theme in ["codemason-light", "document-light"] {
        let (_runtime, harness) = harness(theme, integrated(Layout::default()));
        for label in [MINIMIZE_LABEL, MAXIMIZE_LABEL, CLOSE_LABEL] {
            harness.get_by_label(label);
        }
        // Outermost last: close is the rightmost.
        let close = harness.get_by_label(CLOSE_LABEL).rect();
        let maximize = harness.get_by_label(MAXIMIZE_LABEL).rect();
        assert!(close.left() > maximize.right(), "{theme}");
    }
    let (_runtime, native) = harness("codemason-light", Frame::Native);
    assert!(native.query_by_label(CLOSE_LABEL).is_none());
}

#[test]
fn test_title_bar_captions_follow_the_desktop_layout() {
    let layout = Layout {
        left: vec![Button::Close],
        right: Vec::new(),
    };
    let (_runtime, harness) = harness("document-light", integrated(layout));
    let close = harness.get_by_label(CLOSE_LABEL).rect();
    assert!(close.left() < 100.0, "{close:?}");
    assert!(harness.query_by_label(MINIMIZE_LABEL).is_none());
}

#[test]
fn test_title_bar_long_title_keeps_the_buttons_in_the_window() {
    let runtime = tokio::runtime::Runtime::new().expect("tokio runtime");
    let mut app = {
        let _enter = runtime.enter();
        support::make_app()
    };
    app.themes = ThemeCatalog::builtin();
    // No session is focused, so the title is the project folder's name.
    app.cwd_path = std::path::PathBuf::from("/projects").join("a".repeat(300));
    let width = 600.0;
    let shell = Shell::new(app).with_title_bar(integrated(Layout::default()));
    let mut harness = Harness::builder()
        .with_size(egui::vec2(width, 600.0))
        .allow_missing_glyphs()
        .build_ui_state(|ui, shell: &mut Shell| shell.update(ui), shell);
    harness.run();
    // The caption buttons, and each quick access button (the ribbon has
    // buttons of the same names, which must fit as well).
    for label in [
        CLOSE_LABEL,
        MAXIMIZE_LABEL,
        MINIMIZE_LABEL,
        "New session",
        "Stop",
    ] {
        for node in harness.query_all_by_label(label) {
            let rect = node.rect();
            assert!(
                rect.right() <= width + 0.5 && rect.left() >= 0.0,
                "{label} at {rect:?} is outside the {width} pt window"
            );
        }
    }
}
