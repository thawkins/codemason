//! Mermaid diagrams in the transcript: a ```` ```mermaid ```` block in a
//! reply is drawn as the diagram, toggles to its source, opens at full
//! size, and falls back to its source while streaming or when it does not
//! parse.

#[path = "support/mod.rs"]
mod support;

use codemason_agent::message::Message;
use codemason_app_core::app::App;
use codemason_app_core::theme_tokens::{ThemeCatalog, ThemeSelection};
use codemason_gui::Shell;
use codemason_gui::dialogs::image_view::IMAGE_VIEW_TITLE;
use codemason_gui::palette::Palette;
use codemason_gui::panels::mermaid::{
    DIAGRAM_NAME, SHOW_DIAGRAM, SHOW_SOURCE, is_mermaid, mermaid_theme,
};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;

/// A reply holding one flowchart.
const FLOWCHART: &str =
    "Here is the flow:\n\n```mermaid\nflowchart TD\n  A[Prompt] --> B[Reply]\n```\n";

fn shell_harness(app: App) -> Harness<'static, Shell> {
    let harness = Harness::builder()
        .with_size(egui::vec2(1000.0, 700.0))
        .with_max_steps(64)
        .build_ui_state(|ui, shell: &mut Shell| shell.show(ui), Shell::new(app));
    egui_extras::install_image_loaders(&harness.ctx);
    harness
}

fn reply(text: &str) -> App {
    let mut app = support::make_app();
    app.session.messages = vec![Message::assistant_text("s1", text)];
    app
}

#[tokio::test]
async fn test_mermaid_block_is_drawn_and_toggles_to_its_source() {
    let mut harness = shell_harness(reply(FLOWCHART));
    harness.run();
    assert!(
        harness.query_by_label(DIAGRAM_NAME).is_some(),
        "the diagram"
    );
    assert!(
        harness.query_by_label_contains("A[Prompt]").is_none(),
        "no source"
    );

    harness.get_by_label(SHOW_SOURCE).click();
    harness.run();
    assert!(harness.query_by_label(DIAGRAM_NAME).is_none());
    assert!(
        harness.query_by_label_contains("A[Prompt]").is_some(),
        "the source"
    );

    harness.get_by_label(SHOW_DIAGRAM).click();
    harness.run();
    assert!(harness.query_by_label(DIAGRAM_NAME).is_some());
}

#[tokio::test]
async fn test_mermaid_diagram_click_opens_the_image_view() {
    let mut harness = shell_harness(reply(FLOWCHART));
    harness.run();
    harness.get_by_label(DIAGRAM_NAME).click();
    harness.run();
    assert!(
        harness
            .query_by_role_and_label(egui::accesskit::Role::Dialog, IMAGE_VIEW_TITLE)
            .is_some()
    );
}

#[tokio::test]
async fn test_mermaid_streaming_reply_shows_the_source() {
    let mut app = reply(FLOWCHART);
    app.session.is_processing = true;
    let mut harness = shell_harness(app);
    harness.run();
    assert!(harness.query_by_label(DIAGRAM_NAME).is_none());
    assert!(harness.query_by_label_contains("A[Prompt]").is_some());
}

#[tokio::test]
async fn test_mermaid_parse_error_names_it_and_shows_the_source() {
    let mut harness = shell_harness(reply("```mermaid\nflowchart TD\n  A --> \n  ][\n```\n"));
    harness.run();
    assert!(harness.query_by_label(DIAGRAM_NAME).is_none(), "no diagram");
    assert!(
        harness
            .query_by_label_contains("could not be drawn")
            .is_some(),
        "the error is named"
    );
    assert!(
        harness
            .query_all_by_label_contains("flowchart TD")
            .next()
            .is_some(),
        "the source is shown"
    );
}

#[test]
fn test_mermaid_language_tag_and_theme_colours() {
    assert!(is_mermaid(Some("mermaid")));
    assert!(is_mermaid(Some(" Mermaid ")));
    assert!(!is_mermaid(Some("rust")));
    assert!(!is_mermaid(None));

    let tokens = ThemeCatalog::builtin().resolve(&ThemeSelection::System, true);
    let palette = Palette::from_tokens(&tokens);
    let theme = mermaid_theme(&palette);
    let m3 = &palette.m3;
    let hex =
        |c: codemason_app_core::theme_tokens::Rgb| format!("#{:02x}{:02x}{:02x}", c.0, c.1, c.2);
    assert_eq!(theme.primary_color, hex(m3.surface_container_high));
    assert_eq!(theme.text_color, hex(m3.on_surface));
    assert_eq!(theme.background, hex(m3.surface));
}
