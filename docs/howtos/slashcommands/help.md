# /help

> Show available slash commands

## Overview

`/help` prints the full list of slash commands as a Markdown table with
`Command` and `Description` columns, drawn from the `SLASH_COMMANDS` registry
(the same definitions that drive autocomplete). After the built-in commands, it
appends a `Skills:` table listing every user-invocable skill loaded from the
configured skill directories, with its argument hint appended to the trigger and
its description (or `(no description)`); user-defined and plugin-contributed
commands get their own tables after that. The viewer sizes the columns, so a
long description stays whole in its cell instead of being clipped to a fixed
column width.

It takes no arguments and performs no filtering  -  it is always the complete
inventory.

## Syntax

```
/help
```

## Options / Subcommands

| Form | Description |
| --- | --- |
| `/help` | List all available slash commands plus user-invocable skills |

No arguments or subcommands.

## Examples

```
/help
```

Built-in commands section (a Markdown table):

```
Available commands:

| Command | Description |
| --- | --- |
| `/about` | Show application info, version, and authors |
| `/agent` | Switch the active agent: /agent [<name>] \| /agent help |
| `/agents` | List all agents - built-in and custom |
...
```

Skills section (appended when any user-invocable skills are loaded):

```
Skills:

| Skill | Description |
| --- | --- |
| `/docupdate` | Refresh docs and CHANGELOG |
| `/release <args>` | Run the release pipeline |
```

```
/help
```

No skills loaded: the output ends after the built-in command table.

## Output

- One assistant bubble `From: /help` containing a Markdown table of all
  registered commands (`/trigger` and description), optionally followed by a
  `Skills:` table of user-invocable skills from the skill registry (name +
  argument hint, then description), then tables for any user-defined commands
  (`commands/*.md`) and plugin-contributed commands.
- The working directory and skill directories come from the loaded codemason
  config; skills are discovered fresh at each invocation.
- Status bar shows `help`.

## Related

- `/agents`  -  list agents (commands are listed by `/help`, agents by
  `/agents`)
- Skills are custom YAML packs in the configured skill directories
- `/help` per-command variants such as `/agent help`, `/cancel help`,
  `/history help` print their own subcommand tables