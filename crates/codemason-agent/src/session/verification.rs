//! Verification-command gate for goal-driven agentic loops.
//!
//! This module implements the verification half of the loop contract
//! described in `specs/agentloop/SPEC.md` (FR-007): a loop may carry an
//! optional verification command whose exit status gates goal achievement.
//! When the model first responds without tool calls, the command runs
//! automatically - success completes the loop, failure with steps remaining
//! appends the failure output as an observation so the model can self-correct,
//! and failure without steps remaining ends the run as a budget exhaustion.
//!
//! The command executes in the session's working directory with a fixed
//! timeout and ASCII-truncated output capture, mirroring the shell handling
//! of the core bash tool.
//!
//! Dependencies: `tokio` (blocking offload), `std::process::Command`.

use std::process::Command;
use std::time::Duration;

/// Wall-clock limit for one verification-command run (10 minutes).
///
/// Verification commands are typically quick test/grep invocations; a run
/// that exceeds this budget is treated as a failure (the loop must not hang
/// waiting on a stalled verification step).
const VERIFICATION_TIMEOUT: Duration = Duration::from_mins(10);

/// Maximum number of characters kept from the head of the captured output.
const VERIFICATION_HEAD_CHARS: usize = 8000;

/// Maximum number of characters kept from the tail of the captured output.
const VERIFICATION_TAIL_CHARS: usize = 2000;

/// Truncate `text` to at most head + tail characters, joined by an elision
/// marker. The cut always falls on a UTF-8 character boundary.
fn truncate_head_tail(text: &str, head: usize, tail: usize) -> String {
    // Count once and reuse for the early-exit, the tail offset, and the
    // elided-character figure. Walks the string a single time via
    // `char_indices` so the byte offsets for the head/tail slices are
    // captured in the same pass.
    let total = text.chars().count();
    if total <= head + tail {
        return text.to_string();
    }
    let mut head_end = text.len();
    let mut tail_start = 0usize;
    for (n, (offset, _)) in text.char_indices().enumerate() {
        if n == head {
            head_end = offset;
        }
        if n == total - tail {
            tail_start = offset;
        }
    }
    let head_text = &text[..head_end];
    let tail_text = &text[tail_start..];
    format!(
        "{head_text}\n... [output truncated: {} characters elided] ...\n{tail_text}",
        total - head - tail
    )
}

/// Result of running the loop's verification command (FR-007).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VerificationOutcome {
    /// The command exited with status 0. Carries the captured output.
    Success(String),
    /// The command failed: non-zero exit, timeout, or spawn error.
    /// Carries a short human-readable reason and the captured output.
    Failure {
        /// Short reason: exit code, signal, timeout, or spawn error.
        reason: String,
        /// Captured stdout + stderr (ASCII-truncated).
        output: String,
    },
}

impl VerificationOutcome {
    /// Whether the verification command passed.
    pub fn passed(&self) -> bool {
        matches!(self, Self::Success(_))
    }

    /// The captured command output, for both variants.
    pub fn output(&self) -> &str {
        match self {
            Self::Success(output) | Self::Failure { output, .. } => output,
        }
    }

    /// One-line summary label used in the `Event::LoopTerminated`
    /// verification field and in user-facing notices.
    pub fn label(&self) -> String {
        match self {
            Self::Success(_) => "verification command passed".to_string(),
            Self::Failure { reason, .. } => {
                format!("verification command failed ({reason})")
            }
        }
    }
}

/// Run `cmd` in `working_dir`, capturing stdout and stderr with the fixed
/// verification timeout (FR-007).
///
/// The blocking process execution is off-loaded to `spawn_blocking` so the
/// async executor is not stalled while the command runs.
///
/// # Errors
/// Returns an error only when the command could not be spawned at all
/// (e.g. the program does not exist). A non-zero exit status, a timeout, or
/// an abnormal termination are reported as [`VerificationOutcome::Failure`],
/// not as errors - the gate must see them, not crash on them.
pub async fn run_verification_command(
    cmd: &str,
    working_dir: &std::path::Path,
) -> anyhow::Result<VerificationOutcome> {
    let script = cmd.to_string();
    let dir = working_dir.to_path_buf();
    tokio::task::spawn_blocking(move || execute_verification_script(&script, &dir))
        .await
        .map_err(|e| anyhow::anyhow!("verification command join error: {e}"))?
}

/// Synchronous execution of the verification script.
///
/// Spawns `bash -c <script>` through the shared deadline runner (AG-06): the
/// output drains while the script runs, the whole process tree is killed at
/// [`VERIFICATION_TIMEOUT`] (a grandchild such as a test binary can no longer
/// hold the pipes open past it), and the output is decoded lossily so one
/// non-UTF-8 byte no longer empties it. The blocking work runs on the tokio
/// blocking pool via the async wrapper.
fn execute_verification_script(
    script: &str,
    working_dir: &std::path::Path,
) -> anyhow::Result<VerificationOutcome> {
    let mut cmd = Command::new(codemason_tools_core::bash::bash_program());
    cmd.arg("-c").arg(script).current_dir(working_dir);
    let run = match crate::child_process::run_with_deadline(&mut cmd, None, VERIFICATION_TIMEOUT) {
        Ok(run) => run,
        Err(e) => {
            return Ok(VerificationOutcome::Failure {
                reason: format!("spawn error: {e}"),
                output: String::new(),
            });
        }
    };
    let stdout = String::from_utf8_lossy(&run.stdout);
    let stderr = String::from_utf8_lossy(&run.stderr);

    let Some(status) = run.status else {
        let captured = truncate_head_tail(
            format!("{stdout}{stderr}").trim(),
            VERIFICATION_HEAD_CHARS,
            VERIFICATION_TAIL_CHARS,
        );
        return Ok(VerificationOutcome::Failure {
            reason: format!("timed out after {}s", VERIFICATION_TIMEOUT.as_secs()),
            output: captured,
        });
    };

    let mut captured = stdout.into_owned();
    if !stderr.trim().is_empty() {
        if !captured.is_empty() {
            captured.push('\n');
        }
        captured.push_str(&stderr);
    }
    let captured = truncate_head_tail(
        captured.trim(),
        VERIFICATION_HEAD_CHARS,
        VERIFICATION_TAIL_CHARS,
    );
    if status.success() {
        return Ok(VerificationOutcome::Success(captured));
    }
    let reason = describe_verification_status(&status);
    Ok(VerificationOutcome::Failure {
        reason,
        output: captured,
    })
}

/// Describe a process exit status for the verification-failure reason,
/// mirroring the bash tool's exit-status description.
fn describe_verification_status(status: &std::process::ExitStatus) -> String {
    if let Some(code) = status.code() {
        return format!("exit code {code}");
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        if let Some(sig) = status.signal() {
            return format!("killed by signal {sig}");
        }
    }
    "abnormal termination".to_string()
}

/// Observation message appended to the model's context when the verification
/// command fails but steps remain (FR-007): the failure output becomes the
/// next observation so the model can correct its work.
pub fn verification_failure_observation(cmd: &str, outcome: &VerificationOutcome) -> String {
    format!(
        "System note: the goal's verification command failed. \
         The loop cannot be marked complete until it passes.\n\
         Command: {}\n\
         Result: {}\n\
         Output:\n{}",
        cmd,
        outcome.label(),
        outcome.output()
    )
}

#[cfg(test)]
#[path = "../../tests/inline/verification_tests.rs"]
mod tests;
