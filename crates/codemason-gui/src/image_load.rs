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
//! - [`file_uri`] builds the `file://` URI of a path in the one form the
//!   loader reads back unambiguously on every platform, and [`uri_path`]
//!   reverses it.
//!
//! Dependencies: `egui`, and `resvg`'s `usvg` (the version egui_extras
//! already uses) to read an SVG's size.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use egui::Id;
use egui::load::{BytesPoll, SizeHint};

/// The largest side, in pixels, an SVG is rasterised to.
pub const MAX_DECODE_SIDE: u32 = 4096;

/// Whether `text` is a Windows drive path (`C:\shots\screen.png`).
///
/// Byte comparison rather than `char`s: a drive letter is ASCII, and the
/// separator is the third byte either way it is written.
fn is_drive_path(text: &str) -> bool {
    let bytes = text.as_bytes();
    bytes.len() >= 3
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
        && matches!(bytes[2], b'/' | b'\\')
}

/// The `file://` URI of the image file at `path`.
///
/// egui_extras' file loader reads a `file://` URI back as a path with only
/// minimal translation: on Windows, text after the scheme that does not begin
/// with `/` is taken for a UNC *host*, so `format!("file://{}", path.display())`
/// turns `C:\shots\screen.png` into `\\C:\shots\screen.png` and never finds the
/// file. A drive path is therefore written with the third slash the URI syntax
/// calls for (`file:///C:/shots/screen.png`); a plain absolute path already
/// begins with the separator (`file:///home/me/shot.png`), so it is written as
/// it stands and the loader reads it back unchanged.
///
/// The path is not percent-encoded, matching the loader's own fallback: it
/// tries the raw path before the decoded one, so a file literally named
/// `My%20Doc.png` still resolves.
#[must_use]
pub fn file_uri(path: &Path) -> String {
    let text = path.display().to_string();
    if is_drive_path(&text) {
        format!("file:///{text}")
    } else {
        format!("file://{text}")
    }
}

/// The file path a `file://` URI names, if it names one.
///
/// The inverse of [`file_uri`], and the rule the loader itself applies: the
/// slash that follows the scheme of a drive URI (`file:///C:/...`) is part of
/// the scheme's form and is dropped before the path is built, so a round trip
/// returns `path` unchanged. A URI naming anything else keeps its text as it
/// is. Percent escapes are not decoded, as [`file_uri`] does not write them.
#[must_use]
pub fn uri_path(uri: &str) -> Option<PathBuf> {
    let text = uri.strip_prefix("file://")?;
    let drive = text.strip_prefix('/').filter(|rest| is_drive_path(rest));
    Some(PathBuf::from(drive.unwrap_or(text)))
}

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
    let Some(path) = uri_path(uri) else {
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

/// What state an image's decode is in this frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PictureState {
    /// The image is decoded and can be drawn.
    Ready,
    /// Its bytes are still on their way; it can be drawn next frame.
    Pending,
    /// No loader can read it, so nothing will ever be drawn.
    Unavailable,
}

/// Ask for `uri` to be decoded, after re-reading its file when it changed, and
/// report what there is to draw this frame.
///
/// A caller must draw an image only in [`PictureState::Ready`]: building an
/// `egui::Image` from a URI that is still loading makes egui paint its own
/// failed-image mark when the decode then fails, over a picture that had only
/// not arrived yet. It must also not report a click for one, which would open
/// a viewer with nothing in it.
///
/// # Returns
///
/// The decode's state. [`PictureState::Pending`] asks for the next frame, which
/// is the frame that finishes it.
#[must_use]
pub fn picture_state(ctx: &egui::Context, uri: &str) -> PictureState {
    refresh_if_changed(ctx, uri);
    let Some(hint) = decode_hint(ctx, uri) else {
        ctx.request_repaint();
        return PictureState::Pending;
    };
    match ctx.try_load_image(uri, hint) {
        Ok(egui::load::ImagePoll::Ready { .. }) => PictureState::Ready,
        Ok(egui::load::ImagePoll::Pending { .. }) => {
            ctx.request_repaint();
            PictureState::Pending
        }
        Err(_) => PictureState::Unavailable,
    }
}
