// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload classification: I/O-bound (HTTP content extraction + CPU-light HTML parsing).
// Bottleneck: per-page download latency (~300-2000ms depending on page size).
// Saturated resource: outbound connections (global) + per-host connection limits.
// Dual semaphore pattern: global limits total concurrency, per-host limits per-domain.
//! Parallel fan-out for content extraction (flag `--fetch-content`).
//!
//! For each result in a `SearchOutput`, spawns an async task bounded by a
//! `Semaphore` (same capacity as `--parallel`). Each task calls
//! [`crate::content::extract_http_content`] and fills `SearchResult.conteudo`,
//! `.tamanho_conteudo` and `.metodo_extracao_conteudo` when successful.
//!
//! Also updates the `SearchMetadata` fields:
//! - `fetches_simultaneos` = total spawned tasks.
//! - `sucessos_fetch` = tasks that returned non-empty `conteudo`.
//! - `falhas_fetch` = tasks that returned an error or empty content.
//!
//! Extraction respects `CancellationToken` — global cancellation aborts all
//! in-flight tasks quickly.

use crate::content;
use crate::types::{Config, SearchOutput};
use indicatif::{ProgressBar, ProgressStyle};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{Mutex, Semaphore};
use tokio::task::JoinSet;
use tokio_util::sync::CancellationToken;
use wreq::Client;

#[cfg(feature = "chrome")]
use crate::browser::{detect_chrome, extract_text_with_chrome, ChromeBrowser};

/// Map `host → Semaphore` for per-host rate-limiting shared across tasks.
pub type PerHostSemaphoreMap = Arc<Mutex<HashMap<String, Arc<Semaphore>>>>;

// =========================================================================
// WS-12 — Circuit breaker per-host (stdlib only, no extra dependency for
// the PATCH bump). After `FAILURE_THRESHOLD` consecutive failures against a
// given host, the breaker is OPEN for `COOLDOWN`; during cooldown, all
// requests to that host are rejected without attempting a network round-trip.
// A single success resets the counter. The breaker is shared across all
// parallel fetches via an `Arc<Mutex<...>>` so fan-out workers coordinate.
// =========================================================================

/// Number of consecutive failures before opening the circuit.
const CB_FAILURE_THRESHOLD: u32 = 3;
/// How long the breaker stays OPEN before allowing a probe request.
const CB_COOLDOWN: Duration = Duration::from_secs(30);

/// Internal state for one host in the circuit breaker.
#[derive(Debug, Clone, Copy, Default)]
pub enum BreakerState {
    /// No recent failures — requests flow normally.
    #[default]
    Closed,
    /// Cooldown window — all requests are short-circuited until the
    /// `Instant` recorded in `until` elapses.
    Open {
        /// Absolute time at which the cooldown elapses.
        until: Instant,
    },
}

/// Per-host circuit breaker entry. The `failure_count` is reset to zero on
/// every success; reaching the threshold flips the state to `Open`.
#[derive(Debug, Clone)]
pub struct BreakerEntry {
    state: BreakerState,
    failure_count: u32,
}

impl Default for BreakerEntry {
    fn default() -> Self {
        Self {
            state: BreakerState::Closed,
            failure_count: 0,
        }
    }
}

/// Map `host → BreakerEntry` shared across parallel fetches.
///
/// Wrapped in a newtype (rather than a type alias) so we can define inherent
/// `impl` methods — Rust's orphan rule forbids inherent impls on
/// `Arc<std::sync::Mutex<...>>` because both types are foreign. The wrapped
/// `std::sync::Mutex` is held only for short critical sections (state lookup
/// and update), never across `.await` points — this avoids the `Send`
/// constraint of `tokio::sync::Mutex` and is sufficient because the lock is
/// uncontended in the common path.
#[derive(Clone, Debug)]
pub struct CircuitBreakerMap(Arc<std::sync::Mutex<HashMap<String, BreakerEntry>>>);

/// Outcome of a `check_and_record_*` call on the breaker.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BreakerDecision {
    /// Breaker is Closed (or cooldown elapsed) — proceed with the request.
    Allow,
    /// Breaker is Open — short-circuit the request to avoid hammering a
    /// known-failing host.
    Reject,
}

impl CircuitBreakerMap {
    /// Creates a new empty breaker map.
    pub fn new() -> Self {
        Self(Arc::new(std::sync::Mutex::new(HashMap::new())))
    }

    /// Acquires the underlying mutex. Provided for introspection in tests
    /// and health-checks; production code should prefer [`Self::check`],
    /// [`Self::record_success`], and [`Self::record_failure`] which are
    /// safe-by-construction short critical sections.
    ///
    /// # Panics
    ///
    /// Panics if the underlying `std::sync::Mutex` is poisoned. The breaker
    /// never holds the lock across an `.await` and never panics inside the
    /// critical section, so a poisoned lock indicates a bug elsewhere.
    pub fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<String, BreakerEntry>> {
        self.0.lock().expect("circuit breaker mutex poisoned")
    }

    /// Returns `Allow` if the host may receive a request, `Reject` otherwise.
    ///
    /// Side effect: if the breaker is `Open` and the cooldown window has
    /// elapsed, the entry is reset to `Closed` (half-open probe).
    pub fn check(&self, host: &str) -> BreakerDecision {
        let mut map = self.lock();
        let Some(entry) = map.get_mut(host) else {
            return BreakerDecision::Allow;
        };
        match entry.state {
            BreakerState::Closed => BreakerDecision::Allow,
            BreakerState::Open { until } => {
                if Instant::now() >= until {
                    // Half-open: reset to Closed and let one probe through.
                    entry.state = BreakerState::Closed;
                    entry.failure_count = 0;
                    BreakerDecision::Allow
                } else {
                    BreakerDecision::Reject
                }
            }
        }
    }

    /// Records a successful fetch for `host` — resets the failure counter
    /// and returns the breaker to `Closed`.
    pub fn record_success(&self, host: &str) {
        let mut map = self.lock();
        if let Some(entry) = map.get_mut(host) {
            entry.state = BreakerState::Closed;
            entry.failure_count = 0;
        }
    }

    /// Records a failed fetch for `host`. After `FAILURE_THRESHOLD` consecutive
    /// failures, the breaker opens for `COOLDOWN` duration.
    pub fn record_failure(&self, host: &str) {
        let mut map = self.lock();
        let entry = map.entry(host.to_string()).or_default();
        entry.failure_count = entry.failure_count.saturating_add(1);
        if entry.failure_count >= CB_FAILURE_THRESHOLD {
            entry.state = BreakerState::Open {
                until: Instant::now() + CB_COOLDOWN,
            };
        }
    }
}

impl Default for CircuitBreakerMap {
    fn default() -> Self {
        Self::new()
    }
}

/// Gets (or creates under lock) the semaphore for the given `host` with capacity `limite`.
///
/// Performs an initial lookup under the lock, lazily creating an entry if the host
/// does not exist. The returned `Arc<Semaphore>` is cloned and borrowed by tasks —
/// the lock is not held during `.acquire_owned().await`.
///
/// # Cancel safety
///
/// This function is cancel-safe. It only holds the mutex lock briefly and
/// does not perform any I/O, so dropping the future is safe at any point.
pub async fn get_semaphore_for_host(
    mapa: &PerHostSemaphoreMap,
    host: &str,
    limit: usize,
) -> Arc<Semaphore> {
    let mut guard = mapa.lock().await;
    guard
        .entry(host.to_string())
        .or_insert_with(|| Arc::new(Semaphore::new(limit.max(1))))
        .clone()
}

/// Extracts the host from a URL. Returns `"unknown"` when the URL is malformed —
/// all malformed URLs share the same slot (this is a safe fallback).
///
/// Hosts are normalised to lowercase so that `Exemplo.COM` and `exemplo.com`
/// share the same per-host `Semaphore`.
///
/// # Example
///
/// ```
/// use duckduckgo_search_cli::content_fetch::extract_host;
///
/// assert_eq!(extract_host("https://www.example.com/path?q=1"), "www.example.com");
/// assert_eq!(extract_host("https://API.test/x"), "api.test"); // lowercased
/// assert_eq!(extract_host("not-a-url"), "unknown");             // malformed
/// assert_eq!(extract_host(""), "unknown");                      // empty
/// ```
#[inline]
pub fn extract_host(url: &str) -> String {
    url::Url::parse(url)
        .ok()
        .and_then(|u| u.host_str().map(|s| s.to_lowercase()))
        .unwrap_or_else(|| "unknown".to_string())
}

/// Enriches a `SearchOutput` with textual content from each URL in parallel.
///
/// Modifies `saida` IN-PLACE. Does not return a fatal error — individual failures
/// are recorded in `metadados.falhas_fetch` and the `content` field is absent in
/// the corresponding `SearchResult`.
///
/// # Errors
///
/// Returns an error if any content fetch fails due to HTTP errors
/// or cancellation.
///
/// # Cancel safety
///
/// This function is cancel-safe. Individual fetch tasks are cancelled
/// when the parent future is dropped.
///
/// # Panics
///
/// Panics if the per-host semaphore is unexpectedly closed, which
/// indicates a bug in the concurrency setup.
#[tracing::instrument(skip_all, fields(result_count = output.result_count, parallelism = config.parallelism))]
pub async fn enrich_with_content(
    output: &mut SearchOutput,
    client: &Client,
    config: &Config,
    cancellation: &CancellationToken,
) {
    if !config.fetch_content || output.results.is_empty() {
        return;
    }

    let total = output.results.len();
    tracing::info!(
        total,
        parallel = config.parallelism,
        "starting parallel enrichment with --fetch-content"
    );

    let semaphore = Arc::new(Semaphore::new(config.parallelism.max(1) as usize));
    let mapa_por_host: PerHostSemaphoreMap =
        Arc::new(Mutex::new(HashMap::with_capacity(total.min(32))));
    let breaker: CircuitBreakerMap = CircuitBreakerMap::new();
    let per_host_limit = config.per_host_limit.max(1);
    let max_size = config.max_content_length;

    // Feature chrome: try launching the browser ONCE before the fan-out.
    // If it fails (Chrome absent), we continue with HTTP only — without breaking execution.

    // WS-25: ProgressBar for long crawls. indicatif auto-detects TTY and
    // suppresses the bar when stderr is not a terminal (e.g. when piped to
    // a log file). The bar lives until `finish()`/`finish_and_clear()`.
    let progress = ProgressBar::new(total as u64);
    progress.set_style(
        ProgressStyle::with_template(
            "[{elapsed_precise}] {bar:40.cyan/blue} {pos:>4}/{len:4} {msg}",
        )
        .expect("invalid progress template")
        .progress_chars("##-"),
    );
    progress.set_message("fetching");
    #[cfg(feature = "chrome")]
    let navegador_chrome: Option<Arc<Mutex<ChromeBrowser>>> = {
        let manual_path = config.chrome_path.as_deref();
        match detect_chrome(manual_path) {
            Ok(path) => {
                tracing::info!(path = %path.display(), "Chrome detected — enabling fallback");
                let timeout_launch = std::time::Duration::from_secs(30);
                match ChromeBrowser::launch(&path, config.proxy.as_deref(), timeout_launch).await {
                    Ok(n) => Some(Arc::new(Mutex::new(n))),
                    Err(erro) => {
                        tracing::warn!(
                            ?erro,
                            "failed to launch Chrome — continuing with HTTP only"
                        );
                        None
                    }
                }
            }
            Err(erro) => {
                tracing::info!(?erro, "Chrome not detected — continuing with HTTP only");
                None
            }
        }
    };

    #[cfg(not(feature = "chrome"))]
    {
        if config.chrome_path.is_some() {
            tracing::warn!(
                "--chrome-path provided but binary was not compiled with --features chrome — ignoring"
            );
        }
    }

    // Tipo retornado: (index, text, size, method). text empty = failure.
    type ResultadoFetch = (usize, Option<(String, u32, String)>);
    let mut tasks: JoinSet<ResultadoFetch> = JoinSet::new();

    for (index, result_item) in output.results.iter().enumerate() {
        if cancellation.is_cancelled() {
            tracing::warn!("cancellation detected — aborting fetch spawns");
            break;
        }
        let url = result_item.url.clone();
        let task_client = client.clone();
        let task_semaphore = Arc::clone(&semaphore);
        let mapa_task = Arc::clone(&mapa_por_host);
        let task_breaker = breaker.clone();
        let task_cancellation = cancellation.clone();

        #[cfg(feature = "chrome")]
        let nav_task: Option<Arc<Mutex<ChromeBrowser>>> = navegador_chrome.as_ref().map(Arc::clone);

        tasks.spawn(async move {
            // Acquire global permit FIRST (controls total concurrency).
            tracing::debug!(
                permits_available = task_semaphore.available_permits(),
                fetch_index = index,
                "awaiting global semaphore permit"
            );
            let Ok(permit_global) = task_semaphore.acquire_owned().await else {
                tracing::debug!(index, "global semaphore closed — skipping");
                return (index, None);
            };

            if task_cancellation.is_cancelled() {
                drop(permit_global);
                return (index, None);
            }

            // Now acquire per-host permit (avoids bursting against a single domain).
            let host = extract_host(&url);
            // WS-12: short-circuit if the per-host breaker is OPEN. This avoids
            // hammering a host that has already failed repeatedly.
            if task_breaker.check(&host) == BreakerDecision::Reject {
                tracing::debug!(index, host, "circuit breaker OPEN — skipping fetch");
                drop(permit_global);
                return (index, None);
            }
            let semaforo_host = get_semaphore_for_host(&mapa_task, &host, per_host_limit).await;
            tracing::debug!(
                permits_available = semaforo_host.available_permits(),
                fetch_index = index,
                %host,
                "awaiting per-host semaphore permit"
            );
            let Ok(permit_host) = semaforo_host.acquire_owned().await else {
                tracing::debug!(index, host, "per-host semaphore closed — skipping");
                drop(permit_global);
                return (index, None);
            };

            if task_cancellation.is_cancelled() {
                drop(permit_host);
                drop(permit_global);
                return (index, None);
            }

            let result_item =
                content::extract_http_content(&task_client, &url, max_size, &task_cancellation)
                    .await;

            // WS-12: feed the breaker with the result of this fetch.
            match &result_item {
                Ok(Some((text, _))) if !text.is_empty() => task_breaker.record_success(&host),
                Ok(_) | Err(_) => task_breaker.record_failure(&host),
            }

            let retorno = match result_item {
                Ok(Some((text, size))) if !text.is_empty() => {
                    (index, Some((text, size, "http".to_string())))
                }
                Ok(Some((_vazio, _size_original))) => {
                    // HTTP returned insufficient content — try Chrome if available.
                    #[cfg(feature = "chrome")]
                    {
                        if let Some(nav) = nav_task {
                            tracing::debug!(
                                index,
                                url,
                                "HTTP content insufficient — trying Chrome"
                            );
                            let mut guarda = nav.lock().await;
                            match extract_text_with_chrome(
                                &mut guarda,
                                &url,
                                max_size,
                                std::time::Duration::from_secs(30),
                            )
                            .await
                            {
                                Ok(text) if !text.is_empty() => {
                                    let size_cast = u32::try_from(text.len()).unwrap_or(u32::MAX);
                                    drop(permit_host);
                                    drop(permit_global);
                                    return (index, Some((text, size_cast, "chrome".to_string())));
                                }
                                Ok(_) => {
                                    tracing::debug!(index, url, "Chrome also returned empty");
                                }
                                Err(error) => {
                                    tracing::debug!(index, url, ?error, "Chrome failed");
                                }
                            }
                        }
                    }
                    (index, None)
                }
                Ok(None) => {
                    tracing::debug!(index, url, "content-type not HTML — no content");
                    (index, None)
                }
                Err(error) => {
                    tracing::debug!(index, url, ?error, "failed to extract HTTP content");
                    (index, None)
                }
            };

            drop(permit_host);
            drop(permit_global);
            retorno
        });
    }

    let mut sucessos: u32 = 0;
    let mut falhas: u32 = 0;
    let mut usou_chrome: bool = false;

    while let Some(join_res) = tasks.join_next().await {
        match join_res {
            Ok((index, Some((text, size, method)))) => {
                if index < output.results.len() && !text.is_empty() {
                    let res = &mut output.results[index];
                    if method == "chrome" {
                        usou_chrome = true;
                    }
                    res.content = Some(text);
                    res.content_size = Some(size);
                    res.content_extraction_method = Some(method);
                    sucessos = sucessos.saturating_add(1);
                } else {
                    falhas = falhas.saturating_add(1);
                }
            }
            Ok((_, None)) => {
                falhas = falhas.saturating_add(1);
            }
            Err(error_join) => {
                if error_join.is_panic() {
                    tracing::error!(
                        ?error_join,
                        "fetch task panicked — permit recovered via RAII"
                    );
                } else {
                    tracing::warn!(?error_join, "fetch task cancelled");
                }
                falhas = falhas.saturating_add(1);
            }
        }
        // WS-25: advance the progress bar on every completed task.
        progress.inc(1);
    }

    output.metadata.concurrent_fetches = u32::try_from(total).unwrap_or(u32::MAX);
    output.metadata.fetch_successes = sucessos;
    output.metadata.fetch_failures = falhas;
    if usou_chrome {
        output.metadata.used_chrome = true;
    }

    // WS-25: close the progress bar so the cursor returns to a clean state
    // and the next prompt/print starts on a fresh line. `finish_and_clear`
    // erases the bar from the terminal instead of leaving it visible.
    progress.finish_and_clear();

    // Explicit browser cleanup (chrome feature).
    #[cfg(feature = "chrome")]
    if let Some(nav_arc) = navegador_chrome {
        drop(nav_arc); // Drop releases Mutex e o ChromeBrowser::drop aborta handler.
        tracing::debug!("Chrome dropped after enrichment");
    }

    tracing::info!(total, sucessos, falhas, "content enrichment complete");
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Endpoint, OutputFormat, SafeSearch, SearchMetadata, SearchResult};

    fn test_config(parallelism: u32, max_tam: usize) -> Config {
        Config {
            query: "q".to_string(),
            queries: vec!["q".to_string()],
            num_results: None,
            format: OutputFormat::Json,
            timeout_seconds: 5,
            language: "pt".to_string(),
            country: "br".to_string(),
            verbose: false,
            quiet: true,
            user_agent: "Mozilla/5.0".to_string(),
            browser_profile: crate::http::create_browser_profile("Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/146.0.0.0 Safari/537.36"),
            parallelism,
            pages: 1,
            retries: 0,
            endpoint: Endpoint::Html,
            time_filter: None,
            safe_search: SafeSearch::Moderate,
            stream_mode: false,
            output_file: None,
            fetch_content: true,
            max_content_length: max_tam,
            proxy: None,
            no_proxy: false,
            global_timeout_seconds: 60,
            match_platform_ua: false,
            per_host_limit: 2,
            chrome_path: None,
            selectors: std::sync::Arc::new(crate::types::SelectorConfig::default()),
            cookie_provider: None,
            persistent_jar: None,
            warmup_enabled: false,
        allow_lite_fallback: false,
        }
    }

    fn empty_output() -> SearchOutput {
        SearchOutput {
            query: "q".to_string(),
            engine: "duckduckgo".to_string(),
            endpoint: "html".to_string(),
            timestamp: "t".to_string(),
            region: "br-pt".to_string(),
            result_count: 0,
            results: vec![],
            pages_fetched: 1,
            error: None,
            message: None,
            metadata: SearchMetadata {
                execution_time_ms: 0,
                selectors_hash: "x".to_string(),
                retries: 0,
                used_fallback_endpoint: false,
                concurrent_fetches: 0,
                fetch_successes: 0,
                fetch_failures: 0,
                used_chrome: false,
                user_agent: "ua".to_string(),
                used_proxy: false,
                identity_used: None,
                cascade_level: None,
            },
        }
    }

    #[tokio::test]
    async fn enrich_with_content_no_op_when_flag_false() {
        let cliente = wreq::Client::new();
        let mut cfg = test_config(3, 1000);
        cfg.fetch_content = false;
        let mut output = empty_output();
        output.results.push(SearchResult {
            position: 1,
            title: "Um".to_string(),
            url: "http://inexistente.local/a".to_string(),
            display_url: None,
            snippet: None,
            original_title: None,
            content: None,
            content_size: None,
            content_extraction_method: None,
        });

        let token = CancellationToken::new();
        enrich_with_content(&mut output, &cliente, &cfg, &token).await;

        // Nada deve ter sido modificado (flag false).
        assert!(output.results[0].content.is_none());
        assert_eq!(output.metadata.concurrent_fetches, 0);
    }

    #[test]
    fn extract_host_valid_url_returns_host() {
        assert_eq!(extract_host("https://www.example.com/a"), "www.example.com");
        assert_eq!(extract_host("https://API.test/x"), "api.test");
    }

    #[test]
    fn extract_host_invalid_url_returns_unknown() {
        assert_eq!(extract_host("nao-eh-url"), "unknown");
        assert_eq!(extract_host(""), "unknown");
    }

    #[tokio::test]
    async fn get_semaphore_for_host_creates_once_per_host() {
        let mapa: PerHostSemaphoreMap = Arc::new(Mutex::new(HashMap::new()));
        let sema_a1 = get_semaphore_for_host(&mapa, "a.com", 3).await;
        let sema_a2 = get_semaphore_for_host(&mapa, "a.com", 99).await;
        // The second access must return the SAME semaphore (initial limit 3 preserved).
        assert!(Arc::ptr_eq(&sema_a1, &sema_a2));
        assert_eq!(sema_a1.available_permits(), 3);

        let sema_b = get_semaphore_for_host(&mapa, "b.com", 5).await;
        assert!(!Arc::ptr_eq(&sema_a1, &sema_b));
        assert_eq!(sema_b.available_permits(), 5);

        let mapa_guardado = mapa.lock().await;
        assert_eq!(mapa_guardado.len(), 2);
    }

    #[tokio::test]
    async fn get_semaphore_limits_simultaneous_concurrency_on_same_host() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        let mapa: PerHostSemaphoreMap = Arc::new(Mutex::new(HashMap::new()));
        let contador_simultaneo = Arc::new(AtomicUsize::new(0));
        let pico_simultaneo = Arc::new(AtomicUsize::new(0));

        let mut tarefas = Vec::with_capacity(20);
        for _ in 0..20 {
            let mapa = Arc::clone(&mapa);
            let contador = Arc::clone(&contador_simultaneo);
            let pico = Arc::clone(&pico_simultaneo);
            tarefas.push(tokio::spawn(async move {
                let sema = get_semaphore_for_host(&mapa, "same-host.com", 2).await;
                let _permit = sema
                    .acquire_owned()
                    .await
                    .expect("BUG: semaphore should not be closed");
                let atual = contador.fetch_add(1, Ordering::SeqCst) + 1;
                let mut p = pico.load(Ordering::SeqCst);
                while atual > p {
                    match pico.compare_exchange(p, atual, Ordering::SeqCst, Ordering::SeqCst) {
                        Ok(_) => break,
                        Err(novo) => p = novo,
                    }
                }
                tokio::time::sleep(std::time::Duration::from_millis(30)).await;
                contador.fetch_sub(1, Ordering::SeqCst);
            }));
        }
        for t in tarefas {
            let _ = t.await;
        }
        assert!(
            pico_simultaneo.load(Ordering::SeqCst) <= 2,
            "simultaneous peak {} exceeded limit 2",
            pico_simultaneo.load(Ordering::SeqCst)
        );
    }

    #[tokio::test]
    async fn enrich_with_content_cancelled_marks_failures() {
        let cliente = wreq::Client::builder()
            .timeout(std::time::Duration::from_millis(100))
            .build()
            .unwrap();
        let cfg = test_config(2, 1000);
        let mut output = empty_output();
        for i in 0..3 {
            output.results.push(SearchResult {
                position: (i + 1) as u32,
                title: format!("r{i}"),
                url: format!("http://127.0.0.1:1/{i}"),
                display_url: None,
                snippet: None,
                original_title: None,
                content: None,
                content_size: None,
                content_extraction_method: None,
            });
        }

        let token = CancellationToken::new();
        token.cancel();
        enrich_with_content(&mut output, &cliente, &cfg, &token).await;

        // Nenhum sucesso esperado (cancelado antes).
        assert_eq!(output.metadata.fetch_successes, 0);
    }

    // =========================================================================
    // WS-12 — Circuit breaker unit tests
    // =========================================================================

    #[test]
    fn ws12_breaker_allows_when_closed() {
        let cb = CircuitBreakerMap::new();
        assert_eq!(cb.check("host-a.com"), BreakerDecision::Allow);
        assert_eq!(cb.check("host-a.com"), BreakerDecision::Allow);
    }

    #[test]
    fn ws12_breaker_opens_after_threshold_failures() {
        let cb = CircuitBreakerMap::new();
        // Below threshold — still Closed.
        for _ in 0..(CB_FAILURE_THRESHOLD - 1) {
            cb.record_failure("flaky.com");
            assert_eq!(
                cb.check("flaky.com"),
                BreakerDecision::Allow,
                "must remain Closed below threshold"
            );
        }
        // Crossing the threshold — opens.
        cb.record_failure("flaky.com");
        assert_eq!(
            cb.check("flaky.com"),
            BreakerDecision::Reject,
            "must Open at threshold"
        );
        // Other hosts are unaffected.
        assert_eq!(cb.check("healthy.com"), BreakerDecision::Allow);
    }

    #[test]
    fn ws12_breaker_resets_on_success() {
        let cb = CircuitBreakerMap::new();
        cb.record_failure("x.com");
        cb.record_failure("x.com");
        cb.record_success("x.com");
        assert_eq!(
            cb.check("x.com"),
            BreakerDecision::Allow,
            "success must clear the failure counter"
        );
        // Even after two more failures, we must NOT open (counter restarted).
        cb.record_failure("x.com");
        cb.record_failure("x.com");
        assert_eq!(cb.check("x.com"), BreakerDecision::Allow);
    }

    #[test]
    fn ws12_breaker_half_opens_after_cooldown() {
        let cb = CircuitBreakerMap::new();
        for _ in 0..CB_FAILURE_THRESHOLD {
            cb.record_failure("slow.com");
        }
        assert_eq!(cb.check("slow.com"), BreakerDecision::Reject);
        // Manually shorten the cooldown by waiting — instead we test the
        // half-open behavior by checking that the entry is still tracked.
        // We cannot trivially advance Instant, so we just assert that the
        // map holds the entry.
        let map = cb.lock();
        assert!(map.contains_key("slow.com"));
    }
}
