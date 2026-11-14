//! Integration tests for the `codemason plugins` CLI parity surface
//! (spec `plugins` T-017; FR-021).
//!
//! Each test spawns the built `codemason` binary inside a fresh temp working
//! directory with an isolated `XDG_CONFIG_HOME`, so the project store
//! (`.codemason/plugins/`) and the user-global store are both contained. The tests
//! assert the non-TUI output spelling (`codemason plugins …` attribution rather
//! than `From: /plugins …`), that every subcommand routes to the shared
//! `codemason-plugins` logic, and that the store round-trip works end to end.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::MutexGuard;

/// Serialise tests that change the process working directory.
/// A temp working directory plus an isolated config/home root. Holds the cwd
/// lock for its lifetime and restores the previous cwd on drop, so a panicking
/// test cannot leave the process cwd inside a deleted tempdir.
struct Fixture {
    temp: tempfile::TempDir,
    prev: PathBuf,
    _lock: MutexGuard<'static, ()>,
}

impl Fixture {
    fn new() -> Self {
        let lock = crate::global_env_lock();
        let temp = tempfile::tempdir().expect("tempdir");
        let prev = std::env::current_dir().expect("cwd");
        std::env::set_current_dir(temp.path()).expect("set cwd");
        Self {
            temp,
            prev,
            _lock: lock,
        }
    }

    fn path(&self) -> &Path {
        self.temp.path()
    }

    /// Run `codemason` with `args` against this fixture's isolated environment.
    fn codemason(&self, args: &[&str]) -> Output {
        let config = self.temp.path().join("config");
        let home = self.temp.path().join("home");
        std::fs::create_dir_all(&config).expect("config dir");
        std::fs::create_dir_all(&home).expect("home dir");
        Command::new(env!("CARGO_BIN_EXE_cdmn"))
            .args(args)
            .env("XDG_CONFIG_HOME", &config)
            .env("HOME", &home)
            .env_remove("GITHUB_TOKEN")
            .env_remove("GITLAB_TOKEN")
            .output()
            .expect("spawn codemason")
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::env::set_current_dir(&self.prev);
    }
}

fn stdout(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).into_owned()
}

/// Write a minimal Codex-dialect plugin (one tool, no commands) into `dir`.
fn write_codex_plugin(dir: &Path, id: &str) {
    std::fs::create_dir_all(dir).expect("plugin dir");
    std::fs::write(
        dir.join("codex-plugin.json"),
        format!(r#"{{ "id": "{id}", "name": "{id}", "version": "1.0.0", "entry": "index.js" }}"#),
    )
    .expect("manifest");
    std::fs::write(
        dir.join("index.js"),
        "codemason.register_tool({ name: \"ping\", description: \"Ping\", \
         parameters: { type: \"object\" }, handler: function(){ return \"pong\"; } });",
    )
    .expect("entry");
}

#[test]
fn bare_and_help_print_the_usage_block() {
    let fx = Fixture::new();

    for args in [vec!["plugins"], vec!["plugins", "help"]] {
        let out = fx.codemason(&args);
        assert!(out.status.success(), "usage must succeed: {args:?}");
        let text = stdout(&out);
        assert!(
            text.contains("## codemason plugins command reference"),
            "usage must carry the CLI heading: {text}"
        );
        // The non-TUI surface spelling, not the TUI `From:` attribution.
        assert!(
            text.contains("`codemason plugins list [--verbose]`"),
            "usage must document the CLI row for list: {text}"
        );
        assert!(
            !text.contains("From: /plugins"),
            "usage must not print the TUI attribution: {text}"
        );
    }
}

#[test]
fn list_on_an_empty_store_reports_none_discovered() {
    let fx = Fixture::new();
    let out = fx.codemason(&["plugins", "list"]);
    assert!(out.status.success());
    let text = stdout(&out);
    assert!(
        text.starts_with("codemason plugins list"),
        "list uses the CLI attribution: {text}"
    );
    assert!(
        text.contains("No plugins discovered."),
        "an empty store reports none: {text}"
    );
}

#[test]
fn store_round_trip_uses_the_cli_surface_spelling() {
    let fx = Fixture::new();

    // Stage a source plugin outside the store.
    let src = fx.path().join("src-plugin");
    write_codex_plugin(&src, "codex-weather");

    // add
    let out = fx.codemason(&["plugins", "add", src.to_str().expect("utf8 src")]);
    assert!(out.status.success(), "add must succeed");
    let added = stdout(&out);
    assert!(added.starts_with("codemason plugins add"), "{added}");
    assert!(
        added.contains("[ok] Installed plugin `codex-weather`"),
        "{added}"
    );
    assert!(
        fx.path()
            .join(".codemason/plugins/codex-weather/codex-plugin.json")
            .exists(),
        "the plugin must be copied into the project store"
    );

    // list shows it enabled (FR-007): add records the enable flag
    let listed = stdout(&fx.codemason(&["plugins", "list"]));
    assert!(listed.contains("codex-weather"), "{listed}");
    assert!(listed.contains("enabled"), "{listed}");

    // enable loads it and reports the contributions (FR-011)
    let enabled = stdout(&fx.codemason(&["plugins", "enable", "codex-weather"]));
    assert!(enabled.starts_with("codemason plugins enable"), "{enabled}");
    assert!(enabled.contains("[ok]"), "{enabled}");
    assert!(
        enabled.contains("registered 1 tool(s)"),
        "enable registers the plugin tool: {enabled}"
    );

    // verbose list names the registered tool (FR-009, FR-022)
    let verbose = stdout(&fx.codemason(&["plugins", "list", "--verbose"]));
    assert!(
        verbose.contains("plugin_codex-weather_ping"),
        "verbose list names the registry tool: {verbose}"
    );
    assert!(verbose.contains("Telemetry:"), "{verbose}");

    // test drives the isolated harness (FR-013)
    let tested = stdout(&fx.codemason(&["plugins", "test", "codex-weather"]));
    assert!(tested.starts_with("codemason plugins test"), "{tested}");
    assert!(tested.contains("[ ok ]"), "{tested}");
    assert!(
        tested.contains("live session untouched"),
        "the harness leaves the session untouched: {tested}"
    );

    // disable unloads it (FR-012)
    let disabled = stdout(&fx.codemason(&["plugins", "disable", "codex-weather"]));
    assert!(
        disabled.starts_with("codemason plugins disable"),
        "{disabled}"
    );
    assert!(disabled.contains("deregistered"), "{disabled}");

    // remove uninstalls it (FR-010)
    let removed = stdout(&fx.codemason(&["plugins", "remove", "codex-weather"]));
    assert!(removed.starts_with("codemason plugins remove"), "{removed}");
    assert!(
        removed.contains("[ok] Removed plugin `codex-weather`"),
        "{removed}"
    );
    assert!(
        !fx.path().join(".codemason/plugins/codex-weather").exists(),
        "remove deletes the plugin directory"
    );
}

#[test]
fn missing_plugin_id_is_reported_as_usage_error() {
    let fx = Fixture::new();
    let out = fx.codemason(&["plugins", "enable"]);
    assert!(
        out.status.success(),
        "a usage error is not a process failure"
    );
    let text = stdout(&out);
    assert!(
        text.contains("[err] Missing <pluginid>."),
        "enable without an id reports the usage error: {text}"
    );
}

#[test]
fn stores_reports_each_effective_endpoint_and_its_source() {
    // FR-031 parity: `codemason plugins stores` lists both stores. With no
    // `plugins.stores` block, each is tagged `default` and names its compiled
    // default endpoint.
    let fx = Fixture::new();
    let text = stdout(&fx.codemason(&["plugins", "stores"]));

    assert!(text.contains("codemason plugins"), "{text}");
    for token in ["codex", "claude"] {
        assert!(
            text.contains(&format!("- {token}: [default] https://")),
            "`{token}` must be tagged default with an https endpoint: {text}"
        );
    }

    // An override present in the project config tags that store `config` and
    // names the configured URL, while the other keeps its default.
    std::fs::create_dir_all(fx.path().join(".codemason")).expect(".codemason dir");
    std::fs::write(
        fx.path().join(".codemason/codemason.json"),
        r#"{ "plugins": { "stores": { "codex": { "url": "https://cfg.example/codex.json" } } } }"#,
    )
    .expect("config writable");

    let overridden = stdout(&fx.codemason(&["plugins", "stores"]));
    assert!(
        overridden.contains("- codex: [config] https://cfg.example/codex.json"),
        "an override must be tagged config: {overridden}"
    );
    assert!(
        overridden.contains("- claude: [default] https://"),
        "the unconfigured store keeps its default: {overridden}"
    );
}

#[test]
fn stores_usage_documents_the_optional_check_flag() {
    // The opt-in `--check` availability probe is documented on the CLI surface,
    // and the default `stores` output stays the pure config report (no
    // availability marker), so the plain report is unchanged.
    let fx = Fixture::new();

    let help = stdout(&fx.codemason(&["plugins", "help"]));
    assert!(
        help.contains("codemason plugins stores") && help.contains("--check"),
        "the usage block must document the --check flag: {help}"
    );

    let plain = stdout(&fx.codemason(&["plugins", "stores"]));
    assert!(
        plain.contains("- codex: [default] https://")
            && plain.contains("- claude: [default] https://"),
        "the plain report lists both defaults: {plain}"
    );
    assert!(
        !plain.contains("available") && !plain.contains("unavailable"),
        "the plain report carries no probe suffix: {plain}"
    );
}

#[test]
fn marketplace_show_lists_each_store_section_with_a_name_url_description_row() {
    // `/plugins marketplace show` parity: one section per store (Codex,
    // Claude), each a table with one row naming the marketplace (name, url,
    // description). With no `plugins.stores` block both rows tag the compiled
    // default endpoint.
    let fx = Fixture::new();
    let text = stdout(&fx.codemason(&["plugins", "marketplace", "show"]));

    assert!(
        text.contains("codemason plugins marketplace show"),
        "{text}"
    );
    assert!(text.contains("## Codex marketplace"), "{text}");
    assert!(text.contains("## Claude marketplace"), "{text}");
    for (token, host) in [
        ("codex", "raw.githubusercontent.com/openai/plugins"),
        (
            "claude",
            "raw.githubusercontent.com/anthropics/claude-plugins-official",
        ),
    ] {
        assert!(
            text.contains(token) && text.contains(host),
            "`{token}` row must name its default endpoint: {text}"
        );
    }
    assert!(text.contains("(default)"), "{text}");

    // A `plugins.stores.codex.url` override names the configured URL with a
    // `(config)` description tag; the unconfigured store keeps its default.
    std::fs::create_dir_all(fx.path().join(".codemason")).expect(".codemason dir");
    std::fs::write(
        fx.path().join(".codemason/codemason.json"),
        r#"{ "plugins": { "stores": { "codex": { "url": "https://cfg.example/codex.json" } } } }"#,
    )
    .expect("config writable");

    let overridden = stdout(&fx.codemason(&["plugins", "marketplace", "show"]));
    assert!(
        overridden.contains("codex | https://cfg.example/codex.json")
            && overridden.contains("Codex plugin marketplace (config)"),
        "the configured marketplace must name its URL: {overridden}"
    );
    assert!(
        overridden.contains("raw.githubusercontent.com/anthropics/claude-plugins-official"),
        "the unconfigured store keeps its default: {overridden}"
    );
}

#[test]
fn bare_marketplace_and_unknown_sub_subcommand_render_usage() {
    // `codemason plugins marketplace` is only valid with an explicit `show`
    // sub-subcommand; a bare or unknown tail renders the usage block.
    let fx = Fixture::new();
    for args in [
        &["plugins", "marketplace"][..],
        &["plugins", "marketplace", "bogus"][..],
    ] {
        let text = stdout(&fx.codemason(args));
        assert!(
            text.contains("## codemason plugins command reference"),
            "`{args:?}` must render usage: {text}"
        );
        assert!(
            !text.contains("## Codex marketplace"),
            "`{args:?}` must not render the show report: {text}"
        );
    }
}

#[test]
fn unknown_subcommand_renders_usage() {
    let fx = Fixture::new();
    let text = stdout(&fx.codemason(&["plugins", "frobnicate"]));
    assert!(
        text.contains("## codemason plugins command reference"),
        "an unknown subcommand renders usage: {text}"
    );
}

#[test]
fn master_switch_disables_the_subsystem() {
    // Acceptance criterion 8 (FR-021 parity): with `plugins.enabled: false` the
    // CLI reports the disabled subsystem and changes no state; `help` still
    // prints usage.
    let fx = Fixture::new();
    std::fs::create_dir_all(fx.path().join(".codemason")).expect(".codemason dir");
    std::fs::write(
        fx.path().join(".codemason/codemason.json"),
        r#"{ "plugins": { "enabled": false } }"#,
    )
    .expect("config writable");

    let listed = stdout(&fx.codemason(&["plugins", "list"]));
    assert!(listed.contains("[err]"), "{listed}");
    assert!(listed.contains("plugins.enabled = false"), "{listed}");

    let enabled = stdout(&fx.codemason(&["plugins", "enable", "ghost"]));
    assert!(enabled.contains("[err]"), "{enabled}");

    let help = stdout(&fx.codemason(&["plugins", "help"]));
    assert!(
        help.contains("## codemason plugins command reference"),
        "help still renders while disabled: {help}"
    );
    assert!(
        !fx.path().join(".codemason/plugins").exists(),
        "the disabled subsystem creates no store directory"
    );
}
