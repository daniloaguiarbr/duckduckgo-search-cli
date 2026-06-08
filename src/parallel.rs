// SPDX-License-Identifier: MIT OR Apache-2.0
//! Multi-query parallelism with `JoinSet`, `Semaphore`, staggered launch and `CancellationToken`.
//!
//! Implementation of iteration 2 per sections 4.1–4.6, 13 and 15.8 of the specification.
//!
//! Key contracts:
//! - `Semaphore` limits concurrency to the `--parallel` value (1..=20).
//! - Staggered launch adds `index * 200ms + jitter(0..300ms)` BEFORE the spawn
//!   to avoid a synchronous burst that would trigger rate-limiting.
//! - `CancellationToken` is checked between stages of each task; when SIGINT
//!   fires, in-flight tasks abort gracefully with a `cancelled` error.
//! - Failure of one task does NOT abort the entire `JoinSet`. Other tasks continue.
//!   Failed queries produce a `SearchOutput` with the `error` field filled in.
//! - Client-per-query decision (cookie jar isolation) follows section 4.3:
//!   `paginas == 1` → shared; `paginas > 1` → new Client per query.

// Workload classification: I/O-bound (HTTP scraping against DuckDuckGo).
// Bottleneck: network latency per request (~200-800ms round-trip).
// Saturated resource: outbound HTTP connections + DuckDuckGo rate limits.
// Parallelism removes the latency bottleneck; semaphore prevents connection exhaustion.
//
// Trade-offs chosen:
// - Staggered launch (200ms base + 0-300ms jitter) reduces burst throughput
//   by ~15-20% but prevents DuckDuckGo rate-limit triggers.
// - JoinSet with ordered collection: O(n) memory for all results vs streaming,
//   but preserves input order for deterministic JSON output.
// - Per-host semaphore (content_fetch.rs): limits per-domain concurrency at
//   cost of underutilizing global permits when hosts are diverse.
//
// Expected failure scenarios and their handling:
// - Rate-limit cascade: one HTTP 429 sets AtomicBool flag → all tasks add
//   random delay (500-1200ms) on next attempt. Flag uses Ordering::Relaxed
//   (best-effort, see justification in search.rs).
// - Cancel mid-flight: CancellationToken checked at 3 points — (1) before
//   staggered delay, (2) after acquire_owned, (3) inside select! on HTTP.
//   Partial results preserved; cancelled queries get error SearchOutput.
// - Consumer close (streaming): send() fails → abort_all() kills remaining
//   tasks → function returns StreamStats with consistent counters.
// - Task panic: permit recovered via RAII drop. JoinError differentiated
//   via is_panic() (logged as error) vs is_cancelled() (logged as warn).

use crate::content_fetch;
use crate::error::CliError;
use crate::http;
use crate::http::ProxyConfig;
use crate::search;
use crate::types::{Config, MultiSearchOutput, SearchMetadata, SearchOutput};
use rand::RngExt;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, Semaphore};
use tokio::task::JoinSet;
use tokio_util::sync::CancellationToken;
use wreq::Client;

/// Base delay per index (milliseconds) for staggered launch.
const DELAY_BASE_STAGGERED_MS: u64 = 200;

/// Maximum additional jitter (milliseconds) for staggered launch.
const MAX_STAGGERED_JITTER_MS: u64 = 300;

/// Executes multiple queries in parallel respecting the `--parallel` limit.
///
/// # Arguments
/// * `queries` — already deduplicated/filtered list of queries.
/// * `configuracoes` — configuration template (individual query will be overwritten).
/// * `cancelamento` — token that signals SIGINT / global timeout.
///
/// # Failure behaviour
/// If a query fails, its `SearchOutput` is generated with `error` filled in and
/// `results_count = 0`. The process does NOT abort other in-flight queries.
///
/// # Errors
///
/// Returns an error only if the shared HTTP client cannot be built when `paginas <= 1`.
/// Individual query failures are captured inside the returned [`MultiSearchOutput`]
/// rather than propagated as `Err`.
///
/// # Cancel safety
///
/// This function is cancel-safe. Dropping the future signals the cancellation token
/// to all spawned tasks; each task checks the token before and after acquiring its
/// semaphore permit and terminates gracefully.
#[tracing::instrument(skip_all, fields(query_count = queries.len(), parallelism = config.parallelism))]
pub async fn execute_parallel_searches(
    queries: Vec<String>,
    config: Config,
    cancellation: CancellationToken,
) -> Result<MultiSearchOutput, CliError> {
    let query_count = u32::try_from(queries.len()).unwrap_or(u32::MAX);
    let effective_parallelism = config.parallelism.max(1);
    let start_timestamp = chrono::Utc::now().to_rfc3339();

    tracing::info!(
        queries = query_count,
        parallel = effective_parallelism,
        pages = config.pages,
        "Starting parallel multi-query execution"
    );

    // Permit calculation formula:
    // permits = --parallel (user-supplied, default 5, clamped to 1..=20).
    //
    // Dynamic calculation (min(cpus, free_ram / ram_per_task)) is intentionally
    // NOT used because:
    // 1. Bottleneck is DuckDuckGo rate-limiting, not local CPU/RAM.
    // 2. Each HTTP task uses ~2-5 MB RSS — even 20 tasks < 100 MB total.
    // 3. User controls parallelism explicitly via --parallel flag (exposed as CLI arg).
    // 4. The external rate-limit is the true constraint, not local resources.
    let semaphore = Arc::new(Semaphore::new(effective_parallelism as usize));
    let config = Arc::new(config);
    let flag_rate_limit = Arc::new(AtomicBool::new(false));

    let config_proxy = Arc::new(ProxyConfig::from_options(
        config.proxy.as_deref(),
        config.no_proxy,
    ));

    // Decide whether the Client is shared or built per task.
    // Per section 4.3: pages == 1 → shared; pages > 1 → isolated.
    let client_shared: Option<Client> = if config.pages <= 1 {
        let client = http::build_client_with_proxy_and_cookies(
            &config.browser_profile,
            config.timeout_seconds,
            &config.language,
            &config.country,
            &config_proxy,
            config.cookie_provider.clone(),
        )
        .map_err(|e| CliError::HttpError {
            message: format!("failed to build shared HTTP client for multi-query: {e}"),
            cause: None,
        })?;
        Some(client)
    } else {
        None
    };

    let mut task_set: JoinSet<(usize, Result<SearchOutput, CliError>)> = JoinSet::new();

    for (index, query) in queries.into_iter().enumerate() {
        // Clone refs to move into the spawned task.
        let task_semaphore = Arc::clone(&semaphore);
        let task_config = Arc::clone(&config);
        let task_cancellation = cancellation.clone();
        let task_client = client_shared.clone();
        let flag_rate_limit_task = Arc::clone(&flag_rate_limit);
        let config_proxy_task = Arc::clone(&config_proxy);

        task_set.spawn(async move {
            // Staggered launch: delay before acquiring permit to avoid synchronous burst.
            let jitter_ms = rand::rng().random_range(0..MAX_STAGGERED_JITTER_MS);
            let delay_total = Duration::from_millis(
                DELAY_BASE_STAGGERED_MS.saturating_mul(index as u64) + jitter_ms,
            );

            tokio::select! {
                biased;
                _ = task_cancellation.cancelled() => {
                    return (index, Err(CliError::NetworkError { message: format!("execution cancelled before starting query {index}") }));
                }
                _ = tokio::time::sleep(delay_total) => {}
            }

            // Acquire owned semaphore permit — released on drop at task end.
            tracing::debug!(
                permits_available = task_semaphore.available_permits(),
                query_index = index,
                "awaiting semaphore permit"
            );
            let permit = match task_semaphore.acquire_owned().await {
                Ok(p) => p,
                Err(erro) => {
                    return (
                        index,
                        Err(CliError::NetworkError { message: format!("semaphore closed: {erro}") }),
                    );
                }
            };

            tracing::debug!(index, query = %query, "permit acquired, starting task");

            if task_cancellation.is_cancelled() {
                drop(permit);
                return (index, Err(CliError::NetworkError { message: "execution cancelled after acquiring permit".into() }));
            }

            // Per-task Client decision.
            let client_result = match task_client {
                Some(shared) => Ok(shared),
                None => http::build_client_with_proxy_and_cookies(
                    &task_config.browser_profile,
                    task_config.timeout_seconds,
                    &task_config.language,
                    &task_config.country,
                    &config_proxy_task,
                    task_config.cookie_provider.clone(),
                )
                .map_err(|e| CliError::HttpError { message: format!("failed to build isolated Client for query: {e}"), cause: None }),
            };

            let result = match client_result {
                Ok(client) => {
                    execute_query_with_cancellation(
                        &query,
                        &client,
                        &task_config,
                        &flag_rate_limit_task,
                        &task_cancellation,
                    )
                    .await
                }
                Err(erro) => Err(erro),
            };

            drop(permit);
            (index, result)
        });
    }

    // Coleta todas as tasks — preservando a ordem original das queries.
    let mut ordered_results: Vec<Option<SearchOutput>> = (0..query_count).map(|_| None).collect();

    while let Some(resultado_task) = task_set.join_next().await {
        match resultado_task {
            Ok((index, Ok(output))) => {
                ordered_results[index] = Some(output);
            }
            Ok((index, Err(erro))) => {
                tracing::warn!(index, ?erro, "query failed, generating error SearchOutput");
                ordered_results[index] = Some(error_output(index, erro, &config));
            }
            Err(erro_join) => {
                if erro_join.is_panic() {
                    tracing::error!(?erro_join, "task panicked — permit recovered via RAII");
                } else {
                    tracing::warn!(?erro_join, "task cancelled or aborted");
                }
                if let Some(slot) = ordered_results.iter_mut().find(|s| s.is_none()) {
                    *slot = Some(error_output(
                        0,
                        CliError::NetworkError {
                            message: format!("task panicked: {erro_join}"),
                        },
                        &config,
                    ));
                }
            }
        }
    }

    // Convert Option<SearchOutput> into Vec<SearchOutput> (all slots must be filled).
    let searches: Vec<SearchOutput> = ordered_results
        .into_iter()
        .enumerate()
        .map(|(index, slot)| {
            slot.unwrap_or_else(|| {
                error_output(
                    index,
                    CliError::NetworkError {
                        message: format!("missing result for query {index}"),
                    },
                    &config,
                )
            })
        })
        .collect();

    tracing::info!(total = searches.len(), "multi-query complete");

    Ok(MultiSearchOutput {
        query_count,
        timestamp: start_timestamp,
        parallelism: effective_parallelism,
        searches,
    })
}

/// Aggregated statistics for a multi-query execution in streaming mode.
#[derive(Debug, Clone, Default)]
pub struct StreamStats {
    /// Total queries submitted.
    pub total: u32,
    /// Queries completed successfully (no `error` field).
    pub successes: u32,
    /// Queries completed with an error.
    pub errors: u32,
    /// Timestamp (RFC 3339) of execution start.
    pub start_timestamp: String,
    /// Effective parallelism.
    pub parallelism: u32,
}

/// Executes multiple queries in parallel EMITTING results via `mpsc::Sender`
/// as each task finishes. The consumer (in `pipeline`) receives the results and
/// emits NDJSON / text / markdown incrementally.
///
/// Returns `StreamStats` after all tasks have finished.
///
/// Results arrive in COMPLETION ORDER (not the order of the input queries).
/// Each sent item is `(original_index, SearchOutput)` so the consumer knows
/// which query produced each output.
///
/// # Errors
///
/// Returns an error only if the shared HTTP client cannot be built when `paginas <= 1`.
/// Individual query failures are captured in the `SearchOutput` sent through the channel.
/// If the channel receiver is dropped early, remaining tasks are aborted and the
/// function returns the statistics collected up to that point.
///
/// # Cancel safety
///
/// This function is cancel-safe. Dropping the future stops the staggered-launch loop
/// and causes in-flight tasks to observe the cancellation token on their next checkpoint,
/// aborting gracefully without sending to the (now-dropped) channel.
#[tracing::instrument(skip_all, fields(query_count = queries.len(), parallelism = config.parallelism))]
pub async fn execute_parallel_searches_streaming(
    queries: Vec<String>,
    config: Config,
    cancellation: CancellationToken,
    output_channel: mpsc::Sender<(usize, SearchOutput)>,
) -> Result<StreamStats, CliError> {
    let query_count = u32::try_from(queries.len()).unwrap_or(u32::MAX);
    let effective_parallelism = config.parallelism.max(1);
    let start_timestamp = chrono::Utc::now().to_rfc3339();

    tracing::info!(
        queries = query_count,
        parallel = effective_parallelism,
        "Starting parallel multi-query streaming execution"
    );

    let semaphore = Arc::new(Semaphore::new(effective_parallelism as usize));
    let config = Arc::new(config);
    let flag_rate_limit = Arc::new(AtomicBool::new(false));

    let config_proxy = Arc::new(ProxyConfig::from_options(
        config.proxy.as_deref(),
        config.no_proxy,
    ));

    let client_shared: Option<Client> = if config.pages <= 1 {
        let client = http::build_client_with_proxy_and_cookies(
            &config.browser_profile,
            config.timeout_seconds,
            &config.language,
            &config.country,
            &config_proxy,
            config.cookie_provider.clone(),
        )
        .map_err(|e| CliError::HttpError {
            message: format!("failed to build shared HTTP client for streaming: {e}"),
            cause: None,
        })?;
        Some(client)
    } else {
        None
    };

    let mut task_set: JoinSet<(usize, SearchOutput)> = JoinSet::new();

    for (index, query) in queries.into_iter().enumerate() {
        let task_semaphore = Arc::clone(&semaphore);
        let task_config = Arc::clone(&config);
        let task_cancellation = cancellation.clone();
        let task_client = client_shared.clone();
        let flag_rate_limit_task = Arc::clone(&flag_rate_limit);
        let config_proxy_task = Arc::clone(&config_proxy);

        task_set.spawn(async move {
            let jitter_ms = rand::rng().random_range(0..MAX_STAGGERED_JITTER_MS);
            let delay_total = Duration::from_millis(
                DELAY_BASE_STAGGERED_MS.saturating_mul(index as u64) + jitter_ms,
            );

            tokio::select! {
                biased;
                _ = task_cancellation.cancelled() => {
                    return (
                        index,
                        error_output(
                            index,
                            CliError::NetworkError { message: format!("execution cancelled before query {index}") },
                            &task_config,
                        ),
                    );
                }
                _ = tokio::time::sleep(delay_total) => {}
            }

            tracing::debug!(
                permits_available = task_semaphore.available_permits(),
                query_index = index,
                "awaiting semaphore permit (streaming)"
            );
            let permit = match task_semaphore.acquire_owned().await {
                Ok(p) => p,
                Err(erro) => {
                    return (
                        index,
                        error_output(
                            index,
                            CliError::NetworkError { message: format!("semaphore closed: {erro}") },
                            &task_config,
                        ),
                    );
                }
            };

            tracing::debug!(query_index = index, "permit acquired (streaming)");

            if task_cancellation.is_cancelled() {
                drop(permit);
                return (
                    index,
                    error_output(
                        index,
                        CliError::NetworkError { message: "execution cancelled after permit".into() },
                        &task_config,
                    ),
                );
            }

            let client_result = match task_client {
                Some(c) => Ok(c),
                None => http::build_client_with_proxy_and_cookies(
                    &task_config.browser_profile,
                    task_config.timeout_seconds,
                    &task_config.language,
                    &task_config.country,
                    &config_proxy_task,
                    task_config.cookie_provider.clone(),
                )
                .map_err(|e| CliError::HttpError { message: format!("failed to build isolated Client: {e}"), cause: None }),
            };

            let result = match client_result {
                Ok(client) => {
                    execute_query_with_cancellation(
                        &query,
                        &client,
                        &task_config,
                        &flag_rate_limit_task,
                        &task_cancellation,
                    )
                    .await
                }
                Err(erro) => Err(erro),
            };

            drop(permit);
            match result {
                Ok(output) => (index, output),
                Err(erro) => (index, error_output(index, erro, &task_config)),
            }
        });
    }

    let mut success_count: u32 = 0;
    let mut error_count: u32 = 0;

    while let Some(task_result) = task_set.join_next().await {
        match task_result {
            Ok((index, output)) => {
                if output.error.is_some() {
                    error_count = error_count.saturating_add(1);
                } else {
                    success_count = success_count.saturating_add(1);
                }
                if let Err(send_error) = output_channel.send((index, output)).await {
                    tracing::warn!(
                        ?send_error,
                        "streaming consumer closed channel — aborting send"
                    );
                    task_set.abort_all();
                    break;
                }
            }
            Err(join_err) => {
                if join_err.is_panic() {
                    tracing::error!(
                        ?join_err,
                        "task panicked in streaming — permit recovered via RAII"
                    );
                } else {
                    tracing::warn!(?join_err, "task cancelled in streaming");
                }
                error_count = error_count.saturating_add(1);
            }
        }
    }

    tracing::info!(
        total = query_count,
        successes = success_count,
        errors = error_count,
        "streaming complete"
    );

    Ok(StreamStats {
        total: query_count,
        successes: success_count,
        errors: error_count,
        start_timestamp,
        parallelism: effective_parallelism,
    })
}

/// Executes ONE query with pagination, retry, Lite fallback and fetch-content (if enabled).
async fn execute_query_with_cancellation(
    query: &str,
    client: &Client,
    config: &Config,
    flag_rate_limit: &Arc<AtomicBool>,
    cancellation: &CancellationToken,
) -> Result<SearchOutput, CliError> {
    let start = Instant::now();

    if cancellation.is_cancelled() {
        return Err(CliError::NetworkError {
            message: format!("execution cancelled before request for {query:?}"),
        });
    }

    tracing::info!(query = %query, endpoint = config.endpoint.as_str(), "sending request");

    // Create a copy with the query overridden for `search_with_pagination`.
    let mut cfg_task = config.clone();
    cfg_task.query = query.to_string();

    let agregado = match search::search_with_pagination(
        client,
        &cfg_task,
        query,
        flag_rate_limit,
        cancellation,
    )
    .await
    {
        Ok(a) => a,
        Err(reason) => {
            let elapsed_ms = start.elapsed().as_millis().min(u64::MAX as u128) as u64;
            let timestamp = chrono::Utc::now().to_rfc3339();
            let selectors_hash = crate::pipeline::calculate_selectors_hash(&config.selectors);
            let used_proxy =
                ProxyConfig::from_options(config.proxy.as_deref(), config.no_proxy).is_active();
            return Ok(SearchOutput {
                query: query.to_string(),
                engine: "duckduckgo".to_string(),
                endpoint: config.endpoint.as_str().to_string(),
                timestamp,
                region: search::format_kl(&config.language, &config.country),
                result_count: 0,
                results: Vec::new(),
                pages_fetched: 0,
                error: Some(reason.as_error_code().to_string()),
                message: Some(reason.message()),
                metadata: SearchMetadata {
                    execution_time_ms: elapsed_ms,
                    selectors_hash,
                    retries: config.retries,
                    used_fallback_endpoint: false,
                    concurrent_fetches: 0,
                    fetch_successes: 0,
                    fetch_failures: 0,
                    used_chrome: false,
                    user_agent: config.user_agent.clone(),
                    used_proxy,
                    identity_used: None,
                    cascade_level: None,
                },
            });
        }
    };

    let quantidade = u32::try_from(agregado.results.len()).unwrap_or(u32::MAX);
    let selectors_hash = crate::pipeline::calculate_selectors_hash(&config.selectors);
    let elapsed_ms = start.elapsed().as_millis().min(u64::MAX as u128) as u64;
    let timestamp = chrono::Utc::now().to_rfc3339();
    let retries_count = agregado.attempts.saturating_sub(1);

    let used_proxy =
        ProxyConfig::from_options(config.proxy.as_deref(), config.no_proxy).is_active();
    let metadata_val = SearchMetadata {
        execution_time_ms: elapsed_ms,
        selectors_hash,
        retries: retries_count,
        used_fallback_endpoint: agregado.used_fallback_lite,
        concurrent_fetches: 0,
        fetch_successes: 0,
        fetch_failures: 0,
        used_chrome: false,
        user_agent: config.user_agent.clone(),
        used_proxy,
        identity_used: None,
        cascade_level: None,
    };

    let mut output = SearchOutput {
        query: query.to_string(),
        engine: "duckduckgo".to_string(),
        endpoint: agregado.effective_endpoint.as_str().to_string(),
        timestamp,
        region: search::format_kl(&config.language, &config.country),
        result_count: quantidade,
        results: agregado.results,
        pages_fetched: agregado.pages_fetched,
        error: None,
        message: None,
        metadata: metadata_val,
    };

    // Enriquecimento opcional via --fetch-content (iter. 5).
    content_fetch::enrich_with_content(&mut output, client, config, cancellation).await;

    Ok(output)
}

/// Generates a `SearchOutput` representing a failed query.
///
/// Preserves the position in the multi-query output even when an individual query failed.
#[cold]
fn error_output(index: usize, erro: CliError, config: &Config) -> SearchOutput {
    let query_ref = config.queries.get(index).cloned().unwrap_or_default();
    let message = format!("{erro:#}");
    let timestamp = chrono::Utc::now().to_rfc3339();
    let selectors_hash = crate::pipeline::calculate_selectors_hash(&config.selectors);

    SearchOutput {
        query: query_ref,
        engine: "duckduckgo".to_string(),
        endpoint: "html".to_string(),
        timestamp,
        region: search::format_kl(&config.language, &config.country),
        result_count: 0,
        results: Vec::new(),
        pages_fetched: 0,
        error: Some(crate::error::codes::NETWORK_ERROR.to_string()),
        message: Some(message),
        metadata: SearchMetadata {
            execution_time_ms: 0,
            selectors_hash,
            retries: 0,
            used_fallback_endpoint: false,
            concurrent_fetches: 0,
            fetch_successes: 0,
            fetch_failures: 0,
            used_chrome: false,
            user_agent: config.user_agent.clone(),
            used_proxy: false,
            identity_used: None,
            cascade_level: None,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Endpoint, OutputFormat, SafeSearch, SelectorConfig};

    fn test_config(queries: Vec<String>, parallelism: u32) -> Config {
        let first_query = queries.first().cloned().unwrap_or_default();
        Config {
            query: first_query,
            queries,
            num_results: None,
            format: OutputFormat::Json,
            timeout_seconds: 15,
            language: "pt".to_string(),
            country: "br".to_string(),
            verbose: false,
            quiet: true,
            user_agent: "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/146.0.0.0 Safari/537.36".to_string(),
            browser_profile: crate::http::create_browser_profile("Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/146.0.0.0 Safari/537.36"),
            parallelism,
            pages: 1,
            retries: 0,
            endpoint: Endpoint::Html,
            time_filter: None,
            safe_search: SafeSearch::Moderate,
            stream_mode: false,
            output_file: None,
            fetch_content: false,
            max_content_length: 10_000,
            proxy: None,
            no_proxy: false,
            global_timeout_seconds: 60,
            match_platform_ua: false,
            per_host_limit: 2,
            chrome_path: None,
            selectors: std::sync::Arc::new(SelectorConfig::default()),
            cookie_provider: None,
            persistent_jar: None,
            warmup_enabled: false,
        allow_lite_fallback: false,
        }
    }

    #[test]
    fn error_output_fills_required_fields() {
        let cfg = test_config(vec!["alfa".into(), "beta".into()], 2);
        let erro = CliError::NetworkError {
            message: "synthetic test failure".into(),
        };
        let output = error_output(1, erro, &cfg);
        assert_eq!(output.query, "beta");
        assert_eq!(output.result_count, 0);
        assert!(output.results.is_empty());
        assert!(output.error.is_some());
        assert!(output.message.is_some());
        assert_eq!(output.region, "br-pt");
    }

    #[test]
    fn error_output_index_out_of_bounds_uses_empty_string() {
        let cfg = test_config(vec!["apenas uma".into()], 1);
        let output = error_output(
            99,
            CliError::NetworkError {
                message: "out of bounds".into(),
            },
            &cfg,
        );
        // No query available for the index → empty string, but no panic.
        assert!(output.query.is_empty());
        assert!(output.error.is_some());
    }

    #[tokio::test]
    async fn parallel_searches_cancelled_before_spawn_returns_errors() {
        // Cancelamos ANTES de chamar, todas as tasks devem retornar falha controlada.
        let token = CancellationToken::new();
        token.cancel();
        let cfg = test_config(
            vec!["query-a".into(), "query-b".into(), "query-c".into()],
            3,
        );
        let queries = cfg.queries.clone();
        let result = execute_parallel_searches(queries, cfg, token).await;
        let output = result.expect("function should return Ok even when all fail");
        assert_eq!(output.query_count, 3);
        assert_eq!(output.searches.len(), 3);
        assert_eq!(output.parallelism, 3);
        // Todas devem estar marcadas com erro.
        for search in &output.searches {
            assert!(
                search.error.is_some(),
                "query {:?} deveria ter falhado com cancelamento",
                search.query
            );
        }
    }

    #[test]
    fn calculate_selectors_hash_returns_16_chars() {
        let cfg = SelectorConfig::default();
        let hash = crate::pipeline::calculate_selectors_hash(&cfg);
        assert_eq!(hash.len(), 16);
        assert!(hash.chars().all(|c| c.is_ascii_hexdigit()));
    }
}
