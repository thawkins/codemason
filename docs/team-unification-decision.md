# Team runtime + tool unification decision (ANTIPAT M7.8)

Status: applied. The `codemason-team` crate has been folded into `codemason-agent`.

## Background

Historically the team subsystem lived in `crates/codemason-team/` and was
compiled into `codemason-agent` through 27 `#[path = "../../../codemason-team/..."]`
attributes. REMPLAN.md M3 (T3.3) moved the sources *natively* into
`crates/codemason-agent/src/team/` and `crates/codemason-agent/src/tool/team_*.rs`
to remove the `#[path]` cycle workaround, leaving `crates/codemason-team` as a
59-line pure re-export shim over `codemason-agent`.

By M7.8 the shim had zero definitions of its own, `codemason-tui` already
depended on `codemason-agent` directly, and no other crate used it. The
remaining consumers were:

- `crates/codemason-tui` (15 files, all using `codemason_team::team::*`), and
- the 16 integration test files under `crates/codemason-team/tests/`.

## Decision

Folding beats documenting the split here. The crate was a pass-through with
no independent value: keeping it preserved a second name for the same types,
forced an extra path dependency, and let the stale
`scripts/check-team-duplication.sh` guard (which asserted the crate *was* the
canonical source) contradict the actual post-M3 layout.

## What changed

1. `crates/codemason-team/` was deleted.
2. `crates/codemason-tui` dropped the `codemason-team` dependency and now imports
   `codemason_agent::team::*` / `codemason_agent::tool::*` directly.
3. The 16 team integration tests moved to `crates/codemason-agent/tests/`, with
   `codemason_team::team::` rewritten to `codemason_agent::team::`; the shared
   `tests/support/mod.rs` helper moved to `crates/codemason-agent/tests/team_support/`.
   `codemason-agent` gained `codemason-tools-core` as a dev-dependency for the
   tests that exercise `BashTool` / `CommandCatalog`.
4. `scripts/check-team-duplication.sh` was rewritten to assert the *current*
   invariant: the team runtime lives in `codemason-agent/src/team/`, the team
   tools live in `codemason-agent/src/tool/team_*.rs`, and no `codemason-team`
   crate or `#[path]` references to it are re-introduced. It is wired into
   `ci.yml` and `pre-flight.sh` and ships a `--self-test`.

`crates/codemason-agent/src/team/mod.rs` is the single canonical home for the
team runtime.
