//! Handlers de sinais cross-platform para o binário CLI.
//!
//! Centraliza o tratamento de SIGPIPE (Unix) e SIGINT/Ctrl+C (cross-platform)
//! em um único módulo, conforme RULES SUPREMAS v3.0 Seção 19.
//!
//! - [`restaurar_sigpipe`]: restaura SIG_DFL para SIGPIPE no Unix, evitando
//!   erros silenciosos em pipes (`| jaq`, `| head`).
//! - [`instalar_handler_cancelamento`]: spawna task que aguarda Ctrl+C e
//!   sinaliza cancelamento via `CancellationToken`.

use tokio_util::sync::CancellationToken;

/// Restaura o comportamento padrão de SIGPIPE (SIG_DFL) em plataformas Unix.
///
/// O runtime do Rust ignora SIGPIPE por padrão (SIG_IGN), o que faz com que
/// writes em pipes fechados retornem EPIPE ao invés de terminar o processo.
/// Para CLIs que emitem em stdout e são consumidas via pipes (`| jaq`, `| head`),
/// isso causa erros silenciosos ou output vazio.
///
/// Restaurar SIG_DFL faz o processo terminar limpa e silenciosamente quando o
/// leitor do pipe fecha — comportamento esperado em ferramentas Unix.
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

/// No-op em Windows — SIGPIPE não existe.
#[cfg(not(unix))]
pub fn restaurar_sigpipe() {}

/// Spawna uma task assíncrona que aguarda SIGINT (Ctrl+C) e cancela o token.
///
/// `tokio::signal::ctrl_c()` é cross-platform:
/// - Unix: captura SIGINT
/// - Windows: captura CTRL_C_EVENT via console API
///
/// Quando o sinal é recebido, o `CancellationToken` é cancelado, propagando
/// o cancelamento para todas as tasks que o observam.
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
