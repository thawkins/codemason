//! The `git+<url>#<ref>[:<subpath>]` source parser shared by `/plugins` and
//! `/connectors` (RS-11).
//!
//! The two crates carried parallel copies that had drifted: the connectors
//! copy kept a source whose subpath failed validation (deriving a manifest URL
//! for the whole repository instead) and accepted a subpath starting with `/`.
//! Both now use this one parser, which refuses such a source.
//!
//! Depends on `codemason_types::guard::is_safe_operand` for the git-operand
//! charset.

use codemason_types::guard::is_safe_operand;

/// A parsed `git+<https-url>#<ref>[:<subpath>]` source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitSource {
    /// The `https` repository remote.
    pub url: String,
    /// The ref to check out; `None` uses the remote default branch (HEAD).
    pub ref_name: Option<String>,
    /// A repository-relative subdirectory to install; `None` installs the root.
    pub subpath: Option<String>,
}

/// Parse a `git+<https-url>#<ref>[:<subpath>]` source string.
///
/// Only `https://` remotes are accepted for a store-supplied source; a
/// `file://` remote is additionally accepted so a local git mirror can be
/// installed, mirroring the local-directory source the same command already
/// accepts. Every other scheme (ssh, git, http) returns `None`, so it can never
/// reach the clone (FR-024).
///
/// An unsafe ref is dropped (the default branch is used instead), but an
/// unsafe `:<subpath>` (leading `/` or `-`, a `..` segment, or a character
/// outside `[A-Za-z0-9._/-]`) refuses the whole source with `None`.
///
/// # Arguments
/// * `source` - the raw source string.
///
/// # Returns
/// The parsed source, or `None` when it is not an acceptable `git+` source.
#[must_use]
pub fn parse_git_source(source: &str) -> Option<GitSource> {
    let rest = source.strip_prefix("git+")?;
    let (url, fragment) = match rest.split_once('#') {
        Some((url, fragment)) => (url, Some(fragment)),
        None => (rest, None),
    };
    if url.trim().is_empty() || !(url.starts_with("https://") || url.starts_with("file://")) {
        return None;
    }
    let (ref_name, subpath) = match fragment {
        None => (None, None),
        Some(fragment) => match fragment.split_once(':') {
            Some((r, path)) => {
                let subpath = git_subpath(path);
                // A subpath that was given but fails validation refuses the
                // whole source: dropping it would mean "install the entire
                // repository", which is not what the source asked for.
                if subpath.is_none() && !path.trim().trim_start_matches("./").is_empty() {
                    return None;
                }
                (git_ref(r), subpath)
            }
            None => (git_ref(fragment), None),
        },
    };
    Some(GitSource {
        url: url.to_string(),
        ref_name,
        subpath,
    })
}

/// A git ref, treating an empty value or `HEAD` as "the default branch".
///
/// The ref is validated against a safe charset before it can reach `git`:
/// a `git+` source is untrusted content (it is emitted from a marketplace
/// document fetched over the network or typed by a social-engineered user),
/// and `git` parses a leading `-` as an *option*, not a positional. Options
/// such as `--upload-pack=<cmd>` execute a program, which escapes the plugin
/// sandbox entirely (SEC-codemason-plugins-001). Anything that is not
/// `[A-Za-z0-9._/-]+` - and anything beginning with `-` - is refused.
fn git_ref(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("HEAD") {
        None
    } else if is_safe_git_argument(trimmed) {
        Some(trimmed.to_string())
    } else {
        None
    }
}

/// A repository-relative subpath, normalised without a leading `./`.
///
/// Validated with the same charset as [`git_ref`]: the subpath reaches
/// `git sparse-checkout set <subpath>`, where a leading `-` would again be
/// parsed as an option (SEC-codemason-plugins-001). An absolute subpath is
/// dropped too: `repo_dir.join("/x")` would name `/x` itself, outside the
/// clone.
fn git_subpath(raw: &str) -> Option<String> {
    let trimmed = raw.trim().trim_start_matches("./");
    if trimmed.is_empty() {
        None
    } else if is_safe_git_argument(trimmed) && !trimmed.contains("..") && !trimmed.starts_with('/')
    {
        Some(trimmed.to_string())
    } else {
        None
    }
}

/// Return `true` when `value` is safe to hand to `git` as a positional
/// argument: a non-empty `[A-Za-z0-9._/-]` string that does not begin with
/// `-` (which git would parse as an option) and contains no `..` segment.
///
/// See SEC-codemason-plugins-001: without this gate, a `git+` ref of
/// `--upload-pack=/bin/sh -c true` reaches `Command::new("git")` and executes
/// a program with the user's privileges.
fn is_safe_git_argument(value: &str) -> bool {
    is_safe_operand(value) && !value.contains("..")
}
