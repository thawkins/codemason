//! Formatter-after-edit: runs a code formatter on files a tool just wrote.
//!
//! The dispatch pipeline calls [`format_after_write`] after a successful
//! file-writing tool call on the local backend. Each written file is matched
//! to a formatter by extension (custom formatters from the `formatter` config
//! section first, then the built-ins), the formatter runs as a subprocess with
//! a timeout, and the caller is told which files changed so the model knows
//! its view of them is stale.
//!
//! Depends on `codemason_config` for [`FormatterConfig`] and on `tokio` for
//! the subprocess and the timeout.

use std::path::{Path, PathBuf};
use std::time::Duration;

use codemason_config::FormatterConfig;
use serde_json::Value;

/// How long one formatter run may take before it is killed.
const FORMAT_TIMEOUT: Duration = Duration::from_secs(10);

/// Placeholder replaced by the file path in a formatter command.
const FILE_PLACEHOLDER: &str = "$FILE";

/// The tools whose successful result names files with new content.
const FORMATTED_TOOLS: &[&str] = &[
    "Write",
    "create",
    "write_file",
    "update_file",
    "Edit",
    "multi_edit",
    "multiedit",
    "apply_patch",
    "append_to_file",
];

/// A formatter known without configuration.
struct Builtin {
    name: &'static str,
    extensions: &'static [&'static str],
    command: &'static [&'static str],
}

/// The built-in formatters. Each runs only when its program is installed.
const BUILTINS: &[Builtin] = &[
    Builtin {
        name: "rustfmt",
        extensions: &[".rs"],
        command: &["rustfmt", FILE_PLACEHOLDER],
    },
    Builtin {
        name: "gofmt",
        extensions: &[".go"],
        command: &["gofmt", "-w", FILE_PLACEHOLDER],
    },
    Builtin {
        name: "ruff",
        extensions: &[".py", ".pyi"],
        command: &["ruff", "format", FILE_PLACEHOLDER],
    },
    Builtin {
        name: "prettier",
        extensions: &[
            ".js", ".jsx", ".mjs", ".cjs", ".ts", ".tsx", ".mts", ".cts", ".json", ".css", ".scss",
            ".less", ".html", ".vue", ".yaml", ".yml",
        ],
        command: &["prettier", "--write", FILE_PLACEHOLDER],
    },
];

/// The formatter chosen for a file: its name and argv with the path filled in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedFormatter {
    /// Formatter name (a built-in name or a config key).
    pub name: String,
    /// Program followed by its arguments.
    pub argv: Vec<String>,
}

/// Picks the formatter for `path`, or `None` when no enabled formatter handles
/// its extension.
///
/// A config entry that matches the extension wins over a built-in. A config
/// entry named like a built-in overrides that built-in's `command` and
/// `extensions` where it sets them, and removes it when `disabled`.
#[must_use]
pub fn resolve_formatter(config: &FormatterConfig, path: &Path) -> Option<ResolvedFormatter> {
    if !config.enabled {
        return None;
    }
    let file_name = path.file_name()?.to_str()?.to_lowercase();
    let file = path.display().to_string();

    // Custom formatters: config entries that are not built-in names.
    for (name, entry) in &config.formatters {
        let is_builtin = BUILTINS.iter().any(|b| b.name == name);
        if is_builtin || entry.disabled || entry.command.is_empty() {
            continue;
        }
        if handles(&file_name, &entry.extensions) {
            return Some(ResolvedFormatter {
                name: name.clone(),
                argv: fill(&entry.command, &file),
            });
        }
    }

    for builtin in BUILTINS {
        let entry = config.formatters.get(builtin.name);
        if entry.is_some_and(|e| e.disabled) {
            continue;
        }
        let matched = match entry.filter(|e| !e.extensions.is_empty()) {
            Some(e) => handles(&file_name, &e.extensions),
            None => handles(&file_name, builtin.extensions),
        };
        if !matched {
            continue;
        }
        let argv = match entry.filter(|e| !e.command.is_empty()) {
            Some(e) => fill(&e.command, &file),
            None => fill(builtin.command, &file),
        };
        return Some(ResolvedFormatter {
            name: builtin.name.to_string(),
            argv,
        });
    }
    None
}

/// Whether the lowercased `file_name` ends with one of `extensions`.
fn handles<S: AsRef<str>>(file_name: &str, extensions: &[S]) -> bool {
    extensions.iter().any(|ext| {
        let ext = ext.as_ref();
        !ext.is_empty() && file_name.ends_with(&ext.to_lowercase())
    })
}

/// Builds an argv from `command`, replacing the file placeholder with `file`.
fn fill<S: AsRef<str>>(command: &[S], file: &str) -> Vec<String> {
    command
        .iter()
        .map(|part| part.as_ref().replace(FILE_PLACEHOLDER, file))
        .collect()
}

/// The files a successful call of `tool_name` wrote, read from its result
/// metadata. Empty for a tool that is not a file writer.
///
/// Relative paths are resolved against `working_dir`. Files a patch deleted
/// are left out.
#[must_use]
pub fn written_paths(tool_name: &str, metadata: &Value, working_dir: &Path) -> Vec<PathBuf> {
    if !FORMATTED_TOOLS.contains(&tool_name) {
        return Vec::new();
    }
    let mut paths: Vec<PathBuf> = Vec::new();
    let mut push = |value: &Value| {
        if let Some(path) = value.as_str().filter(|p| !p.is_empty()) {
            let path = working_dir.join(path);
            if !paths.contains(&path) {
                paths.push(path);
            }
        }
    };
    push(&metadata["path"]);
    for key in ["file_stats", "per_file"] {
        for item in metadata[key].as_array().into_iter().flatten() {
            if item["operation"].as_str() != Some("delete") {
                push(&item["path"]);
            }
        }
    }
    paths
}

/// Runs the matching formatter on each of `paths`.
///
/// Returns `(path, formatter name)` for every file whose content the formatter
/// changed. A formatter that is not installed, fails, or times out is logged
/// at `debug!` and leaves the file as the tool wrote it.
pub async fn format_written_files(
    config: &FormatterConfig,
    paths: &[PathBuf],
    working_dir: &Path,
) -> Vec<(PathBuf, String)> {
    let mut changed = Vec::new();
    for path in paths {
        let Some(formatter) = resolve_formatter(config, path) else {
            continue;
        };
        let Ok(before) = tokio::fs::read(path).await else {
            continue;
        };
        if let Err(reason) = run_formatter(&formatter, working_dir).await {
            tracing::debug!(
                formatter = %formatter.name,
                path = %path.display(),
                %reason,
                "formatter did not run cleanly"
            );
            continue;
        }
        if tokio::fs::read(path)
            .await
            .is_ok_and(|after| after != before)
        {
            changed.push((path.clone(), formatter.name));
        }
    }
    changed
}

/// Runs one formatter to completion, killing it at [`FORMAT_TIMEOUT`].
async fn run_formatter(formatter: &ResolvedFormatter, working_dir: &Path) -> Result<(), String> {
    let (program, args) = formatter
        .argv
        .split_first()
        .ok_or_else(|| "empty command".to_string())?;
    let child = tokio::process::Command::new(program)
        .args(args)
        .current_dir(working_dir)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .map_err(|e| format!("spawn failed: {e}"))?;
    // Dropping the timed-out future drops the child, and `kill_on_drop` kills it.
    let output = tokio::time::timeout(FORMAT_TIMEOUT, child.wait_with_output())
        .await
        .map_err(|_| format!("timed out after {}s", FORMAT_TIMEOUT.as_secs()))?
        .map_err(|e| format!("wait failed: {e}"))?;
    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(format!(
            "{}: {}",
            output.status,
            stderr.lines().next().unwrap_or_default()
        ))
    }
}

/// Runs the configured formatter on the files a successful tool call wrote.
///
/// Does nothing unless the `formatter` section is enabled and the call ran on
/// the local backend (a container backend writes inside the sandbox). For each
/// file a formatter changed, the session's read baseline is refreshed so the
/// next edit is not rejected as stale, and a line is appended to the output
/// telling the model to re-read the file.
pub async fn format_after_write(
    tool_name: &str,
    mut output: crate::tool::ToolOutput,
    tool_ctx: &crate::tool::ToolContext,
) -> crate::tool::ToolOutput {
    let Some(config) = tool_ctx.config.as_ref() else {
        return output;
    };
    if !config.formatter.enabled
        || config.effective_execution_backend() != codemason_config::ExecutionBackendKind::Local
    {
        return output;
    }
    let Some(metadata) = output.metadata.as_ref() else {
        return output;
    };
    let paths = written_paths(tool_name, metadata, &tool_ctx.working_dir);
    let changed = format_written_files(&config.formatter, &paths, &tool_ctx.working_dir).await;
    for (path, formatter) in changed {
        let mtime_ms = std::fs::metadata(&path)
            .and_then(|meta| meta.modified())
            .ok()
            .and_then(|mtime| mtime.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_millis() as u64);
        if let Some(mtime_ms) = mtime_ms {
            let mut baselines = tool_ctx
                .read_timestamps
                .write()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if let Some(baseline) = baselines.get_mut(&path) {
                *baseline = mtime_ms;
            }
        }
        output.content.push_str(&format!(
            "\n[formatter] {formatter} reformatted {}; re-read it before editing it again.",
            path.display()
        ));
    }
    output
}
