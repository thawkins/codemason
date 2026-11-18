//! Cross-session usage and cost statistics.
//!
//! Shared by the `/stats` slash command and the `cdmn stats` CLI subcommand, so
//! both parse the same flags and print the same report. The data comes from the
//! persisted `run_cost_summaries` table through
//! [`codemason_storage::storage::Storage::usage_stats`].

use std::fmt::Write as _;
use std::path::Path;

use codemason_storage::storage::{Storage, UsageStatsRow};

/// Usage line shared by the help page and the argument errors.
pub const STATS_USAGE: &str = "/stats [--days N] [--project] [--json] | /stats help";

/// Parsed `/stats` options.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct StatsArgs {
    /// Only count runs from the last `days` days; `None` means all time.
    pub days: Option<u32>,
    /// Only count sessions of the current project directory.
    pub project: bool,
    /// Emit JSON instead of a table.
    pub json: bool,
}

/// Parses the tokens after `/stats`.
///
/// Accepts `--days N`, `--days=N`, `--project` and `--json` in any order.
///
/// # Errors
///
/// Returns the offending token when it is not a known flag or `--days` has no
/// positive integer value.
pub fn parse_stats_args<'a, I>(tokens: I) -> Result<StatsArgs, String>
where
    I: IntoIterator<Item = &'a str>,
{
    let mut args = StatsArgs::default();
    let mut tokens = tokens.into_iter();
    while let Some(token) = tokens.next() {
        let lowered = token.to_lowercase();
        let days_value = if lowered == "--days" {
            Some(tokens.next().unwrap_or(""))
        } else {
            lowered.strip_prefix("--days=")
        };
        match (lowered.as_str(), days_value) {
            (_, Some(value)) => match value.parse::<u32>() {
                Ok(n) if n > 0 => args.days = Some(n),
                _ => return Err("--days".to_string()),
            },
            ("--project", None) => args.project = true,
            ("--json", None) => args.json = true,
            _ => return Err(token.to_string()),
        }
    }
    Ok(args)
}

/// Builds the usage report for `args`.
///
/// `project_dir` is the directory `--project` filters on. The table form is
/// Markdown; the JSON form is one object with `days`, `project`, `models` and
/// `total`.
///
/// # Errors
///
/// Returns an error when the day window is out of range, the storage query
/// fails, or the JSON cannot be serialised.
pub fn usage_stats_report(
    storage: &Storage,
    args: &StatsArgs,
    project_dir: &Path,
) -> anyhow::Result<String> {
    let since = match args.days {
        Some(days) => {
            let window = chrono::Duration::try_days(i64::from(days))
                .ok_or_else(|| anyhow::anyhow!("day window out of range: {days}"))?;
            Some((chrono::Utc::now() - window).to_rfc3339())
        }
        None => None,
    };
    let directory = args.project.then(|| project_dir.display().to_string());
    let rows = storage.usage_stats(since.as_deref(), directory.as_deref())?;
    let mut total = total_row(&rows);
    total.sessions = storage.usage_session_count(since.as_deref(), directory.as_deref())?;

    if args.json {
        let value = serde_json::json!({
            "days": args.days,
            "project": directory,
            "models": rows,
            "total": total,
        });
        return Ok(serde_json::to_string_pretty(&value)?);
    }

    let window = args
        .days
        .map_or_else(|| "all time".to_string(), |d| format!("last {d} day(s)"));
    let scope = directory.map_or_else(|| "all projects".to_string(), |d| format!("project `{d}`"));
    let mut out = format!("## Usage - {window}, {scope}\n\n");
    if rows.is_empty() {
        out.push_str("No completed agent runs recorded.\n");
        return Ok(out);
    }
    out.push_str(
        "| Model | Runs | Sessions | Input tokens | Output tokens | Cost (USD) | Time |\n",
    );
    out.push_str("|---|---:|---:|---:|---:|---:|---:|\n");
    for row in rows.iter().chain(std::iter::once(&total)) {
        // Writing to a `String` cannot fail.
        let _ = writeln!(
            out,
            "| {} | {} | {} | {} | {} | {:.4} | {} |",
            row.model_id,
            row.runs,
            row.sessions,
            row.input_tokens,
            row.output_tokens,
            row.total_cost_usd,
            format_duration(row.duration_ms),
        ); // INTENTIONAL: fmt::Write on String is infallible
    }
    Ok(out)
}

/// Sums `rows` into one row labelled `Total`.
///
/// `sessions` is left at zero: a session that used two models appears in two
/// rows, so the caller fills in the distinct count.
fn total_row(rows: &[UsageStatsRow]) -> UsageStatsRow {
    rows.iter().fold(
        UsageStatsRow {
            model_id: "Total".to_string(),
            runs: 0,
            sessions: 0,
            input_tokens: 0,
            output_tokens: 0,
            total_cost_usd: 0.0,
            duration_ms: 0,
        },
        |mut acc, row| {
            acc.runs += row.runs;
            acc.input_tokens += row.input_tokens;
            acc.output_tokens += row.output_tokens;
            acc.total_cost_usd += row.total_cost_usd;
            acc.duration_ms += row.duration_ms;
            acc
        },
    )
}

/// Formats milliseconds as `1h 02m`, `3m 04s` or `5s`.
fn format_duration(ms: u64) -> String {
    let secs = ms / 1000;
    let (h, m, s) = (secs / 3600, (secs % 3600) / 60, secs % 60);
    if h > 0 {
        format!("{h}h {m:02}m")
    } else if m > 0 {
        format!("{m}m {s:02}s")
    } else {
        format!("{s}s")
    }
}
