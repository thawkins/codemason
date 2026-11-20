# /theme

> "Switch the colour theme: /theme opens the picker, /theme <name>|system|dark|light switches (/theme help)" (SlashCommandDef, crates/codemason-app-core/src/app/state.rs).

## Overview

`/theme` switches the window's colour theme while it is running.

| Theme | Description |
|---|---|
| `system` | Follows the operating system: `codemason-dark` when it is dark, `codemason-light` when it is light. This is the default. |
| `codemason-dark` | The standard dark theme (alias `dark`) |
| `codemason-light` | The standard light theme (alias `light`) |
| `high-contrast` | White text and white outlines on black |
| `catppuccin-mocha`, `gruvbox-dark`, `nord`, `tokyo-night` | Ports of those palettes |
| `document-light`, `document-dark` | Material Design 3 colour schemes from one blue seed, made for the window's word-processor layout |

Choosing a theme (`/theme <name>`, Enter in the picker, or the command
palette) saves it as the `theme` key in the user-global
`~/.config/codemason/codemason.json` (or the `--config` file), so the next start
opens in it. You can also set the key by hand:

```json
{ "theme": "nord" }
```

The handler is `App::slash_theme` in
`crates/codemason-app-core/src/app/slash.rs`. Themes are tokens
(`crates/codemason-app-core/src/theme_tokens.rs`); the built-in ones are the
JSON files in `crates/codemason-app-core/themes/`, and the window maps them to
colours in `crates/codemason-gui/src/palette.rs` and `theme.rs`.

Every theme also has Material Design 3 colour roles (`ThemeTokens::roles`). A
theme file may give them in an optional `m3` block (the `document-*` themes
do); otherwise they are derived from the other tokens. An `m3` block is
checked for contrast when the file is loaded: every `on_x` role against its
`x` role at 4.5:1 (including `inverse_on_surface` on `inverse_surface`), and
`outline` against `surface` and `inverse_primary` against `inverse_surface` at
3:1. A theme with its own `m3` block also restyles egui's built-in widgets
from those roles.

## Syntax

```
/theme
/theme list
/theme system
/theme dark
/theme light
/theme <name>
/theme help
```

## Options / Subcommands

| Form | Description |
|---|---|
| `/theme` | Open the theme picker. Up/Down (or a click) previews a theme in the window, Enter (Apply) keeps it, Esc (Cancel) restores the previous one |
| `/theme list` | Show the current theme and the available ones |
| `/theme system` | Follow the operating system (`default` is accepted too) |
| `/theme dark` | Switch to `codemason-dark` |
| `/theme light` | Switch to `codemason-light` |
| `/theme <name>` | Switch to any listed theme; the name is case-insensitive (`high_contrast` and `highcontrast` are accepted for `high-contrast`) |
| `/theme help` | Show the help table |

## Your own themes

A theme is a JSON file. Put it in `~/.config/codemason/themes/` (every
project) or `.codemason/themes/` (this project). A project theme replaces a
user theme of the same name, and either replaces a built-in one. Themes are
read when codemason starts.

Copy a built-in file such as
`crates/codemason-app-core/themes/codemason-dark.json` and change the colours.
Every token is required and every colour is `#rrggbb`:

| Token group | Tokens |
|---|---|
| `name`, `dark` | The name to select the theme by; `true` for light text on a dark background |
| `text` | `base`, `muted`, `subtle` |
| `bg` | `base`, `raised1`, `raised2`, `overlay` |
| `border` | `base`, `focus` |
| `accent` | One colour |
| `state` | `success`, `warning`, `error`, `info` |
| `diff` | `added`, `removed`, `context`, `added_bg`, `removed_bg`, `line_number` |
| `syntax` | `keyword`, `string`, `comment`, `function`, `type`, `number`, `operator`, `punctuation` |
| `markdown` | `heading`, `link`, `code`, `blockquote` |
| `categorical` | A list of eight colours |
| `credit` | Optional attribution text |

## Notes

- An unknown name changes nothing and lists the available themes.
- Theme names are trimmed and lowercased when a file is read, so `Nord` in a
  file replaces the built-in `nord`, and `/theme Ocean Mist` selects a theme
  named `ocean mist`. A user theme named `dark` or `light` wins over those
  aliases; `system` and `default` always mean `system`.
- Only the `theme` key is written; every other key in the file is kept. Esc in
  the picker saves nothing. A project `.codemason/codemason.json` that sets
  `theme` still overrides the saved value at startup.
- A theme file that does not parse, is larger than 64 KiB, or is not a regular
  file (a FIFO or a device), is skipped with a warning in the Log pane.
- A loaded theme whose text is hard to read is still used, with a warning in
  the Log pane: `text.base` and `text.muted` need a contrast of 4.5:1 on
  `bg.base`; `text.subtle`, `accent` and the `state` colours need 3:1.
- An unknown `theme` in `codemason.json` falls back to `system`, with a warning
  in the Log pane.
