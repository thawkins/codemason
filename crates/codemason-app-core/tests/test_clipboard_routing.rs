//! Clipboard reads on a thread whose clipboard the frontend owns.
//!
//! The GUI routes its UI thread's clipboard to egui
//! (`route_copies_to_frontend`). A read there must never reach `arboard`:
//! on Wayland the window serves its own selection from that thread, so the
//! read waited on itself and froze the window (Ctrl+Shift+V after a
//! right-click Copy). It leaves a paste request for the frontend instead.

use codemason_app_core::clipboard::{
    get_clipboard_image, get_clipboard_text, route_copies_to_frontend, take_frontend_paste_request,
};

#[test]
fn test_clipboard_routing_text_read_requests_frontend_paste() {
    route_copies_to_frontend(true);
    assert_eq!(get_clipboard_text(), None);
    assert!(take_frontend_paste_request(), "a paste request is left");
    assert!(!take_frontend_paste_request(), "the request is taken once");
    route_copies_to_frontend(false);
}

#[test]
fn test_clipboard_routing_image_read_requests_frontend_paste() {
    route_copies_to_frontend(true);
    assert!(get_clipboard_image().is_none());
    assert!(take_frontend_paste_request());
    route_copies_to_frontend(false);
}

#[test]
fn test_clipboard_routing_off_clears_a_pending_request() {
    route_copies_to_frontend(true);
    assert_eq!(get_clipboard_text(), None);
    route_copies_to_frontend(false);
    assert!(!take_frontend_paste_request());
}
