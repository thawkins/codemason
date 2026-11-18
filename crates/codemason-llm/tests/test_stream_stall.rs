//! Tests for `http_client::next_chunk_or_stall`, the per-chunk idle guard every
//! streaming provider shares (deep-refactor S-01).
//!
//! Time is paused, so the 120 s stall budget elapses instantly.

use codemason_llm::provider::http_client::{STREAM_CHUNK_IDLE_TIMEOUT_SECS, next_chunk_or_stall};
use futures::stream;

#[tokio::test(start_paused = true)]
async fn test_next_chunk_or_stall_reports_stall_with_label_and_budget() {
    let mut silent = stream::pending::<u8>();
    let result = next_chunk_or_stall(&mut silent, "Anthropic").await;
    assert_eq!(
        result,
        Err(format!(
            "Anthropic: stream stalled - no data received for {STREAM_CHUNK_IDLE_TIMEOUT_SECS}s"
        ))
    );
    assert_eq!(STREAM_CHUNK_IDLE_TIMEOUT_SECS, 120);
}

#[tokio::test(start_paused = true)]
async fn test_next_chunk_or_stall_yields_items_then_end() {
    let mut items = stream::iter([1_u8, 2]);
    assert_eq!(next_chunk_or_stall(&mut items, "Ollama").await, Ok(Some(1)));
    assert_eq!(next_chunk_or_stall(&mut items, "Ollama").await, Ok(Some(2)));
    assert_eq!(next_chunk_or_stall(&mut items, "Ollama").await, Ok(None));
}

#[tokio::test(start_paused = true)]
async fn test_next_chunk_or_stall_waits_out_a_gap_shorter_than_the_budget() {
    let gap = std::time::Duration::from_secs(STREAM_CHUNK_IDLE_TIMEOUT_SECS - 1);
    let mut late = Box::pin(stream::once(async move {
        tokio::time::sleep(gap).await;
        7_u8
    }));
    assert_eq!(next_chunk_or_stall(&mut late, "Gemini").await, Ok(Some(7)));
}
