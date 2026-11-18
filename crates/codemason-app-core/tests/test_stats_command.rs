//! `/stats` slash-command tests: registration, parsing, report, errors.

use codemason_app_core::usage_stats::{StatsArgs, parse_stats_args, usage_stats_report};
use codemason_storage::storage::RunCostSummaryRow;
use support::make_app;

mod support;

fn seed(storage: &codemason_storage::Storage, session: &str, dir: &str, model: &str, cost: f64) {
    storage
        .create_session(session, dir)
        .expect("create_session");
    storage
        .create_run_cost_summary(&RunCostSummaryRow {
            id: format!("rc-{session}-{model}"),
            session_id: session.to_string(),
            model_id: model.to_string(),
            input_tokens: 1_000,
            output_tokens: 200,
            total_cost_usd: cost,
            duration_ms: 65_000,
            created_at: chrono::Utc::now().to_rfc3339(),
        })
        .expect("create_run_cost_summary");
}

fn last_text(app: &codemason_app_core::app::App) -> String {
    app.session
        .messages
        .last()
        .expect("command should create a message")
        .text_content()
}

#[test]
fn test_stats_registered_in_slash_commands() {
    let def = codemason_app_core::app::SLASH_COMMANDS
        .iter()
        .find(|cmd| cmd.trigger == "stats")
        .expect("stats must be registered in SLASH_COMMANDS");
    for flag in ["--days", "--project", "--json"] {
        assert!(def.description.contains(flag), "{}", def.description);
    }
}

#[test]
fn test_parse_stats_args_accepts_all_forms() {
    assert_eq!(parse_stats_args([]), Ok(StatsArgs::default()));
    let expected = StatsArgs {
        days: Some(7),
        project: true,
        json: true,
    };
    assert_eq!(
        parse_stats_args(["--days", "7", "--project", "--json"]),
        Ok(expected)
    );
    assert_eq!(
        parse_stats_args(["--JSON", "--days=7", "--project"]),
        Ok(expected)
    );
}

#[test]
fn test_parse_stats_args_rejects_bad_input() {
    assert_eq!(parse_stats_args(["--days"]), Err("--days".to_string()));
    assert_eq!(parse_stats_args(["--days=0"]), Err("--days".to_string()));
    assert_eq!(parse_stats_args(["--days", "x"]), Err("--days".to_string()));
    assert_eq!(parse_stats_args(["wat"]), Err("wat".to_string()));
}

#[test]
fn test_report_table_lists_models_and_total() {
    let storage = codemason_storage::Storage::open_in_memory().expect("storage");
    seed(&storage, "s1", "/tmp/stats-a", "model-x", 0.5);
    seed(&storage, "s2", "/tmp/stats-b", "model-y", 1.25);

    let report = usage_stats_report(
        &storage,
        &StatsArgs::default(),
        std::path::Path::new("/tmp/stats-a"),
    )
    .expect("report");
    assert!(report.contains("all time, all projects"), "{report}");
    assert!(
        report.contains("| model-y | 1 | 1 | 1000 | 200 | 1.2500 | 1m 05s |"),
        "{report}"
    );
    assert!(
        report.contains("| Total | 2 | 2 | 2000 | 400 | 1.7500 | 2m 10s |"),
        "{report}"
    );
}

#[test]
fn test_report_project_filter_and_json() {
    let storage = codemason_storage::Storage::open_in_memory().expect("storage");
    seed(&storage, "s1", "/tmp/stats-a", "model-x", 0.5);
    seed(&storage, "s2", "/tmp/stats-b", "model-y", 1.25);

    let args = StatsArgs {
        days: Some(30),
        project: true,
        json: true,
    };
    let report =
        usage_stats_report(&storage, &args, std::path::Path::new("/tmp/stats-a")).expect("report");
    let value: serde_json::Value = serde_json::from_str(&report).expect("valid JSON");
    assert_eq!(value["days"], 30);
    assert_eq!(value["project"], "/tmp/stats-a");
    assert_eq!(value["models"].as_array().map(Vec::len), Some(1));
    assert_eq!(value["models"][0]["model_id"], "model-x");
    assert_eq!(value["total"]["sessions"], 1);
}

#[test]
fn test_report_empty_database() {
    let storage = codemason_storage::Storage::open_in_memory().expect("storage");
    let report = usage_stats_report(&storage, &StatsArgs::default(), std::path::Path::new("/x"))
        .expect("report");
    assert!(
        report.contains("No completed agent runs recorded."),
        "{report}"
    );
}

#[tokio::test]
async fn test_stats_command_renders_report() {
    let mut app = make_app();
    seed(&app.storage, "s1", "/tmp/stats-cmd", "model-x", 0.5);

    app.execute_slash_command("/stats --days 7").await;

    let text = last_text(&app);
    assert!(
        text.starts_with("From: /stats\n\n## Usage - last 7 day(s)"),
        "{text}"
    );
    assert!(text.contains("model-x"), "{text}");
    assert_eq!(app.status, "stats");
}

#[tokio::test]
async fn test_stats_help_and_bad_argument() {
    let mut app = make_app();

    app.execute_slash_command("/stats help").await;
    let text = last_text(&app);
    assert!(text.starts_with("From: /stats help"), "{text}");
    assert!(text.contains("--project"), "{text}");
    assert_eq!(app.status, "stats: help");

    app.execute_slash_command("/stats --bogus").await;
    let text = last_text(&app);
    assert!(text.contains("Unrecognised argument `--bogus`"), "{text}");
    assert_eq!(app.status, "stats: usage");
}
