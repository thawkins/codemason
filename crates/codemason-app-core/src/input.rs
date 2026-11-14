//! Keyboard input handling for the TUI.
//!
//! Maps frontend-neutral key presses ([`KeyInput`]) to high-level [`InputAction`]s, handling both
//! normal editing mode and the permission dialog intercept.

use crate::input_event::{Key, KeyInput, Modifiers};
use codemason_types::ThinkingLevel;

use crate::app::{
    App, ConfiguredProvider, ContextAction, ModelPickerEntry, PROVIDER_LIST, ProviderSetupStep,
    ProviderSource,
};
use crate::text_utils::is_ollama_family;
use codemason_llm::providers::router_config::Tier;

/// Returns `true` when `key` is one of the always-on global quit keys:
/// `Ctrl+C` (arm quit, or copy when a keyboard selection is active) or
/// `Ctrl+D` (confirm quit).
///
/// The quit keys are handled before any modal interception so they work from
/// every screen. `App::handle_key` intercepts a handful of overlays
/// (the run-cost banner and the history/config-save pickers) *before*
/// delegating to [`handle_key`]; it consults this predicate so a quit key
/// falls through to the normal handler and behaves identically no matter which
/// overlay is active (LOW-6). Keeping the classification in one place means the
/// global handler and the pre-dialog guard cannot drift.
pub fn is_global_quit_key(key: KeyInput) -> bool {
    matches!(key.key, Key::Char('c') | Key::Char('d')) && key.mods.contains(Modifiers::CTRL)
}

/// A high-level action produced by interpreting a key event.
#[derive(Debug)]
pub enum InputAction {
    /// Submit the input buffer as a user message.
    SendMessage(String),
    /// Run a shell command (input started with `!`) and ask the model to
    /// review the output for errors.
    BangCommand(String),
    /// Exit the application.
    Quit,
    /// Confirm guarded keyboard quit (Ctrl+D after Ctrl+C).
    ConfirmQuit,
    /// Scroll the message view upward.
    ScrollUp,
    /// Scroll the message view downward.
    ScrollDown,
    /// Scroll the log panel upward.
    LogScrollUp,
    /// Scroll the log panel downward.
    LogScrollDown,
    /// Scroll active output view upward.
    OutputViewPageUp,
    /// Scroll active output view downward.
    OutputViewPageDown,
    /// Jump active output view to the start.
    OutputViewToStart,
    /// Jump active output view to the end.
    OutputViewToEnd,
    /// Recall the previous entry from input history.
    HistoryUp,
    /// Recall the next entry from input history.
    HistoryDown,
    /// Move the cursor left within the input line.
    MoveCursorLeft,
    /// Move the cursor right within the input line.
    MoveCursorRight,
    /// Move the cursor to the start of the input line.
    MoveCursorHome,
    /// Move the cursor to the end of the input line.
    MoveCursorEnd,
    /// Move the cursor one word left.
    MoveCursorWordLeft,
    /// Move the cursor one word right.
    MoveCursorWordRight,
    /// Delete the character under the cursor.
    Delete,
    /// Delete the previous word.
    DeletePrevWord,
    /// Delete from cursor to end of line.
    DeleteToLineEnd,
    /// Clear entire line.
    ClearLine,
    /// Cycle to the next configured agent.
    SwitchAgent,
    /// Execute a `/`-prefixed command.
    SlashCommand(String),
    /// Cancel the currently running agent (Esc or Ctrl+X while processing).
    CancelAgent,
    /// Confirm a pending forcecleanup modal (Enter -> confirm).
    ConfirmForceCleanup,
    /// Cancel a pending forcecleanup modal (Esc -> cancel).
    CancelForceCleanup,
    /// Accept a pending rollback offer (Enter -> restore the pre-loop
    /// snapshot, FR-020 / T-013).
    ConfirmRollback,
    /// Decline a pending rollback offer (Esc -> keep the loop's changes,
    /// FR-020 / T-013).
    CancelRollback,
    /// Confirm the router save confirmation modal (Enter -> save).
    ConfirmRouterSave,
    /// Cancel the router save confirmation modal (Esc -> cancel).
    CancelRouterSave,
    /// Confirm the `Clear the input queue?` dialog's `Yes` option (spec
    /// `inputqueue` FR-033, FR-036).
    ConfirmQueueClear,
    /// Dismiss the `Clear the input queue?` dialog: the `No` option or `Esc`.
    /// Leaves the queue unchanged (spec `inputqueue` FR-035).
    CancelQueueClear,
    /// Dismiss the single-button notice dialog (`Enter` or `Esc`). It only
    /// reports a message, so there is nothing to confirm.
    DismissNotice,
    /// Confirm the `Clear this project's memory?` dialog's `Yes` option
    /// (opened by `/memory clear`).
    ConfirmMemoryClear,
    /// Dismiss the `Clear this project's memory?` dialog: the `No` option or
    /// `Esc`. Leaves memory unchanged.
    CancelMemoryClear,
    /// Confirm the `Prune messages older than N days?` dialog's `Yes` option
    /// (opened by `/db prune`).
    ConfirmDbPrune,
    /// Dismiss the `Prune messages older than N days?` dialog: the `No` option
    /// or `Esc`. Leaves the database unchanged.
    CancelDbPrune,
    /// Confirm the plan approval dialog (Enter when cursor_approve = true).
    ApprovePlan,
    /// Reject the plan approval dialog (Enter when cursor_approve = false, or `r`/Esc).
    RejectPlan,
    /// Toggle the plan approval dialog cursor left/right (<-/-> arrow keys).
    TogglePlanCursor,
    /// Cycle focus to the next teammate (Alt+Down).
    FocusNextTeammate,
    /// Cycle focus to the previous teammate (Alt+Up).
    FocusPrevTeammate,
    /// Insert a literal newline at cursor (Shift+Enter - multiline input).
    InsertNewline,
    /// Select all input text (Ctrl+A).
    SelectAll,
    /// Extend keyboard selection one character to the left (Shift+Left).
    SelectCharLeft,
    /// Extend keyboard selection one character to the right (Shift+Right).
    SelectCharRight,
    /// Extend keyboard selection one word to the left (Ctrl+Shift+Left).
    SelectWordLeft,
    /// Extend keyboard selection one word to the right (Ctrl+Shift+Right).
    SelectWordRight,
    /// Copy the active keyboard selection to the clipboard (Ctrl+C when selection active).
    CopyToClipboard,
    /// Cut the active keyboard selection to the clipboard (Ctrl+X).
    CutToClipboard,
    /// Paste text from the clipboard at the cursor (Ctrl+V).
    PasteFromClipboard,
    /// Toggle the log panel visibility (Alt+L).
    ToggleLog,
    /// Toggle the profiler panel visibility and profiler state (Alt+P).
    ToggleProfile,
    /// Toggle the Tasks panel visibility (Alt+I).
    ToggleTasksPanel,
    /// Toggle the Memory side panel visibility (Alt+M).
    ToggleMemory,
    /// Move the Memory panel cursor up.
    MemoryCursorUp,
    /// Move the Memory panel cursor down.
    MemoryCursorDown,
    /// Open the selected memory in a full overlay.
    OpenMemoryView,
    /// Prompt to delete the selected memory (shows confirmation).
    PromptMemoryDelete,
    /// Confirm the pending memory delete (Enter on confirmation dialog).
    ConfirmMemoryDelete,
    /// Cancel the pending memory delete (Esc on confirmation dialog).
    CancelMemoryDelete,
    /// Scroll the memory view overlay upward.
    MemoryViewPageUp,
    /// Scroll the memory view overlay downward.
    MemoryViewPageDown,
    /// Jump the memory view overlay to the start.
    MemoryViewToStart,
    /// Jump the memory view overlay to the end.
    MemoryViewToEnd,
    /// Scroll the memory view overlay up by one line.
    MemoryViewLineUp,
    /// Scroll the memory view overlay down by one line.
    MemoryViewLineDown,
    /// Toggle autopilot mode (Alt+O).
    ToggleAutopilot,
    /// Toggle the Context side panel visibility (Alt+C).
    ToggleContextPanel,
    /// Toggle YOLO mode (Alt+Y).
    ToggleYolo,
    /// Toggle edit-operation logging (Alt+E).
    ToggleEditLog,
    /// Toggle GCF tool-result encoding (Alt+G).
    ToggleGcf,
    /// Toggle the Agents window (Alt+A) - spec `agentssetup` FR-022.
    ///
    /// Routed to the same [`App::toggle_agents_window`] the Agents button's
    /// click calls, so the keyboard and the mouse share one visibility path.
    ToggleAgentsWindow,
    /// Toggle the Teams window (Alt+T) - spec `agentssetup` FR-045.
    ///
    /// Routed to the same [`App::toggle_teams_window`] the Teams button's
    /// click calls, mirroring [`InputAction::ToggleAgentsWindow`].
    ToggleTeamsWindow,
    /// Open the queue-control menu overlay (Alt+Q) - spec `inputqueue` FR-021.
    OpenQueueMenu,
    /// Scroll the research markdown viewer up.
    ResearchViewPageUp,
    /// Scroll the research markdown viewer down.
    ResearchViewPageDown,
    /// Jump the research markdown viewer to the start.
    ResearchViewToStart,
    /// Jump the research markdown viewer to the end.
    ResearchViewToEnd,
    /// Scroll the research markdown viewer up by one line.
    ResearchViewLineUp,
    /// Scroll the research markdown viewer down by one line.
    ResearchViewLineDown,
}

/// Translate a [`KeyInput`] into an optional [`InputAction`].
///
/// When a permission dialog is active, only `y` / `a` / `n` keys are handled
/// (publishing a permission reply). When the provider setup dialog is active,
/// keys are routed to the dialog. When the slash-command menu is active,
/// arrow keys navigate and Enter selects. Otherwise normal editing and
/// navigation keys are processed.
///
/// # Examples
///
/// ```rust,no_run
/// # use codemason_app_core::input_event::{Key, KeyInput, Modifiers};
/// # use codemason_app_core::app::App;
/// # use codemason_app_core::input::handle_key;
/// # async fn example(app: &mut App) {
/// let key = KeyInput {
///     key: Key::Enter,
///     mods: Modifiers::NONE,
/// };
/// if let Some(action) = handle_key(app, key).await {
///     println!("Action: {action:?}");
/// }
/// # }
/// ```
///
/// Key releases are filtered out by the frontend before conversion to a
/// [`KeyInput`], so every key reaching this function is a press.
pub async fn handle_key(app: &mut App, key: KeyInput) -> Option<InputAction> {
    // Always check for quit commands first, before any modal interception.
    // This ensures Ctrl+C (arm quit) and Ctrl+D (confirm quit) work globally.
    if is_global_quit_key(key) {
        if key.key == Key::Char('c') {
            // Copy if a keyboard selection is active; otherwise arm quit.
            if app.session.kb_select_anchor.is_some() {
                return Some(InputAction::CopyToClipboard);
            }
            return Some(InputAction::Quit);
        }
        return Some(InputAction::ConfirmQuit);
    }

    // If a memory delete confirmation modal is active, intercept Enter/Esc
    // before any other dialog so the confirmation always takes precedence.
    if app.pending_memory_delete.is_some() {
        match key.key {
            Key::Enter => return Some(InputAction::ConfirmMemoryDelete),
            Key::Esc => return Some(InputAction::CancelMemoryDelete),
            _ => return None,
        }
    }

    // If a router save confirmation modal is active, intercept Enter/Esc
    // before any other dialog so the confirmation always takes precedence.
    if app.pending_router_save.is_some() {
        match key.key {
            Key::Enter => return Some(InputAction::ConfirmRouterSave),
            Key::Esc => return Some(InputAction::CancelRouterSave),
            _ => return None,
        }
    }

    // While the plugin-store browse panel is open it owns the keyboard. The
    // panel has three focusable panes (spec `pluginstores` FR-052): the left column
    // is split vertically into the category navigator (top) and the marketplace
    // navigator (bottom), with the result list to their right. `Tab`/`BackTab` cycle
    // forward/backward through `Results` -> `Categories` -> `Marketplaces` -> `Results`;
    // `Up`/`Down` move the focused pane's cursor and, for either navigator, apply the
    // highlighted filter immediately (FR-007, FR-008, FR-017, FR-052); `ENTER` over a
    // navigator applies the highlighted filter and focuses the result list without
    // installing anything, while `ENTER` over the result list keeps the existing
    // install behaviour (FR-010, FR-024); `c` clears the focused navigator's filter to
    // `ALL` (FR-015, FR-052); `Backspace` and `Esc` edit the query (and `Esc` on an
    // empty query dismisses the panel), and printable characters type into the panel's
    // own search field. Every other key is swallowed, nothing here mutates the message
    // input buffer or the input queue, and the field stays locked (FR-007, FR-009,
    // FR-017, FR-024). Placed before the queue modals so the panel has priority while
    // it is open.
    if app.plugin_store.is_some() {
        match key.key {
            Key::Tab => app.plugin_store_focus_next(),
            Key::BackTab => app.plugin_store_focus_prev(),
            Key::Up => {
                if app.plugin_store_nav_focused() {
                    app.plugin_store_category_move_up();
                } else if app.plugin_store_marketplace_focused() {
                    app.plugin_store_marketplace_move_up();
                } else {
                    app.plugin_store_move_up();
                }
            }
            Key::Down => {
                if app.plugin_store_nav_focused() {
                    app.plugin_store_category_move_down();
                } else if app.plugin_store_marketplace_focused() {
                    app.plugin_store_marketplace_move_down();
                } else {
                    app.plugin_store_move_down();
                }
            }
            Key::Enter => {
                if app.plugin_store_nav_focused() {
                    app.plugin_store_category_confirm();
                } else if app.plugin_store_marketplace_focused() {
                    app.plugin_store_marketplace_confirm();
                } else {
                    app.plugin_store_install_selected();
                }
            }
            Key::Char('c') if !key.mods.contains(Modifiers::CTRL) => {
                if app.plugin_store_marketplace_focused() {
                    app.plugin_store_clear_marketplace();
                } else {
                    app.plugin_store_clear_category();
                }
            }
            Key::Backspace | Key::Esc => {
                app.plugin_store_edit_or_close();
            }
            Key::Char(c) if !key.mods.contains(Modifiers::CTRL) => {
                app.plugin_store_push_char(c);
            }
            _ => {}
        }
        return None;
    }

    // While the connector-catalogue browse panel (`/connectors claude`) is open
    // it owns the keyboard exactly as the plugin-store panel does. The panel has
    // three focusable panes: the category navigator (top of the left column), the
    // marketplace navigator (bottom of the left column), and the result list.
    // `Tab`/`BackTab` cycle focus through them; `Up`/`Down` move the focused
    // pane's cursor and, for either navigator, apply the highlighted filter
    // immediately; `ENTER` over a navigator applies its filter and returns focus
    // to the result list without installing anything, while `ENTER` over the
    // result list keeps the install behaviour; `c` clears the focused navigator's
    // filter to `ALL`; `Backspace` and `Esc` edit the query (and `Esc` on an empty
    // query dismisses the panel), and printable characters type into the panel's
    // own search field. Every other key is swallowed and the input field stays
    // locked.
    if app.connector_store.is_some() {
        match key.key {
            Key::Tab => app.connector_catalogue_focus_next(),
            Key::BackTab => app.connector_catalogue_focus_prev(),
            Key::Up => {
                if app.connector_catalogue_nav_focused() {
                    app.connector_catalogue_category_move_up();
                } else if app.connector_catalogue_marketplace_focused() {
                    app.connector_catalogue_marketplace_move_up();
                } else {
                    app.connector_catalogue_move_up();
                }
            }
            Key::Down => {
                if app.connector_catalogue_nav_focused() {
                    app.connector_catalogue_category_move_down();
                } else if app.connector_catalogue_marketplace_focused() {
                    app.connector_catalogue_marketplace_move_down();
                } else {
                    app.connector_catalogue_move_down();
                }
            }
            Key::Enter => {
                if app.connector_catalogue_nav_focused() {
                    app.connector_catalogue_category_confirm();
                } else if app.connector_catalogue_marketplace_focused() {
                    app.connector_catalogue_marketplace_confirm();
                } else {
                    app.connector_catalogue_install_selected();
                }
            }
            Key::Char('c') if !key.mods.contains(Modifiers::CTRL) => {
                if app.connector_catalogue_marketplace_focused() {
                    app.connector_catalogue_clear_marketplace();
                } else {
                    app.connector_catalogue_clear_category();
                }
            }
            Key::Backspace | Key::Esc => {
                app.connector_catalogue_edit_or_close();
            }
            Key::Char(c) if !key.mods.contains(Modifiers::CTRL) => {
                app.connector_catalogue_push_char(c);
            }
            _ => {}
        }
        return None;
    }

    // While the `/backend` switcher panel (spec `openhands` FR-008) is open it
    // owns the keyboard: `Up`/`Down` move the block cursor, `Enter` switches the
    // active backend to the highlighted row, and `Esc` dismisses the panel. Every
    // other key is swallowed and the input field stays locked.
    if app.backend_panel.is_some() {
        match key.key {
            Key::Up => app.backend_panel_move_up(),
            Key::Down => app.backend_panel_move_down(),
            Key::Enter => app.backend_panel_activate_selected(),
            Key::Esc => app.close_backend_panel(),
            _ => {}
        }
        return None;
    }

    // While the queue-entry panel (ALT-Q `Show` row) is open it swallows every
    // keystroke so none of them can mutate the editable input buffer: `Up`/`Down`
    // move the highlight, `Enter` moves the highlighted entry one step toward the
    // front, `Del` removes it, and `Esc` is the only key that dismisses the panel.
    // Every change repaints on the next frame; the input buffer, staged
    // attachments, the running turn, and (apart from `Enter`/`Del`) the queue are
    // left untouched.
    if app.queue_show_open {
        match key.key {
            Key::Up => app.queue_show_move_up(),
            Key::Down => app.queue_show_move_down(),
            Key::Enter => app.queue_show_promote_selected(),
            Key::Delete => app.queue_show_delete_selected(),
            Key::Esc => app.queue_show_close(),
            _ => {}
        }
        return None;
    }

    // While the queue-control menu overlay (ALT-Q) is open it swallows every
    // keystroke so none of them can mutate the editable input buffer (FR-031).
    // `Up`/`Down` move the highlight (skipping the non-selectable empty-queue
    // `Next` row, FR-023), `Enter` activates the highlighted row, and `Esc`
    // dismisses the menu taking no action (FR-032): the input buffer, staged
    // attachments, the queue, and the running turn are all left untouched, and
    // the closure is painted on the next frame (NFR-008).
    if app.queue_menu_open {
        match key.key {
            Key::Up => app.queue_menu_move_up(),
            Key::Down => app.queue_menu_move_down(),
            Key::Enter => app.queue_menu_activate_selected().await,
            Key::Esc => {
                app.close_queue_menu();
                app.needs_redraw = true;
            }
            _ => {}
        }
        return None;
    }

    // If context menu is active, route all keys there.
    if app.context_menu.is_some() {
        handle_context_menu_key(app, key);
        return None;
    }

    // If shortcuts panel is active, only Esc or '?' dismiss it.
    if app.show_shortcuts {
        if key.key == Key::Esc || key.key == Key::Char('?') {
            app.show_shortcuts = false;
        }
        return None;
    }

    // If provider setup dialog is active, route all keys there.
    if app.provider_setup.is_some() {
        handle_provider_setup_key(app, key);
        return None;
    }

    // If MCP discover dialog is active, route all keys there
    if app.mcp_discover.is_some() {
        handle_mcp_discover_key(app, key).await;
        return None;
    }

    // If the `/plugins list` dialog is active, route all keys there (FR-009).
    if app.plugins_list.is_some() {
        handle_plugins_list_key(app, key).await;
        return None;
    }

    // If the /loop setup dialog is active, route all keys there (T-014).
    if app.loop_setup.is_some() {
        crate::app::handle_loop_setup_key(app, key);
        return None;
    }

    // If question dialog is active, intercept keys. Enter and Esc answer
    // only once the front question has been shown for the answer delay, so a
    // double press cannot answer the next question unread; the answered
    // question leaves the queue at once (see `App::question_answerable`).
    if !app.session.question_queue.is_empty() {
        app.note_question_front();
        let answerable = app.question_answerable();
        let has_options = app
            .session
            .question_queue
            .front()
            .map(|r| !r.options.is_empty())
            .unwrap_or(false);

        if has_options {
            return match key.key {
                Key::Up | Key::Char('k') => {
                    if app.session.question_selected_index > 0 {
                        app.session.question_selected_index -= 1;
                    }
                    None
                }
                Key::Down | Key::Char('j') => {
                    if let Some(req) = app.session.question_queue.front() {
                        if app.session.question_selected_index + 1 < req.options.len() {
                            app.session.question_selected_index += 1;
                        }
                    }
                    None
                }
                Key::Enter if !answerable => None,
                Key::Enter => {
                    if let Some(req) = app.session.question_queue.front().cloned() {
                        // FUNC-060: clamp the selection to the current option
                        // set. A stale index (e.g. the queue advanced while the
                        // selection was mid-flight) must never submit a blank
                        // answer to the agent. The Up/Down handlers keep the
                        // index in range, so an out-of-range value here means
                        // the option list shrank; treat it as "no selection"
                        // and keep the dialog open rather than answering empty.
                        let selected = app
                            .session
                            .question_selected_index
                            .min(req.options.len().saturating_sub(1));
                        if let Some(response) = req.options.get(selected).cloned() {
                            app.event_bus.publish(
                                codemason_agent::event::Event::QuestionAnswered {
                                    session_id: req.session_id.clone(),
                                    request_id: req.id.clone(),
                                    response,
                                    secret: req.secret,
                                },
                            );
                            app.session.question_queue.pop_front();
                            app.note_question_front();
                        }
                    }
                    None
                }
                Key::Esc if !answerable => None,
                Key::Esc => {
                    if let Some(req) = app.session.question_queue.front().cloned() {
                        app.event_bus
                            .publish(codemason_agent::event::Event::QuestionAnswered {
                                session_id: req.session_id.clone(),
                                request_id: req.id.clone(),
                                response: "[User dismissed question]".to_string(),
                                secret: req.secret,
                            });
                    }
                    app.session.question_queue.pop_front();
                    app.note_question_front();
                    None
                }
                _ => None,
            };
        }

        // Free-text question: accept typed input, submit on Enter, cancel on Esc.
        return match key.key {
            Key::Enter | Key::Esc if !answerable => None,
            Key::Enter => {
                if let Some(req) = app.session.question_queue.front().cloned() {
                    let response = app.session.pending_question_input.trim().to_string();
                    if !response.is_empty() {
                        app.event_bus
                            .publish(codemason_agent::event::Event::QuestionAnswered {
                                session_id: req.session_id.clone(),
                                request_id: req.id.clone(),
                                response,
                                secret: req.secret,
                            });
                        app.session.question_queue.pop_front();
                        app.note_question_front();
                    }
                }
                None
            }
            Key::Esc => {
                if let Some(req) = app.session.question_queue.front().cloned() {
                    app.event_bus
                        .publish(codemason_agent::event::Event::QuestionAnswered {
                            session_id: req.session_id.clone(),
                            request_id: req.id.clone(),
                            response: "[User dismissed question]".to_string(),
                            secret: req.secret,
                        });
                }
                app.session.question_queue.pop_front();
                app.note_question_front();
                None
            }
            Key::Backspace => {
                app.session.pending_question_input.pop();
                None
            }
            Key::Char(c) => {
                app.session.pending_question_input.push(c);
                None
            }
            _ => None,
        };
    }

    // If permission dialog is active, intercept keys
    if !app.session.permission_queue.is_empty() {
        // Standard permission dialog: y/a/n only. An answer counts only once
        // the front request has been shown for the answer delay, so a double
        // press cannot grant the next request unread; the answered request
        // leaves the queue at once (see `App::answer_front_permission`).
        let (decision, verb) = match key.key {
            Key::Char('y') => (
                codemason_agent::permission::PermissionDecision::Once,
                "grant",
            ),
            Key::Char('a') => (
                codemason_agent::permission::PermissionDecision::Always,
                "grant (always)",
            ),
            Key::Char('n') => (
                codemason_agent::permission::PermissionDecision::Deny,
                "deny",
            ),
            _ => return None,
        };
        app.note_permission_front();
        if app.permission_answerable() {
            app.answer_front_permission(decision, verb);
        }
        return None;
    }

    // The single-button notice dialog has nothing to decide: `Enter` and `Esc`
    // both close it, and every other key is swallowed so it cannot mutate the
    // editable input buffer behind the modal. Intercepted before the confirm
    // dialogs so a notice raised over one is not reachable through it.
    if app.notice_open {
        match key.key {
            Key::Enter | Key::Esc => return Some(InputAction::DismissNotice),
            _ => return None,
        }
    }

    // If the `Clear the input queue?` confirmation dialog (opened by the
    // queue-control menu's `Clear` row) is active, intercept keys before any
    // other dialog. It reuses the shared modal key-dispatch machinery
    // (NFR-010): `Left`/`Right` (and `Tab`/`BackTab`) move between `Yes` and
    // `No`, `Enter` selects, `Esc` dismisses. Every other key is swallowed so
    // it cannot mutate the editable input buffer (FR-035, FR-037).
    if app.queue_clear_confirm_open {
        match key.key {
            Key::Left | Key::Right | Key::Tab | Key::BackTab => {
                app.queue_clear_confirm_toggle();
                // NFR-011: the selection change must paint on the next frame.
                app.needs_redraw = true;
                return None;
            }
            Key::Enter => {
                return Some(if app.queue_clear_confirm_is_yes() {
                    InputAction::ConfirmQueueClear
                } else {
                    InputAction::CancelQueueClear
                });
            }
            Key::Esc => return Some(InputAction::CancelQueueClear),
            _ => return None,
        }
    }

    // If the `Clear this project's memory?` confirmation dialog (opened by
    // `/memory clear`) is active, intercept keys before any other dialog.
    // `Left`/`Right` (and `Tab`/`BackTab`) move between `Yes` and `No`, `Enter`
    // selects, `Esc` dismisses. Every other key is swallowed so it cannot mutate
    // the editable input buffer.
    if app.memory_clear_confirm_open {
        match key.key {
            Key::Left | Key::Right | Key::Tab | Key::BackTab => {
                app.memory_clear_confirm_toggle();
                app.needs_redraw = true;
                return None;
            }
            Key::Enter => {
                return Some(if app.memory_clear_confirm_is_yes() {
                    InputAction::ConfirmMemoryClear
                } else {
                    InputAction::CancelMemoryClear
                });
            }
            Key::Esc => return Some(InputAction::CancelMemoryClear),
            _ => return None,
        }
    }

    // If the `Prune messages older than N days?` confirmation dialog (opened by
    // `/db prune`) is active, intercept keys before any other dialog. Same
    // two-option shape as the memory dialog above.
    if app.db_prune_confirm_open {
        match key.key {
            Key::Left | Key::Right | Key::Tab | Key::BackTab => {
                app.db_prune_confirm_toggle();
                app.needs_redraw = true;
                return None;
            }
            Key::Enter => {
                return Some(if app.db_prune_confirm_is_yes() {
                    InputAction::ConfirmDbPrune
                } else {
                    InputAction::CancelDbPrune
                });
            }
            Key::Esc => return Some(InputAction::CancelDbPrune),
            _ => return None,
        }
    }

    // If a router save confirmation modal is active, intercept Enter/Esc
    if app.pending_router_save.is_some() {
        match key.key {
            Key::Enter => return Some(InputAction::ConfirmRouterSave),
            Key::Esc => return Some(InputAction::CancelRouterSave),
            _ => return None,
        }
    }

    // If a forcecleanup confirmation modal is active, intercept Enter/Esc
    if app.session.pending_forcecleanup.is_some() {
        match key.key {
            Key::Enter => return Some(InputAction::ConfirmForceCleanup),
            Key::Esc => return Some(InputAction::CancelForceCleanup),
            _ => return None,
        }
    }

    // If a rollback offer is pending after a terminated loop, intercept
    // Enter/Esc (FR-020, T-013): Enter rolls the workspace back to the
    // pre-loop snapshot, Esc keeps the changes. All other keys are consumed
    // so stray typing does not leak into the input box mid-decision.
    if app.session.pending_rollback.is_some() {
        match key.key {
            Key::Enter => return Some(InputAction::ConfirmRollback),
            Key::Esc => return Some(InputAction::CancelRollback),
            _ => return None,
        }
    }

    // If a plan approval dialog is active, intercept keys
    if let Some(ref state) = app.session.plan_approval_pending {
        let cursor_approve = state.cursor_approve;
        match key.key {
            Key::Enter => {
                if cursor_approve {
                    return Some(InputAction::ApprovePlan);
                } else {
                    return Some(InputAction::RejectPlan);
                }
            }
            Key::Left | Key::Right => return Some(InputAction::TogglePlanCursor),
            Key::Char('r') | Key::Esc => return Some(InputAction::RejectPlan),
            _ => return None,
        }
    }

    // If slash menu is active, intercept navigation keys
    if app.slash_menu.is_some() {
        match key.key {
            Key::Up => {
                if let Some(ref mut menu) = app.slash_menu {
                    if !menu.matches.is_empty() {
                        menu.selected = if menu.selected == 0 {
                            menu.matches.len() - 1
                        } else {
                            menu.selected - 1
                        };
                    }
                }
                return None;
            }
            Key::Down => {
                if let Some(ref mut menu) = app.slash_menu {
                    if !menu.matches.is_empty() {
                        menu.selected = (menu.selected + 1) % menu.matches.len();
                    }
                }
                return None;
            }
            Key::Enter => {
                // Slash commands are queueable like plain messages (FR-017
                // amendment): a `SlashCommand` action is emitted while the agent
                // executes and the submit handler enqueues it. No busy refusal.
                // Select the highlighted command, or use the typed text.
                // If the user typed more than just the trigger, preserve the full
                // input so subcommands and arguments are not lost.
                let command = {
                    let menu = app.slash_menu.as_ref()?;
                    let raw = app.session.input.trim_end().to_string();
                    if let Some(entry) = menu.matches.get(menu.selected) {
                        let with_slash = format!("/{}", entry.trigger);
                        // Raw input extends beyond the matched trigger with a space -> use raw
                        if raw.starts_with(&with_slash)
                            && raw.len() > with_slash.len()
                            && raw.as_bytes().get(with_slash.len()) == Some(&b' ')
                        {
                            raw
                        } else {
                            format!("/{}", entry.trigger)
                        }
                    } else {
                        menu.filter.clone()
                    }
                };
                return Some(InputAction::SlashCommand(command));
            }
            Key::Tab => {
                app.accept_slash_menu_selection();
                return None;
            }
            Key::Esc => {
                app.slash_menu = None;
                app.set_cursor_char_index_clamped(app.input_len_chars());
                return None;
            }
            Key::Char(c) => {
                app.insert_char_at_cursor(c);
                return None;
            }
            Key::Backspace => {
                app.delete_prev_char();
                return None;
            }
            _ => return None,
        }
    }

    // If file menu is active, intercept navigation keys while still allowing
    // normal in-line editing and cursor motion.
    if app.file_menu.is_some() {
        match key.key {
            Key::Up => {
                if let Some(ref mut menu) = app.file_menu
                    && !menu.matches.is_empty()
                {
                    menu.selected = if menu.selected == 0 {
                        menu.matches.len() - 1
                    } else {
                        menu.selected - 1
                    };
                    const FILE_MENU_VISIBLE_ROWS: usize = 8;
                    if menu.selected < menu.scroll_offset {
                        menu.scroll_offset = menu.selected;
                    } else if menu.selected + 1 < FILE_MENU_VISIBLE_ROWS {
                        menu.scroll_offset = 0;
                    }
                }
                return None;
            }
            Key::Down => {
                if let Some(ref mut menu) = app.file_menu
                    && !menu.matches.is_empty()
                {
                    menu.selected = (menu.selected + 1) % menu.matches.len();
                    const FILE_MENU_VISIBLE_ROWS: usize = 8;
                    if menu.selected >= menu.scroll_offset + FILE_MENU_VISIBLE_ROWS {
                        menu.scroll_offset = menu.selected + 1 - FILE_MENU_VISIBLE_ROWS;
                    }
                }
                return None;
            }
            Key::Tab => {
                // If the menu is showing a directory, Tab navigates into it;
                // if it is a file, insert it and close the menu.
                let _ = app.accept_file_menu_selection(); // INTENTIONAL: acceptance flag, not a fallible error
                return None;
            }
            Key::Enter => {
                // Accept selection only. Sending is a separate Enter after menu closes.
                let _ = app.accept_file_menu_selection(); // INTENTIONAL: acceptance flag, not a fallible error
                return None;
            }
            Key::Esc => {
                app.file_menu = None;
                return None;
            }
            Key::Char('\\') if key.mods.contains(Modifiers::CTRL) => {
                app.file_menu_show_hidden = !app.file_menu_show_hidden;
                app.refresh_input_menus();
                return None;
            }
            Key::Char('a') if key.mods.contains(Modifiers::CTRL) => {
                return Some(InputAction::MoveCursorHome);
            }
            Key::Char('e') if key.mods.contains(Modifiers::CTRL) => {
                return Some(InputAction::MoveCursorEnd);
            }
            Key::Char('b') if key.mods.contains(Modifiers::CTRL) => {
                return Some(InputAction::MoveCursorLeft);
            }
            Key::Char('f') if key.mods.contains(Modifiers::CTRL) => {
                return Some(InputAction::MoveCursorRight);
            }
            Key::Char('w') if key.mods.contains(Modifiers::CTRL) => {
                return Some(InputAction::DeletePrevWord);
            }
            Key::Char('k') if key.mods.contains(Modifiers::CTRL) => {
                return Some(InputAction::DeleteToLineEnd);
            }
            Key::Char('u') if key.mods.contains(Modifiers::CTRL) => {
                return Some(InputAction::ClearLine);
            }
            Key::Left if key.mods.contains(Modifiers::CTRL) => {
                return Some(InputAction::MoveCursorWordLeft);
            }
            Key::Right if key.mods.contains(Modifiers::CTRL) => {
                return Some(InputAction::MoveCursorWordRight);
            }
            Key::Home if key.mods.contains(Modifiers::CTRL) => {
                return Some(InputAction::MoveCursorHome);
            }
            Key::End if key.mods.contains(Modifiers::CTRL) => {
                return Some(InputAction::MoveCursorEnd);
            }
            Key::Left => return Some(InputAction::MoveCursorLeft),
            Key::Right => return Some(InputAction::MoveCursorRight),
            Key::Home => return Some(InputAction::MoveCursorHome),
            Key::End => return Some(InputAction::MoveCursorEnd),
            Key::Delete => return Some(InputAction::Delete),
            Key::Char(c) => {
                app.insert_char_at_cursor(c);
                return None;
            }
            Key::Backspace => {
                app.delete_prev_char();
                return None;
            }
            _ => return None,
        }
    }

    // Research markdown viewer: intercept scroll/close keys.
    if app.research_view.is_some() {
        return match key.key {
            Key::PageUp if key.mods.contains(Modifiers::CTRL) => {
                Some(InputAction::ResearchViewToStart)
            }
            Key::PageDown if key.mods.contains(Modifiers::CTRL) => {
                Some(InputAction::ResearchViewToEnd)
            }
            Key::PageUp => Some(InputAction::ResearchViewPageUp),
            Key::PageDown => Some(InputAction::ResearchViewPageDown),
            Key::Up => Some(InputAction::ResearchViewLineUp),
            Key::Down => Some(InputAction::ResearchViewLineDown),
            Key::Esc => {
                app.research_view = None;
                None
            }
            _ => None,
        };
    }

    if app.memory_view.is_some() {
        return match key.key {
            Key::PageUp if key.mods.contains(Modifiers::CTRL) => {
                Some(InputAction::MemoryViewToStart)
            }
            Key::PageDown if key.mods.contains(Modifiers::CTRL) => {
                Some(InputAction::MemoryViewToEnd)
            }
            Key::PageUp => Some(InputAction::MemoryViewPageUp),
            Key::PageDown => Some(InputAction::MemoryViewPageDown),
            Key::Up => Some(InputAction::MemoryViewLineUp),
            Key::Down => Some(InputAction::MemoryViewLineDown),
            Key::Esc => {
                app.memory_view = None;
                None
            }
            _ => None,
        };
    }

    if app.output_view.is_some() {
        return match key.key {
            Key::PageUp if key.mods.contains(Modifiers::CTRL) => {
                Some(InputAction::OutputViewToStart)
            }
            Key::PageDown if key.mods.contains(Modifiers::CTRL) => {
                Some(InputAction::OutputViewToEnd)
            }
            Key::PageUp => Some(InputAction::OutputViewPageUp),
            Key::PageDown => Some(InputAction::OutputViewPageDown),
            // Alt+Down/Up cycle teammate focus even while output view is open
            Key::Down if key.mods.contains(Modifiers::ALT) => Some(InputAction::FocusNextTeammate),
            Key::Up if key.mods.contains(Modifiers::ALT) => Some(InputAction::FocusPrevTeammate),
            Key::Esc => {
                app.close_output_view();
                None
            }
            _ => None,
        };
    }

    // Session sidebar (spec MULTISESSION M5): its global shortcuts, and every
    // key while it has focus, after all modal dialogs had their chance.
    if crate::sessions_sidebar_state::handle_key(app, key) {
        return None;
    }

    match key.key {
        Key::Enter if key.mods.contains(Modifiers::SHIFT) || key.mods.contains(Modifiers::ALT) => {
            // Shift+Enter (Kitty protocol) or Alt+Enter (universal fallback):
            // insert a literal newline without sending the message.
            app.clear_kb_selection();
            app.insert_char_at_cursor('\n');
            None
        }
        Key::Enter if app.show_memory && app.session.input.is_empty() => {
            // With the Memory side panel visible and an empty prompt, Enter
            // opens the selected memory. When the prompt has text, Enter
            // falls through to the normal send path so the memory panel
            // does not intercept message submission.
            Some(InputAction::OpenMemoryView)
        }
        Key::Enter => {
            let text = app.session.input.clone();
            if text.is_empty() {
                return None;
            }
            // FR-002/FR-012: a plain message is accepted while the primary
            // agent is executing; the submit path queues it instead of
            // rejecting it. FR-017 (amended): slash commands are queued the
            // same way; only bang commands keep their existing busy behaviour
            // while a turn is running. FR-058 (amended): teammate focus no
            // longer redirects the input box, so a focused teammate is not a
            // reason to refuse a send.
            let is_bang = text.starts_with('!');
            if is_bang && app.is_input_blocked() {
                app.status = "busy - wait for the current turn to finish".to_string();
                return None;
            }
            if text.starts_with('/') {
                return Some(InputAction::SlashCommand(text));
            }
            if text.starts_with('!') {
                return Some(InputAction::BangCommand(text));
            }
            Some(InputAction::SendMessage(text))
        }
        Key::Char('x') if key.mods.contains(Modifiers::CTRL) => {
            if app.session.is_processing {
                Some(InputAction::CancelAgent)
            } else {
                Some(InputAction::CutToClipboard)
            }
        }
        Key::Char('v')
            if key.mods.contains(Modifiers::CTRL) && !key.mods.contains(Modifiers::ALT) =>
        {
            Some(InputAction::PasteFromClipboard)
        }
        Key::Char('v') if key.mods.contains(Modifiers::ALT) => {
            // Alt+V: paste image from clipboard as a staged attachment.
            app.paste_image_from_clipboard();
            None
        }
        Key::Char('?') if app.session.input.is_empty() && app.shortcuts_panel_enabled => {
            // Show keybindings help panel when '?' is typed on an empty input.
            app.show_shortcuts = true;
            None
        }
        Key::Char('a') if key.mods.contains(Modifiers::CTRL) => Some(InputAction::SelectAll),
        Key::Char('e') if key.mods.contains(Modifiers::CTRL) => Some(InputAction::MoveCursorEnd),
        Key::Char('b') if key.mods.contains(Modifiers::CTRL) => Some(InputAction::MoveCursorLeft),
        Key::Char('f') if key.mods.contains(Modifiers::CTRL) => Some(InputAction::MoveCursorRight),
        Key::Char('w') if key.mods.contains(Modifiers::CTRL) => Some(InputAction::DeletePrevWord),
        Key::Char('k') if key.mods.contains(Modifiers::CTRL) => Some(InputAction::DeleteToLineEnd),
        Key::Char('u') if key.mods.contains(Modifiers::CTRL) => Some(InputAction::ClearLine),
        Key::Char('l') if key.mods.contains(Modifiers::ALT) => Some(InputAction::ToggleLog),
        Key::Char('p') if key.mods.contains(Modifiers::ALT) => Some(InputAction::ToggleProfile),
        // Alt+I toggles the TODO panel (placed before generic char-insert
        // handling so the `i` is never inserted into the input buffer - NFR-002).
        Key::Char('i') if key.mods.contains(Modifiers::ALT) => Some(InputAction::ToggleTasksPanel),
        // Alt+M toggles the Memory side panel (placed before generic char-insert
        // handling so the `m` is never inserted into the input buffer - NFR-002,
        // FR-011).
        Key::Char('m') if key.mods.contains(Modifiers::ALT) => Some(InputAction::ToggleMemory),
        // Memory panel cursor navigation: when the panel is visible, Up/Down
        // move the block cursor through memories instead of navigating input
        // history (FR-016).
        Key::Up if app.show_memory => Some(InputAction::MemoryCursorUp),
        Key::Down if app.show_memory => Some(InputAction::MemoryCursorDown),
        // Open / delete the selected memory when the Memory panel is focused.
        // Alt+O toggles autopilot mode (placed before generic char-insert handling
        // so the `o` is never inserted into the input buffer - NFR-002). The
        // Telemetry side panel is still reachable via `/telemetry_panel`.
        Key::Char('o') if key.mods.contains(Modifiers::ALT) => Some(InputAction::ToggleAutopilot),
        Key::Char('c') if key.mods.contains(Modifiers::ALT) => {
            Some(InputAction::ToggleContextPanel)
        }
        Key::Char('y') if key.mods.contains(Modifiers::ALT) => Some(InputAction::ToggleYolo),
        // Alt+E toggles edit-operation logging.
        Key::Char('e') if key.mods.contains(Modifiers::ALT) => Some(InputAction::ToggleEditLog),
        // Alt+G toggles GCF tool-result encoding (placed before generic
        // char-insert handling so the `g` is never inserted into the input
        // buffer - NFR-002).
        Key::Char('g') if key.mods.contains(Modifiers::ALT) => Some(InputAction::ToggleGcf),
        // Alt+A toggles the Agents window (spec `agentssetup` FR-022: the panel
        // is openable from the keyboard alone). It resolves to an
        // `InputAction`, not an inline mutation, so the dispatch arm can call
        // the same `App::toggle_agents_window` the Agents button's click uses -
        // one visibility path, not a keyboard-only lookalike. Placed before the
        // generic char-insert arm so the `a` is never typed into the input
        // buffer (NFR-002). ALT was otherwise unbound for `a`, so no other key
        // behaviour changes (NFR-009).
        Key::Char('a') if key.mods.contains(Modifiers::ALT) => {
            Some(InputAction::ToggleAgentsWindow)
        }
        // Alt+T toggles the Teams window (spec `agentssetup` FR-045), the twin
        // of the Alt+A arm above and resolved to an `InputAction` for the same
        // reason: the dispatch arm calls the very `App::toggle_teams_window`
        // the Teams button's click calls, so the keyboard and the mouse cannot
        // drift apart. Placed before the generic char-insert arm so the `t` is
        // never typed into the input buffer (NFR-002). ALT was otherwise
        // unbound for `t`, so no other key behaviour changes (NFR-009).
        Key::Char('t') if key.mods.contains(Modifiers::ALT) => Some(InputAction::ToggleTeamsWindow),
        // Alt+Q opens the queue-control menu overlay (spec `inputqueue` FR-021).
        // Placed before the generic char-insert arm so the `q` is never typed
        // into the input buffer (FR-022, FR-031). ALT was otherwise unbound for
        // `q`, so no other key behaviour changes (NFR-009).
        Key::Char('q') if key.mods.contains(Modifiers::ALT) => Some(InputAction::OpenQueueMenu),
        // An Alt+character the arms above do not bind (e.g. Alt+X, whose
        // stop-agent shortcut was removed: Esc stops a running agent) is
        // ignored, never typed into the input buffer (NFR-002). Ctrl+Alt is
        // left alone, since some terminals report AltGr characters that way.
        Key::Char(_)
            if key.mods.contains(Modifiers::ALT) && !key.mods.contains(Modifiers::CTRL) =>
        {
            None
        }
        Key::Char(c) => {
            if app.is_input_locked() {
                app.status = "busy - wait for the current turn to finish".to_string();
                return None;
            }
            // Typing a character replaces the active keyboard selection.
            if let Some((start, end)) = app.kb_selection_char_range() {
                app.remove_input_char_range(start, end);
                app.session.kb_select_anchor = None;
            }
            app.insert_char_at_cursor(c);
            None
        }
        Key::Backspace => {
            // Backspace deletes the selection when one is active.
            if let Some((start, end)) = app.kb_selection_char_range() {
                app.remove_input_char_range(start, end);
                app.session.kb_select_anchor = None;
            } else {
                app.delete_prev_char();
            }
            None
        }
        Key::Delete => {
            // Delete removes the active selection, otherwise the character
            // under the cursor (forward delete).  When the Memory panel is
            // focused and the prompt is empty, Delete prompts to delete the
            // selected memory instead.
            if app.show_memory && app.session.input.is_empty() {
                Some(InputAction::PromptMemoryDelete)
            } else if let Some((start, end)) = app.kb_selection_char_range() {
                app.remove_input_char_range(start, end);
                app.session.kb_select_anchor = None;
                None
            } else {
                app.delete_next_char();
                None
            }
        }
        Key::Left if key.mods.contains(Modifiers::CTRL) && key.mods.contains(Modifiers::SHIFT) => {
            Some(InputAction::SelectWordLeft)
        }
        Key::Right if key.mods.contains(Modifiers::CTRL) && key.mods.contains(Modifiers::SHIFT) => {
            Some(InputAction::SelectWordRight)
        }
        Key::Left if key.mods.contains(Modifiers::CTRL) => Some(InputAction::MoveCursorWordLeft),
        Key::Right if key.mods.contains(Modifiers::CTRL) => Some(InputAction::MoveCursorWordRight),
        Key::Home if key.mods.contains(Modifiers::CTRL) => Some(InputAction::MoveCursorHome),
        Key::End if key.mods.contains(Modifiers::CTRL) => Some(InputAction::MoveCursorEnd),
        Key::Left if key.mods.contains(Modifiers::SHIFT) => Some(InputAction::SelectCharLeft),
        Key::Right if key.mods.contains(Modifiers::SHIFT) => Some(InputAction::SelectCharRight),
        Key::Left => Some(InputAction::MoveCursorLeft),
        Key::Right => Some(InputAction::MoveCursorRight),
        Key::Home => Some(InputAction::MoveCursorHome),
        Key::End => Some(InputAction::MoveCursorEnd),
        Key::Up if key.mods.contains(Modifiers::SHIFT) => Some(InputAction::ScrollUp),
        Key::Down if key.mods.contains(Modifiers::SHIFT) => Some(InputAction::ScrollDown),
        Key::Down if key.mods.contains(Modifiers::ALT) => Some(InputAction::FocusNextTeammate),
        Key::Up if key.mods.contains(Modifiers::ALT) => Some(InputAction::FocusPrevTeammate),
        Key::Up => Some(InputAction::HistoryUp),
        Key::Down => Some(InputAction::HistoryDown),
        Key::PageUp if key.mods.contains(Modifiers::CTRL) => Some(InputAction::LogScrollUp),
        Key::PageDown if key.mods.contains(Modifiers::CTRL) => Some(InputAction::LogScrollDown),
        Key::PageUp => Some(InputAction::ScrollUp),
        Key::PageDown => Some(InputAction::ScrollDown),
        Key::Tab | Key::BackTab => Some(InputAction::SwitchAgent),
        Key::Esc if app.session.is_processing => Some(InputAction::CancelAgent),
        _ => None,
    }
}

/// Handle key events inside the provider setup dialog.
///
/// `pub(crate)` so the mouse router can drive the same cursor movement from a
/// wheel notch: every list-bearing setup selector (provider, model, router
/// model, Azure resource, thinking level, agent, router panes) then scrolls
/// through exactly one code path, so the wheel and `Up`/`Down` can never
/// disagree about the cursor or the clamped window.
pub(crate) fn handle_provider_setup_key(app: &mut App, key: KeyInput) {
    // Escape always closes non-router setup dialogs. Router dialogs handle Esc
    // internally so that the model picker can cancel back to the cluster panel
    // without discarding the whole flow.
    if key.key == Key::Esc
        && !matches!(
            app.provider_setup,
            Some(
                ProviderSetupStep::SetupRouter { .. } | ProviderSetupStep::SelectRouterModel { .. }
            )
        )
    {
        app.provider_setup = None;
        return;
    }

    let Some(step) = app.provider_setup.take() else {
        return;
    };

    match step {
        ProviderSetupStep::SelectProvider {
            selected,
            force_key_entry,
        } => match key.key {
            Key::Up => {
                let new = if selected == 0 {
                    PROVIDER_LIST.len() - 1
                } else {
                    selected - 1
                };
                app.provider_setup = Some(ProviderSetupStep::SelectProvider {
                    selected: new,
                    force_key_entry,
                });
            }
            Key::Down => {
                let new = (selected + 1) % PROVIDER_LIST.len();
                app.provider_setup = Some(ProviderSetupStep::SelectProvider {
                    selected: new,
                    force_key_entry,
                });
            }
            Key::Enter => {
                let (pid, pname) = PROVIDER_LIST[selected];
                // If the provider is already configured and the dialog was opened
                // via `/model` (force_key_entry == false), skip straight to the
                // model picker. When opened via `/provider` (force_key_entry ==
                // true) the API-key prompt is always shown so the user can edit
                // the key.
                //
                // The Model Router is an exception: it is a virtual provider with
                // no API key of its own, so selecting it always opens the cluster
                // setup UI regardless of force_key_entry.
                //
                // Azure Resource is also an exception: it has no API key, only a
                // resource-file picker, so it always opens that picker.
                let already_configured = App::get_configured_providers(&app.storage)
                    .iter()
                    .any(|p| p.id == pid);
                if already_configured && !force_key_entry {
                    if pid == "azure_resource" {
                        app.refresh_provider();
                        let provider =
                            codemason_agent::provider::azure_resource::AzureResourceProvider::new();
                        let entries = provider.entries();
                        if entries.is_empty() {
                            app.provider_setup = Some(ProviderSetupStep::SelectAzureResource {
                                entries: Vec::new(),
                                selected: 0,
                                error: Some(
                                    "No Azure resources found. Check azureresources.json."
                                        .to_string(),
                                ),
                            });
                        } else {
                            // Try to restore last selection
                            let mut selected = 0usize;
                            if let Ok(Some(last)) =
                                app.storage.get_setting("azure_resource_last_selection")
                            {
                                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&last)
                                {
                                    if let Some(last_id) = parsed.get("id").and_then(|v| v.as_str())
                                    {
                                        if let Some(idx) =
                                            entries.iter().position(|e| e.id == last_id)
                                        {
                                            selected = idx;
                                        } else {
                                            let _ = app  // INTENTIONAL: fallible wiring/persistence call; the in-memory state is authoritative
                                                .storage
                                                .delete_setting("azure_resource_last_selection");
                                        }
                                    }
                                }
                            }
                            app.provider_setup = Some(ProviderSetupStep::SelectAzureResource {
                                entries,
                                selected,
                                error: None,
                            });
                        }
                        return;
                    }
                    if pid == "router" {
                        let providers = App::get_configured_providers_for_router(&app.storage);
                        if providers.is_empty() {
                            app.status =
                                "[warn] No concrete providers - configure one first".to_string();
                            app.push_log_no_agent(
                                crate::app::LogLevel::Warn,
                                "provider router: no concrete providers configured".to_string(),
                            );
                            app.provider_setup = Some(ProviderSetupStep::SelectProvider {
                                selected,
                                force_key_entry,
                            });
                        } else {
                            app.provider_setup = Some(app.seeded_router_setup_step(providers));
                        }
                        return;
                    }
                    app.refresh_provider();
                    app.provider_setup = Some(ProviderSetupStep::LoadingModels {
                        provider_id: pid.to_string(),
                        provider_name: pname.to_string(),
                    });
                    app.start_model_discovery(pid.to_string(), pname.to_string());
                } else if pid == "ollama" {
                    // Ollama doesn't require a key - store empty and mark configured
                    let _ = app.storage.set_provider_auth(pid, ""); // INTENTIONAL: best-effort persistence; the in-memory state is authoritative
                    let _ = app // INTENTIONAL: fallible wiring/persistence call; the in-memory state is authoritative
                        .storage
                        .delete_setting(&format!("provider_{pid}_disabled"));
                    app.refresh_provider();
                    app.provider_setup = Some(ProviderSetupStep::LoadingModels {
                        provider_id: pid.to_string(),
                        provider_name: pname.to_string(),
                    });
                    app.start_model_discovery(pid.to_string(), pname.to_string());
                } else if pid == "copilot" {
                    // Copilot: try auto-discover and verify token exchange
                    let storage = app.storage.clone();
                    let db_lookup = || {
                        storage
                            .get_provider_auth("copilot")
                            .ok()
                            .flatten()
                            .filter(|k| !k.is_empty())
                    };
                    let token = codemason_agent::provider::copilot::resolve_copilot_github_token(
                        Some(&db_lookup),
                    );
                    if let Some(ref tk) = token {
                        // Token exchange is a network call - run it in a
                        // background task so the UI thread never blocks
                        // (FR-002, FR-004).  Show the loading spinner while we
                        // wait for `CopilotTokenExchangeResult`.
                        if let Ok(handle) = tokio::runtime::Handle::try_current() {
                            app.provider_setup = Some(ProviderSetupStep::LoadingModels {
                                provider_id: pid.to_string(),
                                provider_name: pname.to_string(),
                            });
                            app.model_loading_state = Some(crate::app::ModelLoadingState {
                                provider_id: pid.to_string(),
                                provider_name: pname.to_string(),
                                started_at: std::time::Instant::now(),
                            });
                            let event_bus = app.event_bus.clone();
                            let tk_clone = tk.clone();
                            handle.spawn(async move {
                                let result =
                                    codemason_agent::provider::copilot::resolve_copilot_auth(
                                        &tk_clone, None,
                                    )
                                    .await;
                                let (success, api_base, error) = match result {
                                    Ok(auth) => {
                                        if !auth.base_url.contains("models.inference.ai.azure.com")
                                        {
                                            (true, Some(auth.base_url), None)
                                        } else {
                                            // Azure-hosted Copilot endpoint - treat as
                                            // needing device flow for proper setup.
                                            (false, None, None)
                                        }
                                    }
                                    Err(e) => (false, None, Some(format!("{e:#}"))),
                                };
                                event_bus.publish(
                                    codemason_agent::event::Event::CopilotTokenExchangeResult {
                                        success,
                                        api_base,
                                        error,
                                    },
                                );
                            });
                            return;
                        }
                    }
                    // No token or no runtime - start device flow
                    start_copilot_device_flow_setup(app);
                } else if pid == "azure_resource" {
                    // Azure Resource: load entries from azureresources.json
                    let provider =
                        codemason_agent::provider::azure_resource::AzureResourceProvider::new();
                    let entries = provider.entries();
                    if entries.is_empty() {
                        app.provider_setup = Some(ProviderSetupStep::SelectAzureResource {
                            entries: Vec::new(),
                            selected: 0,
                            error: Some(
                                "No Azure resources found. Check azureresources.json.".to_string(),
                            ),
                        });
                    } else {
                        // Try to restore last selection
                        let mut selected = 0usize;
                        if let Ok(Some(last)) =
                            app.storage.get_setting("azure_resource_last_selection")
                        {
                            if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&last) {
                                if let Some(last_id) = parsed.get("id").and_then(|v| v.as_str()) {
                                    if let Some(idx) = entries.iter().position(|e| e.id == last_id)
                                    {
                                        selected = idx;
                                    } else {
                                        let _ = app  // INTENTIONAL: fallible wiring/persistence call; the in-memory state is authoritative
                                            .storage
                                            .delete_setting("azure_resource_last_selection");
                                    }
                                }
                            }
                        }
                        app.provider_setup = Some(ProviderSetupStep::SelectAzureResource {
                            entries,
                            selected,
                            error: None,
                        });
                    }
                } else if pid == "router" {
                    // The Model Router is a virtual provider with no API key of
                    // its own. Selecting it from the provider picker opens the
                    // router cluster setup panel, mirroring `/provider router`.
                    let providers = App::get_configured_providers_for_router(&app.storage);
                    if providers.is_empty() {
                        app.status =
                            "[warn] No concrete providers - configure one first".to_string();
                        app.push_log_no_agent(
                            crate::app::LogLevel::Warn,
                            "provider router: no concrete providers configured".to_string(),
                        );
                        // Keep the picker open so the user can choose a concrete
                        // provider to configure first.
                        app.provider_setup = Some(ProviderSetupStep::SelectProvider {
                            selected,
                            force_key_entry,
                        });
                    } else {
                        app.provider_setup = Some(app.seeded_router_setup_step(providers));
                    }
                } else {
                    // Pre-fill the key field with the existing stored key (if
                    // any) so the user can edit it rather than re-entering
                    // from scratch.
                    let existing_key = app.provider_api_key(pid).unwrap_or_default();
                    app.provider_setup = Some(ProviderSetupStep::EnterKey {
                        provider_id: pid.to_string(),
                        provider_name: pname.to_string(),
                        key_field: crate::input_field::InputField::with_text(existing_key),
                        endpoint_field: if pid == "generic_openai" {
                            crate::input_field::InputField::with_text(
                                app.storage
                                    .get_setting("generic_openai_api_base")
                                    .ok()
                                    .flatten()
                                    .unwrap_or_default(),
                            )
                        } else if pid == "azure_foundry" {
                            crate::input_field::InputField::with_text(
                                app.storage
                                    .get_setting("azure_foundry_api_base")
                                    .ok()
                                    .flatten()
                                    .unwrap_or_default(),
                            )
                        } else {
                            crate::input_field::InputField::new()
                        },
                        active_field: 0,
                        error: None,
                    });
                }
            }
            _ => {
                app.provider_setup = Some(ProviderSetupStep::SelectProvider {
                    selected,
                    force_key_entry,
                });
            }
        },
        ProviderSetupStep::EnterKey {
            provider_id,
            provider_name,
            mut key_field,
            mut endpoint_field,
            mut active_field,
            ..
        } => match key.key {
            Key::Enter => {
                let trimmed = key_field.text().trim().to_string();
                if trimmed.is_empty() {
                    app.provider_setup = Some(ProviderSetupStep::EnterKey {
                        provider_id,
                        provider_name,
                        key_field,
                        endpoint_field,
                        active_field,
                        error: Some("API key cannot be empty.".to_string()),
                    });
                } else if provider_id == "copilot"
                    && codemason_agent::provider::copilot::is_pat_token(&trimmed)
                {
                    app.provider_setup = Some(ProviderSetupStep::EnterKey {
                          provider_id,
                          provider_name,
                          key_field,
                          endpoint_field,
                          active_field,
                          error: Some(
                              "PATs (github_pat_/ghp_) are not supported by                                the Copilot API. Run: gh auth refresh -s copilot"
                                  .to_string(),
                          ),
                      });
                } else {
                    // FUNC-027: a failed credential write must be surfaced, not
                    // silently dropped while the dialog advances as if the key
                    // had been saved.
                    if let Err(e) = app.storage.set_provider_auth(&provider_id, &trimmed) {
                        app.provider_setup = Some(ProviderSetupStep::EnterKey {
                            provider_id,
                            provider_name,
                            key_field,
                            endpoint_field,
                            active_field,
                            error: Some(format!(
                                "Failed to save API key: {e}. Check the credential store and retry."
                            )),
                        });
                        return;
                    }
                    if provider_id == "generic_openai" || provider_id == "azure_foundry" {
                        let endpoint = endpoint_field.text().trim();
                        let endpoint_result = if endpoint.is_empty() {
                            app.storage
                                .delete_setting(&format!("{provider_id}_api_base"))
                        } else {
                            app.storage
                                .set_setting(&format!("{provider_id}_api_base"), endpoint)
                        };
                        if let Err(e) = endpoint_result {
                            app.provider_setup = Some(ProviderSetupStep::EnterKey {
                                provider_id,
                                provider_name,
                                key_field,
                                endpoint_field,
                                active_field,
                                error: Some(format!(
                                    "Failed to save endpoint: {e}. Check the credential store and retry."
                                )),
                            });
                            return;
                        }
                    }
                    let _ = app // INTENTIONAL: fallible wiring/persistence call; the in-memory state is authoritative
                        .storage
                        .delete_setting(&format!("provider_{provider_id}_disabled"));
                    app.refresh_provider();
                    app.provider_setup = Some(ProviderSetupStep::LoadingModels {
                        provider_id: provider_id.clone(),
                        provider_name: provider_name.clone(),
                    });
                    app.start_model_discovery(provider_id, provider_name);
                }
            }
            Key::Tab if provider_id == "generic_openai" || provider_id == "azure_foundry" => {
                active_field = if active_field == 0 { 1 } else { 0 };
                app.provider_setup = Some(ProviderSetupStep::EnterKey {
                    provider_id,
                    provider_name,
                    key_field,
                    endpoint_field,
                    active_field,
                    error: None,
                });
            }
            _ => {
                let is_endpoint = provider_id == "generic_openai" || provider_id == "azure_foundry";
                let target = if is_endpoint && active_field == 1 {
                    &mut endpoint_field
                } else {
                    &mut key_field
                };
                let consumed = target.handle_key(key);
                if !consumed
                    && matches!(key.key, Key::Char('v') | Key::Char('V'))
                    && key.mods.contains(Modifiers::CTRL)
                    && !key.mods.contains(Modifiers::ALT)
                {
                    target.paste_text_from_clipboard();
                }
                app.provider_setup = Some(ProviderSetupStep::EnterKey {
                    provider_id,
                    provider_name,
                    key_field,
                    endpoint_field,
                    active_field,
                    error: None,
                });
            }
        },
        ProviderSetupStep::DeviceFlowPending {
            flow,
            user_code,
            verification_uri,
        } => match key.key {
            Key::Esc => {
                app.provider_setup = None;
            }
            Key::Char('c') => {
                crate::clipboard::set_clipboard_text(&user_code);
                app.status = "[ok] Device code copied to clipboard".to_string();
                app.provider_setup = Some(ProviderSetupStep::DeviceFlowPending {
                    flow,
                    user_code,
                    verification_uri,
                });
            }
            _ => {
                // Keep showing the device flow pending UI - polling happens
                // in a background task and completes via the appropriate
                // device-flow completion event.
                app.provider_setup = Some(ProviderSetupStep::DeviceFlowPending {
                    flow,
                    user_code,
                    verification_uri,
                });
            }
        },
        ProviderSetupStep::SelectModel {
            provider_id,
            provider_name,
            models,
            selected,
        } => match key.key {
            Key::Up => {
                let new = if models.is_empty() {
                    0
                } else if selected == 0 {
                    models.len() - 1
                } else {
                    selected - 1
                };
                app.provider_setup = Some(ProviderSetupStep::SelectModel {
                    provider_id,
                    provider_name,
                    models,
                    selected: new,
                });
            }
            Key::Down => {
                let new = if models.is_empty() {
                    0
                } else {
                    (selected + 1) % models.len()
                };
                app.provider_setup = Some(ProviderSetupStep::SelectModel {
                    provider_id,
                    provider_name,
                    models,
                    selected: new,
                });
            }
            Key::Enter => {
                if let Some(entry) = models.get(selected).cloned() {
                    // Ollama-family providers are boolean thinkers, but model
                    // detection of reasoning support is unreliable. Always
                    // show the reasoning selector so the user can override it.
                    let force_selector =
                        is_ollama_family(&provider_id) || !entry.thinking_levels.is_empty();
                    if force_selector {
                        let default_level = App::default_thinking_level_for_entry(&entry);
                        let thinking_levels =
                            if entry.thinking_levels.is_empty() && is_ollama_family(&provider_id) {
                                codemason_llm::providers::thinking::full_reasoning_levels()
                            } else {
                                entry.thinking_levels.clone()
                            };
                        let selected_level = thinking_levels
                            .iter()
                            .position(|level| *level == default_level)
                            .unwrap_or(0);
                        app.provider_setup = Some(ProviderSetupStep::SelectThinkingLevel {
                            provider_id,
                            provider_name,
                            model: ModelPickerEntry {
                                thinking_levels,
                                ..entry
                            },
                            selected: selected_level,
                        });
                    } else {
                        let model_name = app.finalize_model_selection(
                            provider_id,
                            provider_name.clone(),
                            &entry,
                            ThinkingLevel::Off,
                        );
                        app.provider_setup = Some(ProviderSetupStep::Done {
                            provider_name,
                            model_name: Some(model_name),
                        });
                    }
                } else {
                    app.provider_setup = Some(ProviderSetupStep::Done {
                        provider_name,
                        model_name: None,
                    });
                }
            }
            _ => {
                app.provider_setup = Some(ProviderSetupStep::SelectModel {
                    provider_id,
                    provider_name,
                    models,
                    selected,
                });
            }
        },
        ProviderSetupStep::SelectAzureResource {
            entries,
            selected,
            error,
        } => match key.key {
            Key::Up => {
                let new = if selected == 0 {
                    entries.len().saturating_sub(1)
                } else {
                    selected - 1
                };
                app.provider_setup = Some(ProviderSetupStep::SelectAzureResource {
                    entries,
                    selected: new,
                    error,
                });
            }
            Key::Down => {
                let new = if entries.is_empty() {
                    0
                } else {
                    (selected + 1) % entries.len()
                };
                app.provider_setup = Some(ProviderSetupStep::SelectAzureResource {
                    entries,
                    selected: new,
                    error,
                });
            }
            Key::Enter => {
                if let Some(entry) = entries.get(selected) {
                    // Persist selection (include api_type for re-hydration)
                    let payload = serde_json::json!({
                        "id": entry.id,
                        "endpoint": entry.endpoint,
                        "api_key": entry.api_key,
                        "api_key_env": entry.api_key_env,
                        "api_type": entry.api_type,
                    });
                    // FUNC-027: persist the Azure Resource selection explicitly
                    // and surface a storage failure instead of silently dropping
                    // it while advancing to Done as if the choice was saved.
                    let mut persist_err: Option<anyhow::Error> = None;
                    for (key, value) in [
                        ("azure_resource_last_selection", payload.to_string()),
                        ("preferred_provider", "azure_resource".to_string()),
                        ("azure_resource_api_base", entry.endpoint.clone()),
                        ("selected_model", format!("azure_resource/{}", entry.id)),
                        (
                            "selected_model_ctx_window",
                            entry.context_window.unwrap_or(128_000).to_string(),
                        ),
                    ] {
                        if let Err(e) = app.storage.set_setting(key, &value) {
                            tracing::warn!(key, error = %e, "failed to persist azure_resource selection");
                            persist_err = Some(e);
                        }
                    }
                    if let Some(e) = persist_err {
                        app.provider_setup = Some(ProviderSetupStep::SelectAzureResource {
                            entries,
                            selected,
                            error: Some(format!(
                                "Failed to save selection: {e}. Check the credential store and retry."
                            )),
                        });
                        return;
                    }
                    app.session.selected_model = Some(format!("azure_resource/{}", entry.id));
                    app.session.selected_model_ctx_window =
                        Some(entry.context_window.unwrap_or(128_000));
                    app.configured_provider = Some(ConfiguredProvider {
                        id: "azure_resource".to_string(),
                        name: "Azure Resource (File)".to_string(),
                        source: ProviderSource::Database,
                    });
                    app.provider_setup = Some(ProviderSetupStep::Done {
                        provider_name: "Azure Resource (File)".to_string(),
                        model_name: Some(entry.name.clone()),
                    });
                }
            }
            _ => {
                app.provider_setup = Some(ProviderSetupStep::SelectAzureResource {
                    entries,
                    selected,
                    error,
                });
            }
        },
        ProviderSetupStep::SelectThinkingLevel {
            provider_id,
            provider_name,
            model,
            selected,
        } => match key.key {
            Key::Up => {
                let new = if selected == 0 {
                    model.thinking_levels.len().saturating_sub(1)
                } else {
                    selected - 1
                };
                app.provider_setup = Some(ProviderSetupStep::SelectThinkingLevel {
                    provider_id,
                    provider_name,
                    model,
                    selected: new,
                });
            }
            Key::Down => {
                let new = if model.thinking_levels.is_empty() {
                    0
                } else {
                    (selected + 1) % model.thinking_levels.len()
                };
                app.provider_setup = Some(ProviderSetupStep::SelectThinkingLevel {
                    provider_id,
                    provider_name,
                    model,
                    selected: new,
                });
            }
            Key::Enter => {
                let level = model
                    .thinking_levels
                    .get(selected)
                    .copied()
                    .unwrap_or(ThinkingLevel::Off);
                let model_name =
                    app.finalize_model_selection(provider_id, provider_name.clone(), &model, level);
                app.provider_setup = Some(ProviderSetupStep::Done {
                    provider_name,
                    model_name: Some(model_name),
                });
            }
            _ => {
                app.provider_setup = Some(ProviderSetupStep::SelectThinkingLevel {
                    provider_id,
                    provider_name,
                    model,
                    selected,
                });
            }
        },
        ProviderSetupStep::Done { .. } => {
            // Any key closes the done screen and triggers a health check
            app.provider_setup = None;
            app.check_provider_health();
        }
        ProviderSetupStep::SelectAgent { agents, selected } => match key.key {
            Key::Up => {
                let new = if agents.is_empty() {
                    0
                } else if selected == 0 {
                    agents.len() - 1
                } else {
                    selected - 1
                };
                app.provider_setup = Some(ProviderSetupStep::SelectAgent {
                    agents,
                    selected: new,
                });
            }
            Key::Down => {
                let new = if agents.is_empty() {
                    0
                } else {
                    (selected + 1) % agents.len()
                };
                app.provider_setup = Some(ProviderSetupStep::SelectAgent {
                    agents,
                    selected: new,
                });
            }
            Key::Enter => {
                if let Some((name, _desc, _is_custom)) = agents.get(selected) {
                    if let Some(idx) = app.cycleable_agents.iter().position(|a| a.name == *name) {
                        app.session.current_agent_index = idx;
                        app.session.agent_info = app.cycleable_agents[idx].clone();
                        app.session.agent_name = app.session.agent_info.name.clone();
                        app.status = format!("Agent: {}", app.session.agent_name);
                    }
                }
                app.provider_setup = None;
            }
            _ => {
                app.provider_setup = Some(ProviderSetupStep::SelectAgent { agents, selected });
            }
        },
        ProviderSetupStep::SelectConfiguredProvider {
            providers,
            selected,
        } => match key.key {
            Key::Up => {
                let new = if selected == 0 {
                    providers.len().saturating_sub(1)
                } else {
                    selected - 1
                };
                app.provider_setup = Some(ProviderSetupStep::SelectConfiguredProvider {
                    providers,
                    selected: new,
                });
            }
            Key::Down => {
                let new = if providers.is_empty() {
                    0
                } else {
                    (selected + 1) % providers.len()
                };
                app.provider_setup = Some(ProviderSetupStep::SelectConfiguredProvider {
                    providers,
                    selected: new,
                });
            }
            Key::Enter => {
                if let Some(prov) = providers.get(selected).cloned() {
                    let prov_id = prov.id.clone();
                    let prov_name = prov.name.clone();
                    // FR-003/FR-004: try persisted model restore, fallback to picker.
                    if app
                        .try_restore_provider_model(&prov_id, &prov_name)
                        .is_some()
                    {
                        // Model was restored - show the Done confirmation.
                        app.provider_setup = Some(ProviderSetupStep::Done {
                            provider_name: prov_name,
                            model_name: app
                                .session
                                .selected_model
                                .as_ref()
                                .and_then(|m| crate::app::model_part_from_selected_model(m))
                                .map(String::from),
                        });
                    } else {
                        app.provider_setup = Some(ProviderSetupStep::LoadingModels {
                            provider_id: prov_id.clone(),
                            provider_name: prov_name.clone(),
                        });
                        app.start_model_discovery(prov_id, prov_name);
                    }
                } else {
                    app.provider_setup = None;
                }
            }
            _ => {
                app.provider_setup = Some(ProviderSetupStep::SelectConfiguredProvider {
                    providers,
                    selected,
                });
            }
        },
        ProviderSetupStep::ShowProviderConfig {
            providers,
            selected,
        } => match key.key {
            Key::Up => {
                let new = if selected == 0 {
                    providers.len().saturating_sub(1)
                } else {
                    selected - 1
                };
                app.provider_setup = Some(ProviderSetupStep::ShowProviderConfig {
                    providers,
                    selected: new,
                });
            }
            Key::Down => {
                let new = if providers.is_empty() {
                    0
                } else {
                    (selected + 1) % providers.len()
                };
                app.provider_setup = Some(ProviderSetupStep::ShowProviderConfig {
                    providers,
                    selected: new,
                });
            }
            Key::Enter => {
                if let Some(prov) = providers.get(selected).cloned() {
                    let report = if prov.id == "router" {
                        app.router_config_report(&app.provider_registry.clone())
                    } else {
                        app.provider_config_report(&prov)
                    };
                    app.append_assistant_text(&report);
                    app.status = format!("provider show: {}", prov.name);
                }
                app.provider_setup = None;
            }
            _ => {
                app.provider_setup = Some(ProviderSetupStep::ShowProviderConfig {
                    providers,
                    selected,
                });
            }
        },

        ProviderSetupStep::ResetProvider { selected } => match key.key {
            Key::Up => {
                let new = if selected == 0 {
                    PROVIDER_LIST.len() - 1
                } else {
                    selected - 1
                };
                app.provider_setup = Some(ProviderSetupStep::ResetProvider { selected: new });
            }
            Key::Down => {
                let new = (selected + 1) % PROVIDER_LIST.len();
                app.provider_setup = Some(ProviderSetupStep::ResetProvider { selected: new });
            }
            Key::Enter => {
                let (pid, pname) = PROVIDER_LIST[selected];
                let _ = app.storage.delete_provider_auth(pid); // INTENTIONAL: best-effort persistence; the in-memory state is authoritative
                let _ = app // INTENTIONAL: fallible wiring/persistence call; the in-memory state is authoritative
                    .storage
                    .set_setting(&format!("provider_{pid}_disabled"), "true");
                // Clear provider-specific settings
                let _ = app // INTENTIONAL: fallible wiring/persistence call; the in-memory state is authoritative
                    .storage
                    .delete_setting(&format!("provider_{pid}_last_model"));
                if pid == "copilot" {
                    let _ = app.storage.delete_setting("copilot_api_base"); // INTENTIONAL: best-effort persistence; the in-memory state is authoritative
                } else if pid == "generic_openai" {
                    let _ = app.storage.delete_setting("generic_openai_api_base"); // INTENTIONAL: best-effort persistence; the in-memory state is authoritative
                } else if pid == "azure_foundry" {
                    let _ = app.storage.delete_setting("azure_foundry_api_base"); // INTENTIONAL: best-effort persistence; the in-memory state is authoritative
                }
                let is_active = app
                    .configured_provider
                    .as_ref()
                    .map_or(false, |p| p.id == pid);
                if is_active {
                    app.configured_provider = None;
                    app.session.selected_model = None;
                    app.session.selected_model_ctx_window = None;
                    app.session.selected_thinking_level = None;
                    let _ = app.storage.delete_setting("selected_model"); // INTENTIONAL: best-effort persistence; the in-memory state is authoritative
                    let _ = app.storage.delete_setting("selected_model_ctx_window"); // INTENTIONAL: best-effort persistence; the in-memory state is authoritative
                    let _ = app.storage.delete_setting("thinking_level"); // INTENTIONAL: best-effort persistence; the in-memory state is authoritative
                    let _ = app.storage.delete_setting("thinking_level_explicit"); // INTENTIONAL: best-effort persistence; the in-memory state is authoritative
                    app.session
                        .provider_health
                        .store(0, std::sync::atomic::Ordering::Relaxed);
                }
                app.status = format!("[ok] Provider {} reset - credentials removed", pname);
                app.provider_setup = None;
            }
            _ => {
                app.provider_setup = Some(ProviderSetupStep::ResetProvider { selected });
            }
        },

        // -- GitLab setup (multi-field form) ------------------------------
        ProviderSetupStep::GitLabSetup {
            mut url_input,
            mut url_cursor,
            mut token_input,
            mut token_cursor,
            mut active_field,
            error,
        } => match key.key {
            Key::Tab | Key::BackTab => {
                active_field = if active_field == 0 { 1 } else { 0 };
                app.provider_setup = Some(ProviderSetupStep::GitLabSetup {
                    url_input,
                    url_cursor,
                    token_input,
                    token_cursor,
                    active_field,
                    error,
                });
            }
            Key::Enter => {
                let url = url_input.trim().to_string();
                let tok = token_input.trim().to_string();
                if url.is_empty() || tok.is_empty() {
                    app.provider_setup = Some(ProviderSetupStep::GitLabSetup {
                        url_input,
                        url_cursor,
                        token_input,
                        token_cursor,
                        active_field,
                        error: Some("Both URL and token are required.".to_string()),
                    });
                } else {
                    // Start async validation
                    app.provider_setup = Some(ProviderSetupStep::GitLabValidating {
                        instance_url: url.clone(),
                        token: tok.clone(),
                    });
                    start_gitlab_validation(app, url, tok);
                }
            }
            Key::Char(c) => {
                if active_field == 0 {
                    url_input.insert(url_cursor, c);
                    url_cursor += 1;
                } else {
                    token_input.insert(token_cursor, c);
                    token_cursor += 1;
                }
                app.provider_setup = Some(ProviderSetupStep::GitLabSetup {
                    url_input,
                    url_cursor,
                    token_input,
                    token_cursor,
                    active_field,
                    error: None,
                });
            }
            Key::Backspace => {
                if active_field == 0 {
                    if url_cursor > 0 {
                        url_cursor -= 1;
                        url_input.remove(url_cursor);
                    }
                } else if token_cursor > 0 {
                    token_cursor -= 1;
                    token_input.remove(token_cursor);
                }
                app.provider_setup = Some(ProviderSetupStep::GitLabSetup {
                    url_input,
                    url_cursor,
                    token_input,
                    token_cursor,
                    active_field,
                    error: None,
                });
            }
            Key::Left => {
                if active_field == 0 {
                    url_cursor = url_cursor.saturating_sub(1);
                } else {
                    token_cursor = token_cursor.saturating_sub(1);
                }
                app.provider_setup = Some(ProviderSetupStep::GitLabSetup {
                    url_input,
                    url_cursor,
                    token_input,
                    token_cursor,
                    active_field,
                    error,
                });
            }
            Key::Right => {
                if active_field == 0 {
                    if url_cursor < url_input.len() {
                        url_cursor += 1;
                    }
                } else if token_cursor < token_input.len() {
                    token_cursor += 1;
                }
                app.provider_setup = Some(ProviderSetupStep::GitLabSetup {
                    url_input,
                    url_cursor,
                    token_input,
                    token_cursor,
                    active_field,
                    error,
                });
            }
            Key::Home => {
                if active_field == 0 {
                    url_cursor = 0;
                } else {
                    token_cursor = 0;
                }
                app.provider_setup = Some(ProviderSetupStep::GitLabSetup {
                    url_input,
                    url_cursor,
                    token_input,
                    token_cursor,
                    active_field,
                    error,
                });
            }
            Key::End => {
                if active_field == 0 {
                    url_cursor = url_input.len();
                } else {
                    token_cursor = token_input.len();
                }
                app.provider_setup = Some(ProviderSetupStep::GitLabSetup {
                    url_input,
                    url_cursor,
                    token_input,
                    token_cursor,
                    active_field,
                    error,
                });
            }
            _ => {
                app.provider_setup = Some(ProviderSetupStep::GitLabSetup {
                    url_input,
                    url_cursor,
                    token_input,
                    token_cursor,
                    active_field,
                    error,
                });
            }
        },

        ProviderSetupStep::GitLabValidating {
            instance_url,
            token,
        } => {
            // Esc cancels and returns to the form
            if key.key == Key::Esc {
                app.provider_setup = Some(ProviderSetupStep::GitLabSetup {
                    url_input: instance_url,
                    url_cursor: 0,
                    token_input: token,
                    token_cursor: 0,
                    active_field: 0,
                    error: Some("Validation cancelled.".to_string()),
                });
            } else {
                app.provider_setup = Some(ProviderSetupStep::GitLabValidating {
                    instance_url,
                    token,
                });
            }
        }

        // -- Telemetry setup (multi-field form) ---------------------------
        ProviderSetupStep::TelemetrySetup {
            mut endpoint_field,
            mut protocol,
            mut interval_field,
            mut timeout_field,
            mut port_field,
            mut logs_field,
            mut active_field,
            error: _,
        } => {
            let is_text_field = active_field != 1;
            // FUNC-043: resolve the active text field by index, returning
            // None for the protocol (non-text) field instead of panicking.
            fn telemetry_field<'a>(
                idx: u8,
                endpoint: &'a mut crate::input_field::InputField,
                interval: &'a mut crate::input_field::InputField,
                timeout: &'a mut crate::input_field::InputField,
                port: &'a mut crate::input_field::InputField,
                logs: &'a mut crate::input_field::InputField,
            ) -> Option<&'a mut crate::input_field::InputField> {
                match idx {
                    0 => Some(endpoint),
                    2 => Some(interval),
                    3 => Some(timeout),
                    4 => Some(port),
                    5 => Some(logs),
                    _ => None,
                }
            }
            match key.key {
                Key::Tab => {
                    active_field = (active_field + 1) % 6;
                }
                Key::BackTab => {
                    active_field = if active_field == 0 {
                        5
                    } else {
                        active_field - 1
                    };
                }
                Key::Up | Key::Down if active_field == 1 => {
                    let (next, from_port, to_port) = match protocol {
                        codemason_config::OtelProtocol::Http => {
                            (codemason_config::OtelProtocol::Grpc, ":4318", ":4317")
                        }
                        codemason_config::OtelProtocol::Grpc => {
                            (codemason_config::OtelProtocol::Http, ":4317", ":4318")
                        }
                    };
                    protocol = next;
                    // Follow the protocol with the endpoint's port when it is
                    // still the OTLP default for the old protocol (4318 for
                    // HTTP, 4317 for gRPC); a custom port is left alone.
                    if let Some(swapped) =
                        swap_otlp_default_port(endpoint_field.text(), from_port, to_port)
                    {
                        endpoint_field = crate::input_field::InputField::with_text(&swapped);
                    }
                }
                Key::Enter => {
                    let endpoint = endpoint_field.text().trim().to_string();
                    let interval_str = interval_field.text().trim();
                    let timeout_str = timeout_field.text().trim();
                    let port_str = port_field.text().trim();
                    let logs_str = logs_field.text().trim().to_ascii_lowercase();

                    let interval = interval_str.parse::<u64>().unwrap_or(0);
                    let timeout = timeout_str.parse::<u64>().unwrap_or(0);
                    let internal_port = if port_str.is_empty() {
                        None
                    } else {
                        match port_str.parse::<u16>() {
                            Ok(p) => Some(p),
                            Err(_) => {
                                app.provider_setup = Some(ProviderSetupStep::TelemetrySetup {
                                    endpoint_field,
                                    protocol,
                                    interval_field,
                                    timeout_field,
                                    port_field,
                                    logs_field,
                                    active_field,
                                    error: Some(format!(
                                        "Internal port must be a number between 0 and {}.",
                                        u16::MAX
                                    )),
                                });
                                return;
                            }
                        }
                    };

                    // The logs field is the OTLP logs-signal export level. Log
                    // export follows telemetry itself (spec `otel` FR-036), so
                    // this draft always enables both signals and an empty
                    // field simply keeps the default level.
                    let logs = codemason_config::OtelLogsConfig {
                        enabled: true,
                        level: if logs_str.is_empty() {
                            "info".to_string()
                        } else {
                            logs_str
                        },
                    };
                    // Merge the form onto the stored block: the dialog has no
                    // rows for `service_name`, `resource_attributes`, the
                    // per-metric toggles or `cardinality_limit`, and building
                    // a fresh block reset all of them to their defaults on
                    // every save.
                    let draft = codemason_config::OtelConfig {
                        enabled: true,
                        endpoint,
                        protocol,
                        export_interval_seconds: interval,
                        export_timeout_seconds: timeout,
                        internal_port,
                        logs,
                        ..app.telemetry_config_snapshot()
                    };

                    let problems = draft.validate();
                    if !problems.is_empty() {
                        app.provider_setup = Some(ProviderSetupStep::TelemetrySetup {
                            endpoint_field,
                            protocol,
                            interval_field,
                            timeout_field,
                            port_field,
                            logs_field,
                            active_field,
                            error: Some(problems.join("; ")),
                        });
                        return;
                    }

                    match app.save_telemetry_otel(&draft) {
                        Ok(()) => {
                            app.session_processor.invalidate_config_cache();
                            // Bug fix: `/telemetry setup` used to persist the
                            // new block and stop there, so an operator who had
                            // already enabled telemetry kept exporting with
                            // the OLD endpoint/protocol/interval until the
                            // next restart. Reconfigure the live subsystem
                            // exactly as `/telemetry on` does, so saving the
                            // form takes effect immediately.
                            let reconfigure_error = app
                                .session_processor
                                .telemetry
                                .reconfigure(draft)
                                .err()
                                .map(|e| e.to_string());
                            app.provider_setup = None;
                            let mut message = String::from(
                                "From: /telemetry setup\n[ok] **Telemetry configuration saved.**",
                            );
                            match reconfigure_error {
                                // Reconfiguring a disabled subsystem is a
                                // no-op, and a save with telemetry off is the
                                // expected path for an operator preparing a
                                // config, not a failure to report.
                                None => message.push_str(
                                    "\n\nThe change is already live: the telemetry subsystem \
                                     was reconfigured in place.",
                                ),
                                Some(e) => message.push_str(&format!(
                                    "\n\n[err] The new configuration was saved but could not be \
                                     applied to the running telemetry subsystem: {e}\n\nThe \
                                     settings take effect on the next codemason start."
                                )),
                            }
                            app.append_assistant_text(&message);
                            app.status = "telemetry: saved".to_string();
                        }
                        Err(e) => {
                            app.provider_setup = Some(ProviderSetupStep::TelemetrySetup {
                                endpoint_field,
                                protocol,
                                interval_field,
                                timeout_field,
                                port_field,
                                logs_field,
                                active_field,
                                error: Some(e),
                            });
                        }
                    }
                    return;
                }
                Key::Char(c) if is_text_field => {
                    let Some(target) = telemetry_field(
                        active_field,
                        &mut endpoint_field,
                        &mut interval_field,
                        &mut timeout_field,
                        &mut port_field,
                        &mut logs_field,
                    ) else {
                        // Not a text field (protocol); no-op rather than panic.
                        return;
                    };
                    target.insert_char(c);
                }
                Key::Backspace if is_text_field => {
                    let Some(target) = telemetry_field(
                        active_field,
                        &mut endpoint_field,
                        &mut interval_field,
                        &mut timeout_field,
                        &mut port_field,
                        &mut logs_field,
                    ) else {
                        // Not a text field (protocol); no-op rather than panic.
                        return;
                    };
                    target.backspace();
                }
                Key::Left if is_text_field => {
                    let Some(target) = telemetry_field(
                        active_field,
                        &mut endpoint_field,
                        &mut interval_field,
                        &mut timeout_field,
                        &mut port_field,
                        &mut logs_field,
                    ) else {
                        // Not a text field (protocol); no-op rather than panic.
                        return;
                    };
                    target.move_left();
                }
                Key::Right if is_text_field => {
                    let Some(target) = telemetry_field(
                        active_field,
                        &mut endpoint_field,
                        &mut interval_field,
                        &mut timeout_field,
                        &mut port_field,
                        &mut logs_field,
                    ) else {
                        // Not a text field (protocol); no-op rather than panic.
                        return;
                    };
                    target.move_right();
                }
                Key::Home if is_text_field => {
                    let Some(target) = telemetry_field(
                        active_field,
                        &mut endpoint_field,
                        &mut interval_field,
                        &mut timeout_field,
                        &mut port_field,
                        &mut logs_field,
                    ) else {
                        // Not a text field (protocol); no-op rather than panic.
                        return;
                    };
                    target.move_home();
                }
                Key::End if is_text_field => {
                    let Some(target) = telemetry_field(
                        active_field,
                        &mut endpoint_field,
                        &mut interval_field,
                        &mut timeout_field,
                        &mut port_field,
                        &mut logs_field,
                    ) else {
                        // Not a text field (protocol); no-op rather than panic.
                        return;
                    };
                    target.move_end();
                }
                _ => {}
            }

            app.provider_setup = Some(ProviderSetupStep::TelemetrySetup {
                endpoint_field,
                protocol,
                interval_field,
                timeout_field,
                port_field,
                logs_field,
                active_field,
                error: None,
            });
        }

        ProviderSetupStep::LoadingModels { .. } => {
            // Loading state is read-only; any key besides Esc just keeps it alive.
            if key.key == Key::Esc {
                app.provider_setup = None;
                app.model_loading_state = None;
            } else {
                app.provider_setup = Some(step);
            }
        }
        ProviderSetupStep::SetupRouter { .. } => {
            app.provider_setup = Some(step);
            handle_router_setup_key(app, key);
        }
        ProviderSetupStep::SelectRouterModel { .. } => {
            app.provider_setup = Some(step);
            handle_router_model_picker_key(app, key);
        }
    }
}
/// Starts the Copilot device flow and spawns a background polling task.
///
/// The device-flow start (a network call to `github.com/login/device/code`)
/// is itself offloaded to a background task so the UI thread never blocks
/// (FR-002, FR-004).  When the start completes the background task publishes
/// [`Event::CopilotDeviceFlowStartResult`]; the event handler then sets the
/// `DeviceFlowPending` dialog and spawns the polling task.  On failure the
/// event handler reverts to the key-entry form with an error.
pub(crate) fn start_copilot_device_flow_setup(app: &mut App) {
    let handle = match tokio::runtime::Handle::try_current() {
        Ok(h) => h,
        Err(_) => {
            app.provider_setup = Some(ProviderSetupStep::EnterKey {
                provider_id: "copilot".to_string(),
                provider_name: "GitHub Copilot".to_string(),
                key_field: crate::input_field::InputField::new(),
                endpoint_field: crate::input_field::InputField::new(),
                active_field: 0,
                error: Some("Async runtime not available for device flow.".to_string()),
            });
            return;
        }
    };

    // Show the loading spinner while the device-flow start runs in the
    // background.
    app.provider_setup = Some(ProviderSetupStep::LoadingModels {
        provider_id: "copilot".to_string(),
        provider_name: "GitHub Copilot".to_string(),
    });
    app.model_loading_state = Some(crate::app::ModelLoadingState {
        provider_id: "copilot".to_string(),
        provider_name: "GitHub Copilot".to_string(),
        started_at: std::time::Instant::now(),
    });

    let event_bus = app.event_bus.clone();
    handle.spawn(async move {
        match codemason_agent::provider::copilot::start_copilot_device_flow().await {
            Ok(flow) => {
                event_bus.publish(
                    codemason_agent::event::Event::CopilotDeviceFlowStartResult {
                        user_code: Some(flow.user_code),
                        verification_uri: Some(flow.verification_uri),
                        device_code: Some(flow.device_code),
                        interval: Some(flow.interval),
                        error: None,
                    },
                );
            }
            Err(e) => {
                event_bus.publish(
                    codemason_agent::event::Event::CopilotDeviceFlowStartResult {
                        user_code: None,
                        verification_uri: None,
                        device_code: None,
                        interval: None,
                        error: Some(format!("{e:#}")),
                    },
                );
            }
        }
    });
}

/// Spawns an async task to validate a GitLab PAT and save credentials on success.
///
/// On completion the task publishes an `AgentError` event with the result,
/// and clears `provider_setup` (or reverts to the form with an error).
fn start_gitlab_validation(app: &mut App, instance_url: String, token: String) {
    let event_bus = app.event_bus.clone();
    let sid = app.session.session_id.clone().unwrap_or_default();
    let storage = app.storage.clone();

    let handle = match tokio::runtime::Handle::try_current() {
        Ok(h) => h,
        Err(_) => {
            app.provider_setup = Some(ProviderSetupStep::GitLabSetup {
                url_input: instance_url,
                url_cursor: 0,
                token_input: token,
                token_cursor: 0,
                active_field: 0,
                error: Some("No async runtime available.".to_string()),
            });
            return;
        }
    };

    handle.spawn(async move {
        match codemason_agent::gitlab::auth::validate_token(&instance_url, &token).await {
            Ok(username) => {
                // Save token (encrypted) and config to database
                let cfg = codemason_agent::gitlab::auth::GitLabConfig {
                    instance_url: instance_url.clone(),
                    username: username.clone(),
                };
                let mut errors = Vec::new();
                if let Err(e) = codemason_agent::gitlab::auth::save_token(storage.as_ref(), &token)
                {
                    errors.push(format!("token save: {e}"));
                }
                if let Err(e) = codemason_agent::gitlab::auth::save_config(storage.as_ref(), &cfg) {
                    errors.push(format!("config save: {e}"));
                }
                if errors.is_empty() {
                    event_bus.publish(codemason_agent::event::Event::AgentError {
                        session_id: sid,
                        error: format!(
                            "[ok] GitLab configured successfully!\n\n\
                             **Instance**: {instance_url}\n\
                             **Username**: {username}\n\
                             **Token**: saved (encrypted)"
                        ),
                    });
                } else {
                    event_bus.publish(codemason_agent::event::Event::AgentError {
                        session_id: sid,
                        error: format!(
                            "[warn] GitLab authenticated as {username} but failed to save: {}",
                            errors.join(", ")
                        ),
                    });
                }
                // Signal the TUI to close the dialog
                event_bus.publish(codemason_agent::event::Event::GitLabSetupComplete {
                    success: errors.is_empty(),
                    error: if errors.is_empty() {
                        None
                    } else {
                        Some(errors.join(", "))
                    },
                });
            }
            Err(e) => {
                event_bus.publish(codemason_agent::event::Event::GitLabSetupComplete {
                    success: false,
                    error: Some(format!("{e}")),
                });
            }
        }
    });
}

/// Handle key events when the MCP discover dialog is active.
///
/// `Up`/`Down` (or `k`/`j`) move the block cursor, `Space` toggles the
/// highlighted server's enabled state, and `Esc` dismisses the dialog. The list
/// scroll is re-clamped to keep the cursor visible at the next render.
async fn handle_mcp_discover_key(app: &mut App, key: KeyInput) {
    match key.key {
        // Dismiss on Escape
        Key::Esc => {
            app.mcp_discover = None;
        }

        // Move the block cursor up.
        Key::Up | Key::Char('k') => {
            if let Some(state) = app.mcp_discover.as_mut() {
                state.selected = state.selected.saturating_sub(1);
            }
        }

        // Move the block cursor down.
        Key::Down | Key::Char('j') => {
            if let Some(state) = app.mcp_discover.as_mut() {
                let last = state.servers.len().saturating_sub(1);
                if state.selected < last {
                    state.selected += 1;
                }
            }
        }

        Key::Home => {
            if let Some(state) = app.mcp_discover.as_mut() {
                state.selected = 0;
            }
        }

        Key::End => {
            if let Some(state) = app.mcp_discover.as_mut() {
                state.selected = state.servers.len().saturating_sub(1);
            }
        }

        // Toggle the highlighted server's enabled state.
        Key::Char(' ') => {
            let Some(srv) = app
                .mcp_discover
                .as_ref()
                .and_then(|s| s.servers.get(s.selected).cloned())
            else {
                return;
            };
            let row_enabled = app
                .mcp_discover
                .as_ref()
                .and_then(|s| s.enabled.get(s.selected).copied())
                .unwrap_or(false);
            let (feedback, enabled) = app.toggle_discovered_mcp_server(&srv, row_enabled).await;
            if let Some(state) = app.mcp_discover.as_mut() {
                state.feedback = Some(feedback);
                if let Some(row) = state.enabled.get_mut(state.selected) {
                    *row = enabled;
                }
            }
        }

        _ => {}
    }
}

/// Handle key events when the `/plugins list` dialog is active (spec `plugins`
/// FR-009).
///
/// `Up`/`Down` (or `k`/`j`) move the block cursor, `Home`/`End` jump to the first
/// or last row, `Space` toggles the highlighted plugin's enabled state (mirroring
/// the `/mcp discover` dialog), and `Esc` dismisses the dialog. The list scroll is
/// re-clamped to keep the cursor visible at the next render. `Space` on a
/// non-plugin body line (a capability block or the notice row) is a no-op: only
/// the leading table rows carry a plugin id.
async fn handle_plugins_list_key(app: &mut App, key: KeyInput) {
    match key.key {
        // Dismiss on Escape
        Key::Esc => {
            app.plugins_list = None;
        }

        // Move the block cursor up.
        Key::Up | Key::Char('k') => {
            if let Some(state) = app.plugins_list.as_mut() {
                state.selected = state.selected.saturating_sub(1);
            }
        }

        // Move the block cursor down.
        Key::Down | Key::Char('j') => {
            if let Some(state) = app.plugins_list.as_mut() {
                let last = state.view.body.len().saturating_sub(1);
                if state.selected < last {
                    state.selected += 1;
                }
            }
        }

        Key::Home => {
            if let Some(state) = app.plugins_list.as_mut() {
                state.selected = 0;
            }
        }

        Key::End => {
            if let Some(state) = app.plugins_list.as_mut() {
                state.selected = state.view.body.len().saturating_sub(1);
            }
        }

        // Toggle the highlighted plugin's enabled state. The plugin table rows are
        // the first `view.rows.len()` body lines, so a cursor on a trailing
        // capability block has no row to toggle and is ignored.
        Key::Char(' ') => {
            let Some((plugin_id, currently_enabled)) = app.plugins_list.as_ref().and_then(|s| {
                s.view
                    .rows
                    .get(s.selected)
                    .map(|row| (row.id.clone(), row.enabled))
            }) else {
                return;
            };
            let feedback = crate::app::toggle_plugin_enabled(app, &plugin_id, currently_enabled);
            // `toggle_plugin_enabled` rebuilds the panel; set the feedback line on
            // the fresh state so the next render shows the outcome.
            if let Some(state) = app.plugins_list.as_mut() {
                state.feedback = Some(feedback);
            }
        }

        _ => {}
    }
}

/// Handle key input when the right-click context menu is open.
///
/// Up/Down navigate items; Enter activates the highlighted item; Esc closes
/// without acting; any other key is ignored.
fn handle_context_menu_key(app: &mut App, key: KeyInput) {
    match key.key {
        Key::Esc => {
            app.context_menu = None;
        }

        Key::Up => {
            if let Some(ref mut menu) = app.context_menu {
                // Skip disabled items going upward.
                let count = menu.items.len();
                let mut idx = menu.selected;
                for _ in 0..count {
                    idx = (idx + count - 1) % count;
                    if menu.items[idx].1 {
                        menu.selected = idx;
                        break;
                    }
                }
            }
        }

        Key::Down => {
            if let Some(ref mut menu) = app.context_menu {
                let count = menu.items.len();
                let mut idx = menu.selected;
                for _ in 0..count {
                    idx = (idx + 1) % count;
                    if menu.items[idx].1 {
                        menu.selected = idx;
                        break;
                    }
                }
            }
        }

        Key::Enter => {
            if let Some(menu) = app.context_menu.clone() {
                let (action, enabled): (ContextAction, bool) = menu.items[menu.selected];
                if enabled {
                    app.execute_context_action(action);
                } else {
                    app.context_menu = None;
                }
            }
        }

        _ => {}
    }
}

/// Handle keyboard input inside the router cluster setup panel.
/// Navigation is entirely keyboard-driven: Tab switches between the provider
/// list (left) and the tier bucket columns (right). Space toggles a provider in
/// or out of the cluster palette. Enter opens the model picker for the selected
/// provider and assigns the chosen model to the active bucket. Ctrl+S saves the
/// cluster to `codemason.json`, preserving existing classifier weights/boundaries.
fn handle_router_setup_key(app: &mut App, key: KeyInput) {
    let Some(ProviderSetupStep::SetupRouter {
        providers,
        mut selected_provider_ids,
        mut selected_provider_index,
        mut draft_config,
        mut active_bucket,
        mut active_bucket_index,
        mut left_pane_focused,
        error: _,
    }) = app.provider_setup.take()
    else {
        return;
    };

    let mut error: Option<String> = None;

    match key.key {
        Key::Esc => {
            app.provider_setup = None;
            return;
        }
        Key::Tab => {
            left_pane_focused = !left_pane_focused;
            active_bucket_index = 0;
        }
        Key::Left if !left_pane_focused => {
            let tiers = Tier::all();
            let idx = tiers.iter().position(|t| *t == active_bucket).unwrap_or(0);
            active_bucket = *tiers
                .get((idx + tiers.len() - 1) % tiers.len())
                .unwrap_or(&Tier::Simple);
            active_bucket_index = 0;
        }
        Key::Right if !left_pane_focused => {
            let idx = Tier::all()
                .iter()
                .position(|t| *t == active_bucket)
                .unwrap_or(0);
            active_bucket = *Tier::all().get(idx + 1).unwrap_or(&Tier::Simple);
            active_bucket_index = 0;
        }
        Key::Up if key.mods.contains(Modifiers::CTRL) && !left_pane_focused => {
            if let Some(tier_config) = draft_config.tiers.get_mut(&active_bucket.to_string()) {
                if active_bucket_index > 0 && active_bucket_index < tier_config.models.len() {
                    tier_config
                        .models
                        .swap(active_bucket_index, active_bucket_index - 1);
                    active_bucket_index -= 1;
                }
            }
        }
        Key::Down if key.mods.contains(Modifiers::CTRL) && !left_pane_focused => {
            if let Some(tier_config) = draft_config.tiers.get_mut(&active_bucket.to_string()) {
                if active_bucket_index + 1 < tier_config.models.len() {
                    tier_config
                        .models
                        .swap(active_bucket_index, active_bucket_index + 1);
                    active_bucket_index += 1;
                }
            }
        }
        Key::Up => {
            if left_pane_focused {
                if providers.is_empty() {
                    selected_provider_index = 0;
                } else {
                    selected_provider_index = if selected_provider_index == 0 {
                        providers.len() - 1
                    } else {
                        selected_provider_index - 1
                    };
                }
            } else {
                let models = draft_config
                    .tiers
                    .get(&active_bucket.to_string())
                    .map(|t| t.models.len())
                    .unwrap_or(0);
                if models == 0 {
                    active_bucket_index = 0;
                } else {
                    active_bucket_index = if active_bucket_index == 0 {
                        models - 1
                    } else {
                        active_bucket_index - 1
                    };
                }
            }
        }
        Key::Down => {
            if left_pane_focused {
                if providers.is_empty() {
                    selected_provider_index = 0;
                } else {
                    selected_provider_index = (selected_provider_index + 1) % providers.len();
                }
            } else {
                let models = draft_config
                    .tiers
                    .get(&active_bucket.to_string())
                    .map(|t| t.models.len())
                    .unwrap_or(0);
                if models == 0 {
                    active_bucket_index = 0;
                } else {
                    active_bucket_index = (active_bucket_index + 1) % models;
                }
            }
        }
        Key::Char(' ') if left_pane_focused => {
            if let Some(provider) = providers.get(selected_provider_index) {
                let id = provider.id.clone();
                if selected_provider_ids.contains(&id) {
                    selected_provider_ids.retain(|x| x != &id);
                    // Remove any existing assignments for this provider from
                    // every tier so the cluster stays consistent.
                    for tier_config in draft_config.tiers.values_mut() {
                        tier_config.models.retain(|entry| entry.provider_id != id);
                    }
                } else {
                    selected_provider_ids.push(id);
                }
            }
        }
        Key::Enter => {
            let provider = if left_pane_focused {
                providers.get(selected_provider_index).cloned()
            } else {
                providers
                    .get(selected_provider_index)
                    .filter(|p| selected_provider_ids.contains(&p.id))
                    .cloned()
            };
            if let Some(provider) = provider {
                if !selected_provider_ids.contains(&provider.id) {
                    error = Some("Select provider with Space first".to_string());
                } else {
                    let models = app.models_for_provider(&provider.id);
                    if models.is_empty() {
                        error = Some(format!("No models available for {}", provider.name));
                    } else {
                        app.router_draft_providers = providers.clone();
                        app.router_draft_selected_ids = selected_provider_ids.clone();
                        app.router_draft_config = Some(draft_config.clone());
                        app.provider_setup = Some(ProviderSetupStep::SelectRouterModel {
                            provider_id: provider.id,
                            provider_name: provider.name,
                            models,
                            selected: 0,
                            target_tier: active_bucket,
                        });
                        return;
                    }
                }
            }
        }
        Key::Char('s') if key.mods.contains(Modifiers::CTRL) => {
            let has_any = draft_config.tiers.values().any(|tc| !tc.models.is_empty());
            if !has_any {
                error = Some("At least one tier must contain a model".to_string());
            } else {
                app.pending_router_save = Some(draft_config.clone());
                app.provider_setup = Some(ProviderSetupStep::SetupRouter {
                    providers,
                    selected_provider_ids,
                    selected_provider_index,
                    draft_config,
                    active_bucket,
                    active_bucket_index,
                    left_pane_focused,
                    error,
                });
                return;
            }
        }
        Key::Delete if !left_pane_focused => {
            if let Some(tier_config) = draft_config.tiers.get_mut(&active_bucket.to_string()) {
                if active_bucket_index < tier_config.models.len() {
                    tier_config.models.remove(active_bucket_index);
                    if active_bucket_index >= tier_config.models.len() && active_bucket_index > 0 {
                        active_bucket_index -= 1;
                    }
                }
            }
        }
        _ => {}
    }

    app.provider_setup = Some(ProviderSetupStep::SetupRouter {
        providers,
        selected_provider_ids,
        selected_provider_index,
        draft_config,
        active_bucket,
        active_bucket_index,
        left_pane_focused,
        error,
    });
}

/// Handle keyboard input inside the router model picker sub-dialog.
/// Esc cancels and returns to the router setup panel without assigning a model.
/// Up/Down navigate the model list. Enter assigns the selected model to the
/// target tier and returns to the bucket pane.
fn handle_router_model_picker_key(app: &mut App, key: KeyInput) {
    let Some(ProviderSetupStep::SelectRouterModel {
        provider_id,
        provider_name,
        models,
        selected,
        target_tier,
    }) = app.provider_setup.take()
    else {
        return;
    };

    match key.key {
        Key::Esc => {
            // Restore the stashed router setup state, returning focus to the
            // provider palette.
            let draft_config = app.router_draft_config.take().unwrap_or_default();
            app.provider_setup = Some(ProviderSetupStep::SetupRouter {
                providers: app.router_draft_providers.clone(),
                selected_provider_ids: app.router_draft_selected_ids.clone(),
                selected_provider_index: app
                    .router_draft_providers
                    .iter()
                    .position(|p| p.id == provider_id)
                    .unwrap_or(0),
                draft_config,
                active_bucket: target_tier,
                active_bucket_index: 0,
                left_pane_focused: true,
                error: None,
            });
            app.router_draft_providers.clear();
            app.router_draft_selected_ids.clear();
        }
        Key::Up => {
            let new = if models.is_empty() {
                0
            } else if selected == 0 {
                models.len() - 1
            } else {
                selected - 1
            };
            app.provider_setup = Some(ProviderSetupStep::SelectRouterModel {
                provider_id,
                provider_name,
                models,
                selected: new,
                target_tier,
            });
        }
        Key::Down => {
            let new = if models.is_empty() {
                0
            } else {
                (selected + 1) % models.len()
            };
            app.provider_setup = Some(ProviderSetupStep::SelectRouterModel {
                provider_id,
                provider_name,
                models,
                selected: new,
                target_tier,
            });
        }
        Key::Enter => {
            if let Some(model) = models.get(selected).cloned() {
                let mut draft_config = app.router_draft_config.take().unwrap_or_default();
                let tier_key = target_tier.to_string();
                let tier_config = draft_config.tiers.entry(tier_key).or_default();
                // Prevent recursive routing: the router must never route to itself.
                if provider_id == "router" {
                    app.provider_setup = Some(ProviderSetupStep::SetupRouter {
                        providers: app.router_draft_providers.clone(),
                        selected_provider_ids: app.router_draft_selected_ids.clone(),
                        selected_provider_index: app
                            .router_draft_providers
                            .iter()
                            .position(|p| p.id == provider_id)
                            .unwrap_or(0),
                        draft_config,
                        active_bucket: target_tier,
                        active_bucket_index: 0,
                        left_pane_focused: true,
                        error: Some("The router cannot route to itself".to_string()),
                    });
                    app.router_draft_providers.clear();
                    app.router_draft_selected_ids.clear();
                    return;
                }

                // Avoid duplicate exact provider/model pairs in the same tier.
                let already_present = tier_config
                    .models
                    .iter()
                    .any(|entry| entry.provider_id == provider_id && entry.model == model.id);
                if !already_present {
                    tier_config
                        .models
                        .push(codemason_llm::providers::router_config::TierEntry {
                            provider_id: provider_id.clone(),
                            model: model.id.clone(),
                        });
                }
                let new_bucket_index = tier_config.models.len().saturating_sub(1);
                app.provider_setup = Some(ProviderSetupStep::SetupRouter {
                    providers: app.router_draft_providers.clone(),
                    selected_provider_ids: app.router_draft_selected_ids.clone(),
                    selected_provider_index: app
                        .router_draft_providers
                        .iter()
                        .position(|p| p.id == provider_id)
                        .unwrap_or(0),
                    draft_config,
                    active_bucket: target_tier,
                    active_bucket_index: new_bucket_index,
                    left_pane_focused: false,
                    error: None,
                });
                app.router_draft_providers.clear();
                app.router_draft_selected_ids.clear();
            } else {
                app.provider_setup = Some(ProviderSetupStep::SelectRouterModel {
                    provider_id,
                    provider_name,
                    models,
                    selected,
                    target_tier,
                });
            }
        }
        _ => {
            app.provider_setup = Some(ProviderSetupStep::SelectRouterModel {
                provider_id,
                provider_name,
                models,
                selected,
                target_tier,
            });
        }
    }
}

/// Swap the OTLP default port in `endpoint` from `from_port` to `to_port`
/// (each written as `":4318"`), used when the telemetry setup dialog toggles
/// the transport protocol.
///
/// Returns `None` unless the URL's authority ends in exactly `from_port`, so a
/// custom port, a host without a port, or a path, query or fragment that
/// happens to contain the digits is left untouched.
fn swap_otlp_default_port(endpoint: &str, from_port: &str, to_port: &str) -> Option<String> {
    let after_scheme = endpoint.find("://").map_or(0, |i| i + 3);
    // The authority ends at the first path, query or fragment delimiter, so
    // a `:4318` inside a query value is never mistaken for the port.
    let authority_end = endpoint[after_scheme..]
        .find(['/', '?', '#'])
        .map_or(endpoint.len(), |i| after_scheme + i);
    let authority = &endpoint[after_scheme..authority_end];
    let host = authority.strip_suffix(from_port)?;
    Some(format!(
        "{}{host}{to_port}{}",
        &endpoint[..after_scheme],
        &endpoint[authority_end..]
    ))
}
