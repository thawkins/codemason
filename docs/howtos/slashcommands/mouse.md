# /mouse

> Toggle mouse support: /mouse on | off | help

## Overview

`/mouse` sets the app's `mouse_enabled` flag and, when disabled, lists the
keyboard shortcuts that replace mouse interaction. The GUI window does not read
this flag: mouse input in the window is always on, so the command only changes
the state it reports.

## Syntax

```
/mouse              Show the current mouse state and usage
/mouse on           Enable mouse support
/mouse off          Disable mouse support (keyboard-only)
/mouse help         Print the help table
```

## Options / Subcommands

| Form | Description |
| ---- | ----------- |
| (bare) | Show the current state (enabled or disabled) plus the usage line. |
| `on` | Set `mouse_enabled = true` and confirm. |
| `off` | Set `mouse_enabled = false` and print the keyboard-only message with the shortcut list. |
| `help` | Print the help table. |

`--help` and `-h` are accepted as aliases of `help`.

## Examples

```
/mouse on
```
Enables mouse support; scroll and click events are delivered to the UI.

```
/mouse off
```
Disables mouse support and prints the keyboard-only reminder with the
shortcuts.

```
/mouse
```
Reports whether mouse support is currently on or off, and the usage line.

```
/mouse help
```
Prints the table above.

## Output

`on` confirms mouse support is enabled. `off` confirms it is disabled and
lists the keyboard shortcuts that cover mouse functions:

- `Alt+Up` / `Alt+Down` -- move between teammates
- `Tab` -- navigate
- `Enter` -- select
- `Esc` -- close dialogs
- `Ctrl+C` -- copy, `Ctrl+V` -- paste

The bare form prints the current state and usage; `help` prints the table.

## Related

- Keyboard shortcuts replace all mouse interactions when disabled.
- Text selection in the message window (see the GUI quickstart).