//! Layout of frontend-neutral [`StyledLine`]s as egui text.
//!
//! [`styled_to_layout_job`] turns the styled lines the core produces (the
//! transcript, markdown) into one [`LayoutJob`] that egui wraps and paints.
//! Bold spans use the bold families of [`crate::fonts`] when they are
//! installed ([`Bold`]). [`drawable`] swaps the characters the window's
//! fonts still cannot draw for ASCII stand-ins.
//!
//! Dependencies: `egui` (text layout), `codemason-app-core` (styled text),
//! [`crate::palette`] (colours) and [`crate::fonts`] (the bold families).

use codemason_app_core::styled::{StyledLine, TextStyle};
use egui::text::{LayoutJob, TextFormat};
use egui::{Color32, FontFamily, FontId, Stroke};

use crate::fonts::{BOLD_FAMILY, bold_font};
use crate::palette::Palette;

/// Font size, in points, of transcript text (proportional and monospace
/// alike, so a code line is as tall as a prose line).
pub const FONT_SIZE: f32 = 14.0;

/// Opacity factor applied to `dim` text.
const DIM_FACTOR: f32 = 0.6;

/// How `bold` spans are drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Bold {
    /// In the bold families of [`crate::fonts`] (installed in the context).
    Face,
    /// At regular weight: the context has no bold faces (e.g. a test
    /// harness that did not install [`crate::fonts`]).
    Regular,
}

impl Bold {
    /// How `ctx` can draw bold text.
    ///
    /// # Arguments
    ///
    /// * `ctx` - the context the text is laid out in.
    ///
    /// # Returns
    ///
    /// [`Bold::Face`] when the context's fonts have the bold family, else
    /// [`Bold::Regular`].
    #[must_use]
    pub fn of(ctx: &egui::Context) -> Self {
        let family = FontFamily::Name(BOLD_FAMILY.into());
        if ctx.fonts(|fonts| fonts.definitions().families.contains_key(&family)) {
            Self::Face
        } else {
            Self::Regular
        }
    }
}

/// Build one layout job from `lines`.
///
/// Lines are joined with `\n`, so an empty line is an empty row. Each span
/// keeps its style: the foreground colour (or [`Palette::text`]), the
/// background, italics, underline in the text colour, a dimmed colour for
/// `dim`, and [`FontId::monospace`] for `monospace` (proportional
/// otherwise). `bold` uses the bold face of the span's family when `bold`
/// is [`Bold::Face`].
///
/// # Arguments
///
/// * `lines` - the styled lines to lay out.
/// * `palette` - resolves each [`codemason_app_core::styled::ThemeColor`].
/// * `wrap_width` - the width, in points, rows wrap at.
/// * `bold` - whether bold spans have a bold face ([`Bold::of`]).
///
/// # Returns
///
/// A job ready for `Painter::layout_job` or a label.
#[must_use]
pub fn styled_to_layout_job(
    lines: &[StyledLine],
    palette: &Palette,
    wrap_width: f32,
    bold: Bold,
) -> LayoutJob {
    let mut job = LayoutJob::default();
    job.wrap.max_width = wrap_width;
    let newline = TextFormat {
        font_id: FontId::proportional(FONT_SIZE),
        color: palette.text,
        ..TextFormat::default()
    };
    for (i, line) in lines.iter().enumerate() {
        if i > 0 {
            job.append("\n", 0.0, newline.clone());
        }
        for span in &line.spans {
            job.append(&span.text, 0.0, text_format(&span.style, palette, bold));
        }
    }
    job
}

/// The egui format of one span `style`.
fn text_format(style: &TextStyle, palette: &Palette, bold: Bold) -> TextFormat {
    let mut color = style.fg.map_or(palette.text, |c| palette.color(c));
    if style.dim {
        color = color.gamma_multiply(DIM_FACTOR);
    }
    let font_id = match (style.bold, bold, style.monospace) {
        (true, Bold::Face, monospace) => bold_font(FONT_SIZE, monospace),
        (false, _, true) | (true, Bold::Regular, true) => FontId::monospace(FONT_SIZE),
        (false, _, false) | (true, Bold::Regular, false) => FontId::proportional(FONT_SIZE),
    };
    TextFormat {
        font_id,
        color,
        background: style.bg.map_or(Color32::TRANSPARENT, |c| palette.color(c)),
        italics: style.italic,
        underline: if style.underline {
            Stroke::new(1.0, color)
        } else {
            Stroke::NONE
        },
        ..TextFormat::default()
    }
}

/// `text` with every character `font` has no glyph for replaced by an ASCII
/// stand-in, so the window never draws a box ("tofu") for it.
///
/// With [`crate::fonts`] installed the bundled faces draw app-core's
/// running diamond and success dot, block elements and box drawing, and
/// system fonts draw the rest. The status icons no bundled face has become
/// ASCII when no system font has them either: the error cross `x`, the info
/// sign `i` and the warning sign `!`; any other missing character becomes
/// `?`. Control characters are kept (egui lays them out itself).
///
/// # Arguments
///
/// * `ctx` - the context whose fonts draw the text.
/// * `font` - the font the text is drawn in.
/// * `text` - the text to draw.
///
/// # Returns
///
/// `text` itself when every character can be drawn, else the substituted
/// copy.
#[must_use]
pub fn drawable(ctx: &egui::Context, font: &FontId, text: &str) -> String {
    ctx.fonts_mut(|fonts| {
        if fonts.has_glyphs(font, text) {
            return text.to_string();
        }
        text.chars()
            .map(|c| {
                if c.is_control() || fonts.has_glyph(font, c) {
                    c
                } else {
                    ascii_stand_in(c)
                }
            })
            .collect()
    })
}

/// The ASCII character drawn in place of `c`.
fn ascii_stand_in(c: char) -> char {
    match c {
        // Status icons in no bundled face, only in some system fonts.
        // Error cross (`codemason_app_core::status::ICON_ERROR`).
        '\u{2717}' => 'x',
        // Information source.
        '\u{2139}' => 'i',
        // Warning sign.
        '\u{26A0}' => '!',
        _ => '?',
    }
}
