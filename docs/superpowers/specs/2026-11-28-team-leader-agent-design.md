# Team Leader Agent - Design

Date: 2026-11-28
Status: awaiting approval

## Intent

Today the primary agent calls `team_create` itself and becomes the team's
lead. A session can drive only one live team, so the primary can never run
two teams at once, and every teammate's coordination traffic lands in the
primary's context.

The change: the primary spawns a **team leader** agent per team. The leader
creates and runs its team, waits for it to finish, and reports **one
summary** back to the primary. Several leaders can run at once, so several
teams can run at once. The leader and its teammates are visible in the
Sessions pane, where the user can watch their output and type to them
directly.

## Goals

- The primary can run two or more teams concurrently, one leader per team.
- The primary receives only the leader's final summary, never teammate
  chatter.
- A dedicated `team-leader` agent type exists, with a prompt and tool set
  tailored to running a team, and it can be specialised (see "Specialising
  the leader").
- Leader and teammate sessions appear in the Sessions pane, nested under the
  session that spawned them; opening one shows its live output, and typing
  in it sends a direct user turn to that agent.

## Non-goals

- Mid-flight monitoring or steering of a team *by the primary agent*. The
  primary gets the final result only; live steering is the human's job,
  through the Sessions pane.
- Teams coordinating with each other.
- Changing how teammates coordinate inside a team (tasks, mailbox, plans).

## Anti-goals (a failure even if it works)

- Teammate traffic leaking into the primary's context.
- Two teams interfering: one team's manager, roster, task list or watchdog
  acting on another team.
- Child sessions cluttering the Sessions pane permanently as top-level rows.

## Current state (what the change touches)

- `SessionProcessor.team_manager` is a single `TeamManagerSlot`
  (`crates/codemason-agent/src/session/processor.rs:539`). One processor
  serves the primary and all its sub-agents, and `ensure_team_manager`
  (`processor.rs:826`) keeps a live manager bound and refuses a second team.
  **This, not who calls `team_create`, is what blocks multiple teams.**
- `TeamContext` (`crates/codemason-agent/src/tool/mod.rs:152`) resolves a
  session's team by matching `lead_session_id`; when the leader session calls
  `team_create`, it becomes the lead naturally.
- `new_agent` refuses calls made while a team context is active
  (`REASON_TEAM_CONTEXT_ACTIVE`, `tool/new_agent.rs:32`).
- Background `new_agent` results are already injected into the parent on
  completion and awaitable with `wait_agents` / listable with `list_agents`.
- Sub-agents and teammates are already stored sessions with a `parent_id`
  and a `SessionKind` (`session/mod.rs:264`, `create_child_session`); the
  sidebar deliberately hides them.
- Built-in agent types live in `crates/codemason-agent/src/agent/mod.rs`
  (`builtin_agents`); custom agents load from `.codemason/agents/` (project),
  `~/.config/codemason/agents/`, `~/.codemason/agents/`
  (`agent/custom.rs`); per-name prompt/model/temperature overrides come from
  `config.agent.<name>` (`resolve_agent`).
- The Teams pane and `Alt+Up/Down` teammate focus show a single team's
  members inside the lead session's view (`panels/teams.rs`,
  `cycle_focused_teammate`).

## Approaches considered

1. **Leader is a `new_agent` sub-agent of a new `team-leader` type** -
   *chosen.* The primary calls `new_agent agent="team-leader"
   background=true` per team; the leader calls `team_create`, `team_wait`,
   reads results, and returns a summary through the existing background
   result path. Least new machinery; the leader can adapt (re-assign,
   spawn extra teammates). Cost: one extra model in the loop per team.
2. **Dedicated `team_launch` tool** - code creates the leader session and
   team, waits, then asks the leader only to summarise. More predictable
   and cheaper, but a new tool and lifecycle to maintain, and the leader
   cannot steer its team. Rejected.
3. **No leader; primary runs N teams directly** over a multi-manager
   runtime. Works mechanically but puts every team's traffic in the
   primary's context. Rejected.

The user picked approach 1 and asked that the leader be a new agent type
that can be specialised for the role.

## Design

### 1. One team manager per team

Replace the single `TeamManagerSlot` on `SessionProcessor` with a registry
keyed by team name (e.g. `TeamManagers`: `team_name -> Arc<TeamManager>`).

- `ensure_team_manager` binds a manager for the calling session's team if
  that team has none, and unbinds managers whose team is gone. It never
  touches another live team's entry.
- Every current reader of the slot (`team_spawn`, `team_wait`,
  `team_task_cancel`, app-core `event_handler.rs` / `models.rs`, the Teams
  pane) looks up the manager for *its* team via `TeamContext.team_name`
  instead of reading "the" manager.
- Each manager keeps its own watchdog, roster and task-list binding, as
  today; the existing "a manager drives exactly one team" guard
  (`manager.rs:3908`) stays.

### 2. The `team-leader` agent type

A new built-in agent in `builtin_agents()` named `team-leader`.

- **Prompt:** run exactly one team for the task you were given: pick a
  blueprint, call `team_create` with full `context`, call `team_wait`, read
  `team_status` and the team's output, handle stragglers (re-assign or shut
  down), then clean up the team and return a concise summary of results,
  files written and anything unresolved. Never do the team's work itself.
- **Tools:** the team management tools (`team_create`, `team_wait`,
  `team_status`, `team_spawn`, task and message tools, `team_cleanup`) plus
  read-only file tools for reading results. No `new_agent` (no nested
  leaders or ad-hoc sub-agents).
- **Return value:** its final message is the summary the primary receives.

### 3. Specialising the leader *(builder's call on mechanism, intent fixed)*

Proposed mechanism, reusing what exists:

- `config.agent.team-leader` overrides (prompt, model, temperature) apply
  as for any built-in.
- A custom agent file may declare itself a leader (e.g. a `role:
  "team-leader"` field in its frontmatter / OASF payload). Such an agent
  gets the leader tool policy and can be passed to `new_agent` by name
  (e.g. `review-leader` with a prompt that always uses the `code-review`
  blueprint and a house summary format).
- Only agents of the leader role may call `team_create`.

### 4. Who may create teams

- The primary agent no longer calls `team_create` directly; its tool
  description and `AGENTS.md` "Team Workflow" guidance change to "spawn a
  `team-leader` with `new_agent` (background) and wait for it with
  `wait_agents`".
- **Every entry point goes through a leader (decided: 1a).** `/team
  create` (`app/slash.rs`), the swarm path (`app/swarm.rs`) and headless,
  ACP/server and automation runs no longer make the primary a team lead;
  they spawn a `team-leader` that creates the team. The "primary is lead"
  mode is removed: the Teams pane, `Alt+Up/Down` teammate focus and the
  app-core manager lookups are refactored to the per-team registry, not
  kept alongside it. Scripts or automations that call `team_create` from a
  non-leader session break by design and get a refusal naming
  `new_agent agent="team-leader"` as the replacement.
- Teammates cannot create teams or spawn leaders (no nesting beyond
  primary -> leader -> teammates).
- `new_agent`'s `REASON_TEAM_CONTEXT_ACTIVE` guard stays for teammates; it
  must not block the primary from spawning further leaders while other
  teams run (the primary is no longer a team lead).

### 5. Sessions pane visibility

- `sessions_sidebar_state` lists leader and teammate child sessions nested
  under the session that spawned them:

  ```
  v codemason
    Fix auth flow                (primary)
      > team-leader: code-review
          reviewer-security
          reviewer-perf
      > team-leader: docs-sweep
          writer-1
  ```

- Leader rows are collapsible; the label is `team-leader: <team name>`;
  teammate rows use the teammate's `name` and show its status (working,
  idle, done) the way session rows show attention today.
- When a team finishes and is cleaned up, its rows disappear from the
  default view (still reachable through the parent's history, as child
  sessions are archived with their parent today).
- Other sub-agent kinds (plain `new_agent` tasks, forked skills) stay
  hidden as today.

### 6. Typing into a leader or teammate

- Opening a child row switches the main view to that session's live
  messages (the same rendering teammate output uses today).
- Submitting a prompt there delivers a **direct user turn** to that agent
  (decided: option A, not a mailbox message). If the agent is mid-step the
  turn is queued and runs at its next step, as user prompts to a busy
  session do today.
- The leader is not automatically told about a user's redirect of one of
  its teammates (accepted trade-off of option A).
- **A finished leader still answers, in its own session only (decided:
  2a).** After the leader has returned its summary, a user turn to it
  runs normally and its reply stays in its row; nothing further is
  delivered to the primary.

### 7. Calls made while writing this up *(designer's calls, correct if wrong)*

- **Unique team names.** `open_team_store` silently reopens an existing
  team of the same name, so two leaders choosing `code-review` would join
  one team. A `team_create` call from a leader that names an existing
  active team is refused; when the name is omitted the generated
  `<blueprint>-<timestamp>` name gains a short suffix on collision. The
  manager registry stays keyed by team name, which is now unique.
- **Leaders and background limits.** A leader blocked in `team_wait` must
  not be killed by the background stall timeout: `team_wait` reports
  progress while teammates are active, so a waiting leader counts as
  live. Leaders count against `experimental.max_background_agents` like
  any background agent; no separate limit.
- **User turns interrupt `team_wait`.** A direct user turn to a leader
  that is blocked in `team_wait` ends the wait early (the tool returns a
  "user message pending" result) so the turn runs now rather than after
  the whole team finishes.
- **Sidebar signal.** Rows show while the team is `Active`. When the team
  is cleaned up or closed, its rows leave the default view. If the leader
  session ends while the team is still active (crash), its rows stay,
  greyed, until `/team cleanup` or `/team forcecleanup` removes the team.
- **Enforcement in code.** "Only a leader may call `team_create`" is
  checked in the tool, not only in prompts.

## Constraints

- Rust 2024, workspace rules in `AGENTS.md` / `AGENTS-LANGUAGE.md`: tests in
  each crate's `tests/` via `tests/all.rs`, no new `unsafe`, no `.unwrap()`
  on user-facing paths, `cargo fmt`/`clippy` clean.
- GUI work uses `codemason_gui::material` components.
- Both front ends that read the sidebar state (GUI, TUI) must keep working.
- Existing on-disk team format (`config.json`, tasks, mailboxes) is
  unchanged.

## Left to the builder

- Name and shape of the per-team manager registry and its API.
- Exact leader prompt wording and tool list beyond the intent above.
- The concrete field that marks a custom agent as a leader.
- How the primary is told which blueprints exist (pass-through in the
  leader prompt vs. the leader discovering them).
- Sidebar row styling within the Material components.
- Migration of existing tests that assume one manager per processor.
