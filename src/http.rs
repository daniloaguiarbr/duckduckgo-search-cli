//! Construção do `reqwest::Client` e seleção de User-Agent.
//!
//! O cliente HTTP é configurado com:
//! - TLS via `rustls-tls` (sem dependência de OpenSSL em nenhuma plataforma).
//! - Cookie store habilitado (necessário para paginação com token `vqd`).
//! - Compressão `gzip` + `brotli` (reduz bandwidth).
//! - Redirect policy limitada a 5 saltos.
//! - Headers que imitam browser real com perfil completo de família (Chrome, Firefox, Safari, Edge).
//! - Timeout total configurável.
//! - Proxy opcional HTTP/HTTPS/SOCKS5.
//! - User-Agents carregados de `user-agents.toml` externo OU defaults embutidos.
//!
//! ## Perfis de Browser (v0.6.0)
//!
//! Cada UA carregado recebe um [`PerfilBrowser`] que encapsula a família detectada
//! (`Chrome`, `Firefox`, `Safari`, `Edge`) e gera headers Sec-Fetch completos.
//! Chrome e Edge também emitem Client Hints (`Sec-CH-UA*`), replicando exatamente
//! o comportamento de browsers reais e reduzindo detecção anti-bot.

use crate::platform;
use anyhow::{Context, Result};
use rand::seq::SliceRandom;
use reqwest::{
    header::{
        HeaderMap, HeaderName, HeaderValue, ACCEPT, ACCEPT_ENCODING, ACCEPT_LANGUAGE, CACHE_CONTROL,
    },
    redirect::Policy,
    Client,
};
use serde::Deserialize;
use std::time::Duration;

/// Lista de User-Agents embutida no binário para fallback caso `config/user-agents.toml`
/// não esteja disponível.
///
/// v0.3.0 — ATUALIZAÇÃO DO POOL (2026-04-14):
/// Os UAs antigos de browsers de texto (Lynx, w3m, Links, ELinks) foram REMOVIDOS.
/// Empiricamente eles ainda retornam HTTP 200, mas o DuckDuckGo serve HTML
/// DEGRADADO para esses agentes: o layout fica sem classes `.result__snippet`
/// consistentes, forçando o extractor a cair na Estratégia 2 e retornar snippets
/// vazios/incorretos.
///
/// Validação empírica final (2026-04-14, requests reais ao /html/):
///   Chrome 146 Win/Mac/Linux → 200 OK ✓
///   Edge   145 Windows       → 200 OK ✓
///   Safari 17.6 macOS        → 200 OK ✓
///   Firefox 134 Linux        → 200 OK ✓
///   Firefox 134 Windows      → 202 ANOMALY ✗ (REMOVIDO)
///   Firefox 134 macOS        → 202 ANOMALY ✗ (REMOVIDO)
///
/// O DuckDuckGo bloqueia Firefox desktop Win/Mac no `/html/` endpoint
/// (heurística anti-bot: UA prometendo browser completo sem JS). Linux Firefox
/// passa porque é desktop minoritário — o filtro do DDG é menos agressivo.
const USER_AGENTS_PADRAO: &[&str] = &[
    // Chrome desktop (Windows / macOS / Linux) — abril 2026
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/146.0.0.0 Safari/537.36",
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/146.0.0.0 Safari/537.36",
    "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/146.0.0.0 Safari/537.36",
    // Edge Windows
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/145.0.0.0 Safari/537.36 Edg/145.0.3800.97",
    // Firefox desktop (somente Linux — Win/Mac dão HTTP 202 no /html/)
    "Mozilla/5.0 (X11; Linux x86_64; rv:134.0) Gecko/20100101 Firefox/134.0",
    // Safari macOS
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.6 Safari/605.1.15",
];

// ---------------------------------------------------------------------------
// Família de browser
// ---------------------------------------------------------------------------

/// Família do browser detectada a partir da string User-Agent.
///
/// Usada para gerar headers específicos por família (Client Hints, Accept, etc.).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FamiliaBrowser {
    /// Google Chrome ou derivados Chromium (exceto Edge).
    Chrome,
    /// Mozilla Firefox.
    Firefox,
    /// Apple Safari (sem indicativo de Chrome no UA).
    Safari,
    /// Microsoft Edge (baseado em Chromium, contém `Edg/`).
    Edge,
}

// ---------------------------------------------------------------------------
// Perfil de browser
// ---------------------------------------------------------------------------

/// Perfil completo de um browser derivado de seu User-Agent.
///
/// Encapsula família, versão major e plataforma para gerar headers
/// Sec-Fetch e Client Hints corretos por família.
#[derive(Debug, Clone)]
pub struct PerfilBrowser {
    /// Família do browser detectada.
    pub familia: FamiliaBrowser,
    /// String completa do User-Agent.
    pub user_agent: String,
    /// Versão major do browser (ex: 146 para Chrome 146).
    pub versao_major: u32,
    /// Plataforma normalizada para Client Hints (ex: `"Windows"`, `"macOS"`, `"Linux"`).
    pub plataforma_ua: String,
}

/// Detecta a família do browser a partir de uma string User-Agent.
///
/// Prioridade de detecção:
/// 1. `Edg/` → Edge
/// 2. `Chrome/` → Chrome
/// 3. `Firefox/` → Firefox
/// 4. `Safari/` sem `Chrome/` → Safari
/// 5. Fallback → Firefox
///
/// # Exemplos
///
/// ```
/// use duckduckgo_search_cli::http::{detectar_familia, FamiliaBrowser};
/// assert_eq!(detectar_familia("Mozilla/5.0 ... Chrome/146 ... Edg/145"), FamiliaBrowser::Edge);
/// assert_eq!(detectar_familia("Mozilla/5.0 ... Chrome/146 ..."), FamiliaBrowser::Chrome);
/// ```
pub fn detectar_familia(ua: &str) -> FamiliaBrowser {
    if ua.contains("Edg/") {
        FamiliaBrowser::Edge
    } else if ua.contains("Chrome/") {
        FamiliaBrowser::Chrome
    } else if ua.contains("Firefox/") {
        FamiliaBrowser::Firefox
    } else if ua.contains("Safari/") {
        FamiliaBrowser::Safari
    } else {
        FamiliaBrowser::Firefox
    }
}

/// Extrai a versão major do browser a partir do UA e da família detectada.
///
/// Padrões suportados: `Chrome/146`, `Firefox/134`, `Version/17` (Safari), `Edg/145`.
/// Retorna `0` se nenhum padrão for encontrado.
fn extrair_versao_major(ua: &str, familia: FamiliaBrowser) -> u32 {
    let prefixo = match familia {
        FamiliaBrowser::Chrome => "Chrome/",
        FamiliaBrowser::Firefox => "Firefox/",
        FamiliaBrowser::Safari => "Version/",
        FamiliaBrowser::Edge => "Edg/",
    };

    if let Some(pos) = ua.find(prefixo) {
        let resto = &ua[pos + prefixo.len()..];
        let num_str: String = resto.chars().take_while(|c| c.is_ascii_digit()).collect();
        return num_str.parse().unwrap_or(0);
    }
    0
}

/// Extrai a plataforma do UA e normaliza para o formato de Client Hints.
///
/// Mapeamentos:
/// - `Windows NT` → `"Windows"`
/// - `Macintosh` → `"macOS"`
/// - Fallback → `"Linux"`
fn extrair_plataforma_ua(ua: &str) -> String {
    if ua.contains("Windows NT") {
        "Windows".to_string()
    } else if ua.contains("Macintosh") {
        "macOS".to_string()
    } else {
        "Linux".to_string()
    }
}

/// Constrói um [`PerfilBrowser`] completo a partir de uma string User-Agent.
///
/// Combina `detectar_familia`, `extrair_versao_major` e `extrair_plataforma_ua`.
///
/// O perfil resultante emite automaticamente os headers `Sec-Fetch-*` e Client Hints
/// corretos para a família detectada — **não injete headers Sec-Fetch ou Accept
/// customizados sobre este perfil** (veja regra R33 em `AGENT_RULES.md`).
///
/// # Exemplos
///
/// ```
/// use duckduckgo_search_cli::http::{criar_perfil_browser, FamiliaBrowser};
///
/// // Chrome UA → família Chrome, versão major extraída, plataforma Linux
/// let ua_chrome = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 \
///                  (KHTML, like Gecko) Chrome/146.0.0.0 Safari/537.36";
/// let perfil = criar_perfil_browser(ua_chrome);
/// assert_eq!(perfil.familia, FamiliaBrowser::Chrome);
/// assert_eq!(perfil.versao_major, 146);
/// assert_eq!(perfil.plataforma_ua, "Linux");
///
/// // Edge UA → família Edge (Sec-CH-UA* headers emitidos automaticamente)
/// let ua_edge = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 \
///                (KHTML, like Gecko) Chrome/145.0.0.0 Safari/537.36 Edg/145.0.0.0";
/// let perfil_edge = criar_perfil_browser(ua_edge);
/// assert_eq!(perfil_edge.familia, FamiliaBrowser::Edge);
/// assert_eq!(perfil_edge.plataforma_ua, "Windows");
/// ```
pub fn criar_perfil_browser(ua: &str) -> PerfilBrowser {
    let familia = detectar_familia(ua);
    let versao_major = extrair_versao_major(ua, familia);
    let plataforma_ua = extrair_plataforma_ua(ua);
    PerfilBrowser {
        familia,
        user_agent: ua.to_string(),
        versao_major,
        plataforma_ua,
    }
}

impl PerfilBrowser {
    /// Gera os headers iniciais completos para o primeiro request da sessão.
    ///
    /// Inclui headers universais (Accept, Accept-Language, Accept-Encoding,
    /// Upgrade-Insecure-Requests, Sec-Fetch-*) e, para Chrome/Edge, Client Hints
    /// (Sec-CH-UA, Sec-CH-UA-Mobile, Sec-CH-UA-Platform, Cache-Control).
    ///
    /// # Argumentos
    /// * `idioma` — código de idioma BCP-47 (ex: `"pt"`, `"en"`).
    /// * `pais` — código de país ISO 3166-1 alpha-2 (ex: `"br"`, `"us"`).
    ///
    /// # Erros
    /// Retorna erro se algum valor de header contiver bytes inválidos.
    pub fn headers_iniciais(&self, idioma: &str, pais: &str) -> Result<HeaderMap> {
        let mut headers = HeaderMap::new();

        // Accept por família
        let accept_valor = match self.familia {
            FamiliaBrowser::Chrome | FamiliaBrowser::Edge => {
                "text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,image/apng,*/*;q=0.8"
            }
            FamiliaBrowser::Firefox => {
                "text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,*/*;q=0.8"
            }
            FamiliaBrowser::Safari => {
                "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8"
            }
        };
        headers.insert(ACCEPT, HeaderValue::from_static(accept_valor));

        // Accept-Language com q-values
        let idioma_lower = idioma.to_ascii_lowercase();
        let pais_upper = pais.to_ascii_uppercase();
        let accept_language = if idioma_lower == "en" {
            "en-US,en;q=0.9".to_string()
        } else {
            format!("{idioma_lower}-{pais_upper},{idioma_lower};q=0.9,en-US;q=0.8,en;q=0.7")
        };
        headers.insert(
            ACCEPT_LANGUAGE,
            HeaderValue::from_str(&accept_language)
                .context("Accept-Language contém caracteres inválidos")?,
        );

        // Accept-Encoding
        headers.insert(
            ACCEPT_ENCODING,
            HeaderValue::from_static("gzip, deflate, br"),
        );

        // Upgrade-Insecure-Requests
        headers.insert(
            HeaderName::from_static("upgrade-insecure-requests"),
            HeaderValue::from_static("1"),
        );

        // Sec-Fetch universais
        headers.insert(
            HeaderName::from_static("sec-fetch-dest"),
            HeaderValue::from_static("document"),
        );
        headers.insert(
            HeaderName::from_static("sec-fetch-mode"),
            HeaderValue::from_static("navigate"),
        );
        headers.insert(
            HeaderName::from_static("sec-fetch-site"),
            HeaderValue::from_static("none"),
        );
        headers.insert(
            HeaderName::from_static("sec-fetch-user"),
            HeaderValue::from_static("?1"),
        );

        // Client Hints — exclusivo Chrome e Edge
        if matches!(self.familia, FamiliaBrowser::Chrome | FamiliaBrowser::Edge) {
            let sec_ch_ua = match self.familia {
                FamiliaBrowser::Edge => format!(
                    r#""Chromium";v="{v}", "Microsoft Edge";v="{v}", "Not-A.Brand";v="99""#,
                    v = self.versao_major
                ),
                _ => format!(
                    r#""Chromium";v="{v}", "Google Chrome";v="{v}", "Not-A.Brand";v="99""#,
                    v = self.versao_major
                ),
            };
            headers.insert(
                HeaderName::from_static("sec-ch-ua"),
                HeaderValue::from_str(&sec_ch_ua)
                    .context("Sec-CH-UA contém caracteres inválidos")?,
            );
            headers.insert(
                HeaderName::from_static("sec-ch-ua-mobile"),
                HeaderValue::from_static("?0"),
            );
            let plataforma_quoted = format!(r#""{}""#, self.plataforma_ua);
            headers.insert(
                HeaderName::from_static("sec-ch-ua-platform"),
                HeaderValue::from_str(&plataforma_quoted)
                    .context("Sec-CH-UA-Platform contém caracteres inválidos")?,
            );
            headers.insert(CACHE_CONTROL, HeaderValue::from_static("max-age=0"));
        }

        Ok(headers)
    }

    /// Gera os headers para requests de paginação (mesma sessão, site já conhecido).
    ///
    /// Diferença em relação a `construir_headers`: `Sec-Fetch-Site` passa a ser
    /// `same-origin` em vez de `none`.
    pub fn headers_paginacao(&self) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(
            HeaderName::from_static("sec-fetch-dest"),
            HeaderValue::from_static("document"),
        );
        headers.insert(
            HeaderName::from_static("sec-fetch-mode"),
            HeaderValue::from_static("navigate"),
        );
        headers.insert(
            HeaderName::from_static("sec-fetch-site"),
            HeaderValue::from_static("same-origin"),
        );
        headers.insert(
            HeaderName::from_static("sec-fetch-user"),
            HeaderValue::from_static("?1"),
        );
        headers
    }
}

// ---------------------------------------------------------------------------
// Entry TOML do arquivo user-agents.toml externo
// ---------------------------------------------------------------------------

/// Entry TOML do arquivo `user-agents.toml` externo.
#[derive(Debug, Clone, Deserialize)]
struct AgenteTomlExterno {
    ua: String,
    #[serde(default = "plataforma_any")]
    platform: String,
    /// Campo opcional: família do browser (`"chrome"`, `"firefox"`, `"safari"`, `"edge"`).
    /// Se ausente, a família é detectada automaticamente em `criar_perfil_browser()`.
    #[serde(default)]
    #[allow(dead_code)]
    browser: Option<String>,
}

fn plataforma_any() -> String {
    "any".to_string()
}

#[derive(Debug, Clone, Deserialize)]
struct ArquivoUserAgents {
    #[serde(default)]
    agents: Vec<AgenteTomlExterno>,
}

// ---------------------------------------------------------------------------
// Carregamento de User-Agents
// ---------------------------------------------------------------------------

/// Carrega lista de User-Agents combinando arquivo externo (se existir) com defaults.
///
/// Se `corresponde_plataforma` é true, filtra por plataforma atual (`linux`/`macos`/`windows`)
/// OU `any`. Retorna SEMPRE lista não-vazia — em caso de falha, usa `USER_AGENTS_PADRAO`.
pub fn carregar_user_agents(corresponde_plataforma: bool) -> Vec<String> {
    let Some(caminho) = platform::caminho_user_agents_toml() else {
        tracing::debug!("sem diretório de config — usando UAs embutidos");
        return uas_padrao_como_vec();
    };

    let conteudo = match std::fs::read_to_string(&caminho) {
        Ok(c) => c,
        Err(erro) => {
            tracing::info!(
                caminho = %caminho.display(),
                ?erro,
                "user-agents.toml não encontrado — usando UAs embutidos"
            );
            return uas_padrao_como_vec();
        }
    };

    let arquivo: ArquivoUserAgents = match toml::from_str(&conteudo) {
        Ok(a) => a,
        Err(erro) => {
            tracing::warn!(
                caminho = %caminho.display(),
                ?erro,
                "user-agents.toml inválido — usando UAs embutidos"
            );
            return uas_padrao_como_vec();
        }
    };

    let plataforma_atual = platform::nome_plataforma();
    let filtrados: Vec<String> = arquivo
        .agents
        .into_iter()
        .filter(|a| {
            if !corresponde_plataforma {
                return true;
            }
            a.platform == "any" || a.platform == plataforma_atual
        })
        .map(|a| a.ua)
        .filter(|ua| !ua.is_empty())
        .collect();

    if filtrados.is_empty() {
        tracing::warn!("user-agents.toml não produziu nenhum UA aplicável — usando defaults");
        return uas_padrao_como_vec();
    }

    tracing::info!(
        caminho = %caminho.display(),
        total = filtrados.len(),
        corresponde_plataforma,
        "User-Agents carregados de user-agents.toml externo"
    );
    filtrados
}

fn uas_padrao_como_vec() -> Vec<String> {
    USER_AGENTS_PADRAO.iter().map(|s| s.to_string()).collect()
}

// ---------------------------------------------------------------------------
// Seleção de User-Agent / PerfilBrowser
// ---------------------------------------------------------------------------

/// Seleciona um User-Agent aleatório da lista embutida.
pub fn escolher_user_agent() -> String {
    let mut rng = rand::thread_rng();
    USER_AGENTS_PADRAO
        .choose(&mut rng)
        .copied()
        .unwrap_or(USER_AGENTS_PADRAO[0])
        .to_string()
}

/// Seleciona um User-Agent aleatório usando a lista fornecida (útil após `carregar_user_agents`).
///
/// Se a lista estiver vazia, cai de volta no default embutido.
pub fn escolher_user_agent_da_lista(lista: &[String]) -> String {
    let mut rng = rand::thread_rng();
    lista
        .choose(&mut rng)
        .cloned()
        .unwrap_or_else(escolher_user_agent)
}

/// Seleciona um [`PerfilBrowser`] aleatório a partir da lista fornecida.
///
/// Cada string da lista é convertida em [`PerfilBrowser`] via [`criar_perfil_browser`].
/// Se a lista estiver vazia, cria um perfil a partir do default embutido.
///
/// # Exemplos
///
/// ```
/// use duckduckgo_search_cli::http::{escolher_perfil_da_lista, FamiliaBrowser};
///
/// // Lista com um único UA Chrome → sempre retorna perfil Chrome
/// let lista = vec![
///     "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 \
///      (KHTML, like Gecko) Chrome/146.0.0.0 Safari/537.36"
///         .to_string(),
/// ];
/// let perfil = escolher_perfil_da_lista(&lista);
/// assert_eq!(perfil.familia, FamiliaBrowser::Chrome);
///
/// // Lista vazia → cai no default embutido (retorna algum perfil válido)
/// let perfil_default = escolher_perfil_da_lista(&[]);
/// // familia é um dos valores conhecidos de FamiliaBrowser
/// let _ = perfil_default.familia;
/// ```
pub fn escolher_perfil_da_lista(lista: &[String]) -> PerfilBrowser {
    let ua = escolher_user_agent_da_lista(lista);
    criar_perfil_browser(&ua)
}

/// Seleciona um User-Agent aleatório diferente do informado em `excluindo` (quando possível).
///
/// Usado pelo mecanismo de retry ao detectar HTTP 403 — rotacionar UA reduz a chance
/// de fingerprinting consistente. Se todas as UAs da lista coincidirem com `excluindo`
/// (ou a lista tiver um único item), retorna qualquer UA da lista.
pub fn selecionar_user_agent_aleatorio(excluindo: Option<&str>) -> String {
    let mut rng = rand::thread_rng();
    let candidatos: Vec<&&str> = USER_AGENTS_PADRAO
        .iter()
        .filter(|ua| match excluindo {
            Some(excl) => **ua != excl,
            None => true,
        })
        .collect();

    if candidatos.is_empty() {
        return escolher_user_agent();
    }

    candidatos
        .choose(&mut rng)
        .copied()
        .copied()
        .unwrap_or(USER_AGENTS_PADRAO[0])
        .to_string()
}

// ---------------------------------------------------------------------------
// Configuração de Proxy
// ---------------------------------------------------------------------------

/// Configuração de proxy para o cliente HTTP.
///
/// - `Nenhum` → reqwest respeita env vars HTTP_PROXY/HTTPS_PROXY/ALL_PROXY automaticamente.
/// - `Desabilitado` → `.no_proxy()` — ignora env vars.
/// - `Url(u)` → `Proxy::all(u)` com basic-auth extraído do userinfo, se presente.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfiguracaoProxy {
    Nenhum,
    Desabilitado,
    Url(String),
}

impl ConfiguracaoProxy {
    /// Constrói a configuração a partir das flags `--proxy` e `--no-proxy`.
    pub fn a_partir_de(proxy: Option<&str>, sem_proxy: bool) -> Self {
        if sem_proxy {
            return Self::Desabilitado;
        }
        match proxy {
            Some(u) if !u.is_empty() => Self::Url(u.to_string()),
            _ => Self::Nenhum,
        }
    }

    pub fn esta_ativo(&self) -> bool {
        matches!(self, Self::Url(_))
    }
}

// ---------------------------------------------------------------------------
// Construção do Client
// ---------------------------------------------------------------------------

/// Constrói um `reqwest::Client` pronto para fazer requests ao DuckDuckGo.
///
/// # Argumentos
/// * `user_agent` — string do User-Agent a ser enviada em todos os requests.
/// * `timeout_segundos` — timeout total (incluindo leitura do body).
/// * `idioma` — código de idioma para o header `Accept-Language` (ex: `"pt"`).
/// * `pais` — código de país para o header `Accept-Language` (ex: `"br"`).
///
/// # Erros
/// Retorna erro se o build do `ClientBuilder` falhar.
pub fn construir_cliente(
    user_agent: &str,
    timeout_segundos: u64,
    idioma: &str,
    pais: &str,
) -> Result<Client> {
    let perfil = criar_perfil_browser(user_agent);
    construir_cliente_com_proxy(
        &perfil,
        timeout_segundos,
        idioma,
        pais,
        &ConfiguracaoProxy::Nenhum,
    )
}

/// Mascara credenciais em uma URL de proxy para uso seguro em logs e mensagens de erro.
///
/// Transforma `http://user:password@proxy:8080` em `http://us***@proxy:8080`.
/// Se a URL não contiver credenciais, retorna a representação segura sem userinfo.
fn mascarar_url_proxy(url_bruta: &str) -> String {
    match reqwest::Url::parse(url_bruta) {
        Ok(parseada) => {
            let usuario = parseada.username();
            let tem_senha = parseada.password().is_some();

            if usuario.is_empty() && !tem_senha {
                return format!(
                    "{}://{}{}",
                    parseada.scheme(),
                    parseada.host_str().unwrap_or("?"),
                    parseada.port().map(|p| format!(":{p}")).unwrap_or_default()
                );
            }

            let usuario_mascarado = if usuario.len() > 2 {
                format!("{}***", &usuario[..2])
            } else {
                format!("{usuario}***")
            };

            format!(
                "{}://{}@{}{}",
                parseada.scheme(),
                usuario_mascarado,
                parseada.host_str().unwrap_or("?"),
                parseada.port().map(|p| format!(":{p}")).unwrap_or_default()
            )
        }
        Err(_) => "***URL_MALFORMADA***".to_string(),
    }
}

/// Constrói um `reqwest::Client` com perfil de browser e configuração de proxy.
///
/// Usa [`PerfilBrowser::headers_iniciais`] para gerar headers específicos por família,
/// incluindo Sec-Fetch completos e Client Hints (Chrome/Edge).
///
/// # Argumentos
/// * `perfil` — perfil do browser que define headers por família.
/// * `timeout_segundos` — timeout total.
/// * `idioma` — código de idioma (ex: `"pt"`).
/// * `pais` — código de país (ex: `"br"`).
/// * `proxy` — configuração de proxy.
///
/// # Erros
/// Retorna erro se os headers forem inválidos ou a configuração de proxy falhar.
pub fn construir_cliente_com_proxy(
    perfil: &PerfilBrowser,
    timeout_segundos: u64,
    idioma: &str,
    pais: &str,
    proxy: &ConfiguracaoProxy,
) -> Result<Client> {
    let headers = perfil
        .headers_iniciais(idioma, pais)
        .context("falha ao montar headers do perfil browser")?;

    let mut builder = Client::builder()
        .user_agent(&perfil.user_agent)
        .default_headers(headers)
        .cookie_store(true)
        .gzip(true)
        .brotli(true)
        .redirect(Policy::limited(5))
        .timeout(Duration::from_secs(timeout_segundos));

    match proxy {
        ConfiguracaoProxy::Nenhum => {}
        ConfiguracaoProxy::Desabilitado => {
            builder = builder.no_proxy();
            tracing::info!("proxy explicitamente desabilitado via --no-proxy");
        }
        ConfiguracaoProxy::Url(url) => {
            let parseada = reqwest::Url::parse(url)
                .with_context(|| format!("URL de proxy inválida: {}", mascarar_url_proxy(url)))?;
            let user = parseada.username().to_string();
            let senha = parseada
                .password()
                .map(|s| s.to_string())
                .unwrap_or_default();

            let mut proxy_rq = reqwest::Proxy::all(url).with_context(|| {
                format!(
                    "falha ao configurar Proxy::all({})",
                    mascarar_url_proxy(url)
                )
            })?;

            if !user.is_empty() {
                proxy_rq = proxy_rq.basic_auth(&user, &senha);
            }
            builder = builder.proxy(proxy_rq);
            tracing::info!(
                host = parseada.host_str(),
                scheme = parseada.scheme(),
                "proxy configurado"
            );
        }
    }

    let cliente = builder
        .build()
        .context("falha ao construir reqwest::Client com rustls-tls")?;

    Ok(cliente)
}

// ---------------------------------------------------------------------------
// Testes
// ---------------------------------------------------------------------------

#[cfg(test)]
mod testes {
    use super::*;

    // --- Testes existentes ---------------------------------------------------

    #[test]
    fn escolher_user_agent_retorna_string_nao_vazia() {
        let ua = escolher_user_agent();
        assert!(!ua.is_empty());
    }

    #[test]
    fn escolher_user_agent_retorna_ua_moderno_do_pool() {
        let ua = escolher_user_agent();
        assert!(
            USER_AGENTS_PADRAO.contains(&ua.as_str()),
            "UA selecionado deve estar na lista padrão: {ua}"
        );
        assert!(
            ua.starts_with("Mozilla/5.0 ("),
            "UAs padrão v0.3.0 iniciam com 'Mozilla/5.0 (' (browser real): {ua}"
        );
    }

    #[test]
    fn pool_padrao_contem_browsers_modernos_em_todas_as_familias() {
        let pool = USER_AGENTS_PADRAO;
        assert!(pool.iter().any(|ua| ua.contains("Chrome/")));
        assert!(pool.iter().any(|ua| ua.contains("Firefox/")));
        assert!(pool.iter().any(|ua| ua.contains("Edg/")));
        assert!(pool
            .iter()
            .any(|ua| ua.contains("Safari/") && !ua.contains("Chrome/")));
    }

    #[test]
    fn pool_padrao_nao_contem_browsers_de_texto_removidos() {
        for ua in USER_AGENTS_PADRAO {
            assert!(!ua.contains("Lynx"), "UA banido detectado (Lynx): {ua}");
            assert!(!ua.contains("w3m"), "UA banido detectado (w3m): {ua}");
            assert!(
                !ua.starts_with("Links ("),
                "UA banido detectado (Links): {ua}"
            );
            assert!(!ua.contains("ELinks"), "UA banido detectado (ELinks): {ua}");
            assert!(
                !ua.starts_with("duckduckgo-search-cli"),
                "UA banido detectado (self-cli): {ua}"
            );
            assert_ne!(
                *ua, "Mozilla/5.0",
                "UA minimalista 'Mozilla/5.0' deve ter sido removido"
            );
        }
        assert!(!USER_AGENTS_PADRAO.is_empty());
    }

    #[test]
    fn selecionar_user_agent_aleatorio_sem_exclusao_retorna_valido() {
        let ua = selecionar_user_agent_aleatorio(None);
        assert!(!ua.is_empty());
    }

    #[test]
    fn selecionar_user_agent_aleatorio_evita_excluido_quando_possivel() {
        let excluido = USER_AGENTS_PADRAO[0];
        for _ in 0..20 {
            let ua = selecionar_user_agent_aleatorio(Some(excluido));
            assert_ne!(ua, excluido);
            assert!(!ua.is_empty());
        }
    }

    #[test]
    fn construir_cliente_com_valores_validos_funciona() {
        let cliente = construir_cliente("Mozilla/5.0 teste", 15, "pt", "br");
        assert!(cliente.is_ok(), "cliente deve ser construído sem erro");
    }

    #[test]
    fn construir_cliente_com_proxy_http_funciona() {
        let perfil = criar_perfil_browser("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/146.0.0.0 Safari/537.36");
        let proxy = ConfiguracaoProxy::Url("http://user:pass@proxy.local:8080".to_string());
        let cliente = construir_cliente_com_proxy(&perfil, 10, "pt", "br", &proxy);
        assert!(cliente.is_ok(), "cliente com proxy HTTP deve construir");
    }

    #[test]
    fn construir_cliente_com_proxy_socks5_funciona() {
        let perfil = criar_perfil_browser(
            "Mozilla/5.0 (X11; Linux x86_64; rv:134.0) Gecko/20100101 Firefox/134.0",
        );
        let proxy = ConfiguracaoProxy::Url("socks5://127.0.0.1:9050".to_string());
        let cliente = construir_cliente_com_proxy(&perfil, 10, "pt", "br", &proxy);
        assert!(cliente.is_ok(), "cliente com SOCKS5 deve construir");
    }

    #[test]
    fn construir_cliente_com_no_proxy_funciona() {
        let perfil = criar_perfil_browser(
            "Mozilla/5.0 (X11; Linux x86_64; rv:134.0) Gecko/20100101 Firefox/134.0",
        );
        let proxy = ConfiguracaoProxy::Desabilitado;
        let cliente = construir_cliente_com_proxy(&perfil, 10, "pt", "br", &proxy);
        assert!(cliente.is_ok(), "cliente com no_proxy deve construir");
    }

    #[test]
    fn construir_cliente_com_proxy_url_invalida_falha() {
        let perfil = criar_perfil_browser(
            "Mozilla/5.0 (X11; Linux x86_64; rv:134.0) Gecko/20100101 Firefox/134.0",
        );
        let proxy = ConfiguracaoProxy::Url("nao eh uma url".to_string());
        let cliente = construir_cliente_com_proxy(&perfil, 10, "pt", "br", &proxy);
        assert!(cliente.is_err(), "URL inválida deve rejeitar");
    }

    #[test]
    fn configuracao_proxy_a_partir_de_flags() {
        assert_eq!(
            ConfiguracaoProxy::a_partir_de(None, false),
            ConfiguracaoProxy::Nenhum
        );
        assert_eq!(
            ConfiguracaoProxy::a_partir_de(None, true),
            ConfiguracaoProxy::Desabilitado
        );
        assert_eq!(
            ConfiguracaoProxy::a_partir_de(Some("http://x:9"), false),
            ConfiguracaoProxy::Url("http://x:9".to_string())
        );
        assert_eq!(
            ConfiguracaoProxy::a_partir_de(Some("http://x:9"), true),
            ConfiguracaoProxy::Desabilitado
        );
    }

    #[test]
    fn configuracao_proxy_esta_ativo_so_em_url() {
        assert!(!ConfiguracaoProxy::Nenhum.esta_ativo());
        assert!(!ConfiguracaoProxy::Desabilitado.esta_ativo());
        assert!(ConfiguracaoProxy::Url("http://x".to_string()).esta_ativo());
    }

    #[test]
    fn mascara_url_proxy_com_credenciais() {
        let resultado = mascarar_url_proxy("http://admin:s3cret@proxy.local:8080");
        assert!(!resultado.contains("s3cret"), "password vazou: {resultado}");
        assert!(
            !resultado.contains("admin"),
            "username completo vazou: {resultado}"
        );
        assert!(
            resultado.contains("ad***"),
            "username mascarado ausente: {resultado}"
        );
        assert!(resultado.contains("proxy.local"));
        assert!(resultado.contains("8080"));
    }

    #[test]
    fn mascara_url_proxy_sem_credenciais() {
        let resultado = mascarar_url_proxy("http://proxy.local:8080");
        assert_eq!(resultado, "http://proxy.local:8080");
    }

    #[test]
    fn mascara_url_proxy_so_username() {
        let resultado = mascarar_url_proxy("http://user@proxy.local:3128");
        assert!(resultado.contains("us***"));
        assert!(!resultado.contains("user@"));
    }

    #[test]
    fn mascara_url_proxy_malformada() {
        let resultado = mascarar_url_proxy("not-a-url");
        assert_eq!(resultado, "***URL_MALFORMADA***");
    }

    #[test]
    fn mascara_url_proxy_socks5() {
        let resultado = mascarar_url_proxy("socks5://root:toor@127.0.0.1:1080");
        assert!(!resultado.contains("toor"));
        assert!(resultado.contains("socks5://"));
        assert!(resultado.contains("127.0.0.1"));
    }

    #[test]
    fn mascara_url_proxy_username_curto() {
        let resultado = mascarar_url_proxy("http://a:pass@proxy:80");
        assert!(resultado.contains("a***"));
        assert!(!resultado.contains("pass"));
    }

    #[test]
    fn carregar_user_agents_retorna_pelo_menos_um_default() {
        let lista = carregar_user_agents(false);
        assert!(!lista.is_empty());
        for ua in &lista {
            assert!(!ua.is_empty());
        }
    }

    #[test]
    fn escolher_user_agent_da_lista_retorna_item_da_lista() {
        let lista = vec!["A".to_string(), "B".to_string(), "C".to_string()];
        for _ in 0..10 {
            let escolhido = escolher_user_agent_da_lista(&lista);
            assert!(lista.contains(&escolhido));
        }
    }

    // --- Testes novos: PerfilBrowser -----------------------------------------

    #[test]
    fn detectar_familia_chrome() {
        let uas_chrome = [
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/146.0.0.0 Safari/537.36",
            "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/146.0.0.0 Safari/537.36",
            "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/146.0.0.0 Safari/537.36",
        ];
        for ua in &uas_chrome {
            assert_eq!(
                detectar_familia(ua),
                FamiliaBrowser::Chrome,
                "esperado Chrome para: {ua}"
            );
        }
    }

    #[test]
    fn detectar_familia_edge_antes_de_chrome() {
        // Edge UA contém "Chrome/" mas deve retornar Edge por ter "Edg/" primeiro
        let ua = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/145.0.0.0 Safari/537.36 Edg/145.0.3800.97";
        assert_eq!(detectar_familia(ua), FamiliaBrowser::Edge);
    }

    #[test]
    fn detectar_familia_firefox() {
        let ua = "Mozilla/5.0 (X11; Linux x86_64; rv:134.0) Gecko/20100101 Firefox/134.0";
        assert_eq!(detectar_familia(ua), FamiliaBrowser::Firefox);
    }

    #[test]
    fn detectar_familia_safari() {
        // Safari puro não contém "Chrome/"
        let ua = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.6 Safari/605.1.15";
        assert_eq!(detectar_familia(ua), FamiliaBrowser::Safari);
    }

    #[test]
    fn extrair_versao_major_chrome_146() {
        let ua = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/146.0.0.0 Safari/537.36";
        let versao = extrair_versao_major(ua, FamiliaBrowser::Chrome);
        assert_eq!(versao, 146, "versão major Chrome deve ser 146");
    }

    #[test]
    fn extrair_versao_major_firefox_134() {
        let ua = "Mozilla/5.0 (X11; Linux x86_64; rv:134.0) Gecko/20100101 Firefox/134.0";
        let versao = extrair_versao_major(ua, FamiliaBrowser::Firefox);
        assert_eq!(versao, 134, "versão major Firefox deve ser 134");
    }

    #[test]
    fn headers_iniciais_chrome_inclui_sec_fetch() {
        let ua = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/146.0.0.0 Safari/537.36";
        let perfil = criar_perfil_browser(ua);
        let headers = perfil
            .headers_iniciais("pt", "br")
            .expect("deve montar headers");
        assert!(
            headers.contains_key("sec-fetch-dest"),
            "sec-fetch-dest ausente"
        );
        assert!(
            headers.contains_key("sec-fetch-mode"),
            "sec-fetch-mode ausente"
        );
        assert!(
            headers.contains_key("sec-fetch-site"),
            "sec-fetch-site ausente"
        );
        assert!(
            headers.contains_key("sec-fetch-user"),
            "sec-fetch-user ausente"
        );
    }

    #[test]
    fn headers_iniciais_chrome_inclui_client_hints() {
        let ua = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/146.0.0.0 Safari/537.36";
        let perfil = criar_perfil_browser(ua);
        let headers = perfil
            .headers_iniciais("pt", "br")
            .expect("deve montar headers");
        assert!(headers.contains_key("sec-ch-ua"), "sec-ch-ua ausente");
        assert!(
            headers.contains_key("sec-ch-ua-mobile"),
            "sec-ch-ua-mobile ausente"
        );
        assert!(
            headers.contains_key("sec-ch-ua-platform"),
            "sec-ch-ua-platform ausente"
        );
    }

    #[test]
    fn headers_iniciais_firefox_omite_client_hints() {
        let ua = "Mozilla/5.0 (X11; Linux x86_64; rv:134.0) Gecko/20100101 Firefox/134.0";
        let perfil = criar_perfil_browser(ua);
        let headers = perfil
            .headers_iniciais("pt", "br")
            .expect("deve montar headers");
        assert!(
            !headers.contains_key("sec-ch-ua"),
            "Firefox NÃO deve ter sec-ch-ua"
        );
        assert!(
            !headers.contains_key("sec-ch-ua-mobile"),
            "Firefox NÃO deve ter sec-ch-ua-mobile"
        );
        assert!(
            !headers.contains_key("sec-ch-ua-platform"),
            "Firefox NÃO deve ter sec-ch-ua-platform"
        );
    }

    #[test]
    fn headers_paginacao_sec_fetch_site_same_origin() {
        let ua = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/146.0.0.0 Safari/537.36";
        let perfil = criar_perfil_browser(ua);
        let headers = perfil.headers_paginacao();
        let valor = headers
            .get("sec-fetch-site")
            .expect("sec-fetch-site deve estar presente");
        assert_eq!(valor.to_str().unwrap(), "same-origin");
    }

    #[test]
    fn accept_language_com_q_values_pt() {
        let ua = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/146.0.0.0 Safari/537.36";
        let perfil = criar_perfil_browser(ua);
        let headers = perfil
            .headers_iniciais("pt", "br")
            .expect("deve montar headers");
        let al = headers
            .get(ACCEPT_LANGUAGE)
            .expect("Accept-Language presente");
        let al_str = al.to_str().unwrap();
        assert!(al_str.contains("pt-BR"), "deve conter pt-BR: {al_str}");
        assert!(
            al_str.contains("pt;q=0.9"),
            "deve conter pt;q=0.9: {al_str}"
        );
        assert!(
            al_str.contains("en-US;q=0.8"),
            "deve conter en-US;q=0.8: {al_str}"
        );
        assert!(
            al_str.contains("en;q=0.7"),
            "deve conter en;q=0.7: {al_str}"
        );
    }

    #[test]
    fn accept_language_com_q_values_en() {
        let ua = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/146.0.0.0 Safari/537.36";
        let perfil = criar_perfil_browser(ua);
        let headers = perfil
            .headers_iniciais("en", "us")
            .expect("deve montar headers");
        let al = headers
            .get(ACCEPT_LANGUAGE)
            .expect("Accept-Language presente");
        let al_str = al.to_str().unwrap();
        assert_eq!(
            al_str, "en-US,en;q=0.9",
            "formato en deve ser simplificado: {al_str}"
        );
    }

    // Testes existentes atualizados para usar PerfilBrowser

    #[test]
    fn headers_padrao_inclui_accept_e_idioma() {
        // Teste atualizado para usar PerfilBrowser em vez de headers_padrao()
        let ua = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/146.0.0.0 Safari/537.36";
        let perfil = criar_perfil_browser(ua);
        let headers = perfil
            .headers_iniciais("pt", "br")
            .expect("deve montar headers");
        let accept = headers.get(ACCEPT).expect("ACCEPT presente");
        assert!(accept.to_str().unwrap().contains("text/html"));
        let al = headers
            .get(ACCEPT_LANGUAGE)
            .expect("ACCEPT_LANGUAGE presente");
        assert!(al.to_str().unwrap().contains("pt-BR"));
    }

    #[test]
    fn headers_padrao_omite_dnt_e_referer() {
        // Descoberta empírica iter. 4: DNT + Referer permanente delatam fingerprint.
        // Atualizado para usar PerfilBrowser.
        let ua = "Mozilla/5.0 (X11; Linux x86_64; rv:134.0) Gecko/20100101 Firefox/134.0";
        let perfil = criar_perfil_browser(ua);
        let headers = perfil
            .headers_iniciais("en", "us")
            .expect("deve montar headers");
        assert!(headers.get(reqwest::header::DNT).is_none());
        assert!(headers.get(reqwest::header::REFERER).is_none());
    }
}
