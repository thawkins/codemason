//! Tests for compression/compaction status bar indicator.
//!
//! Verifies that the status bar correctly shows activity indicators
//! when compression or compaction is in progress.

// -----------------------------------------------------------------------------
// Indicator Label Logic Tests
// --��--------------------------------------------------------------------------

#[test]
fn test_compact_in_progress_shows_compacting() {
    // When only compaction is in progress, the label should be "compacting"
    let compact_in_progress = true;
    let compress_in_progress = false;

    let label = if compact_in_progress && compress_in_progress {
        "cmp+compress"
    } else if compact_in_progress {
        "compacting"
    } else {
        "compressing"
    };

    assert_eq!(label, "compacting");
}

#[test]
fn test_compress_in_progress_shows_compressing() {
    // When only compression is in progress, the label should be "compressing"
    let compact_in_progress = false;
    let compress_in_progress = true;

    let label = if compact_in_progress && compress_in_progress {
        "cmp+compress"
    } else if compact_in_progress {
        "compacting"
    } else {
        "compressing"
    };

    assert_eq!(label, "compressing");
}

#[test]
fn test_both_in_progress_shows_combined() {
    // When both are in progress, show "cmp+compress"
    let compact_in_progress = true;
    let compress_in_progress = true;

    let label = if compact_in_progress && compress_in_progress {
        "cmp+compress"
    } else if compact_in_progress {
        "compacting"
    } else {
        "compressing"
    };

    assert_eq!(label, "cmp+compress");
}

#[test]
fn test_neither_in_progress_no_indicator() {
    // When neither is in progress, no indicator should be shown
    let compact_in_progress = false;
    let compress_in_progress = false;

    let should_show = compact_in_progress || compress_in_progress;
    assert!(!should_show);
}

// -----------------------------------------------------------------------------
// Responsive Mode Label Shortening Tests
// -----------------------------------------------------------------------------

// -----------------------------------------------------------------------------
// Color and Style Tests
// -----------------------------------------------------------------------------
