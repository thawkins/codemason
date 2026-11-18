//! User-defined slash commands loaded from Markdown files.
//!
//! A file `<name>.md` in `~/.config/codemason/commands/` or in the nearest
//! `.codemason/commands/` directory of the project defines the slash command
//! `/<name>`. The file body is a prompt template that is sent to the agent as a
//! user turn; an optional `---` frontmatter block may carry a `description`.
//! A project file replaces a user-global file of the same name.
//!
//! Placeholders in the body:
//!
//! | Placeholder   | Replacement                                   |
//! |---------------|-----------------------------------------------|
//! | `$ARGUMENTS`  | Everything typed after the command            |
//! | `$1` .. `$9`  | The Nth argument (1-based, quotes group words) |
//!
//! Depends on `codemason_plugins` for the frontmatter parser shared with
//! plugin prompt commands, and on `codemason_config` for the user-global
//! directory.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use crate::skill::args::parse_args;

/// Directory name holding command files, under `.codemason/` or the user-global
/// config directory.
const COMMANDS_DIR: &str = "commands";

/// One user-defined slash command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserCommand {
    /// The trigger, without a leading `/` (the file stem).
    pub name: String,
    /// Description from the frontmatter; empty when the file has none.
    pub description: String,
    /// The prompt template (file body after the frontmatter).
    pub prompt: String,
    /// The file the command was loaded from.
    pub path: PathBuf,
}

/// User-global commands directory: `~/.config/codemason/commands/`.
#[must_use]
pub fn global_commands_dir() -> Option<PathBuf> {
    codemason_config::user_dirs::global_state_dir().map(|d| d.join(COMMANDS_DIR))
}

/// Walks up from `working_dir` to the nearest `.codemason/commands/` directory.
#[must_use]
pub fn find_project_commands_dir(working_dir: &Path) -> Option<PathBuf> {
    working_dir
        .ancestors()
        .map(|dir| dir.join(".codemason").join(COMMANDS_DIR))
        .find(|candidate| candidate.is_dir())
}

/// Loads the user-defined commands visible from `working_dir`.
///
/// `existing` holds the triggers already taken (built-in commands and skills);
/// a file whose name collides with one is skipped. Returns the commands sorted
/// by name and one diagnostic line per skipped file.
#[must_use]
pub fn load_user_commands(
    working_dir: &Path,
    existing: &BTreeSet<String>,
) -> (Vec<UserCommand>, Vec<String>) {
    let dirs: Vec<PathBuf> = global_commands_dir()
        .into_iter()
        .chain(find_project_commands_dir(working_dir))
        .collect();
    load_user_commands_from(&dirs, existing)
}

/// Loads commands from `dirs`, lowest priority first.
///
/// A later directory replaces a same-named command from an earlier one.
/// Subdirectories and non-`.md` files are ignored. See [`load_user_commands`]
/// for the collision rule and the return value.
#[must_use]
pub fn load_user_commands_from(
    dirs: &[PathBuf],
    existing: &BTreeSet<String>,
) -> (Vec<UserCommand>, Vec<String>) {
    let mut commands: BTreeMap<String, UserCommand> = BTreeMap::new();
    let mut diagnostics = Vec::new();
    for dir in dirs {
        let Ok(entries) = std::fs::read_dir(dir) else {
            continue;
        };
        let mut paths: Vec<PathBuf> = entries
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| path.is_file() && path.extension().is_some_and(|ext| ext == "md"))
            .collect();
        paths.sort();
        for path in paths {
            match load_command_file(&path, existing) {
                Ok(command) => {
                    commands.insert(command.name.clone(), command);
                }
                Err(reason) => diagnostics.push(format!("{}: {reason}", path.display())),
            }
        }
    }
    (commands.into_values().collect(), diagnostics)
}

/// Reads one command file, or says why it cannot be a command.
fn load_command_file(path: &Path, existing: &BTreeSet<String>) -> Result<UserCommand, String> {
    let name = path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or_default()
        .to_string();
    if !is_valid_name(&name) {
        return Err(
            "command name must start with a lowercase letter or digit and contain only \
             lowercase letters, digits, `-` and `_`"
                .to_string(),
        );
    }
    if existing.contains(&name) {
        return Err(format!("`/{name}` is already a built-in command or skill"));
    }
    let content = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    let (description, prompt) = codemason_plugins::manifest::parse_command_markdown(&content);
    if prompt.trim().is_empty() {
        return Err("the prompt body is empty".to_string());
    }
    Ok(UserCommand {
        name,
        description,
        prompt,
        path: path.to_path_buf(),
    })
}

/// Whether `name` is usable as a slash trigger.
fn is_valid_name(name: &str) -> bool {
    let mut chars = name.chars();
    chars
        .next()
        .is_some_and(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
}

/// Substitutes `$ARGUMENTS` and `$1`..`$9` in a command prompt.
///
/// Arguments are split on whitespace, with single or double quotes grouping
/// words. A positional placeholder with no matching argument becomes empty. A
/// `$` followed by anything else is left as written.
///
/// # Examples
///
/// ```
/// use codemason_agent::user_command::render_user_command;
///
/// let out = render_user_command("Review $1 against $2. Notes: $ARGUMENTS", "a.rs \"b c\"");
/// assert_eq!(out, "Review a.rs against b c. Notes: a.rs \"b c\"");
/// ```
#[must_use]
pub fn render_user_command(prompt: &str, args: &str) -> String {
    let parsed = parse_args(args);
    let mut out = String::with_capacity(prompt.len() + args.len());
    let mut rest = prompt;
    while let Some(pos) = rest.find('$') {
        out.push_str(&rest[..pos]);
        let after = &rest[pos + 1..];
        if let Some(tail) = after.strip_prefix("ARGUMENTS") {
            out.push_str(args);
            rest = tail;
        } else if let Some(digit) = after.chars().next().and_then(|c| c.to_digit(10))
            && digit > 0
        {
            if let Some(arg) = parsed.get(digit as usize - 1) {
                out.push_str(arg);
            }
            rest = &after[1..];
        } else {
            out.push('$');
            rest = after;
        }
    }
    out.push_str(rest);
    out
}
