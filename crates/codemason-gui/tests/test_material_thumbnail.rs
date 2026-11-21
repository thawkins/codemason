//! Tests for `codemason_gui::material::thumbnail`: the box's size, the close
//! mark's hit target and accessible name, and the name drawn in place of an
//! image that cannot be loaded.

#[path = "support_material.rs"]
mod support_material;

use codemason_gui::material::thumbnail::{CLOSE_SIZE, REMOVE_LABEL, SIZE, thumbnail};
use egui_kittest::kittest::Queryable;

/// A scratch folder of `name` under the git-ignored `target/temp`.
fn scratch(name: &str) -> std::path::PathBuf {
    let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/temp/material_thumbnail")
        .join(format!("{name}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("scratch dir");
    dir
}

/// A 2x1 opaque red PNG, so a thumbnail has a real picture to load.
const PNG: &[u8] = &[
    0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0xF4, 0x22, 0x7F,
    0x8A, 0x00, 0x00, 0x00, 0x0E, 0x49, 0x44, 0x41, 0x54, 0x78, 0xDA, 0x63, 0x38, 0xA1, 0xA1, 0xF1,
    0x1F, 0x84, 0x01, 0x12, 0x33, 0x04, 0x2F, 0xB4, 0x17, 0x82, 0x1E, 0x00, 0x00, 0x00, 0x00, 0x49,
    0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
];

/// Write [`PNG`] at `path`, so the thumbnail has a picture to load.
fn write_png(path: &std::path::Path) {
    std::fs::write(path, PNG).expect("write png");
}

/// A point inside the picture of the thumbnail labelled `label`, clear of
/// its close mark: the mark covers the box's whole top-right quarter at
/// [`SIZE`], so a click at the box's centre would land on the mark.
fn picture_click_point(harness: &egui_kittest::Harness<'_, u32>, label: &str) -> egui::Pos2 {
    harness.get_by_label(label).rect().left_bottom() + egui::vec2(SIZE / 2.0, -SIZE / 4.0)
}

#[test]
fn test_material_thumbnail_is_a_square_box() {
    let palette = support_material::palette();
    let harness = support_material::builder().build_ui(move |ui| {
        thumbnail(
            ui,
            &palette,
            "file:///nonexistent/shot.png",
            "shot.png",
            SIZE,
        );
    });
    let rect = harness.get_by_label("shot.png").rect();
    assert!((rect.width() - SIZE).abs() < 0.5, "{rect:?}");
    assert!((rect.height() - SIZE).abs() < 0.5, "{rect:?}");
}

#[test]
fn test_material_thumbnail_close_mark_is_a_full_hit_target() {
    let palette = support_material::palette();
    let harness = support_material::builder().build_ui(move |ui| {
        thumbnail(
            ui,
            &palette,
            "file:///nonexistent/shot.png",
            "shot.png",
            SIZE,
        );
    });
    let rect = harness.get_by_label(REMOVE_LABEL).rect();
    assert!((rect.width() - CLOSE_SIZE).abs() < 0.5, "{rect:?}");
    assert!((rect.height() - CLOSE_SIZE).abs() < 0.5, "{rect:?}");
}

#[test]
fn test_material_thumbnail_close_mark_sits_in_the_top_right_corner() {
    let palette = support_material::palette();
    let harness = support_material::builder().build_ui(move |ui| {
        thumbnail(
            ui,
            &palette,
            "file:///nonexistent/shot.png",
            "shot.png",
            SIZE,
        );
    });
    let box_rect = harness.get_by_label("shot.png").rect();
    let close = harness.get_by_label(REMOVE_LABEL).rect();
    assert!(
        close.right() <= box_rect.right() + 0.5 && close.right() > box_rect.right() - 1.0,
        "the mark's right edge is the box's, {close:?} in {box_rect:?}"
    );
    assert!(
        close.top() < box_rect.center().y,
        "the mark is in the top half, {close:?} in {box_rect:?}"
    );
}

#[test]
fn test_material_thumbnail_reports_a_click_on_its_close_mark() {
    let palette = support_material::palette();
    let mut harness = support_material::builder().build_ui_state(
        move |ui, clicks: &mut u32| {
            if thumbnail(
                ui,
                &palette,
                "file:///nonexistent/shot.png",
                "shot.png",
                SIZE,
            )
            .close
            .clicked()
            {
                *clicks += 1;
            }
        },
        0,
    );
    harness.get_by_label(REMOVE_LABEL).click();
    harness.run();
    assert_eq!(*harness.state(), 1, "the close mark's click is reported");
}

#[test]
fn test_material_thumbnail_never_shrinks_below_its_close_mark() {
    let palette = support_material::palette();
    let harness = support_material::builder().build_ui(move |ui| {
        thumbnail(
            ui,
            &palette,
            "file:///nonexistent/shot.png",
            "shot.png",
            4.0,
        );
    });
    let rect = harness.get_by_label("shot.png").rect();
    assert!(
        rect.width() >= CLOSE_SIZE - 0.5,
        "a small request still fits the mark: {rect:?}"
    );
}

#[test]
fn test_material_thumbnail_names_an_image_it_cannot_load() {
    let palette = support_material::palette();
    let harness = support_material::builder().build_ui(move |ui| {
        thumbnail(
            ui,
            &palette,
            "file:///nonexistent/gone.png",
            "gone.png",
            SIZE,
        );
    });
    // The picture cannot be decoded, so the name stands in for it: the box
    // keeps its accessible name and there is still a way to remove it.
    harness.get_by_label("gone.png");
    harness.get_by_label(REMOVE_LABEL);
}

#[test]
fn test_material_thumbnail_does_not_take_the_keyboard() {
    let palette = support_material::palette();
    let mut harness = support_material::builder().build_ui(move |ui| {
        thumbnail(
            ui,
            &palette,
            "file:///nonexistent/shot.png",
            "shot.png",
            SIZE,
        );
    });
    harness.run();
    // A focusable picture would take Tab's focus from the prompt and keep
    // typed text from reaching it; a thumbnail answers the mouse only.
    harness.get_by_label("shot.png").focus();
    harness.run();
    assert!(
        !harness.get_by_label("shot.png").is_focused(),
        "the picture is not in the Tab order"
    );
}

#[test]
fn test_material_thumbnail_reports_a_double_click_on_its_picture() {
    let dir = scratch("double");
    let path = dir.join("shot.png");
    write_png(&path);
    let uri = format!("file://{}", path.display());
    let palette = support_material::palette();
    let mut harness = support_material::builder().build_ui_state(
        move |ui, clicks: &mut u32| {
            if thumbnail(ui, &palette, &uri, "shot.png", SIZE)
                .picture
                .double_clicked()
            {
                *clicks += 1;
            }
        },
        0,
    );
    // The shell installs these at startup (`main_thread`); the widget test
    // harness is a bare context, so a `file://` image would never load and
    // the picture would sense no click.
    egui_extras::install_image_loaders(&harness.ctx);
    harness.run();
    let pos = picture_click_point(&harness, "shot.png");
    // Each click is one frame (press and release), so the two clicks are one
    // frame apart, inside egui's double-click delay.
    for _ in 0..2 {
        for pressed in [true, false] {
            harness.input_mut().events.push(egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::NONE,
            });
        }
        harness.step();
    }
    harness.run();
    assert_eq!(
        *harness.state(),
        1,
        "the picture's double click is reported"
    );
}

#[test]
fn test_material_thumbnail_reports_no_double_click_on_an_unloadable_picture() {
    let palette = support_material::palette();
    let mut harness = support_material::builder().build_ui_state(
        move |ui, clicks: &mut u32| {
            if thumbnail(
                ui,
                &palette,
                "file:///nonexistent/gone.png",
                "gone.png",
                SIZE,
            )
            .picture
            .double_clicked()
            {
                *clicks += 1;
            }
        },
        0,
    );
    harness.run();
    let pos = picture_click_point(&harness, "gone.png");
    for _ in 0..2 {
        for pressed in [true, false] {
            harness.input_mut().events.push(egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::NONE,
            });
        }
        harness.step();
    }
    harness.run();
    // There is nothing to enlarge, so the name in the frame is inert.
    assert_eq!(*harness.state(), 0, "a broken thumbnail enlarges nothing");
}
