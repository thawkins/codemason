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
    let commands = shell_commands(command);
    let (forge, args, marker, head_flags): (&str, &[String], &str, &[&str]) =
        commands.iter().find_map(|words| {
            // Leading `NAME=value` assignments are not the program.
            let words = &words[words.iter().take_while(|w| is_assignment(w)).count()..];
            let is = |program: [&str; 3]| words.iter().take(3).eq(program.iter());
            if is(["gh", "pr", "create"]) {
                Some(("github", &words[3..], "/pull/", &["--head", "-H"][..]))
            } else if is(["glab", "mr", "create"]) {
                Some((
                    "gitlab",
                    &words[3..],
                    "/-/merge_requests/",
                    &["--source-branch", "-s"][..],
                ))
            } else {
                None
            }
        })?;
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

/// Whether `word` is a `NAME=value` variable assignment.
fn is_assignment(word: &str) -> bool {
    word.split_once('=').is_some_and(|(name, _)| {
        !name.is_empty()
            && name.chars().all(|c| c == '_' || c.is_ascii_alphanumeric())
            && !name.starts_with(|c: char| c.is_ascii_digit())
    })
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

/// Splits a shell command line into its simple commands, each a list of
/// words. Whitespace separates words; single and double quotes group (so a
/// quoted `$(cat <<EOF ...)` body is one word); a backslash escapes the
/// next character outside single quotes; an unquoted `;`, `&`, `|` or
/// newline ends a command; and an unquoted `#` starting a word comments out
/// the rest of the line. Enough to find a `gh`/`glab` invocation and read
/// its flags; not a full shell parser.
fn shell_commands(command: &str) -> Vec<Vec<String>> {
    let mut commands = Vec::new();
    let mut words: Vec<String> = Vec::new();
    let mut word = String::new();
    let mut in_word = false;
    let mut chars = command.chars().peekable();
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
            '#' if !in_word => while chars.next_if(|&c| c != '\n').is_some() {},
            c if c.is_whitespace() || matches!(c, ';' | '&' | '|') => {
                if in_word {
                    words.push(std::mem::take(&mut word));
                    in_word = false;
                }
                if (c == '\n' || matches!(c, ';' | '&' | '|')) && !words.is_empty() {
                    commands.push(std::mem::take(&mut words));
                }
            }
            c => {
                in_word = true;
                word.push(c);
            }
        }
    }
    if in_word {
        words.push(word);
    }
    if !words.is_empty() {
        commands.push(words);
    }
    commands
}
