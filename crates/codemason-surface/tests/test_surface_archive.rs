//! RS-10/RS-11: the shared archive extractor and `git+` parser.

use std::io::Write as _;

use codemason_surface::archive::{ArchiveKind, ExtractError, ExtractLimits, extract};
use codemason_surface::git_source::parse_git_source;

const LIMITS: ExtractLimits = ExtractLimits {
    max_entry_bytes: 64,
    max_total_bytes: 1024,
    max_entries: 10,
};

/// A zip holding one file `name` with `body`.
fn zip_with(name: &str, body: &[u8]) -> Vec<u8> {
    let mut out = std::io::Cursor::new(Vec::new());
    let mut zip = zip::ZipWriter::new(&mut out);
    zip.start_file(name, zip::write::SimpleFileOptions::default())
        .expect("start file");
    zip.write_all(body).expect("write body");
    zip.finish().expect("finish zip");
    out.into_inner()
}

fn staging(name: &str) -> std::path::PathBuf {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/temp/surface-archive")
        .join(name);
    let _ = std::fs::remove_dir_all(&dir); // INTENTIONAL: a fresh scratch dir
    dir
}

/// A safe entry lands under the staging root.
#[test]
fn test_surface_archive_extracts_a_safe_entry() {
    let dir = staging("safe");
    extract(
        &zip_with("a/b.txt", b"hello"),
        ArchiveKind::Zip,
        &dir,
        LIMITS,
    )
    .expect("extract");
    assert_eq!(std::fs::read(dir.join("a/b.txt")).expect("read"), b"hello");
}

/// A Windows drive-prefixed entry is refused on every platform; the old
/// plugins copy accepted `C:/x` on Unix.
#[test]
fn test_surface_archive_refuses_a_drive_prefixed_entry() {
    let dir = staging("drive");
    let err = extract(
        &zip_with("C:/evil.txt", b"x"),
        ArchiveKind::Zip,
        &dir,
        LIMITS,
    )
    .expect_err("a drive prefix escapes the root");
    assert!(
        matches!(err, ExtractError::UnsafePath(ref n) if n == "C:/evil.txt"),
        "{err:?}"
    );
}

/// An oversized entry is reported as data, so each crate keeps its wording.
#[test]
fn test_surface_archive_reports_an_oversized_entry() {
    let dir = staging("big");
    let err = extract(
        &zip_with("big.bin", &[0_u8; 100]),
        ArchiveKind::Zip,
        &dir,
        LIMITS,
    )
    .expect_err("over the entry cap");
    assert!(
        matches!(err, ExtractError::EntryTooLarge { format: "zip", ref name, size: 65 } if name == "big.bin"),
        "{err:?}"
    );
}

/// A source whose subpath fails validation is refused, not installed (or
/// looked up) at the repository root; the old connectors copy dropped it.
#[test]
fn test_surface_git_source_refuses_an_unsafe_subpath() {
    assert_eq!(
        parse_git_source("git+https://github.com/a/b#v1:../etc"),
        None
    );
    assert_eq!(parse_git_source("git+https://github.com/a/b#v1:/abs"), None);
    let ok = parse_git_source("git+https://github.com/a/b#v1:pkg/x").expect("parses");
    assert_eq!(ok.subpath.as_deref(), Some("pkg/x"));
    assert_eq!(ok.ref_name.as_deref(), Some("v1"));
}
