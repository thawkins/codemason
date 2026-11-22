# /compact
> Summarise and compact the conversation history

## Overview

`/compact` replaces the current session's message history with an LLM-generated
summary plus the most recent turns. The session processor compacts the stored
history, the in-memory transcript is swapped for the result, and the context
window measurement drops immediately. Use it when token usage approaches the
model's context limit or output starts to degrade.

Compaction is asynchronous: the command starts a background compaction task,
shows a `compacting...` status, and updates the message window when the summary
arrives. The same engine also runs automatic compaction (configured in the
`compaction` section of `codemason.json`), and manual compaction inherits the
post-compaction queued-prompt drain so a prompt submitted during compaction is
sent after the history swap completes.

`/compress` is a deprecated alias for `/compact` and forwards to the identical
code path; it additionally logs a deprecation notice. Do not create new
content that uses `/compress`.

## Syntax

```
/compact
```

Bare `/compact` takes no arguments and operates on the current session only.
`/compact help` prints the usage. The only subcommand is `prune`, described
below.

Alias:

```
/compress        # deprecated alias, identical behaviour
```

### `/compact prune`

Every compaction anchors on the previous summary, so a later summary already
merges in everything still true from the earlier ones, and only the newest is
meaningful. The pre-send path nevertheless only *appends*, so a long-lived
session accumulates one summary block per compaction and the transcript builder
replays each as a separate assistant turn.

`/compact prune` is the delete that clears those superseded copies:

| Form | Behaviour |
| --- | --- |
| `/compact prune` | Delete every compaction summary except the newest one |
| `/compact prune --all` | Delete every compaction summary, behind a confirmation prompt |
| `/compact prune --dry-run` | Report the count that would be deleted and delete nothing |

The count and the delete come from
`Storage::delete_compaction_messages(session, keep)` /
`Storage::count_compaction_messages(session, keep)`. The request side is
filtered whether or not you ever prune: `session::history::drop_superseded_summaries`
collapses consecutive summaries to the newest before the transcript is built, so
the model never sees a superseded copy even in a session that has never been
pruned. `/compact prune` is what removes the stored rows.

`--all` never deletes directly. It opens the `Delete every compaction summary?`
dialog with `No` preselected, so a stray `Enter` deletes nothing; only an
explicit `Yes` runs the delete. `--dry-run` never opens the dialog.

## Options / Subcommands

| Form | Description |
| --- | --- |
| `/compact` | Start manual compaction of the current session history |
| `/compress` | Deprecated alias of `/compact`; logs a deprecation line then behaves identically |
| `/compact prune` | Delete every compaction summary but the newest |
| `/compact prune --all` | Delete every compaction summary, with a confirmation |
| `/compact prune --dry-run` | Report what prune would delete, deleting nothing |
| `/compact help` | Show the usage |

## Examples

Compact the current session:

```
/compact
```

Same result via the deprecated alias (avoid in new sessions):

```
/compress
```

Combine with a cost check first, to decide whether compaction is worthwhile:

```
/cost
/compact
```

Check token headroom after compaction:

```
/compact
/context
```

Queue the next prompt while compaction runs; it is sent automatically when the
history swap completes:

```
/compact
continue with the next test file
```

Delete the superseded summaries a long-lived session has accumulated, keeping
the newest:

```
/compact prune
```

See what `--all` would delete before committing to it:

```
/compact prune --all --dry-run
```

## Output

What appears in the message window and status bar:

- Status `compacting...` while the summary is being generated (manual
  compaction; auto compaction shows `compacting before send...`).
- On success the conversation history is replaced with a compaction summary
  followed by the retained recent messages; the status returns to `ready`, a
  log line records that session history was replaced with the summary, and the
  context-snapshot refresh is scheduled.
- On failure the status shows `[warn] compact failed: <error>` and the failure
  is logged.
- Guard messages when compaction cannot start or continue:
  - No active session: `[warn] No active session to compact`
  - Empty history: `[warn] No messages to compact`
  - No model selected: `[warn] No model selected - use /model to choose`

## Related

- `/cost` - token usage and estimated cost for the session
- `/context` - context cache management and window measurement
- `/model` - switch the model used for compaction and chatting
- `/clear` - wipe history entirely (no summary retained)
- `compaction` config section in `codemason.json` - automatic compaction,
  threshold, buffer, summary token cap, and optional dedicated compaction model