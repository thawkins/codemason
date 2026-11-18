//! Tests for plugin-contributed slash commands in the TUI (spec `plugins`
//! FR-031): prompt commands inject their body as a user turn, inline commands
//! report that they need the plugin host, and `/help` lists both.

use codemason_agent::plugin::PluginCommand;
use codemason_app_core::app::App;

mod support;

use support::make_app;

fn prompt_command(trigger: &str, name: &str, prompt: &str) -> PluginCommand {
    PluginCommand {
        plugin_id: "commit-commands".to_string(),
        name: name.to_string(),
        trigger: trigger.to_string(),
        description: "Create a git commit".to_string(),
        prompt: Some(prompt.to_string()),
    }
}

fn inline_command(trigger: &str, name: &str) -> PluginCommand {
    PluginCommand {
        plugin_id: "commit-commands".to_string(),
        name: name.to_string(),
        trigger: trigger.to_string(),
        description: "Runs in the sandbox".to_string(),
        prompt: None,
    }
}

fn last_text(app: &App) -> String {
    app.session
        .messages
        .last()
        .map(|m| m.text_content())
        .unwrap_or_default()
}

#[tokio::test]
async fn prompt_plugin_command_is_injected_as_a_user_turn() {
    let mut app = make_app();
    app.session.session_id = Some("s".to_string());
    app.plugin_commands = vec![prompt_command(
        "commit",
        "commit",
        "Commit the staged changes.",
    )];

    app.execute_slash_command("/commit").await;

    // The invocation is shown as a user message and the command is handled by
    // the plugin path (not reported as an unknown command).
    assert_eq!(app.status, "plugin command /commit");
    let text = last_text(&app);
    assert!(
        text.contains("/commit"),
        "user message records the invocation, got: {text}"
    );
    assert!(
        !text.contains("Unknown command"),
        "plugin command must not fall through to the unknown-command arm"
    );
}

#[tokio::test]
async fn prompt_plugin_command_substitutes_arguments_and_uses_namespaced_trigger() {
    let mut app = make_app();
    app.session.session_id = Some("s".to_string());
    // A namespaced trigger (a colliding bare name) is what the surface holds.
    app.plugin_commands = vec![prompt_command(
        "plugin:commit-commands:commit",
        "commit",
        "Message: $ARGUMENTS",
    )];

    app.execute_slash_command("/plugin:commit-commands:commit fix the bug")
        .await;

    assert_eq!(app.status, "plugin command /plugin:commit-commands:commit");
    let text = last_text(&app);
    assert!(
        text.contains("/plugin:commit-commands:commit fix the bug"),
        "the recorded user turn echoes the trigger and args, got: {text}"
    );
}

#[tokio::test]
async fn inline_plugin_command_reports_it_needs_the_plugin_host() {
    let mut app = make_app();
    app.session.session_id = Some("s".to_string());
    app.plugin_commands = vec![inline_command("clean_gone", "clean_gone")];

    app.execute_slash_command("/clean_gone").await;

    assert_eq!(
        app.status,
        "plugin command /clean_gone requires the plugin host"
    );
    let text = last_text(&app);
    let flat = text.split_whitespace().collect::<Vec<_>>().join(" ");
    assert!(
        flat.contains("is an inline plugin command"),
        "inline command explains it needs the plugin host, got: {text}"
    );
}

#[tokio::test]
async fn help_lists_plugin_commands() {
    let mut app = make_app();
    app.session.session_id = Some("s".to_string());
    app.plugin_commands = vec![prompt_command("commit", "commit", "Commit.")];

    app.execute_slash_command("/help").await;

    let text = last_text(&app);
    assert!(
        text.contains("Plugin commands:"),
        "/help must add a plugin-command section, got: {text}"
    );
    assert!(text.contains("/commit"), "/help must list the command");
}

#[tokio::test]
async fn prompt_plugin_command_marks_the_session_busy_at_dispatch() {
    let mut app = make_app();
    app.session.session_id = Some("s".to_string());
    app.plugin_commands = vec![prompt_command("commit", "commit", "Commit.")];

    app.execute_slash_command("/commit").await;

    // Busy from dispatch, not from the first `MessageStart`: otherwise the
    // input queue dispatches the next queued entry over the starting turn.
    assert!(app.session.is_processing);
}

#[tokio::test]
async fn inline_plugin_command_leaves_the_session_idle() {
    let mut app = make_app();
    app.session.session_id = Some("s".to_string());
    app.plugin_commands = vec![inline_command("clean_gone", "clean_gone")];

    app.execute_slash_command("/clean_gone").await;

    assert!(!app.session.is_processing);
}

#[tokio::test]
async fn prompt_plugin_command_reports_a_turn_that_fails_to_start() {
    let mut app = make_app();
    // No such session in storage, so the turn cannot start.
    app.session.session_id = Some("missing-session".to_string());
    app.plugin_commands = vec![prompt_command("commit", "commit", "Commit.")];
    let mut events = app.event_bus.subscribe();

    app.execute_slash_command("/commit").await;

    // The failure must end the busy state through the ordinary error event.
    let error = tokio::time::timeout(std::time::Duration::from_secs(10), async {
        loop {
            if let Ok(codemason_agent::event::Event::AgentError { session_id, error }) =
                events.recv().await
            {
                break (session_id, error);
            }
        }
    })
    .await
    .expect("an AgentError event for the failed turn");
    assert_eq!(error.0, "missing-session");
    assert!(error.1.contains("plugin command failed"), "{}", error.1);
}

#[tokio::test]
async fn prompt_plugin_command_reports_a_failed_turn_exactly_once() {
    let mut app = make_app();
    // The session exists, so the turn gets as far as preparing the model
    // client, which fails here (no usable provider) and reports that itself.
    app.session.session_id = Some("s-once".to_string());
    app.storage
        .create_session("s-once", "/tmp/plugin-command-once")
        .expect("create_session");
    app.plugin_commands = vec![prompt_command("commit", "commit", "Commit.")];
    let mut events = app.event_bus.subscribe();

    app.execute_slash_command("/commit").await;

    // A second terminal error would clear the busy state of whatever turn the
    // first one let the queue start.
    // Wait for the first report however long the turn takes to fail, then
    // give a duplicate a moment to arrive.
    let mut errors = 0;
    let mut deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(120);
    while let Ok(event) = tokio::time::timeout_at(deadline, events.recv()).await {
        if let Ok(codemason_agent::event::Event::AgentError { session_id, .. }) = event
            && session_id == "s-once"
        {
            errors += 1;
            deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(1);
        }
    }
    assert_eq!(errors, 1, "the failed turn must be reported once");
}
