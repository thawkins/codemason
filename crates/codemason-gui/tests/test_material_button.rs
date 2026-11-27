//! Tests for `codemason_gui::material::button`: size, click reporting,
//! the accessible name, and that a disabled button reports no clicks.

#[path = "support_material.rs"]
mod support_material;

use codemason_gui::material::button::{ButtonKind, HEIGHT, button};
use egui_kittest::kittest::{NodeT, Queryable};

#[test]
fn test_material_button_every_kind_is_40_points_high_and_clickable() {
    for kind in [
        ButtonKind::Filled,
        ButtonKind::Tonal,
        ButtonKind::Outlined,
        ButtonKind::Text,
    ] {
        let palette = support_material::palette();
        let mut harness = support_material::builder().build_ui_state(
            move |ui, clicks: &mut u32| {
                if button(ui, &palette, kind, None, "Send").clicked() {
                    *clicks += 1;
                }
            },
            0,
        );
        let node = harness.get_by_label("Send");
        assert!((node.rect().height() - HEIGHT).abs() < 0.5, "{kind:?}");
        node.click();
        harness.run();
        assert_eq!(*harness.state(), 1, "{kind:?}");
    }
}

#[test]
fn test_material_button_icon_widens_the_button() {
    let palette = support_material::palette();
    let mut widths = Vec::new();
    for icon in [None, Some(codemason_gui::icons::Icon::Check)] {
        let harness = support_material::builder().build_ui(move |ui| {
            button(ui, &palette, ButtonKind::Filled, icon, "Send");
        });
        widths.push(harness.get_by_label("Send").rect().width());
    }
    assert!(widths[1] > widths[0], "{widths:?}");
}

#[test]
fn test_material_button_disabled_reports_no_click() {
    let palette = support_material::palette();
    let mut harness = support_material::builder().build_ui_state(
        move |ui, clicks: &mut u32| {
            ui.add_enabled_ui(false, |ui| {
                if button(ui, &palette, ButtonKind::Filled, None, "Send").clicked() {
                    *clicks += 1;
                }
            });
        },
        0,
    );
    harness.get_by_label("Send").click();
    harness.run();
    assert_eq!(*harness.state(), 0);
}

/// UI-13: a small button is `SMALL_HEIGHT` high, reports a click, and
/// never takes the keyboard focus (it would share Enter with the prompt).
#[test]
fn test_material_small_button_is_small_click_only_and_clickable() {
    use codemason_gui::material::button::{SMALL_HEIGHT, small_button};
    let palette = support_material::palette();
    let mut harness = support_material::builder().build_ui_state(
        move |ui, clicks: &mut u32| {
            if small_button(ui, &palette, "Copy", None).clicked() {
                *clicks += 1;
            }
        },
        0,
    );
    let node = harness.get_by_label("Copy");
    assert!((node.rect().height() - SMALL_HEIGHT).abs() < 0.5);
    node.focus();
    harness.run();
    assert!(!harness.get_by_label("Copy").is_focused(), "click-only");
    harness.get_by_label("Copy").click();
    harness.run();
    assert_eq!(*harness.state(), 1);
}

/// A toggle reports its state to accessibility; a plain button does not.
#[test]
fn test_material_small_button_reports_a_toggle_state() {
    use codemason_gui::material::button::small_button;
    let palette = support_material::palette();
    let harness = support_material::builder().build_ui(move |ui| {
        small_button(ui, &palette, "Wrap", Some(true));
        small_button(ui, &palette, "Copy", None);
    });
    assert_eq!(
        harness.get_by_label("Wrap").accesskit_node().toggled(),
        Some(egui::accesskit::Toggled::True)
    );
    assert_eq!(
        harness.get_by_label("Copy").accesskit_node().toggled(),
        None
    );
}
