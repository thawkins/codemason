#!/usr/bin/env bash
# CI guard (OCGAPPLAN phase 1): fail if a production source names a colour by
# value or by ANSI hue outside the theme modules. Every colour must come from
# the theme tokens, so a theme switch restyles the whole window.
#
# Flagged in crates/codemason-gui/src and crates/codemason-app-core/src:
#   - egui literals: Color32::from_rgb/from_gray/from_rgba_*/from_black_alpha/
#     from_white_alpha, Color32::<NAMED> (TRANSPARENT is allowed), hex_color!
#   - literal or ANSI ThemeColor variants: Rgb(..), Indexed(..), Black, Red, ...
#
# Theme modules (allowed): crates/codemason-gui/src/palette.rs, which maps
# ThemeColor to the theme, and crates/codemason-app-core/src/styled.rs, which
# declares ThemeColor. Exempt one line with a trailing
# `// theme-literal-ok: <reason>` comment.
#
# Usage: bash scripts/check-theme-literals.sh [--self-test]
# Exit code: 0 = clean, 1 = a literal colour was found.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

EGUI='Color32::(from_(rgb|gray|rgba_premultiplied|rgba_unmultiplied|black_alpha|white_alpha)\b|[A-Z][A-Z_]+\b)|hex_color!'
ANSI='ThemeColor::(Rgb|Indexed|Black|Red|Green|Yellow|Blue|Magenta|Cyan|Gray|DarkGray|LightRed|LightGreen|LightYellow|LightBlue|LightMagenta|LightCyan|White)\b'

# Prints the offending lines of the given files, one per line.
scan() {
    grep -nHE "$EGUI|$ANSI" "$@" 2>/dev/null \
        | grep -v 'Color32::TRANSPARENT' \
        | grep -v 'theme-literal-ok:' || true
}

if [[ "${1:-}" == "--self-test" ]]; then
    mkdir -p "$ROOT/target/temp"
    seeded="$(mktemp --tmpdir="$ROOT/target/temp" theme-literal-XXXXXX.rs)"
    trap 'rm -f "$seeded"' EXIT
    printf '%s\n' \
        'let a = Color32::from_rgb(1, 2, 3);' \
        'let b = Color32::WHITE;' \
        'let c = ThemeColor::Yellow;' \
        'let d = Color32::TRANSPARENT;' \
        'let e = ThemeColor::Warning;' \
        'let f = Color32::BLACK; // theme-literal-ok: test' >"$seeded"
    found="$(scan "$seeded" | wc -l)"
    if [[ "$found" -ne 3 ]]; then
        echo "self-test FAILED: expected 3 seeded violations, found $found" >&2
        exit 1
    fi
    echo "self-test passed: 3 seeded violations found, 3 allowed lines ignored"
    exit 0
fi

mapfile -t files < <(
    find crates/codemason-gui/src crates/codemason-app-core/src -name '*.rs' \
        ! -path 'crates/codemason-gui/src/palette.rs' \
        ! -path 'crates/codemason-app-core/src/styled.rs' | sort
)
violations="$(scan "${files[@]}")"
if [[ -n "$violations" ]]; then
    echo "Literal colours outside the theme modules:" >&2
    echo "$violations" >&2
    echo >&2
    echo "Use a semantic ThemeColor / Palette field, or add the colour to the theme tokens." >&2
    exit 1
fi
echo "check-theme-literals: clean (${#files[@]} files)"
