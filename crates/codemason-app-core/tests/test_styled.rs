//! Tests for `codemason_app_core::styled`.

use codemason_app_core::styled::{StyledLine, StyledSpan, TextStyle, ThemeColor};

#[test]
fn test_styled_line_plain_text_roundtrip() {
    assert_eq!(StyledLine::plain("hi").text(), "hi");
}

#[test]
fn test_text_style_builders_set_fields() {
    let s = TextStyle::new()
        .fg(ThemeColor::Cyan)
        .bg(ThemeColor::Indexed(236))
        .bold()
        .italic()
        .underline()
        .dim();
    assert_eq!(s.fg, Some(ThemeColor::Cyan));
    assert_eq!(s.bg, Some(ThemeColor::Indexed(236)));
    assert!(s.bold && s.italic && s.underline && s.dim && !s.monospace);
    assert_eq!(TextStyle::new(), TextStyle::default());
}

#[test]
fn test_styled_line_raw_drops_newlines_and_empty() {
    assert!(StyledLine::raw("").spans.is_empty());
    let l = StyledLine::raw("a\nb");
    assert_eq!(l.spans.len(), 2);
    assert_eq!(l.text(), "ab");
}

#[test]
fn test_styled_line_from_spans() {
    let red = TextStyle::new().fg(ThemeColor::Red);
    let line = StyledLine::from(vec![
        StyledSpan::raw("x"),
        StyledSpan::styled("y", red.clone()),
    ]);
    assert_eq!(line.spans[1].style, red);
    assert_eq!(StyledLine::from(StyledSpan::raw("z")).text(), "z");
}

#[test]
fn test_styled_line_display_is_plain_text() {
    let line = StyledLine::from(vec![
        StyledSpan::styled("a", TextStyle::new().bold()),
        StyledSpan::raw("b"),
    ]);
    assert_eq!(line.to_string(), "ab");
}
