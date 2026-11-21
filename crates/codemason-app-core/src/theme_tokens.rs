//! Semantic theme tokens, the built-in themes and the theme catalog.
//!
//! A [`ThemeTokens`] names every colour a frontend draws with by meaning
//! (`text.muted`, `diff.added`, `markdown.heading`), not by hue. It carries no
//! frontend types: each frontend maps the tokens to its own colour type. Themes
//! are JSON in the token shape; the built-in ones are embedded from
//! `crates/codemason-app-core/themes/`, and [`ThemeCatalog::load_dir`] adds the
//! user's own. [`ThemeSelection`] is what the user picked: a named theme, or
//! `system`, which follows the operating system between the built-in dark and
//! light themes.
//!
//! Every theme also has Material Design 3 colour roles ([`M3Roles`]): the
//! `document-light` and `document-dark` themes carry their own (an `m3`
//! block, generated from one blue seed), and [`ThemeTokens::roles`] derives
//! them for every other theme.
//!
//! Dependencies: `serde` and `serde_json` (theme files), `codemason-config`
//! (saving the chosen theme).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// Name of the built-in dark theme, used when the system is dark.
pub const DARK_THEME: &str = "codemason-dark";

/// Name of the built-in light theme, used when the system is light.
pub const LIGHT_THEME: &str = "codemason-light";

/// Name of the built-in high-contrast theme.
pub const HIGH_CONTRAST_THEME: &str = "high-contrast";

/// WCAG 2 level AA minimum contrast ratio for body text.
pub const AA_CONTRAST: f32 = 4.5;

/// WCAG 2 minimum contrast ratio for large text and graphical objects, the
/// floor for de-emphasised text and state colours.
pub const UI_CONTRAST: f32 = 3.0;

/// Largest theme file [`ThemeCatalog::load_dir`] reads, in bytes.
const MAX_THEME_BYTES: u64 = 64 * 1024;

/// The built-in themes as `(file name, JSON)`, the dark default first.
const BUILTIN_JSON: [(&str, &str); 9] = [
    (
        "codemason-dark.json",
        include_str!("../themes/codemason-dark.json"),
    ),
    (
        "codemason-light.json",
        include_str!("../themes/codemason-light.json"),
    ),
    (
        "high-contrast.json",
        include_str!("../themes/high-contrast.json"),
    ),
    (
        "catppuccin-mocha.json",
        include_str!("../themes/catppuccin-mocha.json"),
    ),
    (
        "gruvbox-dark.json",
        include_str!("../themes/gruvbox-dark.json"),
    ),
    ("nord.json", include_str!("../themes/nord.json")),
    (
        "tokyo-night.json",
        include_str!("../themes/tokyo-night.json"),
    ),
    (
        "document-light.json",
        include_str!("../themes/document-light.json"),
    ),
    (
        "document-dark.json",
        include_str!("../themes/document-dark.json"),
    ),
];

/// An opaque sRGB colour, written `#rrggbb` in a theme file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Rgb(pub u8, pub u8, pub u8);

impl Rgb {
    /// Parses `#rrggbb` (the `#` is required, the digits are case-insensitive).
    ///
    /// Returns `None` for any other shape.
    #[must_use]
    pub fn from_hex(text: &str) -> Option<Self> {
        let digits = text.strip_prefix('#')?;
        if digits.len() != 6 || !digits.is_ascii() {
            return None;
        }
        let channel = |at: usize| u8::from_str_radix(&digits[at..at + 2], 16).ok();
        Some(Self(channel(0)?, channel(2)?, channel(4)?))
    }

    /// Formats the colour as lowercase `#rrggbb`, the shape a theme file uses.
    #[must_use]
    pub fn to_hex(self) -> String {
        format!("#{:02x}{:02x}{:02x}", self.0, self.1, self.2)
    }

    /// The WCAG 2 relative luminance of this colour, 0.0 (black) to 1.0 (white).
    #[must_use]
    pub fn relative_luminance(self) -> f32 {
        fn channel(value: u8) -> f32 {
            let c = f32::from(value) / 255.0;
            if c <= 0.039_28 {
                c / 12.92
            } else {
                ((c + 0.055) / 1.055).powf(2.4)
            }
        }
        let red = 0.2126 * channel(self.0);
        let green = 0.7152f32.mul_add(channel(self.1), red);
        0.0722f32.mul_add(channel(self.2), green)
    }
}

impl<'de> Deserialize<'de> for Rgb {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        Self::from_hex(&text).ok_or_else(|| {
            serde::de::Error::custom(format!("expected a colour as \"#rrggbb\", got \"{text}\""))
        })
    }
}

impl Serialize for Rgb {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_hex())
    }
}

/// The WCAG 2 contrast ratio between two colours.
///
/// Returns `(L1 + 0.05) / (L2 + 0.05)` for the lighter (`L1`) and darker (`L2`)
/// relative luminance: 1.0 for equal colours, 21.0 for white on black. The
/// ratio is symmetric.
#[must_use]
pub fn contrast_ratio(a: Rgb, b: Rgb) -> f32 {
    let (la, lb) = (a.relative_luminance(), b.relative_luminance());
    let (light, dark) = if la >= lb { (la, lb) } else { (lb, la) };
    (light + 0.05) / (dark + 0.05)
}

/// Text colours, from most to least prominent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TextTokens {
    /// Body text.
    pub base: Rgb,
    /// Secondary text.
    pub muted: Rgb,
    /// De-emphasised text: timestamps, hints, gutters.
    pub subtle: Rgb,
}

/// Surface colours, from the window background upwards.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BgTokens {
    /// The window and panel background.
    pub base: Rgb,
    /// The first raised surface: input fields, code blocks, resting widgets.
    pub raised1: Rgb,
    /// The second raised surface: buttons and hovered rows.
    pub raised2: Rgb,
    /// Floating surfaces: dialogs, menus and popups.
    pub overlay: Rgb,
}

/// Border colours.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BorderTokens {
    /// Resting outlines and separators.
    pub base: Rgb,
    /// The outline of the focused or hovered widget.
    pub focus: Rgb,
}

/// State colours.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StateTokens {
    /// Success.
    pub success: Rgb,
    /// Warning.
    pub warning: Rgb,
    /// Error.
    pub error: Rgb,
    /// Information.
    pub info: Rgb,
}

/// Diff colours.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DiffTokens {
    /// Text of an added line.
    pub added: Rgb,
    /// Text of a removed line.
    pub removed: Rgb,
    /// Text of an unchanged line.
    pub context: Rgb,
    /// Background of an added line.
    pub added_bg: Rgb,
    /// Background of a removed line.
    pub removed_bg: Rgb,
    /// Line numbers and hunk markers.
    pub line_number: Rgb,
}

/// Syntax-highlighting colours.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SyntaxTokens {
    /// Keywords.
    pub keyword: Rgb,
    /// String literals.
    pub string: Rgb,
    /// Comments.
    pub comment: Rgb,
    /// Function names.
    pub function: Rgb,
    /// Type names.
    pub r#type: Rgb,
    /// Numeric literals.
    pub number: Rgb,
    /// Operators.
    pub operator: Rgb,
    /// Punctuation.
    pub punctuation: Rgb,
}

/// Markdown colours.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MarkdownTokens {
    /// Headings.
    pub heading: Rgb,
    /// Links.
    pub link: Rgb,
    /// Inline code and code blocks.
    pub code: Rgb,
    /// Block quotes.
    pub blockquote: Rgb,
}

/// One theme: every colour a frontend draws with, by meaning.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ThemeTokens {
    /// The name the theme is selected by, e.g. `codemason-dark`.
    pub name: String,
    /// Whether the theme is dark (light text on a dark background).
    pub dark: bool,
    /// Attribution for a ported palette.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub credit: Option<String>,
    /// Text colours.
    pub text: TextTokens,
    /// Surface colours.
    pub bg: BgTokens,
    /// Border colours.
    pub border: BorderTokens,
    /// The primary accent: selection, focus, the user's own prompt marker.
    pub accent: Rgb,
    /// State colours.
    pub state: StateTokens,
    /// Diff colours.
    pub diff: DiffTokens,
    /// Syntax-highlighting colours.
    pub syntax: SyntaxTokens,
    /// Markdown colours.
    pub markdown: MarkdownTokens,
    /// Eight distinguishable colours for telling agents apart.
    pub categorical: [Rgb; 8],
    /// Material Design 3 colour roles. Optional in a theme file: when absent,
    /// [`ThemeTokens::roles`] derives them from the tokens above.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub m3: Option<M3Roles>,
}

/// Material Design 3 colour roles.
///
/// Each `on_x` role is the text and icon colour drawn on the `x` role.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct M3Roles {
    /// High-emphasis fills: filled buttons, active indicators.
    pub primary: Rgb,
    /// Content on `primary`.
    pub on_primary: Rgb,
    /// Lower-emphasis fills in the primary hue: tonal buttons, selection.
    pub primary_container: Rgb,
    /// Content on `primary_container`.
    pub on_primary_container: Rgb,
    /// Less prominent accents.
    pub secondary: Rgb,
    /// Content on `secondary`.
    pub on_secondary: Rgb,
    /// Fills such as the active navigation item and filter chips.
    pub secondary_container: Rgb,
    /// Content on `secondary_container`.
    pub on_secondary_container: Rgb,
    /// Contrasting accents.
    pub tertiary: Rgb,
    /// Content on `tertiary`.
    pub on_tertiary: Rgb,
    /// Fills in the tertiary hue.
    pub tertiary_container: Rgb,
    /// Content on `tertiary_container`.
    pub on_tertiary_container: Rgb,
    /// Errors.
    pub error: Rgb,
    /// Content on `error`.
    pub on_error: Rgb,
    /// Error fills such as an error banner.
    pub error_container: Rgb,
    /// Content on `error_container`.
    pub on_error_container: Rgb,
    /// The default background.
    pub surface: Rgb,
    /// Text and icons on any surface.
    pub on_surface: Rgb,
    /// Lower-emphasis text and icons on any surface.
    pub on_surface_variant: Rgb,
    /// The lowest surface container (the page).
    pub surface_container_lowest: Rgb,
    /// A low surface container (the desk around the page, drawers).
    pub surface_container_low: Rgb,
    /// The default surface container (menus, the ribbon).
    pub surface_container: Rgb,
    /// A high surface container (dialogs).
    pub surface_container_high: Rgb,
    /// The highest surface container (text fields, code blocks).
    pub surface_container_highest: Rgb,
    /// Important boundaries: text-field outlines.
    pub outline: Rgb,
    /// Decorative boundaries: dividers.
    pub outline_variant: Rgb,
    /// The background of snackbars and tooltips.
    pub inverse_surface: Rgb,
    /// Text and icons on `inverse_surface`.
    pub inverse_on_surface: Rgb,
    /// Actions on `inverse_surface`, such as a snackbar's button.
    pub inverse_primary: Rgb,
    /// The dimming layer behind a modal.
    pub scrim: Rgb,
    /// Shadows under elevated surfaces.
    pub shadow: Rgb,
}

impl M3Roles {
    /// Checks every `on_x` role against its `x` role, `primary` (text
    /// buttons, the open tab, links) against `surface`, and
    /// `inverse_primary` (the snackbar's action) against `inverse_surface`
    /// at [`AA_CONTRAST`], and `outline` against `surface` at
    /// [`UI_CONTRAST`].
    ///
    /// # Arguments
    ///
    /// * `theme` - the theme name the messages carry.
    ///
    /// # Returns
    ///
    /// One message per pair below its floor, empty when all pass.
    #[must_use]
    pub fn contrast_failures(&self, theme: &str) -> Vec<String> {
        let pairs = [
            (
                "on_primary",
                self.on_primary,
                "primary",
                self.primary,
                AA_CONTRAST,
            ),
            (
                "on_primary_container",
                self.on_primary_container,
                "primary_container",
                self.primary_container,
                AA_CONTRAST,
            ),
            (
                "on_secondary",
                self.on_secondary,
                "secondary",
                self.secondary,
                AA_CONTRAST,
            ),
            (
                "on_secondary_container",
                self.on_secondary_container,
                "secondary_container",
                self.secondary_container,
                AA_CONTRAST,
            ),
            (
                "on_tertiary",
                self.on_tertiary,
                "tertiary",
                self.tertiary,
                AA_CONTRAST,
            ),
            (
                "on_tertiary_container",
                self.on_tertiary_container,
                "tertiary_container",
                self.tertiary_container,
                AA_CONTRAST,
            ),
            ("on_error", self.on_error, "error", self.error, AA_CONTRAST),
            (
                "on_error_container",
                self.on_error_container,
                "error_container",
                self.error_container,
                AA_CONTRAST,
            ),
            (
                "on_surface",
                self.on_surface,
                "surface",
                self.surface,
                AA_CONTRAST,
            ),
            (
                "on_surface_variant",
                self.on_surface_variant,
                "surface",
                self.surface,
                AA_CONTRAST,
            ),
            (
                "on_surface",
                self.on_surface,
                "surface_container_highest",
                self.surface_container_highest,
                AA_CONTRAST,
            ),
            (
                "outline",
                self.outline,
                "surface",
                self.surface,
                UI_CONTRAST,
            ),
            (
                "inverse_on_surface",
                self.inverse_on_surface,
                "inverse_surface",
                self.inverse_surface,
                AA_CONTRAST,
            ),
            // The snackbar's action is text on `inverse_surface`.
            (
                "inverse_primary",
                self.inverse_primary,
                "inverse_surface",
                self.inverse_surface,
                AA_CONTRAST,
            ),
            // Text buttons, the open tab and links are `primary` text.
            (
                "primary",
                self.primary,
                "surface",
                self.surface,
                AA_CONTRAST,
            ),
        ];
        pairs
            .into_iter()
            .filter_map(|(fg_name, fg, bg_name, bg, floor)| {
                let ratio = contrast_ratio(fg, bg);
                (ratio < floor).then(|| {
                    format!(
                        "theme `{theme}`: m3.{fg_name} has contrast {ratio:.1}:1 on \
                         m3.{bg_name}, below {floor}:1"
                    )
                })
            })
            .collect()
    }
}

/// Mixes `amount` (0.0 to 1.0) of `b` into `a`, per sRGB channel.
fn mix(a: Rgb, b: Rgb, amount: f32) -> Rgb {
    let channel = |x: u8, y: u8| {
        // The mix of two u8 values is within 0.0..=255.0.
        f32::from(x)
            .mul_add(1.0 - amount, f32::from(y) * amount)
            .round() as u8
    };
    Rgb(channel(a.0, b.0), channel(a.1, b.1), channel(a.2, b.2))
}

/// The action colour on an inverse surface: `accent` mixed into `base`, at
/// the largest of a few amounts that reads at [`AA_CONTRAST`] on `inverse`,
/// else `base` itself (which reads on `inverse`, the body text colour, as
/// body text reads on `base`).
fn inverse_action(base: Rgb, accent: Rgb, inverse: Rgb) -> Rgb {
    [0.5, 0.35, 0.2]
        .into_iter()
        .map(|amount| mix(base, accent, amount))
        .find(|colour| contrast_ratio(*colour, inverse) >= AA_CONTRAST)
        .unwrap_or(base)
}

/// White or black, whichever contrasts more with `fill`.
fn on_colour(fill: Rgb) -> Rgb {
    let (white, black) = (Rgb(255, 255, 255), Rgb(0, 0, 0));
    if contrast_ratio(white, fill) >= contrast_ratio(black, fill) {
        white
    } else {
        black
    }
}

impl ThemeTokens {
    /// Every colour of the theme as `(key, colour)`, keyed by its dotted path
    /// in a theme file: `text.base`, `accent`, `categorical.3`, `m3.primary`.
    ///
    /// The Material roles are listed even when the theme file has none: they
    /// are then the derived [`ThemeTokens::roles`]. Groups and keys come in
    /// the order of a serialised theme file.
    #[must_use]
    pub fn colours(&self) -> Vec<(String, Rgb)> {
        let mut full = self.clone();
        full.m3 = Some(self.roles());
        let mut out = Vec::new();
        if let Ok(serde_json::Value::Object(root)) = serde_json::to_value(&full) {
            for (key, value) in &root {
                collect_colours(key, value, &mut out);
            }
        }
        out
    }

    /// Sets the colour at `key` (a key from [`ThemeTokens::colours`]).
    ///
    /// Setting an `m3.*` key on a theme without its own Material roles first
    /// stores the derived roles, so the other roles keep their look.
    ///
    /// # Returns
    ///
    /// `false`, with the theme unchanged, when `key` names no colour.
    pub fn set_colour(&mut self, key: &str, value: Rgb) -> bool {
        let mut edited = self.clone();
        if key.starts_with("m3.") && edited.m3.is_none() {
            edited.m3 = Some(edited.roles());
        }
        let Ok(mut json) = serde_json::to_value(&edited) else {
            return false;
        };
        let slot = key.split('.').try_fold(&mut json, |node, part| match node {
            serde_json::Value::Object(map) => map.get_mut(part),
            serde_json::Value::Array(items) => {
                part.parse::<usize>().ok().and_then(|i| items.get_mut(i))
            }
            _ => None,
        });
        match slot {
            Some(slot) if slot.as_str().and_then(Rgb::from_hex).is_some() => {
                *slot = serde_json::Value::String(value.to_hex());
            }
            _ => return false,
        }
        match serde_json::from_value::<Self>(json) {
            Ok(theme) => {
                *self = theme;
                true
            }
            Err(_) => false, // no-error-ok: a key that broke the shape names no colour
        }
    }

    /// The theme's Material Design 3 colour roles.
    ///
    /// # Returns
    ///
    /// The theme file's `m3` block when it has one. Otherwise roles derived
    /// from the other tokens: `accent` is `primary`, `text.muted` is
    /// `secondary`, `markdown.link` is `tertiary`, `state.error` is `error`,
    /// the `bg` ladder gives the surface containers and the borders give the
    /// outlines. Containers are their colour mixed 20% into `bg.base`, and
    /// content on a solid fill is white or black, whichever contrasts more.
    /// The inverse surface is `text.base` with `bg.base` on it, and its
    /// action colour is the accent mixed into `bg.base`, as much of it as
    /// still reads at [`AA_CONTRAST`] on the inverse surface.
    #[must_use]
    pub fn roles(&self) -> M3Roles {
        if let Some(roles) = self.m3 {
            return roles;
        }
        let base = self.bg.base;
        let container = |colour: Rgb| mix(base, colour, 0.2);
        M3Roles {
            primary: self.accent,
            on_primary: on_colour(self.accent),
            primary_container: container(self.accent),
            on_primary_container: self.text.base,
            secondary: self.text.muted,
            on_secondary: base,
            secondary_container: self.bg.raised2,
            on_secondary_container: self.text.base,
            tertiary: self.markdown.link,
            on_tertiary: on_colour(self.markdown.link),
            tertiary_container: container(self.markdown.link),
            on_tertiary_container: self.text.base,
            error: self.state.error,
            on_error: on_colour(self.state.error),
            error_container: container(self.state.error),
            on_error_container: self.text.base,
            surface: base,
            on_surface: self.text.base,
            on_surface_variant: self.text.muted,
            surface_container_lowest: base,
            surface_container_low: mix(base, self.bg.raised1, 0.5),
            surface_container: self.bg.raised1,
            surface_container_high: self.bg.raised2,
            surface_container_highest: mix(self.bg.raised2, self.text.base, 0.08),
            outline: self.text.subtle,
            outline_variant: self.border.base,
            inverse_surface: self.text.base,
            inverse_on_surface: base,
            inverse_primary: inverse_action(base, self.accent, self.text.base),
            scrim: Rgb(0, 0, 0),
            shadow: Rgb(0, 0, 0),
        }
    }

    /// Parses a theme from JSON in the token shape.
    ///
    /// The name is trimmed and lowercased, so a theme is selected, listed and
    /// replaced by one spelling: a file naming its theme `Nord` replaces the
    /// built-in `nord`.
    ///
    /// # Errors
    ///
    /// Returns the parse error when the JSON is malformed, a token is missing
    /// or unknown, a colour is not `#rrggbb`, or the name is empty.
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        let mut theme: Self = serde_json::from_str(json)?;
        theme.name = theme.name.trim().to_lowercase();
        if theme.name.is_empty() {
            return Err(serde::de::Error::custom("the theme name is empty"));
        }
        Ok(theme)
    }

    /// Checks the theme's text colours against its window background.
    ///
    /// Body and secondary text must reach [`AA_CONTRAST`]; de-emphasised text,
    /// the accent and the state colours must reach [`UI_CONTRAST`]. A theme
    /// file's own `m3` block is checked with [`M3Roles::contrast_failures`];
    /// derived roles are not, so a token failure is reported once.
    ///
    /// Returns one message per colour below its floor, empty when all pass.
    #[must_use]
    pub fn contrast_failures(&self) -> Vec<String> {
        let checks = [
            ("text.base", self.text.base, AA_CONTRAST),
            ("text.muted", self.text.muted, AA_CONTRAST),
            ("text.subtle", self.text.subtle, UI_CONTRAST),
            ("accent", self.accent, UI_CONTRAST),
            ("state.success", self.state.success, UI_CONTRAST),
            ("state.warning", self.state.warning, UI_CONTRAST),
            ("state.error", self.state.error, UI_CONTRAST),
            ("state.info", self.state.info, UI_CONTRAST),
        ];
        let mut failures: Vec<String> = checks
            .into_iter()
            .filter_map(|(token, color, floor)| {
                let ratio = contrast_ratio(color, self.bg.base);
                (ratio < floor).then(|| {
                    format!(
                        "theme `{}`: {token} has contrast {ratio:.1}:1 on bg.base, below {floor}:1",
                        self.name
                    )
                })
            })
            .collect();
        if let Some(roles) = &self.m3 {
            failures.extend(roles.contrast_failures(&self.name));
        }
        failures
    }
}

/// The theme the user picked.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum ThemeSelection {
    /// Follow the operating system between [`DARK_THEME`] and [`LIGHT_THEME`].
    #[default]
    System,
    /// The catalog theme of this name.
    Named(String),
}

impl ThemeSelection {
    /// The name the selection is shown and selected by.
    #[must_use]
    pub fn display_name(&self) -> &str {
        match self {
            Self::System => "system",
            Self::Named(name) => name,
        }
    }
}

/// The themes that can be selected: the built-in ones plus any loaded from
/// theme directories.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThemeCatalog {
    /// The themes in listing order; names are unique.
    themes: Vec<Arc<ThemeTokens>>,
    /// The file each theme loaded by [`ThemeCatalog::load_dir`] came from.
    /// Built-in themes have no entry.
    sources: BTreeMap<String, PathBuf>,
}

impl Default for ThemeCatalog {
    fn default() -> Self {
        Self::builtin()
    }
}

impl ThemeCatalog {
    /// The catalog of built-in themes.
    #[must_use]
    pub fn builtin() -> Self {
        static BUILTIN: OnceLock<Vec<Arc<ThemeTokens>>> = OnceLock::new();
        let themes = BUILTIN.get_or_init(|| {
            BUILTIN_JSON
                .iter()
                .filter_map(|(file, json)| match ThemeTokens::from_json(json) {
                    Ok(theme) => Some(Arc::new(theme)),
                    Err(error) => {
                        tracing::error!(file, %error, "built-in theme does not parse");
                        None
                    }
                })
                .collect()
        });
        Self {
            themes: themes.clone(),
            sources: BTreeMap::new(),
        }
    }

    /// Adds every `*.json` theme in `dir`, in file-name order.
    ///
    /// A theme whose name is already in the catalog replaces that entry, so a
    /// directory loaded later wins. A missing directory adds nothing.
    ///
    /// Returns one warning per file that was skipped (unreadable, larger than
    /// 64 KiB, or not a valid theme) and per contrast failure of a loaded theme.
    pub fn load_dir(&mut self, dir: &Path) -> Vec<String> {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return Vec::new();
        };
        let mut files: Vec<_> = entries
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
            .collect();
        files.sort();
        let mut warnings = Vec::new();
        for path in files {
            match read_theme(&path) {
                Ok(theme) => {
                    warnings.extend(theme.contrast_failures());
                    self.insert_from(theme, path);
                }
                Err(reason) => {
                    warnings.push(format!("theme file {} skipped: {reason}", path.display()));
                }
            }
        }
        warnings
    }

    /// Adds `theme`, replacing the entry of the same name.
    pub fn insert(&mut self, theme: ThemeTokens) {
        let theme = Arc::new(theme);
        match self.themes.iter_mut().find(|t| t.name == theme.name) {
            Some(slot) => *slot = theme,
            None => self.themes.push(theme),
        }
    }

    /// Adds `theme`, loaded from `path`, replacing the entry of the same name,
    /// and records `path` as the theme's source file.
    pub fn insert_from(&mut self, theme: ThemeTokens, path: PathBuf) {
        self.sources.insert(theme.name.clone(), path);
        self.insert(theme);
    }

    /// The file the theme called `name` was loaded from, or `None` for a
    /// built-in theme no file replaced.
    #[must_use]
    pub fn source(&self, name: &str) -> Option<&Path> {
        self.sources.get(name).map(PathBuf::as_path)
    }

    /// Whether `name` is one of the built-in themes, whether or not a file
    /// currently replaces it.
    #[must_use]
    pub fn is_builtin(&self, name: &str) -> bool {
        Self::builtin().get(name).is_some()
    }

    /// The theme called `name`, if the catalog has one.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<Arc<ThemeTokens>> {
        self.themes.iter().find(|t| t.name == name).cloned()
    }

    /// The theme names in listing order.
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.themes.iter().map(|t| t.name.as_str())
    }

    /// Parses what the user typed into a selection.
    ///
    /// `system` and `default` follow the operating system. Any other input is
    /// a theme name; failing that, `dark`, `light` and the spellings
    /// `high_contrast`/`highcontrast` are aliases of the built-in themes, so a
    /// user theme called `dark` is reachable by its name. Matching trims and
    /// ignores case.
    ///
    /// Returns `None` when `input` names no theme.
    #[must_use]
    pub fn parse(&self, input: &str) -> Option<ThemeSelection> {
        let lowered = input.trim().to_lowercase();
        if matches!(lowered.as_str(), "system" | "default") {
            return Some(ThemeSelection::System);
        }
        let alias = match lowered.as_str() {
            "dark" => DARK_THEME,
            "light" => LIGHT_THEME,
            "high_contrast" | "highcontrast" => HIGH_CONTRAST_THEME,
            other => other,
        };
        [lowered.as_str(), alias]
            .into_iter()
            .find_map(|name| self.get(name))
            .map(|t| ThemeSelection::Named(t.name.clone()))
    }

    /// The tokens `selection` draws with.
    ///
    /// `system_dark` is whether the operating system asks for a dark theme; it
    /// only matters for [`ThemeSelection::System`]. A named theme that is no
    /// longer in the catalog resolves like `System`.
    #[must_use]
    pub fn resolve(&self, selection: &ThemeSelection, system_dark: bool) -> Arc<ThemeTokens> {
        let named = match selection {
            ThemeSelection::System => None,
            ThemeSelection::Named(name) => self.get(name),
        };
        named
            .or_else(|| self.get(if system_dark { DARK_THEME } else { LIGHT_THEME }))
            .unwrap_or_else(fallback_theme)
    }
}

/// The built-in dark theme, for a catalog that lost its own copy.
fn fallback_theme() -> Arc<ThemeTokens> {
    static FALLBACK: OnceLock<Arc<ThemeTokens>> = OnceLock::new();
    FALLBACK
        .get_or_init(|| {
            let json = BUILTIN_JSON[0].1;
            let theme = ThemeTokens::from_json(json).expect("embedded theme"); // no-panic-ok: compile-time constant, parsed by test_theme_tokens
            Arc::new(theme)
        })
        .clone()
}

/// Reads and parses one theme file.
///
/// Only a regular file (or a symlink to one) is read, so a FIFO cannot block
/// startup and a device cannot be read without end. At most
/// [`MAX_THEME_BYTES`] + 1 bytes are read, so a file that grows after the size
/// check is still refused.
fn read_theme(path: &Path) -> Result<ThemeTokens, String> {
    use std::io::Read as _;

    let metadata = std::fs::metadata(path).map_err(|e| e.to_string())?;
    if !metadata.is_file() {
        return Err("not a regular file".to_string());
    }
    if metadata.len() > MAX_THEME_BYTES {
        return Err(format!(
            "{} bytes is over the {MAX_THEME_BYTES} byte limit",
            metadata.len()
        ));
    }
    let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let mut json = String::new();
    file.take(MAX_THEME_BYTES + 1)
        .read_to_string(&mut json)
        .map_err(|e| e.to_string())?;
    if json.len() as u64 > MAX_THEME_BYTES {
        return Err(format!("over the {MAX_THEME_BYTES} byte limit"));
    }
    ThemeTokens::from_json(&json).map_err(|e| e.to_string())
}

/// Adds the colours under `value` to `out`, keyed by their dotted path from
/// `key`. Only `#rrggbb` strings inside a group or array are colours; the
/// top-level `name` and `credit` strings are not.
fn collect_colours(key: &str, value: &serde_json::Value, out: &mut Vec<(String, Rgb)>) {
    match value {
        serde_json::Value::String(text) if key != "name" && key != "credit" => {
            if let Some(colour) = Rgb::from_hex(text) {
                out.push((key.to_string(), colour));
            }
        }
        serde_json::Value::Object(map) => {
            for (child, inner) in map {
                collect_colours(&format!("{key}.{child}"), inner, out);
            }
        }
        serde_json::Value::Array(items) => {
            for (i, inner) in items.iter().enumerate() {
                collect_colours(&format!("{key}.{i}"), inner, out);
            }
        }
        _ => {}
    }
}

/// The test override of [`user_themes_dir`].
fn user_themes_dir_override() -> &'static Mutex<Option<PathBuf>> {
    static OVERRIDE: OnceLock<Mutex<Option<PathBuf>>> = OnceLock::new();
    OVERRIDE.get_or_init(|| Mutex::new(None))
}

/// Points [`user_themes_dir`] at `dir` (or back at the real directory with
/// `None`), so tests never write into the user's config.
#[doc(hidden)]
pub fn set_user_themes_dir_override(dir: Option<PathBuf>) {
    *user_themes_dir_override()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = dir;
}

/// The one-shot test pin of [`user_themes_dir`], set by
/// [`use_scratch_themes_dir_for_tests`].
static THEMES_DIR_PIN: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();

/// Point [`user_themes_dir`] at the workspace scratch tree for this test
/// process.
///
/// A test that builds an [`crate::app::App`] loads the user's themes
/// directory, so a theme the developer saved with the theme editor would add
/// its contrast warnings to the app's log and break "log is empty" assertions.
/// Pinning the directory to a git-ignored `target/temp/themes` path keeps a
/// test run independent of the machine it runs on. The first call wins and the
/// pin outlives later [`set_user_themes_dir_override`] resets, so a test that
/// keeps its own scratch directory still sets the override and takes
/// precedence.
#[doc(hidden)]
pub fn use_scratch_themes_dir_for_tests() {
    let _ = THEMES_DIR_PIN.set(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join("target/temp/themes"),
    ); // INTENTIONAL: first set wins; later calls are no-ops
}

/// The user's theme directory, `~/.config/codemason/themes`, where an edited
/// built-in theme is saved; `None` when no config directory can be found.
///
/// The [`set_user_themes_dir_override`] value wins, then the test pin, so a
/// test run never reads the machine's own themes.
#[must_use]
pub fn user_themes_dir() -> Option<PathBuf> {
    let forced = user_themes_dir_override()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone();
    forced
        .or_else(|| THEMES_DIR_PIN.get().cloned())
        .or_else(|| codemason_config::Config::global_config_dir().map(|d| d.join("themes")))
}

/// The file name a theme called `name` is saved under: every character
/// outside `[A-Za-z0-9_-]` becomes `_`, so the name cannot leave the theme
/// directory, and `.json` is added.
#[must_use]
pub fn theme_file_name(name: &str) -> String {
    let stem: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    format!("{stem}.json")
}

/// Writes `theme` to `path` as a theme file, creating the parent directory.
///
/// The JSON goes to `<path>.tmp` first and is renamed over `path`, so a
/// failed write never leaves a half-written theme behind.
///
/// # Errors
///
/// Returns an error when the directory cannot be created or the file cannot
/// be written or renamed.
pub fn save_theme(theme: &ThemeTokens, path: &Path) -> anyhow::Result<()> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent)?;
    }
    let mut json = serde_json::to_string_pretty(theme)?;
    json.push('\n');
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, json)?;
    if let Err(err) = std::fs::rename(&tmp, path) {
        let _ = std::fs::remove_file(&tmp); // INTENTIONAL: best-effort cleanup; the rename error is returned
        return Err(err.into());
    }
    Ok(())
}

/// The theme the launch configuration asks for, set once by the binary.
static STARTUP: OnceLock<String> = OnceLock::new();

/// Records the launch configuration's `theme` value, so a new
/// [`App`](crate::app::App) starts in it.
///
/// The binary calls this with the configuration it actually loaded (the
/// `--config` file when one was given), which the layered config chain an App
/// could read for itself does not always match. The first call wins.
pub fn set_startup(theme: &str) {
    // INTENTIONAL: a second call keeps the first theme.
    let _ = STARTUP.set(theme.to_string());
}

/// Saves `name` as the `theme` key of the user-global config, so the next
/// start opens in that theme.
///
/// Only that one key is edited; every other key in the file is left as it is.
/// Under `codemason --config <file>` the explicit file is edited instead (see
/// [`codemason_config::Config::set_config_write_target`]). A project
/// `.codemason/codemason.json` that sets `theme` still overrides the saved
/// value at startup.
///
/// # Errors
///
/// Returns an error if the config file cannot be read, parsed or written.
pub fn persist(name: &str) -> anyhow::Result<()> {
    codemason_config::Config::edit_global_config(|root| {
        let value = serde_json::Value::String(name.to_string());
        if root.get("theme") == Some(&value) {
            return Ok(false);
        }
        root.insert("theme".to_string(), value);
        Ok(true)
    })?;
    Ok(())
}

/// The `theme` value given to [`set_startup`], if any.
#[must_use]
pub fn startup() -> Option<&'static str> {
    STARTUP.get().map(String::as_str)
}
