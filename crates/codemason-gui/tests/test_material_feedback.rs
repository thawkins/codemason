//! Tests for `codemason_gui::material::feedback` and the shared material
//! helpers: the slider follows clicks and arrow keys and stays in range, the
//! progress indicator's size, the snackbar's action, and elevation shadows.

#[path = "support_material.rs"]
mod support_material;

use codemason_gui::material::elevation_shadow;
use codemason_gui::material::feedback::{TRACK_HEIGHT, linear_progress, slider, snackbar};
use egui_kittest::kittest::Queryable;

#[test]
fn test_material_feedback_slider_follows_click_and_keys_within_range() {
    let palette = support_material::palette();
    let mut harness = support_material::builder()
        .with_size(egui::vec2(300.0, 80.0))
        .build_ui_state(
            move |ui, value: &mut f32| {
                slider(ui, &palette, value, 0.0..=100.0, "Zoom");
            },
            0.0,
        );
    // A click in the middle of the track moves the value to the middle.
    harness.get_by_label("Zoom").click();
    harness.run();
    let middle = *harness.state();
    assert!((middle - 50.0).abs() < 2.0, "{middle}");
    harness.get_by_label("Zoom").focus();
    harness.run();
    harness.key_press(egui::Key::ArrowRight);
    harness.run();
    assert!((*harness.state() - (middle + 1.0)).abs() < 0.01);
    for _ in 0..200 {
        harness.key_press(egui::Key::ArrowLeft);
    }
    harness.run();
    assert!(harness.state().abs() < f32::EPSILON);
}

#[test]
fn test_material_feedback_linear_progress_is_a_thin_track() {
    let palette = support_material::palette();
    let harness = support_material::builder()
        .with_size(egui::vec2(300.0, 40.0))
        .build_ui(move |ui| {
            let response = linear_progress(ui, &palette, Some(0.5));
            assert!((response.rect.height() - TRACK_HEIGHT).abs() < 0.5);
        });
    drop(harness);
}

#[test]
fn test_material_feedback_snackbar_reports_its_action() {
    let palette = support_material::palette();
    let mut harness = support_material::builder()
        .with_size(egui::vec2(400.0, 300.0))
        .build_ui_state(
            move |ui, undone: &mut bool| {
                let id = egui::Id::unique("test_snackbar");
                if snackbar(ui.ctx(), &palette, id, "Saved", Some("Undo")) {
                    *undone = true;
                }
            },
            false,
        );
    harness.get_by_label("Undo").click();
    harness.run();
    assert!(*harness.state());
}

#[test]
fn test_material_feedback_elevation_shadows_grow_with_level() {
    let palette = support_material::palette();
    assert_eq!(elevation_shadow(0, &palette), egui::Shadow::NONE);
    let blurs: Vec<u8> = (1..=3)
        .map(|level| elevation_shadow(level, &palette).blur)
        .collect();
    assert!(blurs.windows(2).all(|pair| pair[0] < pair[1]), "{blurs:?}");
}
