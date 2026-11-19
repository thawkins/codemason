//! Integration tests for the `xlsx` writer helper (audit T-702).
//!
//! `write_xlsx` turns a JSON `{ "sheets": [...] }` document into a real XLSX
//! workbook. These tests cover the happy path (mixed cell types, multiple
//! sheets, default sheet name) and the invalid-shape error path.

use serde_json::json;

use codemason_bench::xlsx::write_xlsx;

/// A scratch directory under the workspace's git-ignored `target/temp/`, removed
/// on drop. `tempfile::tempdir()` would land in the OS temp dir (`/tmp`), which
/// the project's temporary-file rules forbid.
fn scratch_dir() -> tempfile::TempDir {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/temp");
    std::fs::create_dir_all(&root).expect("create target/temp");
    tempfile::tempdir_in(&root).expect("tempdir under target/temp")
}

/// An XLSX file is a ZIP archive, so it begins with the `PK\x03\x04` signature.
fn assert_is_zip_archive(path: &std::path::Path) {
    let bytes = std::fs::read(path).expect("read written workbook");
    assert!(bytes.len() > 4, "workbook should not be empty");
    assert_eq!(
        &bytes[..4],
        b"PK\x03\x04",
        "xlsx output must be a ZIP archive"
    );
}

#[test]
fn test_write_xlsx_happy_path_mixed_cell_types() {
    let dir = scratch_dir();
    let path = dir.path().join("out.xlsx");
    let content = json!({
        "sheets": [
            {
                "name": "Sheet1",
                "rows": [
                    ["A1", "B1"],
                    [1, true],
                    [null, 2.5]
                ]
            }
        ]
    });

    write_xlsx(&path, &content).expect("write_xlsx should succeed");
    assert!(path.exists());
    assert_is_zip_archive(&path);
}

#[test]
fn test_write_xlsx_multiple_sheets_and_default_name() {
    let dir = scratch_dir();
    let path = dir.path().join("multi.xlsx");
    let content = json!({
        "sheets": [
            { "rows": [["only a name-less sheet"]] },
            { "name": "Second", "rows": [[1, 2, 3]] }
        ]
    });

    write_xlsx(&path, &content).expect("write_xlsx should accept a default sheet name");
    assert_is_zip_archive(&path);
}

#[test]
fn test_write_xlsx_missing_sheets_array_is_error() {
    let dir = scratch_dir();
    let path = dir.path().join("bad.xlsx");
    let err = write_xlsx(&path, &json!({ "rows": [] }))
        .unwrap_err()
        .to_string();
    assert!(
        err.contains("Missing 'sheets' array in xlsx content"),
        "unexpected error message: {err}"
    );
    assert!(!path.exists(), "no workbook should be written on error");
}

#[test]
fn test_write_xlsx_empty_sheets_is_valid_empty_workbook() {
    let dir = scratch_dir();
    let path = dir.path().join("empty.xlsx");
    write_xlsx(&path, &json!({ "sheets": [] })).expect("empty sheet list is valid");
    assert_is_zip_archive(&path);
}

/// Moved from tools-core `test_file_ops_happy_paths.rs` with the writer (#10, TL-07).
#[test]
fn test_xlsx_writer_edge_shapes() {
    let tmp = scratch_dir();
    let dir = tmp.path();
    // Missing sheets array is an error.
    assert!(write_xlsx(&dir.join("bad.xlsx"), &json!({})).is_err());

    // A sheet with no rows still writes a valid workbook.
    let path = dir.join("empty.xlsx");
    write_xlsx(&path, &json!({"sheets": [{"name": "Only"}]})).unwrap();
    let bytes = std::fs::read(&path).unwrap();
    assert_eq!(&bytes[..4], b"PK\x03\x04");

    // Default sheet name when `name` is absent; null cells are skipped.
    let path2 = dir.join("def.xlsx");
    write_xlsx(
        &path2,
        &json!({"sheets": [{"rows": [[1, true, "s", null]]}]}),
    )
    .unwrap();
    assert_eq!(&std::fs::read(&path2).unwrap()[..4], b"PK\x03\x04");
}
