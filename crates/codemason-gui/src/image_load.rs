//! Loading transcript images safely and freshly.
//!
//! egui's loaders cache an image by its URI and rasterise an SVG at the
//! size they are asked for. Two helpers keep that in check:
//!
//! - [`decode_hint`] asks for an SVG at its own size, capped at
//!   [`MAX_DECODE_SIDE`] pixels a side, so a huge (or hostile) SVG is never
//!   rasterised at full size; its own size is read once (`usvg`) and
//!   cached. Raster images keep the default hint (their decoders read the
//!   file at its own size).
//! - [`refresh_if_changed`] forgets a `file://` image whose file changed
//!   (or appeared) since it was loaded, so a regenerated `chart.png` shows
//!   the new chart instead of the cached one.
//!
//! Dependencies: `egui`, and `resvg`'s `usvg` (the version egui_extras
//! already uses) to read an SVG's size.

use std::path::Path;
use std::time::SystemTime;

use egui::Id;
use egui::load::{BytesPoll, SizeHint};

/// The largest side, in pixels, an SVG is rasterised to.
pub const MAX_DECODE_SIDE: u32 = 4096;

/// Whether `uri` names an SVG image.
#[must_use]
pub fn is_svg(uri: &str) -> bool {
    Path::new(uri)
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("svg"))
}

/// The id under which an SVG's own size is cached.
fn size_id(uri: &str) -> Id {
    Id::unique(("codemason_svg_size", uri))
}

/// `size` scaled down, keeping its shape, so neither side passes
/// [`MAX_DECODE_SIDE`]; at least one pixel each way.
#[must_use]
pub fn capped(width: f32, height: f32) -> (u32, u32) {
    let longest = width.max(height).max(1.0);
    let scale = (MAX_DECODE_SIDE as f32 / longest).min(1.0);
    (
        ((width * scale).round() as u32).max(1),
        ((height * scale).round() as u32).max(1),
    )
}

/// The size hint to load `uri` with.
///
/// # Returns
///
/// For an SVG, its own size capped at [`MAX_DECODE_SIDE`] (no aspect
/// change); `None` while its bytes are still loading, and the default hint
/// when they cannot be read as SVG (the loader then reports the error).
/// For any other image, the default hint.
#[must_use]
pub fn decode_hint(ctx: &egui::Context, uri: &str) -> Option<SizeHint> {
    if !is_svg(uri) {
        return Some(SizeHint::default());
    }
    if let Some((width, height)) = ctx.data(|d| d.get_temp::<(u32, u32)>(size_id(uri))) {
        return Some(SizeHint::Size {
            width,
            height,
            maintain_aspect_ratio: true,
        });
    }
    let bytes = match ctx.try_load_bytes(uri) {
        Ok(BytesPoll::Ready { bytes, .. }) => bytes,
        Ok(BytesPoll::Pending { .. }) => return None,
        Err(_) => return Some(SizeHint::default()),
    };
    let Ok(tree) = resvg::usvg::Tree::from_data(&bytes, &resvg::usvg::Options::default()) else {
        return Some(SizeHint::default());
    };
    let size = capped(tree.size().width(), tree.size().height());
    ctx.data_mut(|d| d.insert_temp(size_id(uri), size));
    Some(SizeHint::Size {
        width: size.0,
        height: size.1,
        maintain_aspect_ratio: true,
    })
}

/// Forget the `file://` image at `uri` when its file changed, appeared or
/// went since this was last called for it, so the next load reads the file
/// again. Other URIs are left alone.
pub fn refresh_if_changed(ctx: &egui::Context, uri: &str) {
    let Some(path) = uri.strip_prefix("file://") else {
        return;
    };
    let stamp: Option<(SystemTime, u64)> = std::fs::metadata(path)
        .ok()
        .and_then(|meta| Some((meta.modified().ok()?, meta.len())));
    let id = Id::unique(("codemason_image_stamp", uri));
    let last = ctx.data(|d| d.get_temp::<Option<(SystemTime, u64)>>(id));
    if last.is_some_and(|last| last != stamp) {
        ctx.forget_image(uri);
        ctx.data_mut(|d| d.remove::<(u32, u32)>(size_id(uri)));
    }
    if last != Some(stamp) {
        ctx.data_mut(|d| d.insert_temp(id, stamp));
    }
}
