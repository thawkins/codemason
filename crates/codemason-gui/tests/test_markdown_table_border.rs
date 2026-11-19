//! The border of a markdown table in the transcript is drawn at
//! `TABLE_BORDER_SCALE` times egui's noninteractive stroke, and markdown
//! without a table keeps the normal width.

use codemason_agent::message::Message;
use codemason_gui::palette::Palette;
use codemason_gui::panels::messages::{MessagesView, TABLE_BORDER_SCALE};
use egui_kittest::Harness;

#[path = "support/mod.rs"]
mod support;

/// The stroke widths of every rectangle drawn for an assistant reply `text`,
/// and egui's noninteractive stroke width.
fn rect_stroke_widths(text: &str) -> (Vec<f32>, f32) {
    let mut app = support::make_app();
    app.session.messages = vec![Message::assistant_text("s1", text)];
    let mut harness = Harness::builder()
        .with_size(egui::vec2(800.0, 600.0))
        .with_max_steps(64)
        .build_ui_state(
            |ui, state: &mut (codemason_app_core::app::App, MessagesView)| {
                let palette = Palette::from_tokens(&state.0.themes.resolve(&state.0.theme, true));
                state.1.show(ui, &state.0, &palette);
            },
            (app, MessagesView::default()),
        );
    harness.run();
    let base = harness
        .ctx
        .global_style()
        .visuals
        .widgets
        .noninteractive
        .bg_stroke
        .width;
    let widths = harness
        .output()
        .shapes
        .iter()
        .filter_map(|clipped| match &clipped.shape {
            egui::Shape::Rect(rect) => Some(rect.stroke.width),
            _ => None,
        })
        .collect();
    (widths, base)
}

#[test]
fn test_markdown_table_border_is_doubled_only_for_tables() {
    let (with_table, base) = rect_stroke_widths("| a | b |\n|---|---|\n| 1 | 2 |\n");
    let wide = base * TABLE_BORDER_SCALE;
    assert!(
        with_table.iter().any(|w| (w - wide).abs() < 0.01),
        "no {wide} pt table border among {with_table:?}"
    );
    let (without, _) = rect_stroke_widths("Plain **markdown** with no table.");
    assert!(
        !without.iter().any(|w| (w - wide).abs() < 0.01),
        "a {wide} pt border without a table: {without:?}"
    );
}
