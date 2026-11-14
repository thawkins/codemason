//! Small frontend-neutral text and time helpers used by the app state.
//!
//! Moved out of the render-side `utils` and `widgets::message_widget`
//! modules (which re-export them) so the app core does not depend on them.
//!
//! Depends on `codemason-types` (`strutil`).

/// Returns true for providers that use Ollama's boolean `think` parameter.
///
/// Covers both the local (`ollama`) and cloud (`ollama_cloud`) variants so a
/// call site never re-spells the two-id check inline.
pub fn is_ollama_family(provider_id: &str) -> bool {
    provider_id == "ollama" || provider_id == "ollama_cloud"
}

/// Current wall-clock time as Unix milliseconds.
///
/// Returns `0` if the system clock is set before the Unix epoch (which cannot
/// happen in practice); callers use this for relative deadlines, where a `0`
/// base simply expires immediately rather than panicking.
#[must_use]
pub fn now_unix_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Helper to build a ternary for pluralization (e.g., "1 item" vs "2 items").
/// Return the singular or plural form based on `count`.
pub fn pluralize(count: usize, singular: &str, plural: &str) -> String {
    if count == 1 {
        format!("{} {}", count, singular)
    } else {
        format!("{} {}", count, plural)
    }
}

/// Truncate a string to a maximum length, appending an ellipsis if truncated.
///
/// The budget counts visible characters only: `max_len` characters are kept and
/// the `...` is appended after them, so a preview never loses content to the
/// marker. That is [`codemason_types::strutil::truncate_chars_no_ellipsis`] plus the
/// marker here, not [`codemason_types::strutil::truncate_chars`], which reserves
/// three of the budget for the ellipsis.
pub fn truncate_str(s: &str, max_len: usize) -> String {
    let head = codemason_types::strutil::truncate_chars_no_ellipsis(s, max_len);
    if head.len() == s.len() {
        return head;
    }
    format!("{head}...")
}
