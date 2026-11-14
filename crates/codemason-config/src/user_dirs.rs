//! Canonical global-state directory helpers for codemason.
//!
//! codemason consolidates ALL user-global state under `~/.config/codemason/`
//! (the XDG config directory). Legacy locations are:
//!
//! - `~/.codemason/`            - legacy home-dir root
//! - `~/.local/share/codemason/` - legacy XDG data root
//!
//! This module exposes `Config::global_state_dir()` as the single source of
//! truth for the global root, plus one helper per state subdirectory. Use
//! these instead of hand-writing `home.join(".codemason").join("...")` or
//! `dirs::data_dir().join("codemason").join("...")` so the path is consistent
//! and the legacy migration target never drifts.
//!
//! ## Subdirectories
//!
//! | Helper                        | Canonical path                              | Legacy path |
//! |-------------------------------|---------------------------------------------|-------------|
//! | [`global_memory_dir`]         | `~/.config/codemason/memory/`                  | `~/.codemason/memory/`           |
//! | [`global_agents_dir`]         | `~/.config/codemason/agents/`                  | `~/.codemason/agents/`           |
//! | [`global_skills_dir`]         | `~/.config/codemason/skills/`                  | `~/.codemason/skills/`           |
//! | [`global_teams_dir`]          | `~/.config/codemason/teams/`                   | `~/.codemason/teams/`            |
//! | [`global_blueprints_dir`]     | `~/.config/codemason/blueprints/`              | `~/.codemason/blueprints/`       |
//! | [`global_templates_dir`]      | `~/.config/codemason/templates/`               | `~/.codemason/templates/`        |
//! | [`global_agent_memory_dir`]   | `~/.config/codemason/agent-memory/`            | `~/.codemason/agent-memory/`     |
//! | [`global_mcp_servers_dir`]    | `~/.config/codemason/servers/`                 | `~/.mcp/servers/`             |
//! | [`global_models_dir`]         | `~/.config/codemason/models/`                  | `~/.local/share/codemason/models/` |
//! | [`global_loop_state_dir`]     | `~/.config/codemason/loop-state/`              | `~/.local/share/codemason/loop-state/` |
//! | [`global_inbox_dir`]          | `~/.config/codemason/log/inbox/`               | `~/.local/share/codemason/log/inbox/`  |
//! | [`global_history_path`]       | `~/.config/codemason/input_history.txt`        | `~/.local/share/codemason/input_history.txt` |
//! | [`global_db_path`]            | `~/.config/codemason/codemason.db`                | `~/.local/share/codemason/codemason.db`   |
//! | [`global_github_token_path`]  | `~/.config/codemason/github_token`             | `~/.codemason/github_token`            |
//! | [`global_gitlab_token_path`]  | `~/.config/codemason/gitlab_token`             | `~/.codemason/gitlab_token`            |
//! | [`global_gitlab_config_path`] | `~/.config/codemason/gitlab_config.json`       | `~/.codemason/gitlab_config.json`      |

use std::path::PathBuf;

use crate::config::Config;

/// Return the canonical global-state root: `~/.config/codemason/` (Linux),
/// `%APPDATA%/codemason` (Windows), `~/Library/Application Support/` (macOS).
///
/// Returns `None` when the platform config directory cannot be determined
/// (e.g. `XDG_CONFIG_HOME` unset on a headless Linux box).
///
/// This is the single source of truth for the consolidated state root.
/// Everything under `~/.codemason/` or `~/.local/share/codemason/` that is not
/// project-local should resolve through this.
///
/// Note: on Windows and macOS this collides with `dirs::data_dir()` (they
/// point at the same backing dir); on Linux it intentionally folds the data/
/// split into `config/` per codemason's consolidation policy.
#[must_use]
pub fn global_state_dir() -> Option<PathBuf> {
    Config::global_state_dir()
}

/// Legacy home-dir root. Kept only for the legacy-migration scan; new code
/// must not read or write here.
///
/// Returns `None` when the home directory cannot be determined.
#[doc(hidden)]
#[must_use]
pub fn legacy_home_dir() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".codemason"))
}

/// Legacy XDG-data root. Kept only for the legacy-migration scan; new code
/// must not read or write here.
///
/// Returns `None` when the platform data directory cannot be determined.
#[doc(hidden)]
#[must_use]
pub fn legacy_data_dir() -> Option<PathBuf> {
    dirs::data_dir().map(|d| d.join("codemason"))
}

/// Global memory directory: `~/.config/codemason/memory/`.
#[must_use]
pub fn global_memory_dir() -> Option<PathBuf> {
    global_state_dir().map(|d| d.join("memory"))
}

/// Global custom-agents directory: `~/.config/codemason/agents/`.
///
/// Priority sits between project-local (lowest) and OpenSkills global
/// (highest), matching the existing multi-tier agent discovery chain.
#[must_use]
pub fn global_agents_dir() -> Option<PathBuf> {
    global_state_dir().map(|d| d.join("agents"))
}

/// Global skills directory: `~/.config/codemason/skills/`.
#[must_use]
pub fn global_skills_dir() -> Option<PathBuf> {
    global_state_dir().map(|d| d.join("skills"))
}

/// Global teams directory: `~/.config/codemason/teams/`.
#[must_use]
pub fn global_teams_dir() -> Option<PathBuf> {
    global_state_dir().map(|d| d.join("teams"))
}

/// Global team-blueprints directory: `~/.config/codemason/blueprints/`.
#[must_use]
pub fn global_blueprints_dir() -> Option<PathBuf> {
    global_state_dir().map(|d| d.join("blueprints"))
}

/// Global templates directory: `~/.config/codemason/templates/`.
#[must_use]
pub fn global_templates_dir() -> Option<PathBuf> {
    global_state_dir().map(|d| d.join("templates"))
}

/// Global per-agent memory directory root: `~/.config/codemason/agent-memory/`.
#[must_use]
pub fn global_agent_memory_dir() -> Option<PathBuf> {
    global_state_dir().map(|d| d.join("agent-memory"))
}

/// Global MCP server-definition directory: `~/.config/codemason/servers/`.
///
/// `/mcp discover` scans this directory for `server.json` files or direct
/// `*.json` MCP configs. It is the codemason-native replacement for the generic
/// `~/.mcp/servers/` location, which is still scanned for backwards
/// compatibility.
#[must_use]
pub fn global_mcp_servers_dir() -> Option<PathBuf> {
    global_state_dir().map(|d| d.join("servers"))
}

/// Global embedding / LLM model directory: `~/.config/codemason/models/`.
#[must_use]
pub fn global_models_dir() -> Option<PathBuf> {
    global_state_dir().map(|d| d.join("models"))
}

/// Global loop-state directory: `~/.config/codemason/loop-state/`.
#[must_use]
pub fn global_loop_state_dir() -> Option<PathBuf> {
    global_state_dir().map(|d| d.join("loop-state"))
}

/// Global inbox directory: `~/.config/codemason/log/inbox/`.
#[must_use]
pub fn global_inbox_dir() -> Option<PathBuf> {
    global_state_dir().map(|d| d.join("log").join("inbox"))
}

/// Global input history file: `~/.config/codemason/input_history.txt`.
#[must_use]
pub fn global_history_path() -> Option<PathBuf> {
    global_state_dir().map(|d| d.join("input_history.txt"))
}

/// Global structured-memory database: `~/.config/codemason/codemason.db`.
#[must_use]
pub fn global_db_path() -> Option<PathBuf> {
    global_state_dir().map(|d| d.join("codemason.db"))
}

/// Global GitHub personal access token file: `~/.config/codemason/github_token`.
#[must_use]
pub fn global_github_token_path() -> Option<PathBuf> {
    global_state_dir().map(|d| d.join("github_token"))
}

/// Global GitLab personal access token file: `~/.config/codemason/gitlab_token`.
#[must_use]
pub fn global_gitlab_token_path() -> Option<PathBuf> {
    global_state_dir().map(|d| d.join("gitlab_token"))
}

/// Global GitLab legacy config JSON: `~/.config/codemason/gitlab_config.json`.
#[must_use]
pub fn global_gitlab_config_path() -> Option<PathBuf> {
    global_state_dir().map(|d| d.join("gitlab_config.json"))
}

/// Return the AGENTS.md global instruction file directory.
///
/// This is the directory that `collect_agents_md_content_with_discovery`
/// resolves as "the global directory" for `AGENTS.md` / `CLAUDE.md` /
/// `INSTRUCTIONS.md` files when the project root has no instruction file.
/// The resolved path is used for discovery only - actual loading is
/// per-file inside the walker.
#[must_use]
pub fn global_instructions_dir() -> Option<PathBuf> {
    global_state_dir()
}
