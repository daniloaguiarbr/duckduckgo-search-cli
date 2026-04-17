//! Entry point for the `duckduckgo-search-cli` binary.
//!
//! This function does ONLY the minimum:
//! 1. Restores SIGPIPE via [`duckduckgo_search_cli::signals`].
//! 2. Creates the `CancellationToken` and installs the SIGINT handler.
//! 3. Delegates to [`duckduckgo_search_cli::run()`] in `lib.rs`.
//! 4. Propagates the returned exit code to the operating system.
//!
//! ALL business logic lives in `lib.rs` and its submodules.

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
