# OCGAP: Codemason vs opencode v2 Feature Gap Analysis

Date: 2026-11-18

## Scope and sources

- **Codemason**: branch `fix/bug-batch` at `454bfb2`. Evidence is `SPEC.md`, `QUICKSTART.md` and `cdmn --help`. "Not found" means the feature has no entry in those three sources; the source tree was only spot-checked.
- **opencode v2**: the 2.x line of `anomalyco/opencode`, latest release v2.3.0 (2026-11-13). Evidence is the live documentation at `https://opencode.ai/docs/`, the v1 migration guide (`/docs/migrate-v1/`) and the changelog. Items the docs do not state are marked UNVERIFIED. The Console SSO/SCIM/budget pages, the theme and keybind pages, the GUI/RPC plugin pages and most of the API reference were not read.

Legend: **Yes** = present, **Partial** = present with a narrower scope, **No** = not found, **n/a** = does not apply.

## Summary

Codemason is ahead on multi-agent coordination, knowledge systems (code index, memory, specs, research), sandboxed execution and forge integration. opencode v2 is ahead on client reach (terminal UI, web, pairing, shared service), session undo/redo, the plugin and SDK surface, the size of the HTTP API and organisation-managed configuration.

### Top gaps in codemason (opencode v2 has, codemason does not)

1. Closed 2026-11-18: `/undo` now restores files and `/redo` exists. Still
   missing: revert to an arbitrary earlier message.
2. A terminal UI. Codemason removed its TUI; the choices are the egui GUI or plain stdin/stdout.
3. Web UI, remote pairing (`opencode pair`, QR/one-time link) and one shared background service for all clients.
4. A broad HTTP API (173 operations: PTY, worktree, VCS, permission, usage, forms) with network and embedded SDKs. Codemason serves about 19 routes.
5. Code Mode (`execute`): the model writes JavaScript that calls tools, and MCP tools route through it by default.

### Top gaps in opencode v2 (codemason has, opencode v2 does not)

1. Teams, swarm, blueprints, mailbox messaging and plan approval.
2. Code index (tree-sitter + Tantivy) with a semantic code graph.
3. Persistent memory with FTS5 recall, decay and a knowledge graph.
4. Sandboxed execution backends (`docker`, `podman`, `remote`) and an LLM security analyzer. opencode v2 documents no sandbox.
5. Built-in GitHub and GitLab issue/PR/MR/pipeline tools and PR Auto-fix. opencode discontinued its GitHub Action in v2.2.0.

## 1. Interfaces

| Feature | Codemason | opencode v2 | Gap |
| --- | --- | --- | --- |
| Terminal UI | No (removed; `--no-tui` is plain stdin/stdout) | Yes (tabs, command palette, leader key) | Codemason |
| Minimal terminal client | Partial (`--no-tui`) | Yes (`opencode mini`) | Codemason |
| Desktop GUI | Yes (egui, dockable panes, default frontend) | Yes (beta) | None |
| Web UI | No | Yes (served by the shared server) | Codemason |
| Remote pairing | No | Yes (`opencode pair`, 5-minute link/QR, `--remote` tunnel) | Codemason |
| Shared background service | No | Yes (`opencode service ...`, one server for all clients) | Codemason |
| Headless run | Yes (`cdmn run`, `--dry-run --json`) | Yes (`opencode run`, `--format json`) | None |
| HTTP server | Yes (`cdmn serve`, about 19 routes, bearer token) | Yes (173 operations, Basic auth) | Codemason (breadth) |
| OpenAPI document | Yes (3.1, `/openapi.json`, `cdmn openapi`) | Yes (3.1) | None |
| OpenAI-compatible endpoint | Yes (`/v1/models`, `/v1/chat/completions`) | No (not documented) | opencode |
| ACP server | Yes (`cdmn acp-server`; Zed, VS Code, JetBrains) | Yes (`opencode acp`; Zed documented) | None |
| ACP client (drive other agents) | Yes (spec 19E.1) | No (not documented) | opencode |
| Shell mode (`!` prefix) | Yes | Yes | None |
| `@file#lines` references in prompt | Partial (`@` opens a file picker; no content expansion, no `#lines`) | Yes | Codemason |
| Input queue while agent runs | Yes (`/queue`) | Yes (queue and steer) | Partial: no steer in codemason |
| External editor for prompt | No | Yes (`/editor`) | Codemason |
| Side question without derailing (`/btw`) | No | Yes | Codemason |
| UI internationalisation | Yes (`/i18n`) | UNVERIFIED (changelog is localised) | Unknown |

## 2. Agents and orchestration

| Feature | Codemason | opencode v2 | Gap |
| --- | --- | --- | --- |
| Built-in agents | 15 visible (`ask`, `general`, `build`, `plan`, `explore`, language and role specialists) plus `title`, `summary` | `build`, `plan`, `general`, `explore` plus `compaction`, `title`, `summary` | opencode (count) |
| Custom agents | Yes (Markdown/OASF in `.codemason/agents/`) | Yes (`.opencode/agents/*.md` or config) | None |
| Per-agent model and permissions | Yes | Yes (plus `#variant`, `steps`, `color`) | None |
| Sub-agents | Yes (`new_agent`, `wait_agents`, `cancel_agent`, `list_agents`) | Yes (`subagent`, always background, depth limit) | None |
| Detached background agent | Yes (`/spawn`) | Yes (background subagents) | None |
| Teams (named teammates, mailbox, shared tasks) | Yes (`team_*`, 20 tools) | No | opencode |
| Team blueprints | Yes | No | opencode |
| Plan approval workflow | Yes (`team_submit_plan` / `team_approve_plan`) | No | opencode |
| Swarm decomposition | Yes (`/swarm`) | No | opencode |
| Autopilot / autonomous loop | Yes (`/autopilot`) | Partial (`/goal`) | None |
| Goal with reviewer confirmation | Partial (`/loop` goal-driven loop; `/goal` is a stub that does not persist the goal) | Yes (`/goal` runs until a reviewer confirms) | Codemason |
| Plan mode | Yes (`EnterPlanMode` / `ExitPlanMode`, `plan` agent) | Yes (`plan` agent) | None |
| Durable initiatives (milestones) | Yes (`initiative`) | No | opencode |
| Agent suspend / resume / kill UI | Yes (Agents window) | UNVERIFIED | Unknown |

## 3. Built-in tools

| Feature | Codemason | opencode v2 | Gap |
| --- | --- | --- | --- |
| Read / write / edit / glob / grep | Yes | Yes | None |
| Multi-edit, move, copy, mkdir, rm, diff | Yes | No (shell only) | opencode |
| Patch | Yes (`patch`, `apply_patch`) | Partial (`patch` on supported GPT models) | opencode |
| Shell | Yes (`Bash`, persistent state, `bash_reset`) | Yes (`shell`, background option) | None |
| Background shell tasks | Yes (`bg`) | Yes | None |
| Web fetch / search | Yes (`webfetch`, `websearch`, `http_request`, `mf_*` crawl/search) | Yes (`webfetch`, `websearch`; 6 search providers) | None |
| Ask user | Yes (`AskUserQuestion`) | Yes (`question`) | None |
| Task list | Yes (`TaskCreate` etc., on by default) | Partial (todo off by default, `experimental.todo`) | opencode |
| Skill tool | Yes (`Skill`) | Yes (`skill`) | None |
| Code Mode (script that calls tools) | No | Yes (`execute`) | Codemason |
| Code index tools | Yes (`codeindex_*`, 10 tools) | No | opencode |
| Memory tools | Yes (`memory_*`, `conversation_search`, `session_search`) | No | opencode |
| PDF read/write | Yes (`pdf_read`, `pdf_write`) | Partial (PDF as attachment) | opencode |
| GitHub / GitLab tools | Yes | No | opencode |
| Image generation | No | Yes (experimental) | Codemason |
| Browser control | Partial (`mf_screenshot`, engine-gated) | Yes (desktop `browser` namespace) | Codemason |
| Worktree tools callable by the model | No | Yes (`worktree_create/list/remove`) | Codemason |
| LSP | No | No (removed in v2) | None |
| Custom tools | Partial (plugins, MCP) | Yes (plugin `tool.transform`, MCP) | None |

## 4. Models and providers

| Feature | Codemason | opencode v2 | Gap |
| --- | --- | --- | --- |
| Provider mechanism | 14 compiled-in provider IDs | models.dev catalogue plus native, npm and AI SDK packages | Codemason (breadth) |
| OpenAI-compatible generic endpoint | Yes (`generic_openai`) | Yes | None |
| Anthropic-compatible generic endpoint | No | Yes | Codemason |
| Google Vertex | No (not in provider table) | Yes | Codemason |
| Local models | Yes (Ollama) | Yes (Ollama, LM Studio, vLLM) | Partial: via `generic_openai` |
| Copilot | Yes | Yes (device OAuth, "Auto" routing) | None |
| ChatGPT Plus/Pro sign-in | No | Yes | Codemason |
| Claude Code as a provider | No | Yes (Claude Agent provider, v2.2.3) | Codemason |
| Model router | Yes (`router`, 15-dimension classifier) | No | opencode |
| Provider health checks | Yes | UNVERIFIED | Unknown |
| Reasoning levels | Yes (\`/thinking auto | off | low |
| Hosted model subscription | No | Yes (OpenCode Console / Go plans) | n/a (commercial) |
| Auth CLI | Partial (`cdmn auth <provider> <key>`) | Yes (\`auth list | login |
| Cache warming | No | Yes (`warming`) | Codemason |

## 5. Configuration

| Feature | Codemason | opencode v2 | Gap |
| --- | --- | --- | --- |
| Config format | `codemason.json`, compatible with opencode v1 `opencode.json` | `opencode.json(c)`, v2 key names | See note |
| Global + project layering | Yes | Yes (walks up parent directories) | None |
| Untrusted project overlay (privileged keys stripped) | Yes | No (not documented) | opencode |
| System / MDM managed config | No | Yes (`/etc/opencode`, macOS MDM) | Codemason |
| Organisation-delivered config | No | Yes (Console workspace) | Codemason |
| Deny-only hard policies | No | Yes (`policies`) | Codemason |
| `{env:VAR}` / `{file:path}` substitution | No | Yes | Codemason |
| `AGENTS.md` instructions | Yes | Yes (walk-up, lazy nested loading) | None |
| `CLAUDE.md` fallback | Yes (`AGENT_FILE_NAMES`: `AGENTS.md`, `CLAUDE.md`, `.codemason.md`, `INSTRUCTIONS.md`) | Yes | None |
| Instructions from globs and URLs | No (`instructions` key holds plain strings and has no consumer) | Yes (`instructions`) | Codemason |
| External references (dirs / git repos) | No | Yes (`references`) | Codemason |
| Config backup / restore | Yes (`/config save`, `/config list`) | No | opencode |
| Config inspection | Yes (`/config show`, `cdmn config`) | Yes (`opencode debug config`) | None |

Note: opencode v2 renamed config keys (`agent` to `agents`, `provider` to `providers`, `permission` to ordered `permissions`, `command` to `commands`, `plugin` to `plugins`). Codemason's stated compatibility is with the v1 shape, so a v2 `opencode.json` is not a drop-in file.

## 6. Extensibility

| Feature | Codemason | opencode v2 | Gap |
| --- | --- | --- | --- |
| Plugins | Yes (sandboxed JS runtime; Codex and Claude dialects; marketplaces) | Yes (own API: server, TUI, GUI, RPC parts) | Different models |
| Plugin UI extension | No | Yes (TUI and GUI plugin parts) | Codemason |
| Plugin sandboxing | Yes | No (not documented) | opencode |
| Hooks | Yes (`PreToolUse`, session start/end, turn start/end, error, permission denied, compaction, notification) | Yes (session, model request, HTTP request/response, permission, tool before/after, shell) | Partial: no model/HTTP request hooks in codemason |
| MCP stdio + Streamable HTTP | Yes | Yes | None |
| MCP OAuth | No (connector `Oauth` shape prints a hint; no flow) | Yes (PKCE, CIMD, DCR) | Codemason |
| MCP elicitation, pooling, prompts, resources | No (client sends `initialize`, `tools/list`, `tools/call` only) | Yes | Codemason |
| Connectors catalogue | Yes (`/connectors`, marketplaces) | No | opencode |
| Skills | Yes (bundled and custom, `allowed-tools`) | Yes (`SKILL.md`, URL catalogues) | None |
| Custom slash commands | Yes (`.codemason/commands/*.md`, `$ARGUMENTS`, `$1`..`$9`; no per-command agent/model) | Yes (`.opencode/commands/*.md`, `$ARGUMENTS`) | Partial: no per-command agent/model |
| Formatters after edit | Yes (opt-in `formatter` section; 4 built in plus custom) | Yes (about 25 built in, on by default) | Partial: fewer built-ins, off by default |
| Themes | No | Yes | Codemason |
| Keybind customisation | Yes (`gui-keys.json`) | Yes (`cli.json`) | None |
| SDK | Partial (generated TypeScript client typings) | Yes (`@opencode/sdk` embedded, `@opencode/client`) | Codemason |

## 7. Sessions

| Feature | Codemason | opencode v2 | Gap |
| --- | --- | --- | --- |
| Persistence | Yes (SQLite) | Yes (local database) | None |
| Multiple concurrent sessions | Yes (background sessions keep running) | Yes (tabs) | None |
| Projects | Yes (`/project`, stored records) | Partial (per-directory) | opencode |
| Resume / list | Yes | Yes | None |
| Fork | Yes (`/session fork`) | Yes | None |
| Pin, archive, unread, sections | Yes | Yes (archive, unread, move) | None |
| Undo / redo / revert to message | Yes (`/undo`, `/redo`, shadow-git snapshots per turn; no revert to an arbitrary message, redo stack in memory only) | Yes (`/undo`, `/redo`, git-object snapshots) | Partial: no revert-to-message picker |
| Per-turn diff view | No | Yes (`/diff`) | Codemason |
| Export / import | Yes (`export-archive`, `import`) | Yes (`export --sanitize`, `import`) | Partial: no sanitize |
| Compaction | Yes (`/compact`, auto) | Yes (auto, background, native/summary) | None |
| Usage and cost statistics | Yes (`/stats`, `cdmn stats`; `/cost` for the current session) | Yes (`opencode stats --days --models --cost`) | None |
| Message retention / prune | Yes (`/db prune`, `storage.retention`) | UNVERIFIED | Unknown |
| Search across sessions | Yes (`session_search`, sidebar search) | UNVERIFIED | Unknown |
| Share links | No | No (absent from v2 docs) | None |

## 8. Safety

| Feature | Codemason | opencode v2 | Gap |
| --- | --- | --- | --- |
| allow / ask / deny rules | Yes | Yes (ordered, last match wins) | None |
| Default posture | Ask-based; YOLO opt-in | Allow-all except `.env` reads | opencode |
| Shell command analysis | Yes (7-layer bash safety) | Yes (scanner; refuses unanalysable syntax; PowerShell) | None |
| File path guards | Yes | Partial (`external_directory` now allow) | opencode |
| Secret redaction | Yes | Not documented | opencode |
| Sandboxed execution | Yes (`docker`, `podman`, `remote` backends) | No | opencode |
| LLM security analyzer | Yes (`/security`) | No | opencode |
| Git worktree per session | Yes (`/session new --branch`) | Yes (`run -w`, `worktree.directory`) | None |
| Proxy / custom CA / mTLS | No (no explicit proxy, CA or client-cert configuration on any HTTP client) | Yes | Codemason |
| Permission prompt timeout | Yes (120 s, denies) | UNVERIFIED | Unknown |

## 9. Knowledge, automation and integrations

| Feature | Codemason | opencode v2 | Gap |
| --- | --- | --- | --- |
| Code index and semantic graph | Yes | No | opencode |
| Persistent memory | Yes | No | opencode |
| Spec-driven development (`/spec`) | Yes | No | opencode |
| Research system (`/research`, hyperresearch) | Yes | No | opencode |
| Project scaffolding (`/new`) | Yes | No | opencode |
| Scheduling | Yes (`/cron`) | Yes (`/schedule`, experimental) | None |
| Webhook-triggered automations | Yes (`/automation`, `POST /auto/{id}`) | No | opencode |
| GitHub integration | Yes (tools, Auto-fix) | No (Action discontinued in v2.2.0) | opencode |
| GitLab integration | Yes (issues, MRs, pipelines) | No | opencode |
| OpenTelemetry export | Yes | UNVERIFIED | Unknown |
| Benchmarks (`/bench`) | Yes | No | opencode |
| Attachments | Yes (images, PDF via tool) | Yes (PNG, JPEG, GIF, WebP, PDF, text; 20 MiB) | None |
| Voice | No | Partial (experimental API) | Codemason |
| Self-update | No (`/update` reserved) | Yes (`opencode upgrade`) | Codemason |

## Codemason gaps ranked

Must (most users will notice):

1. Undo/redo and revert-to-message. `codemason-storage::snapshot` already takes and restores file snapshots for loop capture, so the storage half exists. – please implement. **Done 2026-11-18** (branch `feat/ocgap`)
2. User-defined slash commands from `.codemason/commands/*.md`. – please implement. **Done 2026-11-18** (branch `feat/ocgap`)
3. Usage and cost statistics across sessions. - Please implement. **Done 2026-11-18** (branch `feat/ocgap`)
4. Self-update (`/update` is reserved and unimplemented). - Not required
5. Formatters run after edits. Please implement. **Done 2026-11-18** (branch `feat/ocgap`)

Later (larger or narrower):

1. Web UI, pairing and a shared background service.
2. Terminal UI.
3. Wider HTTP API and an SDK.
4. Managed/organisation config, deny-only policies, `{env:}`/`{file:}` substitution.
5. Code Mode, provider breadth (Vertex, Anthropic-compatible, ChatGPT sign-in).

## Open questions

- opencode v2 items marked UNVERIFIED need a second pass against the pages that were not read (API reference, themes, keybinds, Console SSO/SCIM/budgets).
- Resolved 2026-11-18: the rows previously marked "Not found" were checked against source and updated above.
-
