// SPDX-License-Identifier: MIT OR Apache-2.0
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

// System allocator used intentionally: CLI is short-lived with moderate
// allocations. jemalloc/mimalloc not justified for single-shot workload.
//
// worker_threads and max_blocking_threads use Tokio defaults.
// spawn_blocking concurrency is bounded indirectly by the global semaphore
// in content_fetch.rs, so the default blocking pool (512 threads) is safe.
#[tokio::main(flavor = "multi_thread")]
async fn main() -> ExitCode {
    duckduckgo_search_cli::signals::restore_sigpipe();

    #[cfg(all(feature = "console", tokio_unstable))]
    console_subscriber::init();

    let cancellation = CancellationToken::new();
    duckduckgo_search_cli::signals::install_cancellation_handler(cancellation.clone());

    let exit_code = duckduckgo_search_cli::run(cancellation).await;
    ExitCode::from(exit_code as u8)
}
