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
    /// A new session (`add_comment`).
    Add,
    /// The session list (`forum`).
    Sessions,
    /// Undo (`undo`).
    Undo,
    /// Redo (`redo`).
    Redo,
    /// Stop the running turn (`stop_circle`).
    Stop,
    /// Compact the conversation (`compress`).
    Compact,
    /// An image (`image`).
    Image,
    /// A prompt template (`article`).
    Template,
    /// Open a folder (`folder_open`).
    FolderOpen,
    /// A new folder (`create_new_folder`).
    NewFolder,
    /// The model (`psychology`).
    Model,
    /// An agent loop (`repeat`).
    Loop,
    /// The log (`receipt_long`).
    Log,
    /// The profiler (`speed`).
    Profile,
    /// The task list (`checklist`).
    Tasks,
    /// Memory (`memory`).
    Memory,
    /// The colour theme (`palette`).
    Theme,
    /// Cost (`payments`).
    Cost,
    /// Usage statistics (`bar_chart`).
    Stats,
    /// Diagnostics (`health_and_safety`).
    Doctor,
    /// Quit (`logout`).
    Quit,
    /// Collapse (`expand_less`).
    ChevronUp,
}

impl Icon {
    /// Every icon.
    pub const ALL: [Self; 34] = [
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
        Self::Add,
        Self::Sessions,
        Self::Undo,
        Self::Redo,
        Self::Stop,
        Self::Compact,
        Self::Image,
        Self::Template,
        Self::FolderOpen,
        Self::NewFolder,
        Self::Model,
        Self::Loop,
        Self::Log,
        Self::Profile,
        Self::Tasks,
        Self::Memory,
        Self::Theme,
        Self::Cost,
        Self::Stats,
        Self::Doctor,
        Self::Quit,
        Self::ChevronUp,
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
            Self::Add => "\u{e266}",
            Self::Sessions => "\u{e8af}",
            Self::Undo => "\u{e166}",
            Self::Redo => "\u{e15a}",
            Self::Stop => "\u{ef71}",
            Self::Compact => "\u{e94d}",
            Self::Image => "\u{e3f4}",
            Self::Template => "\u{ef42}",
            Self::FolderOpen => "\u{e2c8}",
            Self::NewFolder => "\u{e2cc}",
            Self::Model => "\u{ea4a}",
            Self::Loop => "\u{e040}",
            Self::Log => "\u{ef6e}",
            Self::Profile => "\u{e9e4}",
            Self::Tasks => "\u{e6b1}",
            Self::Memory => "\u{e322}",
            Self::Theme => "\u{e40a}",
            Self::Cost => "\u{ef63}",
            Self::Stats => "\u{e26b}",
            Self::Doctor => "\u{e1d5}",
            Self::Quit => "\u{e9ba}",
            Self::ChevronUp => "\u{e5ce}",
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
            Self::Add => "+",
            Self::Sessions => "=",
            Self::Undo => "<-",
            Self::Redo => "->",
            Self::Stop => "[]",
            Self::Compact => "><",
            Self::Image => "img",
            Self::Template => "tpl",
            Self::FolderOpen => "dir",
            Self::NewFolder => "+dir",
            Self::Model => "llm",
            Self::Loop => "oo",
            Self::Log => "log",
            Self::Profile => "prf",
            Self::Tasks => "tsk",
            Self::Memory => "mem",
            Self::Theme => "thm",
            Self::Cost => "$$",
            Self::Stats => "#",
            Self::Doctor => "dr",
            Self::Quit => "q",
            Self::ChevronUp => "^",
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
