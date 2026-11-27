//! Tests for `codemason_gui::image_load`: an SVG is rasterised at its own
//! size capped at `MAX_DECODE_SIDE`, a changed image file is read again, and
//! a path's `file://` URI is one the loader reads back as the same path.

use codemason_gui::image_load::{
    MAX_DECODE_SIDE, capped, decode_hint, file_uri, file_uri_dir, is_svg, refresh_if_changed,
    uri_path,
};
use egui::load::SizeHint;

/// A scratch folder of `name` under the git-ignored `target/temp`.
fn scratch(name: &str) -> std::path::PathBuf {
    let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/temp/image_load")
        .join(format!("{name}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("scratch dir");
    dir
}

#[test]
fn test_image_load_capped_keeps_shape_and_never_enlarges() {
    assert_eq!(capped(300.0, 120.0), (300, 120));
    assert_eq!(
        capped(20_000.0, 10_000.0),
        (MAX_DECODE_SIDE, MAX_DECODE_SIDE / 2)
    );
    assert!(is_svg("file:///a/B.SVG"));
    assert!(!is_svg("file:///a/b.png"));
}

#[test]
fn test_image_load_huge_svg_is_hinted_at_the_cap() {
    let dir = scratch("huge");
    let path = dir.join("huge.svg");
    std::fs::write(
        &path,
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="20000" height="20000"><rect width="10" height="10"/></svg>"#,
    )
    .expect("write svg");
    let ctx = egui::Context::default();
    egui_extras::install_image_loaders(&ctx);
    // Built by `file_uri`, not by hand: a hand-rolled `file://C:\...` is read as
    // a UNC host on Windows, so the test would prove nothing about the path it
    // wrote.
    let uri = file_uri(&path);
    let mut hint = None;
    // The file loader reads on a thread: poll until the bytes are in.
    for _ in 0..200 {
        hint = decode_hint(&ctx, &uri);
        if hint.is_some() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert_eq!(
        hint,
        Some(SizeHint::Size {
            width: MAX_DECODE_SIDE,
            height: MAX_DECODE_SIDE,
            maintain_aspect_ratio: true,
        })
    );
    assert_eq!(
        decode_hint(&ctx, "file:///x/photo.png"),
        Some(SizeHint::default())
    );
}

#[test]
fn test_image_load_changed_file_is_forgotten() {
    let dir = scratch("changed");
    let path = dir.join("chart.svg");
    let write = |side: u32| {
        std::fs::write(
            &path,
            format!(
                r#"<svg xmlns="http://www.w3.org/2000/svg" width="{side}" height="{side}"></svg>"#
            ),
        )
        .expect("write svg");
    };
    write(100);
    let ctx = egui::Context::default();
    egui_extras::install_image_loaders(&ctx);
    // The URI the widget would build, so this exercises the path `file_uri` and
    // `uri_path` agree on rather than a form the loader reads differently.
    let uri = file_uri(&path);
    let hint_of = |ctx: &egui::Context| {
        for _ in 0..200 {
            refresh_if_changed(ctx, &uri);
            if let Some(hint) = decode_hint(ctx, &uri) {
                return hint;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        panic!("never loaded");
    };
    let first = hint_of(&ctx);
    // A different size and length: the stamp changes even within one
    // second of mtime resolution.
    std::thread::sleep(std::time::Duration::from_millis(20));
    write(250);
    let second = hint_of(&ctx);
    assert_ne!(first, second, "the new file is read");
    assert_eq!(
        second,
        SizeHint::Size {
            width: 250,
            height: 250,
            maintain_aspect_ratio: true,
        }
    );
}

#[test]
fn test_image_load_file_uri_round_trips_an_absolute_path() {
    let path = std::path::Path::new("/home/me/shots/screen.png");
    let uri = file_uri(path);
    assert_eq!(uri, "file:///home/me/shots/screen.png");
    assert_eq!(uri_path(&uri).as_deref(), Some(path), "and back again");
}

#[test]
fn test_image_load_file_uri_puts_the_third_slash_of_a_drive_path() {
    // `file://C:/...` reads `C:` as a UNC host, so the file is never found;
    // the drive needs the slash a URI puts after an empty authority.
    let path = std::path::Path::new("C:\\shots\\screen.png");
    let uri = file_uri(path);
    assert_eq!(uri, "file:///C:\\shots\\screen.png");
    // The loader drops that slash, and so does the inverse here.
    assert_eq!(
        uri_path(&uri).as_deref(),
        Some(std::path::Path::new("C:\\shots\\screen.png"))
    );
    // A forward-slash drive path is the same path written the other way.
    let forward = file_uri(std::path::Path::new("C:/shots/screen.png"));
    assert_eq!(forward, "file:///C:/shots/screen.png");
    assert_eq!(
        uri_path(&forward).as_deref(),
        Some(std::path::Path::new("C:/shots/screen.png"))
    );
}

#[test]
fn test_image_load_file_uri_puts_a_unc_host_after_the_scheme() {
    // A UNC path's share is the URI's authority on Windows: the loader puts the
    // two backslashes back in front of the host, so `file://server/share` is the
    // share path and `file://\\server\share` would be a path named `\\`. Only
    // Windows rebuilds a host, so elsewhere the share has no authority to move
    // into and the path is written as it stands.
    let path = std::path::Path::new(r"\\server\share\shots\screen.png");
    let uri = file_uri(path);
    if cfg!(windows) {
        assert_eq!(uri, r"file://server\share\shots\screen.png");
    } else {
        assert_eq!(uri, r"file://\\server\share\shots\screen.png");
    }
    // Either way the inverse is exact, which is what `refresh_if_changed` needs
    // to stat the file the URI came from.
    assert_eq!(uri_path(&uri).as_deref(), Some(path), "and back again");
    // The extended-length form is a share path too, and round-trips as it is.
    let long = std::path::Path::new(r"\\?\C:\shots\screen.png");
    if cfg!(windows) {
        assert_eq!(file_uri(long), r"file://?\C:\shots\screen.png");
    }
    assert_eq!(uri_path(&file_uri(long)).as_deref(), Some(long));
}

#[test]
fn test_image_load_file_uri_leaves_a_relative_path_alone() {
    // A relative path is not a drive, so it keeps the scheme's two slashes.
    let uri = file_uri(std::path::Path::new("shots/screen.png"));
    assert_eq!(uri, "file://shots/screen.png");
    // Reading it back is Unix-only: on Windows `uri_path` (like the loader)
    // takes the text after `file://` as a UNC host, by design.
    #[cfg(not(windows))]
    assert_eq!(
        uri_path(&uri).as_deref(),
        Some(std::path::Path::new("shots/screen.png"))
    );
}

#[test]
fn test_image_load_file_uri_is_not_percent_encoded() {
    // The loader reads the raw path before the decoded one, so a space or a
    // `%` in a file name is the file name: encoding it would name another file.
    for path in [
        std::path::Path::new("/home/me/my shots/screen shot.png"),
        std::path::Path::new("/home/me/shots/My%20Doc.png"),
        std::path::Path::new("/home/me/shots/100% done.png"),
    ] {
        let uri = file_uri(path);
        assert!(
            uri.ends_with(&path.display().to_string()),
            "{uri} names {path:?} as it stands"
        );
        assert_eq!(uri_path(&uri).as_deref(), Some(path), "and back again");
    }
}

#[test]
fn test_image_load_file_uri_dir_keeps_a_drive_path_resolvable() {
    // A markdown image is a relative path appended to this prefix, so the
    // prefix decides whether the loader finds it: on Windows a prefix built as
    // `file://C:/shots` reads `C:` as a UNC host.
    let unix = file_uri_dir(std::path::Path::new("/home/me/shots"));
    assert_eq!(unix, "file:///home/me/shots/");
    assert_eq!(
        uri_path(&format!("{unix}screen.png")).as_deref(),
        Some(std::path::Path::new("/home/me/shots/screen.png"))
    );
    let drive = file_uri_dir(std::path::Path::new("C:\\shots"));
    assert_eq!(drive, "file:///C:\\shots/");
    assert_eq!(
        uri_path(&format!("{drive}screen.png")).as_deref(),
        Some(std::path::Path::new("C:\\shots/screen.png"))
    );
    // One separator either way: a directory already ending in one gains none.
    assert_eq!(
        file_uri_dir(std::path::Path::new("/home/me/shots/")),
        "file:///home/me/shots/"
    );
    // A share is the authority on Windows, so its host stays after the scheme.
    let share = file_uri_dir(std::path::Path::new(r"\\server\share\shots"));
    assert_eq!(
        share,
        if cfg!(windows) {
            r"file://server\share\shots/"
        } else {
            r"file://\\server\share\shots/"
        }
    );
    assert_eq!(
        uri_path(&format!("{share}screen.png")).as_deref(),
        Some(std::path::Path::new(r"\\server\share\shots/screen.png")),
        "a share prefix stays a share path"
    );
}

#[test]
fn test_image_load_uri_path_ignores_a_uri_that_is_not_a_file() {
    assert_eq!(uri_path("bytes://diagram.svg"), None);
    assert_eq!(uri_path("https://example.com/shot.png"), None);
    assert_eq!(uri_path("/home/me/shot.png"), None);
}

#[test]
fn test_image_load_uri_path_drops_only_the_scheme_slash() {
    // A path that starts with a slash of its own keeps every one but the
    // scheme's: only a drive path is read as `//C:/...` rather than `/C:/...`.
    assert_eq!(
        uri_path("file:////server/share/shot.png").as_deref(),
        Some(std::path::Path::new("//server/share/shot.png"))
    );
    assert_eq!(
        uri_path("file:///home/me/shot.png").as_deref(),
        Some(std::path::Path::new("/home/me/shot.png"))
    );
}

#[test]
fn test_image_load_uri_path_rebuilds_a_unc_share_from_its_host() {
    // The loader's own rule on Windows: a URI whose text after the scheme does
    // not begin with a slash is a host, and becomes a UNC path. Elsewhere the
    // text is the path itself, and one that already holds the two backslashes was
    // written as it stands either way.
    assert_eq!(
        uri_path(r"file://\\server\share\shot.png").as_deref(),
        Some(std::path::Path::new(r"\\server\share\shot.png")),
        "a path that starts with one of its own keeps it"
    );
    let host = uri_path("file://server/share/shot.png");
    if cfg!(windows) {
        assert_eq!(
            host.as_deref(),
            Some(std::path::Path::new(r"\\server/share/shot.png"))
        );
    } else {
        assert_eq!(
            host.as_deref(),
            Some(std::path::Path::new("server/share/shot.png"))
        );
    }
    assert_eq!(
        uri_path("file://").as_deref(),
        Some(std::path::Path::new("")),
        "an empty authority names the empty path, not a host"
    );
}
