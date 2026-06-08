// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload classification: I/O-bound orchestrator (dispatches to parallel.rs and content_fetch.rs).
// No direct parallelism in this module — delegates fan-out to parallel::execute_*.
// Bounded mpsc channel provides backpressure between producer and consumer in streaming mode.
//! Orchestration of the CLI execution flow.
//!
//! In iteration 2, decides between single-query and multi-query flow based on
//! the number of effective queries (after combining positional + file + stdin,
//! dedup and empty-string filtering).
//!
//! - Single-query (1 query): uses the legacy `execute_single_search` flow and emits `SearchOutput`.
//! - Multi-query (>=2 queries): delegates to `parallel::execute_parallel_searches`
//!   and emits `MultiSearchOutput`.

use crate::content_fetch;
use crate::error::CliError;
use crate::http;
use crate::http::ProxyConfig;
use crate::parallel;
use crate::search;
use crate::types::{Config, MultiSearchOutput, SearchMetadata, SearchOutput, SelectorConfig};
use std::collections::HashSet;
use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::Instant;
use tokio_util::sync::CancellationToken;

/// Result emitted by the pipeline — may be a single output, aggregated multi output, or an already-emitted stream.
///
/// The `Stream` variant indicates that output was already emitted incrementally by
/// the consumer; the final `output` step MUST NOT re-emit anything. Only the
/// aggregated statistics are available for logging / exit-code decisions.
#[derive(Debug, Clone)]
pub enum PipelineResult {
    /// Single-query execution produced one output.
    Single(Box<SearchOutput>),
    /// Multi-query execution produced aggregated output.
    Multi(Box<MultiSearchOutput>),
    /// Streaming mode — output already emitted incrementally; only stats remain.
    Stream(crate::parallel::StreamStats),
}

impl PipelineResult {
    /// Total results summed across all queries (used for exit-code decisions).
    ///
    /// For `Stream` returns `successes` — a sufficient approximation for exit codes 0/5
    /// (success vs zero-results).
    pub fn total_results(&self) -> u32 {
        match self {
            PipelineResult::Single(s) => s.result_count,
            PipelineResult::Multi(m) => m
                .searches
                .iter()
                .map(|b| b.result_count)
                .fold(0u32, |acc, v| acc.saturating_add(v)),
            PipelineResult::Stream(e) => e.successes,
        }
    }
}

/// Entry point for iteration 2: decides single vs multi based on `configuracoes.queries`.
///
/// `cancelamento` is the token that signals SIGINT (ctrl+c). In single-query mode
/// cancellation only affects the request via `reqwest` timeout; in multi-query mode it
/// is propagated explicitly to each task.
///
/// # Errors
///
/// Returns an error if the query list is empty, if the HTTP client cannot be built,
/// or if the underlying single-query or multi-query execution fails unrecoverably.
///
/// # Cancel safety
///
/// This function is cancel-safe. Dropping the future propagates the cancellation
/// token to any in-flight sub-tasks, which will terminate gracefully.
pub async fn execute_pipeline(
    config: Config,
    cancellation: CancellationToken,
) -> Result<PipelineResult, CliError> {
    match config.queries.len() {
        0 => Err(CliError::InvalidConfig {
            message: "no queries to execute (list empty after filtering)".into(),
        }),
        1 => {
            if config.stream_mode {
                tracing::warn!(
                    "--stream ignored in single-query mode (only 1 effective query); \
                     emitting default aggregated output"
                );
            }
            // Clone intentional: overwrites query field for single-query compatibility.
            // Cost: ~15 String clones, executed exactly once per CLI invocation.
            let mut cfg_single = config.clone();
            cfg_single.query = cfg_single.queries[0].clone();
            let output = execute_single_search(&cfg_single, &cancellation).await?;
            persist_cookies(&cfg_single);
            Ok(PipelineResult::Single(Box::new(output)))
        }
        _ => {
            if config.stream_mode {
                return execute_pipeline_streaming(config, cancellation).await;
            }
            let queries = config.queries.clone();
            // Persist cookies after the parallel search completes, using
            // a clone of `config` because `config` is moved into the
            // search call.
            let config_for_persist = config.clone();
            let multi = parallel::execute_parallel_searches(queries, config, cancellation).await?;
            persist_cookies(&config_for_persist);
            Ok(PipelineResult::Multi(Box::new(multi)))
        }
    }
}

/// Persists the cookie jar to disk after the search completes. v0.7.3 PR2.
fn persist_cookies(config: &Config) {
    if let Some(persistent_jar) = config.persistent_jar.as_ref() {
        persistent_jar.save();
    }
}

/// Performs the warm-up `GET https://duckduckgo.com/` request to populate
/// session cookies. Failures are surfaced to the caller but never fatal;
/// the caller logs and continues. v0.7.3 PR2.
async fn do_warmup(client: &wreq::Client, cfg: &Config) -> Result<(), CliError> {
    let warmup_url = "https://duckduckgo.com/";
    tracing::info!(url = warmup_url, "Warming up session with cookie jar");
    let response = client
        .get(warmup_url)
        .send()
        .await
        .map_err(|e| CliError::HttpError {
            message: format!("warm-up request to {warmup_url} failed: {e}"),
            cause: None,
        })?;
    tracing::debug!(
        status = response.status().as_u16(),
        url = warmup_url,
        "warm-up response received"
    );
    let _ = cfg; // cfg is reserved for future per-query warm-up tuning
    Ok(())
}

/// Pipeline in streaming mode — emits results as tasks complete.
///
/// The spawned consumer drains the mpsc channel and emits NDJSON/text/markdown line by line.
/// Returns `PipelineResult::Stream` at the end, indicating there is nothing left to emit.
async fn execute_pipeline_streaming(
    config: Config,
    cancellation: CancellationToken,
) -> Result<PipelineResult, CliError> {
    use crate::types::OutputFormat;
    use tokio::sync::mpsc;

    let format = config.format;
    let output_file = config.output_file.clone();
    let queries = config.queries.clone();
    let paralelismo = config.parallelism.max(1) as usize;

    // Buffer = parallelism * 2, per spec. Min 2 to avoid trivial starvation.
    let (tx, mut rx) = mpsc::channel::<(usize, SearchOutput)>(paralelismo.saturating_mul(2).max(2));

    // Spawn consumer: drains items and emits per format.
    let consumer = tokio::spawn(async move {
        let mut emitidos: u64 = 0;
        while let Some((index, output)) = rx.recv().await {
            let resolved_format = match format {
                OutputFormat::Auto | OutputFormat::Json => OutputFormat::Json,
                outro => outro,
            };
            let res = match resolved_format {
                OutputFormat::Json | OutputFormat::Auto => {
                    crate::output::emit_ndjson(&output, output_file.as_deref())
                }
                OutputFormat::Text => {
                    crate::output::emit_stream_text(index, &output, output_file.as_deref())
                }
                OutputFormat::Markdown => {
                    crate::output::emit_stream_markdown(index, &output, output_file.as_deref())
                }
            };
            if let Err(erro) = res {
                if crate::output::is_broken_pipe(&erro) {
                    tracing::debug!("BrokenPipe in streaming — stopping consumer");
                    return Ok(());
                }
                tracing::error!(?erro, "failed to emit streaming item — aborting consumer");
                return Err(erro);
            }
            emitidos = emitidos.saturating_add(1);
        }
        tracing::info!(emitidos, "streaming consumer finished");
        Ok::<(), CliError>(())
    });

    let stats =
        parallel::execute_parallel_searches_streaming(queries, config, cancellation, tx).await?;

    match consumer.await {
        Ok(Ok(())) => {}
        Ok(Err(erro)) => return Err(erro),
        Err(erro_join) => {
            if erro_join.is_panic() {
                tracing::error!(?erro_join, "streaming consumer panicked");
            } else {
                tracing::warn!(?erro_join, "streaming consumer cancelled");
            }
            return Err(CliError::NetworkError {
                message: format!("streaming consumer panicked: {erro_join}"),
            });
        }
    }

    Ok(PipelineResult::Stream(stats))
}

/// Executes the full flow for a single-query search with pagination, retry and Lite fallback.
///
/// # Errors
///
/// Returns an error if the HTTP client cannot be built. Search failures (rate limit,
/// timeout, block) are captured in the returned [`SearchOutput`] error fields rather
/// than propagated as `Err`.
///
/// # Cancel safety
///
/// This function is cancel-safe. Dropping the future aborts the in-flight HTTP
/// request; any partial pagination state is discarded without side effects.
pub async fn execute_single_search(
    cfg: &Config,
    cancellation: &CancellationToken,
) -> Result<SearchOutput, CliError> {
    let start = Instant::now();

    let config_proxy = ProxyConfig::from_options(cfg.proxy.as_deref(), cfg.no_proxy);
    let client = http::build_client_with_proxy_and_cookies(
        &cfg.browser_profile,
        cfg.timeout_seconds,
        &cfg.language,
        &cfg.country,
        &config_proxy,
        cfg.cookie_provider.clone(),
    )?;

    // v0.7.3 PR2: warm up the session with a `GET https://duckduckgo.com/`
    // so the cookie jar is populated before the real query. Best-effort:
    // any failure is logged and the real query runs anyway.
    if cfg.warmup_enabled {
        if let Err(e) = do_warmup(&client, cfg).await {
            tracing::warn!(error = %e, "warm-up request failed; continuing without it");
        }
    }

    tracing::info!(query = %cfg.query, endpoint = cfg.endpoint.as_str(), "Executing search");

    let flag_rate_limit = Arc::new(AtomicBool::new(false));

    let agregado = match search::search_with_pagination(
        &client,
        cfg,
        &cfg.query,
        &flag_rate_limit,
        cancellation,
    )
    .await
    {
        Ok(a) => a,
        Err(reason) => {
            return Ok(failure_output(cfg, &reason, start));
        }
    };

    let quantidade = u32::try_from(agregado.results.len()).unwrap_or(u32::MAX);
    let selectors_hash = calculate_selectors_hash(&cfg.selectors);
    let elapsed_ms = start.elapsed().as_millis().min(u64::MAX as u128) as u64;
    let timestamp = chrono::Utc::now().to_rfc3339();
    // Retries = attempts - 1 (the first request does not count as a retry).
    let retries_count = agregado.attempts.saturating_sub(1);

    let metadata_val = SearchMetadata {
        execution_time_ms: elapsed_ms,
        selectors_hash,
        retries: retries_count,
        used_fallback_endpoint: agregado.used_fallback_lite,
        concurrent_fetches: 0,
        fetch_successes: 0,
        fetch_failures: 0,
        used_chrome: false,
        user_agent: cfg.user_agent.clone(),
        used_proxy: config_proxy.is_active(),
        identity_used: None,
        cascade_level: None,
    };

    let mut output = SearchOutput {
        query: cfg.query.clone(),
        engine: "duckduckgo".to_string(),
        endpoint: agregado.effective_endpoint.as_str().to_string(),
        timestamp,
        region: search::format_kl(&cfg.language, &cfg.country),
        result_count: quantidade,
        results: agregado.results,
        pages_fetched: agregado.pages_fetched,
        error: None,
        message: None,
        metadata: metadata_val,
    };

    // Enriquecimento opcional via --fetch-content (iter. 5).
    content_fetch::enrich_with_content(&mut output, &client, cfg, cancellation).await;

    tracing::info!(
        total = output.result_count,
        pages = output.pages_fetched,
        fallback = output.metadata.used_fallback_endpoint,
        fetch_content = cfg.fetch_content,
        fetch_successes = output.metadata.fetch_successes,
        "Search completed successfully"
    );
    Ok(output)
}

/// Generates a `SearchOutput` from a retry failure, preserving the structured error code
/// and partial metrics.
#[cold]
fn failure_output(cfg: &Config, reason: &search::RetryFailReason, start: Instant) -> SearchOutput {
    let elapsed_ms = start.elapsed().as_millis().min(u64::MAX as u128) as u64;
    let timestamp = chrono::Utc::now().to_rfc3339();
    let selectors_hash = calculate_selectors_hash(&cfg.selectors);
    let used_proxy = ProxyConfig::from_options(cfg.proxy.as_deref(), cfg.no_proxy).is_active();

    SearchOutput {
        query: cfg.query.clone(),
        engine: "duckduckgo".to_string(),
        endpoint: cfg.endpoint.as_str().to_string(),
        timestamp,
        region: search::format_kl(&cfg.language, &cfg.country),
        result_count: 0,
        results: Vec::new(),
        pages_fetched: 0,
        error: Some(reason.as_error_code().to_string()),
        message: Some(reason.message()),
        metadata: SearchMetadata {
            execution_time_ms: elapsed_ms,
            selectors_hash,
            retries: cfg.retries,
            used_fallback_endpoint: false,
            concurrent_fetches: 0,
            fetch_successes: 0,
            fetch_failures: 0,
            used_chrome: false,
            user_agent: cfg.user_agent.clone(),
            used_proxy,
            identity_used: None,
            cascade_level: None,
        },
    }
}

/// Backwards-compatible alias — preserves the `execute` name used in the original `lib.rs`.
///
/// # Errors
///
/// Returns an error if the HTTP client cannot be built or if `execute_single_search`
/// fails unrecoverably (see that function's documentation for details).
///
/// # Cancel safety
///
/// This function is cancel-safe. It delegates directly to [`execute_single_search`]
/// with a fresh, never-cancelled [`CancellationToken`]; dropping the future is safe.
pub async fn execute(cfg: &Config) -> Result<SearchOutput, CliError> {
    execute_single_search(cfg, &CancellationToken::new()).await
}

/// Combines queries from three sources (positional, file, stdin), deduplicates
/// preserving the ORDER of the first occurrence, and filters empty strings after trim.
///
/// Performs no I/O: expects the caller to have already collected the lines (useful for tests).
///
/// # Example
///
/// ```
/// use duckduckgo_search_cli::pipeline::combine_and_dedup_queries;
///
/// let result_vec = combine_and_dedup_queries(
///     vec!["rust".into(), "  ".into(), "tokio".into()],
///     vec!["rust".into(), "serde".into()],
///     vec!["".into(), "serde".into(), "axum".into()],
/// );
///
/// // Dedup preserves order of first occurrence; empty strings (after trim) are removed.
/// assert_eq!(result_vec, vec!["rust", "tokio", "serde", "axum"]);
/// ```
pub fn combine_and_dedup_queries(
    posicionais: Vec<String>,
    de_arquivo: Vec<String>,
    de_stdin: Vec<String>,
) -> Vec<String> {
    let capacity = posicionais.len() + de_arquivo.len() + de_stdin.len();
    let mut vistos: HashSet<String> = HashSet::with_capacity(capacity);
    let mut result_vec: Vec<String> = Vec::with_capacity(capacity);

    let todas = posicionais.into_iter().chain(de_arquivo).chain(de_stdin);

    for raw in todas {
        let clean = raw.trim().to_string();
        if clean.is_empty() {
            continue;
        }
        if vistos.insert(clean.clone()) {
            result_vec.push(clean);
        }
    }

    result_vec
}

/// Reads a queries file — one query per line, ignoring empty lines after trim.
///
/// Correctly handles both `\n` and `\r\n` (Windows) via `BufRead::lines`.
///
/// # Errors
///
/// Returns an error if the file cannot be opened or if any line cannot be read
/// (e.g. invalid UTF-8 or an I/O error).
// std::fs is intentional: query files are small config files (<1 KB typical)
// read synchronously BEFORE fan-out begins. No async tasks are blocked.
// Migrating to tokio::fs would add complexity without measurable benefit.
pub fn read_queries_from_file(path: &Path) -> Result<Vec<String>, CliError> {
    use std::io::BufRead;
    let file = std::fs::File::open(path).map_err(|e| CliError::PathError {
        message: format!("failed to open query file {}: {e}", path.display()),
    })?;
    let reader = std::io::BufReader::new(file);
    let mut lines_vec: Vec<String> = Vec::with_capacity(20);
    for (index, line) in reader.lines().enumerate() {
        let line = line.map_err(|e| CliError::PathError {
            message: format!(
                "failed to read line {} of {}: {e}",
                index + 1,
                path.display()
            ),
        })?;
        let trimmed = line.trim().to_string();
        if !trimmed.is_empty() {
            lines_vec.push(trimmed);
        }
    }
    Ok(lines_vec)
}

/// Reads queries from stdin — one per line — ONLY if stdin is not a TTY.
/// Returns an empty `Vec` when stdin is a TTY (i.e. the user did not pipe/redirect input).
///
/// # Errors
///
/// Returns an error if any line from stdin cannot be read (e.g. invalid UTF-8
/// or an I/O error while consuming the piped input).
pub fn read_queries_from_stdin_if_pipe() -> Result<Vec<String>, CliError> {
    use std::io::{BufRead, IsTerminal};
    if std::io::stdin().is_terminal() {
        return Ok(Vec::new());
    }
    let reader = std::io::stdin().lock();
    let mut lines_vec: Vec<String> = Vec::with_capacity(20);
    for (index, line) in reader.lines().enumerate() {
        let line = line.map_err(|e| CliError::PathError {
            message: format!("failed to read line {} from stdin: {e}", index + 1),
        })?;
        let trimmed = line.trim().to_string();
        if !trimmed.is_empty() {
            lines_vec.push(trimmed);
        }
    }
    Ok(lines_vec)
}

/// Computes a blake3 hash (hex, first 16 chars) of the serialised selector configuration.
/// Useful for versioning changes to the `selectors.toml` file in future iterations.
pub(crate) fn calculate_selectors_hash(cfg: &SelectorConfig) -> String {
    match toml::to_string(cfg) {
        Ok(serialized) => {
            let hash = blake3::hash(serialized.as_bytes());
            hash.to_hex().chars().take(16).collect()
        }
        Err(err) => {
            tracing::warn!(?err, "failed to serialize selector config for hash");
            "unknown".to_string()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn calculate_selectors_hash_returns_16_chars() {
        let cfg = SelectorConfig::default();
        let hash = calculate_selectors_hash(&cfg);
        assert_eq!(hash.len(), 16);
        assert!(hash.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn calculate_selectors_hash_is_deterministic() {
        let cfg = SelectorConfig::default();
        let h1 = calculate_selectors_hash(&cfg);
        let h2 = calculate_selectors_hash(&cfg);
        assert_eq!(h1, h2);
    }

    #[test]
    fn combinar_deduplica_preservando_ordem_da_primeira_ocorrencia() {
        let posicionais = vec!["alfa".to_string(), "beta".to_string()];
        let de_arquivo = vec!["beta".to_string(), "gama".to_string()];
        let de_stdin = vec!["alfa".to_string(), "delta".to_string()];
        let combinado = combine_and_dedup_queries(posicionais, de_arquivo, de_stdin);
        assert_eq!(
            combinado,
            vec!["alfa", "beta", "gama", "delta"],
            "ordem deve ser da primeira ocorrência; duplicatas devem ser removidas"
        );
    }

    #[test]
    fn combinar_remove_strings_vazias_e_apenas_espacos() {
        let posicionais = vec!["   ".to_string(), "rust".to_string(), "".to_string()];
        let de_arquivo = vec!["\t\t".to_string(), "tokio".to_string()];
        let de_stdin = vec![];
        let combinado = combine_and_dedup_queries(posicionais, de_arquivo, de_stdin);
        assert_eq!(combinado, vec!["rust", "tokio"]);
    }

    #[test]
    fn combine_trims_whitespace_before_comparing() {
        let posicionais = vec!["  alfa  ".to_string()];
        let de_arquivo = vec!["alfa".to_string()];
        let de_stdin = vec!["alfa\t".to_string()];
        let combinado = combine_and_dedup_queries(posicionais, de_arquivo, de_stdin);
        assert_eq!(
            combinado,
            vec!["alfa"],
            "queries equivalentes após trim devem ser deduplicadas"
        );
    }

    #[test]
    fn combine_empty_returns_empty() {
        let combinado = combine_and_dedup_queries(vec![], vec![], vec![]);
        assert!(combinado.is_empty());
    }

    #[test]
    fn read_queries_from_file_accepts_windows_lines_and_empty() {
        use std::io::Write;
        let dir = std::env::temp_dir().join("ddg_cli_iter2_queries_test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("queries.txt");
        let content = "rust\r\ntokio\r\n\r\n  axum  \n\nhttp://exemplo.com\n";
        let mut file = std::fs::File::create(&path).unwrap();
        file.write_all(content.as_bytes()).unwrap();
        drop(file);

        let lines = read_queries_from_file(&path).expect("should read file");
        assert_eq!(lines, vec!["rust", "tokio", "axum", "http://exemplo.com"]);
        // Cleanup best-effort.
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn total_results_in_single_output() {
        let output = SearchOutput {
            query: "q".into(),
            engine: "duckduckgo".into(),
            endpoint: "html".into(),
            timestamp: "t".into(),
            region: "br-pt".into(),
            result_count: 7,
            results: vec![],
            pages_fetched: 1,
            error: None,
            message: None,
            metadata: SearchMetadata {
                execution_time_ms: 0,
                selectors_hash: "x".into(),
                retries: 0,
                used_fallback_endpoint: false,
                concurrent_fetches: 0,
                fetch_successes: 0,
                fetch_failures: 0,
                used_chrome: false,
                user_agent: "ua".into(),
                used_proxy: false,
                identity_used: None,
                cascade_level: None,
            },
        };
        assert_eq!(PipelineResult::Single(Box::new(output)).total_results(), 7);
    }

    #[test]
    fn total_results_in_multi_output_sums_all() {
        let nova_saida = |n: u32| SearchOutput {
            query: "q".into(),
            engine: "duckduckgo".into(),
            endpoint: "html".into(),
            timestamp: "t".into(),
            region: "br-pt".into(),
            result_count: n,
            results: vec![],
            pages_fetched: 1,
            error: None,
            message: None,
            metadata: SearchMetadata {
                execution_time_ms: 0,
                selectors_hash: "x".into(),
                retries: 0,
                used_fallback_endpoint: false,
                concurrent_fetches: 0,
                fetch_successes: 0,
                fetch_failures: 0,
                used_chrome: false,
                user_agent: "ua".into(),
                used_proxy: false,
                identity_used: None,
                cascade_level: None,
            },
        };
        let multi = MultiSearchOutput {
            query_count: 3,
            timestamp: "t".into(),
            parallelism: 3,
            searches: vec![nova_saida(2), nova_saida(5), nova_saida(0)],
        };
        assert_eq!(PipelineResult::Multi(Box::new(multi)).total_results(), 7);
    }
}
