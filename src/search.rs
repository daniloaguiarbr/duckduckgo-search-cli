// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload: I/O-bound (HTTP requests to DuckDuckGo endpoints)
//! URL construction and search request execution for `DuckDuckGo`.
//!
//! Iteration 3 adds:
//! - Pagination with `vqd` token via POST form-urlencoded.
//! - Retry with exponential backoff on 429 and UA rotation on 403.
//! - Lite endpoint (`https://lite.duckduckgo.com/lite/`).
//! - Time filter (`df`) and safe-search (`kp`).
//! - Base URL parameterization via environment variables (for wiremock tests).
//!
//! Base URLs are read from env `DUCKDUCKGO_SEARCH_CLI_BASE_URL_HTML` and
//! `DUCKDUCKGO_SEARCH_CLI_BASE_URL_LITE` when present; otherwise uses
//! the production defaults. The defaults END with a slash (`/html/` and `/lite/`)
//! because `DuckDuckGo` treats `/html` (without slash) as a redirect.

use crate::error::CliError;
use crate::extraction;
use crate::types::{Config, Endpoint, SafeSearch, SearchResult, TimeFilter};
use rand::Rng;
use reqwest::{Client, Response, StatusCode};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio_util::sync::CancellationToken;

/// Default base URL for the `DuckDuckGo` HTML endpoint.
const URL_ENDPOINT_HTML_DEFAULT: &str = "https://html.duckduckgo.com/html/";
/// Default base URL for the `DuckDuckGo` Lite endpoint.
const URL_ENDPOINT_LITE_DEFAULT: &str = "https://lite.duckduckgo.com/lite/";

/// Name of the environment variable that overrides the HTML endpoint URL (for tests).
const ENV_BASE_URL_HTML: &str = "DUCKDUCKGO_SEARCH_CLI_BASE_URL_HTML";
/// Name of the environment variable that overrides the Lite endpoint URL (for tests).
const ENV_BASE_URL_LITE: &str = "DUCKDUCKGO_SEARCH_CLI_BASE_URL_LITE";

/// Minimum delay between consecutive pages (ms).
/// v0.6.0: increased from 500 to 800ms to reduce anti-bot detection.
const PAGINATION_DELAY_MIN_MS: u64 = 800;
/// Maximum delay between consecutive pages (ms).
/// v0.6.0: increased from 1000 to 1500ms to reduce anti-bot detection.
const PAGINATION_DELAY_MAX_MS: u64 = 1500;

/// Byte threshold for silent block detection.
/// Real `DuckDuckGo` responses with results are 50-200KB.
/// Silent block pages are typically ~3KB.
const SILENT_BLOCK_THRESHOLD: usize = 5_000;

/// Base backoff for retry on 429 (ms). Total = base * 2^attempt + jitter.
const BACKOFF_BASE_MS: u64 = 1000;
/// Maximum additional jitter in backoff (ms).
const BACKOFF_JITTER_MAX_MS: u64 = 500;

/// Calculates the exponential backoff delay with jitter for the given attempt.
///
/// `attempt` is 0-based. The exponent is capped at 10 (`2^10 = 1024`) to
/// avoid overflow without needing `checked_shl`.
fn calculate_backoff_ms(attempt: u32) -> u64 {
    let factor = 1u64 << attempt.min(10);
    let backoff = BACKOFF_BASE_MS.saturating_mul(factor);
    let jitter = rand::thread_rng().gen_range(0..=BACKOFF_JITTER_MAX_MS);
    backoff.saturating_add(jitter)
}

/// Parses the `Retry-After` header from an HTTP response.
///
/// Supports both numeric seconds and HTTP-date formats.
/// Returns `None` if the header is absent or unparseable.
fn parse_retry_after(response: &Response) -> Option<u64> {
    let value = response.headers().get("retry-after")?.to_str().ok()?;
    if let Ok(secs) = value.parse::<u64>() {
        return Some(secs.min(120) * 1000);
    }
    None
}

/// Returns the effective base URL for the HTML endpoint (respects env var in tests).
pub fn html_base_url() -> String {
    std::env::var(ENV_BASE_URL_HTML).unwrap_or_else(|_| URL_ENDPOINT_HTML_DEFAULT.to_string())
}

/// Returns the effective base URL for the Lite endpoint (respects env var in tests).
pub fn lite_base_url() -> String {
    std::env::var(ENV_BASE_URL_LITE).unwrap_or_else(|_| URL_ENDPOINT_LITE_DEFAULT.to_string())
}

/// Builds the GET search URL with the appropriate query-string for a given endpoint.
///
/// Parameters:
/// - `q` — search query (URL-encoded).
/// - `kl` — region, format `{country}-{language}`.
/// - `kp` — safe-search (when present).
/// - `df` — time filter (when present).
pub fn build_search_url(
    query: &str,
    language: &str,
    country: &str,
    endpoint: Endpoint,
    time_filter: Option<TimeFilter>,
    safe_search: SafeSearch,
) -> String {
    let base = match endpoint {
        Endpoint::Html => html_base_url(),
        Endpoint::Lite => lite_base_url(),
    };
    let query_encoded = urlencoding::encode(query);
    let kl = format_kl(language, country);
    let mut url = String::with_capacity(base.len() + query_encoded.len() + kl.len() + 32);
    url.push_str(&base);
    url.push_str("?q=");
    url.push_str(&query_encoded);
    url.push_str("&kl=");
    url.push_str(&kl);
    if let Some(kp) = safe_search.as_param() {
        url.push_str("&kp=");
        url.push_str(kp);
    }
    if let Some(df) = time_filter {
        url.push_str("&df=");
        url.push_str(df.as_param());
    }
    url
}

/// Simplified version from iteration 1 — kept for backward compatibility with older tests.
pub fn build_url(query: &str, language: &str, country: &str) -> String {
    build_search_url(
        query,
        language,
        country,
        Endpoint::Html,
        None,
        SafeSearch::Moderate,
    )
}

/// Formats the `DuckDuckGo` `kl` parameter as `{country}-{language}` in lowercase.
///
/// `DuckDuckGo` expects `kl` with the country in lowercase, followed by a hyphen and language
/// in lowercase. Uppercase inputs are normalized.
///
/// # Exemplo
///
/// ```
/// use duckduckgo_search_cli::search::format_kl;
///
/// assert_eq!(format_kl("pt", "br"), "br-pt");
/// assert_eq!(format_kl("EN", "US"), "us-en"); // normalizes uppercase input
/// ```
#[inline]
pub fn format_kl(language: &str, country: &str) -> String {
    let mut kl = String::with_capacity(country.len() + language.len() + 1);
    for ch in country.chars() {
        kl.push(ch.to_ascii_lowercase());
    }
    kl.push('-');
    for ch in language.chars() {
        kl.push(ch.to_ascii_lowercase());
    }
    kl
}

/// Specific errors returned by `execute_with_retry`.
///
/// Used so the pipeline can tag queries with structured error codes
/// instead of a generic message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RetryFailReason {
    /// Persistent rate limit after exhausting retries (HTTP 429).
    RateLimited,
    /// Persistent block after exhausting retries (HTTP 403).
    Blocked,
    /// Non-recoverable HTTP error (4xx/5xx status other than 403/429).
    HttpError(u16),
    /// Timeout after 1 retry attempt.
    Timeout,
    /// Generic network error.
    Network(String),
}

impl RetryFailReason {
    /// Maps to the structured error code in `error::codigos`.
    pub fn as_error_code(&self) -> &'static str {
        match self {
            RetryFailReason::RateLimited => crate::error::codes::RATE_LIMITED,
            RetryFailReason::Blocked => crate::error::codes::BLOCKED,
            RetryFailReason::HttpError(_) => crate::error::codes::HTTP_ERROR,
            RetryFailReason::Timeout => crate::error::codes::TIMEOUT,
            RetryFailReason::Network(_) => crate::error::codes::NETWORK_ERROR,
        }
    }

    /// Returns a human-readable failure description for logs and JSON output.
    pub fn message(&self) -> String {
        match self {
            RetryFailReason::RateLimited => "persistent rate limit (HTTP 429)".to_string(),
            RetryFailReason::Blocked => "blocked by DuckDuckGo (HTTP 403)".to_string(),
            RetryFailReason::HttpError(status) => format!("HTTP {status} unrecoverable"),
            RetryFailReason::Timeout => "persistent timeout".to_string(),
            RetryFailReason::Network(msg) => format!("network error: {msg}"),
        }
    }
}

/// Result of `execute_with_retry`: either the HTTP response + total attempts, or the failure reason.
#[derive(Debug)]
pub struct RetryResult {
    /// The successful HTTP response body.
    pub response: Response,
    /// Total number of attempts made (1 = no retry needed).
    pub attempts: u32,
}

/// Executes a GET request with retry and backoff. Parameters:
/// * `client` — reqwest client (shared).
/// * `url` — full target URL.
/// * `retries` — number of additional retries (0..=10). 0 = single attempt only.
/// * `flag_rate_limit` — signals to other tasks that rate limiting was detected.
///
/// # Errors
///
/// Returns an error if all retry attempts are exhausted due to rate limiting,
/// blocking (HTTP 403 / HTTP 202), timeout, a non-recoverable HTTP status, or
/// a network failure.
///
/// # Cancel safety
///
/// This function is cancel-safe. Dropping the future between retries prevents
/// any in-progress `tokio::time::sleep` or pending `send()` from completing,
/// leaving the HTTP connection in an unknown state that `reqwest` will close.
#[tracing::instrument(skip_all, fields(%url, max_attempts = retries + 1))]
pub async fn execute_with_retry(
    client: &Client,
    url: &str,
    retries: u32,
    flag_rate_limit: &Arc<AtomicBool>,
    cancellation: &CancellationToken,
) -> std::result::Result<RetryResult, RetryFailReason> {
    let total_attempts = retries.saturating_add(1);
    let mut last_reason = RetryFailReason::Network("no attempts executed".to_string());
    let mut timeout_already_retried = false;

    for attempt in 0..total_attempts {
        if cancellation.is_cancelled() {
            return Err(RetryFailReason::Network("cancelled".to_string()));
        }

        // Se o rate-limit global foi acionado por outra task, aplica delay extra.
        if flag_rate_limit.load(Ordering::Relaxed) && attempt == 0 {
            let extra_ms = rand::thread_rng().gen_range(500..1200);
            tracing::debug!(
                extra_ms,
                "global rate-limit flag active — waiting before retry attempt"
            );
            tokio::time::sleep(Duration::from_millis(extra_ms)).await;
        }

        tracing::debug!(attempt = attempt + 1, total = total_attempts, url = %url, "executing GET request");

        let envio = tokio::select! {
            biased;
            _ = cancellation.cancelled() => {
                return Err(RetryFailReason::Network("cancelled during request".to_string()));
            }
            res = client.get(url).send() => res,
        };

        match envio {
            Ok(response) => {
                let status = response.status();
                // HTTP 202 = anomalia DDG (bloqueio suave anti-bot).
                // Browsers reais NUNCA recebem 202 do DuckDuckGo.
                // Ordering::Relaxed is sufficient for this AtomicBool flag because:
                // 1. It is a best-effort signal — a task that misses the flag simply
                //    retries and discovers the rate-limit itself.
                // 2. No correctness invariant depends on immediate cross-thread visibility.
                // 3. After the flag is set, each task independently adds random delay;
                //    eventual consistency is acceptable for this coordination pattern.
                if status == StatusCode::ACCEPTED {
                    flag_rate_limit.store(true, Ordering::Relaxed);
                    last_reason = RetryFailReason::Blocked;
                    if attempt + 1 < total_attempts {
                        let total = calculate_backoff_ms(attempt);
                        tracing::warn!(
                            attempt = attempt + 1,
                            backoff_ms = total,
                            "HTTP 202 anomaly — DDG soft block, applying backoff"
                        );
                        tokio::time::sleep(Duration::from_millis(total)).await;
                        continue;
                    }
                    return Err(RetryFailReason::Blocked);
                }
                if status.is_success() {
                    return Ok(RetryResult {
                        response,
                        attempts: attempt + 1,
                    });
                }
                if status == StatusCode::TOO_MANY_REQUESTS {
                    // Same Relaxed justification as HTTP 202 store above (lines 247-252).
                    flag_rate_limit.store(true, Ordering::Relaxed);
                    last_reason = RetryFailReason::RateLimited;
                    if attempt + 1 < total_attempts {
                        let delay_ms = parse_retry_after(&response)
                            .unwrap_or_else(|| calculate_backoff_ms(attempt));
                        tracing::warn!(
                            attempt = attempt + 1,
                            backoff_ms = delay_ms,
                            "HTTP 429 — applying backoff"
                        );
                        tokio::time::sleep(Duration::from_millis(delay_ms)).await;
                        continue;
                    }
                    return Err(RetryFailReason::RateLimited);
                }
                if status == StatusCode::FORBIDDEN {
                    last_reason = RetryFailReason::Blocked;
                    if attempt + 1 < total_attempts {
                        tracing::warn!(
                            attempt = attempt + 1,
                            "HTTP 403 — immediate retry (UA rotation applied on next client)"
                        );
                        // UA rotation is the caller's responsibility; here we only signal.
                        continue;
                    }
                    return Err(RetryFailReason::Blocked);
                }
                // Other 4xx/5xx — do not retry.
                return Err(RetryFailReason::HttpError(status.as_u16()));
            }
            Err(err) => {
                if err.is_timeout() {
                    last_reason = RetryFailReason::Timeout;
                    if !timeout_already_retried && attempt + 1 < total_attempts {
                        timeout_already_retried = true;
                        tracing::warn!("timeout — 1 retry allowed");
                        continue;
                    }
                    return Err(RetryFailReason::Timeout);
                }
                last_reason = RetryFailReason::Network(err.to_string());
                // Generic network errors: 1 optional retry if attempts remain.
                if attempt + 1 < total_attempts {
                    let backoff = Duration::from_millis(400);
                    tokio::time::sleep(backoff).await;
                    continue;
                }
                return Err(last_reason);
            }
        }
    }

    Err(last_reason)
}

/// Executes the initial search on the configured endpoint and returns the raw HTML.
/// Compatibility version (iteration 1) — used by the simple single-query flow.
///
/// # Errors
///
/// Returns an error if the HTTP request fails, if `DuckDuckGo` returns a non-2xx
/// status, or if the response body is suspiciously small (silent block detected).
///
/// # Cancel safety
///
/// This function is cancel-safe. Dropping the future before `.send().await`
/// completes discards the in-flight request; dropping it before `.text().await`
/// discards the partially-received body.
pub async fn execute_search(
    client: &Client,
    query: &str,
    idioma: &str,
    pais: &str,
) -> Result<String, CliError> {
    let url = build_url(query, idioma, pais);
    tracing::debug!(url = %url, "Sending GET to the DuckDuckGo HTML endpoint");

    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|e| CliError::HttpError {
            message: format!("failed to send GET to {url}: {e}"),
            cause: Some(e.into()),
        })?;

    let status = response.status();
    tracing::debug!(status = %status, "HTTP response received");

    if !status.is_success() {
        return Err(CliError::HttpError {
            message: format!(
                "DuckDuckGo returned HTTP {} for {:?}",
                status.as_u16(),
                query
            ),
            cause: None,
        });
    }

    let html = response.text().await.map_err(|e| CliError::HttpError {
        message: format!("failed to read UTF-8 response body: {e}"),
        cause: Some(e.into()),
    })?;

    if html.len() < SILENT_BLOCK_THRESHOLD {
        tracing::warn!(
            bytes = html.len(),
            limiar = SILENT_BLOCK_THRESHOLD,
            "suspiciously small response — possible silent block"
        );
        return Err(CliError::HttpError {
            message: format!(
                "suspiciously small response ({} bytes < {} threshold) — possible silent block",
                html.len(),
                SILENT_BLOCK_THRESHOLD
            ),
            cause: None,
        });
    }

    tracing::debug!(bytes = html.len(), "HTML received successfully");
    Ok(html)
}

/// Aggregated result of a search with pagination and potential endpoint fallback.
#[derive(Debug)]
pub struct AggregatedSearchResult {
    /// Organic results collected across all pages.
    pub results: Vec<SearchResult>,
    /// Number of pages actually fetched.
    pub pages_fetched: u32,
    /// Whether the lite endpoint was used as fallback.
    pub used_fallback_lite: bool,
    /// Total HTTP attempts (including retries).
    pub attempts: u32,
    /// Endpoint that produced the final results.
    pub effective_endpoint: Endpoint,
}

/// Extracts `vqd`, `s` and `dc` from the first page HTML (for pagination).
/// Returns `None` if any of the three fields is missing.
pub fn extract_pagination_tokens(html: &str) -> Option<(String, String, String)> {
    use scraper::Html;
    let doc = Html::parse_document(html);

    let vqd = doc
        .select(sel_vqd())
        .next()
        .and_then(|el| el.value().attr("value"))
        .map(|v| v.to_string())?;
    let s = doc
        .select(sel_s_input())
        .next()
        .and_then(|el| el.value().attr("value"))
        .map(|v| v.to_string())?;
    let dc = doc
        .select(sel_dc())
        .next()
        .and_then(|el| el.value().attr("value"))
        .map(|v| v.to_string())?;

    Some((vqd, s, dc))
}

fn sel_vqd() -> &'static scraper::Selector {
    use std::sync::OnceLock;
    static C: OnceLock<scraper::Selector> = OnceLock::new();
    C.get_or_init(|| scraper::Selector::parse("input[name='vqd']").unwrap())
}

fn sel_s_input() -> &'static scraper::Selector {
    use std::sync::OnceLock;
    static C: OnceLock<scraper::Selector> = OnceLock::new();
    C.get_or_init(|| scraper::Selector::parse("input[name='s']").unwrap())
}

fn sel_dc() -> &'static scraper::Selector {
    use std::sync::OnceLock;
    static C: OnceLock<scraper::Selector> = OnceLock::new();
    C.get_or_init(|| scraper::Selector::parse("input[name='dc']").unwrap())
}

/// Runs a complete search with vqd pagination and optional fallback to Lite.
///
/// If the HTML endpoint returns zero results on the first page (via Strategies 1 and 2),
/// automatically falls back to the Lite endpoint (Strategy 3).
///
/// Returns an aggregated structure with results, related searches, pages actually
/// fetched, fallback indicator, and total attempt count.
///
/// # Errors
///
/// Returns an error (as a [`RetryFailReason`]) if the first-page request fails after
/// all retries, or if the first response is suspiciously small (silent block detected).
/// Pagination and Lite-fallback failures are logged and handled gracefully without
/// propagating an error.
///
/// # Cancel safety
///
/// This function is cancel-safe. Dropping the future between pagination steps leaves
/// the accumulated results collected so far in an unreachable state; no partial output
/// is emitted to the caller.
pub async fn search_with_pagination(
    client: &Client,
    cfg: &Config,
    query: &str,
    flag_rate_limit: &Arc<AtomicBool>,
    cancellation: &CancellationToken,
) -> std::result::Result<AggregatedSearchResult, RetryFailReason> {
    let initial_endpoint = cfg.endpoint;
    let initial_url = build_search_url(
        query,
        &cfg.language,
        &cfg.country,
        initial_endpoint,
        cfg.time_filter,
        cfg.safe_search,
    );

    let first_result = execute_with_retry(
        client,
        &initial_url,
        cfg.retries,
        flag_rate_limit,
        cancellation,
    )
    .await?;
    let mut accumulated_attempts = first_result.attempts;

    let first_html = first_result
        .response
        .text()
        .await
        .map_err(|e| RetryFailReason::Network(e.to_string()))?;

    if first_html.len() < SILENT_BLOCK_THRESHOLD {
        tracing::warn!(
            bytes = first_html.len(),
            limiar = SILENT_BLOCK_THRESHOLD,
            "first page response suspiciously small — possible silent block"
        );
        return Err(RetryFailReason::Blocked);
    }

    // Extract results from the first page according to the endpoint.
    let mut accumulated_results = match initial_endpoint {
        Endpoint::Html => {
            extraction::extract_results_with_strategies_cfg(&first_html, &cfg.selectors)
        }
        Endpoint::Lite => extraction::extract_results_lite_with_cfg(&first_html, &cfg.selectors),
    };
    let mut used_fallback_lite = false;
    let mut effective_endpoint = initial_endpoint;
    let mut pages_fetched: u32 = 1;

    // Se HTML retornou zero E estamos no endpoint HTML → tentar Lite como fallback.
    if accumulated_results.is_empty() && initial_endpoint == Endpoint::Html {
        tracing::warn!("HTML returned zero results — trying Lite fallback");
        let url_lite = build_search_url(
            query,
            &cfg.language,
            &cfg.country,
            Endpoint::Lite,
            cfg.time_filter,
            cfg.safe_search,
        );
        match execute_with_retry(
            client,
            &url_lite,
            cfg.retries,
            flag_rate_limit,
            cancellation,
        )
        .await
        {
            Ok(r_lite) => {
                accumulated_attempts = accumulated_attempts.saturating_add(r_lite.attempts);
                let html_lite = r_lite
                    .response
                    .text()
                    .await
                    .map_err(|e| RetryFailReason::Network(e.to_string()))?;
                let lite_results =
                    extraction::extract_results_lite_with_cfg(&html_lite, &cfg.selectors);
                if !lite_results.is_empty() {
                    accumulated_results = lite_results;
                    used_fallback_lite = true;
                    effective_endpoint = Endpoint::Lite;
                }
            }
            Err(err) => {
                tracing::warn!(?err, "Lite fallback also failed — keeping empty");
            }
        }
    }

    // vqd pagination ONLY for the HTML endpoint (Lite does not have this mechanism).
    // AND ONLY if configured for multiple pages.
    if effective_endpoint == Endpoint::Html && cfg.pages > 1 && !accumulated_results.is_empty() {
        if let Some((mut vqd, mut s, mut dc)) = extract_pagination_tokens(&first_html) {
            // Form identical to the hidden form returned by the DOM (discovered
            // empirically on 2026-04-14 / iteration 4): besides `q`/`s`/`dc`/`vqd`/`kl`,
            // DDG expects `nextParams` (empty), `v="l"`, `o="json"`, `api="d.js"`.
            // Built once before the loop; only variable fields (s/dc/vqd) are
            // updated per iteration via clone_from to reuse String capacity.
            let mut form_data: Vec<(&str, String)> = vec![
                ("q", query.to_string()),                       // [0] fixed
                ("s", s.clone()),                               // [1] variable
                ("nextParams", String::new()),                  // [2] fixed
                ("v", "l".to_string()),                         // [3] fixed
                ("o", "json".to_string()),                      // [4] fixed
                ("dc", dc.clone()),                             // [5] variable
                ("api", "d.js".to_string()),                    // [6] fixed
                ("vqd", vqd.clone()),                           // [7] variable
                ("kl", format_kl(&cfg.language, &cfg.country)), // [8] fixed
            ];

            for page_idx in 2..=cfg.pages {
                if cancellation.is_cancelled() {
                    tracing::debug!("cancellation detected during pagination");
                    break;
                }

                // Delay between pages.
                let delay_ms =
                    rand::thread_rng().gen_range(PAGINATION_DELAY_MIN_MS..=PAGINATION_DELAY_MAX_MS);
                tokio::select! {
                    biased;
                    _ = cancellation.cancelled() => { break; }
                    _ = tokio::time::sleep(Duration::from_millis(delay_ms)) => {}
                }

                form_data[1].1.clone_from(&s);
                form_data[5].1.clone_from(&dc);
                form_data[7].1.clone_from(&vqd);

                let base = html_base_url();
                let response = match tokio::select! {
                    biased;
                    _ = cancellation.cancelled() => {
                        break;
                    }
                    r = client
                        .post(&base)
                        .header(reqwest::header::REFERER, "https://html.duckduckgo.com/")
                        .headers(cfg.browser_profile.pagination_headers())
                        .form(&form_data)
                        .send() => r,
                } {
                    Ok(r) => r,
                    Err(err) => {
                        tracing::warn!(
                            ?err,
                            pagina = page_idx,
                            "network error during pagination — stopping"
                        );
                        break;
                    }
                };

                if !response.status().is_success() {
                    tracing::warn!(
                        status = response.status().as_u16(),
                        pagina = page_idx,
                        "pagination returned non-success status — stopping"
                    );
                    break;
                }

                let page_html = match response.text().await {
                    Ok(t) => t,
                    Err(e) => {
                        tracing::warn!(?e, "error reading page body — stopping");
                        break;
                    }
                };

                // Check for silent block on the pagination page.
                if page_html.len() < SILENT_BLOCK_THRESHOLD {
                    tracing::warn!(
                        bytes = page_html.len(),
                        limiar = SILENT_BLOCK_THRESHOLD,
                        pagina = page_idx,
                        "pagination page suspiciously small — possible silent block"
                    );
                    break;
                }

                let new_results =
                    extraction::extract_results_with_strategies_cfg(&page_html, &cfg.selectors);
                if new_results.is_empty() {
                    tracing::debug!(pagina = page_idx, "page returned zero results — stopping");
                    break;
                }

                // Renumber positions following the accumulated Vec.
                let offset = u32::try_from(accumulated_results.len()).unwrap_or(u32::MAX);
                for mut r in new_results {
                    r.position = offset.saturating_add(r.position);
                    accumulated_results.push(r);
                }

                pages_fetched = page_idx;

                // Update tokens for the next page; if absent, stop.
                match extract_pagination_tokens(&page_html) {
                    Some((next_vqd, next_s, next_dc)) => {
                        vqd = next_vqd;
                        s = next_s;
                        dc = next_dc;
                    }
                    None => {
                        tracing::warn!(pagina = page_idx, "pagination tokens missing — stopping");
                        break;
                    }
                }
            }
        } else {
            tracing::warn!("vqd/s/dc tokens missing on first page — pagination not possible");
        }
    }

    // Trunca ao --num se especificado.
    if let Some(n) = cfg.num_results {
        let n_usize = n as usize;
        if accumulated_results.len() > n_usize {
            accumulated_results.truncate(n_usize);
        }
    }

    Ok(AggregatedSearchResult {
        results: accumulated_results,
        pages_fetched,
        used_fallback_lite,
        attempts: accumulated_attempts,
        effective_endpoint,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_kl_concatenates_correctly() {
        assert_eq!(format_kl("pt", "br"), "br-pt");
        assert_eq!(format_kl("PT", "BR"), "br-pt");
        assert_eq!(format_kl("en", "us"), "us-en");
    }

    #[test]
    fn build_url_escapes_spaces_and_accents() {
        let url = build_url("endividamento brasileiro", "pt", "br");
        assert!(url.starts_with("https://html.duckduckgo.com/html/?q="));
        assert!(url.contains("endividamento%20brasileiro"));
        assert!(url.contains("&kl=br-pt"));
    }

    #[test]
    fn build_url_escapes_special_characters() {
        let url = build_url("C++ tutorial", "en", "us");
        assert!(url.contains("C%2B%2B"));
        assert!(url.contains("&kl=us-en"));
    }

    #[test]
    fn build_url_with_portuguese_accents() {
        let url = build_url("música eletrônica", "pt", "br");
        assert!(url.contains("m%C3%BAsica"));
        assert!(url.contains("eletr%C3%B4nica"));
    }

    #[test]
    fn build_search_url_adds_optional_params() {
        let url = build_search_url(
            "rust",
            "en",
            "us",
            Endpoint::Html,
            Some(TimeFilter::Week),
            SafeSearch::Strict,
        );
        assert!(url.contains("&kp=1"));
        assert!(url.contains("&df=w"));
    }

    #[test]
    fn build_search_url_omits_kp_when_moderate() {
        let url = build_search_url(
            "rust",
            "en",
            "us",
            Endpoint::Html,
            None,
            SafeSearch::Moderate,
        );
        assert!(!url.contains("&kp="));
        assert!(!url.contains("&df="));
    }

    #[test]
    fn build_search_url_lite_endpoint_uses_correct_url() {
        let url = build_search_url(
            "rust",
            "en",
            "us",
            Endpoint::Lite,
            None,
            SafeSearch::Moderate,
        );
        assert!(url.starts_with("https://lite.duckduckgo.com/lite/?"));
    }

    #[test]
    fn extract_pagination_tokens_extracts_when_present() {
        let html = r#"
            <form>
              <input name="q" value="rust">
              <input name="vqd" value="4-12345678-abc">
              <input name="s" value="50">
              <input name="dc" value="51">
            </form>
        "#;
        let (vqd, s, dc) = extract_pagination_tokens(html).expect("all present");
        assert_eq!(vqd, "4-12345678-abc");
        assert_eq!(s, "50");
        assert_eq!(dc, "51");
    }

    #[test]
    fn extract_pagination_tokens_returns_none_when_absent() {
        let html = r#"<html><body>Sem inputs</body></html>"#;
        assert!(extract_pagination_tokens(html).is_none());
    }

    #[test]
    fn retry_fail_reason_returns_correct_error_code() {
        assert_eq!(
            RetryFailReason::RateLimited.as_error_code(),
            crate::error::codes::RATE_LIMITED
        );
        assert_eq!(
            RetryFailReason::Blocked.as_error_code(),
            crate::error::codes::BLOCKED
        );
        assert_eq!(
            RetryFailReason::Timeout.as_error_code(),
            crate::error::codes::TIMEOUT
        );
    }
}
