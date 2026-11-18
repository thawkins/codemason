//! Transcript preference tests: parsing, display, defaults, slash commands.

use codemason_app_core::transcript_prefs::{ThinkingDisplay, TranscriptPrefs, Verbosity};

mod support;

#[test]
fn test_transcript_prefs_round_trip() {
    for v in [Verbosity::Low, Verbosity::Medium, Verbosity::High] {
        assert_eq!(v.to_string().parse::<Verbosity>(), Ok(v));
    }
    for t in [
        ThinkingDisplay::Collapsed,
        ThinkingDisplay::Expanded,
        ThinkingDisplay::Hidden,
    ] {
        assert_eq!(t.to_string().parse::<ThinkingDisplay>(), Ok(t));
    }
}

#[test]
fn test_transcript_prefs_defaults() {
    let prefs = TranscriptPrefs::default();
    assert_eq!(prefs.verbosity, Verbosity::Low);
    assert_eq!(prefs.thinking, ThinkingDisplay::Collapsed);
}

#[test]
fn test_transcript_prefs_unknown_value_rejected() {
    let err = "loud".parse::<Verbosity>().expect_err("loud is not valid");
    assert!(err.contains("low, medium or high"), "{err}");
    assert!("loud".parse::<ThinkingDisplay>().is_err());
}

#[test]
fn test_transcript_prefs_from_config() {
    let mut config = codemason_config::Config::default();
    config.verbosity = Some("high".to_string());
    config.thinking_display = Some("bogus".to_string());
    let prefs = TranscriptPrefs::from_config(&config);
    assert_eq!(prefs.verbosity, Verbosity::High);
    assert_eq!(prefs.thinking, ThinkingDisplay::Collapsed);
}

#[test]
fn test_transcript_prefs_registered_in_slash_commands() {
    for trigger in ["verbosity", "thinking-display", "thinking"] {
        assert!(
            codemason_app_core::app::SLASH_COMMANDS
                .iter()
                .any(|cmd| cmd.trigger == trigger),
            "{trigger}"
        );
    }
}

#[tokio::test]
async fn test_transcript_prefs_verbosity_command() {
    let mut app = support::make_app();
    app.execute_slash_command("/verbosity high").await;
    assert_eq!(app.transcript_prefs.verbosity, Verbosity::High);
    assert_eq!(app.status, "verbosity: high");

    app.execute_slash_command("/verbosity loud").await;
    assert_eq!(app.transcript_prefs.verbosity, Verbosity::High);
    assert!(app.status.contains("low, medium or high"), "{}", app.status);
}

#[tokio::test]
async fn test_transcript_prefs_thinking_display_command() {
    let mut app = support::make_app();
    app.execute_slash_command("/thinking-display show").await;
    assert_eq!(app.transcript_prefs.thinking, ThinkingDisplay::Expanded);
    app.execute_slash_command("/thinking-display hide").await;
    assert_eq!(app.transcript_prefs.thinking, ThinkingDisplay::Hidden);
    app.execute_slash_command("/thinking-display collapse")
        .await;
    assert_eq!(app.transcript_prefs.thinking, ThinkingDisplay::Collapsed);
    app.execute_slash_command("/thinking-display nope").await;
    assert!(
        app.status.contains("show, hide or collapse"),
        "{}",
        app.status
    );
    assert_eq!(app.transcript_prefs.thinking, ThinkingDisplay::Collapsed);
}

#[test]
fn test_new_app_starts_with_the_startup_prefs() {
    // No launch configuration named preferences in this process, so a new
    // App starts with the defaults, whatever the developer's config says.
    assert_eq!(
        codemason_app_core::transcript_prefs::startup(),
        TranscriptPrefs::default()
    );
    assert_eq!(
        support::make_app().transcript_prefs,
        TranscriptPrefs::default()
    );
}
