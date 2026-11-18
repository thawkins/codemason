//! Tests for `codemason_app_core::syntax_highlight`: code blocks become role
//! spans that cover the whole code.

use codemason_app_core::syntax_highlight::{
    HighlightSpan, MAX_HIGHLIGHT_BYTES, SyntaxRole, highlight,
};

/// The text of each span with its role.
fn pieces<'a>(code: &'a str, spans: &[HighlightSpan]) -> Vec<(&'a str, Option<SyntaxRole>)> {
    spans
        .iter()
        .map(|s| (&code[s.range.clone()], s.role))
        .collect()
}

/// Whether `code` highlighted as `lang` has `text` in `role`.
fn has(code: &str, lang: &str, text: &str, role: SyntaxRole) -> bool {
    let spans = highlight(code, lang).expect("supported language");
    pieces(code, &spans)
        .iter()
        .any(|(piece, r)| piece.contains(text) && *r == Some(role))
}

#[test]
fn test_syntax_highlight_spans_cover_the_code_in_order() {
    let code = "fn main() {\n    let x = \"hi\"; // greet\n}\n";
    let spans = highlight(code, "rust").expect("rust");
    let mut end = 0;
    for span in &spans {
        assert_eq!(span.range.start, end, "{spans:?}");
        end = span.range.end;
    }
    assert_eq!(end, code.len());
}

#[test]
fn test_syntax_highlight_roles_per_language() {
    let rust = "fn main() {\n    let x = \"hi\"; // greet\n    let n = 42;\n}\n";
    assert!(has(rust, "rust", "fn", SyntaxRole::Keyword));
    assert!(has(rust, "rs", "\"hi\"", SyntaxRole::String));
    assert!(has(rust, "rust", "// greet", SyntaxRole::Comment));
    assert!(has(rust, "rust", "main", SyntaxRole::Function));
    assert!(has(rust, "rust", "42", SyntaxRole::Number));

    let py = "def add(a, b):\n    return a + b  # sum\n";
    assert!(has(py, "py", "def", SyntaxRole::Keyword));
    assert!(has(py, "python", "# sum", SyntaxRole::Comment));

    let ts = "const x: number = 1;\n";
    assert!(has(ts, "ts", "const", SyntaxRole::Keyword));
    assert!(has(ts, "typescript", "number", SyntaxRole::Type));

    let sh = "echo \"hello\" # say it\n";
    assert!(has(sh, "sh", "# say it", SyntaxRole::Comment));
    assert!(has(sh, "bash", "\"hello\"", SyntaxRole::String));

    let json = "{\"a\": 1, \"b\": true}";
    assert!(has(json, "json", "1", SyntaxRole::Number));

    for (lang, code) in [
        ("go", "package main\nfunc main() {}\n"),
        ("c", "int main(void) { return 0; }\n"),
        ("cpp", "class A { public: int x; };\n"),
        ("java", "class A { void f() {} }\n"),
        ("js", "function f() { return 1; }\n"),
        ("tsx", "const a = <div>hi</div>;\n"),
    ] {
        assert!(
            highlight(code, lang).is_some_and(|s| s.iter().any(|s| s.role.is_some())),
            "{lang}"
        );
    }
}

#[test]
fn test_syntax_highlight_refuses_unknown_languages_and_huge_code() {
    assert_eq!(highlight("x", "cobol"), None);
    assert_eq!(highlight("x", ""), None);
    let huge = "a".repeat(MAX_HIGHLIGHT_BYTES + 1);
    assert_eq!(highlight(&huge, "rust"), None);
}
