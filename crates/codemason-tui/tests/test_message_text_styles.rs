//! Style constants moved into the ratatui-free `message_text` / `message_lines`
//! modules must paint exactly like the terminal theme they replaced.

use codemason_agent::message::{Message, MessagePart, Role};
use codemason_tui::adapters::{to_ratatui_line, to_ratatui_style};
use codemason_tui::message_lines::messages_to_lines;
use codemason_tui::message_text::{ICON_ATTACHMENT, THINK_STYLE, render_agent_notice_lines};
use codemason_tui::theme;
use ratatui::style::{Color, Modifier, Style};
use std::collections::HashMap;

#[test]
fn test_think_style_matches_theme_think() {
    assert_eq!(to_ratatui_style(&THINK_STYLE), theme::think());
}

#[test]
fn test_icon_attachment_matches_theme() {
    assert_eq!(ICON_ATTACHMENT, theme::ICON_ATTACHMENT);
}

#[test]
fn test_agent_notice_style_is_bold_yellow() {
    let expected = Style::default()
        .fg(Color::Yellow)
        .add_modifier(Modifier::BOLD);
    for line in render_agent_notice_lines("Agent Notice\nitem") {
        for span in to_ratatui_line(&line).spans {
            assert_eq!(span.style, expected);
        }
    }
}

#[test]
fn test_reasoning_lines_use_theme_think() {
    let msg = Message::new(
        "s1",
        Role::Assistant,
        vec![MessagePart::Reasoning {
            text: "pondering".to_string(),
            signature: None,
        }],
    );
    let lines = messages_to_lines(&[msg], &HashMap::new(), &HashMap::new(), "/p");
    let line = lines
        .iter()
        .map(to_ratatui_line)
        .find(|l| l.to_string().contains("pondering"))
        .expect("reasoning line rendered");
    assert_eq!(line.spans[0].style, theme::think());
}
