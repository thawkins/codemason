//! User-defined slash commands (`commands/*.md`) on the app surface: dispatch,
//! the slash menu and `/help`.

use std::path::PathBuf;

use codemason_agent::user_command::UserCommand;
use support::make_app;

mod support;

fn greet() -> UserCommand {
    UserCommand {
        name: "greet".to_string(),
        description: "Say hello".to_string(),
        prompt: "Say hello to $1.".to_string(),
        path: PathBuf::from("/nonexistent/greet.md"),
    }
}

#[tokio::test]
async fn test_user_command_is_sent_as_a_user_turn() {
    let mut app = make_app();
    app.session.session_id = Some("test-session".to_string());
    app.user_commands = vec![greet()];

    app.execute_slash_command("/greet world").await;

    let last = app.session.messages.last().expect("a user message");
    assert_eq!(last.role, codemason_types::message::Role::User);
    assert_eq!(last.text_content(), "/greet world");
    assert_eq!(app.status, "user command /greet");
}

#[tokio::test]
async fn test_unknown_command_is_not_treated_as_user_command() {
    let mut app = make_app();
    app.session.session_id = Some("test-session".to_string());
    app.user_commands = vec![greet()];

    app.execute_slash_command("/nosuchcommand").await;

    assert_ne!(app.status, "user command /nosuchcommand");
}

#[test]
fn test_user_command_appears_in_slash_menu() {
    let mut app = make_app();
    app.user_commands = vec![greet()];
    app.session.input = "/gre".to_string();
    app.update_slash_menu();

    let menu = app.slash_menu.as_ref().expect("slash menu opens");
    let entry = menu
        .matches
        .iter()
        .find(|m| m.trigger == "greet")
        .expect("menu lists the user command");
    assert_eq!(entry.description, "Say hello (user command)");
}

#[tokio::test]
async fn test_help_lists_user_commands() {
    let mut app = make_app();
    app.session.session_id = Some("test-session".to_string());
    app.user_commands = vec![greet()];

    app.execute_slash_command("/help").await;

    let text = app
        .session
        .messages
        .last()
        .expect("help creates a message")
        .text_content();
    assert!(text.contains("User commands:"), "{text}");
    assert!(text.contains("/greet"), "{text}");
}
