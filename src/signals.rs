//! Cross-platform signal handlers for the CLI binary.
//!
//! Centralizes handling of SIGPIPE (Unix) and SIGINT/Ctrl+C (cross-platform)
//! in a single module, per RULES SUPREMAS v3.0 Section 19.
//!
//! - [`restaurar_sigpipe`]: restores SIG_DFL for SIGPIPE on Unix, avoiding
//!   silent errors in pipes (`| jaq`, `| head`).
//! - [`instalar_handler_cancelamento`]: spawns a task that awaits Ctrl+C and
//!   signals cancellation via `CancellationToken`.

use tokio_util::sync::CancellationToken;

/// Restores the default behavior of SIGPIPE (SIG_DFL) on Unix platforms.
///
/// The Rust runtime ignores SIGPIPE by default (SIG_IGN), which causes
/// writes to closed pipes to return EPIPE instead of terminating the process.
/// For CLIs that emit to stdout and are consumed via pipes (`| jaq`, `| head`),
/// this causes silent errors or empty output.
///
/// Restoring SIG_DFL makes the process terminate cleanly and silently when the
/// pipe reader closes — the expected behavior for Unix tools.
#[cfg(unix)]
pub fn restaurar_sigpipe() {
    // POSIX: SIGPIPE = 13 em todas as plataformas Unix (Linux, macOS, *BSD).
    // SIG_DFL = 0 em todas as plataformas POSIX.
    extern "C" {
        fn signal(sig: i32, handler: usize) -> usize;
    }
    // SAFETY: signal() com SIG_DFL é seguro neste contexto — chamado antes de
    // qualquer thread ser criada, no topo de main(). SIGPIPE (13) e SIG_DFL (0)
    // são constantes POSIX estáveis em todas as plataformas Unix.
    unsafe {
        signal(13, 0);
    }
}

/// No-op on Windows — SIGPIPE does not exist.
#[cfg(not(unix))]
pub fn restaurar_sigpipe() {}

/// Spawns an async task that awaits SIGINT (Ctrl+C) and cancels the token.
///
/// `tokio::signal::ctrl_c()` is cross-platform:
/// - Unix: captures SIGINT
/// - Windows: captures CTRL_C_EVENT via console API
///
/// When the signal is received, the `CancellationToken` is cancelled, propagating
/// cancellation to all tasks observing it.
pub fn instalar_handler_cancelamento(cancelamento: CancellationToken) {
    tokio::spawn(async move {
        if let Err(erro) = tokio::signal::ctrl_c().await {
            tracing::warn!(?erro, "falha ao instalar handler de ctrl+c");
            return;
        }
        tracing::warn!("SIGINT/Ctrl+C recebido — cancelando tasks em voo");
        cancelamento.cancel();
    });
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn restaurar_sigpipe_nao_faz_panic() {
        // Garante que chamar restaurar_sigpipe() não causa panic.
        // Em Unix, restaura SIG_DFL. Em Windows, é no-op.
        restaurar_sigpipe();
    }

    #[tokio::test]
    async fn instalar_handler_nao_faz_panic() {
        let token = CancellationToken::new();
        // Apenas verifica que instalar o handler não causa panic.
        // Não enviamos SIGINT no teste — apenas validamos a instalação.
        instalar_handler_cancelamento(token);
    }
}
