//! Tests for `codemason_gui::material::list::avatar_list_item`: its height,
//! accessible name and selected state, the trailing slot's clicks, and that
//! a click on its meta line (chips, badges) selects the item.

#[path = "support_material.rs"]
mod support_material;

use codemason_gui::material::chip::{ChipKind, chip_dense};
use codemason_gui::material::list::{AVATAR_ITEM_HEIGHT, AvatarItem, avatar_list_item};
use egui::accesskit::Role;
use egui_kittest::kittest::{NodeT, Queryable};

/// What the harness records.
#[derive(Default)]
struct Clicks {
    item: usize,
    trailing: usize,
}

/// A harness with a selected and a plain item; the plain one has a chip
/// in its meta line and a trailing button.
fn harness() -> egui_kittest::Harness<'static, Clicks> {
    let palette = support_material::palette();
    let mut harness = support_material::builder()
        .with_size(egui::vec2(600.0, 200.0))
        .build_ui_state(
            move |ui, clicks: &mut Clicks| {
                let selected = AvatarItem {
                    headline: "alpha",
                    subtitle: "1.0.0 - acme",
                    supporting: "The first entry",
                    selected: true,
                };
                avatar_list_item(ui, &palette, &selected, |_| {}, |_| ());
                let plain = AvatarItem {
                    headline: "bravo",
                    subtitle: "2.0.0 - acme",
                    supporting: "The second entry",
                    selected: false,
                };
                let (response, pressed) = avatar_list_item(
                    ui,
                    &palette,
                    &plain,
                    |ui| {
                        chip_dense(ui, &palette, ChipKind::Assist, None, "tag-one");
                    },
                    |ui| ui.button("Act").clicked(),
                );
                if response.clicked() {
                    clicks.item += 1;
                }
                if pressed {
                    clicks.trailing += 1;
                }
            },
            Clicks::default(),
        );
    harness.run();
    harness
}

#[test]
fn test_material_avatar_item_height_and_selected_state() {
    let harness = harness();
    let selected = harness.get_by_role_and_label(Role::ListItem, "alpha");
    assert!((selected.rect().height() - AVATAR_ITEM_HEIGHT).abs() < 0.5);
    assert_eq!(selected.accesskit_node().is_selected(), Some(true));
    let plain = harness.get_by_role_and_label(Role::ListItem, "bravo");
    assert_ne!(plain.accesskit_node().is_selected(), Some(true));
}

#[test]
fn test_material_avatar_item_trailing_click_is_not_an_item_click() {
    let mut harness = harness();
    harness.get_by_label("Act").click();
    harness.run();
    assert_eq!(harness.state().trailing, 1);
    assert_eq!(harness.state().item, 0);
}

#[test]
fn test_material_avatar_item_meta_click_selects_the_item() {
    let mut harness = harness();
    harness.get_by_label("tag-one").click();
    harness.run();
    assert_eq!(
        harness.state().item,
        1,
        "a click on a chip is an item click"
    );
    harness
        .get_by_role_and_label(Role::ListItem, "bravo")
        .click();
    harness.run();
    assert_eq!(harness.state().item, 2);
}

#[test]
fn test_material_avatar_item_meta_line_fits_inside_the_item() {
    let harness = harness();
    let item = harness
        .get_by_role_and_label(Role::ListItem, "bravo")
        .rect();
    let chip = harness.get_by_label("tag-one").rect();
    // The headline, the supporting line and a dense chip all fit in the
    // item's text area (its height less the top and bottom insets).
    let inset = codemason_gui::metrics::space::XS;
    assert!(
        chip.top() >= item.top() + inset && chip.bottom() <= item.bottom() - inset + 0.5,
        "chip {chip:?} is clipped in item {item:?}"
    );
}
