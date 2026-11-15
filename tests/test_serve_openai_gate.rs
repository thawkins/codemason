//! Refuse-to-start gate for the OpenAI-compatible surface (spec `openhands`
//! T-018; FR-024, FR-032).
//!
//! FR-032 requires `codemason serve` to refuse to start the OpenAI-compatible
//! surface when it is enabled without a configured bearer token, rather than
//! serving it unauthenticated. This is a *startup* decision made in the binary
//! (`src/main.rs`, the `Serve` arm) before the router is built, so it is
//! exercised here by spawning the built binary - the same pattern the other
//! `codemason <subcommand>` CLI tests use.
//!
//! The token lives in a **trusted** config (`--config`), not the project-local
//! `.codemason/codemason.json`: the project overlay is untrusted repository content
//! and its `openai` section is stripped (SEC-codemason-config), so a project file
//! could never enable or arm the surface on its own.

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

/// How long to let the server start before treating it as "still running"
/// (i.e. it did not refuse) and killing it. Startup is sub-second; this is
/// deliberately generous for a loaded CI host.
const SERVE_TIMEOUT: Duration = Duration::from_secs(15);

/// Whether the `serve` child exited on its own or was still running when the
/// deadline passed (the latter means the surface started).
enum ServeOutcome {
    /// The process exited with `code`; its stderr is captured.
    Exited { code: Option<i32>, stderr: String },
    /// The process was still running at the deadline; it was killed.
    StillRunning,
}

/// An isolated working directory plus a private `HOME`/`XDG_CONFIG_HOME`, so a
/// developer's own config and the project-local `.codemason/` cannot influence the
/// child's startup decision.
struct Fixture {
    temp: tempfile::TempDir,
}

impl Fixture {
    fn new() -> Self {
        Self {
            temp: tempfile::tempdir().expect("tempdir"),
        }
    }

    /// Write a trusted config file and return its path.
    fn trusted_config(&self, json: &str) -> PathBuf {
        let path = self.temp.path().join("trusted.json");
        std::fs::write(&path, json).expect("write trusted config");
        path
    }

    /// Spawn `codemason serve --addr 127.0.0.1:0 --config <trusted>`, bounded by
    /// [`SERVE_TIMEOUT`]. Credential-bearing env vars are stripped so the
    /// surface token can only come from the trusted config.
    fn serve(&self, trusted: &Path) -> ServeOutcome {
        let home = self.temp.path().join("home");
        let config_home = self.temp.path().join("config");
        std::fs::create_dir_all(&home).expect("home dir");
        std::fs::create_dir_all(&config_home).expect("config dir");

        let mut child: Child = Command::new(env!("CARGO_BIN_EXE_cdmn"))
            .args(["serve", "--addr", "127.0.0.1:0", "--config"])
            .arg(trusted)
            .current_dir(self.temp.path())
            .env("HOME", &home)
            .env("XDG_CONFIG_HOME", &config_home)
            .env_remove("CODEMASON_TOKEN")
            .env_remove("CODEMASON_CONFIG")
            .env_remove("CODEMASON_CONFIG_CONTENT")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn codemason serve");

        let deadline = Instant::now() + SERVE_TIMEOUT;
        loop {
            match child.try_wait().expect("try_wait") {
                Some(status) => {
                    let output = child.wait_with_output().expect("wait_with_output");
                    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
                    return ServeOutcome::Exited {
                        code: status.code(),
                        stderr,
                    };
                }
                None if Instant::now() >= deadline => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return ServeOutcome::StillRunning;
                }
                None => std::thread::sleep(Duration::from_millis(50)),
            }
        }
    }
}

#[test]
fn serve_refuses_to_start_the_surface_when_enabled_without_a_token() {
    // FR-032: `openai.enabled: true` and no token => the server refuses to start
    // the surface (non-zero exit) and reports why, rather than serving it
    // unauthenticated.
    let fixture = Fixture::new();
    let trusted = fixture.trusted_config(r#"{ "openai": { "enabled": true } }"#);

    match fixture.serve(&trusted) {
        ServeOutcome::Exited { code, stderr } => {
            assert_eq!(
                code,
                Some(1),
                "the server must exit non-zero when the surface is enabled with no token: {stderr}"
            );
            assert!(
                stderr.contains("OpenAI-compatible surface") && stderr.contains("openai.token"),
                "the refusal must explain the missing token: {stderr}"
            );
        }
        ServeOutcome::StillRunning => {
            panic!("the server started the surface without a token (FR-032 violated)");
        }
    }
}

#[test]
fn serve_starts_the_surface_when_a_token_is_configured() {
    // FR-024, FR-032: with a token the surface starts (the process keeps
    // running past the gate) - the refusal is specific to the missing token,
    // not a blanket startup failure.
    let fixture = Fixture::new();
    let trusted = fixture.trusted_config(
        r#"{ "openai": { "enabled": true, "token": "sk-codemason-local-test-0001" } }"#,
    );

    match fixture.serve(&trusted) {
        ServeOutcome::StillRunning => {}
        ServeOutcome::Exited { code, stderr } => {
            panic!(
                "the server should have started with a token, but exited with {code:?}: {stderr}"
            );
        }
    }
}

/// Spawn `cdmn serve` with `trusted`, collect stderr for up to `wait`, then
/// kill it and return what it printed.
fn serve_stderr(fixture: &Fixture, trusted: &Path, wait: Duration) -> String {
    use std::io::Read as _;
    let home = fixture.temp.path().join("home");
    let config_home = fixture.temp.path().join("config");
    std::fs::create_dir_all(&home).expect("home dir");
    std::fs::create_dir_all(&config_home).expect("config dir");
    let mut child: Child = Command::new(env!("CARGO_BIN_EXE_cdmn"))
        .args(["serve", "--addr", "127.0.0.1:0", "--config"])
        .arg(trusted)
        .current_dir(fixture.temp.path())
        .env("HOME", &home)
        .env("XDG_CONFIG_HOME", &config_home)
        .env_remove("CODEMASON_TOKEN")
        .env_remove("CODEMASON_CONFIG")
        .env_remove("CODEMASON_CONFIG_CONTENT")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn codemason serve");
    let mut pipe = child.stderr.take().expect("stderr");
    let reader = std::thread::spawn(move || {
        let mut text = String::new();
        let _ = pipe.read_to_string(&mut text);
        text
    });
    std::thread::sleep(wait);
    let _ = child.kill();
    let _ = child.wait();
    reader.join().expect("stderr reader")
}

#[test]
fn serve_prints_the_generated_token_when_none_is_configured() {
    // SI-02: with no configured token, the generated one must be shown, or
    // every protected route answers 401 with no way to learn the credential.
    let fixture = Fixture::new();
    let trusted = fixture.trusted_config("{}");
    let stderr = serve_stderr(&fixture, &trusted, Duration::from_secs(3));
    let token = stderr
        .lines()
        .skip_while(|line| !line.contains("using a token generated for this run"))
        .nth(1)
        .map(str::trim)
        .unwrap_or_else(|| panic!("the generated token must be printed: {stderr}"));
    assert!(
        uuid::Uuid::parse_str(token).is_ok(),
        "the printed token is the generated UUID: {token:?}"
    );
}

#[test]
fn serve_does_not_print_a_configured_token() {
    let fixture = Fixture::new();
    let trusted =
        fixture.trusted_config(r#"{ "openai": { "token": "sk-codemason-local-test-0002" } }"#);
    let stderr = serve_stderr(&fixture, &trusted, Duration::from_secs(3));
    assert!(
        !stderr.contains("sk-codemason-local-test-0002")
            && !stderr.contains("generated for this run"),
        "a configured token must never be echoed: {stderr}"
    );
}
