//! Tests for the `/research open` markdown viewer rendering.

mod support;

use std::path::PathBuf;

// markdown_to_lines is private in layout.rs, so we test via the public
// render path using a small wrapper that exposes it for tests.

mod interaction_tests {
    use crate::term_events::{
        KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
    };

    use codemason_app_core::app::ResearchViewState;

    use super::{PathBuf, support};
    use crate::term_events::CrosstermAppExt;
    use codemason_app_core::geometry::Area;

    fn make_research_view() -> ResearchViewState {
        ResearchViewState {
            name: "test".to_string(),
            path: PathBuf::from("target/temp/research/test/RESEARCH.md"),
            base_dir: PathBuf::from("target/temp/research/test"),
            markdown: "line\n".repeat(200),
            scroll_offset: 10,
            max_scroll: 100,
            line_cache: codemason_app_core::app::OutputViewLineCache {
                wrapped_lines: Vec::new(),
                content_lines: Vec::new(),
                wrapped_count: 0,
                cache_width: 0,
                source_generation: 0,
            },
        }
    }

    fn mouse_scroll_up(col: u16, row: u16) -> MouseEvent {
        MouseEvent {
            kind: MouseEventKind::ScrollUp,
            column: col,
            row,
            modifiers: KeyModifiers::empty(),
        }
    }

    fn mouse_scroll_down(col: u16, row: u16) -> MouseEvent {
        MouseEvent {
            kind: MouseEventKind::ScrollDown,
            column: col,
            row,
            modifiers: KeyModifiers::empty(),
        }
    }

    fn mouse_click(col: u16, row: u16) -> MouseEvent {
        MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: col,
            row,
            modifiers: KeyModifiers::empty(),
        }
    }

    #[tokio::test]
    async fn test_research_view_esc_closes() {
        let mut app = support::make_app();
        app.research_view = Some(make_research_view());
        app.handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE))
            .await;
        assert!(
            app.research_view.is_none(),
            "Esc should close research view"
        );
    }

    #[tokio::test]
    async fn test_research_view_page_down_scrolls() {
        let mut app = support::make_app();
        app.research_view = Some(make_research_view());
        app.handle_key_event(KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE))
            .await;
        assert_eq!(app.research_view.as_ref().unwrap().scroll_offset, 5);
    }

    #[tokio::test]
    async fn test_research_view_page_up_scrolls() {
        let mut app = support::make_app();
        app.research_view = Some(make_research_view());
        app.handle_key_event(KeyEvent::new(KeyCode::PageUp, KeyModifiers::NONE))
            .await;
        assert_eq!(app.research_view.as_ref().unwrap().scroll_offset, 15);
    }

    #[tokio::test]
    async fn test_research_view_down_arrow_scrolls() {
        let mut app = support::make_app();
        app.research_view = Some(make_research_view());
        app.handle_key_event(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE))
            .await;
        assert_eq!(app.research_view.as_ref().unwrap().scroll_offset, 9);
    }

    #[tokio::test]
    async fn test_research_view_up_arrow_scrolls() {
        let mut app = support::make_app();
        app.research_view = Some(make_research_view());
        app.handle_key_event(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE))
            .await;
        assert_eq!(app.research_view.as_ref().unwrap().scroll_offset, 11);
    }

    #[tokio::test]
    async fn test_research_view_ctrl_page_end_jumps_to_end() {
        let mut app = support::make_app();
        app.research_view = Some(make_research_view());
        app.handle_key_event(KeyEvent::new(KeyCode::PageDown, KeyModifiers::CONTROL))
            .await;
        assert_eq!(app.research_view.as_ref().unwrap().scroll_offset, 0);
    }

    #[tokio::test]
    async fn test_research_view_ctrl_page_home_jumps_to_start() {
        let mut app = support::make_app();
        app.research_view = Some(make_research_view());
        app.handle_key_event(KeyEvent::new(KeyCode::PageUp, KeyModifiers::CONTROL))
            .await;
        assert_eq!(app.research_view.as_ref().unwrap().scroll_offset, 100);
    }

    #[test]
    fn test_research_view_mouse_scroll_inside() {
        let mut app = support::make_app();
        app.research_view = Some(make_research_view());
        app.research_view_area = Area::new(5, 5, 70, 20);

        app.handle_mouse_event(mouse_scroll_down(10, 10));
        assert_eq!(app.research_view.as_ref().unwrap().scroll_offset, 7);

        app.handle_mouse_event(mouse_scroll_up(10, 10));
        assert_eq!(app.research_view.as_ref().unwrap().scroll_offset, 10);
    }

    #[test]
    fn test_research_view_click_outside_closes() {
        let mut app = support::make_app();
        app.research_view = Some(make_research_view());
        app.research_view_area = Area::new(5, 5, 70, 20);

        app.handle_mouse_event(mouse_click(2, 2));
        assert!(
            app.research_view.is_none(),
            "click outside should close research view"
        );
    }

    #[test]
    fn test_research_view_click_inside_keeps_open() {
        let mut app = support::make_app();
        app.research_view = Some(make_research_view());
        app.research_view_area = Area::new(5, 5, 70, 20);

        app.handle_mouse_event(mouse_click(10, 10));
        assert!(
            app.research_view.is_some(),
            "click inside should keep research view open"
        );
    }

    #[tokio::test]
    async fn test_research_view_keys_do_not_type_into_input() {
        let mut app = support::make_app();
        app.research_view = Some(make_research_view());
        app.handle_key_event(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE))
            .await;
        assert!(app.research_view.is_some());
        assert_eq!(app.session.input, "");
    }
}
