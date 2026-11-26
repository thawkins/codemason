//! Tests for `codemason_gui::material::menu`: item size, clicks, the menu
//! surface's colours, the three item states a picker needs (a trailing
//! shortcut, a checkmark, and a disabled row), the selected row a keyboard
//! menu puts on its cursor, and the which-key popup built from the same
//! surface.

#[path = "support_material.rs"]
mod support_material;

use codemason_gui::icons::Icon;
use codemason_gui::material::menu::{ITEM_HEIGHT, menu_frame, menu_item, menu_item_selected};
use codemason_gui::palette::rgb;
use egui::accesskit::Role;
use egui_kittest::kittest::{NodeT, Queryable};

#[test]
fn test_material_menu_item_is_48_points_high_and_clickable() {
    let palette = support_material::palette();
    let mut harness = support_material::builder().build_ui_state(
        move |ui, chosen: &mut Option<&'static str>| {
            menu_frame(&palette).show(ui, |ui| {
                if menu_item(ui, &palette, Some(Icon::Copy), "Copy", Some("Ctrl+C")).clicked() {
                    *chosen = Some("Copy");
                }
                if menu_item(ui, &palette, None, "Find", None).clicked() {
                    *chosen = Some("Find");
                }
            });
        },
        None,
    );
    let rect = harness.get_by_label("Copy").rect();
    assert!((rect.height() - ITEM_HEIGHT).abs() < 0.5);
    assert!(rect.width() >= 112.0);
    harness.get_by_label("Find").click();
    harness.run();
    assert_eq!(*harness.state(), Some("Find"));
}

#[test]
fn test_material_menu_item_keeps_its_trailing_shortcut_out_of_its_name() {
    let palette = support_material::palette();
    let mut harness = support_material::builder()
        .with_size(egui::vec2(320.0, 160.0))
        .build_ui_state(
            move |ui, chosen: &mut Option<&'static str>| {
                menu_frame(&palette).show(ui, |ui| {
                    if menu_item(ui, &palette, None, "Copy", Some("Ctrl+C")).clicked() {
                        *chosen = Some("Copy");
                    }
                    menu_item(ui, &palette, None, "Paste", None);
                });
            },
            None,
        );
    // The shortcut is painted at the trailing edge, so it is neither part of
    // the accessible name nor a widget of its own.
    harness.get_by_role_and_label(Role::MenuItem, "Copy");
    assert!(
        harness.query_by_label("Ctrl+C").is_none(),
        "painted, not added"
    );
    harness
        .get_by_role_and_label(Role::MenuItem, "Copy")
        .click();
    harness.run();
    assert_eq!(*harness.state(), Some("Copy"));
}

#[test]
fn test_material_menu_item_carries_a_checkmark_as_its_leading_icon() {
    let palette = support_material::palette();
    let mut harness = support_material::builder().build_ui_state(
        move |ui, chosen: &mut Option<&'static str>| {
            menu_frame(&palette).show(ui, |ui| {
                if menu_item(ui, &palette, Some(Icon::Check), "Dark", None).clicked() {
                    *chosen = Some("Dark");
                }
                menu_item(ui, &palette, None, "Light", None);
            });
        },
        None,
    );
    harness
        .get_by_role_and_label(Role::MenuItem, "Dark")
        .click();
    harness.run();
    assert_eq!(*harness.state(), Some("Dark"));
}

#[test]
fn test_material_menu_item_is_disabled_inside_a_disabled_ui() {
    let palette = support_material::palette();
    let mut harness = support_material::builder().build_ui_state(
        move |ui, clicks: &mut u32| {
            menu_frame(&palette).show(ui, |ui| {
                ui.add_enabled_ui(false, |ui| {
                    if menu_item(ui, &palette, None, "Remove", None).clicked() {
                        *clicks += 1;
                    }
                });
                menu_item(ui, &palette, None, "Rename", None);
            });
        },
        0,
    );
    let row = harness.get_by_role_and_label(Role::MenuItem, "Remove");
    assert!(row.accesskit_node().is_disabled(), "no selection to remove");
    assert!(
        !harness
            .get_by_role_and_label(Role::MenuItem, "Rename")
            .accesskit_node()
            .is_disabled(),
        "the other row stays live"
    );
    harness
        .get_by_role_and_label(Role::MenuItem, "Remove")
        .click();
    harness.run();
    assert_eq!(*harness.state(), 0);
}

#[test]
fn test_material_menu_frame_is_a_surface_container_with_a_shadow() {
    let palette = support_material::palette();
    let frame = menu_frame(&palette);
    assert_eq!(frame.fill, rgb(palette.m3.surface_container));
    assert_ne!(frame.shadow, egui::Shadow::NONE);
}

#[test]
fn test_material_menu_item_selected_is_the_menus_current_row() {
    let palette = support_material::palette();
    let mut harness = support_material::builder().build_ui_state(
        move |ui, chosen: &mut Option<&'static str>| {
            menu_frame(&palette).show(ui, |ui| {
                if menu_item_selected(ui, &palette, Some(Icon::Copy), "Copy", Some("Ctrl+C"))
                    .clicked()
                {
                    *chosen = Some("Copy");
                }
                menu_item(ui, &palette, None, "Find", None);
            });
        },
        None,
    );
    // The current row states it to assistive technology (the keyboard's row
    // is the one a press runs), and a plain item does not.
    let on_cursor = harness.get_by_role_and_label(Role::MenuItem, "Copy");
    assert_eq!(on_cursor.accesskit_node().is_selected(), Some(true));
    assert_eq!(
        on_cursor.accesskit_node().description().as_deref(),
        Some("Ctrl+C"),
        "the shortcut is still the row's description"
    );
    assert_eq!(
        harness
            .get_by_role_and_label(Role::MenuItem, "Find")
            .accesskit_node()
            .is_selected(),
        Some(false)
    );
    // A selected row is still an item: it can be chosen.
    harness
        .get_by_role_and_label(Role::MenuItem, "Copy")
        .click();
    harness.run();
    assert_eq!(*harness.state(), Some("Copy"));
}

#[test]
fn test_which_key_popup_is_a_menu_surface_of_items_keyed_at_the_trailing_edge() {
    use codemason_gui::dialogs::which_key::{WHICH_KEY_TITLE, show};
    let palette = support_material::palette();
    let entries = || {
        vec![
            ('m', "Choose Model".to_string()),
            ('t', "Show Tasks".to_string()),
        ]
    };
    let harness = support_material::builder()
        .with_size(egui::vec2(600.0, 400.0))
        .build_ui(move |ui| show(ui.ctx(), &palette, &entries()));
    // The popup is a Material menu surface with its title as the dialog's
    // accessible name, and one item per binding: the action is the row and
    // its key is the trailing text, so the keys column up and a long action
    // name gets the room.
    harness.get_by_role_and_label(Role::Dialog, WHICH_KEY_TITLE);
    for (key, title) in [("m", "Choose Model"), ("t", "Show Tasks")] {
        let row = harness.get_by_role_and_label(Role::MenuItem, title);
        assert_eq!(
            row.accesskit_node().description().as_deref(),
            Some(key),
            "{title}"
        );
    }
    assert!(
        harness.query_by_label("m").is_none(),
        "the key is painted, not a widget of its own"
    );
}
