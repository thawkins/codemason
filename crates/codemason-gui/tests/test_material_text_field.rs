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

#[test]
fn test_material_text_field_outlined_value_shows_its_value_read_only() {
    let palette = support_material::palette();
    let harness = support_material::builder()
        .with_size(egui::vec2(400.0, 120.0))
        .build_ui(move |ui| {
            let field = codemason_gui::material::text_field::outlined_value(
                ui,
                &palette,
                "API key",
                "sk-1****cdef",
                true,
                None,
                false,
            );
            assert!((field.rect.height() - HEIGHT).abs() < 0.5);
        });
    let field = harness.get_by_label("API key");
    assert_eq!(field.value().as_deref(), Some("sk-1****cdef"));
}

#[test]
fn test_material_text_field_value_caret_follows_text_and_stays_inside() {
    use codemason_gui::material::text_field::{CARET_WIDTH, value_caret};
    let input = egui::Rect::from_min_size(egui::pos2(16.0, 20.0), egui::vec2(200.0, 24.0));
    let empty = value_caret(input, 0.0);
    assert!(
        (empty.left() - 17.0).abs() < 0.01,
        "an empty field's caret starts the text"
    );
    assert!((empty.width() - CARET_WIDTH).abs() < 0.01);
    assert!(input.y_range().contains(empty.center().y));
    let after = value_caret(input, 80.0);
    assert!(
        (after.left() - 97.0).abs() < 0.01,
        "the caret follows the text"
    );
    let overflow = value_caret(input, 900.0);
    assert!(
        overflow.right() <= input.right() + 0.01,
        "the caret stays in the field"
    );
}
