# GUI Quick Start Guide for codemason

A hands-on guide to using **codemason** through its desktop window (the egui GUI).

Keystrokes are given as Linux/Windows and macOS forms side by side, written
`Linux/Windows | macOS`. Keys with no macOS form given (and `-` in a macOS
column) have no macOS binding in the GUI keymap: the Linux/Windows chord is sent
as typed, and on macOS an Option+letter that types a character types it in the
prompt instead. Plain keys (`Enter`, `Esc`, `Tab`, arrows) are the same
everywhere.

---

## 1. Starting the GUI

Open a terminal in the project you want to work on and run:

```bash
cdmn
```

If `codemason` is not on your `PATH`, use the full path after building:

```bash
/path/to/codemason/target/release/cdmn
```

A window opens with the message pane and the prompt in the middle, the status
bar along the bottom, and the other panels (sessions, log, profile, tasks,
memory, agents, teams) docked around them when shown. The window opens
maximized the first time; after that it remembers its size and position. You can
start typing in the prompt immediately.

`--gui` is accepted and does nothing: the GUI is the default.

### No display

On Linux and the BSDs the GUI needs `DISPLAY` or `WAYLAND_DISPLAY`. With
neither set (an SSH session, a container), `cdmn` exits with a message that
points at `cdmn run`:

```bash
cdmn run "Summarise the open TODOs in this repository"
```

`cdmn run <prompt>` runs a single prompt headlessly and needs no window. The
server and ACP modes are headless too and are unchanged.

### Useful startup flags

| Flag                                   | What it does                                         |
| -------------------------------------- | ---------------------------------------------------- |
| `--model ollama_cloud/kimi-k2.7-code` | Start with a specific model                          |
| `--agent rust-coder`                 | Start with the `rust-coder` agent profile          |
| `--yes`                              | Auto-approve all permission prompts (YOLO-style)     |
| `--no-tui`                           | Skip the window and run a single prompt in the terminal |
| `--log`                              | Open the GUI with the log panel already visible      |
| `--new`                              | Start a fresh session instead of reopening the last ones |

`cdmn session resume <id>` opens the GUI on a stored session. The background
services (cron, automation, code index, spec manager) start as before.

### Example

```bash
cdmn --model ollama_cloud/kimi-k2.7-code --agent rust-coder
```

You can also open the provider setup directly from the prompt:

```text
/provider
```

---

## 2. Creating an `AGENTS.md` file for your project

`AGENTS.md` is a project-level instruction file that codemason automatically
loads into its system prompt. It tells the agent how your codebase is
organised, what conventions to follow, and what tools to prefer.

Create it at the root of your project workspace:

```bash
# In the root of the project you are working on
cdmn run "Create a concise AGENTS.md for this Rust project"
```

Or create it manually:

```bash
touch AGENTS.md
```

### Typical `AGENTS.md` contents

```markdown
# Project Guidelines

## Technology Stack
- Language: Rust (edition 2024)
- Build tool: Cargo
- Minimum Rust version: 1.85

## Code Style
- 4-space indentation, 100-column line limit
- Use snake_case for functions and variables, PascalCase for types
- Prefer explicit types and `Result<T, E>` error handling
- Use `tracing` for logging; avoid `println!` in library code

## Testing
- Tests live in each crate's `tests/` directory
- Use `#[test]` for sync tests, `#[tokio::test]` for async tests
- Each crate builds one `tests/all.rs` target that `#[path]`-includes every
  `tests/test_*.rs` file; add a new test as one more `#[path] ... mod ...;` line
- Run tests with `cargo test --workspace`
- Run one file with `cargo test -p <crate> --test all test_foo::`

## Tool Preferences
- Use `codeindex_search` and `codeindex_references` for code symbol lookups
- Use `read` with `start_line` + `num_lines` for large files
- Prefer `multi_edit` when changing several files at once

## Documentation
- Use `///` doc comments for public functions
- Update `CHANGELOG.md` and `SPEC.md` when adding features
```

Keep it factual and concise. codemason reads it on every launch, so overly long
files can consume context window.

---

## 3. Selecting a provider and model

### Provider setup dialog

1. Start codemason.
2. Type `/provider` in the prompt.
3. Select a provider from the list.
4. The API-key field is pre-filled with any existing stored key, shown unmasked
   in a wide dialog so you can edit or replace it. Press `Enter` to keep the
   current key, or type a new one. (Skip this step for providers that do not
   require a key, such as local Ollama.)
5. Choose a model from the provider's discovered model list.
6. Press `Enter` to confirm.

Every list in these dialogs (provider, model, router tier, thinking level, Azure
resource, router provider/bucket) is a modal over the window: move with
`Up`/`Down` or click a row, confirm with `Enter`, close with `Esc`.

If a provider is already configured, `/model` jumps straight to that provider's
model list instead of asking you to pick a provider again. Use `/model show` to
print metadata for the active model.

The status bar along the bottom shows the selected provider, model, and a
health indicator for the provider: reachable, health check in progress, or
unreachable / key missing.

When the Model Router is active the status bar shows the actual downstream
model and tier, e.g. `Model Router (claude-sonnet-4-20250514) / complex`.

### Example: Ollama Cloud with `kimi-k2.7-code`

1. Type `/provider`.
2. Choose **Ollama Cloud**.
3. Enter your Ollama Cloud API key if prompted.
4. Select **`kimi-k2.7-code`** from the model list.
5. Confirm.

The status bar will update to something like:

```text
ollama_cloud/kimi-k2.7-code
```

Other providers work the same way; only the available model list and the
required credentials differ.

### Available providers

codemason supports these providers out of the box:

| Provider                | Typical credential           | Notes                                |
| ----------------------- | ---------------------------- | ------------------------------------ |
| Anthropic               | `ANTHROPIC_API_KEY`        | Claude family                        |
| OpenAI                  | `OPENAI_API_KEY`           | GPT family                           |
| Google Gemini           | `GEMINI_API_KEY`           | Gemini models                        |
| Hugging Face            | `HF_TOKEN`                 | Open-source models via Inference API |
| GitHub Copilot          | IDE token (auto-discovered)  | No separate API key                  |
| Ollama                  | none (local)                 | Local or remote Ollama server        |
| Ollama Cloud            | `OLLAMA_API_KEY`           | Managed Ollama endpoints             |
| Azure AI Foundry        | `AZURE_AI_FOUNDRY_API_KEY` | Microsoft Azure-hosted models        |
| Azure Resource          | per-entry `api_key_env`    | File-based Azure deployment config   |
| Amazon Bedrock          | AWS credentials              | AWS SigV4 signing                    |
| Generic OpenAI          | `GENERIC_OPENAI_API_KEY`   | Any OpenAI-compatible endpoint       |
| XAI                     | `XAI_API_KEY`              | xAI / Grok models                    |
| Model Router            | cluster of providers         | Virtual provider that routes prompts |

Use `/provider show` to inspect currently configured providers and their
settings, and `/provider router` (or `/router help`) to set up the Model Router.

---

## 3.1 Configuration snapshots

The `/config` slash command family lets you inspect and back up your global
`codemason.json` without leaving the window.

```text
/config show          # show working dir, data/config dirs, storage, code index, memory, agents
/config save          # snapshot the current global codemason.json under ~/.config/codemason/saves/
/config list          # browse saved snapshots and restore one with Enter
```

`/config show` is useful when checking which config file is active or whether
the code index and memory directories exist.

---

## 3.2 Tool listing

Every tool the registry holds is advertised to the model. `/tools` shows the
help block and `/tools list` (alias `/tools show`) prints one row per registered
tool with its source and description.

```text
/tools
/tools list
```

A tool you expect but do not see is not registered: the subsystem that owns it is
inactive. The ten codeindex tools, for example, appear only while the code index
is on (`/codeindex on`).

---

## 3.3 Autopilot

Autopilot lets the agent continue iterating autonomously after each turn until
it calls `task_complete`, hits a user-defined limit, or you run `/autopilot off`.

```text
/autopilot on
/autopilot on --max-tokens 16000 --max-time 300
/autopilot status
/autopilot off
```

`Alt+O` (Linux/Windows) toggles autopilot without leaving the keyboard: it drives the same
state machine as `/autopilot on|off`, but because a key chord cannot carry flags
it applies a default 1800-second (`--max-time`) limit — a stray keystroke cannot
start an unbounded run. Use `/autopilot on` with no flags when you want a run
with no time limit. Pressing `Alt+O` again stops the run.

The status bar shows an `AUTO` indicator that is on while autopilot runs.

---

## 3.4 Startup timings and run-cost banner

After launch, codemason records how long each startup stage took. Run `/startup`
at any time to see a breakdown such as config load, provider health check,
code-index startup, and session creation.

When an agent run completes, a transient banner appears at the top showing
`run complete · input+output tokens · $cost · duration`. The full details are
always written to the log panel. The banner is dismissed by any keypress or its
Dismiss button; if you start typing the next prompt, the first character is
preserved rather than being swallowed.

---

## 4. Issuing prompts to codemason

Click in the **Prompt** pane at the bottom and type your prompt.

Press **`Enter`** to send.

### Multi-line prompts

- Press **`Shift+Enter`** or **`Alt+Enter`** to insert a newline without sending.
- Press **`Enter`** to submit.

### Example prompts

```text
Explain the purpose of the codemason-types crate
```

```text
Refactor src/main.rs to split the CLI argument parsing into a separate module
```

```text
Add unit tests for the bash permission command-name extraction logic
```

### Mentions and attachments

- Type **`@`** to open the file picker and mention a file in your prompt.
- Type **`/`** to open the slash-command menu. **`Tab`** completes the highlighted
  command; `Up`/`Down` move the highlight and `Esc` closes the menu.
- Attach images: see "Attaching files" below.
- Pending attachments appear above the prompt before you send.

### Attaching files

- **File menu** — choose **File > Attach Images...** to open the native file
  dialog and pick one or more images (png, jpg, jpeg, gif, webp, bmp, tiff).
  Cancelling the dialog changes nothing.
- **Clipboard** — paste an image from the clipboard into the prompt, or press
  **`Alt+V`** (Linux/Windows), to stage it as an attachment.

The same **File** menu has **Open Project...** (register a project folder, as
`/project add <path>`) and **New Project Folder...** (start a session in a new
folder, as `/session new <path>`). On macOS the menu bar carries About, Quit
(`Cmd+Q`) and a View menu with the panel toggles; the File items are in the
window's own File menu.

### Clipboard

- **`Ctrl+C` | `Cmd+C`** — copy the selection in the prompt to the clipboard.
- **`Ctrl+X` | `Cmd+X`** — cut the selection from the prompt.
- **`Ctrl+V` | `Cmd+V`** — paste at the cursor, replacing any active selection.
- **Right-click** in the prompt — opens a context menu with **Copy**, **Cut**
  and **Paste**.
- **`/clip`** — copies the entire rendered contents of the message window (the
  plain-text transcript) to the system clipboard in one step.

Transcript text in the message pane is not selectable; use `/clip` to copy it.

### Permission prompts

When codemason wants to run a shell command, write a file, or perform another
protected action, a permission dialog appears over the window. You
have 120 seconds to:

- **`y`** — allow once
- **`a`** — allow always for this session
- **`n`** — deny

The dialog's buttons do the same.

The dialog title shows a live countdown (e.g. `Permission Required (1:45 remaining)`).

### The input queue and the queue-control menu

The input field stays live while the agent is executing. Instead of rejecting
your message with `busy - wait for the current turn to finish`, pressing
**`Enter`** appends it to a bounded FIFO **input queue** (maximum 32 entries).
The queued entries are listed above the prompt with a count, for example `03`
for three queued entries. The oldest queued entry runs automatically when the
running turn ends, and the count decrements each time. Nothing is shown when the
queue is empty.

A queued entry is added to the **input history** the moment you press `Enter`
(so `Up` recalls it immediately), not when it runs, and it is not echoed into
the message window until it is actually dispatched.

A **slash command** (`/…`) is queued exactly like a plain message while a turn
is running (FR-017 amendment): pressing `Enter` on `/status`, `/agent`, or a
`queue`-style command appends it to the queue and it runs at the next turn
boundary (or immediately via the queue menu's `Next` / `/queue next`). A synchronous
command leaves the boundary free, so a run of consecutive queued commands
executes back-to-back rather than stalling behind the first. At queue capacity
the command is rejected and restored to the input field, so nothing is lost.
Only **bang commands** (`!…`) keep the `busy - wait for the current turn to
finish` refusal.

Press **`Alt+Q` | `Cmd+Alt+Q`** during a run to open the **queue-control menu** — a centred
overlay with exactly four options. Use **`Up`**/**`Down`** to move the highlight
and **`Enter`** to activate the highlighted row:

| Option        | Behaviour |
| ------------- | --------- |
| `Next`        | Stops the running turn and immediately dispatches the **oldest** queued entry, then closes the menu. FIFO order is preserved. `Next` is shown as non-selectable when the queue is empty (the highlight skips it). |
| `Stop` / `Resume` | While a turn is executing the row reads **`Stop`**: it halts the agent exactly as pressing `Escape` would and does **not** advance the queue. When the agent is already stopped the same row reads **`Resume`**: it restarts the interrupted work. |
| `Clear`       | Opens a `Clear the input queue?` confirmation dialog with `Yes` / `No`. **`No` is the default**, so pressing `Enter` without changing the selection changes nothing; only `Yes` empties the queue. Selecting `No` or pressing `Esc` closes the dialog and leaves the queue, the input buffer, and the staged attachments untouched. |
| `Show`        | Opens a scrollable **queue panel** listing every queued entry oldest-first with a block cursor (see below). |

`Esc` dismisses the menu taking no action — the input buffer, staged
attachments, the queue, and the running turn are all left untouched. Queued
entries survive a cancelled turn, an agent error, or a compaction; they are
only removed when they are dispatched or explicitly cleared.

#### The `Show` queue panel

The `Show` row opens a panel that lists every queued entry, oldest first, with a
block cursor marking the highlighted entry. Use it to inspect, reorder, or prune
the queue without leaving the window:

| Key      | Behaviour |
| -------- | --------- |
| `Up` / `Down` | Move the block cursor up and down the list; the panel scrolls to keep the highlighted entry visible. |
| `Enter`  | Moves the highlighted entry **one step toward the front** of the queue, so it runs sooner, and keeps the cursor on it. Press `Enter` again to walk it further forward; at the front it is a no-op. The order of every other entry is preserved. |
| `Del`    | Removes the highlighted entry from the queue (the counter decrements). |
| `Esc`    | Dismisses the panel. |

Only `Esc` closes the panel; `Enter`, `Del`, and the navigation keys keep it
open and repaint it immediately. The panel never touches your in-progress draft
or staged attachments.

There is also a `/queue` slash command that inspects the same queue from the
prompt:

| Sub-command  | Behaviour |
| ------------ | --------- |
| `/queue list` or `/queue` | Lists the queued entries in submission order (oldest first) with the queue depth. |
| `/queue clear` | Empties the queue immediately; the counter disappears. |
| `/queue next`  | Dispatching the oldest entry immediately. While a turn is still executing the action is deferred to the turn boundary (it never overlaps a running turn or a compaction); use the queue menu's `Next` to stop the current turn and run it now. |
| `/queue help`  | Shows the sub-command usage. |

---

## 5. Goal-driven loops with `/loop`

The **`/loop`** slash command runs a goal-driven agentic loop: codemason picks
the agent you name, then iterates **plan → act → observe** — planning the
next action, executing tool calls, feeding each observation back into the
model's context — until a stop condition fires. Four stop conditions end the
loop, each rendered as a status-aware end banner with the iteration count:

| Termination status | Stop condition |
| ------------------ | -------------- |
| `completed` | Goal achieved (model responded without tool calls; when a verification command is configured it must exit successfully) |
| `error` | An unrecoverable stage failure (provider transport, tool panic, context overflow, permission hard-deny); the failed stage is **not** retried |
| `budget_exhausted` | The step limit or token-cost budget was consumed **before** the next LLM request |
| `interrupted` | You pressed `Esc` mid-loop; the session stays resumable and a rollback offer may follow |

### Usage

```text
/loop                              # open the interactive setup dialog
/loop <agent> <goal text...>       # start immediately with documented defaults
```

The one-shot form activates the documented defaults: step limit **512**
(or `loop.max_steps` from `codemason.json`), config cost limit, checkpoints
**on**, no verification command, and no restrictions. Three flags override
the run's budgets: `--max-steps N`, `--cost_limit N` (or `--cost-limit N`),
and `--timeout N` (checkpoint-prompt seconds). An empty or
whitespace-only goal never starts a loop — the dialog shows an error naming
the missing field (`goal`) and re-opens for correction; the one-shot form
prints usage help instead.

### Goal format

A goal is the structured contract the loop iterates against. All fields
except the success state are optional:

| Field | Meaning |
| ----- | ------- |
| **Success state** | What must be true for the loop to complete (plain text) |
| **Verification command** | Runs automatically when the model responds without tool calls; success completes the loop, failure appends its output as the next observation (steps remaining) or ends `budget_exhausted` (no steps left) |
| **Scope boundaries** | Path globs the loop's file operations must stay inside |
| **Constraints** | Read-only path globs — writes are denied and returned as observations (anti-cheat: never satisfy the goal by modifying them) |
| **Tool set** | Restricted tool surface; calls outside the set are denied with a scope observation |
| **Budget** | Maximum iterations and maximum accumulated tokens before `budget_exhausted` |

### Setup dialog

With no arguments, `/loop` opens a setup dialog with an agent selector
(arrow keys), goal field, verification-command field, scope field
(comma-separated globs), constraints field, tool-set field, step-limit field (default 512), cost-limit field (from
config), and a checkpoints toggle
(default on). `Esc` cancels without starting anything and preserves all
entered values for the next `/loop` in the same session.

### Budget + checkpoint tunables

```json
{
  "loop": {
    "max_steps": 512,
    "cost_limit": 200000,
    "error_retry_allowance": 3,
    "checkpoints": true,
    "checkpoint_timeout_secs": 120
  }
}
```

- `max_steps` — iterations before `budget_exhausted` (FR-013); a per-agent
  `agent.<name>.max_steps` wins over the loop-level budget.
- `cost_limit` — accumulated input + output tokens before
  `budget_exhausted` (FR-014); `null` disables the gate.
- `error_retry_allowance` — consecutive recoverable tool failures before the
  loop stops with `error` (FR-012).
- `checkpoints` — force a human-approval prompt before destructive actions
  (file deletion, config writes, dependency installation, destructive git
  operations) even when an allow rule would auto-approve them (FR-015); a
  prompt timeout is treated as denial.
- `checkpoint_timeout_secs` — seconds a checkpoint prompt waits before the
  safe-choice denial applies.

### Rollback flow

When a loop terminates, codemason shows the change summary as a diffstat line
and offers a **one-key rollback**: `Enter` accepts and restores the pre-loop
workspace snapshot; `Esc` (or any other key) declines and keeps the changes.
Rollback is snapshot-only when the workspace is not inside a git repository —
the loop warns you before writing anything in that case.

---

## 6. Research with `/research`

Use the **`/research`** slash command to gather information from the web and
cross-reference it with local files.

```text
/research Rust async runtime design patterns
```

codemason will:

1. Search the web for relevant pages.
2. Fetch key pages.
3. Cross-reference findings against your local codebase and project memory.
4. Write a self-contained dated `RESEARCH-<YYYY-MM-DD>.md` report.

### Common forms

```text
/research <topic>                # general research
/research compare tokio vs async-std
/research best practices for headless browser testing
/research create rust-patterns "Rust design patterns" --use-local
/research create rust-patterns --from-url https://example.com/article
/research create rust-patterns --from-file ./path/to/document.pdf
/research list
/research open rust-patterns
/research search "async runtime"
```

`/research list` prints an aligned table (NAME, TITLE, STATUS, CREATED,
MODIFIED) in a preformatted block so the columns are not distorted by
markdown rendering; `/research search` prints a bullet list of matches.

The `create` form supports a number of optional flags:

- `--from-url <URL>` — fetch the URL and use its content as the research subject.
- `--from-file <PATH>` — extract text from a local document (PDF, DOCX, XLSX,
  PPTX, ODT, ODS, ODP, TXT, MD) and use it as the subject.
- `--use-local` — include local files and prior specs in the analysis.
- `--use-specs` — cross-reference existing `specs/`.
- `--use-low-relevance` — keep low-relevance web sources instead of filtering
  them out.
- `--depth shallow|standard|deep` — control how broadly codemason searches.
- `--format report|executive-summary|comparison-table|source-bibliography|imrad|newsletter`
  — choose the output artifact. `newsletter` emits only the reader-facing
  sections (topic, executive summary, top 10 implications, open questions,
  concepts, findings, findings relationship diagram) and drops the audit-trail
  blocks, so the document reads as a newsletter rather than a report.

Reports are saved under `research/<name>/RESEARCH-<YYYY-MM-DD>.md`. The primary
report uses the item's creation date, so it is stable from creation onwards;
each later run (including a same-day `/research update`) writes its own dated
file and leaves the earlier one in place. Open the report with
`/research open <name>` - it resolves whichever report exists rather than
assuming a file name. Quality-assurance detail (contradiction graph, loci
analysis, depth investigation, reconcile, tensions, audits) is kept in the
companion `CORPA-<YYYY-MM-DD>.md`; it includes a `Sources Reference` copy of the
References Index so `[#N]` citations resolve in both documents. An item created
by an older codemason keeps its undated `RESEARCH.md` / `CORPA.md`, which resolves
the same way.

While the run is gathering, the research progress view above the message pane
does not list each captured URL. Instead it maintains a compact summary table
with one row per backend search engine (duckduckgo, brave, openalex, wikipedia, tavily, ...)
showing the counts of captured files by type (`page` / `pdf` / `yt`) and a
`languages` cell listing every currently acquired language with its article
count:

| engine   | page | pdf | yt | total | languages           |
| -------- | ---- | --- | -- | ----- | ------------------- |
| brave    | 3    | 0   | 0  | 3     | ENGLISH:2, FRENCH:1 |
| openalex | 0    | 1   | 0  | 1     | ENGLISH:1           |

A `total` row aggregates every engine. Per-URL capture lines still appear in
the log panel (not the message window) so individual fetch failures remain
traceable.

> Note: web research uses keyless search by default; a `TAVILY_API_KEY` or
> `LANGSEARCH_API_KEY` can be configured in `codemason.json` for higher-quality
> results.

---

## 7. Creating a specification and plan with `/spec`

The **`/spec`** slash command creates a tracked specification (`SPEC.md`) and
an implementation plan (`PLAN.md`) for a feature.

### Create a new spec

```text
/spec create add-user-authentication
```

codemason asks a few clarifying questions if needed, then generates:

- `specs/add-user-authentication/SPEC.md` — requirements, scope, acceptance criteria
- `specs/add-user-authentication/PLAN.md` — tasks, dependencies, effort estimates
- `specs/add-user-authentication/TESTPLAN.md` — manual test plan with `TC-NNN` test cases

### Regenerate plans after editing a spec

```text
/spec update add-user-authentication
```

`/spec update` re-reads the existing `SPEC.md` and regenerates `PLAN.md` and
`TESTPLAN.md` to match the current requirements. The `SPEC.md` file is not
modified; existing task IDs are preserved where unchanged. Archived specs
cannot be updated.

### List and validate specs

```text
/spec list
/spec validate add-user-authentication
/spec status add-user-authentication
```

### Track progress

As you implement tasks, codemason can update the spec status automatically:

```text
/spec task add-user-authentication T-001 completed
```

Specs are stored in the `specs/` directory by default. They are intended to be
user-managed working documents, not part of the main git tree unless you choose
to commit them.

---

## 7a. Scaffolding a new project with `/new`

The **`/new`** slash command scaffolds a brand-new project in the current
directory (it must be empty apart from codemason artifacts). The same surface is
available as the `cdmn new` CLI subcommand.

### Scaffold a minimal project

```text
/new --language rust --type cmdline
```

The command streams each step into the message window as it completes
(guard, file emission, git init, remote status), then prints a summary with
the created files. Generated content:

- the codemason workspace: `.codemason/`, `specs/`, `log/`, `.gitignore`, `AGENTS.md`
- a runnable hello-world artifact set for any of the 26 supported application
  languages (`rust`, `python`, `go`, `typescript`, `shell`, ...) in a
  `library`, `cmdline`, `tui`, or `gui` layout, or sample-document stubs for
  data and build formats (`json`, `yaml`, `sql`, `cmake`, `maven`, ...);
  `/new help` lists every accepted value
- starter docs: `README.md`, `QUICKSTART.md`, `STATS.md`, `docs/`
- a git repository with an initial commit

### Add hosting and stacks

```text
/new --language rust --type cmdline --stack axum --github
```

`--stack axum` layers an axum server starter over the base Rust layout
(`warp`, `raylib` and `gtk4` are also known stacks; `/new help` lists them all); `--github` creates a private
GitHub repository, sets it as `origin`, and pushes. Use `--gitlab` instead
for GitLab. The two hosting flags are mutually exclusive, and a failed
remote step never undoes the local scaffold.

### Help

```text
/new help
```

Prints the usage page with the flag table and the supported language/type
values (derived from the scaffolder's registries, so they cannot drift).

---

## 7b. Managing plugins with `/plugins`

The **`/plugins`** slash command manages sandboxed Codex- and Claude
Code/Desktop-dialect plugins from the window. Enabled plugins contribute tools
(registered as `plugin_<id>_<tool>`) and slash commands. The same operations
are available from a shell as `cdmn plugins <sub>`.

```text
/plugins list --verbose          # list plugins, state, contributions, telemetry
/plugins add ./my-plugin         # install (enabled; loads next session)
/plugins enable my-plugin        # load and register its tools/commands
/plugins test my-plugin          # isolated harness: load + invoke each tool once
/plugins disable my-plugin       # unload and deregister (files kept)
/plugins remove my-plugin        # uninstall (refused while enabled)
/plugins help                    # usage block
```

Reports render in the message window with a `From: /plugins <sub>` header.
`/plugins test` prints one `[ ok ]`/`[fail]` line per harness step and confirms
the harness was unloaded with the live session untouched. Any of `/plugins`
with no arguments, `/plugins help`, or an unrecognised subcommand prints the
usage block. Plugins are discovered under `.codemason/plugins/` (project) or
`~/.config/codemason/plugins/` (user-global); set `plugins.enabled: false` to make
the subsystem inert.

An enabled plugin's non-tool contributions are bridged too: `commands/*.md`
prompt commands appear in the `/` menu, `skills/` directories join skill
discovery, `mcpServers` connect as `<plugin-id>.<server>`, `agents/*.md`
profiles join agent discovery, and `hooks` (declared inline or in a `hooks.json`
file) fire on the matching session lifecycle events. `list` shows a count column
for each kind; `add` records the plugin enabled.

See [`docs/howtos/slashcommands/plugins.md`](docs/howtos/slashcommands/plugins.md).

---

## 8. Stopping the agent with the Escape key

While codemason is actively streaming a response or running tools, press
**`Escape`** to cancel the current operation.

- The LLM stream stops immediately.
- In-progress tool calls are abandoned.
- You can type a new prompt right away.

`Escape` does **not** quit the application; it only interrupts the current
agent step. `Ctrl+X` also stops a running agent. (The old `Alt+X` binding was
removed; an Alt+letter chord with no binding is ignored and is not typed.)

---

## 9. Quitting codemason

To exit safely, do one of:

1. Press **`Ctrl+D` | `Cmd+Q`**.
2. Type `/quit` in the prompt.
3. Close the window.

While an agent turn is running, closing the window asks `Quit codemason?` with
**Quit** and **Cancel** buttons (`Esc` cancels); an idle window closes at once.
You can also press **`Ctrl+C`** to arm quit mode, then **`Ctrl+D`** to confirm.

---

## 10. Log panel — `Alt+L` | `Cmd+Shift+L`

Press **`Alt+L` | `Cmd+Shift+L`** to toggle the **Log pane**, docked on the right by default.

The Log panel shows a time-stamped, color-coded stream of runtime events:

| Prefix  | Meaning                  |
| ------- | ------------------------ |
| `INF` | General information      |
| `TUL` | Tool call / tool result  |
| `WRN` | Warning                  |
| `ERR` | Error                    |
| `CMP` | Context-compaction event |

Log entries include the short session ID and step number when available, e.g.
`[a3f7:12]`. This makes it easy to correlate log lines with message bubbles in
the main chat pane.

### Interacting with the Log panel

- **`Scroll`** with the mouse wheel or drag the scrollbar.
- Press **`Alt+L` | `Cmd+Shift+L`** again to hide the pane.
- `Ctrl+PageUp` / `Ctrl+PageDown` scroll the log.

---

## 11. Profile panel — `Alt+P` | `Cmd+Shift+P`

Press **`Alt+P` | `Cmd+Shift+P`** to toggle the **Profile pane**.

The Profile panel shows live performance data collected by the agent-loop
profiler:

- **uptime** — how long the current session has been running
- **samples** — number of profiler samples taken
- **ops** — number of distinct operations measured
- A table of operations sorted by **self time**, showing:
  - count
  - average milliseconds
  - total milliseconds
  - self milliseconds
  - max milliseconds
  - last milliseconds
  - operation name

Use this panel to spot slow operations, repeated work, or unexpectedly long
tool calls.

### Interacting with the Profile panel

- **`Scroll`** with the mouse wheel or drag the scrollbar.
- Press **`Alt+P` | `Cmd+Shift+P`** again to hide the pane.

Opening the Log pane dismisses the other side panes (an app-core rule); you can
also rearrange the panes (see "Panes and
layout").

---

## 12. TASKS panel — `Alt+I` | `Cmd+Shift+I`

Press **`Alt+I` | `Cmd+Shift+I`** to toggle the **TASKS pane**.

The TASKS panel lists tasks for the current session, fetched from the
SQLite-backed storage on every render. Each row shows:

```text
[<STATUS>] <subject> (owner) [blocked by #id, …]
```

Status colors:

| Status          | Color  |
| --------------- | ------ |
| `pending`     | Yellow |
| `in_progress` | Cyan   |
| `completed`   | Green  |
| blocked (derived) | Red    |

A task is **blocked** when its `blocked_by` list is non-empty and at least one
blocker is not yet `completed`. Blocked-ness is derived at read time — there is
no `blocked` status value (FR-005). When a task is `in_progress` and has an
`active_form`, it is shown as an indented sub-line beneath the subject.

### Managing Tasks

Create, update, or list tasks with the `TaskCreate` / `TaskUpdate` /
`TaskList` tools or the `/task` slash command:

```text
/task add Implement token bucket rate limiter
/task update task-abc123 --status completed
/task list
```

You can also use the tool directly in a prompt:

```text
Create a task list for adding OAuth2 login support
```

codemason will call `TaskCreate` to add the items, and the TASKS panel updates
immediately.

### Interacting with the TASKS panel

- **`Scroll`** with the mouse wheel or drag the scrollbar.
- Press **`Alt+I` | `Cmd+Shift+I`** again to hide the pane.

---

## 13. Memory panel — `Alt+M` | `Cmd+Shift+M`

Press **`Alt+M` | `Cmd+Shift+M`** to toggle the **Memory pane**.

The Memory panel surfaces three sources of project and user memory:

1. **Project Memory** — `.codemason/memory/MEMORY.md`
2. **Project Analysis** — `.codemason/memory/PROJECT_ANALYSIS.md` (if it exists)
3. **User Memory** — `~/.codemason/memory/MEMORY.md`

It also shows a summary line when the structured-memory SQLite store contains
entries:

```text
Structured memories: 42
```

Memory files are plain Markdown. codemason reads them automatically on startup
and can update them with `memory_write` / `memory_replace` during a session.

In addition, the `memory_store` tool writes **structured memories** to the
SQLite store with a category, tags, and confidence score. Successful writes now
report `stored: true`, and the Memory pane reflects the update.

### Typical uses

- Store project conventions that should persist across sessions.
- Keep a running analysis of the architecture.
- Remember user preferences (e.g. "always use `anyhow::Result`").

### Clearing this project's memories

`/memory clear` opens a `Clear this project's memory?` dialog with `Yes` / `No`
(`No` is selected by default, so a stray `Enter` cannot clear memory). Only
memories scoped to the current project directory are deleted; other projects'
entries are never touched.

```text
/memory clear
```

### Updating memory

```text
/memory write project "Use tokio::sync::RwLock for shared state in this codebase"
```

Or simply ask codemason:

```text
Remember that we prefer tracing over println in this project
```

### Interacting with the Memory panel

- **`Scroll`** with the mouse wheel or drag the scrollbar.
- Press **`Alt+M` | `Cmd+Shift+M`** again to hide the pane.

## Multiple sessions and the session sidebar

codemason can hold several sessions at once, each in its own project
directory. A session keeps running while you work in another one.

The Sessions pane (docked on the left by default) lists:

- `+ New session`
- `/ Search`, which filters the list
- a `Pinned` section, always shown; drag a session row onto it to pin it
- your own sections (see "Session menu" below), by name
- one section per project, the current project first. A blank line sits
  above `Pinned` and above each project heading. The heading shows
  the project's name, `v` (open) or `>` (collapsed), and `[github]` or
  `[gitlab]` when the project has a known remote. A project with no
  sessions is listed too, so you can start one in it.

Each session row shows its title, how long ago it was active, and at the
right edge:

- `#` when the session is bound to a pull request (GitHub) or merge
  request (GitLab); `#!` when Auto-fix has paused and is waiting on you.
- `Y` when the session has its own branch, followed by the number of
  pull/merge requests for that branch (no number when there are none or the
  count is not known yet). It is magenta when the session owns its git
  worktree.
- a state marker:

| Marker | Meaning |
| ------ | ------- |
| `!`    | Waiting on you (permission or question) |
| `*`    | Running |
| `x`    | Its run failed while you were elsewhere |
| `o`    | Its run finished while you were elsewhere, or you marked it unread |

Keys:

| Key | Action |
| --- | ------ |
| `Alt+S` | Show and focus, focus, or hide the Sessions pane (Linux/Windows chord) |
| `Ctrl+N` | New session in this project |
| `Ctrl+Up` / `Ctrl+Down` | Previous / next session |
| `Alt+R` | The session you were in last (Linux/Windows chord) |
| `Up`/`Down`, `Enter` | (sidebar focused) move, open the row |
| `Left`/`Right` | (sidebar focused) collapse / expand a project |
| `p` / `a` / `F2` | (sidebar focused) pin, archive, rename |
| `u` / `f` / `D` | (sidebar focused) mark unread/read, fork, delete (asks first) |
| `m` / `Shift+F10` | (sidebar focused) open the menu of the session or project heading; right-click also opens it |
| `v` / `Ctrl`+click | (sidebar focused) add a session to the selection, or take it out |
| `/` | (sidebar focused) filter by title or project |
| `n` | (on a project heading) new session in that project (its checkout must be under the launch directory or in `dirs.allowed_roots`) |
| `b` | (on a project heading) prefill the command for a new session on its own branch, with the project's name and a random branch name such as `focused_turing` (edit it before `Enter`) |
| `Esc` | (sidebar focused) clear the selection, then the filter, then return to the input |

With the mouse, click a row to open it and drag the pane's border to resize it. The same actions are available as `/session` commands
(`/session help`); projects are managed with `/project` (`/project help`). codemason reopens your open sessions on the next start;
use `cdmn --new` to start fresh.

### Session menu

Right-click a session row, or focus the sidebar and press `m` (or
`Shift+F10`) on it. Press an item's letter, or move with `Up`/`Down` and
press `Enter`. `Esc` closes it.

| Item | Key | What it does |
| ---- | --- | ------------ |
| Open PR / Open MR | `G` | Open the bound pull/merge request in your browser (only when bound) |
| Auto-fix: on/off | `X` | Turn Auto-fix on or off (only when bound) |
| PR/MRs (N) | | List every pull/merge request the session has had; click one to open it in your browser |
| Open in | | Open the session's folder in your editor, file manager or a terminal |
| Pin / Unpin | `P` | Keep the session at the top |
| Mark as unread / read | `U` | Show `o` until you open the session |
| Rename | `R` | Prefills `/session rename` |
| Fork | `F` | Copy the conversation into a new session and switch to it |
| Move to section | | Put the session in one of your sections, a new one, or back under its project |
| Archive | `A` | Hide it from the list (`/session unarchive` brings it back) |
| Delete | `D` | Delete it for good, after a Yes/No question (default No) |

Right-click a project heading (or press `m` on it) for the project menu:
New session (`N`), New session on branch (`B`), Open in, Rename project
(`R`), Properties (`P`: description, backend and model for the project;
an unset one follows the global setting), Move up/down (`K`/`J`, the order is remembered), Select all (`S`),
Collapse all/Expand all (`C`/`E`), Archive all (`A`) and Delete all
sessions (`D`, asks once). Greyed items cannot run right now.

With several sessions selected (`v`, `Ctrl`+click or Select all; selected
rows show `*`), the menu on a selected session acts on all of them: Pin,
Mark as unread, Move to section, Clear selection, Archive and Delete.

The same actions as commands: `/session unread`, `/session fork`,
`/session delete <id-or-title>`, `/session section <name>`,
`/session section new <name>`, `/session section rename <name>`,
`/session section delete [name]`.

### Auto-fix for a pull or merge request

Auto-fix wakes a session when its pull request (GitHub) or merge request
(GitLab) needs work: CI failed, the branch has merge conflicts, or someone
left a review comment. The session gets a message describing the problem
(with the end of the failing job's log) and fixes it like any other prompt.

GitHub (github.com):

1. Make sure `GITHUB_TOKEN` is set, or log in with `/github login`.
2. Ask the agent to open a pull request; `github_create_pr` binds it to the
   session. For an existing one, run `/autofix bind 67` (or `/autofix bind`
   for the open PR of the session's branch, or paste its URL).
3. Run `/autofix on`, or press `X` in the session menu.

GitLab (gitlab.com or your own instance):

1. Set up GitLab with `/gitlab setup` (or `GITLAB_URL` and `GITLAB_TOKEN`).
2. Let the agent open a merge request with `gitlab_create_mr`, or run
   `/autofix bind !12`.
3. Run `/autofix on`.

The row shows `#`, and the status bar shows lines like
`PR #67 auto-fix: CI failed (attempt 1/3)`. If the session is busy, the
message is queued; if you are in another session, its row turns `o`. After
3 tries on the same commit Auto-fix pauses (`#!`) until you run
`/autofix on` again. It stops when the request is merged or closed. It never
merges, force-pushes or resolves review threads, and a `git push` still asks
for permission unless your permission mode allows it. `/autofix status`
shows what it last did; `/autofix off` turns it off; `/autofix unbind`
removes the binding.

GitHub Enterprise and settings go in your global `codemason.json` (not a
project's), with the token in `GH_ENTERPRISE_TOKEN`:

```json
{
  "vcs": {
    "github": {
      "hosts": [{ "host": "github.example.com", "api_url": "https://github.example.com/api/v3" }]
    },
    "autofix": { "poll_secs": 60, "max_attempts": 3, "default_on": false }
  }
}
```

---

## Panes and layout

Every panel is a dockable pane: Messages, Prompt, Sessions, Log, Profile,
Tasks, Memory, Active agents, Teams and Queue. The Messages and Prompt panes are
the window's canvas: they cannot be closed or tabbed, but other panes can be
split beside them.

- **Drag a tab** to move a pane: next to another pane, into another pane's tab
  group, or out of the window frame to float it.
- Close a pane from its tab, or with its shortcut (`Alt+L` | `Cmd+Shift+L` for
  Log, and so on). A hidden pane comes back where it was.
- The layout is saved when a drag or resize ends and again when the window
  closes, and is restored on the next start.

The panes and their shortcuts:

| Linux/Windows | macOS         | Pane      | Purpose                                           |
| ------------- | ------------- | --------- | ------------------------------------------------- |
| `Alt+L`       | `Cmd+Shift+L` | Log       | Runtime events, tool calls, warnings, errors      |
| `Alt+P`       | `Cmd+Shift+P` | Profile   | Live agent-loop profiler output                   |
| `Alt+I`       | `Cmd+Shift+I` | TASKS     | Session tasks and status                          |
| `Alt+M`       | `Cmd+Shift+M` | Memory    | Project/user memory and structured-memory summary |
| `Alt+S`       | -             | Sessions  | Session sidebar                                   |
| `Alt+A`       | -             | Active agents | Running sub-agents                            |
| `Alt+T`       | -             | Teams     | Agent teams                                       |

The macOS View menu holds the four panel toggles. `Alt+O` toggles autopilot.

## Window and layout reset

The window geometry and the dock layout are plain JSON files in the codemason
config directory (`~/.config/codemason/` on Linux; the platform config
directory elsewhere):

| File                | Holds                                              |
| ------------------- | -------------------------------------------------- |
| `gui-window.json`   | Window size, position and maximized state          |
| `gui-layout.json`   | Dock layout and where each hidden pane was         |
| `gui-keys.json`     | Your overrides of the keymap commands (optional)   |

To reset the window, quit codemason and delete `gui-window.json`: the next start
opens maximized again. To reset the layout, delete `gui-layout.json`: the next
start uses the default arrangement. A file that cannot be read is replaced by the
defaults, and a window saved off screen (for example a disconnected monitor)
reopens maximized.

---

## Common keybindings

| Linux/Windows                   | macOS                 | Action                                      |
| ------------------------------- | --------------------- | ------------------------------------------- |
| `Enter`                         | `Enter`               | Send prompt                                 |
| `Shift+Enter` / `Alt+Enter`     | `Shift+Enter`         | New line in prompt                          |
| `Esc`                           | `Esc`                 | Cancel current agent operation              |
| `Ctrl+D`                        | `Cmd+Q`               | Quit codemason                              |
| `Ctrl+C`                        | `Cmd+C`               | Copy selection / arm quit                   |
| `Alt+L`                         | `Cmd+Shift+L`         | Toggle Log pane                             |
| `Alt+P`                         | `Cmd+Shift+P`         | Toggle Profile pane                         |
| `Alt+I`                         | `Cmd+Shift+I`         | Toggle TASKS pane                           |
| `Alt+M`                         | `Cmd+Shift+M`         | Toggle Memory pane                          |
| `Alt+Q`                         | `Cmd+Alt+Q`           | Open the queue-control menu (Next/Stop/Clear/Show) |
| `Alt+O`                         | -                     | Toggle autopilot mode                       |
| `Alt+S`                         | -                     | Show/focus/hide the Sessions pane           |
| `Ctrl+N`                        | -                     | New session                                 |
| `Ctrl+Up` / `Ctrl+Down`         | -                     | Previous / next session                     |
| `Alt+R`                         | -                     | Most recently focused session               |
| `Alt+V`                         | -                     | Paste image from clipboard                  |
| `Alt+Y`                         | -                     | Toggle YOLO mode on/off                     |
| `Tab`                           | `Tab`                 | Complete the highlighted slash command      |
| `@`                             | `@`                   | Open file mention picker                    |
| `/`                             | `/`                   | Open slash-command menu                     |
| `?`                             | `?`                   | Show keybindings help (when prompt is empty)|

The `-` rows are not in the GUI keymap: they are the app-core chords sent as
typed. Their keymap-bound neighbours can be rebound in `gui-keys.json`; the
`?` panel lists every chord the window understands.

---

## Next steps

- Read the full `QUICKSTART.md` for CLI, server, and configuration options.
- Browse the per-command howtos in `docs/howtos/slashcommands/` (INDEX.md lists every command).
- See `docs/custom-agents.md` to create your own agent profiles.
- See `docs/howtos/teams.md` to coordinate multi-agent teams.
- Run `cdmn --help` for a complete list of command-line options.
