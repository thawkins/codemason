//! Tests for `codemason_gui::image_load`: an SVG is rasterised at its own
//! size capped at `MAX_DECODE_SIDE`, and a changed image file is read again.

use codemason_gui::image_load::{MAX_DECODE_SIDE, capped, decode_hint, is_svg, refresh_if_changed};
use egui::load::SizeHint;

/// A scratch folder of `name` under the git-ignored `target/temp`.
fn scratch(name: &str) -> std::path::PathBuf {
    let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/temp/image_load")
        .join(format!("{name}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("scratch dir");
    dir
}

#[test]
fn test_image_load_capped_keeps_shape_and_never_enlarges() {
    assert_eq!(capped(300.0, 120.0), (300, 120));
    assert_eq!(
        capped(20_000.0, 10_000.0),
        (MAX_DECODE_SIDE, MAX_DECODE_SIDE / 2)
    );
    assert!(is_svg("file:///a/B.SVG"));
    assert!(!is_svg("file:///a/b.png"));
}

#[test]
fn test_image_load_huge_svg_is_hinted_at_the_cap() {
    let dir = scratch("huge");
    let path = dir.join("huge.svg");
    std::fs::write(
        &path,
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="20000" height="20000"><rect width="10" height="10"/></svg>"#,
    )
    .expect("write svg");
    let ctx = egui::Context::default();
    egui_extras::install_image_loaders(&ctx);
    let uri = format!("file://{}", path.display());
    let mut hint = None;
    // The file loader reads on a thread: poll until the bytes are in.
    for _ in 0..200 {
        hint = decode_hint(&ctx, &uri);
        if hint.is_some() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert_eq!(
        hint,
        Some(SizeHint::Size {
            width: MAX_DECODE_SIDE,
            height: MAX_DECODE_SIDE,
            maintain_aspect_ratio: true,
        })
    );
    assert_eq!(
        decode_hint(&ctx, "file:///x/photo.png"),
        Some(SizeHint::default())
    );
}

#[test]
fn test_image_load_changed_file_is_forgotten() {
    let dir = scratch("changed");
    let path = dir.join("chart.svg");
    let write = |side: u32| {
        std::fs::write(
            &path,
            format!(
                r#"<svg xmlns="http://www.w3.org/2000/svg" width="{side}" height="{side}"></svg>"#
            ),
        )
        .expect("write svg");
    };
    write(100);
    let ctx = egui::Context::default();
    egui_extras::install_image_loaders(&ctx);
    let uri = format!("file://{}", path.display());
    let hint_of = |ctx: &egui::Context| {
        for _ in 0..200 {
            refresh_if_changed(ctx, &uri);
            if let Some(hint) = decode_hint(ctx, &uri) {
                return hint;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        panic!("never loaded");
    };
    let first = hint_of(&ctx);
    // A different size and length: the stamp changes even within one
    // second of mtime resolution.
    std::thread::sleep(std::time::Duration::from_millis(20));
    write(250);
    let second = hint_of(&ctx);
    assert_ne!(first, second, "the new file is read");
    assert_eq!(
        second,
        SizeHint::Size {
            width: 250,
            height: 250,
            maintain_aspect_ratio: true,
        }
    );
}
