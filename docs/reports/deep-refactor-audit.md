# Deep-Refactor Audit

**Audited at**: `5b9f1b1` (Version 1.0.170), 2026-11-15, branch `work/2026-11-15`.
**Scope**: the whole workspace: 20 crates under `crates/` plus the root `codemason` binary (`src/`).
**Method**:
- Six read-only passes over disjoint areas:
  - app-core, gui and the root binary
  - agent
  - research and specs
  - llm, storage, telemetry, config and types
  - tools and codeindex
  - plugins, connectors, server, bench and build hygiene
- Scanner baseline: `cargo clippy --workspace --all-targets`, `cargo machete`, and every `scripts/check-*.sh`.
- Two second-pass sweeps:
  - the project-config trust boundary;
  - an adversarial re-check of each defect the first pass did not reproduce.

**Supersedes**: the previous version of this file, audited at `3de6683` (still in git history). Its open rows are reconciled in section 9.

Every count carries the command that produced it and the commit it was taken at.
Re-derive a count on the branch point of the car that uses it; never quote the
snapshot. Repro programs and per-area detail live in the git-ignored
`target/temp/deep-refactor/` tree (`area-*.md`, `sweep2-*.md`, `verify/repro/`).

Verdict key:
- **R**: reproduced by a scratch program or test against the real crate API.
- **C**: confirmed by tracing the code path.
- **P**: plausible; the code path is clear but it could not be reproduced.

---

## 1. Architecture verdict

**The crate layering is sound and needs no structural ticket. The debt is
(a) a porous trust boundary between user-global and per-project config,
(b) defects at I/O boundaries (pipes, archives, streams, symlinks), and
(c) god-functions, worst of all the 8,683-line slash dispatcher.**

Evidence:

- **No cycles.** The graph comes from `grep -oE '^codemason-[a-z-]+' crates/*/Cargo.toml` at `5b9f1b1`:
  - `types` is the base;
  - `config` sits on top of it;
  - storage, surface, codeindex, llm, tools, plugins and connectors come next;
  - then research and specs;
  - then agent;
  - then server and bench;
  - then app-core, then gui, then the `codemason` binary.
- **app-core / gui split** (it replaced `codemason-tui`): clean and one-way. `rg 'egui|eframe|ratatui|crossterm' crates/codemason-app-core/src` finds only one doc comment.
- **TUI leftovers in app-core:**
  - 19 dead `App` fields (AG-04);
  - a write-only `needs_redraw` (AG-05);
  - 196 stale "TUI" mentions (AG-08).
- **Justified edges:**
  - `app-core -> server`: `/webapi` runs the server in-process on the live bus.
  - `app-core -> bench`: `/bench`.
  - `llm -> storage`: one method, `get_provider_auth`. It is the wrong shape (LC-08) but it works.
- **Misplaced modules, not inverted layers.** specs and research use library modules that live in `codemason-tools-extended` (masterfetch, document_extract, archdoc types). This makes specs, a markdown crate, build 16 tree-sitter crates (`cargo tree -p codemason-specs -e normal --prefix none | sort -u | grep -c tree-sitter` = 16). Moving them is effort L and stays out of this train.
- **agent internals:** session, tool, team, agent and task form one cyclic module group joined through `tool::TeamManagerInterface`. That is a real seam (one prod impl plus a test mock). team/ must not become its own crate: it needs `SessionProcessor`, and `scripts/check-team-duplication.sh` pins it in place.
- **Provider layer:** the `Provider`/`LlmClient` traits are real (30 impls). The stream loop, however, is copy-paste across 9 providers (11 `chat()` bodies, 250-398 lines each). Only openai and openrouter use the shared `sse.rs` parser (S-01, LC-05).
- **Tool crates:** four `Tool`/`ToolContext` copies (core, extended, vcs, agent). Each one deliberately narrows what its tools can reach; unifying them is TL-08, Rejected.
- **hyperresearch:** a draft scaffold. No crate depends on it, and 18 of its 43 files are 4-line placeholders. Whether to keep it is an owner decision (O-05).

---

## 2. Defects (report first)

### 2a. Security

| ID | Defect | Where (at `5b9f1b1`) | Verdict |
|----|--------|----------------------|---------|
| LC-03 | An untrusted project `codemason.json` can set `execution_backend` + `backends[]`, and `merge_project` strips neither. The remote backend sends the user's stored provider key (`credentials: ["anthropic"]`) as a bearer token to the attacker's URL. The container backend passes the same secrets as env vars into an image the attacker chooses. | `crates/codemason-config/src/config.rs` `merge_inner` (backends); `crates/codemason-agent/src/backend/remote.rs` (`get_provider_auth`, bearer); `backend/secrets.rs` | R |
| CF-05 | A project config replaces `automation` wholesale. The scheduler auto-starts, and `token_env` reads any env var and posts it as a bearer to the job URL. Already reported in `docs/reports/code-audit.md` and still unfixed. | `config.rs` (automation merge); `crates/codemason-app-core/src/app/startup.rs`, `src/main.rs` (scheduler start); automation `dispatch.rs` | R |
| CF-06 | A project config can set `gitlab.instance_url`. GitLab auth then sends the stored GitLab token to any https host. `vcs` is stripped for exactly this reason; `gitlab` was missed. | `config.rs` (gitlab merge); `crates/codemason-tools-vcs/src/gitlab/auth.rs` | R |
| CF-04 | `save_to_source` writes the *merged* config into the project file. Every toggle that uses it copies global secrets into the repo file; measured: `tavily_api_key` landed in `.codemason/codemason.json`. Affected toggles: `/codeindex`, i18n, `/security`, `/backend`, legacy flags. | `config.rs` `save_to_source` | R |
| SI-04 | A project `"plugins": {}` replaces the section and wipes the global `plugins.permissions`. `build_gate` then returns `None`, which gives the full host API, and budgets can be raised. It needs a plugin the user has already enabled. | `config.rs` (plugins merge); `crates/codemason-plugins/src/lifecycle.rs` `build_gate` | R |
| CF-07 | A project `"execution_backend": "local"` downgrades a user-global docker sandbox to running on the host. | `config.rs` `merge_inner` | R |
| TL-02 | A dangling symlink inside the workspace is treated as a "missing tail", so the containment check passes. `Write`/`Create`/`AppendFile`/`CopyFile` then follow it and create the target outside the root. The doc claim "no component of the missing tail can be a symlink" is false. Realistic trigger: `git_clone` a repo containing a dangling link, then a prompt-injected write. | `crates/codemason-tools-core/src/lib.rs` `resolve_missing_path`; `write.rs`, `create.rs`, `append_file.rs`, `copy_file` | R |
| SI-03 | The unauthenticated `POST /auto/{id}` takes raw `Bytes`. A drive-by web page can send a CORS simple request (`text/plain`, no preflight) and start an agent run with the body as prompt context. Measured: `202`. | `crates/codemason-server/src/routes/automation.rs` | R |
| SI-01 | `/plugins add` calls `read_to_end` on archive entries with no bound and checks the size afterwards. A 306 KB tar.gz holding a 300 MiB entry raised peak RSS by 275 MiB before it was rejected. Connectors fixed the same bug with `.take()` (SEC-070); plugins did not. | `crates/codemason-plugins/src/add.rs` (tar and zip entry reads) | R |
| TL-03 | The GitLab job-log tool reads the whole trace with `resp.text()`, with no byte cap. The GitHub side has a cap (SEC-032), and `GitLabClient::get_text_tail` exists but only autofix calls it. | `crates/codemason-tools-vcs/src/gitlab/gitlab_pipelines.rs` | C |
| SI-05 | `git+` plugin install has no timeout and no size cap, unlike the archive path. An absolute subpath passes `is_safe_operand`; only git cone mode stops it. | `crates/codemason-plugins/src/add.rs` (git path) | C (low) |
| LC-04 | Three parse-failure log sites use `redact_secrets(data)` without the 512 B `redact_frame` cap that SEC-llm-020 requires, so frames up to 1 MiB are logged at warn. | `llm/providers/ollama.rs`, `ollama_cloud.rs`, `openai_responses.rs` | C (low) |

### 2b. Data loss / correctness

| ID | Defect | Where | Verdict |
|----|--------|-------|---------|
| AG-02 | `/codeindex enable\|disable` loads config with `unwrap_or_default()` and then calls `save_to_source()`. A project config that fails to parse makes the default config overwrite the user's *global* file: a 54-byte global holding a secret became a 2,336-byte default. Also, `save_to_source` resolves `.codemason/` against the process cwd, not the project's `cwd_path`. | `crates/codemason-app-core/src/app/slash.rs` (`/codeindex` arms, `rg -n 'save_to_source\(\)' crates/codemason-app-core/src`); `config.rs` `save_to_source` | R |
| TL-01 | `run_git_output` pipes stdout but only polls `try_wait` and never drains the pipe. Once git has written about 64 KiB it blocks and is killed at 60 s. All 18 local git tools go through this function, so `git_diff`/`git_show`/`git_log` fail on any real change. | `crates/codemason-tools-vcs/src/git/mod.rs` `run_git_output` | R (`git log -n 50 -p` errors after 60 s) |
| TL-04 | The bash capture keeps only the first 2 MiB, so the "last 15 KB" tail is not the real tail. For `seq 1 2000000` the last line shown is `31`, it reports 310,102 lines omitted against about 1.99 M, and nothing says output was dropped. Typical loss: the `cargo test` summary. | `crates/codemason-tools-core/src/bash.rs` (capture append, tail render) | R |
| AA-03 | Corrupt-mailbox recovery renames the file after dropping its lock. A concurrent `push` returns `Ok`, but its message lands only in the moved-aside file. The rename result is discarded with `let _ =`. Two recoveries in the same second overwrite each other's aside file. | `crates/codemason-agent/src/team/mailbox.rs` (`read_all`, `peek_unread`, recovery) | R |
| LC-02 | Gemini puts back a malformed line with its own `\n`, so the put-back can never complete it. After 3 retries the loop breaks and drops the complete frames behind it: `garbage\n<good frame>\n` yields `[]` instead of `[TextDelta "hi", Finish]`. | `crates/codemason-llm/src/providers/gemini.rs` (line put-back, leftover handler) | R |
| AA-01 | `sanitize_for_id` keeps non-ASCII alphanumerics and then calls `String::truncate(20)`, a byte cut. A `new_agent` call with an unknown CJK agent name panics. | `crates/codemason-agent/src/task/mod.rs` `sanitize_for_id` | R |
| AA-02 | `extract_explicit_agent_type` / `strip_explicit_agent_type_hint` find the marker in `to_lowercase()` and slice the original text with that offset (the D-07 class). Results are wrong for `ẞ`, and `"İ [agent:日本]"` panics. Reached from LLM swarm output. | `crates/codemason-agent/src/team/classify.rs` | R |
| AG-01 | The GitLab setup dialog inserts typed chars using the cursor as a byte index but moves it by one per char. Typing `é` then `a` panics on the UI thread. Backspace, Right and End have the same confusion. | `crates/codemason-app-core/src/input.rs` (GitLab dialog keys) | R |
| SI-02 | `cdmn serve` with no token configured generates a random token and never shows it, so every protected route returns 401, while the log says "Server auth token configured". `QUICKSTART.md` and `SPEC.md` both describe other behaviour. | `src/main.rs` (Serve); `crates/codemason-server/src/routes/mod.rs` | R |
| RS-02 | Search snippets use match offsets from the lowercased text on the original text, so the snippet misses the match when lowercasing changes byte length: 300×`İ` + `needle` gives offset 601 vs 901. There are three snippet helpers with three window rules. | `crates/codemason-research/src/manager.rs` (`extract_snippet`); `crates/codemason-specs/src/manager.rs` | R |
| RS-04 | Spec frontmatter status parsing uses `content[3..].find("---")`, so `title: a --- b` ends the frontmatter early and the status-mismatch warning disappears. A second check treats any `---` line as frontmatter. | `crates/codemason-specs/src/validate.rs` | R |
| RS-01 | The regex `version \d+\.?d+` needs a literal `d`, so "version 16" never counts as concrete and the user gets an unnecessary clarifying question. | `crates/codemason-research/src/clarify.rs` | R |
| AA-04 | Blueprint seed tasks use snapshot `next_task_id()` + `add_task`: the race D-14 fixed only in `team_task_create`. An `Err` is dropped silently. | `crates/codemason-agent/src/tool/team_create.rs` | P (race) / C (silent Err) |
| AG-09 | `/about` "Built:" prints `chrono::Utc::now()`, not the real build time (`env!("BUILD_TIME")`, which is private in the agent crate). | `slash.rs` (`/about` arm) | C |
| AG-06 | Nothing emits or colours `[red]` markers any more, so `/mcp` failed-server rows lose their glyph while `[green]` and `[yellow]` still show. The pipeline doc and a models.rs comment ("ANSI sentinels") are wrong. | `crates/codemason-app-core/src/md_worker.rs`, `app/models.rs` | C (cosmetic) |
| RS-05 | Two character sets contain `'-', '-'` twice; an ASCII cleanup most likely replaced en/em dashes. An em dash survives title cleaning, and `## Article 3 – Title` does not parse. | `research/src/web_gatherer/title.rs`; `specs/src/constitution.rs` | C (low) |
| RS-06 | hyperresearch `help` lists six subcommands, but the parser accepts only `help`. `core/config.rs` has zero callers and drops lines without `=`. | `crates/codemason-hyperresearch/src/cli/args.rs` | C (low) |

---

## 3. Measured performance waste

| ID | Finding | Evidence | Status |
|----|---------|----------|--------|
| RS-03 (was D-18) | `extract_snippet` lowercases the whole remaining body at every char index, synchronously inside the async research run. | `target/temp/deep-refactor/d18/bench.rs`, keyword at the end of the body: 16 KiB 25 ms -> 0.22 ms; 64 KiB 315 ms -> 0.88 ms; **256 KiB 4.75 s -> 3.7 ms** with a per-char compare | Approved |
| LC-01 | `codemason-types` depends on bundled `rusqlite` only for `CodemasonError::Storage(#[from] rusqlite::Error)`, which is never constructed (`rg -o 'CodemasonError::(\w+)'` finds 9 hits, all `WireLoss`). Everything downstream waits for it. | Cold `RUSTC_WRAPPER= cargo build -p rusqlite` = 12.8 s wall | Approved |

---

## 4. Infra / gates

| ID | Finding | Evidence | Status |
|----|---------|----------|--------|
| I-01 | `cargo clippy --workspace --all-targets` had 12 warnings, down from 143 at the previous audit. All 12 are fixed on `work/2026-11-15`: 3 `redundant_pub_crate` went to `pub(super)` (they collide with CI's `-D unreachable_pub`), and 9 `assert!(x.is_empty())` became `assert_eq!(x, Vec::<T>::new())`. | Clippy now reports 0. The CI dead-code flags pass on app-core. The 87 affected tests pass. | **Done (uncommitted)** |
| SI-06 | B-06 can now be done with no test edit: move tower-http `fs` to server dev-deps, drop the unused workspace `trace` feature, and remove the duplicate dev-dep entries. | `rg 'TraceLayer\|tower_http::trace'` = 0; `cargo tree -e features -i tower-http` | Approved |
| SI-07 | `memchr` in types is unused (machete plus `rg` = 0). These normal deps are used only in tests or benches: bench `rustc-hash`, bench `tempfile`, agent `tokio-stream`, gui `codemason-plugins`, telemetry `serde_json`. | `cargo machete`; per-crate `rg` | Approved |
| SI-10 | No gating CI job compiles `benches/`: check is `--lib` only, clippy has no `--all-targets`, and the bench workflow is manual with `\|\| true`. | `.github/workflows/ci.yml`, `ci_benchmarks.yml` | Approved |
| SI-11 (was I-04) | The `ring` clarify in `deny.toml` is dead: its hash `0xbd0eed23` is not the actual `0x96476bb3`, and ring 0.17.14 declares its own licence. A comment names `webpki-roots`; the lockfile has `webpki-root-certs`. | crc32 of the registry LICENSE; `Cargo.lock` | Approved |
| SI-13 | `check_security_unwraps.py` and `check_poison_locks.py` scan `crates/` only, not the root `src/` (0 sites today, so the gap is latent). | The scripts' own `count_sites` run on `src/` | Approved |
| SI-12 | `AGENTS.md` cites `codemason_tui::app::set_log_root_override`; that function now lives in app-core `helpers.rs`. | `rg` | Owner (user-owned file) |

All 9 repo guards pass, and none is vacuous: the dead-code guard covers 63/63 forms, and every nextest group filter matches a live module.

---

## 5. Safe backlog (behavior-preserving, frozen tests)

| ID | Candidate | Canonical owner after | Pins | Effort |
|----|-----------|-----------------------|------|--------|
| S-01 | Per-provider "stream stalled" block. 11 copies: anthropic, azure_resource, bedrock ×2, copilot, gemini, huggingface, ollama, ollama_cloud, openai_responses, plus `sse.rs`. | `http_client::next_chunk_or_stall(label, ..)` | Stall text asserted by agent `test_session_processor.rs` and telemetry `test_logs_export.rs`; 120 s by `test_m5_6_provider_consistency.rs` | M |
| LC-05 | Ollama local hand-rolls about 150 lines that `sse::parse_openai_sse_stream` already does. | `sse.rs` (plus a flag that suppresses text after tool calls) | `test_ollama_tool_handling.rs` (F6); `"empty/malformed event stream"` in agent `test_agent_memory_deep.rs` | M |
| SI-08 (was S-02) | `EndpointSource` + `tag()` are identical in plugins and connectors. | `codemason-surface`, re-exported from both | `test_stores_report.rs`, app-core `test_plugin_store_stores_report.rs`, connectors `test_connector_store_ops.rs` / `test_connector_store_index.rs` (paths kept by re-export) | S |
| SI-09 | `config_splice` re-implements `guard::write_atomic`, and both `_state.json` ledgers use non-atomic `fs::write`. The connectors `save` doc names the wrong error. | `codemason_types::guard` | `test_surface_config_splice.rs` messages "cannot create" / "cannot replace" | S |
| S-03 rest | Two inline download-cap loops (plugins `add.rs`, connectors `manifest.rs`). | Folds into the SI-01 fix | see SI-01 | S |
| RS-07 / S-04 | Hand-rolled truncation: research `supervisor.rs` (2 copies of `truncate_bytes`), gui `panels/teams.rs` `clip` (= `truncate_chars_no_ellipsis`), codeindex `search.rs` (use `floor_char_boundary`). `cluster.rs` and `document.rs` differ only in their marker text. | `codemason_types::strutil` | `truncate_body_to_bytes` is public (agent `research_adapter.rs`) | S |
| S-05 / RS-08 / AA-07 | Free `#[test]` fns after the inline-test hook: research `cli.rs` (7), specs `plan_parser.rs` (4), app-core `research_progress.rs` (7), agent `research_adapter.rs` (3). | The existing `tests/inline/*` files | `scripts/security-unwrap-baseline.txt` counts drop (shrink-only) | S |
| AG-04 | 19 `App` fields that are only declared and initialised; `code_index_stats_cache` is never read; `tips.rs` is dead. | (delete) | none; `App` is only built in `init.rs` | S |
| AG-07 | gui re-implements `cursor_on_first/last_logical_line`. | app-core `session_ops.rs` | none | S |
| AA-06 | `is_token_overflow_error` / `is_permanent_api_error` are one-line forwards with one caller each; one doc comment stops mid-sentence. | Inline at the call sites | none | S |
| AA-08 | `append_context` duplicated in `conversation_search.rs` and `session_search.rs`. | One helper | none | S (low value) |
| TL-05 | The uncached `check_path_within_any_root` duplicates the cached one (D-01, and now TL-02, have to be fixed in both). It has no production callers, and its re-exports are imported nowhere. | Wrapper over the cached fn with a fresh cache | `test_path_traversal.rs`, `test_lib_registry.rs`, `tools-core/DOCS.md`, agent `benches/hot_paths.rs` | S |
| LC-07 | The `TodoRow` alias has 0 users, and the "`get_todos` is kept" comment is false. | (delete) | none | S |
| LC-11 | The credential-prefix lists in telemetry `sensitive.rs` and `types/sanitize.rs` have drifted. Also treating `redact_secrets(v) != v` as sensitive only widens redaction. | `sanitize` | telemetry sensitive tests and doctest | S |
| Lying docs (S-08 and new) | - agent `bang_command.rs` (chars vs bytes; keep the "characters omitted" wording)<br>- `compaction/runner.rs` `cap_chars`<br>- `take_sse_line` "no allocation" (LC-09)<br>- `cardinality.rs` Clone comment (LC-10)<br>- `types/llm.rs` `stream_timeout_secs` (LC-06)<br>- `research_adapter.rs` `legacy_verifier` "None disables" (AA-09)<br>- `http_request.rs` "reuse the shared client" (TL-06)<br>- `needs_periodic_redraw` cites a missing constant (AG-05 doc part)<br>- `/about` "TUI-based" (AG-08) | the code | Test pins: `test_bang_command.rs`; `tests/inline/test_compaction_runner.rs` + `docs/agentorch.md` (names); `test_research_app_more.rs` (research "not yet implemented in the TUI" string) | S |
| S-09 / AG-03 / RS-09 | Monster functions, decomposed *in place* into phase helpers in the same file:<br>- `execute_slash_command_inner` 8,683 lines (spec arm 1,955; team 1,218; codeindex 840)<br>- `process_user_message` 2,275<br>- `handle_focused_event` 2,261<br>- `async_main` 1,482<br>- `gather_with_observer` 1,407<br>- `handle_provider_setup_key` 1,374<br>- `tool_result_summary` 1,314<br>- `call_llm_step` 845<br>- `build_system_prompt_with_storage_inner` 729<br>- `team_create::execute` 548<br>- `create_builtin_agents` 545 | Phase helpers bound in the same module | **Source-text pins:**<br>- 14 assertions in 7 agent test files read `processor.rs` (incl. `test_parallel_gate.rs` `find()` offset order, `test_tool_input_error.rs` literal `signature: llm_result.reasoning_signature.clone()`)<br>- 4 guard tests read `slash.rs` by path (`test_block_in_place_guard`, `test_codeindex_empty_graph_guard`, `test_codeindex_disabled_guard`, `test_prompt_subagent_render`)<br>- `check_tool_rename.py` path exemptions<br>- one silent-error-baseline entry<br><br>So: no file moves, literal strings stay in their file. | L (one car per big function or arm group) |

Line counts come from a brace-matching scan at `5b9f1b1` (`target/temp/deep-refactor/fnlen.py`, `ag_fnlen.py`). Re-derive with
`python3 target/temp/deep-refactor/fnlen.py <file>` or
`awk '/fn process_user_message/,/^    }$/' crates/codemason-agent/src/session/processor.rs | wc -l`.

---

## 6. Blocked by pins (need a test-edit-sanctioned ticket)

| ID | Candidate | Pin |
|----|-----------|-----|
| AA-05 | 11 PERF-016 `*_blocking` async wrappers have zero production callers. The comment says they keep the executor free, yet the team stores still do synchronous flock and fs I/O from async `Tool::execute` (18 tool files). The fix is to *use* them, not delete them. | `test_team_task_store.rs`, `test_team_store_extra.rs`, `test_team_manager_extra.rs` |
| AG-05 | `needs_redraw` is written 217 times and read 0 times in production; `ScreenMode{Chat}` has one variant. | 214 / 8 / ~23 test refs |
| AA-10 | `chat_request_payload_bytes` is a backward-compatibility alias with no production caller. | `test_session_processor.rs`, `test_history_estimators.rs`, `benches/agent_loop.rs` |
| LC-13 | `obfuscate_key` / `deobfuscate_key` are deprecated forwarders; the doc says v2, the format is v3. | storage `test_provider_auth_store.rs` + doctests |
| TL-07 | `tools-core/xlsx.rs`: only bench uses it, but `rust_xlsxwriter` is built for every tools-core consumer. | `tests/test_xlsx.rs`, `test_file_ops_happy_paths.rs`, `DOCS.md` |
| B-03 | `guard::cap_read` and `http_client::append_stream_chunk_capped` have test-only callers; `check_path_within_root(_cached)` widens into TL-05. | their tests and doctests |
| B-04 | `research/source_registry.rs`: a one-impl trait with no production caller. | `tests/inline/source_registry_tests.rs`, `lib.rs`, `DOCS.md` |
| B-05 | Exact-status-string assertions in the UI tests: 896 (875 app-core, 21 gui), up from 709. | the tests themselves |
| LC-12 | `http_client::MAX_RETRY_AFTER` has 0 callers; D-09 kept the name on purpose. | none, low value |
| LC-08 | Replace `Arc<Storage>` in llm with `Arc<dyn ProviderAuthSource>` (trait in types) to remove the `llm -> storage` edge. tools-vcs already defines the same trait. | `test_openrouter_provider.rs`, `test_router_provider.rs` (should coerce; verify) |

---

## 7. Owner decisions (not ticketed until decided)

| ID | Question |
|----|----------|
| O-01 | **Project-config policy.** Strip `backends`/`execution_backend`, `automation`, `gitlab`, and `plugins.permissions` from project configs (sections 2a), or keep them but require an explicit per-project trust prompt? Either way `docs/howtos/sandbox-backends.md` and `docs/howtos/automation.md` / `QUICKSTART.md` (which tell users to put these keys in the project file) change. |
| O-02 | CF-08 / CF-09 / CF-10 / CF-11 / CF-12: may a project override search API keys, raise budgets (`loop.max_steps`, `cost_limit`, background-agent limits, `prices`), disable checkpoints or `activity_log`, or replace `connectors`? |
| O-03 | SI-03 webhook auth: a per-automation HMAC/bearer secret (recommended), or only reject cross-origin `text/plain`? |
| O-04 | SI-02: print the generated `serve` token to stderr, write it to a 0600 file, or refuse to start without one? |
| O-05 | hyperresearch: build it on top of research, or remove the scaffold? |
| O-06 | D-11 (telemetry over-redacts `model:tag`) and D-15 (router doc claims FR-020 stripping that does not happen): still blocked, unchanged at `5b9f1b1`. |
| O-07 | TL-10: `git_clone` accepts a bare local path and `file://`, so it can copy a repo from outside the workspace into it. The code says this is intended; confirm. |
| O-08 | SI-12: update the stale `codemason_tui` path in `AGENTS.md`. |

---

## 8. Deliberate / Rejected (do not relitigate)

| Item | Why |
|------|-----|
| TL-08: unify the four `Tool`/`ToolContext` copies | Each context deliberately narrows what its tools can do; unifying them touches 55 test files that construct `ToolContext`. |
| TL-09: SSRF check skips DNS resolution | Documented choice; redirects are re-checked on every hop. |
| TL-11: `open` with `file://` skips containment | Opens in a GUI viewer only, nothing reaches the model, and the tool is permission-gated. |
| RS-10: one-impl traits (SourceSummarizer, Verifier, ResearcherNode, Critic) | Public, listed in DOCS.md, imported by tests; changing them costs more than it saves. |
| Splitting `Storage::migrate` / `Config::merge_inner` | Source-scanned by `test_warm_index_scan`; no defect measured. |
| Trimming the plugins/connectors re-export surface (~211 / ~206 names) | Public-API churn with no gain. |
| S-07 `HumanFallback` | Rejected at the previous audit (documented extension point). |
| `*_for_test` seams (17 in agent, 35 `#[doc(hidden)]` in app-core) | Follow from the tests-in-`tests/` rule in AGENTS.md. |
| `app-core -> bench`, `app-core -> server` edges | `/bench` and the in-process `/webapi`. |
| `compaction/serializer.rs` `truncate`, `research/comparison.rs` `truncate_cell`, `#[allow(dead_code)]` in `#[path]`-included research files | Unchanged from the previous audit; different semantics or test inclusion. |
| CF-14: project keys with no production reader (`command`, `instructions`, provider `options.auto_start`, `piegap.*`) | Inert; noted only. |

---

## 9. Previous ledger reconciled (audit at `3de6683`)

| Old ID | Now |
|--------|-----|
| D-01, D-12 | Done; no regression. TL-02 is a *different* hole in the same function. |
| D-06, D-10, D-16, D-17 | Done; still hold. |
| D-14 | Done in `team_task_create`; the same pattern survives in `team_create` seeding -> AA-04 (possible regression). |
| D-18 | Still open -> RS-03 (now measured). |
| D-11, D-15 | Still blocked -> O-06. |
| I-01 | Done at `work/2026-11-15` (see section 4). |
| I-04 | Still open -> SI-11. |
| S-01 | Still open (11 copies). |
| S-02 | Still open -> SI-08. |
| S-03 | Partly done (the fn copies delegate to `codemason_types::read_capped`); the inline loops fold into SI-01. |
| S-04 | Hook-warning site Done. research and codeindex sites still open -> RS-07. The `tui/layout_teams` `clip` moved to `gui/panels/teams.rs`, still open. |
| S-05 | Still open -> S-05 / RS-08 (tui file moved to app-core `research_progress.rs`). |
| S-06 | Agent items Done. Still open: specs `create_spec`, `search_specs_with_archived`, `resolve_requirements`, `task_id_at`, and codeindex `hash_file` (each named only in its crate `DOCS.md`; a doc edit, not a test edit) -> Approved. |
| S-08 | Widget rows Superseded (files gone; leftover is AG-06). Other rows still open (section 5). |
| S-09 | Still open; `build_system_prompt_with_storage_inner` grew to 729 lines. **Correction:** the old "no source-text pins" was wrong (see section 5). |
| B-01 | Superseded: `MessageWidget` is gone. |
| B-03, B-04 | Still open. |
| B-05 | Still open and grown (896). |
| B-06 | Unblocked -> SI-06. |
| Server `test_state` copy | Still open: `test_openai_surface.rs` `state_with` -> add `support::test_state_with(registry, cap)` (test-only, S). |

---

## 10. Cleanup train ledger

Statuses: `Proposed`, `Approved`, `In progress`, `Blocked`, `Superseded`, `Done`, `Rejected`. One car = one PR, in order.

| Car | IDs | Preserved behavior | Validation | Status |
|-----|-----|--------------------|-----------|--------|
| 1. Config trust boundary | LC-03, CF-04..07, SI-04, CF-05, CF-06, AG-02 | Global config is the only source for privileged keys. Project configs keep every non-privileged key. A toggle never writes merged secrets or a default over an unreadable file. | `cargo nextest run -p codemason-config -E 'test(test_merge_project_privileged::)'` plus new locking tests per key; `-p codemason-app-core -E 'test(codeindex)'`. **Sanctioned test edits:** `test_backend_command.rs` (project-file backend case), `test_plugins_command.rs` (project-file plugin cases). | Blocked: O-01 |
| 2. Tool/IO security | TL-02, SI-01 (+S-03 rest), TL-03, SI-05, LC-04 | In-root paths and real files still accepted. Archives and traces still tailed and capped with the same messages ("fetching job trace", "showing last N of M lines"). | `-p codemason-tools-core -E 'test(test_path_traversal::) + test(test_lib_registry::)'`; `-p codemason-plugins -E 'test(test_add::)'`; `-p codemason-tools-vcs -E 'test(test_gitlab_pipeline_tools::)'` | Approved |
| 3. Server surface | SI-03, SI-02 | Authenticated routes unchanged; `/health` public. | `-p codemason-server -E 'test(test_automation_surface::)'` (sanctioned edits if O-03 changes the public contract) | Blocked: O-03, O-04 |
| 4. Bug batch | TL-01, TL-04, AA-03, LC-02, AA-01, AA-02, AG-01, RS-02, RS-04, RS-01, AA-04, AG-09, AG-06, RS-05, RS-06 | Each keeps current behaviour on ASCII / small inputs; each gets a locking test for its failing path. | per-crate filters (`test_m6_resilience::`, `test_gemini_*`, `tests/inline/task_mod_tests`, `test_team_classify::`, `test_input_deep*`, `tests/inline/manager_tests`, `tests/inline/validate`) | Approved |
| 5. Perf | RS-03, LC-01 | Same snippet positions; same error enum minus a never-built variant. | `-p codemason-research -E 'test(locus)'`; `cargo check --workspace --all-targets` | Approved |
| 6. Infra diet | SI-06, SI-07, SI-10, SI-11, SI-13 | Same build; images and features unchanged. | `cargo machete`; `cargo check --workspace --all-targets`; CI | Approved |
| 7..n. Behavior-preserving refactors | Section 5, one car per row group; S-09/AG-03 one car per function or arm group | Suite passes unchanged; no test edits. | `cargo nextest run -p <crate>` | Approved |
| Last. Structural | Section 6 rows | Each sanctioned test edit enumerated in its ticket; every rewritten guard gets a non-vacuity proof. | per row | Blocked: needs test-edit sanction |
| - | I-01 | - | clippy 0 warnings | Done (uncommitted) |
| - | Section 7 | - | - | Blocked: owner decision |
| - | Section 8 | - | - | Rejected / Deliberate |

### Prevention decisions

- **Car 1:** Guard added. Extend `test_merge_project_privileged.rs` with a table-driven case per privileged key, so a new privileged section fails until it is classified.
- **TL-01, TL-04, SI-01, TL-03:** Guard added (a locking test with output or entries above the cap).
- **AA-01, AA-02, AG-01, RS-02:** Guard added (non-ASCII input tests). `scripts/check_shared_guards.py` cannot see "offset from lowercased text applied to the original"; No guard justified beyond the tests (a source grep would be brittle).
- **Lying docs:** No guard justified.

---

## 11. Lessons for the next audit

- **Config trust is the highest-yield area.** `merge_project` uses a strip-list, so every new privileged section is unsafe by default. Three independent auditors hit it from three directions. Audit config merges by enumerating keys, never by reading the strip list.
- **First-pass claims held up.** The adversarial pass refuted 0 of 10. But it corrected details: LC-02 does not spin "forever", and SI-03 is not stopped by a JSON extractor because the handler takes raw `Bytes`. Verify the mechanism, not just the verdict.
- **Byte/char confusion has new homes.** The pattern "find an offset in `to_lowercase()`, slice the original" (D-07, AA-02, RS-02) and "cursor moves by chars, indexes by bytes" (AG-01) slip past every guard script.
- **`#[path]` tests can re-implement production code.** `test_gemini_stream_spin.rs` copies the loop inline, so it asserts nothing about production and encodes a false premise.
- **Free `#[test]` fns after the inline hook** are still not caught by `check_inline_tests.py`; 21 exist across 4 crates.
- **Source-text pins are easy to miss.** The previous audit recorded "none" for S-09, yet 14 assertions read `processor.rs`. Always run `rg 'include_str!|read_to_string\(.*src/'` over `tests/`.
