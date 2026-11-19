//! Icons from the bundled Material Symbols Rounded font, with ASCII stand-ins.
//!
//! [`Icon`] names the icons the window draws; [`Icon::glyph`] is its code
//! point in the bundled Material Symbols Rounded font
//! (`assets/fonts/MaterialSymbolsRounded.ttf`, a static FILL 0 / wght 400 /
//! opsz 24 instance subset to these icons, Apache License 2.0, see
//! `assets/fonts/MaterialSymbols-LICENSE.txt`, which also gives the command
//! that rebuilds the subset) and [`Icon::ascii`] what is drawn when the font
//! stack cannot draw it ([`icon_label`]).
//!
//! The icon face is alone in its own family, [`ICON_FAMILY`] (see
//! [`crate::fonts::font_stack`]): its glyphs sit at private-use code points
//! that other faces, and model output, use for other things, so text never
//! falls back to it. [`icon_label`] draws an icon in that family.
//!
//! Dependencies: `egui`.

use egui::{FontFamily, FontId, RichText};

/// Name of the Material Symbols face in the font stack.
pub const ICON_FACE: &str = "MaterialSymbolsRounded";

/// The font family that holds only the Material Symbols face.
pub const ICON_FAMILY: &str = "codemason-icons";

/// The Material Symbols Rounded icon font, subset to the [`Icon`] glyphs.
pub(crate) const MATERIAL_SYMBOLS: &[u8] =
    include_bytes!("../assets/fonts/MaterialSymbolsRounded.ttf");

/// An icon the window draws.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Icon {
    /// Reading files (`description`).
    Read,
    /// Editing files (`edit_document`).
    Edit,
    /// Running a command (`terminal`).
    Command,
    /// Searching (`search`).
    Search,
    /// An agent or another tool (`smart_toy`).
    Agent,
    /// Done (`check`).
    Check,
    /// Failed (`close`).
    Cross,
    /// A warning (`warning`).
    Warning,
    /// Information (`info`).
    Info,
    /// Closed (`chevron_right`).
    ChevronRight,
    /// Open (`expand_more`).
    ChevronDown,
    /// Copy (`content_copy`).
    Copy,
}

impl Icon {
    /// Every icon.
    pub const ALL: [Self; 12] = [
        Self::Read,
        Self::Edit,
        Self::Command,
        Self::Search,
        Self::Agent,
        Self::Check,
        Self::Cross,
        Self::Warning,
        Self::Info,
        Self::ChevronRight,
        Self::ChevronDown,
        Self::Copy,
    ];

    /// The icon's character in the Material Symbols font.
    #[must_use]
    pub const fn glyph(self) -> &'static str {
        match self {
            Self::Read => "\u{e873}",
            Self::Edit => "\u{f88c}",
            Self::Command => "\u{eb8e}",
            Self::Search => "\u{ef7a}",
            Self::Agent => "\u{f06c}",
            Self::Check => "\u{e668}",
            Self::Cross => "\u{e5cd}",
            Self::Warning => "\u{f083}",
            Self::Info => "\u{e88e}",
            Self::ChevronRight => "\u{e5cc}",
            Self::ChevronDown => "\u{e5cf}",
            Self::Copy => "\u{e14d}",
        }
    }

    /// What is drawn when the font stack has no icon face.
    #[must_use]
    pub const fn ascii(self) -> &'static str {
        match self {
            Self::Read => "r",
            Self::Edit => "e",
            Self::Command => "$",
            Self::Search => "?",
            Self::Agent => "@",
            Self::Check => "ok",
            Self::Cross => "x",
            Self::Warning => "!",
            Self::Info => "i",
            Self::ChevronRight => ">",
            Self::ChevronDown => "v",
            Self::Copy => "cp",
        }
    }
}

/// Whether the installed fonts can draw `icon` from [`ICON_FAMILY`].
#[must_use]
pub fn has_icon(ctx: &egui::Context, icon: Icon) -> bool {
    let family = FontFamily::Name(ICON_FAMILY.into());
    ctx.fonts_mut(|fonts| {
        fonts
            .definitions()
            .families
            .get(&family)
            .is_some_and(|faces| faces.iter().any(|face| face == ICON_FACE))
            && fonts.has_glyphs(&FontId::new(12.0, family.clone()), icon.glyph())
    })
}

/// `icon` as text of `size` in `color`: its icon glyph in [`ICON_FAMILY`]
/// when the installed fonts have it, else its ASCII stand-in.
#[must_use]
pub fn icon_label(ctx: &egui::Context, icon: Icon, size: f32, color: egui::Color32) -> RichText {
    if has_icon(ctx, icon) {
        RichText::new(icon.glyph())
            .family(FontFamily::Name(ICON_FAMILY.into()))
            .size(size)
            .color(color)
    } else {
        RichText::new(icon.ascii()).size(size).color(color)
    }
}
