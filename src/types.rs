// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload: declarative (shared data types and serde configuration)
//! Shared data types used across the application.
//!
//! Output structs (`SearchOutput`, `MultiSearchOutput`, `SearchResult`,
//! `SearchMetadata`) serialize with JSON field names preserved via
//! `#[serde(rename = "...")]` for backward compatibility.

use crate::cli::CliIdentityProfile;
use crate::http::BrowserProfile;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Causa classificada de um zero-result no envelope JSON.
///
/// v0.8.0: distingue zero legítimo, filtro silencioso do DDG, ghost-block
/// do Cloudflare (HTTP 200 sub-4KB sem markers), anti-bot explícito, e
/// resposta inválida ou truncada. Marcado como `#[non_exhaustive]` para
/// permitir variantes futuras sem breaking change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum ZeroCause {
    /// Query genuinamente sem resultados no índice do DDG.
    Legitimo,
    /// DDG dropou a query silenciosamente sem interstitial detectável.
    FiltroSilencioso,
    /// Cloudflare serviu HTTP 200 com body sub-4KB sem markers literais.
    GhostBlock,
    /// Anti-bot explícito (HTTP 202, 403 persistente, interstitial CF/DDG).
    AntiBot,
    /// Resposta inválida ou truncada (body vazio, JSON malformado, proxy intercept).
    RespostaInvalida,
    /// Body descomprimido na faixa suspeita (5-15KB) sem result-page signal
    /// e sem interstitial literal. Indica provavel bloqueio upstream pelo
    /// HTTP client (rustls fingerprint TLS divergente do browser real) onde
    /// o DDG serve SERP vazia proposital sem challenge explicito. Distinto
    /// de `Legitimo` porque o body nao tem marcadores de result page. v0.8.0
    /// audit E2E 2026-06-19.
    ZeroResultsSuspeito,
}

/// Represents a single `DuckDuckGo` search result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    /// Result position on the page (1-indexed, already after ad filtering).
    #[serde(rename = "posicao")]
    pub position: u32,

    /// Result title, extracted from the `.result__a` element.
    #[serde(rename = "titulo")]
    pub title: String,

    /// Result URL, extracted from the `href` attribute of `.result__a`.
    pub url: String,

    /// Display URL (more user-friendly), extracted from `.result__url`.
    #[serde(rename = "url_exibicao")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_url: Option<String>,

    /// Descriptive snippet for the result, extracted from `.result__snippet`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub snippet: Option<String>,

    /// Literal title text as rendered by `DuckDuckGo`, preserved for auditing
    /// when substitution heuristics are applied (e.g., DDG returns "Official site"
    /// for verified domains — we replace it with `display_url` and keep the
    /// original here). Absent when the title was not modified.
    #[serde(rename = "titulo_original")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub original_title: Option<String>,

    /// Full text content of the page (only with `--fetch-content`; not implemented in the MVP).
    #[serde(rename = "conteudo")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,

    /// Size in characters of the extracted content (only with `--fetch-content`).
    #[serde(rename = "tamanho_conteudo")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_size: Option<u32>,

    /// Method used to extract content: `"http"` or `"chrome"` (only with `--fetch-content`).
    #[serde(rename = "metodo_extracao_conteudo")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_extraction_method: Option<String>,
}

/// Search execution metadata, useful for diagnostics and LLM integration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchMetadata {
    /// Total execution time in milliseconds.
    #[serde(rename = "tempo_execucao_ms")]
    pub execution_time_ms: u64,

    /// Blake3 hash (hex, first 16 characters) of the selector configuration used.
    #[serde(rename = "hash_seletores")]
    pub selectors_hash: String,

    /// Number of retries performed (0 in MVP — retry not yet implemented).
    /// Number of retries actually executed by the pipeline (excludes the
    /// first attempt). `0` indicates the initial request succeeded without
    /// any retry. GAP-AUD-007 v0.8.0: renamed from `retries` and added
    /// `retries_configured` to disambiguate configured-vs-executed.
    #[serde(rename = "retentativas_executadas")]
    pub retries: u32,

    /// Number of retries that the operator configured via `--retries N`.
    /// Distinguishes between "0 retries ran because the first try worked"
    /// and "0 retries ran because none was requested". `None` when the
    /// operator did not override the default. v0.8.0 GAP-AUD-007.
    #[serde(rename = "retentativas_configuradas")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retries_configured: Option<u32>,

    /// Indicates whether the Lite endpoint was used as fallback (always `false` in MVP).
    #[serde(rename = "usou_endpoint_fallback")]
    pub used_fallback_endpoint: bool,

    /// Number of parallel content fetches started (0 in MVP).
    #[serde(rename = "fetches_simultaneos")]
    pub concurrent_fetches: u32,

    /// Successful content fetches (0 in MVP).
    #[serde(rename = "sucessos_fetch")]
    pub fetch_successes: u32,

    /// Failed content fetches (0 in MVP).
    #[serde(rename = "falhas_fetch")]
    pub fetch_failures: u32,

    /// Indicates whether Chrome was used (always `false` in MVP).
    #[serde(rename = "usou_chrome")]
    pub used_chrome: bool,

    /// Indicates whether Chrome-primary search was attempted.
    /// `true` when the `chrome` feature is enabled and the pipeline
    /// tried the Chrome path (regardless of success or failure).
    #[serde(rename = "tentou_chrome")]
    pub chrome_attempted: bool,

    /// User-Agent used during execution.
    pub user_agent: String,

    /// Identity tag actually used for the request (WS-26).
    ///
    /// Format: `<family>-<platform>-<16hex>`. This field is additive — when
    /// the WS-26 identity rotation is disabled (default in v0.6.4) it
    /// contains a synthetic tag derived from the static UA. When rotation
    /// is active, the tag reports the identity that was used for the
    /// successful response (or the last attempt on failure).
    #[serde(rename = "identidade_usada")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub identity_used: Option<String>,

    /// Cascade level reached during the request (0..=4). `None` when the
    /// identity rotation was not active. See `IdentityPool::rotate_on_block`.
    #[serde(rename = "nivel_cascata")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cascade_level: Option<u32>,

    /// Indicates whether a proxy was configured (always `false` in MVP).
    #[serde(rename = "usou_proxy")]
    pub used_proxy: bool,

    /// Indicates whether the pre-flight ghost-block detection was triggered.
    /// `true` when `--pre-flight` is active AND a sub-4KB body with no
    /// result-page signal was classified as `Cloudflare`. v0.7.10.
    #[serde(rename = "pre_flight_disparado")]
    pub pre_flight_fired: bool,

    /// Causa classificada do zero-result quando `result_count == 0`.
    ///
    /// `None` quando o classificador não rodou ou busca retornou resultados.
    /// Auto-preenchido pelo classificador causal em `pipeline::classify_zero_result`.
    /// v0.8.0 — fecha GAP-AUD-003.
    #[serde(rename = "causa_zero")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub zero_cause: Option<ZeroCause>,
    /// Sugestão acionável de próxima ação quando .
    ///
    /// String fixa por variante de  (sem campo  separado).
    ///  quando o classificador não rodou ou busca retornou resultados.
    /// v0.8.0.
    #[serde(rename = "sugestao_proxima_acao")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sugestao_proxima_acao: Option<String>,

    /// Bytes brutos recebidos do DDG antes da descompressão.
    ///
    ///  quando a busca não chegou a executar (erro de config, sub-4KB
    /// body sem response, ou telemetria indisponível). GAP-NEW-002 v0.8.0.
    /// Permite ao operador distinguir entre body vazio e shell de 14KB
    /// (stealth block do Cloudflare) sem precisar de build debug.
    #[serde(rename = "bytes_brutos")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bytes_raw: Option<u64>,

    /// Bytes após descompressão gzip/deflate/br.
    ///
    ///  quando descompressão não ocorreu ou telemetria indisponível.
    /// Quando , a
    /// taxa de compressão pode ser calculada como
    /// . GAP-NEW-002 v0.8.0.
    #[serde(rename = "bytes_descomprimidos")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bytes_decompressed: Option<u64>,

    /// Nível de cascata observado no probe-deep mais recente da mesma
    /// sessão de processo. Cacheado em
    /// para uso como sinal cruzado pelo classificador de zero-result
    /// quando  não está ativo. GAP-NEW-003 v0.8.0.
    #[serde(rename = "cascata_nivel_observado")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cascade_level_observed: Option<u32>,
}

/// Complete output for a single-query search (serialized as JSON in the MVP).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchOutput {
    /// Original search query submitted by the user.
    pub query: String,

    /// Search engine used — always `"duckduckgo"`.
    #[serde(rename = "motor")]
    pub engine: String,

    /// Endpoint used — `"html"` or `"lite"` (always `"html"` in MVP).
    pub endpoint: String,

    /// ISO-8601 (RFC 3339) timestamp of when the search was executed.
    pub timestamp: String,

    /// `kl` region code used (e.g., `"br-pt"`).
    #[serde(rename = "regiao")]
    pub region: String,

    /// Count of results returned after ad filtering.
    #[serde(rename = "quantidade_resultados")]
    pub result_count: u32,

    /// List of organic results.
    #[serde(rename = "resultados")]
    pub results: Vec<SearchResult>,

    /// Number of pages fetched (always 1 in MVP).
    #[serde(rename = "paginas_buscadas")]
    pub pages_fetched: u32,

    /// Structured error code if the search partially failed (None on full success).
    #[serde(rename = "erro")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,

    /// Additional human-readable message (used for non-fatal warnings).
    #[serde(rename = "mensagem")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,

    /// Execution metadata.
    #[serde(rename = "metadados")]
    pub metadata: SearchMetadata,
}

/// Complete output for a multi-query execution (serialized as JSON).
///
/// Per section 14.1 of the specification. Each inner `SearchOutput` retains the
/// single-query format (including per-query `error`), and the root-level fields
/// aggregate metadata from the parallel execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultiSearchOutput {
    /// Total number of queries executed (success + failure).
    #[serde(rename = "quantidade_queries")]
    pub query_count: u32,

    /// ISO-8601 (RFC 3339) timestamp of the start of the parallel execution.
    pub timestamp: String,

    /// Effective `--parallel` value used during execution (after validation/clamp).
    #[serde(rename = "paralelismo")]
    pub parallelism: u32,

    /// Result of each individual query, in the same order as the input queries.
    #[serde(rename = "buscas")]
    pub searches: Vec<SearchOutput>,

    /// Histograma agregado de `causa_zero` em todas as sub-queries (deep-research).
    ///
    /// `BTreeMap` para ordem lexicográfica estável no output JSON determinístico.
    /// Chave é o nome kebab-case da variante de `ZeroCause`; valor é a contagem.
    /// v0.8.0.
    #[serde(rename = "causa_zero_histogram")]
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub causa_zero_histogram: BTreeMap<String, u32>,
}

/// CSS selector configuration (loaded from selectors.toml or hardcoded defaults).
///
/// Retains the existing fields (`html_endpoint`) for backward compatibility with
/// tests and selector hashing. Starting from iteration 6, adds flat additional
/// fields for the Lite endpoint, pagination, and related searches, enabling
/// full externalization via an external TOML file.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct SelectorConfig {
    /// Legacy group — retained for compatibility with existing serialization and tests.
    pub html_endpoint: HtmlSelectors,

    /// Selector group for the Lite endpoint.
    #[serde(default)]
    pub lite_endpoint: LiteSelectors,

    /// Selectors used to extract pagination data (form `s`).
    #[serde(default)]
    pub pagination: PaginationSelectors,

    /// Selectors used to extract "related searches".
    #[serde(default)]
    pub related_searches: RelatedSelectors,
}

/// CSS selectors for the full HTML endpoint (`html.duckduckgo.com`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct HtmlSelectors {
    /// Outer container holding all organic results.
    pub results_container: String,
    /// Individual result item (excludes ads).
    pub result_item: String,
    /// Link element carrying the title and destination URL.
    pub title_and_url: String,
    /// Element containing the result snippet/description.
    pub snippet: String,
    /// Element showing the display URL below the title.
    pub display_url: String,
    /// Rules for filtering out sponsored/ad results.
    pub ads_filter: AdFilter,
}

/// Patterns used to detect and filter out sponsored results.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AdFilter {
    /// CSS classes that mark an element as an ad.
    pub ad_classes: Vec<String>,
    /// HTML attributes indicating sponsored content.
    pub ad_attributes: Vec<String>,
    /// URL substrings found in ad-tracking redirects.
    pub ad_url_patterns: Vec<String>,
}

/// CSS selectors for the lite endpoint (`lite.duckduckgo.com`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct LiteSelectors {
    /// Table element wrapping all results.
    pub results_table: String,
    /// Anchor element linking to the result page.
    pub result_link: String,
    /// Cell containing the result snippet text.
    pub result_snippet: String,
}

/// CSS selectors for extracting pagination tokens from the HTML form.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct PaginationSelectors {
    /// Hidden input carrying the `vqd` token.
    pub vqd_input: String,
    /// Hidden input carrying the `s` (start offset) value.
    pub s_input: String,
    /// Hidden input carrying the `dc` (document count) value.
    pub dc_input: String,
    /// Form element for the "next page" action.
    pub next_form: String,
}

/// CSS selectors for related-searches links (currently unused; DDG HTML does not expose them).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct RelatedSelectors {
    /// Container element for the related-searches block.
    pub container: String,
    /// Anchor elements inside the related-searches block.
    pub links: String,
}

impl Default for HtmlSelectors {
    fn default() -> Self {
        Self {
            results_container: "#links".to_string(),
            result_item:
                "#links .result:not(.result--ad), #links .results_links, div.result:not(.result--ad)"
                    .to_string(),
            title_and_url: ".result__a, a.result__a, .result__title a".to_string(),
            // v0.3.0: removido `.result__body` — casava o container pai e trazia
            // titulo+url+snippet concatenados no campo snippet.
            snippet: ".result__snippet, a.result__snippet".to_string(),
            display_url: ".result__url, span.result__url".to_string(),
            ads_filter: AdFilter::default(),
        }
    }
}

impl Default for AdFilter {
    fn default() -> Self {
        Self {
            ad_classes: vec![".result--ad".to_string(), ".badge--ad".to_string()],
            ad_attributes: vec!["data-nrn=ad".to_string()],
            ad_url_patterns: vec!["duckduckgo.com/y.js".to_string()],
        }
    }
}

impl Default for LiteSelectors {
    fn default() -> Self {
        Self {
            results_table: "table, body table".to_string(),
            result_link: "a.result-link, td a[href]".to_string(),
            result_snippet: "td.result-snippet, tr.result-snippet td".to_string(),
        }
    }
}

impl Default for PaginationSelectors {
    fn default() -> Self {
        Self {
            vqd_input: "input[name='vqd'], input[type='hidden'][name='vqd']".to_string(),
            s_input: "input[name='s']".to_string(),
            dc_input: "input[name='dc']".to_string(),
            next_form: "form.result--more__btn, form[action='/html/']".to_string(),
        }
    }
}

impl Default for RelatedSelectors {
    fn default() -> Self {
        Self {
            container: ".result--more__btn, .result--sep".to_string(),
            links: "a".to_string(),
        }
    }
}

/// `DuckDuckGo` endpoint chosen via `--endpoint`.
///
/// - `Html` (default): `https://html.duckduckgo.com/html/` with `.result` in the DOM.
/// - `Lite`: `https://lite.duckduckgo.com/lite/` with tabular layout (no JavaScript).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Endpoint {
    /// Full HTML endpoint with `.result` DOM structure.
    Html,
    /// Lightweight endpoint with tabular layout (no JavaScript).
    Lite,
}

impl Endpoint {
    /// Returns the short string used in logs and metadata output.
    pub fn as_str(&self) -> &'static str {
        match self {
            Endpoint::Html => "html",
            Endpoint::Lite => "lite",
        }
    }
}

/// `DuckDuckGo` `df` time filter.
///
/// Values accepted by the API: `d` (day), `w` (week), `m` (month), `y` (year).
/// Absence of the parameter means "no time filter".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeFilter {
    /// Results from the last 24 hours.
    Day,
    /// Results from the last 7 days.
    Week,
    /// Results from the last 30 days.
    Month,
    /// Results from the last 365 days.
    Year,
}

impl TimeFilter {
    /// Returns the code accepted by the URL's `df` parameter.
    pub fn as_param(&self) -> &'static str {
        match self {
            TimeFilter::Day => "d",
            TimeFilter::Week => "w",
            TimeFilter::Month => "m",
            TimeFilter::Year => "y",
        }
    }
}

/// `DuckDuckGo` safe-search (`kp` parameter).
///
/// Accepted values: `-2` moderate (DDG default, sent as absence of the parameter),
/// `-1` off (disables filters), `1` strict (filters adult content).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SafeSearch {
    /// Disables all content filters (`kp=-1`).
    Off,
    /// DDG default — moderate filtering (no `kp` parameter sent).
    Moderate,
    /// Strict filtering of adult content (`kp=1`).
    Strict,
}

impl SafeSearch {
    /// Value for the `kp` parameter. `None` means "do not add the parameter"
    /// (equivalent to DDG's moderate default).
    pub fn as_param(&self) -> Option<&'static str> {
        match self {
            SafeSearch::Off => Some("-1"),
            SafeSearch::Moderate => None,
            SafeSearch::Strict => Some("1"),
        }
    }
}

/// Global settings derived from the CLI, passed through the pipeline.
///
/// The `query` field remains as the "active query" in single-query executions
/// (useful for the legacy flow in `pipeline::execute`). In multi-query mode, the
/// pipeline iterates over `queries` and clones this struct for each task,
/// overwriting `query` with the current iteration item.
#[derive(Clone)]
pub struct Config {
    /// "Active" query — populated before calling the single-query flow.
    /// In multi-query mode starts equal to the first query and is overwritten per task.
    pub query: String,
    /// Full list of queries to execute. Always contains at least 1 item.
    pub queries: Vec<String>,
    /// Desired number of results (maps to pagination logic).
    pub num_results: Option<u32>,
    /// Output format chosen via `--format`.
    pub format: OutputFormat,
    /// Per-request HTTP timeout in seconds.
    pub timeout_seconds: u64,
    /// Language code for DDG `kl` parameter (e.g. `"pt-br"`).
    pub language: String,
    /// Country code for DDG `kl` parameter (e.g. `"br"`).
    pub country: String,
    /// Verbosity level of stderr logs (0=INFO, 1+=DEBUG, 2+=TRACE).
    /// Populated by the `-v`/`-vv`/`-vvv` repeated flag via `clap::ArgAction::Count`.
    pub verbose: u8,
    /// `--quiet` flag — suppresses non-essential stderr output.
    pub quiet: bool,
    /// Selected User-Agent string sent in HTTP headers.
    pub user_agent: String,
    /// Full browser profile — family, version and platform derived from `user_agent`.
    /// Kept alongside the `user_agent` field (used in `SearchMetadata` and JSON output).
    pub browser_profile: BrowserProfile,
    /// Effective parallelism degree (1..=20). Informational only in single-query mode.
    pub parallelism: u32,
    /// Number of pages to fetch per query (1..=5).
    pub pages: u32,
    /// Number of retry attempts (0..=10). 0 = no retry; 2 is the default.
    pub retries: u32,
    /// Preferred endpoint (html by default; lite forces the no-JavaScript endpoint).
    pub endpoint: Endpoint,
    /// Optional time filter (`df`).
    pub time_filter: Option<TimeFilter>,
    /// Safe-search (`kp`).
    pub safe_search: SafeSearch,
    /// `--stream` flag (placeholder — not implemented in this iteration).
    pub stream_mode: bool,
    /// Optional path for writing output (instead of stdout).
    pub output_file: Option<std::path::PathBuf>,
    /// `--fetch-content` flag — enables text content extraction from result pages.
    pub fetch_content: bool,
    /// Value of `--max-content-length` — maximum content size in characters (1..=100000).
    pub max_content_length: usize,
    /// HTTP/HTTPS/SOCKS5 proxy URL via `--proxy`. When `Some`, takes precedence over env vars.
    pub proxy: Option<String>,
    /// `--no-proxy` flag — disables any proxy (including env vars). Mutually exclusive with `proxy`.
    pub no_proxy: bool,
    /// Value of `--global-timeout` in seconds (global timeout for the entire execution).
    pub global_timeout_seconds: u64,
    /// `--match-platform-ua` flag — restricts UAs from the external config to the current platform.
    pub match_platform_ua: bool,
    /// Per-host concurrent fetch limit in `--fetch-content` mode (1..=10, default 2).
    pub per_host_limit: usize,
    /// Optional manual path to Chrome/Chromium (`--chrome-path` flag, `chrome` feature).
    /// Without the `chrome` feature or `--fetch-content`, this value is ignored with a warning.
    pub chrome_path: Option<std::path::PathBuf>,
    /// CSS selector configuration (loaded from selectors.toml or built-in defaults).
    /// Wrapped in `Arc` for cheap cloning across concurrent tasks.
    pub selectors: std::sync::Arc<SelectorConfig>,
    /// Pre-built cookie jar for `reqwest::Client::cookie_provider`. Built by
    /// `build_config` from the persistent JSON file (or an empty jar if
    /// persistence is disabled). v0.7.3 PR2.
    pub cookie_provider: Option<std::sync::Arc<reqwest::cookie::Jar>>,
    /// Persistent jar handle used by the pipeline to save cookies back to
    /// disk after the request completes. v0.7.3 PR2.
    pub persistent_jar: Option<crate::cookie_adapter::PersistentJar>,
    /// Whether to perform the warm-up `GET https://duckduckgo.com/`
    /// before the first real query. v0.7.3 PR2.
    pub warmup_enabled: bool,
    /// Whether to allow automatic fallback to the `lite` endpoint when
    /// the `html` endpoint returns a bot-detection interstitial. v0.7.3 PR3.
    pub allow_lite_fallback: bool,
    /// Whether to enable pre-flight ghost-block detection. v0.7.9 GAP-WS-58.
    /// When `true`, a sub-4KB body with no result-page selector is
    /// classified as a Cloudflare ghost-block and triggers an
    /// automatic Lite fallback even WITHOUT `allow_lite_fallback`.
    /// Default `false` — opt-in via `--pre-flight` to preserve v0.7.8
    /// behavior when the operator does not ask for the new gate.
    pub pre_flight: bool,
    /// Selected browser identity profile from the 12-identity pool.
    /// Default `Auto` selects the adaptive cascade (rotates on block).
    /// When set to a specific family+platform tuple, the session is
    /// pinned to that single identity. v0.7.10 GAP-WS-60 fix — the
    /// flag was previously declared on the CLI but never propagated
    /// to `Config` (help-first drift).
    pub identity_profile: CliIdentityProfile,
    /// Cache do nível de cascata observado no último probe-deep
    /// bem-sucedido da mesma sessão de processo. Usado pelo classificador
    /// de zero-result como sinal cruzado quando  não está
    /// ativo. v0.8.0 GAP-NEW-003.
    pub last_probe_cascade_level: Option<u32>,
}

impl Default for Config {
    fn default() -> Self {
        use std::sync::Arc;
        Self {
            query: String::new(),
            queries: Vec::new(),
            num_results: None,
            format: OutputFormat::Json,
            timeout_seconds: 30,
            language: "en".to_string(),
            country: "us".to_string(),
            verbose: 0,
            quiet: false,
            user_agent: String::new(),
            browser_profile: crate::http::BrowserProfile::default(),
            parallelism: 1,
            pages: 1,
            retries: 2,
            endpoint: Endpoint::Html,
            time_filter: None,
            safe_search: SafeSearch::Moderate,
            stream_mode: false,
            output_file: None,
            fetch_content: false,
            max_content_length: 4096,
            proxy: None,
            no_proxy: false,
            global_timeout_seconds: 60,
            match_platform_ua: false,
            per_host_limit: 2,
            chrome_path: None,
            selectors: Arc::new(SelectorConfig::default()),
            cookie_provider: None,
            persistent_jar: None,
            warmup_enabled: false,
            allow_lite_fallback: false,
            pre_flight: false,
            identity_profile: CliIdentityProfile::Auto,
            last_probe_cascade_level: None,
        }
    }
}

impl std::fmt::Debug for Config {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Config")
            .field("query", &self.query)
            .field("endpoint", &self.endpoint)
            .field("warmup_enabled", &self.warmup_enabled)
            .field("allow_lite_fallback", &self.allow_lite_fallback)
            .field("pre_flight", &self.pre_flight)
            .field("identity_profile", &self.identity_profile)
            .finish()
    }
}

/// Output formats supported by the CLI (only `Json` is supported in the MVP).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    /// Structured JSON (default for pipes and LLM consumption).
    Json,
    /// Human-readable plain text.
    Text,
    /// Markdown with headers and links.
    Markdown,
    /// Auto-detect: JSON when stdout is not a TTY, Text otherwise.
    Auto,
}

impl OutputFormat {
    /// Converts a `"json"|"text"|"markdown"|"auto"` string into the corresponding enum variant.
    pub fn from_str_value(value: &str) -> Option<Self> {
        match value.to_ascii_lowercase().as_str() {
            "json" => Some(Self::Json),
            "text" => Some(Self::Text),
            "markdown" | "md" => Some(Self::Markdown),
            "auto" => Some(Self::Auto),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selector_config_default_has_result_container() {
        let cfg = SelectorConfig::default();
        assert_eq!(cfg.html_endpoint.results_container, "#links");
        assert!(cfg
            .html_endpoint
            .ads_filter
            .ad_url_patterns
            .contains(&"duckduckgo.com/y.js".to_string()));
    }

    #[test]
    fn output_format_parses_valid_variants() {
        assert_eq!(
            OutputFormat::from_str_value("json"),
            Some(OutputFormat::Json)
        );
        assert_eq!(
            OutputFormat::from_str_value("TEXT"),
            Some(OutputFormat::Text)
        );
        assert_eq!(
            OutputFormat::from_str_value("markdown"),
            Some(OutputFormat::Markdown)
        );
        assert_eq!(
            OutputFormat::from_str_value("md"),
            Some(OutputFormat::Markdown)
        );
        assert_eq!(
            OutputFormat::from_str_value("Auto"),
            Some(OutputFormat::Auto)
        );
        assert_eq!(OutputFormat::from_str_value("xml"), None);
    }

    #[test]
    fn search_output_serializes_pt_json_keys() {
        let output = SearchOutput {
            query: "teste".to_string(),
            engine: "duckduckgo".to_string(),
            endpoint: "html".to_string(),
            timestamp: "2026-04-14T00:00:00Z".to_string(),
            region: "br-pt".to_string(),
            result_count: 0,
            results: vec![],
            pages_fetched: 1,
            error: None,
            message: None,
            metadata: SearchMetadata {
                execution_time_ms: 0,
                selectors_hash: "abc123".to_string(),
                retries: 0,
                retries_configured: None,
                used_fallback_endpoint: false,
                concurrent_fetches: 0,
                fetch_successes: 0,
                fetch_failures: 0,
                used_chrome: false,
                chrome_attempted: false,
                user_agent: "Mozilla/5.0".to_string(),
                used_proxy: false,
                identity_used: None,
                cascade_level: None,
                pre_flight_fired: false,
                zero_cause: None,
                sugestao_proxima_acao: None,
                bytes_raw: None,
                bytes_decompressed: None,
                cascade_level_observed: None,
            },
        };
        let json = serde_json::to_string(&output).expect("serialization should work");
        // Portuguese JSON keys must be preserved (backward-compat invariant).
        assert!(json.contains("\"query\""));
        assert!(json.contains("\"quantidade_resultados\""));
        assert!(json.contains("\"tempo_execucao_ms\""));
        assert!(json.contains("\"resultados\""));
        assert!(json.contains("\"metadados\""));
        // v0.3.0 BREAKING: campo `buscas_relacionadas` removido do schema.
        assert!(!json.contains("\"buscas_relacionadas\""));
        // English Rust field names must NOT leak into JSON output.
        assert!(!json.contains("\"results_count\""));
        assert!(!json.contains("\"results\":"));
        assert!(!json.contains("\"metadata\""));
        assert!(!json.contains("\"related_searches\""));
    }

    #[test]
    fn multi_search_output_serializes_pt_json_keys() {
        let output = MultiSearchOutput {
            query_count: 2,
            timestamp: "2026-04-14T00:00:00Z".to_string(),
            parallelism: 5,
            searches: vec![],
            causa_zero_histogram: BTreeMap::new(),
        };
        let json = serde_json::to_string(&output).expect("serialization should work");
        // Portuguese JSON keys must be preserved.
        assert!(json.contains("\"quantidade_queries\":2"));
        assert!(json.contains("\"paralelismo\":5"));
        assert!(json.contains("\"buscas\":[]"));
        // English field names must NOT appear in JSON.
        assert!(!json.contains("\"queries_count\""));
        assert!(!json.contains("\"parallel\""));
        assert!(!json.contains("\"searches\""));
    }
}
