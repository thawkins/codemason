//! Tests for formatter-after-edit: formatter selection, written-path
//! extraction, and the post-write step on a tool result.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use codemason_agent::event::EventBus;
use codemason_agent::formatter::{format_after_write, resolve_formatter, written_paths};
use codemason_agent::tool::{ToolContext, ToolOutput};
use codemason_config::{Config, FormatterConfig, FormatterEntry};
use serde_json::json;

fn enabled(entries: &[(&str, FormatterEntry)]) -> FormatterConfig {
    FormatterConfig {
        enabled: true,
        formatters: entries
            .iter()
            .map(|(name, entry)| ((*name).to_string(), entry.clone()))
            .collect(),
    }
}

fn custom(command: &[&str], extensions: &[&str]) -> FormatterEntry {
    FormatterEntry {
        disabled: false,
        command: command.iter().map(|s| (*s).to_string()).collect(),
        extensions: extensions.iter().map(|s| (*s).to_string()).collect(),
    }
}

/// A fresh scratch directory under the git-ignored `target/temp/` tree.
fn scratch(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/temp/formatter")
        .join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create scratch dir");
    dir.canonicalize().expect("canonical scratch dir")
}

fn ctx(working_dir: &Path, formatter: FormatterConfig) -> ToolContext {
    let mut config = Config::default();
    config.formatter = formatter;
    ToolContext {
        session_id: "formatter-test".to_string(),
        working_dir: working_dir.to_path_buf(),
        event_bus: Arc::new(EventBus::default()),
        storage: None,
        agent_manager: None,
        active_model: None,
        provider_registry: None,
        team_context: None,
        team_manager: None,
        code_index: None,
        bg_service: None,
        spec_manager: None,
        active_spec_id: None,
        config: Some(Arc::new(config)),
        allowed_roots: Vec::new(),
        read_timestamps: Arc::new(std::sync::RwLock::new(std::collections::HashMap::new())),
        cached_team_dir: Arc::new(std::sync::Mutex::new(None)),
        permission_checker: None,
        canonical_cache: Arc::new(codemason_tools_core::CanonicalPathCache::new()),
        tool_registry: ToolContext::default_tool_registry(),
    }
}

fn mtime_ms(path: &Path) -> u64 {
    std::fs::metadata(path)
        .and_then(|m| m.modified())
        .expect("mtime")
        .duration_since(std::time::UNIX_EPOCH)
        .expect("epoch")
        .as_millis() as u64
}

fn output(path: &Path) -> ToolOutput {
    ToolOutput {
        content: "Wrote file".to_string(),
        metadata: Some(json!({ "path": path.display().to_string() })),
    }
}

#[test]
fn test_resolve_formatter_disabled_section_selects_nothing() {
    let config = FormatterConfig::default();
    assert_eq!(resolve_formatter(&config, Path::new("/p/a.rs")), None);
}

#[test]
fn test_resolve_formatter_builtin_by_extension() {
    let config = enabled(&[]);
    let rs = resolve_formatter(&config, Path::new("/p/a.RS")).expect("rustfmt");
    assert_eq!(rs.name, "rustfmt");
    assert_eq!(rs.argv, ["rustfmt", "/p/a.RS"]);
    let ts = resolve_formatter(&config, Path::new("/p/a.tsx")).expect("prettier");
    assert_eq!(ts.argv, ["prettier", "--write", "/p/a.tsx"]);
    assert_eq!(resolve_formatter(&config, Path::new("/p/a.unknown")), None);
    assert_eq!(resolve_formatter(&config, Path::new("/p/Makefile")), None);
}

#[test]
fn test_resolve_formatter_builtin_can_be_disabled_or_overridden() {
    let off = enabled(&[(
        "rustfmt",
        FormatterEntry {
            disabled: true,
            ..FormatterEntry::default()
        },
    )]);
    assert_eq!(resolve_formatter(&off, Path::new("/p/a.rs")), None);

    let overridden = enabled(&[(
        "rustfmt",
        custom(&["rustfmt", "--edition", "2024", "$FILE"], &[]),
    )]);
    let rs = resolve_formatter(&overridden, Path::new("/p/a.rs")).expect("rustfmt");
    assert_eq!(rs.argv, ["rustfmt", "--edition", "2024", "/p/a.rs"]);
}

#[test]
fn test_resolve_formatter_custom_wins_over_builtin() {
    let config = enabled(&[("mine", custom(&["myfmt", "$FILE"], &[".rs"]))]);
    let chosen = resolve_formatter(&config, Path::new("/p/a.rs")).expect("custom");
    assert_eq!(chosen.name, "mine");
    assert_eq!(chosen.argv, ["myfmt", "/p/a.rs"]);
}

#[test]
fn test_written_paths_reads_each_metadata_shape() {
    let wd = Path::new("/work");
    assert_eq!(
        written_paths("Write", &json!({ "path": "/work/a.rs" }), wd),
        [PathBuf::from("/work/a.rs")]
    );
    assert_eq!(
        written_paths("Edit", &json!({ "path": "rel/b.rs" }), wd),
        [PathBuf::from("/work/rel/b.rs")]
    );
    assert_eq!(
        written_paths(
            "multi_edit",
            &json!({ "file_stats": [{ "path": "/work/a.rs" }, { "path": "/work/a.rs" }] }),
            wd
        ),
        [PathBuf::from("/work/a.rs")]
    );
    assert_eq!(
        written_paths(
            "apply_patch",
            &json!({ "per_file": [
                { "path": "/work/gone.rs", "operation": "delete" },
                { "path": "/work/new.rs", "operation": "add" }
            ] }),
            wd
        ),
        [PathBuf::from("/work/new.rs")]
    );
    // A tool that is not a file writer is never formatted.
    assert_eq!(
        written_paths("read", &json!({ "path": "/work/a.rs" }), wd),
        Vec::<std::path::PathBuf>::new()
    );
    assert_eq!(
        written_paths("rm", &json!({ "path": "/work/a.rs" }), wd),
        Vec::<std::path::PathBuf>::new()
    );
}

#[cfg(unix)]
#[tokio::test]
async fn test_format_after_write_runs_formatter_and_reports_change() {
    let dir = scratch("runs");
    let file = dir.join("a.demo");
    std::fs::write(&file, "unformatted").expect("write");
    let config = enabled(&[(
        "demo",
        custom(
            &["sh", "-c", "printf formatted > \"$0\"", "$FILE"],
            &[".demo"],
        ),
    )]);
    let tool_ctx = ctx(&dir, config);
    // The tool recorded the file's mtime as the session's baseline.
    let written = mtime_ms(&file);
    tool_ctx
        .read_timestamps
        .write()
        .expect("lock")
        .insert(file.clone(), written);
    // Make the formatter's write land on a later mtime.
    tokio::time::sleep(std::time::Duration::from_millis(20)).await;

    let out = format_after_write("Write", output(&file), &tool_ctx).await;

    assert_eq!(std::fs::read_to_string(&file).expect("read"), "formatted");
    assert!(
        out.content.contains("[formatter] demo reformatted"),
        "{}",
        out.content
    );
    let baseline = tool_ctx.read_timestamps.read().expect("lock")[&file];
    assert!(
        baseline > written,
        "read baseline must follow the formatter's write"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn test_format_after_write_is_silent_when_nothing_changes() {
    let dir = scratch("unchanged");
    let file = dir.join("a.demo");
    std::fs::write(&file, "same").expect("write");
    let config = enabled(&[("demo", custom(&["true", "$FILE"], &[".demo"]))]);

    let out = format_after_write("Write", output(&file), &ctx(&dir, config)).await;

    assert_eq!(out.content, "Wrote file");
}

#[cfg(unix)]
#[tokio::test]
async fn test_format_after_write_ignores_failing_and_missing_formatters() {
    let dir = scratch("failing");
    let file = dir.join("a.demo");
    std::fs::write(&file, "kept").expect("write");
    for command in [
        &["false"][..],
        &["codemason-no-such-formatter", "$FILE"][..],
    ] {
        let config = enabled(&[("demo", custom(command, &[".demo"]))]);
        let out = format_after_write("Write", output(&file), &ctx(&dir, config)).await;
        assert_eq!(out.content, "Wrote file");
        assert_eq!(std::fs::read_to_string(&file).expect("read"), "kept");
    }
}

#[cfg(unix)]
#[tokio::test]
async fn test_format_after_write_does_nothing_when_disabled_or_not_a_writer() {
    let dir = scratch("disabled");
    let file = dir.join("a.demo");
    std::fs::write(&file, "kept").expect("write");
    let entry = custom(
        &["sh", "-c", "printf formatted > \"$0\"", "$FILE"],
        &[".demo"],
    );

    let mut off = enabled(&[("demo", entry.clone())]);
    off.enabled = false;
    format_after_write("Write", output(&file), &ctx(&dir, off)).await;
    assert_eq!(std::fs::read_to_string(&file).expect("read"), "kept");

    let on = enabled(&[("demo", entry)]);
    format_after_write("read", output(&file), &ctx(&dir, on)).await;
    assert_eq!(std::fs::read_to_string(&file).expect("read"), "kept");
}

#[cfg(unix)]
#[tokio::test]
async fn test_format_after_write_skips_dry_run_results() {
    let dir = scratch("dry_run");
    let file = dir.join("a.demo");
    std::fs::write(&file, "kept").expect("write");
    let config = enabled(&[(
        "demo",
        custom(
            &["sh", "-c", "printf formatted > \"$0\"", "$FILE"],
            &[".demo"],
        ),
    )]);
    let preview = ToolOutput {
        content: "preview".to_string(),
        metadata: Some(json!({ "path": file.display().to_string(), "dry_run": true })),
    };

    let out = format_after_write("Edit", preview, &ctx(&dir, config)).await;

    assert_eq!(out.content, "preview");
    assert_eq!(std::fs::read_to_string(&file).expect("read"), "kept");
}

#[cfg(unix)]
#[tokio::test]
async fn test_format_after_write_reports_a_change_made_by_a_failing_formatter() {
    let dir = scratch("failing_but_changed");
    let file = dir.join("a.demo");
    std::fs::write(&file, "unformatted").expect("write");
    let config = enabled(&[(
        "demo",
        custom(
            &["sh", "-c", "printf half > \"$0\"; exit 3", "$FILE"],
            &[".demo"],
        ),
    )]);
    let tool_ctx = ctx(&dir, config);
    let written = mtime_ms(&file);
    tool_ctx
        .read_timestamps
        .write()
        .expect("lock")
        .insert(file.clone(), written);
    tokio::time::sleep(std::time::Duration::from_millis(20)).await;

    let out = format_after_write("Write", output(&file), &tool_ctx).await;

    assert!(
        out.content.contains("[formatter] demo reformatted")
            && out.content.contains("the formatter then failed"),
        "{}",
        out.content
    );
    let baseline = tool_ctx.read_timestamps.read().expect("lock")[&file];
    assert!(
        baseline > written,
        "the baseline follows the formatter's write"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn test_format_after_write_keeps_a_stale_baseline_stale() {
    let dir = scratch("foreign_write");
    let file = dir.join("a.demo");
    std::fs::write(&file, "written by someone else").expect("write");
    let config = enabled(&[(
        "demo",
        custom(
            &["sh", "-c", "printf formatted > \"$0\"", "$FILE"],
            &[".demo"],
        ),
    )]);
    let tool_ctx = ctx(&dir, config);
    // The session's baseline predates the file's current content: another
    // writer got in after this session's own write.
    tool_ctx
        .read_timestamps
        .write()
        .expect("lock")
        .insert(file.clone(), 1);

    format_after_write("Write", output(&file), &tool_ctx).await;

    assert_eq!(tool_ctx.read_timestamps.read().expect("lock")[&file], 1);
}
