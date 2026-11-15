//! Tests for the handoff that carries a built GUI session from the runtime
//! thread to the process main thread. The eframe window itself needs a
//! display, so the host runs a stand-in runner here.

#[path = "support/mod.rs"]
mod support;

use codemason_agent::event::EventBus;
use codemason_gui::{GuiLaunch, GuiLaunchError, Shell, main_thread_channel};

/// Build a launch over a fresh app, returning it with the log sender so the
/// test can prove the receiver crossed the handoff.
fn make_launch() -> (GuiLaunch, codemason_app_core::log_channel::TuiLogSender) {
    let (log_tx, log_rx) = codemason_app_core::log_channel::tui_log_channel(4);
    let launch = GuiLaunch {
        app: support::make_app(),
        events: EventBus::default().subscribe(),
        logs: log_rx,
        runtime: tokio::runtime::Handle::current(),
    };
    (launch, log_tx)
}

#[tokio::test(flavor = "multi_thread")]
async fn test_main_thread_launch_delivers_session_to_host() {
    let (launcher, host) = main_thread_channel();
    let host_thread = std::thread::spawn(move || {
        let mut delivered = Vec::new();
        host.serve_with(|launch| {
            let message = launch
                .logs
                .try_recv()
                .map(|record| record.message)
                .unwrap_or_default();
            // The delivered parts are exactly what the window is built from.
            let _shell = Shell::new(launch.app).with_sources(
                tokio::sync::mpsc::unbounded_channel().1,
                launch.logs,
                launch.runtime,
            );
            delivered.push(message);
            Ok(())
        });
        delivered
    });

    let (launch, log_tx) = make_launch();
    log_tx
        .try_send(codemason_app_core::log_channel::TuiLogRecord {
            level: tracing::Level::INFO,
            message: "handoff".to_string(),
        })
        .expect("send log record");
    let result = launcher.launch(launch).await;
    assert!(result.is_ok(), "launch result: {result:?}");

    // Dropping the last launcher must end `serve`, so the join cannot hang.
    drop(launcher);
    let delivered = host_thread.join().expect("host thread");
    assert_eq!(delivered, vec!["handoff".to_string()]);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_main_thread_launch_returns_window_error() {
    let (launcher, host) = main_thread_channel();
    let host_thread = std::thread::spawn(move || {
        host.serve_with(|_launch| Err(GuiLaunchError::Backend("no adapter".into())));
    });

    let (launch, _log_tx) = make_launch();
    let result = launcher.launch(launch).await;
    assert!(
        matches!(&result, Err(GuiLaunchError::Backend(m)) if m == "no adapter"),
        "got {result:?}"
    );
    drop(launcher);
    host_thread.join().expect("host thread");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_main_thread_launch_without_host_is_backend_error() {
    let (launcher, host) = main_thread_channel();
    drop(host);

    let (launch, _log_tx) = make_launch();
    let result = launcher.launch(launch).await;
    assert!(
        matches!(result, Err(GuiLaunchError::Backend(_))),
        "got {result:?}"
    );
}
