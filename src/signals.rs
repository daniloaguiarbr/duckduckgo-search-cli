// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload: declarative (SIGPIPE/SIGINT handler installation)
//! Cross-platform signal handlers for the CLI binary.
//!
//! Centralizes handling of SIGPIPE (Unix) and SIGINT/Ctrl+C (cross-platform)
//! in a single module, per RULES SUPREMAS v3.0 Section 19.
//!
//! - [`restore_sigpipe`]: restores `SIG_DFL` for SIGPIPE on Unix, avoiding
//!   silent errors in pipes (`| jaq`, `| head`).
//! - [`install_cancellation_handler`]: spawns a task that awaits Ctrl+C and
//!   signals cancellation via `CancellationToken`.

use tokio_util::sync::CancellationToken;

/// Restores the default behavior of SIGPIPE (`SIG_DFL`) on Unix platforms.
///
/// The Rust runtime ignores SIGPIPE by default (`SIG_IGN`), which causes
/// writes to closed pipes to return EPIPE instead of terminating the process.
/// For CLIs that emit to stdout and are consumed via pipes (`| jaq`, `| head`),
/// this causes silent errors or empty output.
///
/// Restoring `SIG_DFL` makes the process terminate cleanly and silently when the
/// pipe reader closes — the expected behavior for Unix tools.
#[cfg(unix)]
pub fn restore_sigpipe() {
    // POSIX: SIGPIPE = 13 em todas as plataformas Unix (Linux, macOS, *BSD).
    // SIG_DFL = 0 em todas as plataformas POSIX.
    extern "C" {
        fn signal(sig: i32, handler: usize) -> usize;
    }
    // SAFETY: signal() with SIG_DFL is safe here — called before any thread
    // is spawned, at the top of main(). SIGPIPE (13) and SIG_DFL (0) are
    // stable POSIX constants on all Unix platforms.
    unsafe {
        signal(13, 0);
    }
}

/// No-op on Windows — SIGPIPE does not exist.
#[cfg(not(unix))]
pub fn restore_sigpipe() {}

/// Spawns an async task that awaits SIGINT (Ctrl+C) and cancels the token.
///
/// `tokio::signal::ctrl_c()` is cross-platform:
/// - Unix: captures SIGINT
/// - Windows: captures `CTRL_C_EVENT` via console API
///
/// When the signal is received, the `CancellationToken` is cancelled, propagating
/// cancellation to all tasks observing it.
pub fn install_cancellation_handler(cancellation: CancellationToken) {
    // JoinHandle intentionally not stored (fire-and-forget pattern).
    // This signal handler runs for the entire process lifetime. Storing
    // the handle would require threading it through the entire call stack
    // with no benefit — the task self-terminates after cancellation.
    tokio::spawn(async move {
        if let Err(erro) = tokio::signal::ctrl_c().await {
            tracing::warn!(?erro, "failed to install ctrl+c handler");
            return;
        }
        tracing::warn!("SIGINT/Ctrl+C received — cancelling in-flight tasks");
        cancellation.cancel();
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn restore_sigpipe_does_not_panic() {
        // Ensures calling restore_sigpipe() does not panic.
        // On Unix, restores SIG_DFL. On Windows, is a no-op.
        restore_sigpipe();
    }

    #[tokio::test]
    async fn install_handler_does_not_panic() {
        let token = CancellationToken::new();
        // Only verifies that installing the handler does not panic.
        // We do not send SIGINT in the test — we only validate the installation.
        install_cancellation_handler(token);
    }
}
