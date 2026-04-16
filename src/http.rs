//! Construção do `reqwest::Client` e seleção de User-Agent.
//!
//! O cliente HTTP é configurado com:
//! - TLS via `rustls-tls` (sem dependência de OpenSSL em nenhuma plataforma).
//! - Cookie store habilitado (necessário para paginação com token `vqd`).
//! - Compressão `gzip` + `brotli` (reduz bandwidth).
//! - Redirect policy limitada a 5 saltos.
//! - Headers default que identificam a CLI como um browser real.
//! - Timeout total configurável.
//! - Proxy opcional HTTP/HTTPS/SOCKS5 (iteração 5).
//! - User-Agents carregados de `user-agents.toml` externo OU defaults embutidos.

use crate::platform;
use anyhow::{Context, Result};
use rand::seq::SliceRandom;
use reqwest::{
    header::{HeaderMap, HeaderValue, ACCEPT, ACCEPT_LANGUAGE},
    redirect::Policy,
    Client,
};
use serde::Deserialize;
use std::time::Duration;

/// Lista de User-Agents embutida no binário para fallback caso `config/user-agents.toml`
/// não esteja disponível. Os mesmos valores estão no arquivo TOML em `config/`.
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
/// passa porque é desktop minoritário — o filtro do DDG é menos agressivo. Para
/// evitar ~25% de requests bloqueados, MANTIVEMOS apenas Firefox Linux.
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

/// Entry TOML do arquivo `user-agents.toml` externo.
#[derive(Debug, Clone, Deserialize)]
struct AgenteTomlExterno {
    ua: String,
    #[serde(default = "plataforma_any")]
    platform: String,
}

fn plataforma_any() -> String {
    "any".to_string()
}

#[derive(Debug, Clone, Deserialize)]
struct ArquivoUserAgents {
    #[serde(default)]
    agents: Vec<AgenteTomlExterno>,
}

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
        // Nenhum candidato diferente — retorna qualquer da lista.
        return escolher_user_agent();
    }

    candidatos
        .choose(&mut rng)
        .copied()
        .copied()
        .unwrap_or(USER_AGENTS_PADRAO[0])
        .to_string()
}

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

/// Constrói um `reqwest::Client` pronto para fazer requests ao DuckDuckGo.
///
/// # Argumentos
/// * `user_agent` — string do User-Agent a ser enviada em todos os requests.
/// * `timeout_segundos` — timeout total (incluindo leitura do body).
/// * `idioma` — código de idioma para o header `Accept-Language` (ex: `"pt"`).
/// * `pais` — código de país para o header `Accept-Language` (ex: `"br"`).
///
/// # Erros
/// Retorna erro se o clone/build do `ClientBuilder` falhar (extremamente raro,
/// tipicamente indica configuração inválida do rustls no ambiente).
pub fn construir_cliente(
    user_agent: &str,
    timeout_segundos: u64,
    idioma: &str,
    pais: &str,
) -> Result<Client> {
    construir_cliente_com_proxy(
        user_agent,
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
                // Sem credenciais — retorna scheme + host + port
                return format!(
                    "{}://{}{}",
                    parseada.scheme(),
                    parseada.host_str().unwrap_or("?"),
                    parseada.port().map(|p| format!(":{p}")).unwrap_or_default()
                );
            }

            // Mascara: primeiros 2 chars do username + *** (password sempre oculto)
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

/// Variante que aceita configuração de proxy.
///
/// Nota sobre basic-auth: quando a URL do proxy contém `user:pass@host:port`, o
/// `reqwest::Proxy::all(url)` já propaga essa informação ao construir o tunnel.
/// Aqui adicionamos `.basic_auth()` adicionalmente para garantir que o header
/// `Proxy-Authorization` seja enviado em requests HTTP pelo cliente (reqwest
/// faz isso nativamente em tunneling HTTPS, mas não em GET HTTP puro).
pub fn construir_cliente_com_proxy(
    user_agent: &str,
    timeout_segundos: u64,
    idioma: &str,
    pais: &str,
    proxy: &ConfiguracaoProxy,
) -> Result<Client> {
    let headers = headers_padrao(idioma, pais).context("falha ao montar headers default")?;

    let mut builder = Client::builder()
        .user_agent(user_agent)
        .default_headers(headers)
        .cookie_store(true)
        .gzip(true)
        .brotli(true)
        .redirect(Policy::limited(5))
        .timeout(Duration::from_secs(timeout_segundos));

    match proxy {
        ConfiguracaoProxy::Nenhum => {
            // reqwest lê HTTP_PROXY/HTTPS_PROXY/ALL_PROXY automaticamente
        }
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

            // Adiciona Proxy-Authorization se o userinfo foi explicitado.
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

/// Monta os headers default que acompanham todos os requests da sessão.
///
/// IMPORTANTE — descoberta empírica em 2026-04-14:
/// Headers "completos" tipo browser (Accept detalhado + Accept-Language com
/// q-values + DNT + Referer permanente) somados a UAs Chrome/Firefox são
/// detectados como bot pelo DDG e bloqueados com 202 anomaly. Mantemos apenas
/// `Accept-Language` simples para preservar localização (kl funciona melhor com
/// hint correto), e omitimos os demais. O `Referer` é adicionado apenas na
/// paginação (POST), feita explicitamente em `search.rs`.
fn headers_padrao(idioma: &str, pais: &str) -> Result<HeaderMap> {
    let mut headers = HeaderMap::new();

    headers.insert(ACCEPT, HeaderValue::from_static("text/html, */*;q=0.5"));

    // Accept-Language minimalista: ex "pt-BR" ou "en-US".
    let pais_upper = pais.to_ascii_uppercase();
    let idioma_lower = idioma.to_ascii_lowercase();
    let accept_language = format!("{idioma_lower}-{pais_upper}");
    let accept_language_value = HeaderValue::from_str(&accept_language)
        .context("Accept-Language contém caracteres inválidos")?;
    headers.insert(ACCEPT_LANGUAGE, accept_language_value);

    Ok(headers)
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn escolher_user_agent_retorna_string_nao_vazia() {
        let ua = escolher_user_agent();
        assert!(!ua.is_empty());
    }

    #[test]
    fn escolher_user_agent_retorna_ua_moderno_do_pool() {
        // v0.3.0: o pool agora contém UAs de browsers modernos (Chrome, Firefox,
        // Edge, Safari) — os antigos UAs minimalistas (Lynx, w3m, Links, ELinks,
        // "Mozilla/5.0") foram REMOVIDOS porque o endpoint /html/ do DDG serve
        // HTML degradado para esses agentes.
        let ua = escolher_user_agent();
        assert!(
            USER_AGENTS_PADRAO.contains(&ua.as_str()),
            "UA selecionado deve estar na lista padrão: {ua}"
        );
        // Todo UA default deve parecer um browser real moderno.
        assert!(
            ua.starts_with("Mozilla/5.0 ("),
            "UAs padrão v0.3.0 iniciam com 'Mozilla/5.0 (' (browser real): {ua}"
        );
    }

    #[test]
    fn pool_padrao_contem_browsers_modernos_em_todas_as_familias() {
        // Garante que há pelo menos um UA de Chrome, Firefox, Edge e Safari.
        let pool = USER_AGENTS_PADRAO;
        assert!(
            pool.iter().any(|ua| ua.contains("Chrome/")),
            "pool deve conter ao menos um Chrome"
        );
        assert!(
            pool.iter().any(|ua| ua.contains("Firefox/")),
            "pool deve conter ao menos um Firefox"
        );
        assert!(
            pool.iter().any(|ua| ua.contains("Edg/")),
            "pool deve conter ao menos um Edge"
        );
        assert!(
            pool.iter()
                .any(|ua| ua.contains("Safari/") && !ua.contains("Chrome/")),
            "pool deve conter ao menos um Safari puro"
        );
    }

    #[test]
    fn pool_padrao_nao_contem_browsers_de_texto_removidos() {
        // v0.3.0: removidos Lynx, w3m, Links, ELinks, "duckduckgo-search-cli/*"
        // e "Mozilla/5.0" minimalista — retornavam HTML degradado.
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
        assert!(!USER_AGENTS_PADRAO.is_empty(), "pool nunca pode ser vazio");
    }

    #[test]
    fn selecionar_user_agent_aleatorio_sem_exclusao_retorna_valido() {
        let ua = selecionar_user_agent_aleatorio(None);
        assert!(!ua.is_empty());
    }

    #[test]
    fn selecionar_user_agent_aleatorio_evita_excluido_quando_possivel() {
        // Tenta várias vezes para alta probabilidade de observar rotação.
        let excluido = USER_AGENTS_PADRAO[0];
        for _ in 0..20 {
            let ua = selecionar_user_agent_aleatorio(Some(excluido));
            assert_ne!(ua, excluido, "rotação deve evitar UA excluído");
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
        let proxy = ConfiguracaoProxy::Url("http://user:pass@proxy.local:8080".to_string());
        let cliente = construir_cliente_com_proxy("Mozilla/5.0", 10, "pt", "br", &proxy);
        assert!(cliente.is_ok(), "cliente com proxy HTTP deve construir");
    }

    #[test]
    fn construir_cliente_com_proxy_socks5_funciona() {
        let proxy = ConfiguracaoProxy::Url("socks5://127.0.0.1:9050".to_string());
        let cliente = construir_cliente_com_proxy("Mozilla/5.0", 10, "pt", "br", &proxy);
        assert!(cliente.is_ok(), "cliente com SOCKS5 deve construir");
    }

    #[test]
    fn construir_cliente_com_no_proxy_funciona() {
        let proxy = ConfiguracaoProxy::Desabilitado;
        let cliente = construir_cliente_com_proxy("Mozilla/5.0", 10, "pt", "br", &proxy);
        assert!(cliente.is_ok(), "cliente com no_proxy deve construir");
    }

    #[test]
    fn construir_cliente_com_proxy_url_invalida_falha() {
        let proxy = ConfiguracaoProxy::Url("nao eh uma url".to_string());
        let cliente = construir_cliente_com_proxy("Mozilla/5.0", 10, "pt", "br", &proxy);
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
        // --no-proxy tem precedência lógica se ambos forem passados, mas o clap já
        // garante exclusividade. O método simplesmente segue a flag.
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
    fn headers_padrao_inclui_accept_e_idioma() {
        let headers = headers_padrao("pt", "br").expect("deve montar headers");
        let accept = headers.get(ACCEPT).expect("ACCEPT presente");
        assert!(accept.to_str().unwrap().contains("text/html"));
        let al = headers
            .get(ACCEPT_LANGUAGE)
            .expect("ACCEPT_LANGUAGE presente");
        assert_eq!(al.to_str().unwrap(), "pt-BR");
    }

    #[test]
    fn headers_padrao_omite_dnt_e_referer() {
        // Descoberta empírica iter. 4: DNT + Referer permanente delatam fingerprint
        // browser-like e ajudam o DDG a flagar como bot. Devem estar ausentes do
        // header default; Referer só é adicionado em paginação POST explícita.
        let headers = headers_padrao("en", "us").expect("deve montar headers");
        assert!(headers.get(reqwest::header::DNT).is_none());
        assert!(headers.get(reqwest::header::REFERER).is_none());
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
        assert!(
            resultado.contains("proxy.local"),
            "host ausente: {resultado}"
        );
        assert!(resultado.contains("8080"), "porta ausente: {resultado}");
    }

    #[test]
    fn mascara_url_proxy_sem_credenciais() {
        let resultado = mascarar_url_proxy("http://proxy.local:8080");
        assert_eq!(resultado, "http://proxy.local:8080");
    }

    #[test]
    fn mascara_url_proxy_so_username() {
        let resultado = mascarar_url_proxy("http://user@proxy.local:3128");
        assert!(
            resultado.contains("us***"),
            "username mascarado ausente: {resultado}"
        );
        assert!(
            !resultado.contains("user@"),
            "username completo vazou: {resultado}"
        );
    }

    #[test]
    fn mascara_url_proxy_malformada() {
        let resultado = mascarar_url_proxy("not-a-url");
        assert_eq!(resultado, "***URL_MALFORMADA***");
    }

    #[test]
    fn mascara_url_proxy_socks5() {
        let resultado = mascarar_url_proxy("socks5://root:toor@127.0.0.1:1080");
        assert!(!resultado.contains("toor"), "password vazou: {resultado}");
        assert!(
            resultado.contains("socks5://"),
            "scheme ausente: {resultado}"
        );
        assert!(resultado.contains("127.0.0.1"), "host ausente: {resultado}");
    }

    #[test]
    fn mascara_url_proxy_username_curto() {
        let resultado = mascarar_url_proxy("http://a:pass@proxy:80");
        assert!(
            resultado.contains("a***"),
            "username curto mascarado: {resultado}"
        );
        assert!(!resultado.contains("pass"), "password vazou: {resultado}");
    }

    #[test]
    fn carregar_user_agents_retorna_pelo_menos_um_default() {
        // Em ambientes sem config externo, deve retornar defaults embutidos não-vazios.
        let lista = carregar_user_agents(false);
        assert!(!lista.is_empty(), "lista de UAs nunca deve ser vazia");
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
}
