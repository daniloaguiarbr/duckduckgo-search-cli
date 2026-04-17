//! URL construction and search request execution for DuckDuckGo.
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
//! because DuckDuckGo treats `/html` (without slash) as a redirect.

use crate::extraction;
use crate::types::{Configuracoes, Endpoint, FiltroTemporal, ResultadoBusca, SafeSearch};
use anyhow::{bail, Context, Result};
use rand::Rng;
use reqwest::{Client, Response, StatusCode};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio_util::sync::CancellationToken;

/// Default base URL for the DuckDuckGo HTML endpoint.
const URL_ENDPOINT_HTML_DEFAULT: &str = "https://html.duckduckgo.com/html/";
/// Default base URL for the DuckDuckGo Lite endpoint.
const URL_ENDPOINT_LITE_DEFAULT: &str = "https://lite.duckduckgo.com/lite/";

/// Name of the environment variable that overrides the HTML endpoint URL (for tests).
const ENV_BASE_URL_HTML: &str = "DUCKDUCKGO_SEARCH_CLI_BASE_URL_HTML";
/// Name of the environment variable that overrides the Lite endpoint URL (for tests).
const ENV_BASE_URL_LITE: &str = "DUCKDUCKGO_SEARCH_CLI_BASE_URL_LITE";

/// Minimum delay between consecutive pages (ms).
/// v0.6.0: increased from 500 to 800ms to reduce anti-bot detection.
const DELAY_PAGINACAO_MIN_MS: u64 = 800;
/// Maximum delay between consecutive pages (ms).
/// v0.6.0: increased from 1000 to 1500ms to reduce anti-bot detection.
const DELAY_PAGINACAO_MAX_MS: u64 = 1500;

/// Byte threshold for silent block detection.
/// Real DuckDuckGo responses with results are 50-200KB.
/// Silent block pages are typically ~3KB.
const LIMIAR_BLOQUEIO_SILENCIOSO: usize = 5_000;

/// Base backoff for retry on 429 (ms). Total = base * 2^attempt + jitter.
const BACKOFF_BASE_MS: u64 = 1000;
/// Maximum additional jitter in backoff (ms).
const BACKOFF_JITTER_MAX_MS: u64 = 500;

/// Calculates the exponential backoff delay with jitter for the given attempt.
///
/// `tentativa` is 0-based. The exponent is capped at 10 (`2^10 = 1024`) to
/// avoid overflow without needing `checked_shl`.
fn calcular_backoff_ms(tentativa: u32) -> u64 {
    let fator = 1u64 << tentativa.min(10);
    let backoff = BACKOFF_BASE_MS.saturating_mul(fator);
    let jitter = rand::thread_rng().gen_range(0..=BACKOFF_JITTER_MAX_MS);
    backoff.saturating_add(jitter)
}

/// Returns the effective base URL for the HTML endpoint (respects env var in tests).
pub fn url_base_html() -> String {
    std::env::var(ENV_BASE_URL_HTML).unwrap_or_else(|_| URL_ENDPOINT_HTML_DEFAULT.to_string())
}

/// Returns the effective base URL for the Lite endpoint (respects env var in tests).
pub fn url_base_lite() -> String {
    std::env::var(ENV_BASE_URL_LITE).unwrap_or_else(|_| URL_ENDPOINT_LITE_DEFAULT.to_string())
}

/// Builds the GET search URL with the appropriate query-string for a given endpoint.
///
/// Parameters:
/// - `q` — search query (URL-encoded).
/// - `kl` — region, format `{country}-{language}`.
/// - `kp` — safe-search (when present).
/// - `df` — time filter (when present).
pub fn construir_url_busca(
    query: &str,
    idioma: &str,
    pais: &str,
    endpoint: Endpoint,
    filtro_temporal: Option<FiltroTemporal>,
    safe_search: SafeSearch,
) -> String {
    let base = match endpoint {
        Endpoint::Html => url_base_html(),
        Endpoint::Lite => url_base_lite(),
    };
    let query_encoded = urlencoding::encode(query);
    let kl = formatar_kl(idioma, pais);
    let mut url = format!("{base}?q={query_encoded}&kl={kl}");
    if let Some(kp) = safe_search.como_parametro() {
        url.push_str("&kp=");
        url.push_str(kp);
    }
    if let Some(df) = filtro_temporal {
        url.push_str("&df=");
        url.push_str(df.como_parametro());
    }
    url
}

/// Simplified version from iteration 1 — kept for backward compatibility with older tests.
pub fn construir_url(query: &str, idioma: &str, pais: &str) -> String {
    construir_url_busca(
        query,
        idioma,
        pais,
        Endpoint::Html,
        None,
        SafeSearch::Moderate,
    )
}

/// Formats the DuckDuckGo `kl` parameter as `{country}-{language}` in lowercase.
///
/// DuckDuckGo expects `kl` with the country in lowercase, followed by a hyphen and language
/// in lowercase. Uppercase inputs are normalized.
///
/// # Exemplo
///
/// ```
/// use duckduckgo_search_cli::search::formatar_kl;
///
/// assert_eq!(formatar_kl("pt", "br"), "br-pt");
/// assert_eq!(formatar_kl("EN", "US"), "us-en"); // normalizes uppercase input
/// ```
pub fn formatar_kl(idioma: &str, pais: &str) -> String {
    format!(
        "{}-{}",
        pais.to_ascii_lowercase(),
        idioma.to_ascii_lowercase()
    )
}

/// Specific errors returned by `executar_com_retry`.
///
/// Used so the pipeline can tag queries with structured error codes
/// instead of a generic message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MotivoFalhaRetry {
    /// Persistent rate limit after exhausting retries (HTTP 429).
    RateLimited,
    /// Persistent block after exhausting retries (HTTP 403).
    Blocked,
    /// Non-recoverable HTTP error (4xx/5xx status other than 403/429).
    HttpErro(u16),
    /// Timeout after 1 retry attempt.
    Timeout,
    /// Generic network error.
    Rede(String),
}

impl MotivoFalhaRetry {
    /// Maps to the structured error code in `error::codigos`.
    pub fn como_codigo_erro(&self) -> &'static str {
        match self {
            MotivoFalhaRetry::RateLimited => crate::error::codigos::RATE_LIMITED,
            MotivoFalhaRetry::Blocked => crate::error::codigos::BLOCKED,
            MotivoFalhaRetry::HttpErro(_) => crate::error::codigos::HTTP_ERROR,
            MotivoFalhaRetry::Timeout => crate::error::codigos::TIMEOUT,
            MotivoFalhaRetry::Rede(_) => crate::error::codigos::NETWORK_ERROR,
        }
    }

    pub fn mensagem(&self) -> String {
        match self {
            MotivoFalhaRetry::RateLimited => "rate limit persistente (HTTP 429)".to_string(),
            MotivoFalhaRetry::Blocked => "bloqueado pelo DuckDuckGo (HTTP 403)".to_string(),
            MotivoFalhaRetry::HttpErro(status) => format!("HTTP {status} não recuperável"),
            MotivoFalhaRetry::Timeout => "timeout persistente".to_string(),
            MotivoFalhaRetry::Rede(msg) => format!("erro de rede: {msg}"),
        }
    }
}

/// Result of `executar_com_retry`: either the HTTP response + total attempts, or the failure reason.
#[derive(Debug)]
pub struct ResultadoRetry {
    pub resposta: Response,
    pub tentativas: u32,
}

/// Executes a GET request with retry and backoff. Parameters:
/// * `cliente` — reqwest client (shared).
/// * `url` — full target URL.
/// * `retries` — number of additional retries (0..=10). 0 = single attempt only.
/// * `flag_rate_limit` — signals to other tasks that rate limiting was detected.
pub async fn executar_com_retry(
    cliente: &Client,
    url: &str,
    retries: u32,
    flag_rate_limit: &Arc<AtomicBool>,
    cancelamento: &CancellationToken,
) -> std::result::Result<ResultadoRetry, MotivoFalhaRetry> {
    let total_tentativas = retries.saturating_add(1);
    let mut ultimo_motivo = MotivoFalhaRetry::Rede("nenhuma tentativa executada".to_string());
    let mut timeout_ja_retentado = false;

    for tentativa in 0..total_tentativas {
        if cancelamento.is_cancelled() {
            return Err(MotivoFalhaRetry::Rede("cancelado".to_string()));
        }

        // Se o rate-limit global foi acionado por outra task, aplica delay extra.
        if flag_rate_limit.load(Ordering::Relaxed) && tentativa == 0 {
            let extra_ms = rand::thread_rng().gen_range(500..1200);
            tracing::debug!(
                extra_ms,
                "flag rate-limit global ativa — aguardando antes da tentativa"
            );
            tokio::time::sleep(Duration::from_millis(extra_ms)).await;
        }

        tracing::debug!(tentativa = tentativa + 1, total = total_tentativas, url = %url, "executando GET");

        let envio = tokio::select! {
            biased;
            _ = cancelamento.cancelled() => {
                return Err(MotivoFalhaRetry::Rede("cancelado durante request".to_string()));
            }
            res = cliente.get(url).send() => res,
        };

        match envio {
            Ok(resposta) => {
                let status = resposta.status();
                // HTTP 202 = anomalia DDG (bloqueio suave anti-bot).
                // Browsers reais NUNCA recebem 202 do DuckDuckGo.
                if status == StatusCode::ACCEPTED {
                    flag_rate_limit.store(true, Ordering::Relaxed);
                    ultimo_motivo = MotivoFalhaRetry::Blocked;
                    if tentativa + 1 < total_tentativas {
                        let total = calcular_backoff_ms(tentativa);
                        tracing::warn!(
                            tentativa = tentativa + 1,
                            backoff_ms = total,
                            "HTTP 202 anomaly — bloqueio suave DDG, aplicando backoff"
                        );
                        tokio::time::sleep(Duration::from_millis(total)).await;
                        continue;
                    }
                    return Err(MotivoFalhaRetry::Blocked);
                }
                if status.is_success() {
                    return Ok(ResultadoRetry {
                        resposta,
                        tentativas: tentativa + 1,
                    });
                }
                if status == StatusCode::TOO_MANY_REQUESTS {
                    flag_rate_limit.store(true, Ordering::Relaxed);
                    ultimo_motivo = MotivoFalhaRetry::RateLimited;
                    if tentativa + 1 < total_tentativas {
                        let total = calcular_backoff_ms(tentativa);
                        tracing::warn!(
                            tentativa = tentativa + 1,
                            backoff_ms = total,
                            "HTTP 429 — aplicando backoff exponencial"
                        );
                        tokio::time::sleep(Duration::from_millis(total)).await;
                        continue;
                    }
                    return Err(MotivoFalhaRetry::RateLimited);
                }
                if status == StatusCode::FORBIDDEN {
                    ultimo_motivo = MotivoFalhaRetry::Blocked;
                    if tentativa + 1 < total_tentativas {
                        tracing::warn!(
                            tentativa = tentativa + 1,
                            "HTTP 403 — retry imediato (rotação de UA aplicada no próximo cliente)"
                        );
                        // A rotação de UA é responsabilidade do chamador; aqui apenas sinalizamos.
                        continue;
                    }
                    return Err(MotivoFalhaRetry::Blocked);
                }
                // Outros 4xx/5xx — não retentamos.
                return Err(MotivoFalhaRetry::HttpErro(status.as_u16()));
            }
            Err(erro) => {
                if erro.is_timeout() {
                    ultimo_motivo = MotivoFalhaRetry::Timeout;
                    if !timeout_ja_retentado && tentativa + 1 < total_tentativas {
                        timeout_ja_retentado = true;
                        tracing::warn!("timeout — 1 retry permitido");
                        continue;
                    }
                    return Err(MotivoFalhaRetry::Timeout);
                }
                ultimo_motivo = MotivoFalhaRetry::Rede(erro.to_string());
                // Erros de rede genéricos: 1 retry opcional se ainda houver tentativas.
                if tentativa + 1 < total_tentativas {
                    let backoff = Duration::from_millis(400);
                    tokio::time::sleep(backoff).await;
                    continue;
                }
                return Err(ultimo_motivo);
            }
        }
    }

    Err(ultimo_motivo)
}

/// Executes the initial search on the configured endpoint and returns the raw HTML.
/// Compatibility version (iteration 1) — used by the simple single-query flow.
pub async fn executar_busca(
    cliente: &Client,
    query: &str,
    idioma: &str,
    pais: &str,
) -> Result<String> {
    let url = construir_url(query, idioma, pais);
    tracing::debug!(url = %url, "Enviando GET para o endpoint HTML do DuckDuckGo");

    let resposta = cliente
        .get(&url)
        .send()
        .await
        .with_context(|| format!("falha ao enviar GET para {url}"))?;

    let status = resposta.status();
    tracing::debug!(status = %status, "Resposta HTTP recebida");

    if !status.is_success() {
        bail!(
            "DuckDuckGo retornou status HTTP {} ao buscar {:?}",
            status.as_u16(),
            query
        );
    }

    let html = resposta
        .text()
        .await
        .context("falha ao ler corpo UTF-8 da resposta")?;

    if html.len() < LIMIAR_BLOQUEIO_SILENCIOSO {
        tracing::warn!(
            bytes = html.len(),
            limiar = LIMIAR_BLOQUEIO_SILENCIOSO,
            "resposta suspeitamente pequena — possível bloqueio silencioso"
        );
        bail!(
            "resposta suspeitamente pequena ({} bytes < {} limiar) — possível bloqueio silencioso",
            html.len(),
            LIMIAR_BLOQUEIO_SILENCIOSO
        );
    }

    tracing::debug!(bytes = html.len(), "HTML recebido com sucesso");
    Ok(html)
}

/// Aggregated result of a search with pagination and potential endpoint fallback.
pub struct ResultadoBuscaAgregado {
    pub resultados: Vec<ResultadoBusca>,
    pub paginas_buscadas: u32,
    pub usou_fallback_lite: bool,
    pub tentativas: u32,
    pub endpoint_efetivo: Endpoint,
}

/// Extracts `vqd`, `s` and `dc` from the first page HTML (for pagination).
/// Returns `None` if any of the three fields is missing.
pub fn extrair_tokens_paginacao(html: &str) -> Option<(String, String, String)> {
    use scraper::{Html, Selector};
    let doc = Html::parse_document(html);

    // Os seletores são constantes bem-formadas; use ok() para robustez.
    let sel_vqd = Selector::parse("input[name='vqd']").ok()?;
    let sel_s = Selector::parse("input[name='s']").ok()?;
    let sel_dc = Selector::parse("input[name='dc']").ok()?;

    let vqd = doc
        .select(&sel_vqd)
        .next()
        .and_then(|el| el.value().attr("value"))
        .map(|v| v.to_string())?;
    let s = doc
        .select(&sel_s)
        .next()
        .and_then(|el| el.value().attr("value"))
        .map(|v| v.to_string())?;
    let dc = doc
        .select(&sel_dc)
        .next()
        .and_then(|el| el.value().attr("value"))
        .map(|v| v.to_string())?;

    Some((vqd, s, dc))
}

/// Runs a complete search with vqd pagination and optional fallback to Lite.
///
/// If the HTML endpoint returns zero results on the first page (via Strategies 1 and 2),
/// automatically falls back to the Lite endpoint (Strategy 3).
///
/// Returns an aggregated structure with results, related searches, pages actually
/// fetched, fallback indicator, and total attempt count.
pub async fn buscar_com_paginacao(
    cliente: &Client,
    cfg: &Configuracoes,
    query: &str,
    flag_rate_limit: &Arc<AtomicBool>,
    cancelamento: &CancellationToken,
) -> std::result::Result<ResultadoBuscaAgregado, MotivoFalhaRetry> {
    let endpoint_inicial = cfg.endpoint;
    let url_inicial = construir_url_busca(
        query,
        &cfg.idioma,
        &cfg.pais,
        endpoint_inicial,
        cfg.filtro_temporal,
        cfg.safe_search,
    );

    let resultado_primeiro = executar_com_retry(
        cliente,
        &url_inicial,
        cfg.retries,
        flag_rate_limit,
        cancelamento,
    )
    .await?;
    let mut tentativas_acumuladas = resultado_primeiro.tentativas;

    let html_primeira = resultado_primeiro
        .resposta
        .text()
        .await
        .map_err(|e| MotivoFalhaRetry::Rede(e.to_string()))?;

    if html_primeira.len() < LIMIAR_BLOQUEIO_SILENCIOSO {
        tracing::warn!(
            bytes = html_primeira.len(),
            limiar = LIMIAR_BLOQUEIO_SILENCIOSO,
            "resposta da primeira página suspeitamente pequena — possível bloqueio silencioso"
        );
        return Err(MotivoFalhaRetry::Blocked);
    }

    // Extrai resultados da primeira página conforme endpoint.
    let mut resultados_acumulados = match endpoint_inicial {
        Endpoint::Html => {
            extraction::extrair_resultados_com_estrategias_cfg(&html_primeira, &cfg.seletores)
        }
        Endpoint::Lite => {
            extraction::extrair_resultados_lite_com_cfg(&html_primeira, &cfg.seletores)
        }
    };
    let mut usou_fallback_lite = false;
    let mut endpoint_efetivo = endpoint_inicial;
    let mut paginas_buscadas: u32 = 1;

    // Se HTML retornou zero E estamos no endpoint HTML → tentar Lite como fallback.
    if resultados_acumulados.is_empty() && endpoint_inicial == Endpoint::Html {
        tracing::warn!("HTML retornou zero resultados — tentando fallback Lite");
        let url_lite = construir_url_busca(
            query,
            &cfg.idioma,
            &cfg.pais,
            Endpoint::Lite,
            cfg.filtro_temporal,
            cfg.safe_search,
        );
        match executar_com_retry(
            cliente,
            &url_lite,
            cfg.retries,
            flag_rate_limit,
            cancelamento,
        )
        .await
        {
            Ok(r_lite) => {
                tentativas_acumuladas = tentativas_acumuladas.saturating_add(r_lite.tentativas);
                let html_lite = r_lite
                    .resposta
                    .text()
                    .await
                    .map_err(|e| MotivoFalhaRetry::Rede(e.to_string()))?;
                let resultados_lite =
                    extraction::extrair_resultados_lite_com_cfg(&html_lite, &cfg.seletores);
                if !resultados_lite.is_empty() {
                    resultados_acumulados = resultados_lite;
                    usou_fallback_lite = true;
                    endpoint_efetivo = Endpoint::Lite;
                }
            }
            Err(erro) => {
                tracing::warn!(?erro, "fallback Lite também falhou — mantendo vazio");
            }
        }
    }

    // Paginação vqd APENAS para endpoint HTML (o Lite não tem esse mecanismo).
    // E APENAS se configurado para múltiplas páginas.
    if endpoint_efetivo == Endpoint::Html && cfg.paginas > 1 && !resultados_acumulados.is_empty() {
        if let Some((mut vqd, mut s, mut dc)) = extrair_tokens_paginacao(&html_primeira) {
            for pagina_idx in 2..=cfg.paginas {
                if cancelamento.is_cancelled() {
                    tracing::debug!("cancelamento detectado durante paginação");
                    break;
                }

                // Delay entre páginas.
                let delay_ms =
                    rand::thread_rng().gen_range(DELAY_PAGINACAO_MIN_MS..=DELAY_PAGINACAO_MAX_MS);
                tokio::select! {
                    biased;
                    _ = cancelamento.cancelled() => { break; }
                    _ = tokio::time::sleep(Duration::from_millis(delay_ms)) => {}
                }

                let kl = formatar_kl(&cfg.idioma, &cfg.pais);
                // Forma idêntica ao formulário hidden retornado pelo DOM (descoberto
                // empiricamente em 2026-04-14 / iteração 4): além de `q`/`s`/`dc`/`vqd`/`kl`,
                // o DDG espera `nextParams` (vazio), `v="l"`, `o="json"`, `api="d.js"`.
                let formulario: Vec<(&str, String)> = vec![
                    ("q", query.to_string()),
                    ("s", s.clone()),
                    ("nextParams", String::new()),
                    ("v", "l".to_string()),
                    ("o", "json".to_string()),
                    ("dc", dc.clone()),
                    ("api", "d.js".to_string()),
                    ("vqd", vqd.clone()),
                    ("kl", kl),
                ];

                let base = url_base_html();
                let resposta = match tokio::select! {
                    biased;
                    _ = cancelamento.cancelled() => {
                        break;
                    }
                    r = cliente
                        .post(&base)
                        .header(reqwest::header::REFERER, "https://html.duckduckgo.com/")
                        .headers(cfg.perfil_browser.headers_paginacao())
                        .form(&formulario)
                        .send() => r,
                } {
                    Ok(r) => r,
                    Err(erro) => {
                        tracing::warn!(
                            ?erro,
                            pagina = pagina_idx,
                            "erro de rede na paginação — parando"
                        );
                        break;
                    }
                };

                if !resposta.status().is_success() {
                    tracing::warn!(
                        status = resposta.status().as_u16(),
                        pagina = pagina_idx,
                        "paginação retornou status não-sucesso — parando"
                    );
                    break;
                }

                let html_pagina = match resposta.text().await {
                    Ok(t) => t,
                    Err(e) => {
                        tracing::warn!(?e, "erro ao ler corpo da página — parando");
                        break;
                    }
                };

                // Verifica bloqueio silencioso na página de paginação.
                if html_pagina.len() < LIMIAR_BLOQUEIO_SILENCIOSO {
                    tracing::warn!(
                        bytes = html_pagina.len(),
                        limiar = LIMIAR_BLOQUEIO_SILENCIOSO,
                        pagina = pagina_idx,
                        "página de paginação suspeitamente pequena — possível bloqueio silencioso"
                    );
                    break;
                }

                let novos = extraction::extrair_resultados_com_estrategias_cfg(
                    &html_pagina,
                    &cfg.seletores,
                );
                if novos.is_empty() {
                    tracing::debug!(
                        pagina = pagina_idx,
                        "página retornou zero resultados — parando"
                    );
                    break;
                }

                // Renumera posições seguindo o Vec acumulado.
                let offset = u32::try_from(resultados_acumulados.len()).unwrap_or(u32::MAX);
                for mut r in novos {
                    r.posicao = offset.saturating_add(r.posicao);
                    resultados_acumulados.push(r);
                }

                paginas_buscadas = pagina_idx;

                // Atualiza tokens para a próxima página; se ausentes, interrompe.
                match extrair_tokens_paginacao(&html_pagina) {
                    Some((novo_vqd, novo_s, novo_dc)) => {
                        vqd = novo_vqd;
                        s = novo_s;
                        dc = novo_dc;
                    }
                    None => {
                        tracing::warn!(
                            pagina = pagina_idx,
                            "tokens de paginação ausentes — parando"
                        );
                        break;
                    }
                }
            }
        } else {
            tracing::warn!("tokens vqd/s/dc ausentes na primeira página — sem paginação possível");
        }
    }

    // Trunca ao --num se especificado.
    if let Some(n) = cfg.num_resultados {
        let n_usize = n as usize;
        if resultados_acumulados.len() > n_usize {
            resultados_acumulados.truncate(n_usize);
        }
    }

    Ok(ResultadoBuscaAgregado {
        resultados: resultados_acumulados,
        paginas_buscadas,
        usou_fallback_lite,
        tentativas: tentativas_acumuladas,
        endpoint_efetivo,
    })
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn formatar_kl_concatena_corretamente() {
        assert_eq!(formatar_kl("pt", "br"), "br-pt");
        assert_eq!(formatar_kl("PT", "BR"), "br-pt");
        assert_eq!(formatar_kl("en", "us"), "us-en");
    }

    #[test]
    fn construir_url_escapa_espacos_e_acentos() {
        let url = construir_url("endividamento brasileiro", "pt", "br");
        assert!(url.starts_with("https://html.duckduckgo.com/html/?q="));
        assert!(url.contains("endividamento%20brasileiro"));
        assert!(url.contains("&kl=br-pt"));
    }

    #[test]
    fn construir_url_escapa_caracteres_especiais() {
        let url = construir_url("C++ tutorial", "en", "us");
        assert!(url.contains("C%2B%2B"));
        assert!(url.contains("&kl=us-en"));
    }

    #[test]
    fn construir_url_com_acentos_portugueses() {
        let url = construir_url("música eletrônica", "pt", "br");
        assert!(url.contains("m%C3%BAsica"));
        assert!(url.contains("eletr%C3%B4nica"));
    }

    #[test]
    fn construir_url_busca_adiciona_parametros_opcionais() {
        let url = construir_url_busca(
            "rust",
            "en",
            "us",
            Endpoint::Html,
            Some(FiltroTemporal::Semana),
            SafeSearch::Strict,
        );
        assert!(url.contains("&kp=1"));
        assert!(url.contains("&df=w"));
    }

    #[test]
    fn construir_url_busca_omite_kp_quando_moderate() {
        let url = construir_url_busca(
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
    fn construir_url_busca_endpoint_lite_usa_url_correta() {
        let url = construir_url_busca(
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
    fn extrair_tokens_paginacao_extrai_quando_presentes() {
        let html = r#"
            <form>
              <input name="q" value="rust">
              <input name="vqd" value="4-12345678-abc">
              <input name="s" value="50">
              <input name="dc" value="51">
            </form>
        "#;
        let (vqd, s, dc) = extrair_tokens_paginacao(html).expect("todos presentes");
        assert_eq!(vqd, "4-12345678-abc");
        assert_eq!(s, "50");
        assert_eq!(dc, "51");
    }

    #[test]
    fn extrair_tokens_paginacao_retorna_none_quando_ausentes() {
        let html = r#"<html><body>Sem inputs</body></html>"#;
        assert!(extrair_tokens_paginacao(html).is_none());
    }

    #[test]
    fn motivo_falha_retry_codigo_erro_correto() {
        assert_eq!(
            MotivoFalhaRetry::RateLimited.como_codigo_erro(),
            crate::error::codigos::RATE_LIMITED
        );
        assert_eq!(
            MotivoFalhaRetry::Blocked.como_codigo_erro(),
            crate::error::codigos::BLOCKED
        );
        assert_eq!(
            MotivoFalhaRetry::Timeout.como_codigo_erro(),
            crate::error::codigos::TIMEOUT
        );
    }
}
