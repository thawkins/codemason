//! Tests for `codemason_gui::material::text_field`: the field's height,
//! typing into it, its accessible name, and when the label floats.

#[path = "support_material.rs"]
mod support_material;

use codemason_gui::material::text_field::{HEIGHT, label_floats, outlined_text_field};
use egui_kittest::kittest::Queryable;

#[test]
fn test_material_text_field_label_floats_when_focused_or_filled() {
    assert!(!label_floats(false, true));
    assert!(label_floats(true, true));
    assert!(label_floats(false, false));
    assert!(label_floats(true, false));
}

#[test]
fn test_material_text_field_takes_typed_text_and_keeps_its_height() {
    let palette = support_material::palette();
    let mut harness = support_material::builder()
        .with_size(egui::vec2(400.0, 160.0))
        .build_ui_state(
            move |ui, text: &mut String| {
                let top = ui.cursor().top();
                outlined_text_field(ui, &palette, "Session name", text, Some("Help"), false);
                // The field and its supporting text sit one under the other.
                assert!(ui.cursor().top() - top > HEIGHT);
            },
            String::new(),
        );
    let field = harness.get_by_label("Session name");
    field.focus();
    field.type_text("notes");
    harness.run();
    assert_eq!(harness.state(), "notes");
}

#[test]
fn test_material_text_field_padding_click_focuses_the_editor() {
    let palette = support_material::palette();
    let mut harness = support_material::builder()
        .with_size(egui::vec2(400.0, 160.0))
        .build_ui_state(
            move |ui, text: &mut String| {
                outlined_text_field(ui, &palette, "Session name", text, None, false);
            },
            String::new(),
        );
    let editor = harness.get_by_label("Session name").rect();
    // Inside the outline, left of and above the editor itself.
    let pos = editor.min - egui::vec2(8.0, 8.0);
    for pressed in [true, false] {
        harness.event(egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::default(),
        });
        harness.run();
    }
    assert!(harness.get_by_label("Session name").is_focused());
}
