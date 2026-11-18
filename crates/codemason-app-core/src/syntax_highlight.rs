//! Syntax highlighting of code blocks, as frontend-neutral roles.
//!
//! [`highlight`] parses code with a tree-sitter grammar and its highlight
//! query and returns the code as [`HighlightSpan`]s, each a byte range with
//! the [`SyntaxRole`] it is drawn in (keyword, string, comment, ...) or none.
//! A frontend maps the roles to its theme's `syntax.*` colours.
//!
//! Languages: Rust, Python, JavaScript (and JSX), TypeScript, TSX, Go, C,
//! C++, Java, Bash and JSON, by the name or common alias after a code
//! fence (`rs`, `py`, `js`, `ts`, `sh`, ...). The grammars are the ones
//! `codemason-codeindex` builds, plus Bash and JSON. Each language's
//! configuration is built once, on first use.
//!
//! Dependencies: `tree-sitter`, `tree-sitter-highlight` and the grammar
//! crates.

use std::collections::HashMap;
use std::ops::Range;
use std::sync::OnceLock;

use tree_sitter_highlight::{HighlightConfiguration, HighlightEvent, Highlighter};

/// Largest code block highlighted, in bytes; a longer one is drawn plain.
pub const MAX_HIGHLIGHT_BYTES: usize = 64 * 1024;

/// What a piece of code is, for colouring.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SyntaxRole {
    /// Keywords.
    Keyword,
    /// String and character literals, and escapes.
    String,
    /// Comments.
    Comment,
    /// Function, method and macro names.
    Function,
    /// Type names and constructors.
    Type,
    /// Numbers, booleans and other constants.
    Number,
    /// Operators.
    Operator,
    /// Brackets, delimiters and other punctuation.
    Punctuation,
}

/// A byte range of the code and the role it is drawn in (`None`: plain).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HighlightSpan {
    /// The bytes of the code this span covers.
    pub range: Range<usize>,
    /// The role, or `None` for plain text.
    pub role: Option<SyntaxRole>,
}

/// The capture names the highlight queries are matched against, with the
/// role of each. A query's capture matches the longest name it starts with,
/// so `function.method.call` is `function.method`.
const CAPTURES: &[(&str, Option<SyntaxRole>)] = &[
    ("attribute", Some(SyntaxRole::Function)),
    ("boolean", Some(SyntaxRole::Number)),
    ("character", Some(SyntaxRole::String)),
    ("comment", Some(SyntaxRole::Comment)),
    ("constant", Some(SyntaxRole::Number)),
    ("constant.builtin", Some(SyntaxRole::Number)),
    ("constructor", Some(SyntaxRole::Type)),
    ("escape", Some(SyntaxRole::String)),
    ("float", Some(SyntaxRole::Number)),
    ("function", Some(SyntaxRole::Function)),
    ("function.builtin", Some(SyntaxRole::Function)),
    ("function.macro", Some(SyntaxRole::Function)),
    ("function.method", Some(SyntaxRole::Function)),
    ("keyword", Some(SyntaxRole::Keyword)),
    ("label", None),
    ("number", Some(SyntaxRole::Number)),
    ("operator", Some(SyntaxRole::Operator)),
    ("property", None),
    ("punctuation", Some(SyntaxRole::Punctuation)),
    ("string", Some(SyntaxRole::String)),
    ("string.special", Some(SyntaxRole::String)),
    ("tag", Some(SyntaxRole::Keyword)),
    ("type", Some(SyntaxRole::Type)),
    ("type.builtin", Some(SyntaxRole::Type)),
    ("variable", None),
    ("variable.builtin", Some(SyntaxRole::Keyword)),
    ("variable.parameter", None),
];

/// The grammar a code fence's language name selects, by canonical name.
fn canonical(lang: &str) -> Option<&'static str> {
    Some(match lang.trim().to_lowercase().as_str() {
        "rust" | "rs" => "rust",
        "python" | "py" | "python3" => "python",
        "javascript" | "js" | "mjs" | "cjs" | "jsx" => "javascript",
        "typescript" | "ts" | "mts" | "cts" => "typescript",
        "tsx" => "tsx",
        "go" | "golang" => "go",
        "c" | "h" => "c",
        "cpp" | "c++" | "cc" | "cxx" | "hpp" | "hh" => "cpp",
        "java" => "java",
        "bash" | "sh" | "shell" | "zsh" | "console" => "bash",
        "json" | "jsonc" => "json",
        _ => return None,
    })
}

/// Build the highlight configuration of a canonical language name.
fn configuration(name: &str) -> Option<HighlightConfiguration> {
    let js = tree_sitter_javascript::HIGHLIGHT_QUERY;
    let (language, highlights, injections, locals): (tree_sitter::Language, String, &str, String) =
        match name {
            "rust" => (
                tree_sitter_rust::LANGUAGE.into(),
                tree_sitter_rust::HIGHLIGHTS_QUERY.to_string(),
                tree_sitter_rust::INJECTIONS_QUERY,
                String::new(),
            ),
            "python" => (
                tree_sitter_python::LANGUAGE.into(),
                tree_sitter_python::HIGHLIGHTS_QUERY.to_string(),
                "",
                String::new(),
            ),
            "javascript" => (
                tree_sitter_javascript::LANGUAGE.into(),
                format!("{}\n{js}", tree_sitter_javascript::JSX_HIGHLIGHT_QUERY),
                "",
                tree_sitter_javascript::LOCALS_QUERY.to_string(),
            ),
            "typescript" | "tsx" => (
                if name == "tsx" {
                    tree_sitter_typescript::LANGUAGE_TSX.into()
                } else {
                    tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into()
                },
                format!("{}\n{js}", tree_sitter_typescript::HIGHLIGHTS_QUERY),
                "",
                format!(
                    "{}\n{}",
                    tree_sitter_typescript::LOCALS_QUERY,
                    tree_sitter_javascript::LOCALS_QUERY
                ),
            ),
            "go" => (
                tree_sitter_go::LANGUAGE.into(),
                tree_sitter_go::HIGHLIGHTS_QUERY.to_string(),
                "",
                String::new(),
            ),
            "c" => (
                tree_sitter_c::LANGUAGE.into(),
                tree_sitter_c::HIGHLIGHT_QUERY.to_string(),
                "",
                String::new(),
            ),
            "cpp" => (
                tree_sitter_cpp::LANGUAGE.into(),
                format!(
                    "{}\n{}",
                    tree_sitter_cpp::HIGHLIGHT_QUERY,
                    tree_sitter_c::HIGHLIGHT_QUERY
                ),
                "",
                String::new(),
            ),
            "java" => (
                tree_sitter_java::LANGUAGE.into(),
                tree_sitter_java::HIGHLIGHTS_QUERY.to_string(),
                "",
                String::new(),
            ),
            "bash" => (
                tree_sitter_bash::LANGUAGE.into(),
                tree_sitter_bash::HIGHLIGHT_QUERY.to_string(),
                "",
                String::new(),
            ),
            "json" => (
                tree_sitter_json::LANGUAGE.into(),
                tree_sitter_json::HIGHLIGHTS_QUERY.to_string(),
                "",
                String::new(),
            ),
            _ => return None,
        };
    let mut config =
        match HighlightConfiguration::new(language, name, &highlights, injections, &locals) {
            Ok(config) => config,
            Err(error) => {
                tracing::warn!(language = name, %error, "highlight query does not compile");
                return None;
            }
        };
    let names: Vec<&str> = CAPTURES.iter().map(|(name, _)| *name).collect();
    config.configure(&names);
    Some(config)
}

/// The configuration of every language, built on first use.
fn configurations() -> &'static HashMap<&'static str, HighlightConfiguration> {
    static CONFIGS: OnceLock<HashMap<&'static str, HighlightConfiguration>> = OnceLock::new();
    CONFIGS.get_or_init(|| {
        [
            "rust",
            "python",
            "javascript",
            "typescript",
            "tsx",
            "go",
            "c",
            "cpp",
            "java",
            "bash",
            "json",
        ]
        .into_iter()
        .filter_map(|name| configuration(name).map(|config| (name, config)))
        .collect()
    })
}

/// Highlight `code` written in `lang`.
///
/// # Arguments
///
/// * `code` - the code, as shown.
/// * `lang` - the language name after the code fence, e.g. `rust` or `sh`.
///
/// # Returns
///
/// Spans covering all of `code` in order, or `None` when the language is
/// not supported, the code is longer than [`MAX_HIGHLIGHT_BYTES`], or the
/// grammar fails on it.
#[must_use]
pub fn highlight(code: &str, lang: &str) -> Option<Vec<HighlightSpan>> {
    if code.len() > MAX_HIGHLIGHT_BYTES {
        return None;
    }
    let config = configurations().get(canonical(lang)?)?;
    let mut highlighter = Highlighter::new();
    let events = highlighter
        .highlight(config, code.as_bytes(), None, None, |_| None)
        .ok()?;
    let mut spans: Vec<HighlightSpan> = Vec::new();
    let mut stack: Vec<Option<SyntaxRole>> = Vec::new();
    for event in events {
        match event.ok()? {
            HighlightEvent::HighlightStart(highlight) => {
                stack.push(CAPTURES.get(highlight.0).and_then(|(_, role)| *role));
            }
            HighlightEvent::HighlightEnd => {
                stack.pop();
            }
            HighlightEvent::Source { start, end } => {
                // The innermost capture with a role wins.
                let role = stack.iter().rev().find_map(|role| *role);
                match spans.last_mut() {
                    Some(last) if last.role == role && last.range.end == start => {
                        last.range.end = end;
                    }
                    _ => spans.push(HighlightSpan {
                        range: start..end,
                        role,
                    }),
                }
            }
        }
    }
    Some(spans)
}
