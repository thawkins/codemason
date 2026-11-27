//! `git_status` - Show working tree status.

use anyhow::Result;
use serde_json::{Value, json};

use crate::{Tool, ToolContext, ToolOutput};

/// Tool that shows the working tree status.
pub struct GitStatusTool;

#[async_trait::async_trait]
impl Tool for GitStatusTool {
    fn name(&self) -> &'static str {
        "git_status"
    }

    fn description(&self) -> &'static str {
        "Show the git working tree status, including modified, staged, untracked, and conflicted files. \
         No required parameters. 'short' (boolean, default false) returns a compact, one-line-per-file format; \
         'branch' (boolean, default true) includes the current branch name in the output. \
         The tool parses the output and returns structured lists of staged, modified, untracked, and conflicted paths. \
         Common gotcha: in porcelain mode untracked files are shown with a '?' prefix; conflicted files appear when a merge or rebase is in progress."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "short": {
                    "type": "boolean",
                    "description": "Use short format (default: false)"
                },
                "branch": {
                    "type": "boolean",
                    "description": "Include branch name in output (default: true)"
                }
            }
        })
    }

    fn permission_category(&self) -> &'static str {
        "git:read"
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let short = input["short"].as_bool().unwrap_or(false);
        let branch = input["branch"].as_bool().unwrap_or(true);

        let mut args = vec!["status"];
        if short {
            args.push("--short");
            if branch {
                args.push("--branch");
            }
        } else {
            if branch {
                args.push("--branch");
            }
            args.push("--porcelain=v2");
        }

        let args: Vec<String> = args.into_iter().map(ToString::to_string).collect();

        let (stdout, stderr) = crate::git::run_git_async(args, ctx.working_dir.clone()).await?;

        if stdout.trim().is_empty() && !stderr.is_empty() {
            return Ok(ToolOutput {
                content: format!("git status error: {}", stderr.trim()),
                metadata: None,
            });
        }

        // Parse structured status data (TL-04: v2 and short/v1 records have
        // different layouts, so each mode has its own parser).
        let mut lists = StatusLists::default();
        for line in stdout.lines() {
            if short {
                lists.push_short(line);
            } else {
                lists.push_v2(line);
            }
        }
        let StatusLists {
            modified,
            staged,
            untracked,
            conflicted,
        } = lists;

        let branch_name = if short {
            stdout
                .lines()
                .find(|l| l.starts_with("## "))
                .map(|l| l[3..].to_string())
        } else {
            stdout
                .lines()
                .find(|l| l.starts_with("# branch.head "))
                .map(|l| l[14..].trim().to_string())
        };

        let content = if stdout.trim().is_empty() {
            format!(
                "Working tree clean{}.",
                branch_name
                    .as_ref()
                    .map(|b| format!(" on branch {b}"))
                    .unwrap_or_default()
            )
        } else {
            stdout
        };

        Ok(ToolOutput {
            content,
            metadata: Some(json!({
                "branch": branch_name,
                "staged": staged,
                "modified": modified,
                "untracked": untracked,
                "conflicted": conflicted,
                "staged_count": staged.len(),
                "modified_count": modified.len(),
                "untracked_count": untracked.len(),
                "conflicted_count": conflicted.len(),
            })),
        })
    }
}

/// Path lists extracted from `git status` output.
#[derive(Debug, Default)]
struct StatusLists {
    modified: Vec<String>,
    staged: Vec<String>,
    untracked: Vec<String>,
    conflicted: Vec<String>,
}

impl StatusLists {
    /// Classify one entry from its index (`x`) and worktree (`y`) status.
    ///
    /// `unchanged` is the "no change" marker for the format: `.` in porcelain
    /// v2, a space in short (v1) output.
    fn classify(&mut self, x: char, y: char, unchanged: char, path: &str) {
        if x != unchanged {
            self.staged.push(path.to_string());
        }
        if matches!(y, 'M' | 'D') {
            self.modified.push(path.to_string());
        }
    }

    /// Parse one `--porcelain=v2` line.
    ///
    /// Formats (git-status(1)): `1 XY sub mH mI mW hH hI path`,
    /// `2 XY sub mH mI mW hH hI Xscore path<TAB>origPath`,
    /// `u XY sub m1 m2 m3 mW h1 h2 h3 path`, `? path`, `! path`, and
    /// `# branch.*` headers (handled by the caller).
    fn push_v2(&mut self, line: &str) {
        if let Some(path) = line.strip_prefix("? ") {
            self.untracked.push(unquote_path(path));
            return;
        }
        // Field count before the path for each record kind.
        let (fields, conflict) = match line.get(..2) {
            Some("1 ") => (8, false),
            Some("2 ") => (9, false),
            Some("u ") => (10, true),
            _ => return,
        };
        let mut parts = line.splitn(fields + 1, ' ');
        let xy = parts.nth(1).unwrap_or_default();
        let Some(rest) = parts.nth(fields - 2) else {
            return;
        };
        // A rename record carries `path<TAB>origPath`; report the new path.
        // A tab inside a name is printed quoted as `\t`, so the split is safe.
        let path = unquote_path(rest.split('\t').next().unwrap_or(rest));
        if conflict {
            self.conflicted.push(path);
            return;
        }
        let mut xy = xy.chars();
        if let (Some(x), Some(y)) = (xy.next(), xy.next()) {
            self.classify(x, y, '.', &path);
        }
    }

    /// Parse one `--short` (porcelain v1) line: `XY path`,
    /// `XY orig -> path` for renames/copies, `?? path`, `## branch`.
    fn push_short(&mut self, line: &str) {
        let mut chars = line.chars();
        let (Some(x), Some(y), Some(' ')) = (chars.next(), chars.next(), chars.next()) else {
            return;
        };
        let rest = chars.as_str();
        match (x, y) {
            ('#', '#') | ('!', '!') => {}
            ('?', '?') => self.untracked.push(unquote_path(rest)),
            ('D', 'D')
            | ('A', 'U')
            | ('U', 'D')
            | ('U', 'A')
            | ('D', 'U')
            | ('A', 'A')
            | ('U', 'U') => self.conflicted.push(unquote_path(rest)),
            _ => {
                self.classify(x, y, ' ', &short_target(rest));
            }
        }
    }
}

/// The destination path of a short-format (porcelain v1) entry: the part after
/// ` -> ` for a rename or copy, otherwise the whole entry; decoded.
///
/// An unquoted short-format path contains no spaces (git quotes those), so the
/// separator can only appear inside a quoted origin, which is skipped first.
fn short_target(rest: &str) -> String {
    let after_origin = if rest.starts_with('"') {
        quoted_len(rest).map_or(rest, |len| &rest[len..])
    } else {
        rest
    };
    match after_origin.find(" -> ") {
        Some(at) => unquote_path(&after_origin[at + 4..]),
        None => unquote_path(rest),
    }
}

/// The byte length of the C-style quoted string at the start of `s`,
/// including both quotes, or `None` when it is not closed.
fn quoted_len(s: &str) -> Option<usize> {
    let bytes = s.as_bytes();
    let mut i = 1;
    while i < bytes.len() {
        match bytes[i] {
            b'\\' => i += 2,
            b'"' => return Some(i + 1),
            _ => i += 1,
        }
    }
    None
}

/// Decode a path as git prints it.
///
/// Git wraps a path holding a space (short format), a control character, a
/// quote, a backslash or, under the default `core.quotePath`, a non-ASCII byte
/// in double quotes with C-style escapes: `\t`, `\n`, `\"`, `\\` and the like,
/// and `\ooo` octal for raw bytes (`caf\303\251` is `café` in UTF-8). Anything
/// not wrapped in quotes is returned unchanged.
fn unquote_path(raw: &str) -> String {
    let Some(inner) = raw.strip_prefix('"').and_then(|s| s.strip_suffix('"')) else {
        return raw.to_string();
    };
    let bytes = inner.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] != b'\\' || i + 1 == bytes.len() {
            out.push(bytes[i]);
            i += 1;
            continue;
        }
        let escaped = bytes[i + 1];
        let simple = match escaped {
            b'a' => Some(0x07),
            b'b' => Some(0x08),
            b't' => Some(b'\t'),
            b'n' => Some(b'\n'),
            b'v' => Some(0x0b),
            b'f' => Some(0x0c),
            b'r' => Some(b'\r'),
            b'"' => Some(b'"'),
            b'\\' => Some(b'\\'),
            _ => None,
        };
        if let Some(byte) = simple {
            out.push(byte);
            i += 2;
            continue;
        }
        let octal = &bytes[i + 1..bytes.len().min(i + 4)];
        if octal.len() == 3 && octal.iter().all(|b| (b'0'..=b'7').contains(b)) {
            let value = octal
                .iter()
                .fold(0u32, |acc, b| acc * 8 + u32::from(b - b'0'));
            out.push(u8::try_from(value).unwrap_or(b'?'));
            i += 4;
        } else {
            // An escape git does not produce: keep it as written.
            out.push(b'\\');
            i += 1;
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}
