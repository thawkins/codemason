//! Tests for the transcript context menu of a frontend that lays out
//! messages itself: `App::open_transcript_context_menu`, Copy as Markdown
//! and Quote in reply.

use codemason_app_core::app::{ContextAction, SelectionPane};
use codemason_app_core::clipboard;

mod support;

#[test]
fn test_transcript_menu_items_follow_selection_and_message() {
    let mut app = support::make_app();
    app.open_transcript_context_menu(false, Some("**hi**".to_string()));
    let menu = app.context_menu.clone().expect("menu open");
    assert_eq!(menu.pane, SelectionPane::Messages);
    assert_eq!(
        menu.items,
        [
            (ContextAction::Copy, false),
            (ContextAction::CopyMarkdown, true),
            (ContextAction::QuoteReply, true),
        ]
    );
    assert_eq!(
        menu.selected, 1,
        "the highlight starts on the first enabled item"
    );

    app.open_transcript_context_menu(true, None);
    let menu = app.context_menu.clone().expect("menu open");
    assert_eq!(
        menu.items,
        [
            (ContextAction::Copy, true),
            (ContextAction::CopyMarkdown, false),
            (ContextAction::QuoteReply, false),
        ]
    );
    assert_eq!(app.context_menu_message, None);

    // Opening another pane's menu forgets the message.
    app.open_transcript_context_menu(false, Some("x".to_string()));
    app.open_context_menu(0, 0, SelectionPane::Input, false);
    assert_eq!(app.context_menu_message, None);
}

/// While the frontend owns the clipboard (the GUI), opening the menu must
/// not read it: Paste stays enabled and no paste is requested until chosen.
#[test]
fn test_context_menu_open_does_not_read_a_frontend_clipboard() {
    let mut app = support::make_app();
    clipboard::route_copies_to_frontend(true);
    app.open_context_menu(0, 0, SelectionPane::Input, false);
    let requested = clipboard::take_frontend_paste_request();
    clipboard::route_copies_to_frontend(false);
    let menu = app.context_menu.clone().expect("menu open");
    assert!(
        menu.items.contains(&(ContextAction::Paste, true)),
        "Paste enabled: {:?}",
        menu.items
    );
    assert!(!requested, "opening the menu requests no paste");
}

#[test]
fn test_copy_as_markdown_copies_the_message_source() {
    let mut app = support::make_app();
    clipboard::route_copies_to_frontend(true);
    app.open_transcript_context_menu(false, Some("# Title\n\n`code`".to_string()));
    app.execute_context_action(ContextAction::CopyMarkdown);
    let copies = clipboard::take_frontend_copies();
    clipboard::route_copies_to_frontend(false);
    assert_eq!(copies, ["# Title\n\n`code`"]);
    assert!(app.context_menu.is_none());
    assert_eq!(app.context_menu_message, None);
}

#[test]
fn test_quote_in_reply_quotes_the_message_after_the_draft() {
    let mut app = support::make_app();
    app.session.input = "about this:".to_string();
    app.open_transcript_context_menu(false, Some("first\n\nsecond\n".to_string()));
    app.execute_context_action(ContextAction::QuoteReply);
    assert_eq!(app.session.input, "about this:\n> first\n>\n> second\n\n");
    assert!(app.context_menu.is_none());

    // Into an empty prompt.
    let mut app = support::make_app();
    app.quote_in_reply("one");
    assert_eq!(app.session.input, "> one\n\n");
}
