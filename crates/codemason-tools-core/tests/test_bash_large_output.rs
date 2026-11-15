//! TL-04: bash output larger than the in-memory capture cap must still show the
//! true end of the output and a correct omitted-line count.
//!
//! The former capture kept only the first 2 MiB of each stream, so for
//! `seq 1 2000000` (about 14.9 MB) the "last 15 KB" shown ended at a line in
//! the middle of the stream and the omitted count covered only the captured
//! prefix.

#![cfg(unix)]

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

use codemason_tools_core::bash::BashTool;
use codemason_tools_core::{Tool, ToolContext};
use codemason_types::event::EventBus;
use serde_json::json;

/// A tool context rooted at a per-test `target/temp/` scratch directory.
fn make_ctx() -> ToolContext {
    let dir =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/temp/test_bash_large_output");
    std::fs::create_dir_all(&dir).expect("create scratch dir");
    ToolContext {
        session_id: "codemason-test-bash-large-output".to_string(),
        working_dir: dir.clone(),
        event_bus: Arc::new(EventBus::new(16)),
        read_timestamps: Arc::new(RwLock::new(HashMap::new())),
        canonical_cache: Arc::new(codemason_tools_core::CanonicalPathCache::new()),
        allowed_roots: vec![dir],
    }
}

/// The rendering `truncate_output` must produce for the full stream: the
/// first 15 000 and last 15 000 bytes with the line count of the middle.
fn expected_render(full: &str) -> String {
    let head = &full[..15_000];
    let tail_start = full.len() - 15_000;
    let omitted = full[15_000..tail_start].lines().count();
    format!(
        "{head}\n\n... ({omitted} lines omitted) ...\n\n{}",
        &full[tail_start..]
    )
}

#[tokio::test]
async fn test_bash_large_output_shows_true_tail_and_omitted_count() {
    let full: String = (1..=2_000_000).map(|i| format!("{i}\n")).collect();
    let out = BashTool
        .execute(
            json!({ "command": "seq 1 2000000", "timeout": 120 }),
            &make_ctx(),
        )
        .await
        .expect("bash tool must succeed");

    let last_line = out.content.lines().last().unwrap_or_default();
    assert_eq!(last_line, "2000000", "tail is not the real end of output");

    let expected = expected_render(&full);
    assert!(
        out.content.ends_with(&expected),
        "rendered output differs from the full-stream rendering; marker line: {:?}",
        out.content.lines().find(|l| l.contains("lines omitted"))
    );
}
