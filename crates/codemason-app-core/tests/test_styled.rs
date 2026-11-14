//! Tests for `codemason_app_core::styled`.

use codemason_app_core::styled::StyledLine;

#[test]
fn test_styled_line_plain_text_roundtrip() {
    assert_eq!(StyledLine::plain("hi").text(), "hi");
}
