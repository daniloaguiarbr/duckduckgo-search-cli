//! Tipos de dados compartilhados pela aplicação.
//!
//! Todos os structs de saída (SearchResult, SearchOutput, SearchMetadata) usam
//! `#[serde(rename_all = "snake_case")]` nos campos para que o JSON externo
//! fique estável em nomenclatura inglesa/snake_case, enquanto os nomes internos
//! dos campos permanecem em português brasileiro conforme as regras do projeto.

use serde::{Deserialize, Serialize};

/// Representa um resultado individual de busca do DuckDuckGo.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResultadoBusca {
    /// Posição do resultado na página (1-indexed, já após filtragem de anúncios).
    #[serde(rename = "position")]
    pub posicao: u32,

    /// Título do resultado, extraído do elemento `.result__a`.
    #[serde(rename = "title")]
    pub titulo: String,

    /// URL do resultado, extraída do atributo `href` de `.result__a`.
    #[serde(rename = "url")]
    pub url: String,

    /// URL de exibição (mais amigável), extraída de `.result__url`.
    #[serde(rename = "displayed_url", skip_serializing_if = "Option::is_none")]
    pub url_exibicao: Option<String>,

    /// Snippet descritivo do resultado, extraído de `.result__snippet`.
    #[serde(rename = "snippet", skip_serializing_if = "Option::is_none")]
    pub snippet: Option<String>,

    /// Conteúdo textual completo da página (apenas com `--fetch-content`; não implementado no MVP).
    #[serde(rename = "content", skip_serializing_if = "Option::is_none")]
    pub conteudo: Option<String>,

    /// Tamanho em caracteres do conteúdo extraído (apenas com `--fetch-content`).
    #[serde(rename = "content_length", skip_serializing_if = "Option::is_none")]
    pub tamanho_conteudo: Option<u32>,

    /// Método usado para extrair o conteúdo: `"http"` ou `"chrome"` (apenas com `--fetch-content`).
    #[serde(
        rename = "content_extraction_method",
        skip_serializing_if = "Option::is_none"
    )]
    pub metodo_extracao_conteudo: Option<String>,
}

/// Metadados da execução da busca, úteis para diagnóstico e integração com LLM.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetadadosBusca {
    /// Tempo total de execução em milissegundos.
    #[serde(rename = "execution_time_ms")]
    pub tempo_execucao_ms: u64,

    /// Hash blake3 (hex, primeiros 16 caracteres) da configuração de seletores usada.
    #[serde(rename = "selectors_hash")]
    pub hash_seletores: String,

    /// Número de retentativas realizadas (0 no MVP — retry ainda não implementado).
    #[serde(rename = "retries")]
    pub retentativas: u32,

    /// Indica se o endpoint Lite foi usado como fallback (sempre `false` no MVP).
    #[serde(rename = "fallback_endpoint_used")]
    pub usou_endpoint_fallback: bool,

    /// Número de fetches paralelos de conteúdo iniciados (0 no MVP).
    #[serde(rename = "concurrent_fetches")]
    pub fetches_simultaneos: u32,

    /// Fetches bem-sucedidos de conteúdo (0 no MVP).
    #[serde(rename = "fetch_successes")]
    pub sucessos_fetch: u32,

    /// Fetches com falha (0 no MVP).
    #[serde(rename = "fetch_failures")]
    pub falhas_fetch: u32,

    /// Indica se o Chrome foi usado (sempre `false` no MVP).
    #[serde(rename = "chrome_used")]
    pub usou_chrome: bool,

    /// User-Agent utilizado na execução.
    #[serde(rename = "user_agent")]
    pub user_agent: String,

    /// Indica se um proxy foi configurado (sempre `false` no MVP).
    #[serde(rename = "proxy_used")]
    pub usou_proxy: bool,
}

/// Saída completa da busca single-query (serializada como JSON no MVP).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaidaBusca {
    /// A query de busca original enviada pelo usuário.
    #[serde(rename = "query")]
    pub query: String,

    /// Motor usado — sempre `"duckduckgo"`.
    #[serde(rename = "engine")]
    pub motor: String,

    /// Endpoint usado — `"html"` ou `"lite"` (sempre `"html"` no MVP).
    #[serde(rename = "endpoint")]
    pub endpoint: String,

    /// Timestamp ISO-8601 (RFC 3339) de quando a busca foi executada.
    #[serde(rename = "timestamp")]
    pub timestamp: String,

    /// Código de região `kl` usado (ex: `"br-pt"`).
    #[serde(rename = "region")]
    pub regiao: String,

    /// Contagem de resultados retornados após filtragem de anúncios.
    #[serde(rename = "results_count")]
    pub quantidade_resultados: u32,

    /// Lista de resultados orgânicos.
    #[serde(rename = "results")]
    pub resultados: Vec<ResultadoBusca>,

    /// Buscas relacionadas sugeridas pelo DuckDuckGo (vazio no MVP).
    #[serde(rename = "related_searches")]
    pub buscas_relacionadas: Vec<String>,

    /// Número de páginas buscadas (sempre 1 no MVP).
    #[serde(rename = "pages_fetched")]
    pub paginas_buscadas: u32,

    /// Código de erro estruturado se a busca falhou parcialmente (None em sucesso total).
    #[serde(rename = "error", skip_serializing_if = "Option::is_none")]
    pub erro: Option<String>,

    /// Mensagem humana adicional (usada para avisos não-fatais).
    #[serde(rename = "message", skip_serializing_if = "Option::is_none")]
    pub mensagem: Option<String>,

    /// Metadados da execução.
    #[serde(rename = "metadata")]
    pub metadados: MetadadosBusca,
}

/// Saída completa de uma execução multi-query (serializada como JSON).
///
/// Conforme seção 14.1 da especificação. Cada `SaidaBusca` interna mantém o
/// formato single-query (incluindo `error` por query), e os campos no nível raiz
/// agregam metadados da execução paralela.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaidaBuscaMultipla {
    /// Quantidade total de queries executadas (sucesso + falha).
    #[serde(rename = "queries_count")]
    pub quantidade_queries: u32,

    /// Timestamp ISO-8601 (RFC 3339) do início da execução paralela.
    #[serde(rename = "timestamp")]
    pub timestamp: String,

    /// Valor efetivo de `--parallel` usado na execução (após validação/clamp).
    #[serde(rename = "parallel")]
    pub paralelismo: u32,

    /// Resultado de cada query individual, na mesma ordem das queries de entrada.
    #[serde(rename = "searches")]
    pub buscas: Vec<SaidaBusca>,
}

/// Configuração de seletores CSS (carregada de selectors.toml ou defaults hardcoded).
///
/// Mantém os campos já existentes (`html_endpoint`) para compatibilidade retroativa
/// de testes + hash de seletores. A partir da iteração 6 adiciona campos planos
/// adicionais para o endpoint Lite, paginação e relacionadas, permitindo
/// externalização completa via arquivo TOML externo.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ConfiguracaoSeletores {
    /// Grupo legado — mantido por compat com serialização e testes existentes.
    pub html_endpoint: SeletoresHtml,

    /// Grupo de seletores do endpoint Lite.
    #[serde(default)]
    pub lite_endpoint: SeletoresLite,

    /// Seletores usados para extração de dados de paginação (formulário `s`).
    #[serde(default)]
    pub pagination: SeletoresPaginacao,

    /// Seletores usados para extração de "buscas relacionadas".
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
            result_item: "#links .result, #links .results_links, div.result".to_string(),
            title_and_url: ".result__a, a.result__a, .result__title a".to_string(),
            snippet: ".result__snippet, a.result__snippet, .result__body".to_string(),
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

/// Endpoint do DuckDuckGo escolhido via `--endpoint`.
///
/// - `Html` (default): `https://html.duckduckgo.com/html/` com `.result` no DOM.
/// - `Lite`: `https://lite.duckduckgo.com/lite/` com layout tabular (sem JavaScript).
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

/// Filtro temporal `df` do DuckDuckGo.
///
/// Valores aceitos pela API: `d` (dia), `w` (semana), `m` (mês), `y` (ano).
/// Ausência do parâmetro significa "sem filtro temporal".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FiltroTemporal {
    Dia,
    Semana,
    Mes,
    Ano,
}

impl FiltroTemporal {
    /// Retorna o código aceito pelo parâmetro `df` da URL.
    pub fn como_parametro(&self) -> &'static str {
        match self {
            FiltroTemporal::Dia => "d",
            FiltroTemporal::Semana => "w",
            FiltroTemporal::Mes => "m",
            FiltroTemporal::Ano => "y",
        }
    }
}

/// Safe-search do DuckDuckGo (parâmetro `kp`).
///
/// Valores aceitos: `-2` moderate (default do DDG, enviado como ausência do parâmetro),
/// `-1` off (desativa filtros), `1` strict (filtra conteúdo adulto).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SafeSearch {
    Off,
    Moderate,
    Strict,
}

impl SafeSearch {
    /// Valor para o parâmetro `kp`. `None` significa "não adicionar o parâmetro"
    /// (equivalente ao default moderate do DDG).
    pub fn como_parametro(&self) -> Option<&'static str> {
        match self {
            SafeSearch::Off => Some("-1"),
            SafeSearch::Moderate => None,
            SafeSearch::Strict => Some("1"),
        }
    }
}

/// Configurações globais derivadas da CLI, passadas pelo pipeline.
///
/// O campo `query` permanece como "query ativa" em execuções single-query
/// (útil para o fluxo legado em `pipeline::executar`). Em multi-query, o
/// pipeline itera sobre `queries` e clona esta struct para cada task, sobrescrevendo
/// `query` com o item da iteração.
#[derive(Debug, Clone)]
pub struct Configuracoes {
    /// Query "ativa" — preenchida antes de chamar o fluxo single-query.
    /// Em multi-query começa igual à primeira query e é sobrescrita por task.
    pub query: String,
    /// Lista completa de queries a executar. Sempre contém pelo menos 1 item.
    pub queries: Vec<String>,
    pub num_resultados: Option<u32>,
    pub formato: FormatoSaida,
    pub timeout_segundos: u64,
    pub idioma: String,
    pub pais: String,
    pub modo_verboso: bool,
    pub modo_silencioso: bool,
    pub user_agent: String,
    /// Grau de paralelismo efetivo (1..=20). Em single-query é apenas informativo.
    pub paralelismo: u32,
    /// Número de páginas a buscar por query (1..=5).
    pub paginas: u32,
    /// Número de tentativas de retry (0..=10). 0 = sem retry; 2 é o default.
    pub retries: u32,
    /// Endpoint preferido (html por default; lite força o endpoint sem JavaScript).
    pub endpoint: Endpoint,
    /// Filtro temporal opcional (`df`).
    pub filtro_temporal: Option<FiltroTemporal>,
    /// Safe-search (`kp`).
    pub safe_search: SafeSearch,
    /// Flag `--stream` (placeholder — não implementado nesta iteração).
    pub modo_stream: bool,
    /// Caminho opcional para gravação da saída (em vez de stdout).
    pub arquivo_saida: Option<std::path::PathBuf>,
    /// Flag `--fetch-content` — ativa extração de conteúdo textual das páginas de resultado.
    pub buscar_conteudo: bool,
    /// Valor da flag `--max-content-length` — tamanho máximo do conteúdo em caracteres (1..=100000).
    pub max_tamanho_conteudo: usize,
    /// URL de proxy HTTP/HTTPS/SOCKS5 via `--proxy`. Quando `Some`, tem precedência sobre env vars.
    pub proxy: Option<String>,
    /// Flag `--no-proxy` — desabilita qualquer proxy (env vars inclusive). Mutuamente exclusivo com `proxy`.
    pub sem_proxy: bool,
    /// Valor da flag `--global-timeout` em segundos (timeout global da execução inteira).
    pub timeout_global_segundos: u64,
    /// Flag `--match-platform-ua` — restringe UAs da config externa à plataforma atual.
    pub corresponde_plataforma_ua: bool,
    /// Limite per-host de fetches simultâneos em `--fetch-content` (1..=10, default 2).
    pub limite_por_host: usize,
    /// Caminho manual opcional para Chrome/Chromium (flag `--chrome-path`, feature `chrome`).
    /// Sem feature `chrome` ou sem `--fetch-content`, o valor é ignorado com warning.
    pub caminho_chrome: Option<std::path::PathBuf>,
    /// Configuração de seletores CSS (carregada de selectors.toml ou defaults embutidos).
    /// Envolvida em `Arc` para permitir clonagem barata entre tasks concorrentes.
    pub seletores: std::sync::Arc<ConfiguracaoSeletores>,
}

/// Formatos de saída suportados pela CLI (no MVP apenas `Json` é suportado).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormatoSaida {
    Json,
    Text,
    Markdown,
    Auto,
}

impl FormatoSaida {
    /// Converte uma string `"json"|"text"|"markdown"|"auto"` no enum correspondente.
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
    fn saida_busca_serializa_campos_em_ingles_no_json() {
        let saida = SaidaBusca {
            query: "teste".to_string(),
            motor: "duckduckgo".to_string(),
            endpoint: "html".to_string(),
            timestamp: "2026-04-14T00:00:00Z".to_string(),
            regiao: "br-pt".to_string(),
            quantidade_resultados: 0,
            resultados: vec![],
            buscas_relacionadas: vec![],
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
        assert!(json.contains("\"query\""));
        assert!(json.contains("\"results_count\""));
        assert!(json.contains("\"execution_time_ms\""));
        // Campos internos em português NÃO devem aparecer no JSON.
        assert!(!json.contains("\"quantidade_resultados\""));
        assert!(!json.contains("\"tempo_execucao_ms_pt\""));
    }

    #[test]
    fn saida_busca_multipla_serializa_campos_em_ingles() {
        let saida = SaidaBuscaMultipla {
            quantidade_queries: 2,
            timestamp: "2026-04-14T00:00:00Z".to_string(),
            paralelismo: 5,
            buscas: vec![],
        };
        let json = serde_json::to_string(&saida).expect("serialização deve funcionar");
        assert!(json.contains("\"queries_count\":2"));
        assert!(json.contains("\"parallel\":5"));
        assert!(json.contains("\"searches\":[]"));
        // Campos internos em português NÃO devem aparecer.
        assert!(!json.contains("\"quantidade_queries\""));
        assert!(!json.contains("\"paralelismo\""));
        assert!(!json.contains("\"buscas\""));
    }
}
