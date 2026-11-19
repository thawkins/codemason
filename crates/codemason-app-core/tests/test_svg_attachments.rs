//! SVG attachments: an SVG is rasterised to a PNG (providers take raster
//! images only) and the PNG is attached; an invalid SVG is not attached.
//! Every file goes under the git-ignored `target/temp/`.

#[path = "support/mod.rs"]
mod support;

use std::path::PathBuf;

use codemason_app_core::clipboard::{
    SVG_ATTACHMENT_MAX_SIDE, set_paste_dir_override, svg_to_png_temp,
};

/// A fresh scratch folder for this test, which is also where converted
/// PNGs are written while the returned guard lives.
struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/temp/svg-attachments")
            .join(format!("{name}-{}", std::process::id()));
        if dir.exists() {
            std::fs::remove_dir_all(&dir).expect("clear scratch");
        }
        std::fs::create_dir_all(&dir).expect("create scratch");
        set_paste_dir_override(Some(dir.clone()));
        Self(dir)
    }

    fn svg(&self, name: &str, body: &str) -> PathBuf {
        let path = self.0.join(name);
        std::fs::write(&path, body).expect("write svg");
        path
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        set_paste_dir_override(None);
    }
}

const BLUE_BOX: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="150" height="60"><rect width="150" height="60" fill="#3060c0"/></svg>"##;

#[test]
fn test_svg_attachments_rasterise_at_twice_their_size() {
    let scratch = Scratch::new("twice");
    let png = svg_to_png_temp(&scratch.svg("box.svg", BLUE_BOX)).expect("converted");
    assert!(png.starts_with(&scratch.0), "{png:?} is in the scratch dir");
    assert_eq!(png.extension().and_then(|e| e.to_str()), Some("png"));
    let image = image::open(&png).expect("a PNG").to_rgba8();
    assert_eq!(image.dimensions(), (300, 120));
    assert_eq!(image.get_pixel(10, 10).0, [0x30, 0x60, 0xc0, 0xff]);
}

#[test]
fn test_svg_attachments_large_svg_is_capped() {
    let scratch = Scratch::new("capped");
    let big = r#"<svg xmlns="http://www.w3.org/2000/svg" width="4000" height="1000"><rect width="4000" height="1000"/></svg>"#;
    let png = svg_to_png_temp(&scratch.svg("big.svg", big)).expect("converted");
    let image = image::open(&png).expect("a PNG");
    assert_eq!(image.width(), SVG_ATTACHMENT_MAX_SIDE);
    assert_eq!(image.height(), SVG_ATTACHMENT_MAX_SIDE / 4);
}

#[tokio::test]
async fn test_svg_attachments_attach_the_png() {
    let scratch = Scratch::new("attach");
    let svg = scratch.svg("box.svg", BLUE_BOX);
    let mut app = support::make_app();
    assert!(app.attach_image_path(&svg));
    let attached = app.session.pending_attachments.last().expect("attached");
    assert_eq!(attached.extension().and_then(|e| e.to_str()), Some("png"));
    assert!(attached.exists());
}

#[tokio::test]
async fn test_svg_attachments_invalid_svg_is_not_attached() {
    let scratch = Scratch::new("invalid");
    let svg = scratch.svg("broken.svg", "this is not svg");
    let mut app = support::make_app();
    app.attach_image_path(&svg);
    assert_eq!(app.session.pending_attachments.len(), 0, "nothing attached");
    assert!(svg_to_png_temp(&svg).is_err());
}
