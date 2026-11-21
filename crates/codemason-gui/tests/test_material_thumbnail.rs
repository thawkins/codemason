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
fn picture_click_point(
    harness: &egui_kittest::Harness<'_, impl core::fmt::Debug>,
    label: &str,
) -> egui::Pos2 {
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
    let uri = codemason_gui::image_load::file_uri(&path);
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

/// A loader that reports every image as still loading, whatever its URI.
///
/// The real loaders answer from a cache in the same frame, so a picture is
/// never caught mid-load; this one is, which is how the in-flight state is
/// tested without racing a decoder.
#[derive(Debug)]
struct PendingLoader;

impl egui::load::ImageLoader for PendingLoader {
    fn id(&self) -> &'static str {
        "codemason_gui::tests::PendingLoader"
    }

    fn load(
        &self,
        _ctx: &egui::Context,
        _uri: &str,
        _size_hint: egui::load::SizeHint,
    ) -> egui::load::ImageLoadResult {
        Ok(egui::load::ImagePoll::Pending { size: None })
    }

    fn forget(&self, _uri: &str) {}

    fn forget_all(&self) {}

    fn byte_size(&self) -> usize {
        0
    }
}

/// A builder whose context reports every image as still loading.
fn pending_builder<State>() -> egui_kittest::HarnessBuilder<State> {
    support_material::builder().with_context_setup(|ctx| {
        ctx.add_image_loader(std::sync::Arc::new(PendingLoader));
    })
}

#[test]
fn test_material_thumbnail_describes_its_picture_on_hover() {
    let dir = scratch("hover");
    let path = dir.join("shot.png");
    write_png(&path);
    let uri = codemason_gui::image_load::file_uri(&path);
    let palette = support_material::palette();
    let mut harness = support_material::builder()
        // The picture decodes on a background thread, which repaints until it
        // is done; four steps are not always enough for the file to arrive.
        .with_max_steps(64)
        .build_ui(move |ui| {
            thumbnail(ui, &palette, &uri, "shot.png", SIZE);
        });
    egui_extras::install_image_loaders(&harness.ctx);
    // The label is promised as the picture's hover text, so the tooltip is
    // the label itself: `on_hover_text` opens an area whose only text is it.
    harness.run();
    let pos = picture_click_point(&harness, "shot.png");
    harness
        .input_mut()
        .events
        .push(egui::Event::PointerMoved(pos));
    harness.run();
    let count = harness.get_all_by_label("shot.png").count();
    assert_eq!(
        count, 2,
        "the picture and the tooltip that names it, and nothing else"
    );
}

#[test]
fn test_material_thumbnail_shows_a_picture_that_is_still_loading() {
    let palette = support_material::palette();
    let mut harness = pending_builder().build_ui_state(
        move |ui, clicks: &mut u32| {
            if thumbnail(ui, &palette, "file:///pending/shot.png", "shot.png", SIZE)
                .picture
                .double_clicked()
            {
                *clicks += 1;
            }
        },
        0,
    );
    // `step`, not `run`: a thumbnail still on its way asks for the next frame,
    // so the harness never settles by design.
    harness.step();
    let pos = picture_click_point(&harness, "shot.png");
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
    // There is no picture yet, so there is nothing to enlarge: an image still
    // on its way must not open an empty viewer.
    assert_eq!(*harness.state(), 0, "a loading thumbnail enlarges nothing");
}

/// A loader that records every URI it is asked for and loads none of them.
///
/// The real loaders answer from a cache in the same frame, so asking one
/// after the fact cannot tell whether the widget asked first; this one says
/// so directly.
#[derive(Debug)]
struct SpyLoader {
    asked: std::sync::Mutex<Vec<String>>,
}

impl egui::load::ImageLoader for SpyLoader {
    fn id(&self) -> &'static str {
        "codemason_gui::tests::SpyLoader"
    }

    fn load(
        &self,
        _ctx: &egui::Context,
        uri: &str,
        _size_hint: egui::load::SizeHint,
    ) -> egui::load::ImageLoadResult {
        if let Ok(mut asked) = self.asked.lock() {
            asked.push(uri.to_owned());
        }
        Ok(egui::load::ImagePoll::Pending { size: None })
    }

    fn forget(&self, _uri: &str) {}

    fn forget_all(&self) {}

    fn byte_size(&self) -> usize {
        0
    }
}

#[test]
fn test_material_thumbnail_does_not_load_an_image_it_cannot_show() {
    let dir = scratch("culled");
    let on_screen = dir.join("on.png");
    let off_screen = dir.join("off.png");
    write_png(&on_screen);
    write_png(&off_screen);
    let on_uri = codemason_gui::image_load::file_uri(&on_screen);
    let off_uri = codemason_gui::image_load::file_uri(&off_screen);
    let spy = std::sync::Arc::new(SpyLoader {
        asked: std::sync::Mutex::new(Vec::new()),
    });
    let asked = std::sync::Arc::clone(&spy);
    let (shown, hidden) = (on_uri.clone(), off_uri.clone());
    let palette = support_material::palette();
    // One on-screen thumbnail; the second is laid out far below the viewport,
    // as a long strip is once it is scrolled.
    let mut harness = support_material::builder()
        .with_size(egui::vec2(320.0, 96.0))
        .with_context_setup(move |ctx| {
            ctx.add_image_loader(spy);
        })
        .build_ui(move |ui| {
            thumbnail(ui, &palette, &shown, "on.png", SIZE);
            ui.add_space(4000.0);
            thumbnail(ui, &palette, &hidden, "off.png", SIZE);
        });
    harness.step();
    // Both boxes are laid out, so the strip does not reflow when the second
    // one scrolls into view...
    harness.get_by_label("off.png");
    let asked = asked.asked.lock().expect("spy").clone();
    // ...but only the visible one was handed to a loader: an off-screen
    // thumbnail starts no decode and stats no file.
    assert!(
        asked.contains(&on_uri),
        "a thumbnail in sight loads its picture, asked for {asked:?}"
    );
    assert!(
        !asked.contains(&off_uri),
        "a thumbnail out of sight starts no decode, asked for {asked:?}"
    );
}
