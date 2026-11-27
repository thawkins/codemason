# Deep-Refactor Audit, 2026-11-27

**Audited at:** `32f2b4d` (Version 1.0.186), branch `claude/deep-refactor-8aed50`. Car 0 landed on top, as `8e049db`, `77ae198` and `4347b45`.
**Independence:** this audit was carried out fresh. It does not reconcile or rely on `docs/reports/deep-refactor-audit.md`.
**Scope:** all 20 crates under `crates/` plus the root `codemason` binary.

**Method:**
- Five read-only passes over separate areas:
  - agent;
  - app-core, gui and `src/`;
  - llm, config, storage, telemetry and types;
  - tools and codeindex;
  - research, specs, plugins, connectors, server, surface, bench, plus build and CI hygiene.
- Two adversarial verifiers then tried to refute 60 defect claims. I re-checked 8 more by hand.
- Scanners: `cargo clippy --workspace --all-targets`, every `scripts/check-*.sh`, and test runs of each crate touched.
- Per-area detail, with full quotes and pin inventories, is in the git-ignored `target/temp/deep-refactor2/` tree:
  - `area-*.md`;
  - `verify-a.md`, `verify-b.md`.

Every count gives the command that produced it and the commit it was taken at. Re-derive a count on the branch point of the car that uses it.

**Verdicts:**
- **R**: reproduced (a test or run shows it).
- **C**: confirmed by an adversarial trace of the code path.
- **P**: partly confirmed (the verifier narrowed the claim; the narrowed form is what is stated).

---

## 1. Architecture verdict

**The crate layering is sound: no cycles, and dependencies point downward. The debt is in three places:**
- **Platform assumptions.** The code is Unix-first, and on Windows the tests leaked into the real user profile.
- **I/O boundaries.** Child processes, hooks, archives and stream parsers.
- **God files and copy-paste.** The provider stream parsers, and the archive staging shared by plugins and connectors.

Evidence:
- **Graph** (`grep -oE '^codemason-[a-z-]+' crates/*/Cargo.toml` at `32f2b4d`):
  - Bottom to top: types, then config, then storage / surface / codeindex / llm / tools / plugins / connectors, then research / specs, then agent, then server / bench, then app-core, then gui, then the binary.
  - No leaf crate points back up.
- **app-core / gui split:** clean and one-way. app-core imports no egui, eframe or winit.
  - `app-core -> server`: one listen address and the `/webapi` start.
  - `app-core -> bench`: `/bench`.
  - These are coupling, not defects.
- **One misplaced edge:** specs depends on tools-extended only for the `project_scaffold` flag types and `archdoc::ArchitectureStructure`. That edge pulls 10 tree-sitter grammars, lingua and the PDF stack into every specs build (RS-21, effort L, not in this train).
- **Separate `ToolContext` types are deliberate:** each leaf crate cannot depend on agent, so each context carries only what its tools need. The rest is drift:
  - `Tool` and `ToolOutput` are copied byte-for-byte into the core, extended and vcs crates.
  - All four `Event` types are the same `codemason_types::event::Event`, yet the agent's adapters still copy events through private buses (TL-06).
- **Providers:** the transport helpers in `http_client` are shared, but the frame parsers are copy-pasted. `sse.rs` serves only OpenAI and OpenRouter. Anthropic and Azure-Anthropic are verbatim copies, Bedrock is a third variant, and fixes have drifted between them (CO-05/06/13/15/16).
- **Config trust boundary holds:** no project-level key can redirect credentials, choose a backend, start schedulers, change plugin permissions or downgrade a sandbox.
- **God files** (`wc -l` at `32f2b4d`):
  - `app-core/src/app/slash.rs`: 16,096 lines (`App` has 270 fields).
  - `agent/src/session/processor.rs`: 5,327 lines; `process_user_message` alone is 1,830 lines.
- **Dead TUI layer in app-core:**
  - `App::handle_mouse` (514 lines) has no production caller.
  - 27 cell-coordinate `Area` fields are never set by the GUI.
  - 5 copy buffers are never filled.
  - All of it is pinned by about 24 test files (UI-08..11).

---

## 2. Defects

### 2a. Fixed in this session (Cars 0, 0b, 1)

| ID | Defect | Fix | Commit |
|----|--------|-----|--------|
| W-01 | **Tests wrote into the real user config on Windows.** `Config::global_config_dir()` used `dirs::config_dir()`, which honours `XDG_CONFIG_HOME` only on Linux. 50 test files isolate the config through that variable (`grep -rlE 'set_var\("XDG_CONFIG_HOME"' crates/*/tests tests \| wc -l` at `77ae198`). A connectors run overwrote `%APPDATA%\codemason\codemason.json`. | An absolute `XDG_CONFIG_HOME` is honoured on every platform. `--dry-run`, `/init config` and `/config reload` now resolve through the one function. | `4347b45` |
| W-02 | **Windows path escape:** `validate_relative_component("/etc/passwd")` was accepted, because `is_absolute()` needs a drive prefix on Windows. A connectors archive entry `/etc/passwd` was then joined to `C:\etc\passwd`. | Uses `has_root()`; the comment that said otherwise is corrected. | `77ae198` |
| W-03 | The `codemason-types` test binary did not compile on Windows (`std::os::fd` behind a runtime `cfg!`). The structure guard compared `\` paths against `/` paths, so it failed on Windows. | `#[cfg(unix)]` gates; separator normalisation. Non-vacuity proof done. | `8e049db` |
| CO-01 | Six config sections (`stream`, `memory`, `storage`, `agent_perf`, `trigger`, `security_analyzer`) were never merged from the global or project file. | `merge_inner` merges them; the project guard strips the privileged ones. | `3984496` |
| CO-02 | The credential key was written in place; two first-run processes, or a crash mid-write, minted a second key and orphaned every stored credential. | Temp file + `hard_link` publish (create-if-absent); a damaged key file is left in place. | `86dfcb2` |
| CO-10 | `restore_snapshot` truncated user files in place; a crash mid-rollback left them empty. | `guard::write_atomic`, keeping the file's permissions and writing through a symlink. | `3fba902` |
| W-07 | `process_alive` reported a live process as dead on Windows, so a running session was flagged as a crash. | `GetExitCodeProcess == STILL_ACTIVE`. | `5ff13fb` |
| W-08 | **Agent teams could not be created or updated on Windows**: `sync_all` on a read-only temp handle ("Access is denied"). About 290 agent test failures. | Team store, mailbox and task store open the temp file for write. | `68a3e70` |
| W-09 | **Tests wrote into the real user profile** (global `codemason.json`, plugin ledger, teams, catalogues) on every platform. | A `ctor` in `tests/common/profile_isolation.rs`, included by every test binary, redirects `XDG_CONFIG_HOME`, `HOME`, `XDG_DATA_HOME` to `target/temp/test-profile/`. | `700196c` |
| W-10 | **Security (SEC-079 on Windows):** an image placeholder `/etc/passwd` passed the containment check and read `C:\etc\passwd`. | Only plain and `.` components are contained; an unsafe source is never joined onto the base. | `c71ca52` |
| W-11 | Hooks, verification scripts and `run "!<cmd>"` spawned a bare `sh`/`bash`, so no hook ran on Windows without `sh` on PATH (the gate fell through to the normal permission prompt). | `bash::posix_shell()` / `bash_program()` reuse the Bash tool's Git Bash discovery. | `86ed747` |
| W-12 | **Data loss on Windows:** worktree prune matched subdirectory sessions with `/` only, so it deleted a checkout a live session in `worktree\src` was using. Also: `\?\` paths handed to git; npm discovery never found `npm.cmd`; `/bench` manifests failed their own path guard; container, `@include`, memory, file-menu and clipboard paths assumed `/`. | Either separator in the in-use SQL; `dunce`; `which`; `/`-joined manifest paths; root/prefix checks. | `dd71af1` |
| W-13 | The project-teams walk took `~/.codemason` (the legacy global dir) as a project, so projects under home put their teams in the home folder. | The walk stops at the home folder, compared canonically (8.3 short names). | `cf638a8` |
| W-14 | Embedded skill data was checked out CRLF under `core.autocrlf`, so Windows builds shipped CRLF skill files. | `.gitattributes` pins that directory to LF (narrow form of O-07). | `65a4c2e` |
| AG-01..04, 06, 07 | Hook and child-process limits: pre-tool hooks ignored `timeout_secs` and blocked a runtime worker; timed-out hooks leaked the process; a >128 KiB (32 K chars on Windows) tool input made the spawn fail so guard hooks were skipped; stdin-vs-stdout deadlock; verification killed bash only and lost output on one non-UTF-8 byte; a verbose probe was killed as hung. Found on the way: Git for Windows' `binbash.exe` is a launcher, so killing it left scripts running. | One `child_process::run_with_deadline` (AG-18): concurrent stdin, drained pipes, process-tree kill (process group / `taskkill /T`); pre-hooks on the blocking pool; env values over 64 KiB (16 KiB Windows) become empty + `_TRUNCATED=1`. | batch 2 |
| AG-05, AG-08, AG-13, CO-14 | A trigger with a length-changing lowercase char (U+212A) panicked; `timeout_secs = u64::MAX` overflowed the wait deadline and panicked; a typo'd teammate `memory` scope silently meant none; Bedrock `region`/`profile`/`endpoint_url` were never forwarded and Bedrock compaction ran on the default model. | ASCII case folding; `tool::deadline_after` saturates; unknown scope is an error; `Config::provider_client_options` forwards every provider option plus the model at all three client sites. | batch 3 |
| CO-05, CO-06, CO-12 | An Anthropic `event: error` frame (`overloaded_error`) was swallowed, so the turn looked truncated and the provider message was lost (anthropic and azure_resource copies); copilot and huggingface merged index-less parallel tool calls onto stream 0; the Ollama Cloud done frame (final text, `context` token ids) was logged at INFO. | `"error"` arm yields `StreamEvent::Error`; `StreamKey::from_frame` keys by id when `index` is absent; the done frame logs `done_reason` and eval counts only. | batch 4 |
| TL-01, 02, 03, 04, 09, 12, 14, 25 | Background output stopped at the first non-UTF-8 byte; the git clone directory guard missed Windows rooted/prefixed paths; glob reset its root per level so `src/**/*.rs` missed nested files; git_status mis-parsed porcelain v2 and never reported conflicts; README fetch always failed on github.com (cross-origin download_url); multi_edit ("atomic") left earlier files edited after a failure; rm followed symlinks; fetch truncation kept one char too few. Also found: apply_patch's header check was Windows-blind (later check still refused it). | Lossy capped line reader; root/prefix rejection; glob keeps the root; v2/short parsers by field count; raw-media README on the API origin; staged temp-and-rename writes; `symlink_metadata`; `nth(max_chars)`. | batch 5, `c0f6004` |
| TL-07, TL-08, TL-11, AG-11, RS-04, RS-05, RS-07 | SSRF: a trailing dot bypassed the metadata/rebinding name checks, 100.64/10 and NAT64 were open, and resolved addresses were never checked; model-chosen webfetch size/timeout had no ceiling; GitHub/GitLab and remote-backend bodies were read unbounded; plugin clones were size-checked only after finishing; archives had no entry limit; webhook ingress shared one rate-limit key and leaked which automation ids exist. Residual: a known automation with no secret configured still answers 403 (an existing test asserts it), and the per-id limiter map grows with attacker-chosen ids until its 10k/120 s eviction. | Normalised names, both ranges, `SsrfSafeResolver` pinning; ceilings; 4 KiB/64 KiB capped readers; in-flight clone polling; `MAX_ARCHIVE_ENTRIES`; per-id key, credential check first, uniform 401. | `8589d4a`, git quoting `5bdcd74` |
| RS-01, 02, 03, 06, 08, 09, 14, UI-02 (part), UI-04, UI-06 | CRLF plugin commands and research frontmatter mis-parsed (one corrupted RESEARCH.md); comparison quoted the wrong sentence after length-changing case folds; the rate window confused count with seconds; automations ran in the launch dir, not the project; a zero default timeout; a year regex; history writes could truncate; one provider cleared another's spinner; `/init config` could overwrite. | Byte-accurate offsets, CRLF-aware fences, offset mapping, `RATE_LIMIT_WINDOW`, `research_root` parent, hand-written Default, regex fix, temp+rename, provider-matched clear, `create_new`. | batch 7 |

### 2b. Open: high

| ID | Defect | Where | V |
|----|--------|-------|---|

### 2c. Open: medium

| ID | Defect | Where | V |
|----|--------|-------|---|
| TL-06 | The tool-adapter event forwarders subscribe after the tool starts and are aborted on return, so events are lost. The out-forwarder has no session filter. | `agent/src/tool/mod.rs` adapters | C |
| UI-01 | Ribbon, menu and chip slash commands bypass `is_input_blocked()` and run mid-turn; typed and palette commands queue. A naive fix would also queue `/quit`. | gui `shell.rs` vs app-core `palette.rs` | P |

### 2d. Open: low

| ID | Defect (narrowed by the verifier where noted) |
|----|------|
| W-15 | Six app-core tests fail only in a single-process `cargo test` run (cwd / config races: `test_multisession_new_session_in_a_permitted_path`, `test_project_command_add_registers_permitted_directories_only`, `test_directory_menu_has_back_to_fuzzy_entry`, `test_file_menu_ctrl_backslash_toggles_hidden_filter`, `test_slash_system_sets_prompt`, `test_slash_unknown_command_shows_error`). Each passes alone; every cwd-changing helper holds `global_env_lock`, so the writer is not yet found. Nextest (one process per test) cannot hit it. Same class in agent: `test_loop_telemetry` reads a process-global counter that loop tests in other files also write (its file-local lock cannot exclude them); one failure poisons the lock and cascades. |
| W-16 | `image_load::file_uri` on a *relative* path yields `file://shots/x.png`, which `uri_path` and the loader read as the UNC host `\shots\x.png` on Windows (a network read). Production callers checked pass absolute paths; no guard enforces it. |
| W-17 | 28 agent tests and `scripts/check-test-env-isolation.sh` need `python3`; a Windows box without Python cannot run them, and the guard script cannot run at all. **Resolved 2026-11-28:** Python 3.12 is installed; a `python3.exe` copy beside `python.exe` makes the `python3` name resolve. All 72 Python-dependent agent tests pass, and running them exposed two guard defects, fixed in `bc646fc` and `2f259f1` |
| AG-10 | `TaskStore` async fns take a blocking flock and do `std::fs` I/O on the runtime. |
| CO-07 | The redaction prefix lists in types `sanitize.rs` and telemetry `sensitive.rs` have drifted. |
| CO-09 | `Config::load` fails outright in a read-only cwd with no global config. Creating the default file is deliberate. |
| RS-04 / RS-05 | `git+` install size is checked only after the clone. zip/tar extraction has no entry-count cap (plugins and connectors). |
| TL-05, TL-10, TL-11, TL-13, TL-16 | Unbounded reads that cost memory only. Every tool result is cut to 12k chars before the model (`agent/src/session/history.rs` `MAX_TOOL_RESULT_CHARS_FOR_LLM`), so "floods the model" is refuted. |
| TL-15 | A poisoned containment-cache lock fails open (practically unreachable). |
| UI-02 | (Open part) Two windows still overwrite each other's input history: a merge needs the process to know which entries it added since loading. The crash-truncation part is fixed (batch 7). |
| UI-03 | `/websearch` results go through one shared slot, so the result is lost or lands in the wrong session. **Blocked by pins:** `test_housekeeping.rs` and `test_session_events_deep.rs` write the slot as `Option<String>` and assert the status text; the fix (a queue of origin-tagged results) belongs in the structural batch. |

**Refuted:**
- AG-09: the racy `pending_manager` branch in `team_create.rs` cannot run, because nothing emits that state. It becomes a dead-code row instead.
- AG-15: session ids are ASCII UUIDs.
- AG-12 headline: the dry-run and real `--model` paths agree; the duplication is real.

---

## 3. Measured performance waste

| ID | Finding | Evidence |
|----|---------|----------|
| RS-16 | connectors `Cursor::new(bytes.to_vec())` copies an archive of up to 50 MiB a second time. | Read of `manifest.rs`; the copy is unconditional |
| RS-17 | plugins writes the archive to disk and deletes it unread (up to 50 MiB of I/O per install). | `add.rs`: the temp file is never opened for read |
| RS-18 | research builds a new `reqwest::Client` (pool and TLS) on every open-access call. | `open_access.rs` |
| TL-16 | codeindex `full_reindex` reads every changed file twice. | `lib.rs` (2 reads per file) |

No timing benchmark was run; each saving is quoted as unknown until measured.

---

## 4. Infra / gates

| ID | Finding |
|----|---------|
| I-01 | `cargo-nextest` is not installed on this Windows box, though AGENTS.md mandates it. Runs used the documented `cargo test -p <crate> --test all` form. |
| I-02 | `cargo fmt` with `newline_style = "Unix"` against a `core.autocrlf=true` checkout makes `git status` list every `.rs` file as modified, though the content is unchanged. Commit by explicit path. A `.gitattributes` with `*.rs text eol=lf` would remove this (owner decision O-07). |
| RS-22 | CI clippy runs without `--all-targets`, so tests and benches are never linted. Benches compile only on macOS. The bench workflow is manual, uses `@stable` and ends in `\|\| true`. |
| RS-23 | The shared-guard and redaction suites run three times in CI. |
| RS-24 | Guard gaps: `check_test_env_isolation.py` skips root `tests/` (4 files call `set_current_dir`), `check_shared_guards.py` skips root `src/`, and `check-vcs-duplication.sh` checks only plugins `add.rs`. |
| RS-25 | Dev-deps that duplicate normal deps; literal versions where `workspace = true` belongs. No unused normal deps (each was grepped). |
| W-05 | No CI job runs the suite on Windows; that is how W-01..W-04 survived. |

The `scripts/check-*.sh` guards all ran with no failure text at `32f2b4d`.

---

## 5. Safe backlog (behavior-preserving, frozen tests)

| ID | Candidate | Canonical owner after | Pins | Effort |
|----|-----------|-----------------------|------|--------|
| CO-24 | Destructure the overlay in `merge_inner` without `..`, so a forgotten field is a compile error. This is the guard for CO-01. | `config.rs` | ~10 merge tests (behavioural) | M |
| CO-13/15/16 | Anthropic and Azure parsers become one; the 4 first-byte timeout blocks become one helper; 3 OpenAI-chat loops move onto `sse.rs`. | `llm/providers/{anthropic_sse,sse}.rs`, `http_client` | Messages "initial response timed out", "ended without producing any events", "empty/malformed event stream" stay byte-identical | M each |
| RS-10/11 | Archive staging and the `git+` parser are copied between plugins and connectors (only plugins rejects symlinks). | `codemason-surface` | `check-vcs-duplication.sh` (new home must not contain `value.starts_with('-')`); test_add, test_connector_install/manifest | M |
| TL-17 | The uncached containment check duplicates the cached one and has no production caller. | Wrapper over the cached fn | test_path_traversal (21 calls), test_lib_registry | S |
| TL-19 | `pdf_common::resolve_path` is a byte-identical copy of `path_util::resolve_path`. | tools-core `path_util` | `check-vcs-duplication.sh` placement | S |
| AG-16/17/18 | Hook env built 4 times; 5 char-boundary helpers; 4 child-process timeout and kill policies. One bounded-run helper also closes AG-02/04/06/07. | `agent` process helper; `codemason_types::strutil` | 5 test files call the sync hook API (keep its signature) | M |
| AG-12 | 6 `ModelRef` parsers with 3 rules. | One parser in agent | 2 pub fns | S |
| CO-07 | One credential-prefix list. | `codemason_types::sanitize` | 14 tests; `check_shared_guards.py` | M |
| CO-17 | `wire/trace.rs` private `write_atomic` (umask perms) duplicates `guard::write_atomic` (0600). | `guard` | test_wire_trace | S |
| UI-13/14 | Two identical small-button helpers and a hand-built badge. Needs a small, non-focusable `material::button` variant first. | `codemason_gui::material` | material tests | S |
| RS-19 | `parse_spec_frontmatter_research`: zero callers, wrong doc. | (delete) | none | S |
| Lying comments | CO-19 doc, CO-20 (3), AG-26 (`#[must_use]` swallowed into the doc), RS-27 (2), RS-28 (`/webapi` token "masked on every render path"; it is not), UI-18 (TUI references), the `config.rs` XDG cache comment | the code | No test greps these texts (re-check per edit) | S |
| Monster fns | `merge_inner` 379; `storage::migrate` 686; `process_user_message` 1,830; `dispatch_key` 1,147; `handle_key` 1,021; `message_text` 1,105 / 673; `slash_config` 460, `slash_codeindex` 427; research `session.rs` ~958 lines. Decompose in place, one car per function. | Phase helpers in the same file | **Source-text pins:** `storage/tests/test_warm_index_scan.rs:39` (anchor text in `storage.rs`); `app-core/tests/test_codeindex_empty_graph_guard.rs:23` (`"godnodes" =>` in `slash.rs`); `agent/tests/test_tool_input_error.rs:369` (a literal line in `processor.rs`); `check_tool_rename.py` lists `slash.rs`. Literal strings stay in their file. | L |

Re-derive function lengths by brace counting from the `fn` line, for example `awk '/fn merge_inner/,/^    }$/' crates/codemason-config/src/config.rs | wc -l`.

---

## 6. Blocked by pins (needs a test-edit-sanctioned ticket)

| ID | Candidate | Pin |
|----|-----------|-----|
| UI-08/09 | Delete the TUI mouse layer: `handle_mouse` chain and 27 `Area` fields. | 24 test files call it; 101 use `tests/term_events` |
| UI-10/11 | 5 never-filled copy buffers; write-only fields (`selected_agent_index`, `skill_dirs_cache`, ...). | `test_input_deep3.rs:1132`; 7 files read `selected_agent_index` |
| CO-08 | `Config::save` / `persist_to_source` (zero callers) and `save_to_source` (writes merged secrets into the project file). | `test_codeindex_toggle_persistence.rs`, `test_config_write_target.rs`, `test_tools_config_load.rs` |
| AG-09 | The dead `pending_manager` branch in `team_create.rs`. | `test_team_store_extra.rs:133` |
| AG-21 | The `next_task_id` snapshot API, which only tests call. | `test_team_manager_runtime.rs:461` |
| TL-21 | Three `ToolRegistry` types used only as a list. | test_lib_registry, test_lib_surface, test_vcs_registry_coverage, doctest |
| CO-23 | Split `storage::migrate`. | `test_warm_index_scan.rs:39` source anchor |

---

## 7. Owner decisions (not ticketed until decided)

| ID | Question |
|----|----------|
| O-01 | **Decided: honour them** (W-04, `96c08e2`). |
| O-02 | AG-14: should a teammate keep its custom agent's pinned model, as `task/mod.rs` does, or take the lead's model? |
| O-03 | CO-19: enforce `agent.<name>.permission`, or document it as a prompt hint only? |
| O-04 | UI-01: which slash commands must run immediately mid-turn (`/quit`, `/cancel`, ...)? |
| O-05 | CO-03 / CO-04: run Bedrock and Gemini against live endpoints. The parsers may not match the real wire formats (binary eventstream; multi-line JSON without `alt=sse`). |
| O-06 | AG-20: keep the orchestrator modules (621 lines, no production caller) or remove them? RS-21: extract specs' two tools-extended modules into a small crate? |
| O-07 | Add `.gitattributes` (`* text=auto eol=lf`) and a Windows CI job (I-02, W-05)? |

---

## 8. Deliberate: do not fix

| Item | Why |
|------|-----|
| Separate `ToolContext` per tool crate | Leaf crates cannot depend on agent; each context narrows what its tools reach. |
| AG-23 thin re-export modules | 342 external uses; churn for no gain. |
| CO-26 project `instructions` / `skill_dirs` / `agent.prompt` | Prompt influence from repo content is by design. |
| TL-22 test-only pub fns (codeindex store, edit_log clears) | Pinned by their own tests; they follow from the tests-in-`tests/` rule. |
| RS-26 `event_to_parts` (~882 lines) | A flat exhaustive match; splitting it adds indirection without clarity. |
| UI-05 starter button replaces the draft | Asserted by `test_messages.rs:309`. |
| `app-core -> server`, `app-core -> bench` | `/webapi` and `/bench`. |

---

## 9. Cleanup train ledger

Statuses: `Proposed`, `Approved`, `In progress`, `Blocked`, `Superseded`, `Done`, `Rejected`. One car = one PR, in order.

| Car | IDs | Preserved behavior | Validation | Status |
|-----|-----|--------------------|-----------|--------|
| 0. Windows isolation | W-01, W-02, W-03 | Linux paths unchanged; Windows tests stop touching the profile | config 460/460, connectors 377/377, types 438/438, app-core init/reload 20/20; the real config's mtime is unchanged | **Done** (`8e049db`, `77ae198`, `4347b45`) |
| 0b. Windows portability | W-04, W-07..W-14 | Unix behaviour unchanged; no assertion weakened (test edits enumerated in `feebc7a`, `9f8446d`) | Windows: agent 401 to 29 failed (28 need python3), app-core 82 to 6 (single-process races, invisible under nextest), gui 2 to 0, storage 427/427; real profile untouched across full runs | **Done** (`5ff13fb`..`cf638a8`) |
| 1. Config + storage correctness | CO-01 (+CO-24 guard), CO-02, CO-10 | Existing config files load the same; each of the six sections now merges | New `test_merge_inner_sections` (one case per section); `-p codemason-config`; `-p codemason-storage` | **Done** (`3984496`, `86dfcb2`, `3fba902`) |
| 2. Hooks + child processes | AG-01, AG-02, AG-03, AG-04, AG-06, AG-07 (via AG-18 helper) | Hook decisions are the same for in-limit hooks; env var names kept (`docs/howtos/hooks.md`) | Locking tests: a hung hook times out; a 200 KiB input still runs the guard; a child is killed on timeout; `-p codemason-agent -E hooks` | **Done** (see commit "fix(agent): one deadline-bounded child runner") |
| 3. Agent panics + drift | AG-05, AG-08, AG-13, CO-14 | ASCII inputs unchanged | Non-ASCII trigger test; `timeout_secs = u64::MAX` test; Bedrock options test | **Done** (commit "fix(agent): batch 3 panics and Bedrock options") |
| 4. LLM stream correctness | CO-05, CO-06, CO-12 | Same events for valid streams | An `event: error` frame yields Error+Finish; parallel tool calls without `index` stay separate | **Done** (commit "fix(llm): batch 4 stream frame handling") |
| 5. Tool correctness | TL-02, TL-03, TL-04, TL-09, TL-12, TL-14, TL-01, TL-25 | In-root paths still accepted; the same output format | `test_glob_tool`, `test_git_tools` (strict path asserts), `test_multiedit`, `test_github_client_coverage` (cross-origin raw URL case) | **Done** (`6a71107` + commit "fix(tools): batch 5 git and fetch correctness") |
| 6. Network/SSRF + caps | TL-07 (trailing dot, CGNAT, NAT64), TL-08, TL-11, AG-11, RS-04, RS-05, RS-07 | The same messages for allowed URLs and archives | `test_mf_security*`, `test_add`, `test_connector_install`, `test_automation_surface` | **Done** (`8589d4a`) |
| 7. Research / plugins / server / UI bugs | RS-01, RS-02, RS-03, RS-06, RS-08, RS-09, RS-14, UI-02, UI-03, UI-04, UI-06 | LF / ASCII behaviour unchanged; 429 text unchanged | CRLF fixtures per parser; rate-window test; spinner test | **Done** except UI-03 (blocked by pins) and the UI-02 cross-window merge |
| 8. Perf | RS-16, RS-17, RS-18, TL-16 | Same results | connectors 379, plugins 502, research 716+788 (`cargo test -p <crate>` at `301f33b`) | **Done** (`301f33b`) for RS-16, RS-17, RS-18; TL-16 **Rejected**: a fix needs a field on `ScannedFile` (built as a literal in `test_store.rs`, `test_m6_data_layer.rs`, `test_types_display.rs`) or every scanned body held in memory; the second read hits the page cache |
| 9. Infra | RS-22, RS-23, RS-24, RS-25, CO-17 | Same build | `cargo clippy --workspace --all-targets -- -D warnings` (CI flags) clean; `Cargo.lock` unchanged; each edited crate `cargo check -p <crate> --all-targets`; llm 354; `check-vcs-duplication.sh --self-test` plus a seeded nested file (at `f2634ac`) | **Done** (`4822443`, `f2634ac`). The two Python guard edits (root `src/`, root `tests/`) were verified once Python was available: each guard flags a seeded violation in its new scope and passes the clean tree (`47d1514` fixes a Windows-only path bug in `check_shared_guards.py`). Bench `@stable` kept: it matches `rust-toolchain.toml` |
| 10. Small duplicates | TL-17, TL-19, RS-19, AG-12 | The suites pass unchanged; no test edits | tools-core 469, research 1,510, agent lib 372 and `all` (only the W-15 races remain), app-core (8 W-15 races; each passes alone), at `8f51b44` | **Done** (`8f51b44`). TL-19 **Superseded** (already a re-export). AG-12 re-derived: 9 inline copies, not 6, plus 2 colon-only config readers, which are a different format and stay. Found on the way: agent lib-target Windows failures (`d84aaff`; its test edit is portability only, `/tmp` replaced by `temp_dir()`) |
| 11. Credential prefixes | CO-07 | Existing prefixes behave the same in both places; the new ones are a defect fix (open-low CO-07) | types 501, telemetry 364 (plus 364 with `--features telemetry`), config 490, server 231, agent 2,805 (no failures), app-core (8 W-15 races; each passes alone), shared-guards clean, at `832b373` | **Done** (`832b373`). A bare `npm_` prefix would have redacted the `npm_install` tool name, so the new entries need 16 characters after the prefix |
| 12. LLM stream dedupe | CO-13, CO-15, CO-16 | Every pinned message byte-identical | llm 720 plus 2 new `test_first_byte_timeout` tests, at `1c33601` | CO-13 and CO-15 **Done** (`1c33601`). Pin found: `test_wire_decode` greps `azure_resource.rs` for `ToolCallAccumulator`; the delegation comment names it, which is true. CO-16 **Blocked: owner decision**. copilot and huggingface differ from `sse.rs` on (1) an empty stream (no error vs an error), (2) a body-decode failure (raw vs a friendly message) and (3) tool calls without an `index` (keyed by id, the CO-06 fix, vs skipped); ollama has its own empty-stream text. Folding them needs 3 spec switches or a behaviour change |
| 13. Surface dedupe | RS-10, RS-11 | Each crate keeps its messages | plugins 502, connectors 379, surface 70 (4 new), vcs-duplication and shared-guards clean, at `9b1f042` | **Done** (`9b1f042`). Two drifts fixed: plugins accepted a `C:/x` entry on Unix; connectors kept a source with an unsafe subpath. The claim that only plugins rejects symlinks was stale (connectors refuses them in `copy_dir_recursive`). The download loops still differ in error type and stay per crate |
| 14. GUI Material gaps | UI-13, UI-14 | Same labels, actions, click-only focus and Wrap toggle state; the look changes on purpose (Material First) | gui 742 (2 new material tests), at `d271eaf` | **Done** (`d271eaf`). `material::button::small_button` replaces 4 plain-egui click-only buttons (diff, code block, empty-state starters, status bar Details); the waiting count is a `material::badge` in the error tone |
| 15. Lying comments | CO-19, CO-20, AG-26, RS-27, RS-28, UI-18, config XDG cache comment | Comment-only, except AG-26: its `#[must_use]` now applies | clippy `-D warnings` on 8 crates; doc tests agent 81 and config 8; `test_block_in_place_guard`; no test or script greps the old text, at `7df829a` | **Done** (`7df829a`). UI-18 had 2 more "TUI main loop" sites than listed. Finding: `memory_row_line_indices` is never filled, so the memory panel cursor never scrolls; this is dead code, left as it is |
| 16. Monster: `merge_inner` | CO-24 plus the decomposition | Same merge order and rules | config 490 unchanged; a missing field in the destructure gives E0027 (checked, then restored), at `88ba8d2` | **Done** (`88ba8d2`). Down from 403 lines (`awk` count at `798eb76`; the audit measured 379) to 241, of which 62 are the guard's field list. Eight section helpers plus `override_some` |
| 17. Monster: `storage::migrate` | Decomposition | Same DDL, same order, one transaction | storage 427 plus 54 doc tests unchanged (`test_warm_index_scan` pin included); schema literal byte-identical (`cmp`), at `ce49e83` | **Done** (`ce49e83`). Down from 670 lines (at `585e630`; the audit measured 686) to 62. The schema is now `SCHEMA_DDL`; the four phase helpers sit before `impl Storage`, outside the pinned text window |
| 18. Monster: `process_user_message` | Decomposition | Same events, errors and loop control | agent lib 372 and `all` 2433 unchanged; 65 source-text pin tests; clippy `-D warnings`; tool-rename, silent-error and unwrap gates OK, at `523d594` | **Done** (`523d594`). Down from 1,830 lines to 99: 14 phase helpers plus `TurnScope`/`TurnEnv`/`TurnState` and a `StepExit` loop-control enum. New pin class: `test_approval_blocks_result` matches the result handler's exact indentation (32 spaces), so `tool_phase::run_tool_phase` keeps that code in a scoped block at its original depth. Also found: the silent-error and unwrap gates failed on Windows because of path separators (fixed separately) |
| 10..n. Refactors | Section 5, one car per row group; monster functions one car each | The suite passes unchanged; no test edits | `cargo test -p <crate> --test all` | Approved |
| Last. Structural | Section 6 | Each sanctioned test edit enumerated in its ticket; non-vacuity proof per rewritten guard | per row | Blocked: needs test-edit sanction |
| - | Section 7 | - | - | Blocked: owner decision |
| - | Section 8 | - | - | Deliberate |

### Prevention decisions

- **Car 0:** Guard added. `test_global_config_dir_xdg` fails if the resolver stops honouring the variable. A Windows CI job (O-07) is the real guard for the class.
- **Car 1:** Guard added: `test_merge_inner_sections`. **Correction (batch 16):** this line used to claim car 1 also added exhaustive destructuring (CO-24), but that was never written. Batch 16 added it (`88ba8d2`).
- **Cars 2-7:** Guard added (a locking test per defect, with non-ASCII, CRLF or oversized input).
- **Car 8:** No guard justified. The fixes remove work with no visible behaviour change; the crate suites prove results are unchanged.
- **Car 9:** Guard added. The three guards now cover the root package and the whole plugins crate; the VCS guard has a non-vacuity run.
- **Car 10:** No guard justified. `ModelRef` now owns the three parse rules, so a new copy would have to ignore an obvious method; a source grep would be brittle.
- **Car 11:** Guard added. `test_credential_prefixes` fails when a listed prefix is not redacted (shown by removing the `AIza` arm).
- **Car 12:** Guard added. `test_first_byte_timeout` locks the helper's timeout and send-failure messages.
- **Car 13:** Guard added. `test_surface_archive` locks both drift fixes and the oversized-entry data each crate words for itself.
- **Car 14:** Guard added. `test_material_small_button_*` locks the size, the click-only focus rule and the toggle state.
- **Car 16:** Guard added. Exhaustive destructuring of the overlay in `merge_inner` (CO-24): a new `Config` field is a compile error until it is merged.
- **Lying comments:** No guard justified.

---

## 10. Lessons for the next audit

- **Run the suite on the platform you are on before auditing.** On Windows, three test binaries failed or did not compile, and one leaked into the real profile. A test-isolation guard (`check-test-env-isolation.sh`) that checks a lock is held says nothing about whether the variable it sets is honoured on this OS.
- **`dirs::*` honours XDG and `HOME` only on Linux.** Every `set_var("XDG_*" | "HOME")` isolation is a no-op on Windows and macOS unless the resolver reads the variable itself.
- **Enumerate struct fields against merge functions.** CO-01 was invisible to tests that build `Config` directly.
- **Child-process hygiene is the biggest defect class.** It appears in six places, each with its own timeout and kill policy (AG-01/02/04/06/07, TL-05). One helper is the fix.
- **"Floods the model" claims need the 12k cap in mind:** `MAX_TOOL_RESULT_CHARS_FOR_LLM` refutes them all; only memory use remains.
