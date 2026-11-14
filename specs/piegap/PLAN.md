# Implementation Plan — Pie Feature Gap

## Overview

Each task implements one gap feature as a standalone, independently
selectable module. Tasks are ordered by architectural dependency: foundational
shared types (trigger envelope, hook types) land first, then features that
build on them, then fully independent features.

**FR-009 (LSP Integration)** and **FR-010 (Compiled Extension Registry)** are
marked "NOT Required" in the spec. No implementation tasks are defined for
them. The original task IDs T-013, T-014, and T-015 are retired and skipped to
preserve the numbering of the remaining tasks.

**Effort**: S = small (≤1 day), M = medium (1–3 days), L = large (3–7 days)
**Priority**: Critical, High, Medium, Low

## Tasks

| ID    | Title                                                                                    | Requirement                    | Effort | Priority | Status    | Dependencies |
| ----- | ---------------------------------------------------------------------------------------- | ------------------------------ | ------ | -------- | --------- | ------------ |
| T-001 | Trigger envelope types + trigger runtime (dedup, cycle suppression)                      | FR-002, FR-003                 | L      | High     | completed | pending      |
| T-002 | Dynamic trigger rules — natural-language polling + sub-agent action                     | FR-002                         | L      | High     | completed | pending      |
| T-003 | MCP notification push-event adapter (inject_summary / inject_and_run)                    | FR-003                         | L      | High     | completed | pending      |
| T-004 | `/triggers` slash commands (list, enable, disable, remove, status)                     | FR-002, FR-003                 | M      | High     | completed | pending      |
| T-005 | Stateful loop cron mode +`<loop-state>`/`<inbox>` tag protocol                       | FR-004                         | L      | Medium   | completed | pending      |
| T-006 | Global triage inbox JSONL store +`/inbox` slash commands                               | FR-004                         | M      | Medium   | completed | pending      |
| T-007 | Lifecycle hooks system —`hooks.json` loader + command/webhook runner                  | FR-005                         | L      | Medium   | completed | pending      |
| T-008 | Hook event listener wiring (agent_start/end, turn_start/end, tool_start/end, compaction) | FR-005                         | M      | Medium   | completed | pending      |
| T-009 | Portable session archive export (manifest + transcript + sidecars + SHA-256)             | FR-006                         | L      | Low      | completed | pending      |
| T-010 | Portable session archive import (verify checksums + optional automation activation)      | FR-006                         | L      | Low      | completed | pending      |
| T-011 | `/bug-report` slash command — diagnostic dump + redaction (output to `log/` folder) | FR-007                         | M      | Low      | completed | pending      |
| T-012 | Reusable prompt templates — loader +`/template` slash command                         | FR-008                         | M      | Low      | completed | pending      |
| T-016 | Goal-based autonomous stop hook — evaluator model call +`/goal` commands              | FR-011                         | L      | Medium   | completed | pending      |
| T-017 | OpenAI Responses API provider — reasoning replay + 409 retry + cache-write usage        | FR-012                         | L      | Low      | completed | pending      |
| T-018 | Browser-based web UI — loopback HTTP server + SSE + browser-served HTML/JS              | FR-013                         | L      | Low      | completed | pending      |
| T-019 | `/undo` slash command — remove last user/assistant turn pair                          | FR-014                         | S      | Medium   | completed | pending      |
| T-020 | Session naming —`/name` slash command + persisted display name                        | FR-015                         | S      | Low      | completed | pending      |
| T-021 | Feature-gate configuration blocks in`codemason.json` for all gap features                 | FR-016, FR-018                 | M      | High     | completed | pending      |
| T-022 | Integration tests — verify standalone compilation with each feature disabled            | FR-001, FR-016, FR-017, FR-019 | M      | High     | completed | pending      |

## Requirement Coverage

| Requirement | Covered by tasks           |
| ----------- | -------------------------- |
| FR-001      | T-022                      |
| FR-002      | T-001, T-002, T-004, T-021 |
| FR-003      | T-001, T-003, T-004, T-021 |
| FR-004      | T-005, T-006, T-021        |
| FR-005      | T-007, T-008, T-021        |
| FR-006      | T-009, T-010               |
| FR-007      | T-011                      |
| FR-008      | T-012, T-021               |
| FR-009      | — (NOT required)          |
| FR-010      | — (NOT required)          |
| FR-011      | T-016, T-021               |
| FR-012      | T-017                      |
| FR-013      | T-018                      |
| FR-014      | T-019                      |
| FR-015      | T-020                      |
| FR-016      | T-021, T-022               |
| FR-017      | T-022                      |
| FR-018      | T-021                      |
| FR-019      | T-022                      |

## Architecture Notes

### Crate Placement

| Feature                    | Target crate(s)                                                                                  |
| -------------------------- | ------------------------------------------------------------------------------------------------ |
| Trigger envelope + runtime | `codemason-types` (types), `codemason-agent` (runtime)                                             |
| Dynamic triggers           | `codemason-agent` (logic), `codemason-tui` (slash commands)                                        |
| MCP notification hooks     | `codemason-agent` (hook abstraction), `codemason-tools-extended` (MCP adapter)                     |
| Stateful loops + inbox     | `codemason-agent` (loop logic), `codemason-storage` (inbox JSONL), `codemason-tui` (slash commands) |
| Lifecycle hooks            | `codemason-agent` (hook runner), `codemason-config` (config types), `codemason-tui` (wiring)        |
| Session archive            | `codemason-agent` (archive logic), `codemason-storage` (sidecar read/write)                        |
| Bug report                 | `codemason-tui` (slash command), `codemason-agent` (diagnostic data)                               |
| Prompt templates           | `codemason-agent` (loader), `codemason-tui` (slash command)                                        |
| Goal stop hook             | `codemason-agent` (evaluator + hook), `codemason-tui` (slash commands)                             |
| OpenAI Responses provider  | `codemason-llm` (new provider)                                                                    |
| Web UI                     | `codemason-tui` (web module) or new `codemason-web` crate                                          |
| `/undo`                  | `codemason-agent` (session history), `codemason-tui` (slash command)                               |
| Session naming             | `codemason-storage` (metadata), `codemason-tui` (slash command)                                    |

### Feature-Gate Strategy

Each gap feature shall be gated behind a `serde(default)` boolean in `Config`
so it can be independently enabled/disabled in `codemason.json`. Features that
require background tasks (triggers, hooks) shall no-op cleanly when disabled.
The existing `yolo` toggle pattern shall be followed.

### Dependency Graph

```
T-001 (trigger types + runtime)
├── T-002 (dynamic triggers)
├── T-003 (MCP notification hooks)
│   └── T-004 (/triggers commands)
└── T-005 (stateful loops)
    └── T-006 (inbox)

T-007 (lifecycle hooks loader) → T-008 (event wiring)

Standalone (no deps):
  T-009, T-010 (session archive)
  T-011 (bug report)
  T-012 (prompt templates)
  T-016 (goal stop hook)
  T-017 (OpenAI Responses provider)
  T-018 (web UI)
  T-019 (/undo)
  T-020 (session naming)

T-021 (config gates) — depends on all feature tasks
T-022 (integration tests) — depends on all
```
