//! Prompt modifier detection and stripping for the Model Router.
//!
//! Detects three modifier formats (slash, bracket, word) that allow users to
//! explicitly override the classifier's tier decision. When a modifier is
//! detected, it is stripped from the prompt before forwarding to the LLM
//! (FR-020): [`strip_modifiers_from_messages`] runs on every request the router
//! delegates, including tool-loop continuations that resend the history.
//!
//! Modifier aliases map to tiers per FR-019:
//! - `simple`, `basic`, `cheap` -> SIMPLE
//! - `medium`, `balanced` -> MEDIUM
//! - `complex`, `advanced` -> COMPLEX
//! - `max`, `reasoning`, `think`, `deep` -> REASONING

use std::sync::Arc;

use super::router_config::Tier;
use crate::llm::{ChatContent, ChatMessage, ContentPart};

/// Result of modifier detection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModifierResult {
    /// The tier to route to (from the modifier).
    pub tier: Tier,
    /// The remaining prompt text with the modifier stripped.
    pub remaining_prompt: String,
}

/// Detect and strip a prompt modifier, returning the target tier and
/// the remaining prompt text.
///
/// Checks three modifier formats in order:
/// 1. Slash prefix: `/simple <prompt>`, `/max <prompt>`, etc.
/// 2. Bracket prefix: `[simple] <prompt>`, `[complex] <prompt>`, etc.
/// 3. Word prefix: `simple mode: <prompt>`, `deep mode: <prompt>`, etc.
///
/// Returns `None` if no modifier is detected (fall through to classifier).
pub fn detect_modifier(prompt: &str) -> Option<ModifierResult> {
    let trimmed = prompt.trim();

    // 1. Slash prefix: /simple <prompt>, /max <prompt>, etc.
    if let Some(rest) = trimmed.strip_prefix('/')
        && let Some((alias, remaining)) = split_first_word(rest)
        && let Some(tier) = alias_to_tier(alias)
    {
        return Some(ModifierResult {
            tier,
            remaining_prompt: remaining.trim().to_string(),
        });
    }

    // 2. Bracket prefix: [simple] <prompt>, [complex] <prompt>, etc.
    if trimmed.starts_with('[')
        && let Some(close_bracket) = trimmed.find(']')
    {
        let alias = &trimmed[1..close_bracket];
        let remaining = &trimmed[close_bracket + 1..];
        if let Some(tier) = alias_to_tier(alias) {
            return Some(ModifierResult {
                tier,
                remaining_prompt: remaining.trim().to_string(),
            });
        }
    }

    // 3. Word prefix: simple mode: <prompt>, deep mode: <prompt>, etc.
    // Also handles "simple mode, <prompt>" and "deep mode <prompt>"
    if let Some((alias, rest)) = split_first_word(trimmed) {
        let rest_trimmed = rest.trim_start();
        // "mode" must be a whole word: "deep models" or "basic modern UI" are
        // ordinary prompts, not tier modifiers.
        let is_mode_word = rest_trimmed
            .get(.."mode".len())
            .is_some_and(|head| head.eq_ignore_ascii_case("mode"))
            && rest_trimmed["mode".len()..]
                .chars()
                .next()
                .is_none_or(|c| c == ':' || c == ',' || c.is_whitespace());
        if is_mode_word {
            // Strip "mode" and any following separator (colon, comma, space)
            let after_mode = &rest_trimmed["mode".len()..];
            let separator_stripped = after_mode.trim_start_matches([':', ',', ' ']).to_string();
            if let Some(tier) = alias_to_tier(alias) {
                return Some(ModifierResult {
                    tier,
                    remaining_prompt: separator_stripped,
                });
            }
        }
    }

    None
}

/// Map a modifier alias to a tier (FR-019).
fn alias_to_tier(alias: &str) -> Option<Tier> {
    let lower = alias.to_lowercase();
    match lower.as_str() {
        "simple" | "basic" | "cheap" => Some(Tier::Simple),
        "medium" | "balanced" => Some(Tier::Medium),
        "complex" | "advanced" => Some(Tier::Complex),
        "max" | "reasoning" | "think" | "deep" => Some(Tier::Reasoning),
        _ => None,
    }
}

/// Strip a leading tier modifier from every user message before the request is
/// forwarded to the selected model (FR-020).
///
/// The modifier is a routing instruction for the router, not part of the
/// prompt. Every user message is checked (not only the last) because each call
/// resends the history, so an earlier `/max ...` turn would otherwise reach the
/// model on every later request. For a multi-part message the first text part
/// is checked, the same part the router classifies.
///
/// A message that is only a modifier (e.g. `/max`) is left unchanged, so the
/// model never receives an empty user turn. The shared history is copied only
/// when there is something to strip.
///
/// # Arguments
/// * `messages` - the request messages; replaced by an edited copy if needed.
pub fn strip_modifiers_from_messages(messages: &mut Arc<Vec<ChatMessage>>) {
    if !messages.iter().any(|m| stripped_text(m).is_some()) {
        return;
    }
    for message in Arc::make_mut(messages).iter_mut() {
        if let Some(stripped) = stripped_text(message)
            && let Some(text) = first_text_mut(message)
        {
            *text = stripped;
        }
    }
}

/// The text a user message should carry once its modifier is stripped, or
/// `None` when there is nothing to strip.
fn stripped_text(message: &ChatMessage) -> Option<String> {
    if message.role != "user" {
        return None;
    }
    let text = match &message.content {
        ChatContent::Text(text) => text.as_str(),
        ChatContent::Parts(parts) => parts.iter().find_map(|part| match part {
            ContentPart::Text { text } => Some(text.as_str()),
            _ => None,
        })?,
    };
    detect_modifier(text)
        .map(|result| result.remaining_prompt)
        .filter(|remaining| !remaining.is_empty())
}

/// The message text the router classifies: the whole text, or the first text
/// part of a multi-part message.
fn first_text_mut(message: &mut ChatMessage) -> Option<&mut String> {
    match &mut message.content {
        ChatContent::Text(text) => Some(text),
        ChatContent::Parts(parts) => parts.iter_mut().find_map(|part| match part {
            ContentPart::Text { text } => Some(text),
            _ => None,
        }),
    }
}

/// Split a string at the first whitespace boundary, returning
/// (first_word, remainder).
fn split_first_word(s: &str) -> Option<(&str, &str)> {
    let trimmed = s.trim_start();
    if trimmed.is_empty() {
        return None;
    }
    if let Some(pos) = trimmed.find(char::is_whitespace) {
        Some((&trimmed[..pos], &trimmed[pos..]))
    } else {
        None
    }
}

#[cfg(test)]
#[path = "../../tests/inline/router_modifiers.rs"]
mod router_modifiers_tests;
