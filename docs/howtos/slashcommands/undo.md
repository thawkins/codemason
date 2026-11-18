# /undo

> "Remove the last turn and restore the files it changed; /redo puts it back (/undo help)" (SlashCommandDef, crates/codemason-app-core/src/app/state.rs).

## Overview

`/undo` removes the last turn: the most recent user message and everything
after it. The messages are deleted from the session in memory and in the
session database, so the model no longer sees the turn.

When the turn has a workspace snapshot, the files are restored to their state
from before the turn: edited files go back, created files are deleted and
deleted files return. `/redo` puts the messages and the files back.

The handlers are `App::handle_undo` and `App::handle_redo` in
`crates/codemason-app-core/src/app/undo.rs`; the snapshots are
`crates/codemason-agent/src/session/turn_snapshot.rs`.

## Syntax

```
/undo
/undo help
```

Any other argument is ignored; the command proceeds as bare `/undo`.

## Workspace snapshots

- A snapshot is taken once per turn, immediately before the turn's first tool
  call. A turn that calls no tool has no snapshot, and undoing it changes no
  file.
- Snapshots are recorded only for a session whose directory is inside a git
  work tree, on the `local` execution backend, for a top-level run (not a
  sub-agent's own session). The snapshot covers the whole work tree, so file
  changes made by sub-agents and shell commands during the turn are undone too.
- Snapshots live in a private git repository per project under
  `~/.config/codemason/snapshots/`. The project's own `.git` (history, index,
  stash) is never touched. Files matched by `.gitignore` are neither recorded
  nor restored.
- Undo restores the work tree to the snapshot. Edits you made by hand after
  the turn started are reverted with it; `/redo` brings them back, because redo
  restores the exact state the undo replaced.

## Examples

### Remove the last turn

```
/undo
```

Status line:

```
Undid last turn (removed 2 message(s), 3 file(s) changed); /redo restores it
```

The log pane lists the changed files. `/undo` can be repeated to remove
earlier turns one at a time.

### Show help

```
/undo help
```

## Messages

| Status | Meaning |
|---|---|
| `[warn] No active session to undo` | No session is focused |
| `[warn] Cannot undo while the agent is running` | Stop the agent (`Esc`) first |
| `[warn] Cannot undo while another session is running in this work tree` | Another open session's agent is working in the same git work tree; wait for it or stop it |
| `[warn] No messages to undo` | The conversation is empty |
| `[warn] No user message found to undo` | The conversation has no user turn |
| `... files NOT restored - see log` | The messages were removed but git could not restore the files; the log has the reason |

## Notes

- The removed turn is kept for `/redo` in memory only: it is lost when
  codemason exits, and dropped as soon as a new message is sent.
- Side effects outside the work tree (a pushed commit, a sent request, a
  written memory) are not undone.
