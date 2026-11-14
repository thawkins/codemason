//! The keyboard shortcuts listed in the `?` help panel.
//!
//! [`KEYBINDINGS`] is the frontend-neutral source of the panel: the terminal
//! UI draws it as a table and the GUI as its "Keyboard shortcuts" dialog, so
//! the two lists cannot drift. No dependencies.

/// All documented keybindings, in display order: (keys column, description
/// column). The terminal UI and the GUI both draw their `?` panel from this
/// one list.
pub const KEYBINDINGS: &[(&str, &str)] = &[
    // -- Typing ----------------------------------------------------------
    ("@", "Mention a file - opens file picker"),
    ("/", "Slash command - opens command menu"),
    ("?", "Show this keybindings help panel"),
    (
        "Shift+Enter / Alt+Enter",
        "Insert a newline (multiline input)",
    ),
    ("Left/Right", "Move cursor within the input line"),
    ("Shift+Left/Right", "Extend/shrink keyboard selection"),
    ("Ctrl+Left/Right", "Move cursor by word"),
    ("Ctrl+Shift+Left/Right", "Extend/shrink selection by word"),
    ("Home/End", "Jump to start/end of input"),
    ("Ctrl+Home/End", "Jump to input start/end"),
    ("Ctrl+E", "Jump to end of input (terminal style)"),
    ("Ctrl+B / Ctrl+F", "Move cursor left/right (terminal style)"),
    ("Ctrl+A", "Select all input text"),
    ("Ctrl+C", "Copy selection (or quit if no selection)"),
    ("Ctrl+X", "Cut selection to clipboard"),
    ("Ctrl+V", "Paste text from clipboard"),
    ("Delete", "Delete character under cursor"),
    ("Ctrl+W", "Delete previous word"),
    ("Ctrl+K", "Delete to end of line"),
    ("Alt+V", "Paste image from clipboard as attachment"),
    ("Alt+L", "Toggle log panel visibility"),
    ("Alt+P", "Toggle profiler panel visibility"),
    ("Alt+I", "Toggle Tasks panel visibility"),
    ("Alt+O", "Toggle autopilot mode (autonomous operation)"),
    ("Alt+C", "Toggle context panel visibility"),
    ("Alt+Y", "Toggle YOLO mode (bypass safety checks)"),
    ("Alt+E", "Toggle edit-log (per-edit operation logging)"),
    ("Alt+G", "Toggle GCF tool-result encoding"),
    (
        "Alt+A",
        "Toggle the Agents window (same toggle as the Agents button)",
    ),
    (
        "Alt+T",
        "Toggle the Teams window (same toggle as the Teams button)",
    ),
    // -- Sending ---------------------------------------------------------
    ("Enter", "Send message / confirm"),
    ("Ctrl+C, Ctrl+D", "Quit application (guarded sequence)"),
    // -- Navigation ------------------------------------------------------
    ("Shift+^ / PageUp", "Scroll messages up"),
    ("Shift+v / PageDown", "Scroll messages down"),
    ("^ / v", "Browse input history"),
    ("Ctrl+PageUp", "Scroll log panel up"),
    ("Ctrl+PageDown", "Scroll log panel down"),
    ("PageUp / PageDown", "Scroll opened output overlay"),
    ("Ctrl+PageUp/PageDown", "Output overlay: jump start/end"),
    // -- Agent ------------------------------------------------------------
    ("Tab", "Cycle to next agent"),
    ("Esc / Ctrl+X", "Cancel running agent (while processing)"),
    // -- Teams ------------------------------------------------------------
    ("Alt+v", "Focus next teammate"),
    ("Alt+^", "Focus previous teammate (or clear focus)"),
    // -- Dialogs ----------------------------------------------------------
    ("Esc", "Close any open dialog or menu"),
    ("y / a / n", "Allow / Always / Deny permission request"),
];
