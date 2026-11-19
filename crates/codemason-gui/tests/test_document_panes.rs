//! Tests for the word-processor layout's panes and status bar: the drawer
//! style of the sessions sidebar, and the status bar's zoom slider, drawn
//! only for themes with their own Material roles and bound to the window's
//! zoom factor.

#[path = "support/mod.rs"]
mod support;

use codemason_app_core::theme_tokens::{ThemeCatalog, ThemeSelection};
use codemason_gui::Shell;
use codemason_gui::material::sheet::{ITEM_HEIGHT, drawer_style};
use codemason_gui::palette::{Palette, rgb};
use codemason_gui::panels::status_bar::ZOOM_LABEL;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;

/// A window in theme `theme`.
fn harness(theme: &str) -> (tokio::runtime::Runtime, Harness<'static, Shell>) {
    let runtime = tokio::runtime::Runtime::new().expect("tokio runtime");
    let mut app = {
        let _enter = runtime.enter();
        support::make_app()
    };
    app.themes = ThemeCatalog::builtin();
    app.theme = ThemeSelection::Named(theme.to_string());
    let mut harness = Harness::builder()
        .with_size(egui::vec2(1200.0, 800.0))
        .allow_missing_glyphs()
        .build_ui_state(|ui, shell: &mut Shell| shell.update(ui), Shell::new(app));
    harness.run();
    (runtime, harness)
}

#[test]
fn test_document_panes_drawer_style_makes_pill_rows() {
    let tokens = ThemeCatalog::builtin()
        .get("document-light")
        .expect("built-in");
    let palette = Palette::from_tokens(&tokens);
    let mut style = egui::Style::default();
    drawer_style(&mut style, &palette);
    assert_eq!(
        style.visuals.selection.bg_fill,
        rgb(palette.m3.secondary_container)
    );
    assert_eq!(
        style.visuals.widgets.inactive.corner_radius,
        egui::CornerRadius::same((ITEM_HEIGHT / 2.0) as u8)
    );
}

#[test]
fn test_document_panes_zoom_slider_only_in_document_themes() {
    let (_runtime, legacy) = harness("codemason-light");
    assert!(legacy.query_by_label(ZOOM_LABEL).is_none());
    let (_runtime, mut document) = harness("document-light");
    // A click in the middle of 50%..200% zooms to 125%.
    document.get_by_label(ZOOM_LABEL).click();
    document.run();
    let zoom = document.ctx.zoom_factor();
    assert!((zoom - 1.25).abs() < 0.02, "{zoom}");
}

#[test]
fn test_document_panes_motion_follows_the_animation_setting() {
    use codemason_gui::material::{MOTION_DURATION, motion_time, standard_easing};

    let ctx = egui::Context::default();
    assert!((motion_time(&ctx) - MOTION_DURATION).abs() < f32::EPSILON);
    ctx.all_styles_mut(|style| style.animation_time = 0.0);
    assert!(motion_time(&ctx).abs() < f32::EPSILON);
    assert!(standard_easing(0.0).abs() < f32::EPSILON);
    assert!((standard_easing(1.0) - 1.0).abs() < f32::EPSILON);
    // Decelerating: more than half way at half the time.
    assert!(standard_easing(0.5) > 0.5);
}

#[test]
fn test_document_panes_zoom_slider_keeps_its_arrow_keys() {
    let (_runtime, mut harness) = harness("document-light");
    harness.get_by_label(ZOOM_LABEL).focus();
    harness.run();
    harness.key_press(egui::Key::ArrowRight);
    harness.run();
    // One step of 1.5 / 100 from 100%, rounded to whole percent.
    let zoom = harness.ctx.zoom_factor();
    assert!((zoom - 1.02).abs() < 0.011, "{zoom}");
    assert!(harness.get_by_label(ZOOM_LABEL).is_focused());
}
