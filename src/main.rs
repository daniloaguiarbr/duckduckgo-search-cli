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

#[tokio::main(flavor = "multi_thread")]
async fn main() -> ExitCode {
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
