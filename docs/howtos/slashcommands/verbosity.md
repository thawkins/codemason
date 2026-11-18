# /verbosity

> "Set how much the transcript shows: /verbosity low|medium|high" (SlashCommandDef, crates/codemason-app-core/src/app/state.rs).

## Overview

`/verbosity` sets the transcript detail level held in `App::transcript_prefs`. The default is `low`. The setting lasts until codemason exits; set `"verbosity": "high"` in `codemason.json` to start at a level. The handler is `App::slash_verbosity` in `crates/codemason-app-core/src/app/slash.rs`.

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
