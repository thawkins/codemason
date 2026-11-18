//! Icons from the bundled Lucide font, with ASCII stand-ins.
//!
//! [`Icon`] names the icons the window draws; [`Icon::glyph`] is its code
//! point in the bundled Lucide font (`assets/fonts/lucide.ttf`,
//! `lucide-static` 1.66.0, ISC licence, the Feather-derived icons MIT, see
//! `assets/fonts/Lucide-LICENSE.txt`) and [`Icon::ascii`] what is drawn when
//! the font stack cannot draw it ([`icon_text`]).
//!
//! The Lucide face is alone in its own family, [`ICON_FAMILY`] (see
//! [`crate::fonts::font_stack`]): its glyphs sit at private-use code points
//! that other faces, and model output, use for other things, so text never
//! falls back to it. [`icon_label`] draws an icon in that family.
//!
//! Dependencies: `egui`.

use egui::{FontFamily, FontId, RichText};

/// Name of the Lucide face in the font stack.
pub const ICON_FACE: &str = "Lucide";

/// The font family that holds only the Lucide face.
pub const ICON_FAMILY: &str = "codemason-icons";

/// The Lucide icon font (`lucide-static` 1.66.0).
pub(crate) const LUCIDE: &[u8] = include_bytes!("../assets/fonts/lucide.ttf");

/// An icon the window draws.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Icon {
    /// Reading files (`file`).
    Read,
    /// Editing files (`file-pen`).
    Edit,
    /// Running a command (`square-terminal`).
    Command,
    /// Searching (`search`).
    Search,
    /// An agent or another tool (`bot`).
    Agent,
    /// Done (`check`).
    Check,
    /// Failed (`x`).
    Cross,
    /// A warning (`triangle-alert`).
    Warning,
    /// Information (`info`).
    Info,
    /// Closed (`chevron-right`).
    ChevronRight,
    /// Open (`chevron-down`).
    ChevronDown,
    /// Copy (`copy`).
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

    /// The icon's character in the Lucide font.
    #[must_use]
    pub const fn glyph(self) -> &'static str {
        match self {
            Self::Read => "\u{e0c0}",
            Self::Edit => "\u{e31f}",
            Self::Command => "\u{e20a}",
            Self::Search => "\u{e151}",
            Self::Agent => "\u{e1bb}",
            Self::Check => "\u{e06c}",
            Self::Cross => "\u{e1b2}",
            Self::Warning => "\u{e193}",
            Self::Info => "\u{e0f9}",
            Self::ChevronRight => "\u{e06f}",
            Self::ChevronDown => "\u{e06d}",
            Self::Copy => "\u{e09e}",
        }
    }

    /// What is drawn when the font stack has no Lucide.
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

/// `icon` as text of `size` in `color`: its Lucide glyph in [`ICON_FAMILY`]
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
