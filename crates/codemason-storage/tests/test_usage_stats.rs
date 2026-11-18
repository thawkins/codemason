//! Tests for the cross-session usage aggregation (`Storage::usage_stats`).

use codemason_storage::storage::{RunCostSummaryRow, Storage};

fn row(id: &str, session: &str, model: &str, cost: f64, age_days: i64) -> RunCostSummaryRow {
    RunCostSummaryRow {
        id: id.to_string(),
        session_id: session.to_string(),
        model_id: model.to_string(),
        input_tokens: 100,
        output_tokens: 10,
        total_cost_usd: cost,
        duration_ms: 1_000,
        created_at: (chrono::Utc::now() - chrono::Duration::days(age_days)).to_rfc3339(),
    }
}

fn seeded() -> Storage {
    let storage = Storage::open_in_memory().unwrap();
    storage.create_session("s-a", "/tmp/proj-a").unwrap();
    storage.create_session("s-b", "/tmp/proj-b").unwrap();
    for r in [
        row("1", "s-a", "model-x", 0.10, 0),
        row("2", "s-a", "model-x", 0.20, 10),
        row("3", "s-b", "model-x", 0.05, 0),
        row("4", "s-b", "model-y", 1.00, 0),
    ] {
        storage.create_run_cost_summary(&r).unwrap();
    }
    storage
}

#[test]
fn test_usage_stats_groups_by_model_ordered_by_cost() {
    let rows = seeded().usage_stats(None, None).unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].model_id, "model-y");
    assert_eq!(rows[1].model_id, "model-x");
    assert_eq!(rows[1].runs, 3);
    assert_eq!(rows[1].sessions, 2);
    assert_eq!(rows[1].input_tokens, 300);
    assert_eq!(rows[1].output_tokens, 30);
    assert_eq!(rows[1].duration_ms, 3_000);
    assert!((rows[1].total_cost_usd - 0.35).abs() < 1e-9);
}

#[test]
fn test_usage_stats_since_excludes_older_runs() {
    let since = (chrono::Utc::now() - chrono::Duration::days(5)).to_rfc3339();
    let rows = seeded().usage_stats(Some(&since), None).unwrap();
    let x = rows.iter().find(|r| r.model_id == "model-x").unwrap();
    assert_eq!(x.runs, 2);
}

#[test]
fn test_usage_stats_directory_filter() {
    let rows = seeded().usage_stats(None, Some("/tmp/proj-a")).unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].model_id, "model-x");
    assert_eq!(rows[0].runs, 2);
}

#[test]
fn test_usage_session_count_is_distinct_across_models() {
    let storage = seeded();
    assert_eq!(storage.usage_session_count(None, None).unwrap(), 2);
    assert_eq!(
        storage
            .usage_session_count(None, Some("/tmp/proj-b"))
            .unwrap(),
        1
    );
}
