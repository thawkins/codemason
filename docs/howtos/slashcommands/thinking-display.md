# /thinking-display

> "Set how thinking text shows: /thinking-display show|hide|collapse" (SlashCommandDef, crates/codemason-app-core/src/app/state.rs).

## Overview

`/thinking-display` sets how thinking text is presented, held in `App::transcript_prefs`. It does not change the model's thinking level; that is `/thinking`. The default is collapsed. The setting lasts until codemason exits; set `"thinking_display"` to `collapsed`, `expanded` or `hidden` in `codemason.json` to start in a mode. The handler is `App::slash_thinking_display` in `crates/codemason-app-core/src/app/slash.rs`.

## Syntax

```
/thinking-display
/thinking-display show
/thinking-display hide
/thinking-display collapse
```

## Notes

- `show` is expanded, `hide` is hidden, `collapse` is collapsed.
- A bare form reports the current mode; an unknown value changes nothing.
