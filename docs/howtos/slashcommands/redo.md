# /redo

> "Restore the turn the last /undo removed (/redo help)" (SlashCommandDef, crates/codemason-app-core/src/app/state.rs).

## Overview

`/redo` puts back the turn the last `/undo` removed: its messages return to the
conversation and the session database, and the workspace files return to the
state the undo replaced. After several `/undo` commands, each `/redo` restores
one turn, most recently undone first.

The handler is `App::handle_redo` in
`crates/codemason-app-core/src/app/undo.rs`. See [undo](undo.md) for how
workspace snapshots work.

## Syntax

```
/redo
/redo help
```

## Examples

```
/redo
```

Status line:

```
Redid last turn (restored 2 message(s), 3 file(s) changed)
```

## Messages

| Status | Meaning |
|---|---|
| `[warn] Nothing to redo` | No turn has been undone, or a message was sent since the undo |
| `[warn] Cannot redo while the agent is running` | Stop the agent (`Esc`) first |

## Notes

- Undone turns are kept in memory only; they do not survive a restart.
- A redone turn can be undone again.
