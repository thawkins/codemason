//! Status-bar message types: categories, timestamped messages and history.
//!
//! Frontend-neutral (no terminal-UI types). Colors and styles for a category are
//! resolved by the frontend (see `theme::StatusStyle` in the terminal UI).
//!
//! No dependencies beyond `std`.

/// Success icon (checkmark circle).
pub const ICON_SUCCESS: &str = "● ";
/// Error icon (X).
pub const ICON_ERROR: &str = "✗ ";
/// Running icon (diamond).
pub const ICON_RUNNING: &str = "◆ ";

/// Status message categories for consistent status bar feedback
///
/// This enum standardizes how status messages are categorized and displayed,
/// ensuring consistent visual feedback across the application.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusCategory {
    /// General informational messages (cyan/white)
    Info,
    /// Successful operation completion (green)
    Success,
    /// Warning messages that need attention (yellow)
    Warning,
    /// Error messages (red)
    Error,
    /// Ongoing work with progress indication (cyan with spinner)
    Working,
}

impl StatusCategory {
    /// Get the icon associated with this status category
    pub fn icon(&self) -> &'static str {
        match self {
            Self::Info => "ℹ ",
            Self::Success => ICON_SUCCESS,
            Self::Warning => "⚠ ",
            Self::Error => ICON_ERROR,
            Self::Working => ICON_RUNNING,
        }
    }

    /// Format a message with the appropriate prefix and styling
    pub fn format(&self, message: &str) -> String {
        format!("{} {}", self.icon(), message)
    }

    /// Get a display name for the category
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Info => "info",
            Self::Success => "success",
            Self::Warning => "warning",
            Self::Error => "error",
            Self::Working => "working",
        }
    }
}

/// A status message with category and timestamp for history tracking
#[derive(Debug, Clone)]
pub struct StatusMessage {
    /// The message content
    pub message: String,
    /// The status category
    pub category: StatusCategory,
    /// When the message was created
    pub timestamp: std::time::SystemTime,
}

impl StatusMessage {
    /// Create a new status message
    pub fn new(message: impl Into<String>, category: StatusCategory) -> Self {
        Self {
            message: message.into(),
            category,
            timestamp: std::time::SystemTime::now(),
        }
    }

    /// Create an info status message
    pub fn info(message: impl Into<String>) -> Self {
        Self::new(message, StatusCategory::Info)
    }

    /// Create a success status message
    pub fn success(message: impl Into<String>) -> Self {
        Self::new(message, StatusCategory::Success)
    }

    /// Create a warning status message
    pub fn warning(message: impl Into<String>) -> Self {
        Self::new(message, StatusCategory::Warning)
    }

    /// Create an error status message
    pub fn error(message: impl Into<String>) -> Self {
        Self::new(message, StatusCategory::Error)
    }

    /// Create a working status message
    pub fn working(message: impl Into<String>) -> Self {
        Self::new(message, StatusCategory::Working)
    }

    /// Format the message with category prefix
    pub fn formatted(&self) -> String {
        self.category.format(&self.message)
    }

    /// Get age of the message in seconds
    pub fn age_secs(&self) -> u64 {
        self.timestamp.elapsed().unwrap_or_default().as_secs()
    }
}

/// Status message history for tracking recent messages
#[derive(Debug, Clone, Default)]
pub struct StatusHistory {
    messages: std::collections::VecDeque<StatusMessage>,
    max_size: usize,
}

impl StatusHistory {
    /// Create a new status history with default capacity
    pub fn new() -> Self {
        Self {
            messages: std::collections::VecDeque::new(),
            max_size: 100,
        }
    }

    /// Create a new status history with custom capacity
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            messages: std::collections::VecDeque::new(),
            max_size: capacity,
        }
    }

    /// Add a message to the history
    pub fn push(&mut self, message: StatusMessage) {
        if self.messages.len() >= self.max_size {
            self.messages.pop_front();
        }
        self.messages.push_back(message);
    }

    /// Get all messages in chronological order (oldest first)
    pub fn messages(&self) -> &std::collections::VecDeque<StatusMessage> {
        &self.messages
    }

    /// Get recent messages (newest first, up to n)
    pub fn recent(&self, n: usize) -> Vec<&StatusMessage> {
        self.messages.iter().rev().take(n).collect()
    }

    /// Get messages filtered by category
    pub fn by_category(&self, category: StatusCategory) -> Vec<&StatusMessage> {
        self.messages
            .iter()
            .filter(|m| m.category == category)
            .collect()
    }

    /// Clear all messages
    pub fn clear(&mut self) {
        self.messages.clear();
    }

    /// Get the number of messages in history
    pub fn len(&self) -> usize {
        self.messages.len()
    }

    /// Check if history is empty
    pub fn is_empty(&self) -> bool {
        self.messages.is_empty()
    }
}
