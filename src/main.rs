//! Ponto de entrada do binário `duckduckgo-search-cli`.
//!
//! Esta função faz APENAS o mínimo:
//! 1. Constrói o runtime Tokio multi-threaded (via `#[tokio::main]`).
//! 2. Cria o `CancellationToken` e spawna o handler de SIGINT (ctrl+c).
//! 3. Delega para `duckduckgo_search_cli::run()` em `lib.rs`.
//! 4. Propaga o exit code retornado para o sistema operacional.
//!
//! TODA lógica de negócio vive em `lib.rs` e seus submódulos.

use std::process::ExitCode;
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
fn restaurar_sigpipe() {
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

#[cfg(not(unix))]
fn restaurar_sigpipe() {
    // No-op em Windows — SIGPIPE não existe.
}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> ExitCode {
    restaurar_sigpipe();
    let cancelamento = CancellationToken::new();
    let cancelamento_handler = cancelamento.clone();

    // Spawn do handler de SIGINT. Aguarda ctrl+c e sinaliza o token.
    // tokio::signal::ctrl_c() é cross-platform (Unix → SIGINT, Windows → console ctrl+c).
    tokio::spawn(async move {
        if let Err(erro) = tokio::signal::ctrl_c().await {
            tracing::warn!(?erro, "falha ao instalar handler de ctrl+c");
            return;
        }
        tracing::warn!("SIGINT/Ctrl+C recebido — cancelando tasks em voo");
        cancelamento_handler.cancel();
    });

    let codigo = duckduckgo_search_cli::run(cancelamento).await;
    // i32 da lib converte para ExitCode do std.
    ExitCode::from(codigo as u8)
}
