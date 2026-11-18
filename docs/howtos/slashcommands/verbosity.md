# /verbosity

> "Set how much the transcript shows: /verbosity low|medium|high" (SlashCommandDef, crates/codemason-app-core/src/app/state.rs).

## Overview

`/verbosity` sets the transcript detail level held in `App::transcript_prefs`. The default is `low`. The setting lasts until codemason exits; set `"verbosity": "high"` in `codemason.json` to start at a level. The handler is `App::slash_verbosity` in `crates/codemason-app-core/src/app/slash.rs`.

## Levels

| Level | Transcript |
|---|---|
| `low` (default) | Each run of tool calls and reasoning is one row, e.g. "Read 4 files, edited 2 files, ran 1 command (12 s)"; click it to list the calls |
| `medium` | Each tool call is its own row; reasoning is folded |
| `high` | Every tool call and every piece of reasoning is shown |

## Syntax

```
/verbosity
/verbosity low
/verbosity medium
/verbosity high
```

## Notes

- A bare `/verbosity` reports the current level in the status line.
- An unknown value changes nothing and the status line names the valid values.
