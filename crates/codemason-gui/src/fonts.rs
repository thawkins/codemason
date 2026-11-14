//! The window's font stack: symbol and block-glyph fallbacks, a bold face,
//! and installed system fonts for every other script.
//!
//! egui's default faces leave gaps the core's text falls into: the
//! proportional face (Ubuntu Light) has no box-drawing, block-element or
//! geometric-shape glyphs (the startup logo's U+2580..U+259F blocks, the
//! U+25C6 running diamond), no CJK, and there is no bold weight at all.
//! [`font_stack`] builds craft-ui's [`UiFonts`] stack with:
//!
//! - egui's default faces first in the proportional and monospace families;
//! - egui's bundled `Hack` face ([`SYMBOL_FACE`]) after them in the
//!   proportional family, for the whole U+2500..U+25FF range (box drawing,
//!   block elements, geometric shapes);
//! - two bold families, [`BOLD_FAMILY`] for proportional text and
//!   [`BOLD_MONO_FAMILY`] for code, led by the bundled Hack Bold face
//!   ([`BOLD_FACE`], `assets/fonts/Hack-Bold.ttf`, Hack v3.003, MIT and
//!   Bitstream Vera licences, see `assets/fonts/Hack-LICENSE.txt`);
//! - installed system fonts (craft-ui's `system_fonts` feature) for
//!   everything the bundled faces lack: CJK and other scripts, emoji, and
//!   the U+2717 ballot cross, U+2139 info and U+26A0 warning status icons. [`SystemFonts::Off`] leaves them out, so tests draw
//!   the same on every machine.
//!
//! [`install`] installs the stack in a context; the window does so once at
//! start with [`SystemFonts::AfterCraft`].
//!
//! Dependencies: `craft-ui` ([`UiFonts`], the system font index) and
//! `egui`.

use craft_ui::fonts::{Family, I18nScripts, Stack, SystemFonts, UiFonts};
use egui::{FontData, FontFamily, FontId};

/// The family bold proportional text is drawn in.
pub const BOLD_FAMILY: &str = "codemason-bold";

/// The family bold monospace text is drawn in.
pub const BOLD_MONO_FAMILY: &str = "codemason-bold-mono";

/// The egui font name of the bundled bold face.
pub const BOLD_FACE: &str = "Hack-Bold";

/// egui's bundled face that covers box drawing, block elements and
/// geometric shapes; the proportional family falls back to it.
pub const SYMBOL_FACE: &str = "Hack";

/// Hack Bold v3.003 (Source Foundry; MIT and Bitstream Vera licences).
const HACK_BOLD: &[u8] = include_bytes!("../assets/fonts/Hack-Bold.ttf");

/// The `wght` the bold families draw variable system fonts at.
const BOLD_WEIGHT: u16 = 700;

/// Build the window's font stack without installing it.
///
/// # Arguments
///
/// * `system` - where installed system fonts come in the fallback order,
///   or [`SystemFonts::Off`] to leave them out.
///
/// # Returns
///
/// The stack: egui's default faces, [`SYMBOL_FACE`] as the proportional
/// fallback, the two bold families led by [`BOLD_FACE`], then system fonts
/// when `system` allows them.
#[must_use]
pub fn font_stack(system: SystemFonts) -> Stack {
    let bold = [
        FontFamily::Name(BOLD_FAMILY.into()),
        FontFamily::Name(BOLD_MONO_FAMILY.into()),
    ];
    UiFonts::new(&I18nScripts)
        .app_face(BOLD_FACE, FontData::from_static(HACK_BOLD), &bold)
        .egui_face(SYMBOL_FACE, &[FontFamily::Proportional])
        .family(Family::named(BOLD_FAMILY).weight(BOLD_WEIGHT))
        .family(
            Family::named(BOLD_MONO_FAMILY)
                .weight(BOLD_WEIGHT)
                .monospaced(),
        )
        .system_fonts(system)
        .stack()
}

/// Install the window's font stack in `ctx`.
///
/// The fonts take effect from the context's next pass.
///
/// # Arguments
///
/// * `ctx` - the context to install the fonts in.
/// * `system` - where installed system fonts come, or
///   [`SystemFonts::Off`].
///
/// # Returns
///
/// The installed stack.
pub fn install(ctx: &egui::Context, system: SystemFonts) -> Stack {
    let stack = font_stack(system);
    stack.install(ctx);
    stack
}

/// The font bold text of `size` is drawn in.
///
/// # Arguments
///
/// * `size` - the font size, in points.
/// * `monospace` - whether the text is code.
///
/// # Returns
///
/// A font in [`BOLD_MONO_FAMILY`] for code, else in [`BOLD_FAMILY`].
#[must_use]
pub fn bold_font(size: f32, monospace: bool) -> FontId {
    let family = if monospace {
        BOLD_MONO_FAMILY
    } else {
        BOLD_FAMILY
    };
    FontId::new(size, FontFamily::Name(family.into()))
}
