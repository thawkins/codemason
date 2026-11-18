//! GUI colours for the frontend-neutral [`ThemeColor`]s.
//!
//! [`Palette::from_tokens`] converts a theme's
//! [`ThemeTokens`](codemason_app_core::theme_tokens::ThemeTokens) to egui
//! colours, and [`palette_for`] does it for the App's current theme. Every
//! colour comes from the theme: the 16 ANSI names resolve to the theme's
//! nearest semantic colour (red is the error colour, gray the muted text), so
//! a pane that still asks for an ANSI name follows the theme. Only
//! [`ThemeColor::Rgb`] and the upper 240 entries of [`ThemeColor::Indexed`]
//! (the xterm-256 cube and grey ramp) are literal.
//!
//! Dependencies: `egui` ([`Color32`]) and `codemason-app-core`
//! ([`ThemeColor`], the theme tokens).

use codemason_app_core::app::App;
use codemason_app_core::styled::ThemeColor;
use codemason_app_core::theme_tokens::{Rgb, ThemeTokens};
use egui::Color32;

/// Channel levels of the xterm-256 6x6x6 colour cube (indices 16..232).
const CUBE_LEVELS: [u8; 6] = [0, 95, 135, 175, 215, 255];

/// How much of the accent is mixed into the background for a selection.
const SELECTION_MIX: f32 = 0.35;

/// The egui colour of a theme colour.
#[must_use]
pub const fn rgb(color: Rgb) -> Color32 {
    Color32::from_rgb(color.0, color.1, color.2)
}

/// The theme colour of an egui colour (its alpha is dropped).
#[must_use]
pub const fn to_rgb(color: Color32) -> Rgb {
    Rgb(color.r(), color.g(), color.b())
}

/// Mix `amount` (0.0 to 1.0) of `over` into `base`, per channel.
#[must_use]
pub fn mix(base: Color32, over: Color32, amount: f32) -> Color32 {
    let channel = |a: u8, b: u8| {
        let value = f32::from(a).mul_add(1.0 - amount, f32::from(b) * amount);
        // The mix of two u8 values is within 0.0..=255.0.
        #[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let value = value.round() as u8;
        value
    };
    Color32::from_rgb(
        channel(base.r(), over.r()),
        channel(base.g(), over.g()),
        channel(base.b(), over.b()),
    )
}

/// The resolved colours of one theme.
///
/// Semantic colours are fields; [`Palette::color`] resolves any
/// [`ThemeColor`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Palette {
    /// Whether the theme is dark.
    pub dark: bool,
    /// [`ThemeColor::Accent`]: the theme's primary colour.
    pub accent: Color32,
    /// [`ThemeColor::Text`] and [`ThemeColor::Reset`]: body text.
    pub text: Color32,
    /// [`ThemeColor::Muted`]: secondary text.
    pub muted: Color32,
    /// [`ThemeColor::Subtle`]: de-emphasised text.
    pub subtle: Color32,
    /// [`ThemeColor::Error`].
    pub error: Color32,
    /// [`ThemeColor::Warning`].
    pub warning: Color32,
    /// [`ThemeColor::Success`].
    pub success: Color32,
    /// [`ThemeColor::Info`].
    pub info: Color32,
    /// [`ThemeColor::Border`]: resting outlines.
    pub border: Color32,
    /// The outline of the focused or hovered widget.
    pub border_focus: Color32,
    /// The panel background the text is drawn on.
    pub background: Color32,
    /// [`ThemeColor::Raised`]: input fields, code blocks, resting widgets.
    pub raised1: Color32,
    /// Buttons and hovered rows.
    pub raised2: Color32,
    /// Dialogs, menus and popups.
    pub overlay: Color32,
    /// The background of selected text and rows: the accent mixed into the
    /// background, so body text stays readable on it.
    pub selection: Color32,
    /// [`ThemeColor::DiffAdded`].
    pub diff_added: Color32,
    /// [`ThemeColor::DiffRemoved`].
    pub diff_removed: Color32,
    /// [`ThemeColor::Heading`].
    pub heading: Color32,
    /// [`ThemeColor::Link`].
    pub link: Color32,
    /// [`ThemeColor::Code`].
    pub code: Color32,
    /// [`ThemeColor::Blockquote`].
    pub blockquote: Color32,
    /// [`ThemeColor::Categorical`]: eight colours for telling agents apart.
    pub categorical: [Color32; 8],
}

impl Palette {
    /// The egui colours of `tokens`.
    #[must_use]
    pub fn from_tokens(tokens: &ThemeTokens) -> Self {
        let background = rgb(tokens.bg.base);
        let accent = rgb(tokens.accent);
        Self {
            dark: tokens.dark,
            accent,
            text: rgb(tokens.text.base),
            muted: rgb(tokens.text.muted),
            subtle: rgb(tokens.text.subtle),
            error: rgb(tokens.state.error),
            warning: rgb(tokens.state.warning),
            success: rgb(tokens.state.success),
            info: rgb(tokens.state.info),
            border: rgb(tokens.border.base),
            border_focus: rgb(tokens.border.focus),
            background,
            raised1: rgb(tokens.bg.raised1),
            raised2: rgb(tokens.bg.raised2),
            overlay: rgb(tokens.bg.overlay),
            selection: mix(background, accent, SELECTION_MIX),
            diff_added: rgb(tokens.diff.added),
            diff_removed: rgb(tokens.diff.removed),
            heading: rgb(tokens.markdown.heading),
            link: rgb(tokens.markdown.link),
            code: rgb(tokens.markdown.code),
            blockquote: rgb(tokens.markdown.blockquote),
            categorical: tokens.categorical.map(rgb),
        }
    }

    /// Resolve `color` to an egui colour.
    ///
    /// # Arguments
    ///
    /// * `color` - the semantic or literal colour to resolve.
    ///
    /// # Returns
    ///
    /// The colour this palette draws `color` with. An ANSI name resolves to
    /// the theme colour of the same meaning: black is the background, white
    /// the body text, gray and dark gray the muted and subtle text, red,
    /// green, yellow and cyan the error, success, warning and info colours,
    /// blue the accent and magenta the fifth categorical colour. A bright
    /// variant is the same colour as its normal one.
    #[must_use]
    pub fn color(&self, color: ThemeColor) -> Color32 {
        match color {
            ThemeColor::Accent | ThemeColor::Blue | ThemeColor::LightBlue => self.accent,
            ThemeColor::Text | ThemeColor::Reset | ThemeColor::White => self.text,
            ThemeColor::Muted | ThemeColor::Gray => self.muted,
            ThemeColor::Subtle | ThemeColor::DarkGray => self.subtle,
            ThemeColor::Error | ThemeColor::Red | ThemeColor::LightRed => self.error,
            ThemeColor::Warning | ThemeColor::Yellow | ThemeColor::LightYellow => self.warning,
            ThemeColor::Success | ThemeColor::Green | ThemeColor::LightGreen => self.success,
            ThemeColor::Info | ThemeColor::Cyan | ThemeColor::LightCyan => self.info,
            ThemeColor::Border => self.border,
            ThemeColor::Raised => self.raised1,
            ThemeColor::Black => self.background,
            ThemeColor::DiffAdded => self.diff_added,
            ThemeColor::DiffRemoved => self.diff_removed,
            ThemeColor::Heading => self.heading,
            ThemeColor::Link => self.link,
            ThemeColor::Code => self.code,
            ThemeColor::Blockquote => self.blockquote,
            ThemeColor::Magenta | ThemeColor::LightMagenta => self.categorical[4],
            ThemeColor::Categorical(index) => self.categorical[usize::from(index % 8)],
            ThemeColor::Rgb(r, g, b) => Color32::from_rgb(r, g, b),
            ThemeColor::Indexed(index) => self.indexed(index),
        }
    }

    /// Resolve an xterm-256 palette `index`.
    ///
    /// 0..16 are the ANSI names (see [`Palette::color`]), 16..232 the 6x6x6
    /// colour cube and 232..256 the 24-step grey ramp (8, 18, ..., 238).
    fn indexed(&self, index: u8) -> Color32 {
        const ANSI: [ThemeColor; 16] = [
            ThemeColor::Black,
            ThemeColor::Red,
            ThemeColor::Green,
            ThemeColor::Yellow,
            ThemeColor::Blue,
            ThemeColor::Magenta,
            ThemeColor::Cyan,
            ThemeColor::Gray,
            ThemeColor::DarkGray,
            ThemeColor::LightRed,
            ThemeColor::LightGreen,
            ThemeColor::LightYellow,
            ThemeColor::LightBlue,
            ThemeColor::LightMagenta,
            ThemeColor::LightCyan,
            ThemeColor::White,
        ];
        match index {
            0..=15 => self.color(ANSI[usize::from(index)]),
            16..=231 => {
                let i = usize::from(index - 16);
                Color32::from_rgb(
                    CUBE_LEVELS[i / 36],
                    CUBE_LEVELS[(i / 6) % 6],
                    CUBE_LEVELS[i % 6],
                )
            }
            232..=255 => {
                let v = 8 + 10 * (index - 232);
                Color32::from_rgb(v, v, v)
            }
        }
    }
}

/// The palette of the App's current theme.
///
/// # Arguments
///
/// * `app` - the App, for its theme selection and catalog.
/// * `ctx` - the window's egui context, for the system's dark/light setting.
///
/// # Returns
///
/// The palette of [`crate::theme::active_tokens`].
#[must_use]
pub fn palette_for(app: &App, ctx: &egui::Context) -> Palette {
    Palette::from_tokens(&crate::theme::active_tokens(app, ctx))
}
