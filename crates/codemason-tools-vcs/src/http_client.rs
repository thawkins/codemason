//! Process-wide shared `reqwest::Client` for VCS API calls (PERF-058).
//!
//! Every GitHub/GitLab tool used to construct a fresh `reqwest::Client` per
//! call, which builds a new TLS configuration and connection pool each time.
//! One cached client is cloned (an `Arc` bump) into each client struct so the
//! TLS pool and keep-alive connections are reused across calls.

use std::sync::OnceLock;
use std::time::Duration;

use reqwest::Client;

/// Connect timeout in seconds.
const CONNECT_TIMEOUT_SECS: u64 = 30;

/// Request timeout in seconds.
const REQUEST_TIMEOUT_SECS: u64 = 120;

/// Maximum idle connections retained per host.
const MAX_IDLE_PER_HOST: usize = 8;

/// Return a clone of the process-wide shared HTTP client.
///
/// The client is built once on first use (connection-pool limits, connect and
/// request timeouts, TCP keep-alive) and reused for every subsequent call.
#[must_use]
pub fn shared_client() -> Client {
    static CACHED: OnceLock<Client> = OnceLock::new();
    CACHED
        .get_or_init(|| {
            Client::builder()
                .pool_max_idle_per_host(MAX_IDLE_PER_HOST)
                .pool_idle_timeout(Duration::from_secs(90))
                .connect_timeout(Duration::from_secs(CONNECT_TIMEOUT_SECS))
                .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECS))
                .tcp_keepalive(Duration::from_mins(1))
                .build()
                .unwrap_or_else(|e| {
                    tracing::warn!(
                        error = %e,
                        "Failed to build shared VCS HTTP client, using defaults"
                    );
                    Client::new()
                })
        })
        .clone()
}

/// Reads `resp`'s body, keeping only its last `limit` bytes. The body is
/// streamed and the buffer never holds much more than `limit` bytes, so a
/// huge response (a CI log) costs bandwidth but not memory.
///
/// # Errors
///
/// Returns an error when a body chunk cannot be read.
pub async fn read_tail(resp: reqwest::Response, limit: usize) -> reqwest::Result<Vec<u8>> {
    read_tail_counting_lines(resp, limit)
        .await
        .map(|counted| counted.tail)
}

/// The bounded tail of a streamed body, from [`read_tail_counting_lines`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CountedTail {
    /// The last `limit` bytes of the body.
    pub tail: Vec<u8>,
    /// Lines in the *whole* body, with the same rule as [`str::lines`]: every
    /// `\n` ends a line, plus one more for trailing bytes after the last `\n`.
    /// An empty body has zero lines.
    pub total_lines: u64,
    /// `true` when bytes before `tail` were dropped and the cut fell inside a
    /// line, so the first line of `tail` is only that line's suffix.
    pub starts_mid_line: bool,
}

/// Like [`read_tail`], but also counts the lines of the whole body and says
/// whether the byte cut split a line; see [`CountedTail`].
///
/// # Errors
///
/// Returns an error when a body chunk cannot be read.
pub async fn read_tail_counting_lines(
    resp: reqwest::Response,
    limit: usize,
) -> reqwest::Result<CountedTail> {
    use futures::StreamExt as _;
    let mut stream = resp.bytes_stream();
    let mut tail: Vec<u8> = Vec::new();
    let mut newlines: u64 = 0;
    let mut last_byte: Option<u8> = None;
    // The byte just before the retained tail, once anything was dropped.
    let mut byte_before_tail: Option<u8> = None;
    let mut drop_front = |tail: &mut Vec<u8>, keep: usize| {
        let cut = tail.len() - keep;
        byte_before_tail = Some(tail[cut - 1]);
        tail.drain(..cut);
    };
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        // Chunks are bounded by the HTTP body reader; a plain scan is enough.
        #[allow(clippy::naive_bytecount)]
        let chunk_newlines = chunk.iter().filter(|&&b| b == b'\n').count();
        newlines += chunk_newlines as u64;
        if let Some(&b) = chunk.last() {
            last_byte = Some(b);
        }
        tail.extend_from_slice(&chunk);
        // Trim only once the buffer is twice the limit, so each byte is
        // moved at most a few times.
        if tail.len() > limit.saturating_mul(2) {
            drop_front(&mut tail, limit);
        }
    }
    if tail.len() > limit {
        drop_front(&mut tail, limit);
    }
    let total_lines = match last_byte {
        None => 0,
        Some(b'\n') => newlines,
        Some(_) => newlines + 1,
    };
    Ok(CountedTail {
        tail,
        total_lines,
        starts_mid_line: byte_before_tail.is_some_and(|b| b != b'\n'),
    })
}
