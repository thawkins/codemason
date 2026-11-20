# /about

> Show application info, version, and authors

## Overview

`/about` prints a short informational banner describing codemason: what it is, the
compiled version number (`CARGO_PKG_VERSION`), the build timestamp, the source
repository URL, the license, and the author list. It takes no arguments and
performs no configuration lookups  -  everything shown is baked in at compile
time except the build date, which is stamped when the command runs.

The output is appended to the message window as an assistant bubble prefixed
with `From: /about`, and the status bar is set to `about`.

## Syntax

```
/about
```

## Options / Subcommands

| Form | Description |
| --- | --- |
| `/about` | Show the application info banner |

No subcommands or arguments are supported; extra text after the trigger is
ignored.

## Examples

```
/about
```

Display the banner:

```
  codemason - AI Coding Agent

  An interactive GUI-based AI coding agent
  supporting multiple LLM providers.

  Version:     1.0.95
  Built:       2026-09-11 08:14:02 UTC
  Repository:  https://github.com/thawkins/codemason
  License:     MIT

  Authors:
    Tim Hawkins <tim.thawkins@gmail.com>
```

## Output

- An assistant message bubble titled `From: /about` containing the info block.
- The version line comes from the crate version compiled into the binary.
- The `Built:` line reflects the wall-clock time the command was run
  (UTC), not a compile-time build date.
- Status bar shows `about`.

## Related

- About dialog  -  the GUI's Help tab opens a Material About dialog showing the
  CodeMason lockup (dark or light to match the theme), the version and a link to
  the repository (`dialogs::about`); the macOS app menu's About opens the same
  dialog. Close, `Esc` or a click outside dismisses it, it fits the smallest
  window, and while it is open the app's shortcuts and menu commands do not act
  on the session behind it. `/about` remains the text-only CLI form.
- `/help`  -  list all available slash commands
- `/cost`  -  session token usage and estimated cost
- `/startup`  -  startup timing report