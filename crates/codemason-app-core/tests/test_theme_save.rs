//! Tests for writing themes to disk: serialisation round trips, the colour
//! key list the theme editor edits, file-name sanitising, atomic saves, the
//! size limit and the catalog's record of where each theme was loaded from.

use codemason_app_core::theme_tokens::{
    Rgb, ThemeCatalog, ThemeTokens, save_theme, theme_file_name,
};

/// A fresh scratch directory under `target/temp`.
fn scratch() -> tempfile::TempDir {
    crate::scratch_tempdir("theme-save-")
}

/// The built-in theme `name`.
fn builtin(name: &str) -> ThemeTokens {
    ThemeCatalog::builtin()
        .get(name)
        .map(|t| (*t).clone())
        .expect("built-in theme")
}

#[test]
fn test_theme_save_round_trips_every_builtin() {
    let dir = scratch();
    let catalog = ThemeCatalog::builtin();
    let names: Vec<String> = catalog.names().map(str::to_string).collect();
    assert_eq!(names.len(), 9);
    for name in &names {
        let theme = builtin(name);
        save_theme(&theme, &dir.path().join(theme_file_name(name))).expect("save");
    }
    let mut loaded = ThemeCatalog::builtin();
    let warnings = loaded.load_dir(dir.path());
    assert!(
        warnings.iter().all(|w| !w.contains("skipped")),
        "{warnings:?}"
    );
    for name in &names {
        assert_eq!(*loaded.get(name).expect("loaded"), builtin(name), "{name}");
    }
}

#[test]
fn test_theme_save_creates_missing_dir() {
    let dir = scratch();
    let path = dir.path().join("a").join("b").join("nord.json");
    save_theme(&builtin("nord"), &path).expect("save");
    assert!(path.is_file());
}

#[test]
fn test_theme_save_fails_when_parent_is_a_file() {
    let dir = scratch();
    let blocker = dir.path().join("blocker");
    std::fs::write(&blocker, "x").expect("write");
    assert!(save_theme(&builtin("nord"), &blocker.join("nord.json")).is_err());
}

#[cfg(unix)]
#[test]
fn test_theme_save_never_writes_through_a_planted_tmp_symlink() {
    let dir = scratch();
    let victim = dir.path().join("victim.txt");
    std::fs::write(&victim, "keep me").expect("write");
    let path = dir.path().join("nord.json");
    // The old fixed temporary name: a link there must not be followed.
    std::os::unix::fs::symlink(&victim, path.with_extension("json.tmp")).expect("symlink");
    save_theme(&builtin("nord"), &path).expect("save");
    assert_eq!(std::fs::read_to_string(&victim).expect("read"), "keep me");
    assert!(path.is_file());
    let leftovers: Vec<_> = std::fs::read_dir(dir.path())
        .expect("read dir")
        .filter_map(Result::ok)
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.starts_with(".theme-"))
        .collect();
    assert!(leftovers.is_empty(), "temporary files left: {leftovers:?}");
}

#[test]
fn test_theme_save_refuses_a_theme_over_the_load_limit() {
    let dir = scratch();
    let mut theme = builtin("nord");
    theme.name = "n".repeat(64 * 1024);
    let path = dir.path().join("big.json");
    let err = save_theme(&theme, &path).expect_err("over the limit");
    assert!(err.to_string().contains("byte limit"), "{err}");
    assert!(!path.exists(), "nothing is written");
}

#[test]
fn test_theme_colours_lists_every_key() {
    let theme = builtin("codemason-light");
    let colours = theme.colours();
    let keys: Vec<&str> = colours.iter().map(|(k, _)| k.as_str()).collect();
    let mut unique = keys.clone();
    unique.sort_unstable();
    unique.dedup();
    assert_eq!(unique.len(), keys.len(), "keys are unique");
    for key in ["text.base", "accent", "categorical.7", "m3.primary"] {
        assert!(keys.contains(&key), "{key} missing from {keys:?}");
    }
    assert!(!keys.contains(&"name"));
    assert!(!keys.contains(&"dark"));
    let lookup = |k: &str| colours.iter().find(|(key, _)| key == k).map(|(_, c)| *c);
    assert_eq!(lookup("text.base"), Some(theme.text.base));
    assert_eq!(lookup("m3.primary"), Some(theme.roles().primary));

    // Each key edits exactly its own colour. `document-light` stores its own
    // Material roles; in a theme without them the derived roles follow the
    // base colours, so one edit can move several listed roles.
    let theme = builtin("document-light");
    assert!(theme.m3.is_some());
    let colours = theme.colours();
    let keys: Vec<&str> = colours.iter().map(|(k, _)| k.as_str()).collect();
    let lookup = |k: &str| colours.iter().find(|(key, _)| key == k).map(|(_, c)| *c);
    let marker = Rgb(1, 2, 3);
    for key in &keys {
        let mut edited = theme.clone();
        assert!(edited.set_colour(key, marker), "{key}");
        let changed: Vec<String> = edited
            .colours()
            .into_iter()
            .filter(|(k, c)| lookup(k) != Some(*c))
            .map(|(k, _)| k)
            .collect();
        assert_eq!(changed, vec![(*key).to_string()], "{key}");
    }
}

#[test]
fn test_theme_set_colour_m3_materialises_roles() {
    let mut theme = builtin("nord");
    assert!(theme.m3.is_none());
    let derived = theme.roles();
    assert!(theme.set_colour("m3.primary", Rgb(9, 9, 9)));
    let roles = theme.m3.expect("roles stored");
    assert_eq!(roles.primary, Rgb(9, 9, 9));
    assert_eq!(roles.on_primary, derived.on_primary);
}

#[test]
fn test_theme_set_colour_unknown_key_returns_false() {
    let mut theme = builtin("nord");
    let before = theme.clone();
    for key in [
        "nope",
        "text",
        "text.nope",
        "name",
        "dark",
        "categorical.8",
        "",
    ] {
        assert!(!theme.set_colour(key, Rgb(1, 2, 3)), "{key}");
    }
    assert_eq!(theme, before);
}

#[test]
fn test_theme_file_name_sanitises_paths() {
    assert_eq!(theme_file_name("nord"), "nord.json");
    assert_eq!(theme_file_name("codemason-light"), "codemason-light.json");
    let evil = theme_file_name("../evil");
    assert_eq!(evil, "___evil.json");
    assert!(!evil.contains('/') && !evil.contains('\\'));
}

#[test]
fn test_theme_catalog_records_source() {
    let dir = scratch();
    let mut theme = builtin("nord");
    theme.name = "mine".to_string();
    let path = dir.path().join("custom-file.json");
    save_theme(&theme, &path).expect("save");

    let mut catalog = ThemeCatalog::builtin();
    assert!(catalog.source("nord").is_none());
    assert!(catalog.is_builtin("nord"));
    assert!(!catalog.is_builtin("mine"));
    catalog.load_dir(dir.path());
    assert_eq!(catalog.source("mine"), Some(path.as_path()));
    assert!(!catalog.is_builtin("mine"));
}
