// SPDX-License-Identifier: MIT OR Apache-2.0
//! Testes de integração com `wiremock` — ZERO chamadas HTTP reais.
//!
//! Cada teste sobe um `MockServer` em porta aleatória e define URL base via
//! variável de ambiente lida por `search::html_base_url`/`lite_base_url`. A variável
//! é definida e limpa dentro do próprio teste. Todos os testes que manipulam env
//! vars executam serializados (por constraint implícito do `std::env::set_var`),
//! e cada teste usa paths distintos (ou o mesmo path `/`) para evitar interferência.

use duckduckgo_search_cli::search::{
    execute_with_retry, extract_pagination_tokens, search_with_pagination, RetryFailReason,
};
use duckduckgo_search_cli::types::{Config, Endpoint, OutputFormat, SafeSearch};
use reqwest::Client;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, OnceLock};
use std::time::Duration;
use tokio::sync::Mutex as TokioMutex;
use tokio_util::sync::CancellationToken;
use wiremock::matchers::{body_string_contains, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// Mutex async global para serializar testes que manipulam env vars.
/// `std::env::set_var` is not thread-safe; each test acquires the lock async-friendly.
fn env_lock() -> &'static TokioMutex<()> {
    static LOCK: OnceLock<TokioMutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| TokioMutex::new(()))
}

fn test_client() -> Client {
    Client::builder()
        .timeout(Duration::from_secs(10))
        .user_agent("Mozilla/5.0 (teste)")
        .build()
        .expect("cliente de teste")
}

fn base_config(endpoint: Endpoint, pages: u32, retries: u32) -> Config {
    Config {
        query: "rust".to_string(),
        queries: vec!["rust".to_string()],
        num_results: None,
        format: OutputFormat::Json,
        timeout_seconds: 5,
        language: "pt".to_string(),
        country: "br".to_string(),
        verbose: false,
        quiet: true,
        user_agent: "Mozilla/5.0 (teste)".to_string(),
        browser_profile: duckduckgo_search_cli::http::create_browser_profile("Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/146.0.0.0 Safari/537.36"),
        parallelism: 1,
        pages,
        retries,
        endpoint,
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
        selectors: std::sync::Arc::new(
            duckduckgo_search_cli::types::SelectorConfig::default(),
        ),
    }
}

fn html_with_3_results_class() -> String {
    // Padding garante que o corpo fique acima de LIMIAR_BLOQUEIO_SILENCIOSO (5 000 bytes).
    let padding =
        "<!-- padding para superar o limiar de detecção de bloqueio silencioso do DuckDuckGo. -->"
            .repeat(60);
    format!(
        r#"<html><body>
    {padding}
    <div id="links">
      <div class="result">
        <a class="result__a" href="//exemplo.com/um">Resultado Um</a>
        <a class="result__snippet">Descrição do primeiro resultado.</a>
        <span class="result__url">exemplo.com/um</span>
      </div>
      <div class="result">
        <a class="result__a" href="//exemplo.com/dois">Resultado Dois</a>
        <a class="result__snippet">Descrição do segundo resultado.</a>
      </div>
      <div class="result">
        <a class="result__a" href="//exemplo.com/tres">Resultado Três</a>
        <a class="result__snippet">Descrição do terceiro resultado.</a>
      </div>
      <div class="result result--ad">
        <a class="result__a" href="//anuncio.com">Anúncio Patrocinado</a>
      </div>
    </div>
    </body></html>"#
    )
}

fn html_with_vqd_tokens_and_results(vqd: &str, s: &str, dc: &str, titles: &[&str]) -> String {
    // Padding garante que o corpo fique acima de LIMIAR_BLOQUEIO_SILENCIOSO (5 000 bytes).
    let padding =
        "<!-- padding para superar o limiar de detecção de bloqueio silencioso do DuckDuckGo. -->"
            .repeat(60);
    let mut html = format!("<html><body>{padding}");
    html.push_str(&format!(
        r#"<form><input name="vqd" value="{vqd}"><input name="s" value="{s}"><input name="dc" value="{dc}"></form>"#
    ));
    html.push_str(r#"<div id="links">"#);
    for t in titles {
        html.push_str(&format!(
            r#"<div class="result"><a class="result__a" href="//exemplo.com/{}">{}</a><a class="result__snippet">snippet de {}</a></div>"#,
            t.replace(' ', "-"),
            t,
            t
        ));
    }
    html.push_str("</div></body></html>");
    html
}

fn empty_html_without_result() -> String {
    // Padding garante que o corpo fique acima de LIMIAR_BLOQUEIO_SILENCIOSO (5 000 bytes).
    // HTML sem `.result` para testar o caminho de zero resultados.
    let padding =
        "<!-- padding para superar o limiar de detecção de bloqueio silencioso do DuckDuckGo. -->"
            .repeat(60);
    format!(
        r#"<html><head><title>DuckDuckGo sem resultados</title></head><body>{padding}<div id="links"><p>Nenhum resultado para esta busca específica foi encontrado aqui.</p></div></body></html>"#
    )
}

fn html_lite_tabela() -> String {
    // Padding garante que o corpo fique acima de LIMIAR_BLOQUEIO_SILENCIOSO (5 000 bytes).
    let padding =
        "<!-- padding para superar o limiar de detecção de bloqueio silencioso do DuckDuckGo. -->"
            .repeat(60);
    format!(
        r#"<html><body>
    {padding}
    <table>
      <tr><td valign="top">1.&nbsp;</td><td><a class="result-link" href="//exemplo.com/lite1">Lite Um</a></td></tr>
      <tr><td>&nbsp;</td><td class="result-snippet">Descrição detalhada do primeiro resultado lite.</td></tr>
      <tr><td valign="top">2.&nbsp;</td><td><a class="result-link" href="//exemplo.com/lite2">Lite Dois</a></td></tr>
      <tr><td>&nbsp;</td><td class="result-snippet">Descrição detalhada do segundo resultado lite.</td></tr>
    </table>
    </body></html>"#
    )
}

/// Guard to configure env vars during a test and clean up on exit.
struct EnvGuard {
    keys: Vec<&'static str>,
}
impl EnvGuard {
    fn set(keys: &[(&'static str, String)]) -> Self {
        let mut ks = Vec::new();
        for (k, v) in keys {
            std::env::set_var(k, v);
            ks.push(*k);
        }
        EnvGuard { keys: ks }
    }
}
impl Drop for EnvGuard {
    fn drop(&mut self) {
        for k in &self.keys {
            std::env::remove_var(k);
        }
    }
}

// ---------------------------------------------------------------------------
// Test 1: Strategy 1 with full HTML → 3 extracted results, 0 ads.
// ---------------------------------------------------------------------------
#[tokio::test]
async fn test_strategy_1_success() {
    let _g = env_lock().lock().await;
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(html_with_3_results_class())
                .insert_header("content-type", "text/html; charset=utf-8"),
        )
        .mount(&mock_server)
        .await;

    let base = format!("{}/", mock_server.uri());
    let _env = EnvGuard::set(&[
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_HTML", base.clone()),
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_LITE", base),
    ]);

    let cliente = test_client();
    let cfg = base_config(Endpoint::Html, 1, 0);
    let flag = Arc::new(AtomicBool::new(false));
    let token = CancellationToken::new();

    let agregado = search_with_pagination(&cliente, &cfg, "rust", &flag, &token)
        .await
        .expect("busca deve ter sucesso");

    assert_eq!(agregado.results.len(), 3, "3 resultados orgânicos");
    assert_eq!(agregado.results[0].title, "Resultado Um");
    assert_eq!(agregado.results[0].url, "https://exemplo.com/um");
    assert_eq!(agregado.pages_fetched, 1);
    assert!(!agregado.used_fallback_lite);
    assert_eq!(agregado.effective_endpoint, Endpoint::Html);
}

// ---------------------------------------------------------------------------
// Test 2: Empty HTML → fallback to Lite → results extracted via Strategy 3.
// ---------------------------------------------------------------------------
#[tokio::test]
async fn test_fallback_lite_when_html_empty() {
    let _g = env_lock().lock().await;
    let mock_server_html = MockServer::start().await;
    let mock_server_lite = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(empty_html_without_result())
                .insert_header("content-type", "text/html; charset=utf-8"),
        )
        .mount(&mock_server_html)
        .await;

    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(html_lite_tabela())
                .insert_header("content-type", "text/html; charset=utf-8"),
        )
        .mount(&mock_server_lite)
        .await;

    let base_html = format!("{}/", mock_server_html.uri());
    let base_lite = format!("{}/", mock_server_lite.uri());
    let _env = EnvGuard::set(&[
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_HTML", base_html),
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_LITE", base_lite),
    ]);

    let cliente = test_client();
    let cfg = base_config(Endpoint::Html, 1, 0);
    let flag = Arc::new(AtomicBool::new(false));
    let token = CancellationToken::new();

    let agregado = search_with_pagination(&cliente, &cfg, "rust", &flag, &token)
        .await
        .expect("fallback Lite deve ter sucesso");

    assert_eq!(agregado.results.len(), 2, "2 resultados do Lite");
    assert_eq!(agregado.results[0].title, "Lite Um");
    assert!(agregado.used_fallback_lite, "flag fallback deve estar true");
    assert_eq!(agregado.effective_endpoint, Endpoint::Lite);
}

// ---------------------------------------------------------------------------
// Teste 3: Retry em 429 — 2 primeiras respostas 429, 3a 200.
// ---------------------------------------------------------------------------
#[tokio::test]
async fn test_retry_on_429() {
    let _g = env_lock().lock().await;
    let mock_server = MockServer::start().await;

    // Limitation: each Mock responds with the same template. To sequence responses,
    // use `up_to_n_times` on prior mocks and leave the final mock as catch-all.
    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(ResponseTemplate::new(429))
        .up_to_n_times(2)
        .with_priority(1)
        .mount(&mock_server)
        .await;

    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(html_with_3_results_class())
                .insert_header("content-type", "text/html; charset=utf-8"),
        )
        .with_priority(2)
        .mount(&mock_server)
        .await;

    let base = format!("{}/", mock_server.uri());
    let _env = EnvGuard::set(&[
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_HTML", base.clone()),
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_LITE", base),
    ]);

    let cliente = test_client();
    // 2 retries → up to 3 attempts total.
    let cfg = base_config(Endpoint::Html, 1, 2);
    let flag = Arc::new(AtomicBool::new(false));
    let token = CancellationToken::new();

    let agregado = search_with_pagination(&cliente, &cfg, "rust", &flag, &token)
        .await
        .expect("retry deve eventualmente ter sucesso");

    assert_eq!(agregado.results.len(), 3);
    assert_eq!(
        agregado.attempts, 3,
        "DEVE ter executado exatamente 3 tentativas (2 falhas 429 + 1 sucesso)"
    );
    assert!(
        flag.load(std::sync::atomic::Ordering::Relaxed),
        "flag_rate_limit global deve ter sido ativada"
    );
}

// ---------------------------------------------------------------------------
// Test 4: Persistent 403 → `blocked` error after exhausting retries.
// ---------------------------------------------------------------------------
#[tokio::test]
async fn testa_blocked_apos_retries_esgotados() {
    let _g = env_lock().lock().await;
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(ResponseTemplate::new(403))
        .mount(&mock_server)
        .await;

    let base = format!("{}/", mock_server.uri());
    let _env = EnvGuard::set(&[
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_HTML", base.clone()),
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_LITE", base),
    ]);

    let cliente = test_client();
    // 1 retry → up to 2 attempts.
    let cfg = base_config(Endpoint::Html, 1, 1);
    let flag = Arc::new(AtomicBool::new(false));
    let token = CancellationToken::new();

    let result = execute_with_retry(
        &cliente,
        &format!("{}/", mock_server.uri()),
        cfg.retries,
        &flag,
        &token,
    )
    .await;

    match result {
        Err(RetryFailReason::Blocked) => {}
        other => panic!("esperava RetryFailReason::Blocked, recebi {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// Test 5: vqd pagination — 3 pages, combining results.
// ---------------------------------------------------------------------------
#[tokio::test]
async fn testa_paginacao_vqd() {
    let _g = env_lock().lock().await;
    let mock_server = MockServer::start().await;

    // Page 1 (GET) — HTML with vqd/s/dc tokens.
    let html_pg1 =
        html_with_vqd_tokens_and_results("vqd-pg1", "0", "30", &["Res Um", "Res Dois", "Res Três"]);
    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(html_pg1)
                .insert_header("content-type", "text/html; charset=utf-8"),
        )
        .mount(&mock_server)
        .await;

    // Page 2 (POST with vqd-pg1) — returns HTML with vqd-pg2.
    let html_pg2 =
        html_with_vqd_tokens_and_results("vqd-pg2", "30", "60", &["Res Quatro", "Res Cinco"]);
    Mock::given(method("POST"))
        .and(path("/"))
        .and(body_string_contains("vqd=vqd-pg1"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(html_pg2)
                .insert_header("content-type", "text/html; charset=utf-8"),
        )
        .with_priority(1)
        .mount(&mock_server)
        .await;

    // Page 3 (POST with vqd-pg2).
    let html_pg3 = html_with_vqd_tokens_and_results("vqd-pg3", "60", "90", &["Res Seis"]);
    Mock::given(method("POST"))
        .and(path("/"))
        .and(body_string_contains("vqd=vqd-pg2"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(html_pg3)
                .insert_header("content-type", "text/html; charset=utf-8"),
        )
        .with_priority(2)
        .mount(&mock_server)
        .await;

    let base = format!("{}/", mock_server.uri());
    let _env = EnvGuard::set(&[
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_HTML", base.clone()),
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_LITE", base),
    ]);

    let cliente = test_client();
    let cfg = base_config(Endpoint::Html, 3, 0);
    let flag = Arc::new(AtomicBool::new(false));
    let token = CancellationToken::new();

    let agregado = search_with_pagination(&cliente, &cfg, "rust", &flag, &token)
        .await
        .expect("paginação deve funcionar");

    assert_eq!(
        agregado.results.len(),
        6,
        "deve combinar resultados das 3 páginas"
    );
    assert_eq!(agregado.pages_fetched, 3);
    // Positions must be 1..=6, preserving order per page.
    for (i, r) in agregado.results.iter().enumerate() {
        assert_eq!(r.position, (i + 1) as u32);
    }
    assert_eq!(agregado.results[0].title, "Res Um");
    assert_eq!(agregado.results[3].title, "Res Quatro");
    assert_eq!(agregado.results[5].title, "Res Seis");
}

// ---------------------------------------------------------------------------
// Test 6: Ad filtering — HTML with mixed ads, only organic results returned.
// ---------------------------------------------------------------------------
#[tokio::test]
async fn testa_filtro_anuncios() {
    let _g = env_lock().lock().await;
    let mock_server = MockServer::start().await;

    // Mixed HTML: 2 organic + 2 ads (one by class, another by data-nrn).
    // Padding garante que o corpo fique acima de LIMIAR_BLOQUEIO_SILENCIOSO (5 000 bytes).
    let padding =
        "<!-- padding para superar o limiar de detecção de bloqueio silencioso do DuckDuckGo. -->"
            .repeat(60);
    let html = format!(
        r#"<html><body>
    {padding}
    <div id="links">
      <div class="result result--ad">
        <a class="result__a" href="//anuncio1.com">Anúncio Um</a>
      </div>
      <div class="result">
        <a class="result__a" href="//organico.com/a">Orgânico A</a>
        <a class="result__snippet">Snippet A</a>
      </div>
      <div class="result" data-nrn="ad">
        <a class="result__a" href="//anuncio2.com">Anúncio Dois</a>
      </div>
      <div class="result">
        <a class="result__a" href="//organico.com/b">Orgânico B</a>
        <a class="result__snippet">Snippet B</a>
      </div>
      <div class="result">
        <a class="result__a" href="//duckduckgo.com/y.js?ad=x">Tracker</a>
      </div>
    </div>
    </body></html>"#
    );

    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(html)
                .insert_header("content-type", "text/html; charset=utf-8"),
        )
        .mount(&mock_server)
        .await;

    let base = format!("{}/", mock_server.uri());
    let _env = EnvGuard::set(&[
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_HTML", base.clone()),
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_LITE", base),
    ]);

    let cliente = test_client();
    let cfg = base_config(Endpoint::Html, 1, 0);
    let flag = Arc::new(AtomicBool::new(false));
    let token = CancellationToken::new();

    let agregado = search_with_pagination(&cliente, &cfg, "rust", &flag, &token)
        .await
        .expect("sucesso");

    assert_eq!(
        agregado.results.len(),
        2,
        "apenas orgânicos devem sobreviver ao filtro"
    );
    assert_eq!(agregado.results[0].title, "Orgânico A");
    assert_eq!(agregado.results[1].title, "Orgânico B");
    for r in &agregado.results {
        assert!(!r.url.contains("anuncio"));
        assert!(!r.url.contains("y.js"));
    }
}

// ---------------------------------------------------------------------------
// Test 7 (v0.3.0): "Official site" heuristic — DDG renders the literal text
// "Official site" as the title for verified domains. The scraper replaces it
// with `url_exibicao` and preserves the literal in `titulo_original`.
// ---------------------------------------------------------------------------
#[tokio::test]
async fn testa_heuristica_official_site() {
    let _g = env_lock().lock().await;
    let mock_server = MockServer::start().await;

    // HTML with a result that has the literal title "Official site" + .result__url.
    // Padding garante que o corpo fique acima de LIMIAR_BLOQUEIO_SILENCIOSO (5 000 bytes).
    let padding =
        "<!-- padding para superar o limiar de detecção de bloqueio silencioso do DuckDuckGo. -->"
            .repeat(60);
    let html = format!(
        r#"<html><body>
    {padding}
    <div id="links">
      <div class="result">
        <a class="result__a" href="//saofidelis.rj.gov.br/">Official site</a>
        <span class="result__url">saofidelis.rj.gov.br</span>
        <a class="result__snippet">Prefeitura Municipal de São Fidélis.</a>
      </div>
      <div class="result">
        <a class="result__a" href="//exemplo.com/outro">Título Normal</a>
        <span class="result__url">exemplo.com</span>
        <a class="result__snippet">Snippet qualquer.</a>
      </div>
    </div>
    </body></html>"#
    );

    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(html)
                .insert_header("content-type", "text/html; charset=utf-8"),
        )
        .mount(&mock_server)
        .await;

    let base = format!("{}/", mock_server.uri());
    let _env = EnvGuard::set(&[
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_HTML", base.clone()),
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_LITE", base),
    ]);

    let cliente = test_client();
    let cfg = base_config(Endpoint::Html, 1, 0);
    let flag = Arc::new(AtomicBool::new(false));
    let token = CancellationToken::new();

    let agregado = search_with_pagination(&cliente, &cfg, "saofidelis", &flag, &token)
        .await
        .expect("sucesso");

    assert_eq!(agregado.results.len(), 2, "2 orgânicos esperados");

    // Result 1: title replaced by url_exibicao, original preserved.
    let r1 = &agregado.results[0];
    assert_eq!(
        r1.title, "saofidelis.rj.gov.br",
        "titulo deve ser o url_exibicao"
    );
    assert_eq!(
        r1.original_title.as_deref(),
        Some("Official site"),
        "titulo_original deve preservar o literal"
    );

    // Result 2: normal title → no substitution, original_title = None.
    let r2 = &agregado.results[1];
    assert_eq!(r2.title, "Título Normal");
    assert!(
        r2.original_title.is_none(),
        "original_title deve ser None quando não há substituição"
    );
}

// ---------------------------------------------------------------------------
// Test 8 (v0.3.0): JSON schema NO LONGER contains `buscas_relacionadas`.
// ---------------------------------------------------------------------------
#[tokio::test]
async fn test_schema_v03_without_related_searches() {
    let _g = env_lock().lock().await;
    let mock_server = MockServer::start().await;

    let html = html_with_vqd_tokens_and_results("v", "0", "0", &["T1", "T2"]);
    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(html)
                .insert_header("content-type", "text/html; charset=utf-8"),
        )
        .mount(&mock_server)
        .await;

    let base = format!("{}/", mock_server.uri());
    let _env = EnvGuard::set(&[
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_HTML", base.clone()),
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_LITE", base),
    ]);

    let cliente = test_client();
    let cfg = base_config(Endpoint::Html, 1, 0);
    let flag = Arc::new(AtomicBool::new(false));
    let token = CancellationToken::new();

    let agregado = search_with_pagination(&cliente, &cfg, "teste", &flag, &token)
        .await
        .expect("sucesso");

    // Serialize as JSON and confirm the field does NOT appear.
    use duckduckgo_search_cli::types::{SearchMetadata, SearchOutput};
    let output = SearchOutput {
        query: "teste".into(),
        engine: "duckduckgo".into(),
        endpoint: "html".into(),
        timestamp: "2026-04-14T00:00:00Z".into(),
        region: "br-pt".into(),
        result_count: agregado.results.len() as u32,
        results: agregado.results,
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

    let json = serde_json::to_string_pretty(&output).expect("serializa");
    assert!(
        !json.contains("buscas_relacionadas"),
        "v0.3.0: schema JSON NÃO deve expor buscas_relacionadas"
    );
    assert!(
        !json.contains("related_searches"),
        "v0.3.0: schema JSON NÃO deve expor related_searches"
    );
}

// ===================================================================
// Tests for --fetch-content pure HTTP (iteration 5).
// ===================================================================

/// Real HTML page with enough content to pass the 200-char threshold.
fn html_artigo_real() -> String {
    r#"<!DOCTYPE html><html><head><title>Artigo de Teste</title></head>
    <body>
      <nav><a href="/">Home</a> <a href="/about">About</a></nav>
      <article>
        <h1>Título Principal do Artigo</h1>
        <p>Este é o primeiro parágrafo do artigo com conteúdo substantivo suficiente para passar o limiar de linha mínima. Contém várias frases para simular um texto real de notícia ou documentação técnica.</p>
        <p>Segundo parágrafo relevante com mais informações sobre o tema tratado no artigo. Incluímos conteúdo em português brasileiro com acentuação correta para validar a decodificação UTF-8 adequadamente.</p>
        <p>Terceiro parágrafo conclui o artigo com uma síntese dos pontos principais abordados ao longo do texto. Este conteúdo deve ser preservado integralmente pela extração readability.</p>
      </article>
      <footer>Copyright 2026 Rodapé do site</footer>
    </body></html>"#.to_string()
}

#[tokio::test]
async fn fetch_content_http_extrai_artigo_real_via_wiremock() {
    use duckduckgo_search_cli::content::extract_http_content;

    std::env::set_var("DUCKDUCKGO_SEARCH_CLI_SKIP_SSRF", "1");
    let _guard = env_lock().lock().await;
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/artigo"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_raw(html_artigo_real().into_bytes(), "text/html; charset=utf-8"),
        )
        .mount(&server)
        .await;

    let cliente = test_client();
    let token = CancellationToken::new();
    let url = format!("{}/artigo", server.uri());

    let result = extract_http_content(&cliente, &url, 2000, &token)
        .await
        .expect("fetch deve ter sucesso");
    let (texto, tamanho_orig) = result.expect("conteúdo presente");
    assert!(
        texto.contains("primeiro parágrafo"),
        "deve conter primeiro parágrafo: {texto:?}"
    );
    assert!(texto.contains("Segundo parágrafo"));
    assert!(texto.contains("Terceiro parágrafo"));
    // Nav and footer must have been removed.
    assert!(!texto.contains("About"));
    assert!(!texto.contains("Copyright"));
    // Tamanho original reportado > 0.
    assert!(tamanho_orig > 0);
}

#[tokio::test]
async fn fetch_content_http_rejects_non_html_content_type() {
    use duckduckgo_search_cli::content::extract_http_content;

    std::env::set_var("DUCKDUCKGO_SEARCH_CLI_SKIP_SSRF", "1");
    let _guard = env_lock().lock().await;
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/pdf"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(vec![0u8; 100], "application/pdf"))
        .mount(&server)
        .await;

    let cliente = test_client();
    let token = CancellationToken::new();
    let url = format!("{}/pdf", server.uri());

    let result = extract_http_content(&cliente, &url, 1000, &token)
        .await
        .expect("request OK");
    assert!(result.is_none(), "Content-Type não HTML deve retornar None");
}

#[tokio::test]
async fn fetch_content_http_decodes_latin1_correctly() {
    use duckduckgo_search_cli::content::extract_http_content;

    std::env::set_var("DUCKDUCKGO_SEARCH_CLI_SKIP_SSRF", "1");
    let _guard = env_lock().lock().await;
    let server = MockServer::start().await;

    // HTML in Latin-1 (ISO-8859-1) containing 'c-cedilla' (0xE7) + 'a-acute' (0xE1).
    let mut html: Vec<u8> = b"<html><body><article>".to_vec();
    // Paragraph long enough to pass the threshold (20+ chars).
    html.extend_from_slice(
        b"<p>Uma a\xe7\xe3o interessante foi realizada pelos desenvolvedores do sistema de busca que n\xe3o pode ser ignorada.</p>"
    );
    html.extend_from_slice(
        b"<p>Outro par\xe1grafo relevante com mais texto para chegar ao limiar m\xednimo de conte\xfado necess\xe1rio.</p>"
    );
    html.extend_from_slice(b"</article></body></html>");

    Mock::given(method("GET"))
        .and(path("/latin1"))
        .respond_with(
            ResponseTemplate::new(200).set_body_raw(html, "text/html; charset=iso-8859-1"),
        )
        .mount(&server)
        .await;

    let cliente = test_client();
    let token = CancellationToken::new();
    let url = format!("{}/latin1", server.uri());

    let result = extract_http_content(&cliente, &url, 2000, &token)
        .await
        .expect("fetch deve ter sucesso");
    let (texto, _) = result.expect("conteúdo presente");
    // 'acao' with accent must be present (correctly decoded from Latin-1).
    assert!(
        texto.contains("ação") || texto.contains("parágrafo"),
        "texto deve ter acentuação UTF-8 correta: {texto:?}"
    );
}

// Sanity check of the extract_pagination_tokens helper (extra integration-level
// coverage to ensure the helper is exposed and works).
#[test]
fn sanity_extrair_tokens_paginacao_via_lib_publica() {
    let html = html_with_vqd_tokens_and_results("v1", "0", "10", &["a", "b"]);
    let (vqd, s, dc) = extract_pagination_tokens(&html).expect("tokens presentes");
    assert_eq!(vqd, "v1");
    assert_eq!(s, "0");
    assert_eq!(dc, "10");
}

#[test]
fn ndjson_serializes_search_output_in_valid_single_line() {
    use duckduckgo_search_cli::types::{SearchMetadata, SearchOutput, SearchResult};
    let output = SearchOutput {
        query: "rust".into(),
        engine: "duckduckgo".into(),
        endpoint: "html".into(),
        timestamp: "2026-04-14T00:00:00Z".into(),
        region: "br-pt".into(),
        result_count: 1,
        results: vec![SearchResult {
            position: 1,
            title: "Exemplo com\nnova linha".to_string(),
            url: "https://exemplo.com".to_string(),
            display_url: None,
            snippet: None,
            original_title: None,
            content: None,
            content_size: None,
            content_extraction_method: None,
        }],
        pages_fetched: 1,
        error: None,
        message: None,
        metadata: SearchMetadata {
            execution_time_ms: 100,
            selectors_hash: "abc123".into(),
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
    let linha = serde_json::to_string(&output).expect("serializar NDJSON");
    // A single line (no intermediate \n — \n inside the title is escaped as \\n).
    assert!(
        !linha.contains('\n'),
        "NDJSON deve ser UMA linha só — \\n literais no conteúdo DEVEM estar escapados"
    );
    let parsed: serde_json::Value =
        serde_json::from_str(&linha).expect("NDJSON deve ser JSON válido");
    assert_eq!(parsed["query"], "rust");
    assert_eq!(parsed["quantidade_resultados"], 1);
}

// ---------------------------------------------------------------------------
// Test v0.4.0 #1: default --num 15 + auto-pagination to 2 pages.
//
// Simulates the configuration that `montar_configuracoes` would produce when the user
// does NOT pass `--num` (default=15) and `--pages` is at default (1): it raises
// `paginas` to 2 and fixes `num_resultados = Some(15)`. The test verifies that
// `search_with_pagination` respects this: GETs page 1, POSTs page 2
// (with vqd), aggregates 11 + 10 = 21 results and truncates at 15.
// ---------------------------------------------------------------------------
#[tokio::test]
async fn testa_default_num_15_auto_pagina_2_paginas() {
    let _g = env_lock().lock().await;
    let mock_server = MockServer::start().await;

    // Page 1 — 11 results (typical count for the first DDG page).
    let titles_pg1: Vec<String> = (1..=11).map(|i| format!("Res Pg1 {i}")).collect();
    let refs_pg1: Vec<&str> = titles_pg1.iter().map(String::as_str).collect();
    let html_pg1 = html_with_vqd_tokens_and_results("vqd-auto-pg1", "0", "30", &refs_pg1);
    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(html_pg1)
                .insert_header("content-type", "text/html; charset=utf-8"),
        )
        .mount(&mock_server)
        .await;

    // Page 2 — 10 additional results. The search will come via POST with vqd-auto-pg1.
    let titles_pg2: Vec<String> = (1..=10).map(|i| format!("Res Pg2 {i}")).collect();
    let refs_pg2: Vec<&str> = titles_pg2.iter().map(String::as_str).collect();
    let html_pg2 = html_with_vqd_tokens_and_results("vqd-auto-pg2", "30", "60", &refs_pg2);
    Mock::given(method("POST"))
        .and(path("/"))
        .and(body_string_contains("vqd=vqd-auto-pg1"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(html_pg2)
                .insert_header("content-type", "text/html; charset=utf-8"),
        )
        .with_priority(1)
        .mount(&mock_server)
        .await;

    let base = format!("{}/", mock_server.uri());
    let _env = EnvGuard::set(&[
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_HTML", base.clone()),
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_LITE", base),
    ]);

    let cliente = test_client();
    // Simulate the POST-montar_configuracoes configuration with default --num 15
    // and auto-pagination to 2 pages (new behavior in v0.4.0).
    let mut cfg = base_config(Endpoint::Html, 2, 0);
    cfg.num_results = Some(15);
    let flag = Arc::new(AtomicBool::new(false));
    let token = CancellationToken::new();

    let agregado = search_with_pagination(&cliente, &cfg, "rust", &flag, &token)
        .await
        .expect("auto-paginação deve funcionar");

    assert_eq!(
        agregado.results.len(),
        15,
        "deve truncar em --num=15 após agregar 21 resultados de 2 páginas"
    );
    assert_eq!(
        agregado.pages_fetched, 2,
        "deve ter buscado exatamente 2 páginas"
    );
    assert_eq!(agregado.results[0].title, "Res Pg1 1");
    // Page 2 starts after the 11 from page 1 → positions 12..=15 come from page 2.
    assert_eq!(agregado.results[11].title, "Res Pg2 1");
    assert_eq!(agregado.results[14].title, "Res Pg2 4");
}

// ---------------------------------------------------------------------------
// Test v0.4.0 #2: auto-pagination RESPECTS explicit --pages from the user.
//
// When the user passes --pages 3 explicitly, the auto-pagination logic
// does NOT override it. This test simulates cfg with paginas=3 + num=15 and verifies
// that search_with_pagination runs 3 pages (and truncates at 15).
// ---------------------------------------------------------------------------
#[tokio::test]
async fn test_auto_pagination_respects_explicit_pages() {
    let _g = env_lock().lock().await;
    let mock_server = MockServer::start().await;

    // Page 1 — 11 results.
    let titles_pg1: Vec<String> = (1..=11).map(|i| format!("Pg1-{i}")).collect();
    let refs_pg1: Vec<&str> = titles_pg1.iter().map(String::as_str).collect();
    let html_pg1 = html_with_vqd_tokens_and_results("v-expl-1", "0", "30", &refs_pg1);
    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(html_pg1)
                .insert_header("content-type", "text/html; charset=utf-8"),
        )
        .mount(&mock_server)
        .await;

    // Page 2 — 5 results.
    let titles_pg2: Vec<String> = (1..=5).map(|i| format!("Pg2-{i}")).collect();
    let refs_pg2: Vec<&str> = titles_pg2.iter().map(String::as_str).collect();
    let html_pg2 = html_with_vqd_tokens_and_results("v-expl-2", "30", "60", &refs_pg2);
    Mock::given(method("POST"))
        .and(path("/"))
        .and(body_string_contains("vqd=v-expl-1"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(html_pg2)
                .insert_header("content-type", "text/html; charset=utf-8"),
        )
        .with_priority(1)
        .mount(&mock_server)
        .await;

    // Page 3 — 3 results.
    let titles_pg3: Vec<String> = (1..=3).map(|i| format!("Pg3-{i}")).collect();
    let refs_pg3: Vec<&str> = titles_pg3.iter().map(String::as_str).collect();
    let html_pg3 = html_with_vqd_tokens_and_results("v-expl-3", "60", "90", &refs_pg3);
    Mock::given(method("POST"))
        .and(path("/"))
        .and(body_string_contains("vqd=v-expl-2"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(html_pg3)
                .insert_header("content-type", "text/html; charset=utf-8"),
        )
        .with_priority(2)
        .mount(&mock_server)
        .await;

    let base = format!("{}/", mock_server.uri());
    let _env = EnvGuard::set(&[
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_HTML", base.clone()),
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_LITE", base),
    ]);

    let cliente = test_client();
    // Simulate --num 15 --pages 3 (explicit) → montar_configuracoes does NOT override.
    let mut cfg = base_config(Endpoint::Html, 3, 0);
    cfg.num_results = Some(15);
    let flag = Arc::new(AtomicBool::new(false));
    let token = CancellationToken::new();

    let agregado = search_with_pagination(&cliente, &cfg, "rust", &flag, &token)
        .await
        .expect("deve buscar 3 páginas conforme --pages explícito");

    assert_eq!(
        agregado.pages_fetched, 3,
        "deve respeitar --pages=3 explícito"
    );
    // 11 + 5 + 3 = 19 agregados, truncados em 15.
    assert_eq!(
        agregado.results.len(),
        15,
        "19 agregados devem ser truncados em num=15"
    );
    assert_eq!(agregado.results[0].title, "Pg1-1");
    assert_eq!(agregado.results[11].title, "Pg2-1");
}

// ---------------------------------------------------------------------------
// Test 15: HTTP 202 on the first attempt → recovers on the second with 200 OK.
// Verifies that `flag_rate_limit` was activated and result comes with success.
// ---------------------------------------------------------------------------
#[tokio::test]
async fn testa_202_recupera_na_segunda_tentativa() {
    let _g = env_lock().lock().await;
    let mock_server = MockServer::start().await;

    // First attempt returns 202 (DDG anti-bot anomaly).
    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(ResponseTemplate::new(202).set_body_string(""))
        .up_to_n_times(1)
        .with_priority(1)
        .mount(&mock_server)
        .await;

    // Second attempt returns 200 with valid HTML.
    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(html_with_3_results_class())
                .insert_header("content-type", "text/html; charset=utf-8"),
        )
        .with_priority(2)
        .mount(&mock_server)
        .await;

    let base = format!("{}/", mock_server.uri());
    let _env = EnvGuard::set(&[
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_HTML", base.clone()),
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_LITE", base),
    ]);

    let cliente = test_client();
    // 1 retry → up to 2 attempts total.
    let cfg = base_config(Endpoint::Html, 1, 1);
    let flag = Arc::new(AtomicBool::new(false));
    let token = CancellationToken::new();

    let agregado = search_with_pagination(&cliente, &cfg, "rust", &flag, &token)
        .await
        .expect("deve recuperar após 202 na primeira tentativa");

    assert_eq!(
        agregado.results.len(),
        3,
        "deve ter extraído 3 resultados após recuperação do 202"
    );
    assert!(
        flag.load(std::sync::atomic::Ordering::Relaxed),
        "flag_rate_limit deve ter sido ativada pelo HTTP 202"
    );
}

// ---------------------------------------------------------------------------
// Test 16: HTTP 202 on ALL attempts → `RetryFailReason::Blocked`.
// Verifies that `flag_rate_limit` was activated and the CLI terminates with Blocked.
// ---------------------------------------------------------------------------
#[tokio::test]
async fn test_202_exhausts_retries_returns_blocked() {
    let _g = env_lock().lock().await;
    let mock_server = MockServer::start().await;

    // All attempts return 202 (DDG never lets through).
    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(ResponseTemplate::new(202).set_body_string(""))
        .mount(&mock_server)
        .await;

    let base = format!("{}/", mock_server.uri());
    let _env = EnvGuard::set(&[
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_HTML", base.clone()),
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_LITE", base),
    ]);

    let cliente = test_client();
    // 1 retry → up to 2 attempts (exhausted at 202).
    let cfg = base_config(Endpoint::Html, 1, 1);
    let flag = Arc::new(AtomicBool::new(false));
    let token = CancellationToken::new();

    let result = execute_with_retry(
        &cliente,
        &format!("{}/", mock_server.uri()),
        cfg.retries,
        &flag,
        &token,
    )
    .await;

    match result {
        Err(RetryFailReason::Blocked) => {}
        other => panic!(
            "esperava RetryFailReason::Blocked após esgotar retries com 202, recebi {other:?}"
        ),
    }
    assert!(
        flag.load(std::sync::atomic::Ordering::Relaxed),
        "flag_rate_limit deve ter sido ativada pelo HTTP 202"
    );
}
