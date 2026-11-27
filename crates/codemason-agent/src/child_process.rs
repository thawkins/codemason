//! Run a child process to completion under a deadline (audit AG-18).
//!
//! Hooks, verification scripts and hardware probes each spawn a command, feed
//! it stdin, capture its output and must not outlive a timeout. Doing that
//! safely needs four things together, and each call site was missing one:
//!
//! - stdin is written on its own thread, concurrently with draining, so a
//!   child that prints a lot without reading its input cannot deadlock (AG-04);
//! - stdout and stderr are drained while waiting, so a chatty child never
//!   blocks on a full pipe and gets killed as "hung" (AG-07);
//! - at the deadline the whole process tree is killed (its own process group
//!   on Unix), not just the direct child or the awaiting task (AG-02, AG-06);
//! - a pipe held open by a surviving grandchild never blocks the caller past
//!   the deadline.
//!
//! Dependencies: the approved process-group kill in `team::manager` (Unix).

use std::io::{Read, Write};
use std::process::{Command, ExitStatus, Output, Stdio};
use std::sync::{Arc, Mutex, PoisonError, mpsc};
use std::time::{Duration, Instant};

/// How often the child is polled for exit.
const POLL_INTERVAL: Duration = Duration::from_millis(10);

/// How long the pipes may keep draining once the deadline has passed and the
/// tree was killed, so the output written so far is kept.
const DRAIN_GRACE: Duration = Duration::from_millis(500);

/// What a deadline-bounded child produced.
#[derive(Debug)]
pub struct DeadlineOutput {
    /// The exit status, or `None` when the deadline passed and the process
    /// tree was killed.
    pub status: Option<ExitStatus>,
    /// Everything read from stdout (partial when timed out).
    pub stdout: Vec<u8>,
    /// Everything read from stderr (partial when timed out).
    pub stderr: Vec<u8>,
}

impl DeadlineOutput {
    /// As a [`std::process::Output`], or an [`std::io::ErrorKind::TimedOut`]
    /// error when the deadline passed.
    pub fn into_output(self, timeout: Duration) -> std::io::Result<Output> {
        match self.status {
            Some(status) => Ok(Output {
                status,
                stdout: self.stdout,
                stderr: self.stderr,
            }),
            None => Err(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                format!("timed out after {}s and was killed", timeout.as_secs()),
            )),
        }
    }
}

/// Spawn `cmd`, write `stdin` to it, and wait at most `timeout` for it to
/// exit, capturing its output.
///
/// `cmd`'s stdio is replaced with pipes (stdin is null when `stdin` is
/// `None`). On Unix the child leads its own process group so the kill at the
/// deadline reaches everything it started.
///
/// # Errors
///
/// Returns the spawn error, or a wait error (after killing the tree).
pub fn run_with_deadline(
    cmd: &mut Command,
    stdin: Option<Vec<u8>>,
    timeout: Duration,
) -> std::io::Result<DeadlineOutput> {
    cmd.stdin(if stdin.is_some() {
        Stdio::piped()
    } else {
        Stdio::null()
    })
    .stdout(Stdio::piped())
    .stderr(Stdio::piped());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }
    let mut child = cmd.spawn()?;

    if let (Some(bytes), Some(mut pipe)) = (stdin, child.stdin.take()) {
        std::thread::spawn(move || {
            // INTENTIONAL: the child may exit without reading its stdin
            let _ = pipe.write_all(&bytes);
        });
    }
    let (done_tx, done_rx) = mpsc::channel::<()>();
    let stdout = drain(child.stdout.take(), done_tx.clone());
    let stderr = drain(child.stderr.take(), done_tx);

    let deadline = Instant::now() + timeout;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Some(status),
            Ok(None) if Instant::now() >= deadline => {
                kill_tree(&mut child);
                break None;
            }
            Ok(None) => std::thread::sleep(POLL_INTERVAL),
            Err(e) => {
                kill_tree(&mut child);
                return Err(e);
            }
        }
    };

    // The readers finish when every holder of the pipes has exited. A
    // grandchild that outlives the child (Windows has no process-group kill;
    // a Unix hook may detach one) must not hold the caller past the deadline.
    let drain_until = deadline.max(Instant::now() + DRAIN_GRACE);
    for _ in 0..2 {
        let left = drain_until.saturating_duration_since(Instant::now());
        if done_rx.recv_timeout(left).is_err() {
            break;
        }
    }
    Ok(DeadlineOutput {
        status,
        stdout: take(&stdout),
        stderr: take(&stderr),
    })
}

/// Read `pipe` to its end on a thread, appending to the returned buffer as
/// bytes arrive (so a timed-out child's partial output is kept), and signal
/// `done` at the end.
fn drain<R: Read + Send + 'static>(pipe: Option<R>, done: mpsc::Sender<()>) -> Arc<Mutex<Vec<u8>>> {
    let buffer = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&buffer);
    std::thread::spawn(move || {
        if let Some(mut pipe) = pipe {
            let mut chunk = [0u8; 8192];
            while let Ok(n) = pipe.read(&mut chunk) {
                if n == 0 {
                    break;
                }
                sink.lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .extend_from_slice(&chunk[..n]);
            }
        }
        // INTENTIONAL: the receiver stops listening once the deadline passed
        let _ = done.send(());
    });
    buffer
}

/// The bytes drained so far.
fn take(buffer: &Mutex<Vec<u8>>) -> Vec<u8> {
    std::mem::take(&mut *buffer.lock().unwrap_or_else(PoisonError::into_inner))
}

/// Kill the child and everything it started, then reap it.
///
/// Unix signals the child's process group. Windows has no process groups to
/// signal, and Git for Windows' `bin\bash.exe` is itself a launcher for the
/// real shell, so killing the direct child alone leaves the script running;
/// `taskkill /T` ends the whole tree.
fn kill_tree(child: &mut std::process::Child) {
    #[cfg(unix)]
    crate::team::manager::kill_hook_process_group(child.id());
    #[cfg(windows)]
    {
        // INTENTIONAL: best effort; the direct kill below still runs
        let _ = Command::new("taskkill")
            .args(["/T", "/F", "/PID", &child.id().to_string()])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
    // INTENTIONAL: the child may already have exited
    let _ = child.kill();
    // INTENTIONAL: reap so the killed child does not linger as a zombie
    let _ = child.wait();
}
