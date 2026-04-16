//! Ponto de entrada do binário `duckduckgo-search-cli`.
//!
//! Esta função faz APENAS o mínimo:
//! 1. Restaura SIGPIPE via [`duckduckgo_search_cli::signals`].
//! 2. Cria o `CancellationToken` e instala handler de SIGINT.
//! 3. Delega para [`duckduckgo_search_cli::run()`] em `lib.rs`.
//! 4. Propaga o exit code retornado para o sistema operacional.
//!
//! TODA lógica de negócio vive em `lib.rs` e seus submódulos.

use std::process::ExitCode;
use tokio_util::sync::CancellationToken;

#[tokio::main(flavor = "multi_thread")]
async fn main() -> ExitCode {
    duckduckgo_search_cli::signals::restaurar_sigpipe();

    let cancelamento = CancellationToken::new();
    duckduckgo_search_cli::signals::instalar_handler_cancelamento(cancelamento.clone());

    let codigo = duckduckgo_search_cli::run(cancelamento).await;
    ExitCode::from(codigo as u8)
}
