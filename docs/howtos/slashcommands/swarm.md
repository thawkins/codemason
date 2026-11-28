# /swarm

> Auto-decompose a goal into parallel subtasks (/swarm <prompt> | /swarm status | /swarm help)

## Overview

`/swarm` takes a goal prompt and asks the configured model to decompose it
into parallel subtasks that are then executed by teammates. It is the quickest
way to fan a large piece of work out across a team without hand-writing the
task list: the model proposes the decomposition, and the swarm runtime drives
the subtasks from there.

An optional `--agent <type>` prefix sets the default agent type used for the
decomposed subtasks.

## Syntax

```
/swarm <prompt>
/swarm --agent <type> <prompt>
/swarm status
/swarm cancel
/swarm help
```

## Options / Subcommands

| Form | Description |
|------|-------------|
| `/swarm <prompt>` | Decomposes the prompt into subtasks and starts the swarm. |
| `--agent <type>` | Optional prefix flag; sets the default agent type for the subtasks. |
| `status` | Shows current swarm progress. |
| `cancel` | Cancels the active swarm. |
| `help` | Prints usage. |

## Guards

- Starting a second swarm while one is active is refused:
  `[warn] A swarm is already active - use /swarm cancel first`
- Starting a swarm without a configured model is refused:
  `[warn] /swarm requires a configured model - use /model`

## Examples

```
/swarm Refactor the config parser and update every caller in the tests
```

```
/swarm --agent coder Split the migration into per-crate tasks and run them
```

```
/swarm status
```

```
/swarm cancel
```

```
/swarm help
```

## Output

- On start, the status line shows `[wait] swarm: decomposing goal...` while
  the decomposition request is in flight.
- Decomposition is an asynchronous LLM call: the command returns immediately
  and the decomposed subtasks are reported as the model produces them.
- `/swarm status` reports the running swarm's progress.
- `/swarm cancel` stops the active swarm, after which a new `/swarm` can be
  started. A synthesis call still in flight is aborted, and an answer for a
  swarm that is no longer current is dropped.

### Dependent subtasks inherit their predecessors' results

A decomposed subtask may carry a `depends_on` list. Those subtasks run in
dependency order: a subtask whose dependencies have not completed is not
spawned, and a teammate completes its work with `team_task_complete`, passing an
optional `result` (a short account of what it found or changed, cut at 16 KiB).
The result is stored on the task in `tasks.json`.

When a blocked subtask is unblocked, its prompt is rebuilt with a **`Results
from dependencies`** section carrying up to 4 KiB of each predecessor's result,
so a follow-up task starts with the findings rather than rediscovering them. A
predecessor that recorded no result is marked
`(no result recorded - inspect this task's file changes directly)` rather than
silently omitted, and the teammate is told to pass its own `result` so its
dependents receive it in turn.

A swarm team runs with `auto_claim_tasks` off and gives each task to its own
teammate: a ready task is assigned at spawn, and a dependent one is reserved for
its teammate (assigned but still pending) and claimed when it unblocks. An idle
teammate therefore never takes another subtask with only a task id - it always
has that task's description and dependency results.

Each swarm teammate gets a readable name prefixed by its agent type,
`<agent_type>-<adjective>-<surname>` (for example `explore-focused-turing`).
In the Sessions pane a teammate that is still waiting on its dependencies is
listed dimmed and cannot be opened until it starts; its tooltip says it is
blocked and lists the teammates it is waiting for.

### The swarm's team leader synthesises one answer

A swarm team is led by a `team-leader` sub-agent of your session. The leader
waits until no subtask can make further progress (it never re-assigns tasks or
shuts teammates down; blocked teammates start as their dependencies finish, and
a subtask blocked only on failed or cancelled ones counts as finished), then
combines the completed results, naming what failed or never ran, into a single answer following `SYNTHESIS_SYSTEM_PROMPT`,
calls `team_cleanup`, and returns the answer to your session like any
background `new_agent` result. The swarm view shows the per-task status table
when the subtasks are done. `/swarm cancel` closes the team and cancels its
leader.

## Notes

- A model must be selected before a swarm can start (`/model`).
- The decomposed subtasks run as teammates; use `/team tasks` to watch the
  shared task list fill up, and `/swarm status` for swarm-level progress. The
  task list's completion table has a Result column showing what each task
  reported.
- Only one swarm may be active at a time.

## Related

- `/team` -- team lifecycle commands for the teammates a swarm creates
- `/blueprints` -- list installed team blueprints
- `/model` -- required before a swarm can start
- docs/howtos/teams.md -- teams and swarm how-to manual