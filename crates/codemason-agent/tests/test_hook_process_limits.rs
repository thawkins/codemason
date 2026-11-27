//! Hook child processes stay within their limits (audit AG-01..AG-04).
//!
//! A hung hook is killed at `timeout_secs`; a large tool input still reaches
//! the guard (on stdin, with a truncation marker in the environment); and a
//! hook that ignores its stdin while printing a lot cannot deadlock.

use std::time::{Duration, Instant};

use codemason_agent::hooks::{
    HookConfig, HookTrigger, PreToolUseResult, run_post_tool_use_hooks, run_pre_tool_use_hooks,
};

fn hook(trigger: HookTrigger, command: &str, timeout_secs: u64) -> Vec<HookConfig> {
    vec![HookConfig {
        trigger,
        command: command.to_string(),
        timeout_secs,
        plugin_root: None,
        matcher: None,
    }]
}

/// A JSON tool input of about `bytes` bytes.
fn big_input(bytes: usize) -> String {
    format!(r#"{{"path":"a.txt","content":"{}"}}"#, "x".repeat(bytes))
}

#[test]
fn test_hook_process_limits_pre_tool_hook_is_killed_at_its_timeout() {
    let hooks = hook(HookTrigger::PreToolUse, "sleep 20", 1);
    let started = Instant::now();
    let result = run_pre_tool_use_hooks(&hooks, &std::env::temp_dir(), "Write", "{}", "sess", None);
    assert!(
        started.elapsed() < Duration::from_secs(10),
        "returned after {:?}",
        started.elapsed()
    );
    assert!(matches!(result, PreToolUseResult::NoDecision), "{result:?}");
}

#[test]
fn test_hook_process_limits_large_input_still_runs_the_guard() {
    // 200 KiB is past every platform's per-variable limit, which used to make
    // the spawn fail and skip the guard. The guard now runs and sees the
    // truncation marker, so it can refuse what it cannot inspect.
    let hooks = hook(
        HookTrigger::PreToolUse,
        r#"[ "$CODEMASON_TOOL_INPUT_TRUNCATED" = 1 ] && exit 2; exit 0"#,
        30,
    );
    let result = run_pre_tool_use_hooks(
        &hooks,
        &std::env::temp_dir(),
        "Write",
        &big_input(200 * 1024),
        "sess",
        None,
    );
    assert!(
        matches!(result, PreToolUseResult::Blocked { .. }),
        "the guard must run and block: {result:?}"
    );
}

#[test]
fn test_hook_process_limits_hook_ignoring_stdin_while_printing_does_not_deadlock() {
    let hooks = hook(HookTrigger::PreToolUse, "head -c 300000 /dev/zero", 30);
    let started = Instant::now();
    let result = run_pre_tool_use_hooks(
        &hooks,
        &std::env::temp_dir(),
        "Write",
        &big_input(300 * 1024),
        "sess",
        None,
    );
    assert!(
        started.elapsed() < Duration::from_secs(20),
        "returned after {:?}",
        started.elapsed()
    );
    assert!(matches!(result, PreToolUseResult::NoDecision), "{result:?}");
}

#[tokio::test]
async fn test_hook_process_limits_post_tool_hook_does_not_outlive_its_timeout() {
    let dir = tempfile::tempdir().expect("tempdir");
    let marker = dir.path().join("late");
    let command = format!(
        "sleep 3; echo late > '{}'",
        marker.display().to_string().replace('\\', "/")
    );
    let hooks = hook(HookTrigger::PostToolUse, &command, 1);

    let _ =
        run_post_tool_use_hooks(&hooks, dir.path(), "Write", "{}", "ok", true, "sess", None).await;
    tokio::time::sleep(Duration::from_secs(4)).await;

    assert!(!marker.exists(), "the timed-out hook kept running");
}
