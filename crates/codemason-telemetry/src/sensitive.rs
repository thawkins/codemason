//! Sensitive-data guard for metric attributes and resource attributes (FR-034).
//!
//! FR-034: "The system shall not record sensitive data (API keys, file
//! contents, user prompts) as metric attributes or resource attributes."
//!
//! This module provides [`sanitize_attr_value`], a defensive filter that
//! inspects a candidate attribute value string and replaces it with the
//! [`REDACTED`] sentinel when it matches a known sensitive pattern. The
//! guard is applied at the point where [`InstrumentRegistry`](crate::instruments::InstrumentRegistry)
//! builds attribute key/value pairs (the `attr_*` helpers) and where the
//! [`CardinalityCache`](crate::cardinality::CardinalityCache) resolves
//! attribute sets, so that a caller that accidentally passes an API key,
//! a file path containing secrets, or a chunk of file/prompt content can
//! never leak that data into an exported metric.
//!
//! # What counts as "sensitive"
//!
//! The guard uses a conservative deny-list rather than attempting to
//! understand the semantics of every field. A value is redacted if **any**
//! of the following are true:
//!
//! - It looks like a common API-key prefix (`sk-`, `sk_`, `Bearer `, `ghp_`,
//!   `gho_`, `github_pat_`, `xoxb-`, `AKIA`, `AIza`).
//! - It contains a `:` separator between two long alphanumeric runs (the
//!   classic `username:password` or `key:secret` shape).
//! - It contains a newline, tab, or carriage return - legitimate model,
//!   provider, tool, and session identifiers never contain whitespace
//!   beyond simple spaces; multi-line values indicate file content or a
//!   pasted prompt.
//! - It exceeds 256 characters - legitimate low-cardinality identifiers
//!   are short; a very long value suggests a pasted blob.
//! - It contains an `=` followed by a long base64-like run (a common
//!   shape for inline tokens / credentials).
//!
//! Under a model-name attribute key, a value with the shape of an Ollama
//! `name:tag` id skips the `key:secret` rule (see
//! [`looks_sensitive_model_name`] and [`is_ollama_model_id`]); every other
//! value, including `admin:password1234`, is still checked in full.
//!
//! The guard is intentionally over-cautious: false positives (redacting a
//! genuinely long model name) only collapse the value into a single
//! `redacted` bucket, which is the same behaviour already used by the
//! cardinality cap (FR-035) for overflow values.

use codemason_types::sanitize::CREDENTIAL_PREFIXES;

/// Sentinel value substituted in place of a sensitive attribute value (FR-034).
///
/// Re-exported from [`codemason_types::sanitize::REDACTION_PLACEHOLDER`] so the
/// telemetry guard writes the same sentinel as every other redactor (audit
/// Quick Win 5).
///
/// The distinct [`crate::cardinality::UNKNOWN_BUCKET`] (`"unknown"`) still
/// separates "cardinality overflow" from "sensitive data was redacted".
pub const REDACTED: &str = codemason_types::sanitize::REDACTION_PLACEHOLDER;

/// Maximum number of characters in a legitimate attribute value before the
/// guard treats it as suspicious (FR-034).
///
/// Model names, provider ids, tool names, and session ids are all short
/// identifiers. A value longer than this is almost certainly a pasted blob
/// (file content, prompt text, a long token).
pub const MAX_ATTR_VALUE_LEN: usize = 256;

/// Minimum length of each side of a `key:secret` shaped value before the guard
/// treats it as a credential (LOW-2).
///
/// Shorter fragments are common in legitimate identifiers (e.g. a `name:tag`
/// model id) and are not flagged; a credential-shaped value has dense parts of
/// at least this length on both sides of the colon.
pub const MIN_CRED_PART_LEN: usize = 4;

/// Minimum length of a base64-like run following `=` before the guard treats it
/// as an inline token (LOW-2).
pub const MIN_B64_RUN_LEN: usize = 20;

/// Maximum length of a `tool.name` attribute value accepted by the allow-list
/// (SEC-052).
///
/// Registered tool names are short identifiers (`Bash`, `codeindex_search`,
/// `plugin_<id>_<tool>`); anything longer is rejected rather than exported.
pub const MAX_TOOL_NAME_LEN: usize = 64;

/// Sentinel substituted for a `tool.name` value that fails the allow-list
/// (SEC-052).
pub const UNKNOWN_TOOL_NAME: &str = "unknown";

/// Returns `true` when `value` matches a known sensitive pattern (FR-034).
///
/// This is the core predicate used by [`sanitize_attr_value`]. It is kept
/// separate so that tests can assert on individual patterns.
///
/// # Arguments
///
/// * `value` - The candidate attribute value to inspect.
///
/// # Examples
///
/// ```
/// use codemason_telemetry::sensitive::looks_sensitive;
///
/// assert!(looks_sensitive("sk-proj-abc123"));
/// assert!(looks_sensitive("Bearer abc123"));
/// assert!(looks_sensitive("user:secretpassword"));
/// assert!(looks_sensitive("line1\nline2"));
/// assert!(!looks_sensitive("claude-sonnet-4-20250514"));
/// ```
#[must_use]
pub fn looks_sensitive(value: &str) -> bool {
    looks_sensitive_inner(value, ColonRule::Apply)
}

/// Returns `true` when a model-name attribute value matches a sensitive
/// pattern (FR-034; deep-refactor D-11).
///
/// Identical to [`looks_sensitive`] except for a value that
/// [`is_ollama_model_id`] recognises: Ollama `name:tag` ids (`llama3:latest`,
/// `gpt-oss:120b`) have the `key:secret` colon shape and were being exported
/// as `redacted`, so for them the colon rule is skipped. Any other value under
/// a model key, including `admin:password1234`, gets the full check, and the
/// remaining rules (key prefixes, length, control characters, `=token` runs)
/// apply to every value.
///
/// # Examples
///
/// ```
/// use codemason_telemetry::sensitive::looks_sensitive_model_name;
///
/// assert!(!looks_sensitive_model_name("llama3:latest"));
/// assert!(!looks_sensitive_model_name("gpt-oss:120b"));
/// assert!(looks_sensitive_model_name("admin:password1234"));
/// assert!(looks_sensitive_model_name("sk-proj-abc123"));
/// ```
#[must_use]
pub fn looks_sensitive_model_name(value: &str) -> bool {
    if is_ollama_model_id(value) {
        looks_sensitive_inner(value, ColonRule::Skip)
    } else {
        looks_sensitive(value)
    }
}

/// Tag words Ollama uses as the first part of a model tag, besides `latest`
/// and a parameter size such as `8b`.
const OLLAMA_TAG_WORDS: &[&str] = &[
    "instruct", "chat", "code", "coder", "text", "base", "vision", "mini", "small", "medium",
    "large", "fp16", "q4", "q5", "q8",
];

/// Returns `true` when `value` has the shape of an Ollama `name:tag` model id.
///
/// The name is a run of ASCII alphanumerics, `.`, `_`, `-` or `/`. The tag's
/// first `-`/`_`-separated word must be `latest`, a parameter size (`8b`,
/// `1.5b`, `120b`, `270m`), or one of a few known variant words (`instruct`,
/// `coder`, `fp16`, ...). A credential such as `admin:password1234` has no
/// such tag, so it is not treated as a model id.
///
/// # Examples
///
/// ```
/// use codemason_telemetry::sensitive::is_ollama_model_id;
///
/// assert!(is_ollama_model_id("qwen2.5-coder:32b-instruct"));
/// assert!(!is_ollama_model_id("admin:password1234"));
/// ```
#[must_use]
pub fn is_ollama_model_id(value: &str) -> bool {
    let Some((name, tag)) = value.split_once(':') else {
        return false;
    };
    let name_ok = !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-' | '/'));
    let tag_ok = !tag.is_empty()
        && tag
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'));
    if !name_ok || !tag_ok {
        return false;
    }
    let first = tag
        .split(['-', '_'])
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase();
    first == "latest" || is_parameter_size(&first) || OLLAMA_TAG_WORDS.contains(&first.as_str())
}

/// Whether `word` is a parameter size such as `8b`, `1.5b`, `120b` or `270m`.
fn is_parameter_size(word: &str) -> bool {
    let Some(number) = word.strip_suffix(['b', 'm', 'k']) else {
        return false;
    };
    let mut parts = number.splitn(2, '.');
    let whole = parts.next().unwrap_or_default();
    let fraction = parts.next();
    !whole.is_empty()
        && whole.chars().all(|c| c.is_ascii_digit())
        && fraction.is_none_or(|f| !f.is_empty() && f.chars().all(|c| c.is_ascii_digit()))
}

/// Whether [`looks_sensitive_inner`] applies the `key:secret` colon rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ColonRule {
    Apply,
    Skip,
}

/// The shared sensitive-value predicate behind [`looks_sensitive`] and
/// [`looks_sensitive_model_name`].
fn looks_sensitive_inner(value: &str, colon_rule: ColonRule) -> bool {
    // -- Length guard ------------------------------------------------------
    // Legitimate identifiers are short; a very long value is almost
    // certainly a pasted blob (file content, prompt, long token).
    if value.len() > MAX_ATTR_VALUE_LEN {
        return true;
    }

    // -- Whitespace guard --------------------------------------------------
    // Newlines, tabs, and carriage returns never appear in legitimate
    // model/provider/tool/session identifiers. Their presence indicates
    // multi-line content (file contents, a pasted prompt).
    if value.contains('\n') || value.contains('\r') || value.contains('\t') {
        return true;
    }

    // -- Known API-key prefixes --------------------------------------------
    // CO-07: the one list in `codemason_types::sanitize`, shared with log
    // redaction. Matching is case-insensitive because real tokens are
    // case-mixed while the prefix is consistently cased. An entry with a
    // `min_tail` needs that many characters after the prefix, so a tool name
    // such as `npm_install` is not mistaken for an `npm_` token.
    let lower = value.to_ascii_lowercase();
    if CREDENTIAL_PREFIXES.iter().any(|entry| {
        lower.starts_with(&entry.prefix.to_ascii_lowercase())
            && value.len() - entry.prefix.len() >= entry.min_tail
    }) {
        return true;
    }

    // -- "key:secret" shape ------------------------------------------------
    // A single colon separating two runs of >=4 alphanumeric chars is the
    // classic credential shape (username:password, key:secret). Legitimate
    // model names can contain colons (e.g. "ollama:qwen3:1.7b") so we only
    // flag it when there are exactly two colon-separated parts and both
    // parts are "dense" (no spaces, no dots, at least 4 chars). This avoids
    // false positives on Ollama model identifiers.
    if colon_rule == ColonRule::Apply
        && let Some(idx) = value.find(':')
    {
        let rest = &value[idx + 1..];
        // Only flag if there is no further colon (two parts total).
        if !rest.contains(':') {
            let left = &value[..idx];
            let right = rest;
            // Both parts must be at least 4 chars and contain no spaces or
            // dots (dots are common in model versions like "1.7b").
            if left.len() >= MIN_CRED_PART_LEN
                && right.len() >= MIN_CRED_PART_LEN
                && !left.contains(' ')
                && !right.contains(' ')
                && !left.contains('.')
                && !right.contains('.')
            {
                return true;
            }
        }
    }

    // -- Inline "token=..." shape -------------------------------------------
    // An `=` followed by a run of >=20 base64-like characters is a common
    // shape for inline credentials. Legitimate identifiers never contain
    // `=` at all (OTEL attribute keys/values in codemason are simple strings).
    if let Some(idx) = value.find('=') {
        let after = &value[idx + 1..];
        // Count the base64-like run length immediately after '='.
        let run_len = after
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '/' | '_' | '-'))
            .count();
        if run_len >= MIN_B64_RUN_LEN {
            return true;
        }
    }

    false
}

/// Sanitize a candidate attribute value, replacing it with [`REDACTED`] if
/// it matches a sensitive pattern (FR-034).
///
/// This is the primary entry point used by the `attr_*` helpers in
/// [`InstrumentRegistry`](crate::instruments::InstrumentRegistry) and by
/// the resource-attribute builder in
/// [`TelemetrySubsystem`](crate::subsystem::TelemetrySubsystem). It never
/// panics and never blocks - it is a pure string inspection.
///
/// # Arguments
///
/// * `value` - The candidate attribute value.
///
/// # Returns
///
/// The original `value` (as an owned `String`) if it is safe, or
/// [`REDACTED`] if it matched a sensitive pattern.
///
/// # Examples
///
/// ```
/// use codemason_telemetry::sensitive::{sanitize_attr_value, REDACTED};
///
/// // Safe values pass through unchanged.
/// assert_eq!(sanitize_attr_value("claude-sonnet-4-20250514"), "claude-sonnet-4-20250514");
/// assert_eq!(sanitize_attr_value("ollama"), "ollama");
/// assert_eq!(sanitize_attr_value("bash"), "bash");
///
/// // Sensitive values are redacted.
/// assert_eq!(sanitize_attr_value("sk-proj-abc123def456"), REDACTED);
/// assert_eq!(sanitize_attr_value("Bearer abc123"), REDACTED);
/// ```
#[must_use]
pub fn sanitize_attr_value(value: &str) -> String {
    if looks_sensitive(value) {
        REDACTED.to_string()
    } else {
        value.to_string()
    }
}

/// Returns `true` for attribute keys that carry a model name: `model`, and any
/// key ending in `_model` or `.model` (e.g. `selected_model`).
#[must_use]
pub fn is_model_attr_key(key: &str) -> bool {
    key.strip_suffix("model")
        .is_some_and(|head| head.is_empty() || head.ends_with(['_', '.']))
}

/// Sanitize an attribute value with the check its key calls for: model-name
/// keys ([`is_model_attr_key`]) use [`looks_sensitive_model_name`], every other
/// key uses [`looks_sensitive`].
///
/// # Arguments
///
/// * `key` - The attribute key.
/// * `value` - The candidate attribute value.
///
/// # Returns
///
/// `value` as an owned `String` if it is safe, or [`REDACTED`].
///
/// # Examples
///
/// ```
/// use codemason_telemetry::sensitive::{sanitize_attr_for_key, REDACTED};
///
/// assert_eq!(sanitize_attr_for_key("model", "llama3:latest"), "llama3:latest");
/// assert_eq!(sanitize_attr_for_key("component", "user:secretpassword"), REDACTED);
/// ```
#[must_use]
pub fn sanitize_attr_for_key(key: &str, value: &str) -> String {
    let sensitive = if is_model_attr_key(key) {
        looks_sensitive_model_name(value)
    } else {
        looks_sensitive(value)
    };
    if sensitive {
        REDACTED.to_string()
    } else {
        value.to_string()
    }
}

/// Returns `true` when `name` conforms to the conservative `tool.name`
/// allow-list pattern (SEC-052).
///
/// The telemetry crate cannot see the tool registry, so the allow-list is a
/// shape check rather than a membership check: a conforming name is a short
/// run of ASCII alphanumerics, `_`, `-`, or `.` (which covers every registered
/// tool name, `plugin_<id>_<tool>` bridge names, and namespaced MCP tool ids)
/// and is at most [`MAX_TOOL_NAME_LEN`] characters. Anything else - an empty
/// string, whitespace, slashes, `@`, `:`, quotes, newlines, or a pasted blob -
/// is rejected.
///
/// # Arguments
///
/// * `name` - The candidate `tool.name` attribute value.
///
/// # Examples
///
/// ```
/// use codemason_telemetry::sensitive::is_allowed_tool_name;
///
/// assert!(is_allowed_tool_name("bash"));
/// assert!(is_allowed_tool_name("codeindex_search"));
/// assert!(is_allowed_tool_name("plugin_my-plugin_do_thing"));
/// assert!(!is_allowed_tool_name(""));
/// assert!(!is_allowed_tool_name("../../etc/passwd"));
/// assert!(!is_allowed_tool_name("user:secretpassword"));
/// ```
#[must_use]
pub fn is_allowed_tool_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= MAX_TOOL_NAME_LEN
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.'))
}

/// Sanitize a model-controlled `tool.name` attribute value against the
/// allow-list, replacing a non-conforming value with [`UNKNOWN_TOOL_NAME`]
/// (SEC-052).
///
/// Unlike [`sanitize_attr_value`] (a deny-list keyed on sensitive *shapes*),
/// this is an allow-list: only a value that matches the conservative tool-name
/// pattern is exported verbatim, so a model can never smuggle an arbitrary
/// string (a URL, a credential, prompt text) into the `tool.name` label. A
/// conforming value still passes through [`sanitize_attr_value`] as defence in
/// depth.
///
/// # Arguments
///
/// * `name` - The candidate `tool.name` attribute value.
///
/// # Returns
///
/// `name` when it conforms to the allow-list, otherwise [`UNKNOWN_TOOL_NAME`].
#[must_use]
pub fn sanitize_tool_name(name: &str) -> String {
    if is_allowed_tool_name(name) {
        sanitize_attr_value(name)
    } else {
        UNKNOWN_TOOL_NAME.to_string()
    }
}
