# /theme

> "Switch the colour theme: /theme default|high-contrast, /theme shows the current one (/theme help)" (SlashCommandDef, crates/codemason-app-core/src/app/state.rs).

## Overview

`/theme` switches the window's colour theme while it is running. Two themes
exist: `default` (the standard dark theme) and `high-contrast` (brighter text
and white outlines around every widget).

The switch lasts until codemason exits. To start in a theme, set the `theme`
key in `codemason.json`:

```json
{ "theme": "high-contrast" }
```

The handler is `App::slash_theme` in
`crates/codemason-app-core/src/app/slash.rs`; the palettes are in
`crates/codemason-gui/src/palette.rs` and `theme.rs`.

## Syntax

```
/theme
/theme default
/theme high-contrast
/theme help
```

## Options / Subcommands

| Form | Description |
|---|---|
| `/theme` | Show the current theme and the available ones |
| `/theme default` | Switch to the standard dark theme |
| `/theme high-contrast` | Switch to the high-contrast theme (`high_contrast` and `highcontrast` are accepted too) |
| `/theme help` | Show the help table |

## Notes

- An unknown name changes nothing and prints the usage line.
- `/theme` does not write the config file; only the `theme` key persists.
