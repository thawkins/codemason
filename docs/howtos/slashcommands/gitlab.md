# /gitlab

> GitLab integration: /gitlab setup | logout | status | help

## Overview

`/gitlab` manages GitLab authentication for the GUI session.

`setup` opens an interactive dialog pre-filled with the existing instance
URL (default `https://gitlab.com`) and collects three values:

- Instance URL
- Username
- Personal access token

## Syntax

```
/gitlab
/gitlab setup
/gitlab logout
/gitlab status
/gitlab help
```

## Options

| Form | Description |
| ---- | ----------- |
| `/gitlab` | bare form, no subcommand |
| `/gitlab setup` | interactive dialog for instance URL, username, and personal access token |
| `/gitlab logout` | delete the stored token and config |
| `/gitlab status` | report the current auth state |
| `/gitlab help` | print command help |

## Setup dialog fields

| Field | Description |
| ----- | ----------- |
| Instance URL | pre-filled with the existing instance URL; default `https://gitlab.com` |
| Username | your GitLab username |
| Personal access token | the token used for GitLab API access |

## Examples

Open the setup dialog:

```
/gitlab setup
```

Check the auth state:

```
/gitlab status
```

Remove the stored token and config:

```
/gitlab logout
```

Print command help:

```
/gitlab help
```

## Output

- `setup` opens the dialog.
- `logout` confirms deletion.
- `status` prints the auth state.

## Notes

- The dialog is pre-filled with the existing instance URL; when nothing is
  stored yet the default is `https://gitlab.com`.
- `logout` deletes both the stored token and the config.
- `setup` can be re-run to update the stored instance URL, username, or
  token.
- A merge request the agent opens with `glab mr create` in a **Bash** call is
  bound to the session, exactly as `gitlab_create_mr` and `autofix_add` are, so
  the session's `PR/MRs` submenu, its sidebar count and Auto-fix all see it.
  The binding reads the request URL from the call's output and its title and
  source branch from that command's own `--title`/`-t` and
  `--source-branch`/`-s` arguments. Only a call that exited `0` binds, and only
  when `glab mr create` is the program of a simple command.

## Related

- `/github` - the GitHub counterpart
- `/new --gitlab` - scaffold a project and host it on GitLab