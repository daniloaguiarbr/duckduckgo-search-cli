//! Shared data types used across the application.
//!
//! All output structs (`SaidaBusca`, `SaidaBuscaMultipla`, `ResultadoBusca`,
//! `MetadadosBusca`) serialize with field names in Brazilian Portuguese
//! (snake_case), as per the INVIOLABLE invariant of blueprint v2: "Logs and field
//! names in Brazilian Portuguese". Rust field names and external JSON names
//! coincide — no active `serde(rename)`.

use crate::http::PerfilBrowser;
use serde::{Deserialize, Serialize};

/// Represents a single DuckDuckGo search result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResultadoBusca {
    /// Result position on the page (1-indexed, already after ad filtering).
    pub posicao: u32,

    /// Result title, extracted from the `.result__a` element.
    pub titulo: String,

    /// Result URL, extracted from the `href` attribute of `.result__a`.
    pub url: String,

    /// Display URL (more user-friendly), extracted from `.result__url`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url_exibicao: Option<String>,

    /// Descriptive snippet for the result, extracted from `.result__snippet`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub snippet: Option<String>,

    /// Literal title text as rendered by DuckDuckGo, preserved for auditing
    /// when substitution heuristics are applied (e.g., DDG returns "Official site"
    /// for verified domains — we replace it with `url_exibicao` and keep the
    /// original here). Absent when the title was not modified.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub titulo_original: Option<String>,

    /// Full text content of the page (only with `--fetch-content`; not implemented in the MVP).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub conteudo: Option<String>,

    /// Size in characters of the extracted content (only with `--fetch-content`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tamanho_conteudo: Option<u32>,

    /// Method used to extract content: `"http"` or `"chrome"` (only with `--fetch-content`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metodo_extracao_conteudo: Option<String>,
}

/// Search execution metadata, useful for diagnostics and LLM integration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetadadosBusca {
    /// Total execution time in milliseconds.
    pub tempo_execucao_ms: u64,

    /// Blake3 hash (hex, first 16 characters) of the selector configuration used.
    pub hash_seletores: String,

    /// Number of retries performed (0 in MVP — retry not yet implemented).
    pub retentativas: u32,

    /// Indicates whether the Lite endpoint was used as fallback (always `false` in MVP).
    pub usou_endpoint_fallback: bool,

    /// Number of parallel content fetches started (0 in MVP).
    pub fetches_simultaneos: u32,

    /// Successful content fetches (0 in MVP).
    pub sucessos_fetch: u32,

    /// Failed content fetches (0 in MVP).
    pub falhas_fetch: u32,

    /// Indicates whether Chrome was used (always `false` in MVP).
    pub usou_chrome: bool,

    /// User-Agent used during execution.
    pub user_agent: String,

    /// Indicates whether a proxy was configured (always `false` in MVP).
    pub usou_proxy: bool,
}

/// Complete output for a single-query search (serialized as JSON in the MVP).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaidaBusca {
    /// Original search query submitted by the user.
    pub query: String,

    /// Search engine used — always `"duckduckgo"`.
    pub motor: String,

    /// Endpoint used — `"html"` or `"lite"` (always `"html"` in MVP).
    pub endpoint: String,

    /// ISO-8601 (RFC 3339) timestamp of when the search was executed.
    pub timestamp: String,

    /// `kl` region code used (e.g., `"br-pt"`).
    pub regiao: String,

    /// Count of results returned after ad filtering.
    pub quantidade_resultados: u32,

    /// List of organic results.
    pub resultados: Vec<ResultadoBusca>,

    /// Number of pages fetched (always 1 in MVP).
    pub paginas_buscadas: u32,

    /// Structured error code if the search partially failed (None on full success).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub erro: Option<String>,

    /// Additional human-readable message (used for non-fatal warnings).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mensagem: Option<String>,

    /// Execution metadata.
    pub metadados: MetadadosBusca,
}

/// Complete output for a multi-query execution (serialized as JSON).
///
/// Per section 14.1 of the specification. Each inner `SaidaBusca` retains the
/// single-query format (including per-query `error`), and the root-level fields
/// aggregate metadata from the parallel execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaidaBuscaMultipla {
    /// Total number of queries executed (success + failure).
    pub quantidade_queries: u32,

    /// ISO-8601 (RFC 3339) timestamp of the start of the parallel execution.
    pub timestamp: String,

    /// Effective `--parallel` value used during execution (after validation/clamp).
    pub paralelismo: u32,

    /// Result of each individual query, in the same order as the input queries.
    pub buscas: Vec<SaidaBusca>,
}

/// CSS selector configuration (loaded from selectors.toml or hardcoded defaults).
///
/// Retains the existing fields (`html_endpoint`) for backward compatibility with
/// tests and selector hashing. Starting from iteration 6, adds flat additional
/// fields for the Lite endpoint, pagination, and related searches, enabling
/// full externalization via an external TOML file.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ConfiguracaoSeletores {
    /// Legacy group — retained for compatibility with existing serialization and tests.
    pub html_endpoint: SeletoresHtml,

    /// Selector group for the Lite endpoint.
    #[serde(default)]
    pub lite_endpoint: SeletoresLite,

    /// Selectors used to extract pagination data (form `s`).
    #[serde(default)]
    pub pagination: SeletoresPaginacao,

    /// Selectors used to extract "related searches".
    #[serde(default)]
    pub related_searches: SeletoresRelacionadas,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SeletoresHtml {
    pub results_container: String,
    pub result_item: String,
    pub title_and_url: String,
    pub snippet: String,
    pub display_url: String,
    pub ads_filter: FiltroAnuncios,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct FiltroAnuncios {
    pub ad_classes: Vec<String>,
    pub ad_attributes: Vec<String>,
    pub ad_url_patterns: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SeletoresLite {
    pub results_table: String,
    pub result_link: String,
    pub result_snippet: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SeletoresPaginacao {
    pub vqd_input: String,
    pub s_input: String,
    pub dc_input: String,
    pub next_form: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SeletoresRelacionadas {
    pub container: String,
    pub links: String,
}

impl Default for SeletoresHtml {
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
            ads_filter: FiltroAnuncios::default(),
        }
    }
}

impl Default for FiltroAnuncios {
    fn default() -> Self {
        Self {
            ad_classes: vec![".result--ad".to_string(), ".badge--ad".to_string()],
            ad_attributes: vec!["data-nrn=ad".to_string()],
            ad_url_patterns: vec!["duckduckgo.com/y.js".to_string()],
        }
    }
}

impl Default for SeletoresLite {
    fn default() -> Self {
        Self {
            results_table: "table, body table".to_string(),
            result_link: "a.result-link, td a[href]".to_string(),
            result_snippet: "td.result-snippet, tr.result-snippet td".to_string(),
        }
    }
}

impl Default for SeletoresPaginacao {
    fn default() -> Self {
        Self {
            vqd_input: "input[name='vqd'], input[type='hidden'][name='vqd']".to_string(),
            s_input: "input[name='s']".to_string(),
            dc_input: "input[name='dc']".to_string(),
            next_form: "form.result--more__btn, form[action='/html/']".to_string(),
        }
    }
}

impl Default for SeletoresRelacionadas {
    fn default() -> Self {
        Self {
            container: ".result--more__btn, .result--sep".to_string(),
            links: "a".to_string(),
        }
    }
}

/// DuckDuckGo endpoint chosen via `--endpoint`.
///
/// - `Html` (default): `https://html.duckduckgo.com/html/` with `.result` in the DOM.
/// - `Lite`: `https://lite.duckduckgo.com/lite/` with tabular layout (no JavaScript).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Endpoint {
    Html,
    Lite,
}

impl Endpoint {
    pub fn como_str(&self) -> &'static str {
        match self {
            Endpoint::Html => "html",
            Endpoint::Lite => "lite",
        }
    }
}

/// DuckDuckGo `df` time filter.
///
/// Values accepted by the API: `d` (day), `w` (week), `m` (month), `y` (year).
/// Absence of the parameter means "no time filter".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FiltroTemporal {
    Dia,
    Semana,
    Mes,
    Ano,
}

impl FiltroTemporal {
    /// Returns the code accepted by the URL's `df` parameter.
    pub fn como_parametro(&self) -> &'static str {
        match self {
            FiltroTemporal::Dia => "d",
            FiltroTemporal::Semana => "w",
            FiltroTemporal::Mes => "m",
            FiltroTemporal::Ano => "y",
        }
    }
}

/// DuckDuckGo safe-search (`kp` parameter).
///
/// Accepted values: `-2` moderate (DDG default, sent as absence of the parameter),
/// `-1` off (disables filters), `1` strict (filters adult content).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SafeSearch {
    Off,
    Moderate,
    Strict,
}

impl SafeSearch {
    /// Value for the `kp` parameter. `None` means "do not add the parameter"
    /// (equivalent to DDG's moderate default).
    pub fn como_parametro(&self) -> Option<&'static str> {
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
/// (useful for the legacy flow in `pipeline::executar`). In multi-query mode, the
/// pipeline iterates over `queries` and clones this struct for each task,
/// overwriting `query` with the current iteration item.
#[derive(Debug, Clone)]
pub struct Configuracoes {
    /// "Active" query — populated before calling the single-query flow.
    /// In multi-query mode starts equal to the first query and is overwritten per task.
    pub query: String,
    /// Full list of queries to execute. Always contains at least 1 item.
    pub queries: Vec<String>,
    pub num_resultados: Option<u32>,
    pub formato: FormatoSaida,
    pub timeout_segundos: u64,
    pub idioma: String,
    pub pais: String,
    pub modo_verboso: bool,
    pub modo_silencioso: bool,
    pub user_agent: String,
    /// Full browser profile — family, version and platform derived from `user_agent`.
    /// Kept alongside the `user_agent` field (used in MetadadosBusca and JSON output).
    pub perfil_browser: PerfilBrowser,
    /// Effective parallelism degree (1..=20). Informational only in single-query mode.
    pub paralelismo: u32,
    /// Number of pages to fetch per query (1..=5).
    pub paginas: u32,
    /// Number of retry attempts (0..=10). 0 = no retry; 2 is the default.
    pub retries: u32,
    /// Preferred endpoint (html by default; lite forces the no-JavaScript endpoint).
    pub endpoint: Endpoint,
    /// Optional time filter (`df`).
    pub filtro_temporal: Option<FiltroTemporal>,
    /// Safe-search (`kp`).
    pub safe_search: SafeSearch,
    /// `--stream` flag (placeholder — not implemented in this iteration).
    pub modo_stream: bool,
    /// Optional path for writing output (instead of stdout).
    pub arquivo_saida: Option<std::path::PathBuf>,
    /// `--fetch-content` flag — enables text content extraction from result pages.
    pub buscar_conteudo: bool,
    /// Value of `--max-content-length` — maximum content size in characters (1..=100000).
    pub max_tamanho_conteudo: usize,
    /// HTTP/HTTPS/SOCKS5 proxy URL via `--proxy`. When `Some`, takes precedence over env vars.
    pub proxy: Option<String>,
    /// `--no-proxy` flag — disables any proxy (including env vars). Mutually exclusive with `proxy`.
    pub sem_proxy: bool,
    /// Value of `--global-timeout` in seconds (global timeout for the entire execution).
    pub timeout_global_segundos: u64,
    /// `--match-platform-ua` flag — restricts UAs from the external config to the current platform.
    pub corresponde_plataforma_ua: bool,
    /// Per-host concurrent fetch limit in `--fetch-content` mode (1..=10, default 2).
    pub limite_por_host: usize,
    /// Optional manual path to Chrome/Chromium (`--chrome-path` flag, `chrome` feature).
    /// Without the `chrome` feature or `--fetch-content`, this value is ignored with a warning.
    pub caminho_chrome: Option<std::path::PathBuf>,
    /// CSS selector configuration (loaded from selectors.toml or built-in defaults).
    /// Wrapped in `Arc` for cheap cloning across concurrent tasks.
    pub seletores: std::sync::Arc<ConfiguracaoSeletores>,
}

/// Output formats supported by the CLI (only `Json` is supported in the MVP).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormatoSaida {
    Json,
    Text,
    Markdown,
    Auto,
}

impl FormatoSaida {
    /// Converts a `"json"|"text"|"markdown"|"auto"` string into the corresponding enum variant.
    pub fn a_partir_de_str(valor: &str) -> Option<Self> {
        match valor.to_ascii_lowercase().as_str() {
            "json" => Some(Self::Json),
            "text" => Some(Self::Text),
            "markdown" | "md" => Some(Self::Markdown),
            "auto" => Some(Self::Auto),
            _ => None,
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn configuracao_seletores_default_contem_result_container() {
        let cfg = ConfiguracaoSeletores::default();
        assert_eq!(cfg.html_endpoint.results_container, "#links");
        assert!(cfg
            .html_endpoint
            .ads_filter
            .ad_url_patterns
            .contains(&"duckduckgo.com/y.js".to_string()));
    }

    #[test]
    fn formato_saida_parseia_variantes_validas() {
        assert_eq!(
            FormatoSaida::a_partir_de_str("json"),
            Some(FormatoSaida::Json)
        );
        assert_eq!(
            FormatoSaida::a_partir_de_str("TEXT"),
            Some(FormatoSaida::Text)
        );
        assert_eq!(
            FormatoSaida::a_partir_de_str("markdown"),
            Some(FormatoSaida::Markdown)
        );
        assert_eq!(
            FormatoSaida::a_partir_de_str("md"),
            Some(FormatoSaida::Markdown)
        );
        assert_eq!(
            FormatoSaida::a_partir_de_str("Auto"),
            Some(FormatoSaida::Auto)
        );
        assert_eq!(FormatoSaida::a_partir_de_str("xml"), None);
    }

    #[test]
    fn saida_busca_serializa_campos_em_portugues_no_json() {
        let saida = SaidaBusca {
            query: "teste".to_string(),
            motor: "duckduckgo".to_string(),
            endpoint: "html".to_string(),
            timestamp: "2026-04-14T00:00:00Z".to_string(),
            regiao: "br-pt".to_string(),
            quantidade_resultados: 0,
            resultados: vec![],
            paginas_buscadas: 1,
            erro: None,
            mensagem: None,
            metadados: MetadadosBusca {
                tempo_execucao_ms: 0,
                hash_seletores: "abc123".to_string(),
                retentativas: 0,
                usou_endpoint_fallback: false,
                fetches_simultaneos: 0,
                sucessos_fetch: 0,
                falhas_fetch: 0,
                usou_chrome: false,
                user_agent: "Mozilla/5.0".to_string(),
                usou_proxy: false,
            },
        };
        let json = serde_json::to_string(&saida).expect("serialização deve funcionar");
        // Nomes de campo em PT devem aparecer no JSON (invariante INVIOLÁVEL do blueprint v2).
        assert!(json.contains("\"query\""));
        assert!(json.contains("\"quantidade_resultados\""));
        assert!(json.contains("\"tempo_execucao_ms\""));
        assert!(json.contains("\"resultados\""));
        assert!(json.contains("\"metadados\""));
        // v0.3.0 BREAKING: campo `buscas_relacionadas` removido do schema.
        assert!(!json.contains("\"buscas_relacionadas\""));
        // Nomes em inglês NÃO devem aparecer.
        assert!(!json.contains("\"results_count\""));
        assert!(!json.contains("\"results\":"));
        assert!(!json.contains("\"metadata\""));
        assert!(!json.contains("\"related_searches\""));
    }

    #[test]
    fn saida_busca_multipla_serializa_campos_em_portugues() {
        let saida = SaidaBuscaMultipla {
            quantidade_queries: 2,
            timestamp: "2026-04-14T00:00:00Z".to_string(),
            paralelismo: 5,
            buscas: vec![],
        };
        let json = serde_json::to_string(&saida).expect("serialização deve funcionar");
        // Nomes de campo em PT devem aparecer no JSON.
        assert!(json.contains("\"quantidade_queries\":2"));
        assert!(json.contains("\"paralelismo\":5"));
        assert!(json.contains("\"buscas\":[]"));
        // Nomes em inglês NÃO devem aparecer.
        assert!(!json.contains("\"queries_count\""));
        assert!(!json.contains("\"parallel\""));
        assert!(!json.contains("\"searches\""));
    }
}
