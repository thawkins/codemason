//! Theme token tests: colour parsing, the built-in themes, contrast checks,
//! selection parsing and resolution, and loading theme files.

use std::path::PathBuf;

use codemason_app_core::theme_tokens::{
    AA_CONTRAST, DARK_THEME, HIGH_CONTRAST_THEME, LIGHT_THEME, Rgb, ThemeCatalog, ThemeSelection,
    ThemeTokens, UI_CONTRAST, contrast_ratio,
};

/// A fresh, empty scratch directory under the git-ignored `target/temp`.
fn scratch(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/temp/test_theme_tokens")
        .join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create scratch dir");
    dir
}

/// The built-in dark theme's JSON with `name` and `text.base` replaced.
fn theme_json(name: &str, text_base: &str) -> String {
    include_str!("../themes/codemason-dark.json")
        .replace("\"codemason-dark\"", &format!("\"{name}\""))
        .replace("\"#e6e9ef\"", &format!("\"{text_base}\""))
}

#[test]
fn test_rgb_parses_hex_and_rejects_other_shapes() {
    assert_eq!(Rgb::from_hex("#0f1115"), Some(Rgb(15, 17, 21)));
    assert_eq!(Rgb::from_hex("#FFFFFF"), Some(Rgb(255, 255, 255)));
    for bad in ["0f1115", "#fff", "#0f11155", "#gg0000", "", "#\u{e9}0000"] {
        assert_eq!(Rgb::from_hex(bad), None, "{bad}");
    }
}

#[test]
fn test_contrast_ratio_matches_wcag_reference_values() {
    let (black, white) = (Rgb(0, 0, 0), Rgb(255, 255, 255));
    assert!((contrast_ratio(white, black) - 21.0).abs() < 0.01);
    assert!((contrast_ratio(black, white) - 21.0).abs() < 0.01);
    assert!((contrast_ratio(white, white) - 1.0).abs() < 0.001);
    // #767676 on white is the classic AA boundary grey, 4.54:1.
    assert!((contrast_ratio(Rgb(0x76, 0x76, 0x76), white) - 4.54).abs() < 0.01);
}

#[test]
fn test_builtin_themes_parse_and_pass_the_contrast_check() {
    let themes = ThemeCatalog::builtin();
    let names: Vec<&str> = themes.names().collect();
    assert_eq!(
        names,
        [
            "codemason-dark",
            "codemason-light",
            "high-contrast",
            "catppuccin-mocha",
            "gruvbox-dark",
            "nord",
            "tokyo-night",
            "document-light",
            "document-dark"
        ]
    );
    for name in names {
        let theme = themes.get(name).expect("listed theme");
        assert_eq!(theme.contrast_failures(), Vec::<String>::new(), "{name}");
        let light = matches!(name, "codemason-light" | "document-light");
        assert_eq!(theme.dark, !light, "{name}");
    }
    let credited = |name: &str| themes.get(name).and_then(|t| t.credit.clone());
    for port in ["catppuccin-mocha", "gruvbox-dark", "nord", "tokyo-night"] {
        assert!(
            credited(port).is_some_and(|c| c.contains("licence")),
            "{port}"
        );
    }
}

#[test]
fn test_high_contrast_body_text_is_at_least_aaa() {
    let theme = ThemeCatalog::builtin()
        .get(HIGH_CONTRAST_THEME)
        .expect("built-in");
    assert!(contrast_ratio(theme.text.base, theme.bg.base) >= 7.0);
    assert!(contrast_ratio(theme.text.muted, theme.bg.base) >= 7.0);
}

#[test]
fn test_contrast_failures_name_the_token() {
    let theme = ThemeTokens::from_json(&theme_json("dim", "#202020")).expect("parse");
    let failures = theme.contrast_failures();
    assert_eq!(failures.len(), 1, "{failures:?}");
    assert!(
        failures[0].contains("theme `dim`: text.base"),
        "{failures:?}"
    );
    assert!(failures[0].contains(&format!("below {AA_CONTRAST}:1")));
}

#[test]
fn test_theme_json_rejects_unknown_tokens_and_bad_colours() {
    let unknown =
        theme_json("x", "#e6e9ef").replace("\"dark\": true,", "\"dark\": true, \"glow\": 1,");
    assert!(ThemeTokens::from_json(&unknown).is_err());
    let error =
        ThemeTokens::from_json(&theme_json("x", "red")).expect_err("a colour name is not a colour");
    assert!(error.to_string().contains("#rrggbb"), "{error}");
}

#[test]
fn test_selection_parses_aliases_and_resolves() {
    let themes = ThemeCatalog::builtin();
    let named = |name: &str| Some(ThemeSelection::Named(name.to_string()));
    assert_eq!(themes.parse("dark"), named(DARK_THEME));
    assert_eq!(themes.parse(" Light "), named(LIGHT_THEME));
    assert_eq!(themes.parse("Tokyo-Night"), named("tokyo-night"));
    assert_eq!(themes.parse(""), None);

    let system = ThemeSelection::System;
    assert_eq!(themes.resolve(&system, true).name, DARK_THEME);
    assert_eq!(themes.resolve(&system, false).name, LIGHT_THEME);
    let nord = ThemeSelection::Named("nord".to_string());
    assert_eq!(themes.resolve(&nord, false).name, "nord");
    // A theme that left the catalog resolves like `system`.
    let gone = ThemeSelection::Named("gone".to_string());
    assert_eq!(themes.resolve(&gone, false).name, LIGHT_THEME);
}

#[test]
fn test_load_dir_adds_overrides_and_reports_bad_files() {
    let dir = scratch("load_dir");
    std::fs::write(dir.join("a-mine.json"), theme_json("mine", "#ffffff")).expect("write");
    // Same name as a built-in: the file wins.
    std::fs::write(dir.join("b-nord.json"), theme_json("nord", "#fefefe")).expect("write");
    std::fs::write(dir.join("c-broken.json"), "{ not json").expect("write");
    std::fs::write(dir.join("d-dim.json"), theme_json("dim", "#202020")).expect("write");
    std::fs::write(dir.join("e-huge.json"), " ".repeat(70_000)).expect("write");
    std::fs::write(dir.join("notes.txt"), "not a theme").expect("write");

    let mut themes = ThemeCatalog::builtin();
    let before = themes.names().count();
    let warnings = themes.load_dir(&dir);

    assert_eq!(themes.names().count(), before + 2, "mine and dim are new");
    assert_eq!(
        themes.get("mine").map(|t| t.text.base),
        Some(Rgb(255, 255, 255))
    );
    assert_eq!(
        themes.get("nord").map(|t| t.text.base),
        Some(Rgb(0xfe, 0xfe, 0xfe))
    );
    assert_eq!(warnings.len(), 3, "{warnings:?}");
    assert!(warnings.iter().any(|w| w.contains("c-broken.json skipped")));
    assert!(
        warnings
            .iter()
            .any(|w| w.contains("theme `dim`: text.base"))
    );
    assert!(warnings.iter().any(|w| w.contains("e-huge.json skipped")));
}

#[test]
fn test_load_dir_of_a_missing_directory_adds_nothing() {
    let mut themes = ThemeCatalog::builtin();
    let warnings = themes.load_dir(&scratch("missing").join("nope"));
    assert_eq!(warnings, [] as [std::string::String; 0]);
    assert_eq!(themes, ThemeCatalog::builtin());
}

#[test]
fn test_theme_names_are_trimmed_lowercased_and_never_empty() {
    let theme = ThemeTokens::from_json(&theme_json("  Ocean Mist ", "#ffffff")).expect("parse");
    assert_eq!(theme.name, "ocean mist");
    assert!(ThemeTokens::from_json(&theme_json("   ", "#ffffff")).is_err());

    // A file that spells a built-in name in capitals replaces it.
    let mut themes = ThemeCatalog::builtin();
    let before = themes.names().count();
    themes.insert(ThemeTokens::from_json(&theme_json("Nord", "#fefefe")).expect("parse"));
    assert_eq!(themes.names().count(), before);
    let selected = themes.parse("NORD").expect("nord");
    assert_eq!(
        themes.resolve(&selected, true).text.base,
        Rgb(0xfe, 0xfe, 0xfe)
    );
}

#[test]
fn test_a_user_theme_name_wins_over_the_dark_and_light_aliases() {
    let mut themes = ThemeCatalog::builtin();
    assert_eq!(
        themes.parse("dark"),
        Some(ThemeSelection::Named(DARK_THEME.to_string()))
    );
    themes.insert(ThemeTokens::from_json(&theme_json("dark", "#ffffff")).expect("parse"));
    assert_eq!(
        themes.parse("Dark"),
        Some(ThemeSelection::Named("dark".to_string()))
    );
    assert_eq!(themes.parse("ocean mist"), None);
    themes.insert(ThemeTokens::from_json(&theme_json("Ocean Mist", "#ffffff")).expect("parse"));
    assert_eq!(
        themes.parse(" Ocean Mist "),
        Some(ThemeSelection::Named("ocean mist".to_string()))
    );
    // `system` and `default` stay the system selection.
    themes.insert(ThemeTokens::from_json(&theme_json("system", "#ffffff")).expect("parse"));
    assert_eq!(themes.parse("system"), Some(ThemeSelection::System));
}

#[cfg(unix)]
#[test]
fn test_load_dir_skips_fifos_and_devices_without_reading_them() {
    let dir = scratch("special_files");
    std::os::unix::fs::symlink("/dev/zero", dir.join("a-zero.json")).expect("symlink");
    let fifo = dir.join("b-fifo.json");
    let made = std::process::Command::new("mkfifo")
        .arg(&fifo)
        .status()
        .expect("run mkfifo");
    assert!(made.success(), "mkfifo failed");
    std::fs::write(dir.join("c-ok.json"), theme_json("ok", "#ffffff")).expect("write");

    let mut themes = ThemeCatalog::builtin();
    let warnings = themes.load_dir(&dir);

    assert!(themes.get("ok").is_some());
    assert_eq!(warnings.len(), 2, "{warnings:?}");
    for file in ["a-zero.json", "b-fifo.json"] {
        assert!(
            warnings
                .iter()
                .any(|w| w.contains(file) && w.contains("not a regular file")),
            "{file}: {warnings:?}"
        );
    }
}

#[test]
fn test_document_themes_carry_their_own_m3_roles() {
    let themes = ThemeCatalog::builtin();
    for name in ["document-light", "document-dark"] {
        let theme = themes.get(name).expect("built-in");
        let roles = theme.m3.expect("an m3 block");
        assert_eq!(theme.roles(), roles, "{name}");
        assert_eq!(theme.accent, roles.primary, "{name}");
        assert_eq!(theme.bg.base, roles.surface, "{name}");
        assert_eq!(theme.text.base, roles.on_surface, "{name}");
    }
}

#[test]
fn test_roles_are_derived_for_themes_without_an_m3_block() {
    let theme = ThemeCatalog::builtin().get(LIGHT_THEME).expect("built-in");
    assert_eq!(theme.m3, None);
    let roles = theme.roles();
    assert_eq!(roles.primary, theme.accent);
    assert_eq!(roles.surface, theme.bg.base);
    assert_eq!(roles.on_surface, theme.text.base);
    assert_eq!(roles.error, theme.state.error);
    assert_eq!(roles.outline_variant, theme.border.base);
    assert!(contrast_ratio(roles.on_primary, roles.primary) >= AA_CONTRAST);
    assert!(contrast_ratio(roles.outline, roles.surface) >= UI_CONTRAST);
}

#[test]
fn test_derived_roles_of_builtin_themes_pass_the_m3_contrast_check() {
    let themes = ThemeCatalog::builtin();
    for name in themes.names() {
        let theme = themes.get(name).expect("listed theme");
        assert_eq!(theme.roles().contrast_failures(name), Vec::<String>::new());
    }
}

#[test]
fn test_m3_contrast_failures_name_the_role_pair() {
    let json = include_str!("../themes/document-light.json")
        .replace("\"document-light\"", "\"pale\"")
        .replace("\"on_primary\": \"#ffffff\"", "\"on_primary\": \"#3060c0\"");
    let theme = ThemeTokens::from_json(&json).expect("parse");
    let failures = theme.contrast_failures();
    assert_eq!(failures.len(), 1, "{failures:?}");
    assert!(
        failures[0].contains("theme `pale`: m3.on_primary") && failures[0].contains("m3.primary"),
        "{failures:?}"
    );
}

#[test]
fn test_m3_block_rejects_unknown_roles() {
    let json = include_str!("../themes/document-light.json")
        .replace("\"scrim\":", "\"glow\": \"#000000\", \"scrim\":");
    assert!(ThemeTokens::from_json(&json).is_err());
}

#[test]
fn test_high_contrast_m3_roles_reach_aaa() {
    let theme = ThemeCatalog::builtin()
        .get(HIGH_CONTRAST_THEME)
        .expect("built-in");
    let m3 = theme.roles();
    let pairs = [
        ("on_primary", m3.on_primary, m3.primary),
        (
            "on_primary_container",
            m3.on_primary_container,
            m3.primary_container,
        ),
        ("on_secondary", m3.on_secondary, m3.secondary),
        (
            "on_secondary_container",
            m3.on_secondary_container,
            m3.secondary_container,
        ),
        ("on_tertiary", m3.on_tertiary, m3.tertiary),
        (
            "on_tertiary_container",
            m3.on_tertiary_container,
            m3.tertiary_container,
        ),
        ("on_error", m3.on_error, m3.error),
        (
            "on_error_container",
            m3.on_error_container,
            m3.error_container,
        ),
        ("on_surface", m3.on_surface, m3.surface),
        ("on_surface_variant", m3.on_surface_variant, m3.surface),
        (
            "on_surface/highest",
            m3.on_surface,
            m3.surface_container_highest,
        ),
        (
            "inverse_on_surface",
            m3.inverse_on_surface,
            m3.inverse_surface,
        ),
    ];
    let failures: Vec<String> = pairs
        .iter()
        .filter_map(|(name, fg, bg)| {
            let ratio = contrast_ratio(*fg, *bg);
            (ratio < 7.0).then(|| format!("{name} {ratio:.2}"))
        })
        .collect();
    assert_eq!(failures, Vec::<String>::new());
}
