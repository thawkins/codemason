//! The window's font stack: a proportional UI face with its own bold
//! weight, symbol and block-glyph fallbacks, a bold code face, and installed
//! system fonts for every other script.
//!
//! egui's default proportional face (Ubuntu Light) has one light weight, no
//! box-drawing, block-element or geometric-shape glyphs (the U+2580..U+259F
//! blocks, the U+25C6 running diamond) and no CJK.
//! [`font_stack`] builds craft-ui's [`UiFonts`] stack with:
//!
//! - the bundled Inter Regular face ([`UI_FACE`]) leading the proportional
//!   family, and Inter SemiBold ([`UI_BOLD_FACE`]) leading [`BOLD_FAMILY`],
//!   so bold prose is drawn in the same typeface as the text around it
//!   (`assets/fonts/Inter-Regular.ttf`, `Inter-SemiBold.ttf`, Inter 4.1, SIL
//!   Open Font License 1.1, see `assets/fonts/Inter-LICENSE.txt`);
//! - egui's default faces after it in the proportional family and first in
//!   the monospace family;
//! - the bundled Lucide icon face ([`crate::icons::ICON_FACE`],
//!   `assets/fonts/lucide.ttf`, ISC licence, see
//!   `assets/fonts/Lucide-LICENSE.txt`) alone in
//!   [`crate::icons::ICON_FAMILY`], used only to draw icons;
//! - egui's bundled `Hack` face ([`SYMBOL_FACE`]) after them in the
//!   proportional family, for the whole U+2500..U+25FF range (box drawing,
//!   block elements, geometric shapes);
//! - the bundled Hack Bold face ([`BOLD_FACE`], `assets/fonts/Hack-Bold.ttf`,
//!   Hack v3.003, MIT and Bitstream Vera licences, see
//!   `assets/fonts/Hack-LICENSE.txt`) leading [`BOLD_MONO_FAMILY`] for bold
//!   code, and following Inter SemiBold in [`BOLD_FAMILY`] for the block and
//!   box-drawing glyphs Inter lacks;
//! - installed system fonts (craft-ui's `system_fonts` feature) for
//!   everything the bundled faces lack: CJK and other scripts, emoji, and
//!   the U+2139 info status icon (Inter draws the U+2717 ballot cross and
//!   U+26A0 warning icons in proportional text; in code they come from system
//!   fonts too). [`SystemFonts::Off`] leaves them out, so tests draw the same
//!   on every machine.
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

/// The egui font name of the bundled bold monospace face.
pub const BOLD_FACE: &str = "Hack-Bold";

/// The egui font name of the bundled proportional UI face.
pub const UI_FACE: &str = "Inter-Regular";

/// The egui font name of the bundled bold proportional UI face.
pub const UI_BOLD_FACE: &str = "Inter-SemiBold";

/// egui's bundled face that covers box drawing, block elements and
/// geometric shapes; the proportional family falls back to it.
pub const SYMBOL_FACE: &str = "Hack";

/// Hack Bold v3.003 (Source Foundry; MIT and Bitstream Vera licences).
const HACK_BOLD: &[u8] = include_bytes!("../assets/fonts/Hack-Bold.ttf");

/// Inter Regular 4.1 (The Inter Project Authors; SIL Open Font License 1.1).
const INTER_REGULAR: &[u8] = include_bytes!("../assets/fonts/Inter-Regular.ttf");

/// Inter SemiBold 4.1 (The Inter Project Authors; SIL Open Font License 1.1).
const INTER_SEMIBOLD: &[u8] = include_bytes!("../assets/fonts/Inter-SemiBold.ttf");

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
/// The stack: [`UI_FACE`] then egui's default faces in the proportional
/// family with [`SYMBOL_FACE`] as its fallback, the Lucide icon face alone
/// in [`crate::icons::ICON_FAMILY`], [`BOLD_FAMILY`] led by
/// [`UI_BOLD_FACE`], [`BOLD_MONO_FAMILY`] led by [`BOLD_FACE`], then system
/// fonts when `system` allows them.
#[must_use]
pub fn font_stack(system: SystemFonts) -> Stack {
    let bold = [
        FontFamily::Name(BOLD_FAMILY.into()),
        FontFamily::Name(BOLD_MONO_FAMILY.into()),
    ];
    UiFonts::new(&I18nScripts)
        .app_face(
            UI_FACE,
            FontData::from_static(INTER_REGULAR),
            &[FontFamily::Proportional],
        )
        .app_face(
            UI_BOLD_FACE,
            FontData::from_static(INTER_SEMIBOLD),
            &[FontFamily::Name(BOLD_FAMILY.into())],
        )
        // Icons, in a family of their own: Lucide's private-use code points
        // would otherwise draw icons for characters other faces reserve.
        .app_face(
            crate::icons::ICON_FACE,
            FontData::from_static(crate::icons::LUCIDE),
            &[FontFamily::Name(crate::icons::ICON_FAMILY.into())],
        )
        .app_face(BOLD_FACE, FontData::from_static(HACK_BOLD), &bold)
        .egui_face(SYMBOL_FACE, &[FontFamily::Proportional])
        .family(Family::named(BOLD_FAMILY).weight(BOLD_WEIGHT))
        .family(Family::named(crate::icons::ICON_FAMILY))
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
