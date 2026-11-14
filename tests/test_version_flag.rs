//! Integration tests for the `--version` / `-V` CLI flag.
//!
//! `codemason --version` and its short form `codemason -V` must print the version and
//! exit 0 *without* starting the TUI and *without* leaving a crash marker the
//! next start would report as an unclean exit. Each test spawns the built
//! `codemason` binary inside a fresh empty tempdir so the crash marker (written
//! relative to the session working directory) is observed in isolation.

use std::process::{Command, Output};

/// Build a `codemason` invocation for `args` that cannot touch the developer's
/// real global state.
///
/// `codemason --version` returns before `Storage::open`, so it never needs the
/// database itself - but `--help` and any other argv reaching dispatch does, and
/// a child pointed at the real `~/.config/codemason/codemason.db` contends with every
/// concurrently running codemason for the one shared file. A private
/// `XDG_CONFIG_HOME`/`HOME` per invocation keeps the run hermetic.
///
/// `cwd` is the process working directory; `state` holds the private config and
/// home roots. They are separate so the working directory stays empty.
fn codemason_command(cwd: &std::path::Path, state: &std::path::Path) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_cdmn"));
    command
        .current_dir(cwd)
        .env("XDG_CONFIG_HOME", state.join("config"))
        .env("HOME", state.join("home"));
    command
}

/// Run `codemason` with `args` inside a fresh empty tempdir and return the tempdir
/// path plus the captured output.
///
/// The private `XDG_CONFIG_HOME`/`HOME` live in a *sibling* of the working
/// directory: the working directory must stay genuinely empty so a command
/// that inspects it (e.g. `codemason new`'s "directory is not empty" guard) sees
/// the state the test set up, not the harness's own files.
fn codemason_in_empty_cwd(args: &[&str]) -> (std::path::PathBuf, Output) {
    let temp = tempfile::tempdir().expect("tempdir");
    let cwd = temp.path().join("cwd");
    let state = temp.path().join("state");
    std::fs::create_dir_all(&cwd).expect("cwd dir");
    std::fs::create_dir_all(&state).expect("state dir");
    let output = codemason_command(&cwd, &state)
        .args(args)
        .output()
        .expect("spawn codemason");
    // Keep the tempdir alive until after assertions by leaking it; the test
    // process is short-lived and the tree is small.
    std::mem::forget(temp);
    (cwd, output)
}

fn stdout(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn stderr(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

/// `codemason --version` prints `codemason <version>` on stdout and exits 0.
#[test]
fn long_version_prints_version_and_exits_zero() {
    let (_dir, out) = codemason_in_empty_cwd(&["--version"]);
    assert_eq!(out.status.code(), Some(0), "stderr: {}", stderr(&out));
    assert_eq!(
        stdout(&out).trim(),
        format!("codemason {}", env!("CARGO_PKG_VERSION"))
    );
}

/// The short form `codemason -V` behaves identically.
#[test]
fn short_version_prints_version_and_exits_zero() {
    let (_dir, out) = codemason_in_empty_cwd(&["-V"]);
    assert_eq!(out.status.code(), Some(0), "stderr: {}", stderr(&out));
    assert_eq!(
        stdout(&out).trim(),
        format!("codemason {}", env!("CARGO_PKG_VERSION"))
    );
}

/// A version query must not leave the impression of an unclean exit. Like
/// `--help`, the process writes a `clean exit` crash marker on its normal
/// return; that marker must never be `running`, which the next start would
/// report as a phantom crash.
#[test]
fn version_flag_leaves_no_unclean_marker() {
    let (dir, out) = codemason_in_empty_cwd(&["-V"]);
    assert_eq!(out.status.code(), Some(0), "stderr: {}", stderr(&out));
    let marker = dir.join("log").join("panics").join("last-crash.json");
    if marker.exists() {
        let text = std::fs::read_to_string(&marker).expect("read marker");
        assert!(
            !text.contains("\"status\": \"running\""),
            "version query left an unclean marker: {text}"
        );
    }
}

/// A second version query must not warn about a phantom previous crash.
#[test]
fn version_flag_does_not_warn_about_previous_crash() {
    let (dir, _first) = codemason_in_empty_cwd(&["-V"]);
    // `codemason_in_empty_cwd` returns `<temp>/cwd`; the private config/home root
    // is its sibling, `<temp>/state`.
    let state = dir.with_file_name("state");
    let second = codemason_command(&dir, &state)
        .args(["--version"])
        .output()
        .expect("spawn codemason");
    assert_eq!(second.status.code(), Some(0));
    let err = stderr(&second);
    assert!(
        !err.contains("previous codemason session"),
        "unexpected phantom crash warning: {err}"
    );
}

/// The version flag is documented in `--help` as a top-level option.
#[test]
fn help_lists_version_flag() {
    let (_dir, out) = codemason_in_empty_cwd(&["--help"]);
    assert_eq!(out.status.code(), Some(0), "stderr: {}", stderr(&out));
    let help = stdout(&out);
    assert!(
        help.contains("--version"),
        "help did not list --version:\n{help}"
    );
}
