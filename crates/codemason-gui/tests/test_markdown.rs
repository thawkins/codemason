//! Tests for laying out core markdown output with
//! `codemason_gui::styled_text::styled_to_layout_job`.

use std::path::Path;

use codemason_app_core::markdown::markdown_to_styled;
use codemason_app_core::styled::{StyledLine, StyledSpan, TextStyle, ThemeColor};
use codemason_app_core::theme_mode::ThemeMode;
use codemason_gui::palette::palette_for;
use codemason_gui::styled_text::styled_to_layout_job;
use egui::FontFamily;
use egui::text::{LayoutJob, LayoutSection};

/// The text `section` covers in `job`.
fn section_text<'a>(job: &'a LayoutJob, section: &LayoutSection) -> &'a str {
    &job.text[section.byte_range.start.0..section.byte_range.end.0]
}

#[test]
fn test_markdown_code_block_is_monospace() {
    let lines = markdown_to_styled("intro\n\n```\nlet x = 1;\n```\n", Path::new("/"), 80);
    let palette = palette_for(ThemeMode::Default);
    let job = styled_to_layout_job(&lines, &palette, 400.0);

    let family_of = |needle: &str| -> FontFamily {
        let section = job
            .sections
            .iter()
            .find(|s| section_text(&job, s).contains(needle))
            .expect("section present");
        section.format.font_id.family.clone()
    };
    assert_eq!(family_of("let x = 1;"), FontFamily::Monospace);
    assert_eq!(family_of("intro"), FontFamily::Proportional);
    assert!((job.wrap.max_width - 400.0).abs() < f32::EPSILON);
}

#[test]
fn test_styled_to_layout_job_joins_lines_and_maps_style() {
    let lines = vec![
        StyledLine::from(StyledSpan::styled(
            "err",
            TextStyle::new()
                .fg(ThemeColor::Error)
                .bg(ThemeColor::Black)
                .italic()
                .underline(),
        )),
        StyledLine::default(),
        StyledLine::plain("plain"),
    ];
    let palette = palette_for(ThemeMode::Default);
    let job = styled_to_layout_job(&lines, &palette, 300.0);

    assert_eq!(job.text, "err\n\nplain");
    let err = &job.sections[0].format;
    assert_eq!(err.color, palette.color(ThemeColor::Error));
    assert_eq!(err.background, palette.color(ThemeColor::Black));
    assert!(err.italics);
    assert_eq!(err.underline.color, palette.color(ThemeColor::Error));
    let plain = job
        .sections
        .iter()
        .find(|s| section_text(&job, s).ends_with("plain"))
        .expect("plain section");
    assert_eq!(plain.format.color, palette.color(ThemeColor::Text));
}
