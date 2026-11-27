# Team Leader Agent Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** The primary agent runs any number of concurrent teams, each created and run by a `team-leader` sub-agent that returns one summary; leaders and teammates are visible and promptable in the Sessions pane.

**Architecture:** Replace the processor's single `TeamManagerSlot` with a per-team `TeamManagers` registry. Add an `AgentRole` to `AgentInfo`, a built-in `team-leader` agent, and gate `team_create` on that role. Every team entry point (`/team create`, `/swarm`, the tool) goes through a leader. The sidebar builds leader/teammate rows from active teams, and `dispatch_user_message` routes prompts to the open child session.

**Tech Stack:** Rust 2024, tokio, parking_lot, egui (GUI), cargo nextest.

**Spec:** `docs/superpowers/specs/2026-11-28-team-leader-agent-design.md`

## Global Constraints

- Tests live in each crate's `tests/`, one `test_*.rs` per unit, registered as `#[path = "test_x.rs"] mod test_x;` in that crate's `tests/all.rs`. No new inline `#[cfg(test)]` modules.
- Run tests with `timeout 1500 cargo nextest run -p <crate> -E 'test(<file>::)'`; never a full workspace run per task.
- Tests write only under `target/temp/` (resolved from `env!("CARGO_MANIFEST_DIR")`); tests touching env/cwd take `crate::global_env_lock()`.
- `cargo fmt` after every `.rs` edit; `cargo clippy` clean; no new `unsafe`; no `.unwrap()`/`.expect()` on user-facing paths.
- Exhaustive `match` on enums (no `_` arm on our own enums).
- `///` docs on every new public item; `//!` docs on every new module.
- GUI drawing uses `codemason_gui::material` components.
- On-disk team format (`config.json`, tasks, mailboxes) is unchanged.
- Agent type name is exactly `team-leader`; custom-agent role value is exactly `"team-leader"`; leader sidebar label is exactly `team-leader: <team name>`.

## Review Focus

1. **Two leaders pick the same blueprint at the same moment.** Expect two distinct teams (`code-review-<ts>` and `code-review-<ts>-2`), never one shared team. Test in Task 4.
2. **Leader crashes or is cancelled mid-team.** Expect the team's rows to stay, greyed, and teammates to keep their manager until `/team cleanup`. Test in Task 8.
3. **User types into a teammate that is idle/parked vs. one that has exited.** Idle/parked: woken and runs the turn. Exited/failed: the prompt is refused with a visible error, not silently dropped. Test in Task 7.
4. **App restarts while teams are active.** Rows are rebuilt from active teams on disk; a leader that is not running shows greyed. Test in Task 8.
5. **A third team starts while two are running.** The new manager binds without unbinding or disturbing the other two (watchdogs, task lists). Test in Task 1.

---

### Task 1: Per-team manager registry in `codemason-agent`

**Files:**
- Create: `crates/codemason-agent/src/team/registry.rs`
- Modify: `crates/codemason-agent/src/team/mod.rs` (module + re-export, `//!` table row)
- Modify: `crates/codemason-agent/src/team/manager.rs:631-700` (remove `TeamManagerSlot`)
- Modify: `crates/codemason-agent/src/session/processor.rs:539, 825-875, 3297-3311`
- Modify: `crates/codemason-agent/src/research_adapter.rs:266`, `src/main.rs:1423`
- Test: `crates/codemason-agent/tests/test_team_registry.rs`

**Interfaces:**
- Produces, in `team/registry.rs`:
  - `#[derive(Default)] pub struct TeamManagers` (inner `parking_lot::RwLock<HashMap<String, Arc<TeamManager>>>`; `Debug` lists team names)
  - `pub fn get(&self, team_name: &str) -> Option<Arc<TeamManager>>`
  - `pub fn bind(&self, manager: Arc<TeamManager>) -> Result<(), Arc<TeamManager>>` - refuses when a *live* manager for the same team is bound; replaces a stale one
  - `pub fn remove_if(&self, expected: &Arc<TeamManager>) -> bool`
  - `pub fn prune_stale(&self) -> Vec<Arc<TeamManager>>` - removes and returns managers whose `team_is_live()` is false
  - `pub fn all(&self) -> Vec<Arc<TeamManager>>` - sorted by team name
- `SessionProcessor.team_managers: crate::team::TeamManagers` replaces `team_manager`.
- `ensure_team_manager(&self, team_context, working_dir, model)` keeps its signature; it prunes stale managers, then binds this lead's team if absent. It never touches another live team's entry.
- `ToolContext.team_manager` stays `Option<Arc<dyn TeamManagerInterface>>`, filled from `team_managers.get(&tc.team_name)`.

- [ ] **Step 1: Write failing tests** in `test_team_registry.rs` (build managers over `target/temp/test_team_registry/<test>` team dirs):
  - `test_team_registry_binds_two_teams_independently`: bind `alpha` and `beta`; `get("alpha")` and `get("beta")` are both `Some`, and `all().len() == 2`.
  - `test_team_registry_refuses_second_live_manager_same_team`: second `bind` for live `alpha` returns `Err`.
  - `test_team_registry_replaces_stale_manager`: delete `alpha`'s `config.json`; `bind` of a new `alpha` manager is `Ok`.
  - `test_team_registry_prune_keeps_live_teams`: with `alpha` stale and `beta` live, `prune_stale()` returns `[alpha]`, and `get("beta")` is still `Some` and `Arc::ptr_eq` with the original (Review Focus 5).
  - `test_team_registry_remove_if_ignores_other_manager`: `remove_if` with a different `Arc` for the same name returns `false`.
- [ ] **Step 2: Run** `timeout 1500 cargo nextest run -p codemason-agent -E 'test(test_team_registry::)'`. Expected: FAIL to compile (`TeamManagers` not found).
- [ ] **Step 3: Implement `TeamManagers`**, swap the processor field, and rewrite `ensure_team_manager` and the `ToolContext` fill at `processor.rs:3311`. Delete `TeamManagerSlot` and its re-export. Fix construction sites.
- [ ] **Step 4: Run** the Step 2 command plus `cargo check --workspace`. Expected: the registry tests PASS; the workspace fails only in app-core (fixed in Task 2).
- [ ] **Step 5: Commit** `feat(team): per-team manager registry`. Commit Task 1 and Task 2 together if the workspace must compile at every commit.

### Task 2: Migrate app-core to the registry; Teams pane lists every team

**Files:**
- Modify: `crates/codemason-app-core/src/app/models.rs:1062-1145`
- Modify: `crates/codemason-app-core/src/app/event_handler.rs:2066-2081, 2384-2393, 2486-2637, 3124-3163, 3318-3380`
- Modify: `crates/codemason-app-core/src/app/swarm.rs:253, 554`, `app/slash.rs:7460, 8184, 13822`
- Modify: `crates/codemason-gui/src/panels/teams.rs` (and the TUI `layout_teams.rs` row builder it mirrors)
- Test: `crates/codemason-app-core/tests/test_teams_multi.rs`

**Interfaces:**
- Consumes: `TeamManagers` from Task 1.
- Every app-core `team_manager.get()` becomes `team_managers.get(<team name from the event / row / context>)`. Team events already carry the team name; use it, never "the first manager".
- `ensure_team_manager_for_team_inner(&mut self, team_name, known_team_dir, reconcile)` keeps its signature and uses `prune_stale` + `bind`.
- Teams pane: one section per `team_managers.all()` entry, each with the existing `Team: name (lead + N teammates)` title and table. `teams_row_agent_ids` keys become `(team_name, agent_id)`. `cycle_focused_teammate` walks teammates across all teams in pane order; `SessionView.focused_teammate: Option<(String, String)>` = `(team_name, agent_id)`.

- [ ] **Step 1: Write failing tests** in `test_teams_multi.rs`, using `support::make_app()` with `cwd` pinned to `target/temp/test_teams_multi/<test>`:
  - `test_teams_multi_events_route_to_own_team`: with two bound teams, a `TeammateIdle` event for team `beta` updates `beta`'s member status and leaves `alpha`'s untouched.
  - `test_teams_multi_focus_cycles_across_teams`: with teams `alpha` (2 teammates) and `beta` (1), three `cycle_focused_teammate(true)` calls visit `(alpha,tm-1)`, `(alpha,tm-2)`, `(beta,tm-1)`.
  - `test_teams_multi_rows_list_every_team`: the teams row builder yields two `Team:` title rows.
- [ ] **Step 2: Run** `timeout 1500 cargo nextest run -p codemason-app-core -E 'test(test_teams_multi::)'`. Expected: FAIL.
- [ ] **Step 3: Migrate every call site** listed under Files and convert `focused_teammate` to the tuple. Update existing tests that construct `focused_teammate` or read `team_manager`.
- [ ] **Step 4: Run** the Step 2 command, plus `-E 'test(test_slash_teams_deep::) + test(test_slash_arms_deep::)'` on app-core and `cargo build -p codemason-gui`. Expected: PASS and build OK.
- [ ] **Step 5: Commit** `refactor(app): route team state through the per-team registry`

### Task 3: `AgentRole` and the built-in `team-leader` agent

**Files:**
- Modify: `crates/codemason-agent/src/agent/mod.rs:475-575` (field), `:949-975` (`builtin_agent` default), `:995` roster, `:1076` permissions helpers
- Modify: `crates/codemason-agent/src/agent/oasf.rs:109`, `agent/custom.rs:300, 449-590`
- Modify: `crates/codemason-agent/src/tool/mod.rs:496` (`ToolContext.agent_role`) and its builders
- Test: `crates/codemason-agent/tests/test_agent_team_leader.rs`

**Interfaces:**
- Produces: `#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)] #[serde(rename_all = "kebab-case")] pub enum AgentRole { #[default] Standard, TeamLeader }` in `agent/mod.rs`.
- `AgentInfo.role: AgentRole`; `AgentInfo::new` and `builtin_agent` default to `Standard`.
- `pub fn team_leader_permissions() -> PermissionRuleset`: read tools allow, edit deny, bash deny, `team:*` categories allow, `new_agent` deny.
- Built-in `team-leader`: `AgentInfo { role: AgentRole::TeamLeader, ..builtin_agent("team-leader", "Creates, runs and summarises one agent team", AgentMode::Subagent, team_leader_permissions(), TEAM_LEADER_PROMPT) }`. `TEAM_LEADER_PROMPT` covers: run exactly one team for the task; pick a blueprint; `team_create` with full `context`; `team_wait`; read `team_status` and outputs; re-assign or shut down stragglers; `team_cleanup`; final message = concise summary (results, files written, unresolved items); never do the team's work itself.
- `CodemasonAgentPayload.role: Option<AgentRole>` and `ProfileFrontmatter.role: Option<AgentRole>`, mapped in `record_to_agent_info`.
- `ToolContext.agent_role: AgentRole`, set from the running agent's `AgentInfo.role`.
- `config.agent.team-leader` overrides keep working through `resolve_agent`.

- [ ] **Step 1: Write failing tests:**
  - `test_agent_team_leader_builtin_exists`: `resolve_agent("team-leader", &Config::default())` has `role == AgentRole::TeamLeader` and `mode == AgentMode::Subagent`.
  - `test_agent_team_leader_denies_new_agent_and_edit`: its permission check gives `Deny` for `new_agent` and for edit, `Allow` for `team:manage`.
  - `test_agent_team_leader_custom_md_role`: a `.md` agent in `target/temp/.../.codemason/agents/review-leader.md` with frontmatter `{"name":"review-leader","role":"team-leader"}` loads with `role == TeamLeader`.
  - `test_agent_team_leader_config_prompt_override`: `config.agent["team-leader"].prompt = "X"` gives `prompt == "X"` and keeps `role == TeamLeader`.
  - `test_agent_team_leader_default_role_standard`: `resolve_agent("explore", ..).role == AgentRole::Standard`.
- [ ] **Step 2: Run** `... -p codemason-agent -E 'test(test_agent_team_leader::)'`. Expected: FAIL.
- [ ] **Step 3: Implement** the enum, field, helper, built-in, custom mapping and `ToolContext.agent_role`. A custom agent with `role: team-leader` and no `permissions` gets `team_leader_permissions()`.
- [ ] **Step 4: Run** the Step 2 command. Expected: PASS.
- [ ] **Step 5: Commit** `feat(agent): team-leader agent type and AgentRole`

### Task 4: Gate `team_create` on the leader role; unique team names

**Files:**
- Modify: `crates/codemason-agent/src/tool/team_create.rs:81-275` (description, execute, `open_team_store`)
- Test: `crates/codemason-agent/tests/test_team_create_leader_gate.rs`

**Interfaces:**
- Consumes: `ToolContext.agent_role` (Task 3).
- Produces: `pub const REASON_NOT_TEAM_LEADER: &str = "not_team_leader";` in `team_create.rs`.
- Rule: `seed_only: true` is exempt (used by the leader launch path in Task 6). Otherwise `agent_role != TeamLeader` returns a `reason_error(REASON_NOT_TEAM_LEADER, "team_create may only be called by a team-leader agent. Spawn one with new_agent agent=\"team-leader\" background=true and pass the blueprint and context in its task.")`.
- Names: an explicit `name` that matches an existing **Active** team is refused, and the error names the team. A generated name that collides gets `-2`, `-3`, ... until it is free. The check-and-create runs under the team store's create lock, so concurrent creates cannot both win.

- [ ] **Step 1: Write failing tests:**
  - `test_team_create_leader_gate_refuses_standard_role`: `agent_role = Standard` gives an error whose reason is `not_team_leader`, and no team dir is created.
  - `test_team_create_leader_gate_allows_leader`: `agent_role = TeamLeader` with blueprint `code-review` gives `Ok`, and the team dir exists.
  - `test_team_create_leader_gate_seed_only_exempt`: `Standard` + `seed_only: true` on an existing team gives `Ok`.
  - `test_team_create_leader_gate_refuses_existing_active_name`: a second create with explicit `name: "alpha"` gives `Err` containing `alpha`.
  - `test_team_create_leader_gate_concurrent_generated_names_distinct`: two `tokio::join!` creates with the same blueprint and no name give two different team names (Review Focus 1).
- [ ] **Step 2: Run** `... -E 'test(test_team_create_leader_gate::)'`. Expected: FAIL.
- [ ] **Step 3: Implement** the gate and naming in `team_create.rs`, and update the tool description to say leader-only.
- [ ] **Step 4: Run** the Step 2 command and `-E 'test(test_team_create)'` (existing team_create tests; set `agent_role = TeamLeader` where they relied on the old behaviour). Expected: PASS.
- [ ] **Step 5: Commit** `feat(team): team_create is leader-only with unique names`

### Task 5: Leader runtime - no background cap; user input interrupts `team_wait`

**Files:**
- Modify: `crates/codemason-agent/src/task/mod.rs:817-831, 960-1000, 1324` (`spawn_background_mode`, timeout, `send_to_session`)
- Modify: `crates/codemason-agent/src/tool/team_wait.rs:290-340`
- Modify: `crates/codemason-agent/src/tool/mod.rs:496` (`ToolContext.user_input_pending`)
- Test: `crates/codemason-agent/tests/test_team_leader_runtime.rs`

**Interfaces:**
- Produces: `ToolContext.user_input_pending: Option<Arc<tokio::sync::Notify>>`, set for sub-agent sessions from the `AgentManager` inbox entry; `send_to_session` calls `notify_one()` after it pushes.
- For agents with `role == TeamLeader`, the configured `background_timeout_secs` cap is skipped: `effective_time_bound(time_bound_secs, None)`. A per-call `time_bound_secs` still applies. Leaders still count toward `max_background_agents`.
- `team_wait`'s wait loop `select!`s on `user_input_pending.notified()`. On wake it returns `metadata.interrupted = true`, `success = false`, and content starting `Wait interrupted: a user message is pending. Handle it, then call team_wait again.`

- [ ] **Step 1: Write failing tests:**
  - `test_team_leader_runtime_no_background_cap`: with `background_timeout_secs = 1`, a `TeamLeader` background task whose inner future sleeps 2 s ends `Completed`, not `Failed`; a `Standard` agent under the same setup ends `Failed`.
  - `test_team_leader_runtime_user_input_interrupts_wait`: `team_wait` on a team with one busy teammate, with `notify_one()` fired after 100 ms, returns within 1 s and `metadata["interrupted"] == true`.
  - `test_team_leader_runtime_wait_unaffected_without_input`: the same setup with no notify returns `timed_out == true` after `timeout_secs: 1`.
- [ ] **Step 2: Run** `... -E 'test(test_team_leader_runtime::)'`. Expected: FAIL.
- [ ] **Step 3: Implement** the role-based cap skip, the `Notify` wiring and the `select!` in `team_wait`.
- [ ] **Step 4: Run** the Step 2 command and `-E 'test(test_team_wait)'`. Expected: PASS.
- [ ] **Step 5: Commit** `feat(team): leaders outlive the background cap; user input interrupts team_wait`

### Task 6: Every entry point goes through a leader

**Files:**
- Create: `crates/codemason-agent/src/team/leader.rs`
- Modify: `crates/codemason-app-core/src/app/slash.rs:7336-7470` (`/team create`), `app/swarm.rs:31-160`
- Modify: `AGENTS.md` "Team Workflow" section, `docs/howtos/teams.md`
- Test: `crates/codemason-agent/tests/test_team_leader_launch.rs`, `crates/codemason-app-core/tests/test_slash_team_create_leader.rs`

**Interfaces:**
- Produces, in `team/leader.rs`:
  - `pub struct LeaderLaunch { pub leader_session_id: String, pub task_id: String, pub team_name: String }` (`Debug, Clone`)
  - `pub async fn launch_leader_for_team(agent_manager: &AgentManager, parent_session_id: &str, working_dir: &Path, team_name: &str, task: &str, model: Option<ModelRef>) -> anyhow::Result<LeaderLaunch>`. It creates the leader child session (`SessionKind::Subagent`, agent `team-leader`), runs `TeamStore::create(team_name, &leader_session_id, working_dir, true)`, and spawns the leader as a background (non-detached) task in that session. The parent receives the leader's result as with any background `new_agent`.
  - `pub fn leader_task_for_existing_team(team_name: &str, context: &str) -> String`. The fixed text tells the leader that team `<name>` already exists and is seeded, so it must not call `team_create`, and should go straight to `team_wait`, follow-up and summary.
- `/team create <name> <blueprint> <context>`: calls `launch_leader_for_team`, then seeds with the `team_create` tool using `seed_only: true` in the leader's context (`session_id = leader_session_id`). The primary is never the lead.
- `/swarm`: decomposition is unchanged. The team is created by `launch_leader_for_team` (lead = leader session), and `spawn_swarm_teammates` runs against the registry manager for that team. The leader's task is `leader_task_for_existing_team`.
- `AGENTS.md` example becomes: `new_agent agent="team-leader" background=true task="Use blueprint code-review. Context: ..."` then `wait_agents`.

- [ ] **Step 1: Write failing tests:**
  - `test_team_leader_launch_leader_is_lead`: after launch, `TeamStore::load_by_name(team).config.lead_session_id == leader_session_id`, and that session's `parent_id == parent_session_id`.
  - `test_team_leader_launch_parent_has_no_team_context`: `resolve_team_context_for_session(parent_session_id, dir)` is `None`.
  - `test_slash_team_create_leader_spawns_leader`: `/team create alpha code-review "x"` leaves one active task with agent `team-leader`, and `alpha`'s lead is that task's session.
  - `test_slash_team_create_leader_two_teams`: `/team create alpha ...` then `/team create beta ...` gives two active teams and two leader tasks.
- [ ] **Step 2: Run** `... -p codemason-agent -E 'test(test_team_leader_launch::)'` and `... -p codemason-app-core -E 'test(test_slash_team_create_leader::)'`. Expected: FAIL.
- [ ] **Step 3: Implement** `leader.rs`, rewire `/team create` and `/swarm`, and update `AGENTS.md` and `docs/howtos/teams.md`.
- [ ] **Step 4: Run** the Step 2 commands plus `-E 'test(test_slash_teams_deep::) + test(test_swarm)'` on app-core. Expected: PASS (update assertions that expected the primary as lead).
- [ ] **Step 5: Commit** `feat(team): route /team create and /swarm through a team leader`

### Task 7: Direct user turns to teammates and finished leaders

**Files:**
- Modify: `crates/codemason-agent/src/team/manager.rs:600-615` (handle), `:1527` (`next_teammate_turn`)
- Modify: `crates/codemason-agent/src/task/mod.rs:1324` (`send_to_session` fallback)
- Test: `crates/codemason-agent/tests/test_team_user_turns.rs`

**Interfaces:**
- Produces:
  - `pub fn send_user_turn(&self, agent_id: &str, text: &str) -> anyhow::Result<()>` on `TeamManager`. It pushes onto a new per-handle `user_turns: parking_lot::Mutex<VecDeque<String>>` and calls `wake.notify_one()`. A parked teammate is resumed. A shut-down or failed teammate returns `Err("teammate <name> has exited")`.
  - `next_teammate_turn` returns `TeammateNextTurn::Run(text)` from `user_turns` before it looks at mailbox or task work.
  - `pub async fn resume_detached(&self, child_session_id: &str, agent_name: &str, text: &str) -> anyhow::Result<String>` on `AgentManager`. It runs one more turn in an existing, finished child session as a **detached** background task (its result is never injected into the parent; decision 2a) and returns the task id.
  - `send_to_session` keeps its signature. When no running task owns the session, it returns `Err` with reason `task_not_found`, so the caller can fall back to `resume_detached`.

- [ ] **Step 1: Write failing tests:**
  - `test_team_user_turns_run_before_mailbox`: with a mailbox message and a queued user turn `"hi"`, `next_teammate_turn` gives `Run` containing `hi`.
  - `test_team_user_turns_wake_parked_teammate`: on a parked teammate, `send_user_turn` is `Ok` and its status leaves `Parked`.
  - `test_team_user_turns_exited_teammate_refused`: on a shut-down teammate, `send_user_turn` gives `Err` containing `has exited` (Review Focus 3).
  - `test_team_user_turns_resume_finished_leader_not_injected`: after a leader task completes, `resume_detached` runs a turn, and the parent session's injected-result count is unchanged.
- [ ] **Step 2: Run** `... -E 'test(test_team_user_turns::)'`. Expected: FAIL.
- [ ] **Step 3: Implement** the three functions and the `next_teammate_turn` priority.
- [ ] **Step 4: Run** the Step 2 command and `-E 'test(test_team_manager)'`. Expected: PASS.
- [ ] **Step 5: Commit** `feat(team): direct user turns to teammates and finished leaders`

### Task 8: Sessions pane - nested leader/teammate rows and prompt routing

**Files:**
- Modify: `crates/codemason-app-core/src/sessions_sidebar_state.rs:256, 512-720, 901-920`
- Modify: `crates/codemason-app-core/src/app/session_ops.rs:1065-1100`, `app/sessions.rs:131`
- Modify: `crates/codemason-gui/src/panels/sessions.rs` (row drawing)
- Modify: `GUI-QUICKSTART.md` (Sessions pane section), `QUICKSTART.md`, `SPEC.md`, `CHANGELOG.md` (Unreleased)
- Test: `crates/codemason-app-core/tests/test_sidebar_team_rows.rs`

**Interfaces:**
- Consumes: `team_managers.all()` (Task 1), `send_user_turn` / `resume_detached` / `send_to_session` (Task 7).
- Produces new `SidebarRow` variants:
  - `TeamLeader { session_id: String, team_name: String, running: bool, collapsed: bool }`, labelled `team-leader: <team_name>`.
  - `Teammate { session_id: String, team_name: String, agent_id: String, name: String, status: MemberStatus }`.
- Built by `push_team_rows(rows, app, parent_session_id)` directly under the parent `Session` row, for every **Active** team whose leader session's `parent_id` is that parent. A team is shown even when its leader task is not running, with `running: false` (drawn greyed; Review Focus 2 and 4). Plain `SubAgent` rows for `team-leader` tasks are suppressed so leaders are not listed twice. Rows disappear once the team is no longer Active.
- `activate_row` for both variants opens that child session's live message view (the same rendering teammate output uses).
- `dispatch_user_message` routing, before the primary path: open `Teammate` row → `send_user_turn`; open `TeamLeader` row → `send_to_session`, falling back to `resume_detached` on `task_not_found`. An `Err` is shown as an error toast with the error text, and the prompt text stays in the input.
- `Enter`/click on a `TeamLeader` row toggles `collapsed` when the disclosure triangle is hit (the same hit test as project headings).

- [ ] **Step 1: Write failing tests:**
  - `test_sidebar_team_rows_nested_under_parent`: a primary with two leaders (one with 2 teammates, one with 1) gives row order `Session`, `TeamLeader(a)`, `Teammate`, `Teammate`, `TeamLeader(b)`, `Teammate`.
  - `test_sidebar_team_rows_leader_not_listed_as_subagent`: no `SubAgent` row for the leader's task.
  - `test_sidebar_team_rows_crashed_leader_greyed`: with the leader task gone and the team Active, the row is `TeamLeader { running: false, .. }` and its teammates are still listed (Review Focus 2).
  - `test_sidebar_team_rows_rebuilt_after_restart`: a fresh `App` over the same `target/temp` dir with an Active team on disk lists the leader row with `running: false` (Review Focus 4).
  - `test_sidebar_team_rows_hidden_when_team_closed`: after the team status changes to closed, no team rows remain.
  - `test_sidebar_team_rows_prompt_to_teammate`: with a `Teammate` row open, `dispatch_user_message("go")` queues `go` on that teammate (`next_teammate_turn` gives `Run` containing `go`), and the primary session gets no new user message.
  - `test_sidebar_team_rows_prompt_to_exited_teammate_errors`: with the teammate shut down, the dispatch leaves the input text intact and sets an error toast containing `has exited`.
- [ ] **Step 2: Run** `... -p codemason-app-core -E 'test(test_sidebar_team_rows::)'`. Expected: FAIL.
- [ ] **Step 3: Implement** the variants, builder, activation, routing and GUI drawing (Material list rows; greyed via the palette's disabled role), and update the docs and CHANGELOG.
- [ ] **Step 4: Run** the Step 2 command, `-E 'test(test_sessions_sidebar)'` on app-core, and `cargo build -p codemason-gui`. Expected: PASS and build OK.
- [ ] **Step 5: Commit** `feat(sessions): show team leaders and teammates in the Sessions pane`

### Task 9: Branch verification

- [ ] **Step 1:** Run `cargo fmt --check && cargo clippy --workspace --all-targets -- -D warnings`. Expected: no output, exit 0.
- [ ] **Step 2:** Run `timeout 1500 cargo nextest run --workspace --no-fail-fast 2>&1 | tee target/temp/nextest-team-leader.log`. Expected: `0 failed`. Read failures from the log rather than re-running.
- [ ] **Step 3:** Run `cargo test --doc -p codemason-agent`. Expected: PASS.
- [ ] **Step 4:** Run `git status --porcelain`. Expected: only intended files changed; no test artifacts in the tree.
