//! Frontend-neutral styled text.
//!
//! No dependencies.

/// A semantic or literal color, resolved by each frontend's theme.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ThemeColor {
    /// Accent / heading color.
    Accent,
    /// Default text color.
    #[default]
    Text,
    /// Secondary, de-emphasised text.
    Muted,
    /// Error state.
    Error,
    /// Warning state.
    Warning,
    /// Success state.
    Success,
    /// Informational state.
    Info,
    /// Border color.
    Border,
    /// Literal RGB color.
    Rgb(u8, u8, u8),
}

/// Text styling attributes.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TextStyle {
    /// Foreground color.
    pub fg: Option<ThemeColor>,
    /// Background color.
    pub bg: Option<ThemeColor>,
    /// Bold weight.
    pub bold: bool,
    /// Italic.
    pub italic: bool,
    /// Underline.
    pub underline: bool,
    /// Dimmed.
    pub dim: bool,
    /// Monospace font.
    pub monospace: bool,
}

/// A run of text with one style.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct StyledSpan {
    /// The text.
    pub text: String,
    /// Its style.
    pub style: TextStyle,
}

/// A line made of styled spans.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct StyledLine {
    /// The spans, in order.
    pub spans: Vec<StyledSpan>,
}

impl StyledLine {
    /// Build a single-span line with default style from `s`.
    pub fn plain(s: &str) -> Self {
        Self {
            spans: vec![StyledSpan {
                text: s.to_string(),
                style: TextStyle::default(),
            }],
        }
    }

    /// Concatenate all span text, dropping styling.
    pub fn text(&self) -> String {
        self.spans.iter().map(|s| s.text.as_str()).collect()
    }
}
