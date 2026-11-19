//! Tests for `codemason_app_core::markdown::split_standalone_images`: which
//! images are cut out of a document and what is left around them.

use codemason_app_core::markdown::{MarkdownPiece, split_standalone_images};

fn md(text: &str) -> MarkdownPiece {
    MarkdownPiece::Markdown(text.to_string())
}

fn image(alt: &str, src: &str) -> MarkdownPiece {
    MarkdownPiece::Image {
        alt: alt.to_string(),
        src: src.to_string(),
    }
}

#[test]
fn test_markdown_images_without_images_is_one_piece() {
    assert_eq!(
        split_standalone_images("Just *text*."),
        vec![md("Just *text*.")]
    );
    assert_eq!(split_standalone_images("  \n"), Vec::new());
}

#[test]
fn test_markdown_images_standalone_image_is_cut_out() {
    let doc = "Here is the chart:\n\n![Sales by month](out/chart.png)\n\nAs you can see.";
    assert_eq!(
        split_standalone_images(doc),
        vec![
            md("Here is the chart:\n\n"),
            image("Sales by month", "out/chart.png"),
            md("\nAs you can see."),
        ]
    );
}

#[test]
fn test_markdown_images_document_that_is_only_an_image() {
    assert_eq!(
        split_standalone_images("![*plot*](plot.svg)\n"),
        vec![image("plot", "plot.svg")]
    );
}

#[test]
fn test_markdown_images_inline_listed_and_paired_images_stay_in_markdown() {
    for doc in [
        "See ![icon](a.png) here.",
        "- ![item](a.png)\n- two",
        "> ![quoted](a.png)",
        "![one](a.png) ![two](b.png)",
        "| a |\n|---|\n| ![cell](a.png) |",
    ] {
        assert_eq!(split_standalone_images(doc), vec![md(doc)], "{doc}");
    }
}

#[test]
fn test_markdown_images_web_url_is_kept_as_written() {
    assert_eq!(
        split_standalone_images("![logo](https://example.com/logo.png)"),
        vec![image("logo", "https://example.com/logo.png")]
    );
}
