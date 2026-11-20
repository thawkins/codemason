//! Toolchain presence report for the `/toolchain` slash command.
//!
//! This module holds the static data tables that map the code-index
//! supported-language list (`codemason_codeindex::scanner::SUPPORTED_LANGUAGES`)
//! onto the runtime toolchains required to build and run code in each
//! language, plus the classification of every entry as either an application
//! language (runtime probed) or a data/format entry (runtime skipped), and
//! the report engine: a PATH presence probe (`command -v` semantics, no
//! spawn), a bounded version probe (`<tool> <flag>` with a 2 s timeout), and
//! the markdown/JSON report renderers.
//!
//! Dependencies: `codemason_codeindex::scanner` (language list),
//! `serde_json` (report serialisation), `tempfile` (probe test fixtures).

use codemason_codeindex::scanner::SUPPORTED_LANGUAGES;

/// Classification of a `SUPPORTED_LANGUAGES` entry.
///
/// Application languages have a source runtime (compiler or interpreter) that
/// can be probed on `PATH`; data/format entries are pure data, markup, or
/// build-DSL formats with no source runtime and are reported as
/// `(data format - runtime n/a)` without probing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LanguageClass {
    /// Language with a probed runtime (see [`LanguageRuntime`]).
    Application,
    /// Pure data / markup / build-DSL format - no runtime to probe.
    DataFormat,
}

/// The runtime toolchain(s) an application language requires.
///
/// Each entry lists one primary runtime command plus an ordered fallback
/// chain; presence is satisfied when the primary (or a fallback, when
/// declared) resolves on `PATH` (FR-006). Version capture uses each
/// command's per-tool version flag (FR-008 notes in SPEC FR-006 table).
#[derive(Debug, Clone, Copy)]
pub struct LanguageRuntime {
    /// Ordered runtime commands to probe (primary first, then fallbacks).
    pub commands: &'static [&'static str],
}

impl LanguageRuntime {
    /// Convenience constructor for the multi-command case.
    pub const fn many(commands: &'static [&'static str]) -> Self {
        Self { commands }
    }
}

/// Per-tool version flag, replacing the `--version` default.
///
/// Most tools report a version on stdout with `--version`; the exceptions
/// below need their own flag (see SPEC FR-008 notes).
#[derive(Debug, Clone, Copy)]
pub struct VersionFlag {
    /// Tool command the flag applies to (compile-time constant).
    pub command: &'static str,
    /// The flag argument that makes the tool print its version.
    pub flag: &'static str,
}

/// Version-flag exceptions (command, flag). Everything else defaults to
/// `--version`.
///
/// Go and Zig have no `--version` flag; their versions are subcommands
/// (`go version`, `zig version`).
pub const VERSION_FLAG_OVERRIDES: &[VersionFlag] = &[
    VersionFlag {
        command: "java",
        flag: "-version",
    },
    VersionFlag {
        command: "erl",
        flag: "-version",
    },
    VersionFlag {
        command: "lua",
        flag: "-v",
    },
    VersionFlag {
        command: "luajit",
        flag: "-v",
    },
    VersionFlag {
        command: "go",
        flag: "version",
    },
    VersionFlag {
        command: "zig",
        flag: "version",
    },
];

/// Multi-argument version probes (command, argv). Everything else uses the
/// single flag from [`version_flag_for`].
///
/// `dotnet --version` prints only the active SDK version (and fails when no
/// SDK is installed), so the dotnet probe combines `--list-sdks` and
/// `--list-runtimes` into one line.
pub const VERSION_PROBE_ARGS_OVERRIDES: &[(&str, &[&str])] =
    &[("dotnet", &["--list-sdks", "--list-runtimes"])];

/// Per-language classification and runtime mapping.
///
/// The table must cover every entry of [`SUPPORTED_LANGUAGES`] exactly once
/// (guaranteed by T-002's exhaustiveness check); ids are the canonical
/// scanner ids, not user aliases.
pub const LANGUAGE_RUNTIMES: &[(&str, LanguageClass, Option<LanguageRuntime>)] = &[
    // -- Application languages --------------------------------------------
    (
        "rust",
        LanguageClass::Application,
        Some(LanguageRuntime::many(&["cargo", "rustc"])),
    ),
    (
        "python",
        LanguageClass::Application,
        Some(LanguageRuntime::many(&["python3"])),
    ),
    (
        "typescript",
        LanguageClass::Application,
        Some(LanguageRuntime::many(&["node", "tsc"])),
    ),
    (
        "tsx",
        LanguageClass::Application,
        Some(LanguageRuntime::many(&["node", "tsc"])),
    ),
    (
        "javascript",
        LanguageClass::Application,
        Some(LanguageRuntime::many(&["node"])),
    ),
    (
        "jsx",
        LanguageClass::Application,
        Some(LanguageRuntime::many(&["node"])),
    ),
    (
        "go",
        LanguageClass::Application,
        Some(LanguageRuntime::many(&["go"])),
    ),
    // cc is the portable compiler-driver name; gcc/clang are fallbacks.
    (
        "c",
        LanguageClass::Application,
        Some(LanguageRuntime::many(&["cc", "gcc", "clang"])),
    ),
    (
        "c_header",
        LanguageClass::Application,
        Some(LanguageRuntime::many(&["cc", "gcc", "clang"])),
    ),
    (
        "cpp",
        LanguageClass::Application,
        Some(LanguageRuntime::many(&["c++", "g++", "clang++"])),
    ),
    (
        "cpp_header",
        LanguageClass::Application,
        Some(LanguageRuntime::many(&["c++", "g++", "clang++"])),
    ),
    (
        "java",
        LanguageClass::Application,
        Some(LanguageRuntime::many(&["java", "javac"])),
    ),
    (
        "kotlin",
        LanguageClass::Application,
        Some(LanguageRuntime::many(&["kotlinc"])),
    ),
    (
        "ruby",
        LanguageClass::Application,
        Some(LanguageRuntime::many(&["ruby"])),
    ),
    (
        "swift",
        LanguageClass::Application,
        Some(LanguageRuntime::many(&["swift"])),
    ),
    (
        "csharp",
        LanguageClass::Application,
        Some(LanguageRuntime::many(&["dotnet"])),
    ),
    (
        "lua",
        LanguageClass::Application,
        Some(LanguageRuntime::many(&["lua", "luajit"])),
    ),
    (
        "shell",
        LanguageClass::Application,
        Some(LanguageRuntime::many(&["bash"])),
    ),
    (
        "zsh",
        LanguageClass::Application,
        Some(LanguageRuntime::many(&["zsh"])),
    ),
    (
        "fish",
        LanguageClass::Application,
        Some(LanguageRuntime::many(&["fish"])),
    ),
    (
        "zig",
        LanguageClass::Application,
        Some(LanguageRuntime::many(&["zig"])),
    ),
    (
        "nim",
        LanguageClass::Application,
        Some(LanguageRuntime::many(&["nim"])),
    ),
    (
        "elixir",
        LanguageClass::Application,
        Some(LanguageRuntime::many(&["elixir"])),
    ),
    (
        "erlang",
        LanguageClass::Application,
        Some(LanguageRuntime::many(&["erl"])),
    ),
    (
        "haskell",
        LanguageClass::Application,
        Some(LanguageRuntime::many(&["ghc", "runghc"])),
    ),
    (
        "ocaml",
        LanguageClass::Application,
        Some(LanguageRuntime::many(&["ocaml"])),
    ),
    (
        "r",
        LanguageClass::Application,
        Some(LanguageRuntime::many(&["R"])),
    ),
    (
        "dart",
        LanguageClass::Application,
        Some(LanguageRuntime::many(&["dart"])),
    ),
    (
        "php",
        LanguageClass::Application,
        Some(LanguageRuntime::many(&["php"])),
    ),
    (
        "perl",
        LanguageClass::Application,
        Some(LanguageRuntime::many(&["perl"])),
    ),
    // -- Data / markup / build-DSL formats (no runtime probed) ------------
    ("toml", LanguageClass::DataFormat, None),
    ("yaml", LanguageClass::DataFormat, None),
    ("json", LanguageClass::DataFormat, None),
    ("xml", LanguageClass::DataFormat, None),
    ("html", LanguageClass::DataFormat, None),
    ("css", LanguageClass::DataFormat, None),
    ("scss", LanguageClass::DataFormat, None),
    ("sql", LanguageClass::DataFormat, None),
    ("markdown", LanguageClass::DataFormat, None),
    ("protobuf", LanguageClass::DataFormat, None),
    ("verilog", LanguageClass::DataFormat, None),
    ("vhdl", LanguageClass::DataFormat, None),
    ("terraform", LanguageClass::DataFormat, None),
    ("openscad", LanguageClass::DataFormat, None),
    ("cmake", LanguageClass::DataFormat, None),
    ("gradle", LanguageClass::DataFormat, None),
    ("gradle_kts", LanguageClass::DataFormat, None),
    ("maven", LanguageClass::DataFormat, None),
    ("nix", LanguageClass::DataFormat, None),
    ("hcl", LanguageClass::DataFormat, None),
];

/// Verify the FR-005 exhaustiveness guarantee of [`LANGUAGE_RUNTIMES`] against
/// [`SUPPORTED_LANGUAGES`], returning one human-readable error per violation.
///
/// A mapping is exhaustive when it holds all of the following:
///
/// - every `SUPPORTED_LANGUAGES` id has exactly one mapping row (no gaps, no
///   duplicates), and
/// - every mapping row references an id from `SUPPORTED_LANGUAGES` (no rows
///   outside the scanner list), and
/// - the partition matches FR-005: exactly 30 application rows and exactly 20
///   data-format rows.
///
/// Returns an empty `Vec` when the guarantee holds. Called by the tests as the
/// CI-detectable guard, and reusable at runtime by any caller that wants to
/// assert the invariant before walking the table.
pub fn exhaustiveness_errors() -> Vec<String> {
    let mut errors: Vec<String> = Vec::new();

    // Every scanner id must map exactly once.
    for lang in SUPPORTED_LANGUAGES {
        let match_count = LANGUAGE_RUNTIMES
            .iter()
            .filter(|(id, _, _)| *id == *lang)
            .count();
        if match_count == 0 {
            errors.push(format!(
                "language {lang:?} in SUPPORTED_LANGUAGES has no mapping row"
            ));
        } else if match_count > 1 {
            errors.push(format!(
                "language {lang:?} has {match_count} mapping rows, expected exactly 1"
            ));
        }
    }

    // Every mapping row must reference a scanner id.
    for (id, _, _) in LANGUAGE_RUNTIMES {
        if !SUPPORTED_LANGUAGES.contains(id) {
            errors.push(format!("mapping id {id:?} is not in SUPPORTED_LANGUAGES"));
        }
    }

    // The partition must match FR-005 (30 application / 20 data-format).
    let application = LANGUAGE_RUNTIMES
        .iter()
        .filter(|(_, class, _)| *class == LanguageClass::Application)
        .count();
    let data_format = LANGUAGE_RUNTIMES
        .iter()
        .filter(|(_, class, _)| *class == LanguageClass::DataFormat)
        .count();
    if application != 30 {
        errors.push(format!(
            "application-language row count is {application}, expected 30 (FR-005)"
        ));
    }
    if data_format != 20 {
        errors.push(format!(
            "data-format row count is {data_format}, expected 20 (FR-005)"
        ));
    }

    errors
}

/// Walk [`SUPPORTED_LANGUAGES`] in list order, resolving each id through
/// [`LANGUAGE_RUNTIMES`].
///
/// This is the FR-004/FR-005 derivation API: the report iterates the scanner
/// const and classifies via the single mapping table rather than any second
/// hard-coded language list. A row resolves to `None` only when the mapping is
/// non-exhaustive (guarded by [`exhaustiveness_errors`]).
#[must_use]
pub fn supported_language_rows() -> impl Iterator<
    Item = (
        &'static str,
        Option<(LanguageClass, Option<LanguageRuntime>)>,
    ),
> {
    SUPPORTED_LANGUAGES.iter().map(|&id| (id, lookup(id)))
}

/// Look up the classification and runtime for a canonical language id.
///
/// Returns `None` when the id has no mapping row - a signal that a new
/// `SUPPORTED_LANGUAGES` entry was added without extending this table
/// (guarded by the T-002 exhaustiveness check).
#[must_use]
pub fn lookup(id: &str) -> Option<(LanguageClass, Option<LanguageRuntime>)> {
    LANGUAGE_RUNTIMES
        .iter()
        .find(|(lang_id, _, _)| *lang_id == id)
        .map(|(_, class, runtime)| (*class, runtime.as_ref().copied()))
}

/// Whether a language id is an application language (runtime probed).
#[must_use]
pub fn is_application(id: &str) -> bool {
    matches!(lookup(id), Some((LanguageClass::Application, _)))
}

/// The version flag for a runtime command (default `--version`).
#[must_use]
pub fn version_flag_for(command: &str) -> &'static str {
    VERSION_FLAG_OVERRIDES
        .iter()
        .find(|o| o.command == command)
        .map_or("--version", |o| o.flag)
}

/// Resolve a command name against a list of `PATH` directories
/// (FR-007, `command -v` semantics) without spawning any process.
///
/// Returns the full path of the first matching executable file, or `None`
/// when the command does not resolve. On Unix the file must carry an
/// execute bit; on Windows the file must exist and match a `PATHEXT`
/// extension (an explicit extension in the command is honoured as-is).
fn resolve_in_dirs(command: &str, dirs: &[std::path::PathBuf]) -> Option<std::path::PathBuf> {
    use std::path::Path;

    if command.is_empty() || command.contains('/') {
        // Absolute/relative paths are not probed; only bare command names.
        return None;
    }

    let candidate_names: Vec<String> = if cfg!(windows) {
        let ext = Path::new(command)
            .extension()
            .map_or_else(|| true, |e| e.is_empty());
        if ext {
            let pathext = std::env::var("PATHEXT").unwrap_or_else(|_| ".COM;.EXE;.BAT".into());
            pathext
                .split(';')
                .filter(|s| !s.is_empty())
                .map(|e| format!("{command}{e}"))
                .collect()
        } else {
            vec![command.to_string()]
        }
    } else {
        vec![command.to_string()]
    };

    for dir in dirs {
        for name in &candidate_names {
            let candidate = dir.join(name);
            let Ok(meta) = std::fs::metadata(&candidate) else {
                continue;
            };
            if !meta.is_file() {
                continue;
            }
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                if meta.permissions().mode() & 0o111 != 0 {
                    return Some(candidate);
                }
            }
            #[cfg(not(unix))]
            {
                return Some(candidate);
            }
        }
    }
    None
}

/// Probe whether a runtime command is installed, by `PATH` resolution.
///
/// Presence is determined purely in-process (FR-007): the command name is
/// resolved against the `PATH` environment variable with `command -v`
/// semantics - no tool (or `which`) is spawned and no network access is
/// used. `Some(path)` means `installed`, `None` means `not installed`.
#[must_use]
pub fn probe_installed(command: &str) -> Option<std::path::PathBuf> {
    let dirs: Vec<std::path::PathBuf> = std::env::split_paths(&std::env::var_os("PATH")?).collect();
    resolve_in_dirs(command, &dirs)
}

// ---------------------------------------------------------------------------
// Version probe (T-004, FR-008 / FR-012)
// ---------------------------------------------------------------------------

/// Per-probe timeout for version capture (FR-012).
pub const VERSION_PROBE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(2);

/// Outcome of a version probe (FR-008 / FR-012).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VersionResult {
    /// First non-empty line captured from stdout or stderr.
    Text(String),
    /// Probe could not start, or produced no usable version text.
    Unknown,
    /// Probe exceeded [`VERSION_PROBE_TIMEOUT`] and was killed.
    Timeout,
}

impl VersionResult {
    /// Map to the version string the report carries (FR-012): captured text
    /// for [`VersionResult::Text`], the `unknown` / `timeout` placeholders
    /// otherwise, so an installed-but-unversioned runtime still reports
    /// `installed` rather than `-`.
    #[must_use]
    pub fn placeholder_or_text(self) -> Option<String> {
        match self {
            Self::Text(text) => Some(text),
            Self::Unknown => Some("unknown".to_string()),
            Self::Timeout => Some("timeout".to_string()),
        }
    }
}

/// How many times [`spawn_with_busy_retry`] attempts a spawn before giving up.
///
/// A busy executable clears as soon as the holding descriptor is closed, so a
/// handful of attempts separated by [`BUSY_RETRY_BACKOFF`] spans far more than
/// the microsecond-scale window that produces `ETXTBSY`.
const BUSY_SPAWN_ATTEMPTS: u32 = 8;

/// Delay between `ETXTBSY` spawn attempts.
const BUSY_RETRY_BACKOFF: std::time::Duration = std::time::Duration::from_millis(2);

/// Spawn `cmd`, retrying while the executable is reported busy.
///
/// `ETXTBSY` means the kernel refused `execve` because some process still holds
/// a write descriptor on the program file; the descriptor may belong to an
/// unrelated thread and was merely inherited by that thread's `fork`. The
/// condition is therefore transient and a retry succeeds once the writer
/// closes. Every other spawn error is returned immediately - retrying it would
/// only delay reporting a missing or non-executable program.
fn spawn_with_busy_retry(cmd: &mut std::process::Command) -> std::io::Result<std::process::Child> {
    let mut attempt = 1;
    loop {
        match cmd.spawn() {
            Err(err)
                if err.kind() == std::io::ErrorKind::ExecutableFileBusy
                    && attempt < BUSY_SPAWN_ATTEMPTS =>
            {
                attempt += 1;
                std::thread::sleep(BUSY_RETRY_BACKOFF);
            }
            other => return other,
        }
    }
}

/// Run one bounded version probe and return the raw spawn/timeout outcome.
///
/// Shared engine behind [`probe_version_args`] and
/// [`probe_version_args_all_lines`]: spawns `program args`, polls with
/// `try_wait` on a worker thread so a hung child is killed at the deadline
/// instead of blocking the caller forever (FR-012), and hands the result to
/// the caller over a channel. The outcome is one of:
///
/// - `Ok(Some(Ok(output)))` - a completed probe carrying the pipes' content;
/// - `Ok(Some(Err(io)))` - spawn failure reported by the worker;
/// - `Ok(None)` - the worker's timed-out send (output discarded);
/// - `Err(RecvTimeoutError)` - the worker never reported within the bounded
///   receive window.
///
/// The receive is bounded by [`VERSION_PROBE_TIMEOUT`] plus a slack margin;
/// on a receive timeout the worker thread is **detached, not joined** - the
/// non-timeout path reads the pipes to EOF via `wait_with_output()`, and a
/// grandchild inheriting them (shell wrappers fork) can block that read
/// indefinitely. Joining would hang `/toolchain list` past its own bound;
/// a detached leak of one short-lived thread per timed-out receive is the
/// safe containment.
#[must_use]
fn run_probe_raw(
    program: &str,
    args: &[&str],
) -> Result<Option<Result<std::process::Output, std::io::Error>>, std::sync::mpsc::RecvTimeoutError>
{
    let mut cmd = std::process::Command::new(program);
    cmd.args(args)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());

    // A spawn failure never reaches the worker: report it via the same
    // channel shape so callers keep distinguishing Unknown from Timeout.
    //
    // `ETXTBSY` ("Text file busy") is retried because it is transient, not a
    // property of the command: the kernel refuses `execve` when any process
    // holds a write descriptor open on the file, and a descriptor opened by an
    // unrelated thread is inherited by every child that thread forks. A freshly
    // written executable - a just-installed tool, or a test fixture written
    // moments before the probe - therefore fails intermittently whenever a
    // sibling thread spawns a process while the writer's descriptor is open.
    // The window is microseconds wide, so a short bounded retry resolves it;
    // the alternative is reporting `unknown` for a runtime that is present and
    // healthy.
    let mut child = match spawn_with_busy_retry(&mut cmd) {
        Ok(child) => child,
        Err(err) => return Ok(Some(Err(err))),
    };

    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let deadline = std::time::Instant::now() + VERSION_PROBE_TIMEOUT;
        let mut timed_out = false;
        loop {
            match child.try_wait() {
                Ok(Some(_)) => break,
                Ok(None) => {
                    if std::time::Instant::now() >= deadline {
                        let _ = child.kill(); // INTENTIONAL: process teardown is best-effort
                        let _ = child.wait(); // INTENTIONAL: process teardown is best-effort
                        timed_out = true;
                        break;
                    }
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
                Err(_) => break,
            }
        }
        if timed_out {
            // A killed direct child may leave grandchildren holding the
            // stdout/stderr pipes (shell wrappers fork); reading to EOF could
            // block until those exit. Timeout discards the output, so send
            // without draining the pipes (FR-012 containment).
            let _ = tx.send(None); // INTENTIONAL: channel send on a closed receiver is benign
            return;
        }
        let output = child.wait_with_output();
        let _ = tx.send(Some(output)); // INTENTIONAL: channel send on a closed receiver is benign
    });

    rx.recv_timeout(VERSION_PROBE_TIMEOUT + std::time::Duration::from_secs(2))
}

/// Capture the version of an installed runtime command with explicit
/// arguments.
///
/// `program` may be a bare command name or the path returned by
/// [`probe_installed`]; the walk layer (T-011) passes the resolved path so no
/// second `PATH` lookup is needed. The version flag must be supplied
/// explicitly here (the name-keyed per-tool overrides of
/// [`version_flag_for`] do not match full paths, so callers apply
/// `version_flag_for(command_name)` themselves).
#[must_use]
pub fn probe_version_args(program: &str, args: &[&str]) -> VersionResult {
    match run_probe_raw(program, args) {
        // `None` is the worker's explicit "timed out, output discarded" send.
        Ok(None) => VersionResult::Timeout,
        Ok(Some(Ok(output))) => {
            let text = all_non_empty_lines(&output.stdout)
                .into_iter()
                .next()
                .or_else(|| all_non_empty_lines(&output.stderr).into_iter().next());
            match text {
                Some(line) => VersionResult::Text(line),
                None => VersionResult::Unknown,
            }
        }
        Ok(Some(Err(_))) => VersionResult::Unknown,
        // Receive timeout: the worker never reported; scheduler pathology.
        Err(_) => VersionResult::Timeout,
    }
}

// ---------------------------------------------------------------------------
// Full walk (T-011, FR-004 / FR-006 / FR-009 / FR-010)
// ---------------------------------------------------------------------------

/// Probe every supported language and assemble the full report rows.
///
/// The walk iterates [`SUPPORTED_LANGUAGES`] in list order through
/// [`supported_language_rows`] (FR-004 - no second hard-coded language
/// list). Application languages probe each runtime command in FR-006 order
/// via [`probe_installed`] (FR-007 PATH resolution, no spawn) and - for
/// commands found - [`probe_command_version`] with the resolved binary path
/// and the per-tool flag from [`version_flag_for`] (FR-008, bounded by
/// [`VERSION_PROBE_TIMEOUT`] FR-012). Data-format rows carry empty probes
/// and render the FR-005 marker. A missing runtime reports `not installed`
/// with a `-` version and never aborts or truncates the walk (FR-010).
///
/// Runs entirely in-process; the caller executes it on a blocking thread
/// (FR-016). Read-only: PATH resolution and version probes modify no state
/// (FR-013).
#[must_use]
pub fn build_report_rows() -> Vec<ReportRow> {
    supported_language_rows()
        .map(|(id, resolved)| {
            let Some((class, runtime)) = resolved else {
                // Non-exhaustive mapping (guarded by `exhaustiveness_errors`
                // in CI): emit a data-format-shaped row so the walk still
                // produces one row per scanner entry.
                return ReportRow {
                    id,
                    class: LanguageClass::DataFormat,
                    probes: Vec::new(),
                };
            };
            let probes = runtime
                .map(|rt| {
                    rt.commands
                        .iter()
                        .copied()
                        .map(probe_command)
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            ReportRow { id, class, probes }
        })
        .collect()
}

/// Probe one runtime command: PATH presence first, version only when found.
fn probe_command(command: &'static str) -> CommandProbe {
    let Some(path) = probe_installed(command) else {
        // FR-010: absent runtimes cost one PATH walk, no spawn, and never
        // abort the walk.
        return CommandProbe {
            command,
            installed: false,
            version: None,
        };
    };
    let version =
        probe_command_version(command, path.to_string_lossy().as_ref()).placeholder_or_text();
    CommandProbe {
        command,
        installed: true,
        version,
    }
}

/// All non-empty, trimmed lines of a byte stream, if any.
///
/// Used by the multi-argument probes ([`VERSION_PROBE_ARGS_OVERRIDES`]) whose
/// commands print one version per line (e.g. `dotnet --list-sdks` reports
/// every installed SDK, not just the active one).
fn all_non_empty_lines(bytes: &[u8]) -> Vec<String> {
    String::from_utf8_lossy(bytes)
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

/// Capture the version of an installed runtime command with explicit
/// arguments, keeping every non-empty output line.
///
/// Behaves like [`probe_version_args`] except that multi-line output is
/// joined into one `, ` separated string instead of keeping only the first
/// line. Needed for probes such as `dotnet --list-sdks` where every line is
/// a distinct installed version.
fn probe_version_args_all_lines(program: &str, args: &[&str]) -> VersionResult {
    match run_probe_raw(program, args) {
        // `None` is the worker's explicit "timed out, output discarded" send.
        Ok(None) => VersionResult::Timeout,
        Ok(Some(Ok(output))) => {
            let mut lines = all_non_empty_lines(&output.stdout);
            if lines.is_empty() {
                lines = all_non_empty_lines(&output.stderr);
            }
            if lines.is_empty() {
                VersionResult::Unknown
            } else {
                VersionResult::Text(lines.join(", "))
            }
        }
        Ok(Some(Err(_))) => VersionResult::Unknown,
        // Receive timeout: the worker never reported; scheduler pathology.
        Err(_) => VersionResult::Timeout,
    }
}

/// Version probe for one resolved runtime command.
///
/// Most commands run `<path> <flag>` with the per-tool flag from
/// [`version_flag_for`]. Commands listed in [`VERSION_PROBE_ARGS_OVERRIDES`]
/// need multiple probes (e.g. `dotnet --list-sdks` + `--list-runtimes`);
/// every non-empty line of each probe's output is kept and the combined
/// output is joined into one `, ` separated version string, keeping the
/// first [`VersionResult::Timeout`] when any probe overruns. Keeping every
/// line matters for `dotnet`: `--list-sdks`/`--list-runtimes` print one
/// installed version per line and only the first line would drop the newer
/// SDKs.
fn probe_command_version(command: &'static str, program: &str) -> VersionResult {
    let Some((_, arg_sets)) = VERSION_PROBE_ARGS_OVERRIDES
        .iter()
        .find(|(probe_command, _)| *probe_command == command)
    else {
        return probe_version_args(program, &[version_flag_for(command)]);
    };
    let mut parts: Vec<String> = Vec::new();
    let mut timed_out = false;
    for args in arg_sets.iter() {
        match probe_version_args_all_lines(program, &[args]) {
            VersionResult::Text(text) => parts.push(text),
            VersionResult::Timeout => timed_out = true,
            VersionResult::Unknown => {}
        }
    }
    if !parts.is_empty() {
        VersionResult::Text(parts.join(", "))
    } else if timed_out {
        VersionResult::Timeout
    } else {
        VersionResult::Unknown
    }
}

// ---------------------------------------------------------------------------
// Report model + markdown renderer (T-005, FR-009 / FR-005)
// ---------------------------------------------------------------------------

/// Probe outcome for a single runtime command.
///
/// Produced by the T-011 walk (presence via [`probe_installed`], version via
/// the T-004 probe helper); the renderer consumes these as-is.
#[derive(Debug, Clone)]
pub struct CommandProbe {
    /// Runtime command that was probed (FR-006 order, primary first).
    pub command: &'static str,
    /// Whether the command resolved on `PATH` (FR-007).
    pub installed: bool,
    /// Captured version text (FR-008); `None` renders the `-` placeholder.
    pub version: Option<String>,
}

/// One row of the `/toolchain list` report: a language id plus its probe
/// outcomes.
#[derive(Debug, Clone)]
pub struct ReportRow {
    /// Canonical language id (a `SUPPORTED_LANGUAGES` entry).
    pub id: &'static str,
    /// Classification from the mapping table (FR-005).
    pub class: LanguageClass,
    /// Per-command probes in FR-006 order; empty for data-format rows.
    pub probes: Vec<CommandProbe>,
}

/// Marker rendered in the Status column of data-format rows (FR-005).
pub const DATA_FORMAT_MARKER: &str = "(data format - runtime n/a)";

/// Compute the (runtime, status, version) cell triple for one report row.
///
/// Shared by the markdown and JSON renderers so both surfaces render probe
/// outcomes identically (FR-005 data-format marker, FR-006 multi-runtime
/// per-command status, FR-010 not-installed placeholders).
fn report_fields(row: &ReportRow) -> (String, String, String) {
    match row.class {
        LanguageClass::DataFormat => (
            "-".to_string(),
            DATA_FORMAT_MARKER.to_string(),
            "-".to_string(),
        ),
        LanguageClass::Application => {
            if row.probes.len() <= 1 {
                let probe = row.probes.first();
                let runtime = probe.map_or("-", |p| p.command).to_string();
                let status = match probe {
                    Some(p) if p.installed => "installed",
                    _ => "not installed",
                }
                .to_string();
                let version = probe
                    .and_then(|p| p.version.clone())
                    .unwrap_or_else(|| "-".to_string());
                (runtime, status, version)
            } else {
                let runtime = row
                    .probes
                    .iter()
                    .map(|p| p.command)
                    .collect::<Vec<_>>()
                    .join(", ");
                let status = row
                    .probes
                    .iter()
                    .map(|p| {
                        format!(
                            "{}: {}",
                            p.command,
                            if p.installed {
                                "installed"
                            } else {
                                "not installed"
                            }
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("; ");
                let version = row
                    .probes
                    .iter()
                    .map(|p| format!("{}: {}", p.command, p.version.as_deref().unwrap_or("-")))
                    .collect::<Vec<_>>()
                    .join("; ");
                (runtime, status, version)
            }
        }
    }
}

/// Count application rows and those with at least one installed runtime.
///
/// Returns `(installed, application)` - the markdown summary line and the
/// JSON `installed`/`total` fields share this counter (FR-009 / FR-015).
fn installed_summary(rows: &[ReportRow]) -> (usize, usize) {
    let mut installed = 0usize;
    let mut application = 0usize;
    for row in rows {
        if matches!(row.class, LanguageClass::Application) {
            application += 1;
            if row.probes.iter().any(|p| p.installed) {
                installed += 1;
            }
        }
    }
    (installed, application)
}

/// Render the `/toolchain list` report as a Markdown table (FR-009).
///
/// The message is prefixed `From: /toolchain list`, followed by the
/// Language / Runtime / Status / Version table (rows in the given order) and
/// the `<n>/<m> application runtimes installed.` summary line, where `m` is
/// the number of application rows and `n` the number of those with at least
/// one installed runtime. Data-format rows show [`DATA_FORMAT_MARKER`] in
/// place of presence/version (FR-005); absent runtimes render `not installed`
/// with a `-` version (FR-010); rows with multiple runtime commands list the
/// per-command status (FR-006).
///
/// A Markdown table is the one layout that survives both frontends: the GUI's
/// Markdown viewer sizes the columns itself, and the plain-text renderer's
/// html2text pass draws the same table as an ASCII grid. Every cell is escaped
/// by [`mdtable`](codemason_types::mdtable), so a captured version string
/// (FR-008) carrying a `|` or a line break cannot split its row.
#[must_use]
pub fn render_markdown_report(rows: &[ReportRow]) -> String {
    let table_rows: Vec<Vec<String>> = rows
        .iter()
        .map(|row| {
            let (runtime, status, version) = report_fields(row);
            // Cells go in raw: `mdtable` escapes the pipes and line breaks a
            // captured version string can carry (FR-009).
            vec![row.id.to_string(), runtime, status, version]
        })
        .collect();
    let (installed, application) = installed_summary(rows);
    // The blank line before the summary is required: Markdown reads a line
    // directly after a table as one more table row.
    format!(
        "From: /toolchain list\n\n{}\n{installed}/{application} application runtimes installed.\n",
        codemason_types::mdtable::table(&["Language", "Runtime", "Status", "Version"], table_rows)
    )
}

/// Render the `/toolchain list --json` report as a JSON document (FR-015).
///
/// Emits the bare JSON document with no `From:` prefix so the output can be
/// piped or parsed directly (TC-009). The `languages` array carries one
/// object per report row with `id`, `runtime`, `status`, and `version`
/// fields rendered identically to the markdown table cells (data-format rows
/// included, FR-005); `installed` and `total` summarise the application-class
/// rows exactly like the markdown summary line.
#[must_use]
pub fn render_json_report(rows: &[ReportRow]) -> String {
    let (installed, total) = installed_summary(rows);
    let languages = rows
        .iter()
        .map(|row| {
            let (runtime, status, version) = report_fields(row);
            serde_json::json!({
                "id": row.id,
                "runtime": runtime,
                "status": status,
                "version": version,
            })
        })
        .collect::<Vec<_>>();
    let doc = serde_json::json!({
        "languages": languages,
        "installed": installed,
        "total": total,
    });
    serde_json::to_string_pretty(&doc).unwrap_or_else(|_| "{}".to_string())
}

#[cfg(test)]
#[path = "../../tests/inline/toolchain_tests.rs"]
mod tests;
