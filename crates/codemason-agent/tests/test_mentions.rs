//! Tests for `@file#range` mentions (`codemason_agent::reference`):
//! `parse_mention`'s three forms and its refusals, `parse_refs` splitting a
//! range off a path (but not off a URL), and the resolver including only the
//! named lines, clamped to the file.

use codemason_agent::reference::parse::{FileMention, FileRef, parse_mention, parse_refs};
use codemason_agent::reference::resolve::resolve_all_refs;

/// A fresh directory under the git-ignored `target/temp/`.
fn scratch() -> tempfile::TempDir {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/temp");
    std::fs::create_dir_all(&root).expect("mkdir target/temp");
    tempfile::Builder::new()
        .prefix("mentions-")
        .tempdir_in(root)
        .expect("tempdir")
}

#[test]
fn test_mentions_parse_the_three_forms() {
    assert_eq!(
        parse_mention("@src/a.rs#20-45"),
        Some(FileMention {
            path: "src/a.rs".to_string(),
            range: Some((20, 45)),
        })
    );
    assert_eq!(
        parse_mention("@src/a.rs#20").and_then(|m| m.range),
        Some((20, 20))
    );
    assert_eq!(
        parse_mention("@src/a.rs"),
        Some(FileMention {
            path: "src/a.rs".to_string(),
            range: None,
        })
    );
}

#[test]
fn test_mentions_refuse_bad_ranges() {
    for token in [
        "@src/a.rs#45-20",
        "@src/a.rs#0-3",
        "@src/a.rs#x",
        "@#2-3",
        "@",
    ] {
        assert_eq!(parse_mention(token), None, "{token}");
    }
}

#[test]
fn test_mentions_parse_refs_splits_the_range_but_not_a_url_fragment() {
    let refs = parse_refs("see @src/a.rs#2-3 and @https://example.com/page#4");
    assert_eq!(refs.len(), 2);
    assert_eq!(refs[0].raw, "src/a.rs#2-3");
    assert_eq!(refs[0].kind, FileRef::File("src/a.rs".into()));
    assert_eq!(refs[0].range, Some((2, 3)));
    assert!(matches!(refs[1].kind, FileRef::Url(_)));
    assert_eq!(refs[1].range, None);
}

#[tokio::test]
async fn test_mentions_expansion_includes_only_the_range() {
    let dir = scratch();
    std::fs::write(dir.path().join("five.txt"), "one\ntwo\nthree\nfour\nfive\n").expect("write");
    let (out, refs) = resolve_all_refs("look @five.txt#2-3", dir.path())
        .await
        .expect("resolve");
    assert_eq!(refs.len(), 1);
    assert!(
        out.contains(r#"<file path="five.txt" lines="2-3">"#),
        "{out}"
    );
    assert!(out.contains("   2  two"));
    assert!(out.contains("   3  three"));
    for absent in ["one", "four", "five\n"] {
        assert!(!refs[0].content.contains(absent), "{absent}");
    }
}

#[tokio::test]
async fn test_mentions_range_past_the_end_is_clamped() {
    let dir = scratch();
    std::fs::write(dir.path().join("five.txt"), "one\ntwo\nthree\nfour\nfive\n").expect("write");
    let (_, refs) = resolve_all_refs("@five.txt#4-99", dir.path())
        .await
        .expect("resolve");
    assert_eq!(refs[0].content, "   4  four\n   5  five");
}

/// A one-page PDF whose text is one line per entry of `lines`.
fn text_pdf(lines: &[&str]) -> Vec<u8> {
    use lopdf::content::{Content, Operation};
    use lopdf::{Document, Object, Stream, dictionary};
    let mut doc = Document::with_version("1.5");
    let pages_id = doc.new_object_id();
    let font_id = doc.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => "Helvetica",
    });
    let resources_id = doc.add_object(dictionary! {
        "Font" => dictionary! { "F1" => font_id },
    });
    let mut operations = vec![
        Operation::new("BT", vec![]),
        Operation::new("Tf", vec!["F1".into(), 12.into()]),
        Operation::new("TL", vec![14.into()]),
        Operation::new("Td", vec![72.into(), 720.into()]),
    ];
    for line in lines {
        operations.push(Operation::new("Tj", vec![Object::string_literal(*line)]));
        operations.push(Operation::new("T*", vec![]));
    }
    operations.push(Operation::new("ET", vec![]));
    let content = Content { operations };
    let content_id = doc.add_object(Stream::new(
        dictionary! {},
        content.encode().expect("encode content"),
    ));
    let page_id = doc.add_object(dictionary! {
        "Type" => "Page",
        "Parent" => pages_id,
        "Contents" => content_id,
    });
    doc.objects.insert(
        pages_id,
        Object::Dictionary(dictionary! {
            "Type" => "Pages",
            "Kids" => vec![page_id.into()],
            "Count" => 1,
            "Resources" => resources_id,
            "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
        }),
    );
    let catalog_id = doc.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => pages_id,
    });
    doc.trailer.set("Root", catalog_id);
    let mut bytes = Vec::new();
    doc.save_to(&mut bytes).expect("save pdf");
    bytes
}

#[tokio::test]
async fn test_mentions_pdf_range_sends_only_those_lines() {
    let dir = scratch();
    std::fs::write(
        dir.path().join("report.pdf"),
        text_pdf(&["alpha", "bravo", "charlie", "delta", "echo"]),
    )
    .expect("write pdf");
    let (whole, _) = resolve_all_refs("@report.pdf", dir.path())
        .await
        .expect("resolve whole");
    assert!(whole.contains("alpha") && whole.contains("echo"), "{whole}");
    let (out, refs) = resolve_all_refs("@report.pdf#2-3", dir.path())
        .await
        .expect("resolve range");
    assert_eq!(refs[0].range, Some((2, 3)));
    assert!(out.contains(r#"lines="2-3""#), "{out}");
    for absent in ["alpha", "echo"] {
        assert!(
            !refs[0].content.contains(absent),
            "{absent}: {}",
            refs[0].content
        );
    }
}
