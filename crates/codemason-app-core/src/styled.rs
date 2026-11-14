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
    /// The frontend's default color (explicitly reset, unlike `None`).
    Reset,
    /// ANSI black.
    Black,
    /// ANSI red.
    Red,
    /// ANSI green.
    Green,
    /// ANSI yellow.
    Yellow,
    /// ANSI blue.
    Blue,
    /// ANSI magenta.
    Magenta,
    /// ANSI cyan.
    Cyan,
    /// ANSI gray (normal white).
    Gray,
    /// ANSI dark gray (bright black).
    DarkGray,
    /// ANSI light red.
    LightRed,
    /// ANSI light green.
    LightGreen,
    /// ANSI light yellow.
    LightYellow,
    /// ANSI light blue.
    LightBlue,
    /// ANSI light magenta.
    LightMagenta,
    /// ANSI light cyan.
    LightCyan,
    /// ANSI white (bright white).
    White,
    /// Entry of the 256-color palette.
    Indexed(u8),
}

/// Text styling attributes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
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

impl TextStyle {
    /// The default (unstyled) style; `const` counterpart of `Default`.
    pub const fn new() -> Self {
        Self {
            fg: None,
            bg: None,
            bold: false,
            italic: false,
            underline: false,
            dim: false,
            monospace: false,
        }
    }

    /// This style with foreground `color`.
    #[must_use]
    pub const fn fg(mut self, color: ThemeColor) -> Self {
        self.fg = Some(color);
        self
    }

    /// This style with background `color`.
    #[must_use]
    pub const fn bg(mut self, color: ThemeColor) -> Self {
        self.bg = Some(color);
        self
    }

    /// This style in bold.
    #[must_use]
    pub const fn bold(mut self) -> Self {
        self.bold = true;
        self
    }

    /// This style in italics.
    #[must_use]
    pub const fn italic(mut self) -> Self {
        self.italic = true;
        self
    }

    /// This style underlined.
    #[must_use]
    pub const fn underline(mut self) -> Self {
        self.underline = true;
        self
    }

    /// This style dimmed.
    #[must_use]
    pub const fn dim(mut self) -> Self {
        self.dim = true;
        self
    }
}

/// A run of text with one style.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct StyledSpan {
    /// The text.
    pub text: String,
    /// Its style.
    pub style: TextStyle,
}

impl StyledSpan {
    /// A span with the default style.
    pub fn raw(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            style: TextStyle::new(),
        }
    }

    /// A span with `style`.
    pub fn styled(text: impl Into<String>, style: TextStyle) -> Self {
        Self {
            text: text.into(),
            style,
        }
    }
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

    /// Build an unstyled line from `s`, one span per text line of `s`.
    ///
    /// Newlines are removed and an empty `s` yields a line with no spans
    /// (the same shape as a terminal UI's raw line constructor).
    pub fn raw(s: &str) -> Self {
        Self {
            spans: s.lines().map(StyledSpan::raw).collect(),
        }
    }

    /// Concatenate all span text, dropping styling.
    pub fn text(&self) -> String {
        self.spans.iter().map(|s| s.text.as_str()).collect()
    }
}

impl From<Vec<StyledSpan>> for StyledLine {
    /// A line made of `spans`, in order.
    fn from(spans: Vec<StyledSpan>) -> Self {
        Self { spans }
    }
}

impl From<StyledSpan> for StyledLine {
    /// A line holding the single `span`.
    fn from(span: StyledSpan) -> Self {
        Self { spans: vec![span] }
    }
}
