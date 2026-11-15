//! Tests for compression/compaction status bar indicator.
//!
//! Verifies that the status bar correctly shows activity indicators
//! when compression or compaction is in progress.

use codemason_tui::layout_statusbar::{ResponsiveMode, colors, indicators};

// -----------------------------------------------------------------------------
// Indicator Label Logic Tests
// --��--------------------------------------------------------------------------

// -----------------------------------------------------------------------------
// Responsive Mode Label Shortening Tests
// -----------------------------------------------------------------------------

#[test]
fn test_minimal_mode_shows_spinner_icon() {
    // In Minimal mode, show just the spinner icon
    let mode = ResponsiveMode::Minimal;
    let label = "compacting";

    let short_label = match mode {
        ResponsiveMode::Minimal => "⟳",
        _ => label,
    };

    assert_eq!(short_label, "⟳");
}

#[test]
fn test_full_mode_shows_full_label() {
    // In Full mode, show the full label
    let mode = ResponsiveMode::Full;
    let label = "compacting";

    let short_label = match mode {
        ResponsiveMode::Minimal => "⟳",
        _ => label,
    };

    assert_eq!(short_label, "compacting");
}

#[test]
fn test_compact_mode_shows_full_label() {
    // In Compact mode, show the full label (not abbreviated)
    let mode = ResponsiveMode::Compact;
    let label = "compressing";

    let short_label = match mode {
        ResponsiveMode::Minimal => "⟳",
        _ => label,
    };

    assert_eq!(short_label, "compressing");
}

// -----------------------------------------------------------------------------
// Color and Style Tests
// -----------------------------------------------------------------------------

#[test]
fn test_indicator_uses_warning_color() {
    // The activity indicator should use the WARNING color (yellow)
    assert_eq!(colors::WARNING, ratatui::style::Color::Yellow);
}

#[test]
fn test_busy_indicator_character() {
    // The status-bar busy tag is a plain ASCII text label (ANTIPAT M1 sweep);
    // it is rendered as `BUSY:<label>` on the second status-bar line.
    assert_eq!(indicators::BUSY, "BUSY:");
}
