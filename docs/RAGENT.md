# References to "ragent"

Case-insensitive search of all tracked files (`git grep -n -i ragent`), plus
untracked files outside `target/` and `.git/`. 13 matching lines in 6 files.
No source code (`.rs`) or `Cargo.toml` contains "ragent". Git-ignored files
named `*ragent*` are listed further down.

## .gitignore (2)

| Line | Context |
| --- | --- |
| 90 | Comment: `.ragent/` holds the Tantivy index cache the agent's code index writes |
| 92 | Ignore rule: `.ragent/` |

## CHANGELOG.md (6)

| Line | Context |
| --- | --- |
| 42 | Changed entry: `.gitignore` covers `.ragent/` (Tantivy index cache) |
| 577 | Heading: "project renamed from ragent to codemason; binary is `cdmn`" |
| 579 | Crates renamed `ragent-*` -> `codemason-*` |
| 583 | Env vars are `CODEMASON_*` (were `RAGENT_*`) |
| 584 | State dir is `.codemason/` (was `.ragent/`); `ragent_info` tool renamed |
| 590 | Mentions the fork from ragent and the new names |

## README.md (1)

| Line | Context |
| --- | --- |
| 14 | "codemason started as a copy of [ragent](https://github.com/thawkins/ragent)." |

## SPEC.md (1)

| Line | Context |
| --- | --- |
| 204 | "Codemason was forked from ragent: crates are named `codemason-*`, ..." |

## docs/OCGAPPLAN.md (1)

| Line | Context |
| --- | --- |
| 39 | Plan item 0.1: replace the "RAGENT" logo with a codemason wordmark |

## docs/OCUIGAP.md (2)

| Line | Context |
| --- | --- |
| 54 | Welcome banner that spells "RAGENT" |
| 150 | Empty/welcome state: ASCII logo spells "RAGENT" (`logo.rs:9-14`) |

## Untracked files with "ragent" in the name (main checkout)

Search: `find ~/Projects/codemason -iname '*ragent*'`, skipping `target/`,
`.git/` and `.claude/worktrees/`. 880 paths, all git-ignored. No source in
`main` refers to these names, and every file except `.ragent/` is dated
2026-11-13 or earlier, before the rename (commit `924834f`, 2026-11-14).

### `.codemason/` (project root)

| Path | Size | Modified |
| --- | --- | --- |
| `.codemason/ragent.db` | 0 B | 2026-08-09 |
| `.codemason/ragent.json` | 11.8 KB | 2026-11-07 |
| `.codemason/ragent.json.lock` | 0 B | 2026-07-17 |
| `.codemason/teams/ragent-review/` | team directory | 2026-10-29 |

Files inside `.codemason/` that mention "ragent" in their contents: 48 in
`research_vault/`, 4 in `teams/`, 2 in `plugins/`, and `ragent.json`.

### Per-crate `.codemason/` folders (left behind by test runs)

- `crates/codemason-config/.codemason/ragent.json`
- `crates/codemason-connectors/.codemason/ragent.json`
- `crates/codemason-plugins/.codemason/ragent.json`
- `crates/codemason-server/.codemason/ragent.json`
- `crates/codemason-tools-vcs/.codemason/ragent.json`

### Other locations

| Location | Count | What |
| --- | --- | --- |
| `.ragent/` | 1 dir | Holds `codeindex/`; modified 2026-11-21 |
| `log/subagents/ragent-*.md` | 46 | Old subagent logs |
| `crates/codemason-tools-core/target/temp/ragent-shell/` | 430 | `ragent_cmd_test-timeout-session_*.sh` test scratch |
| `crates/codemason-agent/target/temp/ragent-shell/` | ~390 | `ragent_shell_*.state` test scratch |
| `crates/codemason-storage/target/temp/t*_triggers/` | 3 | `ragent.db`, `-wal`, `-shm` |

## Notes

- The `logo.rs` file cited in `docs/OCUIGAP.md:150` no longer exists under
  `crates/`, so that reference is stale.
- Line 92 of `.gitignore` still ignores `.ragent/`, which CHANGELOG line 42
  describes as the live code-index cache directory, while CHANGELOG line 584
  says the state directory moved to `.codemason/`.
