#!/usr/bin/env bash
# CI guard: the VCS tool layer must have one implementation of each shared
# helper, across crates AND across the GitHub/GitLab split inside
# `codemason-tools-vcs`.
#
# Checks:
#
# 1. `codemason-agent` must not re-introduce local copies of the GitHub/GitLab
#    tool sources; they live canonically in `codemason-tools-vcs` and are
#    registered via the `ExtractedVcsToolAdapter`.
# 2. The git-argument guard (`reject_option_like`) must have a single shared
#    implementation (`codemason_types::guard::reject_option_like`), and
#    `codemason-plugins` must not re-derive the leading-dash check.
# 3. Intra-crate (ANTIPAT M7.9): the per-provider client/detection helpers
#    (`make_client`, `detect`, `detect_repo`, `detect_project`) must be
#    defined only in `github/helpers.rs` / `gitlab/helpers.rs`, not
#    re-derived in the individual `github_*.rs` / `gitlab_*.rs` modules.
#
# See `ANTIPAT.md` M3.9 / M7.9.
#
# Usage: bash scripts/check-vcs-duplication.sh [--self-test]
# Exit code: 0 = clean, 1 = a duplicate implementation was found.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

TOOLS_DUP_DIR="crates/codemason-agent/src/tool"
# RS-24: the whole plugins crate, not only add.rs.
PLUGIN_SRC="crates/codemason-plugins/src"
VCS_SRC="crates/codemason-tools-vcs/src"

# Helper functions that must have exactly one definition, in `helpers.rs`.
SHARED_HELPERS_RE='^(pub )?(async )?fn (make_client|detect|detect_repo|detect_project)\b'

run_checks() {
    local exit_code=0

    # ── 1. Cross-crate copies in codemason-agent ───────────────────────────────
    for f in "$TOOLS_DUP_DIR"/github_*.rs "$TOOLS_DUP_DIR"/gitlab_*.rs; do
        [ -e "$f" ] || continue
        echo "ERROR: duplicate VCS tool source file detected: $f"
        echo "       GitHub/GitLab tools live canonically in crates/codemason-tools-vcs/src/"
        echo "       and are registered into codemason-agent via the ExtractedVcsToolAdapter"
        echo "       in crates/codemason-agent/src/tool/mod.rs. Remove the local copy."
        exit_code=1
    done

    # ── 2. Single git-argument guard ────────────────────────────────────────
    dash_hits="$(grep -rlF --include='*.rs' "value.starts_with('-')" "$PLUGIN_SRC" || true)"
    if [ -n "$dash_hits" ]; then
        echo "ERROR: $dash_hits re-implements the leading-dash git-argument check."
        echo "       Delegate to codemason_types::guard::reject_option_like /"
        echo "       is_safe_operand (via codemason_tools_core::guard) instead"
        echo "       (SECTASKS MS-05 T-068)."
        exit_code=1
    fi

    if ! grep -q 'codemason_types::guard::reject_option_like' \
        crates/codemason-tools-vcs/src/git/mod.rs; then
        echo "ERROR: crates/codemason-tools-vcs/src/git/mod.rs no longer delegates to the"
        echo "       shared codemason_types::guard::reject_option_like (SECTASKS MS-05 T-068)."
        exit_code=1
    fi

    # ── 3. Intra-crate GitHub/GitLab helper duplication (ANTIPAT M7.9) ──────
    for provider in github gitlab; do
        helper_file="$VCS_SRC/$provider/helpers.rs"
        if [ ! -f "$helper_file" ]; then
            echo "ERROR: shared helper file missing: $helper_file"
            exit_code=1
            continue
        fi
        # Every other module in the provider directory must not define a
        # shared helper itself.
        for f in "$VCS_SRC/$provider"/*.rs; do
            [ -e "$f" ] || continue
            [ "$f" = "$helper_file" ] && continue
            [ "$(basename "$f")" = "mod.rs" ] && continue
            # Skip test-support files.
            case "$f" in
                */tests/*) continue ;;
            esac
            if grep -qE "$SHARED_HELPERS_RE" "$f"; then
                echo "ERROR: $f re-defines a shared $provider helper."
                echo "       make_client/detect/detect_repo/detect_project must live once"
                echo "       in $helper_file (ANTIPAT M3.9 / M7.9)."
                exit_code=1
            fi
        done
    done

    return "$exit_code"
}

if [ "${1:-}" = "--self-test" ]; then
    # Seed a re-derived GitHub helper in a real provider module; the guard must
    # fail. We append it to a temp file the guard will scan.
    seed="$VCS_SRC/github/__selftest_dupe.rs"
    printf '//! Self-test fixture for scripts/check-vcs-duplication.sh.\npub fn make_client() {}\n' > "$seed"
    set +e
    run_checks >/dev/null 2>&1
    rc=$?
    set -e
    rm -f "$seed"
    if [ "$rc" -eq 0 ]; then
        echo "ERROR: self-test failed - the guard passed a re-derived GitHub helper."
        exit 1
    fi

    # Second half: a re-introduced leading-dash check in codemason-plugins must be
    # caught in any file of the crate, not only add.rs.
    seed_plugin="crates/codemason-plugins/src/__selftest_add_dupe.rs"
    printf "fn f(value: &str) -> bool { value.starts_with('-') }\n" > "$seed_plugin"
    set +e
    run_checks >/dev/null 2>&1
    rc=$?
    set -e
    rm -f "$seed_plugin"
    if [ "$rc" -eq 0 ]; then
        echo "ERROR: self-test failed - a re-derived leading-dash check passed the guard."
        exit 1
    fi

    echo "OK: self-test - the guard fails a re-derived helper and a re-derived git-arg check."
    exit 0
fi

if run_checks; then
    echo "OK: VCS tools have no cross-crate copies and one shared helper/guard implementation."
    exit 0
fi
exit 1
