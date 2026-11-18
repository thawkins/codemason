# /stats

> "Usage and cost across sessions: /stats [--days N] [--project] [--json] | /stats help" (SlashCommandDef, crates/codemason-app-core/src/app/state.rs).

## Overview

`/stats` reports token usage, estimated cost and run time per model, summed over
every completed agent run stored in the session database. `/cost` covers the
current session only; `/stats` covers all sessions, or the sessions of one
project. The CLI twin is `cdmn stats`, with the same flags and the same output.

The data is the `run_cost_summaries` table. A run is recorded when an agent run
finishes, so a run that is still in progress is not counted. Costs are estimates
from the price table (`prices` in the config, else the built-in prices).

The report is built by `crates/codemason-app-core/src/usage_stats.rs`.

## Syntax

```
/stats [--days N] [--project] [--json]
/stats help
```

## Options / Subcommands

| Form | Description |
|------|-------------|
| `/stats` | All time, all projects |
| `--days N` | Only runs from the last N days (N is a positive integer; `--days=N` also works) |
| `--project` | Only sessions whose working directory is the current project directory |
| `--json` | One JSON object (`days`, `project`, `models`, `total`) instead of a table |
| `/stats help` | Show the help page |

## Examples

```
/stats --days 7 --project
```

```
cdmn stats --days 30 --json
```

## Notes

- Rows are ordered by cost, highest first. The `Total` row counts each session
  once, even when it used several models.
- `--project` matches the session's recorded directory exactly; a session
  started in a subdirectory or a worktree is a different directory.
