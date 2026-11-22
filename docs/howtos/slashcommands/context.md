# /context

> "Manage context cache: /context refresh | /context help" (SlashCommandDef, crates/codemason-app-core/src/app/state.rs).

## Overview

Prompt context (the project file tree, git status, and README content) is
assembled once and cached between messages so that every turn does not have to
re-walk the repository. `/context refresh` drops that cache, forcing the next
user message to recompute the file tree, git status, and README from the
current working directory. Use it after creating, moving, or deleting files
outside of the agent's own tool calls, or when switching branches in a
separate terminal.

The handler is the `context` arm in `crates/codemason-app-core/src/app/slash.rs`
(approximately lines 2215-2235).

## Syntax

```
/context refresh
/context help
```

Bare `/context` is equivalent to `/context help`.

## Options / Subcommands

| Form | Description |
|---|---|
| `/context refresh` | Clear the prompt context cache; the next message recomputes file tree, git status, and README |
| `/context help` | Show the subcommand table |

## Examples

### Force a context refresh

```
/context refresh
```

Output:

```
From: /context refresh
[sync] Context cache cleared - next message will recompute file tree, git status, and README.
```

### Show help

```
/context help
```

Output (subcommand table):

```
From: /context help
| Subcommand | Description |
|---|---|
| refresh | Clear the prompt context cache |
| help | Show this help |
```

### Unknown subcommand

```
/context rebuild
```

Output (usage line):

```
From: /context
Unknown subcommand. Usage: /context refresh | /context help
```

## Output

- `refresh` calls `codemason_agent::agent::clear_prompt_context_cache()` and
  reports `[sync] Context cache cleared - next message will recompute file
  tree, git status, and README.` The status line shows the subcommand name.
- `help` (and bare `/context`) prints the subcommand table.
- Any other argument prints a usage line. Nothing is persisted and no files
  are touched; the effect is a single cache invalidation.

The cache is also invalidated automatically after agent file edits; manual
`refresh` is only needed when the working tree changes outside the agent.

## Related

- `Alt+C` - bring the **Context pane** to the front, or give the front back to
  Sessions. It is a pane, not a subcommand: `/context` manages the cache,
  `Alt+C` shows the result. The pane is a pinned tab beside **Sessions** that
  cannot be closed, floated or moved, and it shows what the next request sends
  to the model: a header with the share of the context window used and
  `used / window`, one stacked bar with a coloured segment per partition, and
  one row per partition (**History**, system prompt, Skills, Memory,
  Instruction files, Built-in tools, MCP tools, Tool metadata) with its tokens
  and share. A row opens on a click to list its items largest first, and the
  history partition is costed as the request actually carries it, so the pane
  can no longer report more tokens than the window. See `GUI-QUICKSTART.md`
  section 13a.
- `/undo` - remove the last turn pair from the conversation
- `/reload` - reload configuration, agents, MCP, and skills
