//! Binding a pull/merge request the agent just created to the session that
//! created it (spec SESSIONMENU FR-14).
//!
//! `github_create_pr` and `gitlab_create_mr` call [`bind_created`] after a
//! successful create. The binding goes to the top-level session (a request
//! opened by a sub-agent belongs to the session the user sees) through
//! [`crate::storage::StorageBackend::bind_session_request`]. Binding is a
//! convenience: a failure is logged and never fails the tool.
//!
//! A request opened from the shell (`gh pr create`, `glab mr create` in a
//! `Bash` call) is bound the same way by [`bind_from_shell`], which reads
//! the request's URL from the command's output.

use codemason_storage::SessionRequest;

use crate::ToolContext;

/// The host of a request's browser URL: `https://host[:port]/...` yields
/// `host[:port]`, lower-cased. `None` when the URL has no host.
#[must_use]
pub fn web_url_host(web_url: &str) -> Option<String> {
    let rest = web_url.split_once("://").map(|(_, rest)| rest)?;
    let host = rest.split(['/', '?', '#']).next().unwrap_or(rest);
    let host = host.rsplit_once('@').map_or(host, |(_, host)| host);
    (!host.is_empty()).then(|| host.to_ascii_lowercase())
}

/// Binds a newly created request to the calling session, if the context
/// has storage. `request.session_id` and `request.host` are filled in here
/// from `ctx` and `request.web_url`.
pub fn bind_created(ctx: &ToolContext, mut request: SessionRequest) {
    let Some(storage) = ctx.storage.as_ref() else {
        return;
    };
    if ctx.session_id.is_empty() || request.number == 0 {
        return;
    }
    let Some(host) = web_url_host(&request.web_url) else {
        tracing::debug!(url = %request.web_url, "created request has no web URL; not bound");
        return;
    };
    request.session_id.clone_from(&ctx.session_id);
    request.host = host;
    let autofix_default = ctx
        .config
        .as_ref()
        .is_some_and(|c| c.vcs.autofix.default_on());
    if let Err(e) = storage.bind_session_request(&request, autofix_default) {
        tracing::debug!(error = %e, number = request.number, "request not bound to session");
    }
}

/// A request a shell command created, as read by [`request_from_shell`].
/// `session_id` and `host` are left empty for [`bind_created`] to fill.
///
/// # Arguments
///
/// * `command` - the shell command that ran.
/// * `output` - its combined output.
///
/// # Returns
///
/// The request when `command` runs `gh pr create` (GitHub) or
/// `glab mr create` (GitLab) and `output` holds the request's URL (the
/// last such URL wins). `None` otherwise.
#[must_use]
pub fn request_from_shell(command: &str, output: &str) -> Option<SessionRequest> {
    let tokens = shell_tokens(command);
    let (forge, start, marker, head_flags): (&str, usize, &str, &[&str]) =
        if let Some(i) = find_words(&tokens, &["gh", "pr", "create"]) {
            ("github", i + 3, "/pull/", &["--head", "-H"])
        } else {
            let i = find_words(&tokens, &["glab", "mr", "create"])?;
            (
                "gitlab",
                i + 3,
                "/-/merge_requests/",
                &["--source-branch", "-s"],
            )
        };
    let args = tokens.get(start..).unwrap_or_default();
    let (web_url, repo, number) = output
        .split(|c: char| c.is_whitespace() || matches!(c, '"' | '\'' | '<' | '>' | '(' | ')'))
        .rev()
        .find_map(|word| request_url(word, marker))?;
    Some(SessionRequest {
        session_id: String::new(),
        forge: forge.to_string(),
        host: String::new(),
        repo,
        number,
        title: flag_value(args, &["--title", "-t"]).unwrap_or_default(),
        web_url,
        head_branch: flag_value(args, head_flags).unwrap_or_default(),
        state: "open".to_string(),
    })
}

/// Binds the request a `Bash` call created (see [`request_from_shell`]) to
/// the calling session. A command that created none is ignored.
pub fn bind_from_shell(ctx: &ToolContext, command: &str, output: &str) {
    if let Some(request) = request_from_shell(command, output) {
        bind_created(ctx, request);
    }
}

/// `(web_url, repo, number)` when `word` is a request URL whose path is
/// `/<repo>{marker}<number>`; trailing punctuation is ignored.
fn request_url(word: &str, marker: &str) -> Option<(String, String, u64)> {
    let url = word.trim_end_matches(['.', ',', ';', ':', '!', '?', ']', '`', '*']);
    let rest = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))?;
    let (_, path) = rest.split_once('/')?;
    let (repo, number) = path.rsplit_once(marker)?;
    let number: u64 = number.parse().ok().filter(|n| *n > 0)?;
    (repo.contains('/') && !repo.starts_with('/'))
        .then(|| (url.to_string(), repo.to_string(), number))
}

/// Index of the first run of `words` in `tokens`.
fn find_words(tokens: &[String], words: &[&str]) -> Option<usize> {
    tokens
        .windows(words.len())
        .position(|w| w.iter().zip(words).all(|(t, w)| t == w))
}

/// The value of the first of `flags` in `args` (`--flag value` or
/// `--flag=value`).
fn flag_value(args: &[String], flags: &[&str]) -> Option<String> {
    args.iter().enumerate().find_map(|(i, arg)| {
        flags.iter().find_map(|flag| {
            if arg == flag {
                args.get(i + 1).cloned()
            } else {
                arg.strip_prefix(flag)
                    .and_then(|rest| rest.strip_prefix('='))
                    .map(str::to_string)
            }
        })
    })
}

/// Splits a shell command into words: whitespace separates, single and
/// double quotes group, a backslash escapes the next character outside
/// single quotes, and `;`, `&` and `|` end a word. Enough to read the flags
/// of a `gh`/`glab` invocation; not a full shell parser.
fn shell_tokens(command: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut word = String::new();
    let mut in_word = false;
    let mut chars = command.chars();
    while let Some(c) = chars.next() {
        match c {
            '\'' => {
                in_word = true;
                word.extend(chars.by_ref().take_while(|&c| c != '\''));
            }
            '"' => {
                in_word = true;
                while let Some(c) = chars.next() {
                    match c {
                        '"' => break,
                        '\\' => word.extend(chars.next()),
                        c => word.push(c),
                    }
                }
            }
            '\\' => {
                in_word = true;
                word.extend(chars.next());
            }
            c if c.is_whitespace() || matches!(c, ';' | '&' | '|') => {
                if in_word {
                    tokens.push(std::mem::take(&mut word));
                    in_word = false;
                }
            }
            c => {
                in_word = true;
                word.push(c);
            }
        }
    }
    if in_word {
        tokens.push(word);
    }
    tokens
}
