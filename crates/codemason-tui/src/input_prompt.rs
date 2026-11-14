//! Chat input prompt prefix (`"> "` / `"NN> "`) helpers.
//!
//! Shared by the app's input geometry (cursor and selection mapping) and the
//! render side, so both derive the prefix width from one source.
//!
//! No dependencies.

/// Width in columns of the `"> "` prompt when no queue counter is shown.
const PROMPT_WIDTH: usize = 2;
/// Width in columns of the two-digit, zero-padded queue counter.
const COUNTER_WIDTH: usize = 2;

/// Build the chat input's prompt prefix string.
///
/// The prompt is `"> "` while the input queue is empty and `"NN> "` (a
/// two-digit, zero-padded counter immediately before `>`) while entries are
/// pending (spec `inputqueue` FR-009, FR-010). The counter is clamped to `99`
/// so the prefix never widens beyond four columns (NFR-002).
pub(crate) fn input_prompt_prefix(queue_len: usize) -> String {
    if queue_len == 0 {
        "> ".to_string()
    } else {
        format!("{:02}> ", queue_len.min(99))
    }
}

/// Width in columns of the chat input's prompt prefix for `queue_len`.
///
/// Mirrors [`input_prompt_prefix`] without allocating.  Every geometry
/// consumer (wrapped rows, cursor position, selection mapping) derives its
/// prefix width from this so the cursor column and wrapped-row geometry stay
/// aligned to the counter prefix (spec `inputqueue` NFR-002, FR-020).
pub(crate) fn input_prompt_prefix_len(queue_len: usize) -> usize {
    if queue_len == 0 {
        PROMPT_WIDTH
    } else {
        PROMPT_WIDTH + COUNTER_WIDTH
    }
}

/// Selection-safe variant of [`input_prompt_prefix`].
///
/// Identical in width to the painted prompt prefix so mouse and keyboard
/// selection columns line up with the rendered frame (NFR-002), but the
/// counter digits are replaced by spaces so the queue counter can never be
/// copied or cut as message content (FR-020).
pub(crate) fn input_prompt_selection_prefix(queue_len: usize) -> String {
    if queue_len == 0 {
        "> ".to_string()
    } else {
        format!("{}> ", " ".repeat(COUNTER_WIDTH))
    }
}
