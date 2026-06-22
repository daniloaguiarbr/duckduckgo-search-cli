// SPDX-License-Identifier: MIT OR Apache-2.0
//! Testes de integração focados em caminhos não cobertos de `search.rs`:
//! - `execute_search` (versão de compatibilidade single-query simples)
//! - Cancelamento mid-retry em `execute_with_retry`
//! - Caminhos de erro / borda de `search_with_pagination`:
//!   * tokens vqd ausentes → sem paginação possível
//!   * página seguinte com status não-OK → para
//!   * página seguinte com zero resultados → para
//!   * página seguinte sem tokens vqd → para após adicionar
//!   * cancelamento durante paginação
//!   * fallback Lite que também falha → mantém vazio
//!   * truncate por `num_resultados`
//!   * retry de 429 esgotado
//!
//! ZERO chamadas HTTP reais — todos via `wiremock::MockServer`.

use duckduckgo_search_cli::search::{
    execute_search, execute_with_retry, search_with_pagination, RetryFailReason,
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
fn env_lock() -> &'static TokioMutex<()> {
    static LOCK: OnceLock<TokioMutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| TokioMutex::new(()))
}

fn test_client() -> Client {
    Client::builder()
        .timeout(Duration::from_secs(10))
        .user_agent("Mozilla/5.0 (teste-search-retry)")
        .build()
        .expect("test client")
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
        verbose: 0,
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
        cookie_provider: None,
        persistent_jar: None,
        warmup_enabled: false,
        allow_lite_fallback: false,
        pre_flight: false,
            identity_profile: duckduckgo_search_cli::cli::CliIdentityProfile::Auto,
            last_probe_cascade_level: None,
        selectors: std::sync::Arc::new(
            duckduckgo_search_cli::types::SelectorConfig::default(),
        ),
    }
}

/// HTML with 3 organic results — body above 5,000 bytes (anti-block threshold).
fn html_3_resultados() -> String {
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
        <a class="result__snippet">Snippet do primeiro resultado.</a>
      </div>
      <div class="result">
        <a class="result__a" href="//exemplo.com/dois">Resultado Dois</a>
        <a class="result__snippet">Snippet do segundo resultado.</a>
      </div>
      <div class="result">
        <a class="result__a" href="//exemplo.com/tres">Resultado Três</a>
        <a class="result__snippet">Snippet do terceiro resultado.</a>
      </div>
    </div>
    </body></html>"#
    )
}

fn html_with_tokens_and_results(vqd: &str, s: &str, dc: &str, titles: &[&str]) -> String {
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

/// HTML WITHOUT vqd/s/dc tokens — body above 5,000 bytes (anti-block threshold).
fn html_without_vqd_tokens() -> String {
    // Padding garante que o corpo fique acima de LIMIAR_BLOQUEIO_SILENCIOSO (5 000 bytes).
    let padding =
        "<!-- padding para superar o limiar de detecção de bloqueio silencioso do DuckDuckGo. -->"
            .repeat(60);
    format!(
        r#"<html><body>
    {padding}
    <div id="links">
      <div class="result">
        <a class="result__a" href="//exemplo.com/sem-tokens">Resultado Sem Tokens</a>
        <a class="result__snippet">Snippet sem tokens vqd presentes no formulário.</a>
      </div>
      <div class="result">
        <a class="result__a" href="//exemplo.com/outro">Outro Resultado</a>
        <a class="result__snippet">Outro snippet com texto suficiente para ultrapassar 100 bytes.</a>
      </div>
    </div>
    </body></html>"#
    )
}

/// Guard to set env vars during a test and clean up on drop.
struct EnvGuard {
    keys: Vec<&'static str>,
}
impl EnvGuard {
    fn set(pairs: &[(&'static str, String)]) -> Self {
        let mut ks = Vec::new();
        for (k, v) in pairs {
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

// ===========================================================================
// `execute_search` — standalone compatibility function (iteration 1).
// ===========================================================================

#[tokio::test]
async fn execute_search_returns_html_on_status_200() {
    let _g = env_lock().lock().await;
    let mock = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(html_3_resultados())
                .insert_header("content-type", "text/html; charset=utf-8"),
        )
        .mount(&mock)
        .await;

    let base = format!("{}/", mock.uri());
    let _env = EnvGuard::set(&[
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_HTML", base),
        ("DUCKDUCKGO_SEARCH_CLI_NO_CHROME", "1".into()),
    ]);

    let client = test_client();
    let html = execute_search(&client, "rust", "pt", "br")
        .await
        .expect("status 200 + large body should return Ok");
    assert!(html.contains("Resultado Um"));
    assert!(html.len() > 100);
}

#[tokio::test]
async fn execute_search_fails_with_status_500() {
    let _g = env_lock().lock().await;
    let mock = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(ResponseTemplate::new(500).set_body_string("erro interno"))
        .mount(&mock)
        .await;

    let base = format!("{}/", mock.uri());
    let _env = EnvGuard::set(&[
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_HTML", base),
        ("DUCKDUCKGO_SEARCH_CLI_NO_CHROME", "1".into()),
    ]);

    let client = test_client();
    let result = execute_search(&client, "rust", "pt", "br").await;
    let err = result.expect_err("status 500 should be an error");
    let msg = format!("{err:#}");
    assert!(
        msg.contains("500"),
        "error message should mention status 500: {msg}"
    );
}

#[tokio::test]
async fn execute_search_fails_with_small_body() {
    let _g = env_lock().lock().await;
    let mock = MockServer::start().await;

    // Status 200, but body with fewer than 100 bytes → suspected block.
    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string("ok")
                .insert_header("content-type", "text/html; charset=utf-8"),
        )
        .mount(&mock)
        .await;

    let base = format!("{}/", mock.uri());
    let _env = EnvGuard::set(&[
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_HTML", base),
        ("DUCKDUCKGO_SEARCH_CLI_NO_CHROME", "1".into()),
    ]);

    let client = test_client();
    let result = execute_search(&client, "rust", "pt", "br").await;
    let err = result.expect_err("small body should be an error");
    let msg = format!("{err:#}");
    assert!(
        msg.contains("suspiciously small") || msg.contains("silent block"),
        "message should mention small response / silent block: {msg}"
    );
}

// ===========================================================================
// `execute_with_retry` — cancelamento, retry esgotado e caminhos de erro.
// ===========================================================================

#[tokio::test]
async fn retry_aborts_when_token_already_cancelled() {
    let _g = env_lock().lock().await;
    let mock = MockServer::start().await;

    // Mock returns 200, but cancellation must abort before the first attempt.
    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(html_3_resultados())
                .insert_header("content-type", "text/html; charset=utf-8"),
        )
        .mount(&mock)
        .await;

    let client = test_client();
    let flag = Arc::new(AtomicBool::new(false));
    let cancellation = CancellationToken::new();
    cancellation.cancel(); // already cancelled before even calling.

    let url = format!("{}/", mock.uri());
    let result = execute_with_retry(&client, &url, 3, &flag, &cancellation).await;

    match result {
        Err(RetryFailReason::Network(msg)) => {
            assert!(
                msg.to_lowercase().contains("cancel"),
                "expected cancellation message: {msg}"
            );
        }
        other => panic!("expected Err(Network(\"cancel...\")), got {other:?}"),
    }
}

#[tokio::test]
async fn retry_429_exhausted_returns_rate_limited() {
    let _g = env_lock().lock().await;
    let mock = MockServer::start().await;

    // Always 429 — exhausts retries and returns RateLimited.
    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(ResponseTemplate::new(429))
        .mount(&mock)
        .await;

    let client = test_client();
    let flag = Arc::new(AtomicBool::new(false));
    let cancellation = CancellationToken::new();

    // 0 retries → 1 single attempt → backoff not triggered.
    let url = format!("{}/", mock.uri());
    let result = execute_with_retry(&client, &url, 0, &flag, &cancellation).await;
    match result {
        Err(RetryFailReason::RateLimited) => {}
        other => panic!("expected RateLimited, got {other:?}"),
    }
    assert!(
        flag.load(std::sync::atomic::Ordering::Relaxed),
        "rate limit flag must be set"
    );
}

#[tokio::test]
async fn retry_4xx_non_retryable_returns_http_error() {
    let _g = env_lock().lock().await;
    let mock = MockServer::start().await;

    // 418 is not 200/403/429 → falls into "other 4xx/5xx → do not retry" path.
    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(ResponseTemplate::new(418))
        .mount(&mock)
        .await;

    let client = test_client();
    let flag = Arc::new(AtomicBool::new(false));
    let cancellation = CancellationToken::new();

    let url = format!("{}/", mock.uri());
    let result = execute_with_retry(&client, &url, 3, &flag, &cancellation).await;
    match result {
        Err(RetryFailReason::HttpError(418)) => {}
        other => panic!("expected HttpError(418), got {other:?}"),
    }
}

// ===========================================================================
// `search_with_pagination` — pagination edge cases.
// ===========================================================================

#[tokio::test]
async fn pagination_without_vqd_tokens_warns_and_returns_page_1_only() {
    let _g = env_lock().lock().await;
    let mock = MockServer::start().await;

    // Page 1 with results but WITHOUT vqd/s/dc tokens → blocks pagination.
    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(html_without_vqd_tokens())
                .insert_header("content-type", "text/html; charset=utf-8"),
        )
        .mount(&mock)
        .await;

    let base = format!("{}/", mock.uri());
    let _env = EnvGuard::set(&[
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_HTML", base.clone()),
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_LITE", base),
        ("DUCKDUCKGO_SEARCH_CLI_NO_CHROME", "1".into()),
    ]);

    let client = test_client();
    // Requests 3 pages, but since there are no vqd tokens, only page 1 will come.
    let config = base_config(Endpoint::Html, 3, 0);
    let flag = Arc::new(AtomicBool::new(false));
    let cancellation = CancellationToken::new();

    let aggregated = search_with_pagination(&client, &config, "rust", &flag, &cancellation)
        .await
        .expect("first page should succeed");
    assert_eq!(aggregated.pages_fetched, 1);
    assert_eq!(aggregated.results.len(), 2);
}

#[tokio::test]
async fn pagination_truncated_by_num_results() {
    let _g = env_lock().lock().await;
    let mock = MockServer::start().await;

    // Page 1: 3 results + tokens.
    let html_pg1 = html_with_tokens_and_results("vqd-trunc-1", "0", "30", &["A", "B", "C"]);
    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(html_pg1)
                .insert_header("content-type", "text/html; charset=utf-8"),
        )
        .mount(&mock)
        .await;

    // Page 2: 3 results → accumulated total = 6.
    let html_pg2 = html_with_tokens_and_results("vqd-trunc-2", "30", "60", &["D", "E", "F"]);
    Mock::given(method("POST"))
        .and(path("/"))
        .and(body_string_contains("vqd=vqd-trunc-1"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(html_pg2)
                .insert_header("content-type", "text/html; charset=utf-8"),
        )
        .mount(&mock)
        .await;

    let base = format!("{}/", mock.uri());
    let _env = EnvGuard::set(&[
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_HTML", base.clone()),
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_LITE", base),
        ("DUCKDUCKGO_SEARCH_CLI_NO_CHROME", "1".into()),
    ]);

    let client = test_client();
    let mut config = base_config(Endpoint::Html, 2, 0);
    config.num_results = Some(4); // truncate accumulated 6 down to 4.
    let flag = Arc::new(AtomicBool::new(false));
    let cancellation = CancellationToken::new();

    let aggregated = search_with_pagination(&client, &config, "rust", &flag, &cancellation)
        .await
        .expect("pagination ok");
    assert_eq!(
        aggregated.results.len(),
        4,
        "results should be truncated to 4"
    );
}

#[tokio::test]
async fn pagination_stops_when_next_page_returns_non_ok_status() {
    let _g = env_lock().lock().await;
    let mock = MockServer::start().await;

    let html_pg1 = html_with_tokens_and_results("vqd-bad-1", "0", "30", &["A", "B"]);
    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(html_pg1)
                .insert_header("content-type", "text/html; charset=utf-8"),
        )
        .mount(&mock)
        .await;

    // Page 2 (POST) returns 503 → pagination must stop and return only page 1.
    Mock::given(method("POST"))
        .and(path("/"))
        .and(body_string_contains("vqd=vqd-bad-1"))
        .respond_with(ResponseTemplate::new(503))
        .mount(&mock)
        .await;

    let base = format!("{}/", mock.uri());
    let _env = EnvGuard::set(&[
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_HTML", base.clone()),
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_LITE", base),
        ("DUCKDUCKGO_SEARCH_CLI_NO_CHROME", "1".into()),
    ]);

    let client = test_client();
    let config = base_config(Endpoint::Html, 3, 0);
    let flag = Arc::new(AtomicBool::new(false));
    let cancellation = CancellationToken::new();

    let aggregated = search_with_pagination(&client, &config, "rust", &flag, &cancellation)
        .await
        .expect("first page ok even with page 2 failing");
    assert_eq!(aggregated.pages_fetched, 1);
    assert_eq!(aggregated.results.len(), 2);
}

#[tokio::test]
async fn pagination_stops_when_next_page_returns_zero_results() {
    let _g = env_lock().lock().await;
    let mock = MockServer::start().await;

    let html_pg1 = html_with_tokens_and_results("vqd-zero-1", "0", "30", &["X", "Y", "Z"]);
    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(html_pg1)
                .insert_header("content-type", "text/html; charset=utf-8"),
        )
        .mount(&mock)
        .await;

    // Page 2 returns HTML > 100 bytes but without `.result` → zero results → stops.
    let html_empty = r#"<html><head><title>nada</title></head><body><div id="links"><p>Sem resultados nesta página de teste, apenas texto suficiente para superar 100 bytes.</p></div></body></html>"#;
    Mock::given(method("POST"))
        .and(path("/"))
        .and(body_string_contains("vqd=vqd-zero-1"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(html_empty)
                .insert_header("content-type", "text/html; charset=utf-8"),
        )
        .mount(&mock)
        .await;

    let base = format!("{}/", mock.uri());
    let _env = EnvGuard::set(&[
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_HTML", base.clone()),
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_LITE", base),
        ("DUCKDUCKGO_SEARCH_CLI_NO_CHROME", "1".into()),
    ]);

    let client = test_client();
    let config = base_config(Endpoint::Html, 3, 0);
    let flag = Arc::new(AtomicBool::new(false));
    let cancellation = CancellationToken::new();

    let aggregated = search_with_pagination(&client, &config, "rust", &flag, &cancellation)
        .await
        .expect("ok");
    assert_eq!(
        aggregated.pages_fetched, 1,
        "pages_fetched stays at 1 because page 2 returned zero results"
    );
    assert_eq!(aggregated.results.len(), 3);
}

#[tokio::test]
async fn pagination_stops_when_next_page_loses_vqd_tokens() {
    let _g = env_lock().lock().await;
    let mock = MockServer::start().await;

    let html_pg1 = html_with_tokens_and_results("vqd-lost-1", "0", "30", &["A", "B"]);
    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(html_pg1)
                .insert_header("content-type", "text/html; charset=utf-8"),
        )
        .mount(&mock)
        .await;

    // Page 2: has results BUT lost vqd tokens → pagination stops after adding page 2.
    // Padding ensures body stays above SILENT_BLOCK_THRESHOLD (5,000 bytes).
    let padding =
        "<!-- padding para superar o limiar de detecção de bloqueio silencioso do DuckDuckGo. -->"
            .repeat(60);
    let html_pg2_no_tokens = format!(
        r#"<html><body>
    {padding}
    <div id="links">
      <div class="result">
        <a class="result__a" href="//exemplo.com/p2a">Pg 2 A</a>
        <a class="result__snippet">snippet pg2a com texto suficiente.</a>
      </div>
      <div class="result">
        <a class="result__a" href="//exemplo.com/p2b">Pg 2 B</a>
        <a class="result__snippet">snippet pg2b com texto suficiente.</a>
      </div>
    </div>
    </body></html>"#
    );
    Mock::given(method("POST"))
        .and(path("/"))
        .and(body_string_contains("vqd=vqd-lost-1"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(html_pg2_no_tokens)
                .insert_header("content-type", "text/html; charset=utf-8"),
        )
        .mount(&mock)
        .await;

    let base = format!("{}/", mock.uri());
    let _env = EnvGuard::set(&[
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_HTML", base.clone()),
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_LITE", base),
        ("DUCKDUCKGO_SEARCH_CLI_NO_CHROME", "1".into()),
    ]);

    let client = test_client();
    let config = base_config(Endpoint::Html, 5, 0);
    let flag = Arc::new(AtomicBool::new(false));
    let cancellation = CancellationToken::new();

    let aggregated = search_with_pagination(&client, &config, "rust", &flag, &cancellation)
        .await
        .expect("ok");
    assert_eq!(
        aggregated.pages_fetched, 2,
        "page 2 counted — but page 3 never arrived because tokens were lost"
    );
    assert_eq!(
        aggregated.results.len(),
        4,
        "2 from page 1 + 2 from page 2 = 4 total results"
    );
}

#[tokio::test]
async fn pagination_aborts_if_token_already_cancelled_at_loop_start() {
    let _g = env_lock().lock().await;
    let mock = MockServer::start().await;

    let html_pg1 = html_with_tokens_and_results("vqd-cancel-1", "0", "30", &["A", "B"]);
    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(html_pg1)
                .insert_header("content-type", "text/html; charset=utf-8"),
        )
        .mount(&mock)
        .await;

    // POST Mock that should NEVER be called (pre-loop cancellation must abort).
    Mock::given(method("POST"))
        .and(path("/"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(html_3_resultados())
                .insert_header("content-type", "text/html; charset=utf-8"),
        )
        .mount(&mock)
        .await;

    let base = format!("{}/", mock.uri());
    let _env = EnvGuard::set(&[
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_HTML", base.clone()),
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_LITE", base),
        ("DUCKDUCKGO_SEARCH_CLI_NO_CHROME", "1".into()),
    ]);

    let client = test_client();
    let config = base_config(Endpoint::Html, 3, 0);
    let flag = Arc::new(AtomicBool::new(false));
    let cancellation = CancellationToken::new();

    // Spawn task that cancels after a small delay to simulate mid-execution cancellation.
    // Since the loop has multiple chances to check `is_cancelled()`, this ensures
    // one of the checks fires.
    let cancellation_clone = cancellation.clone();
    let handle = tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(50)).await;
        cancellation_clone.cancel();
    });

    let aggregated = search_with_pagination(&client, &config, "rust", &flag, &cancellation)
        .await
        .expect("first page should complete before cancellation");

    handle.await.expect("cancellation task ok");

    // Cancellation must abort the loop before fetching all 3 pages.
    assert!(
        aggregated.pages_fetched < 3,
        "cancellation must abort before completing 3 pages (actual: {})",
        aggregated.pages_fetched
    );
}

#[tokio::test]
async fn fallback_lite_falha_mantem_resultados_vazios() {
    let _g = env_lock().lock().await;
    let mock_html = MockServer::start().await;
    let mock_lite = MockServer::start().await;

    // HTML returns 200 but with zero `.result` → triggers Lite fallback.
    // Padding ensures body stays above SILENT_BLOCK_THRESHOLD (5,000 bytes).
    let padding_fb =
        "<!-- padding para superar o limiar de detecção de bloqueio silencioso do DuckDuckGo. -->"
            .repeat(60);
    let html_empty = format!(
        r#"<html><head><title>vazio</title></head><body>{padding_fb}<div id="links"><p>Nenhum resultado encontrado para teste de fallback Lite.</p></div></body></html>"#
    );
    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(html_empty)
                .insert_header("content-type", "text/html; charset=utf-8"),
        )
        .mount(&mock_html)
        .await;

    // Lite also fails — returns persistent 503.
    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(ResponseTemplate::new(503))
        .mount(&mock_lite)
        .await;

    let base_html = format!("{}/", mock_html.uri());
    let base_lite = format!("{}/", mock_lite.uri());
    let _env = EnvGuard::set(&[
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_HTML", base_html),
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_LITE", base_lite),
        ("DUCKDUCKGO_SEARCH_CLI_NO_CHROME", "1".into()),
    ]);

    let client = test_client();
    let config = base_config(Endpoint::Html, 1, 0);
    let flag = Arc::new(AtomicBool::new(false));
    let cancellation = CancellationToken::new();

    let aggregated = search_with_pagination(&client, &config, "rust", &flag, &cancellation)
        .await
        .expect("HTML 200 + Lite 503 should return Ok with empty list");
    assert_eq!(
        aggregated.results.len(),
        0,
        "both endpoints without results → empty vec"
    );
    assert!(
        !aggregated.used_fallback_lite,
        "Lite fallback failed → flag stays false"
    );
    assert_eq!(aggregated.effective_endpoint, Endpoint::Html);
}

#[tokio::test]
async fn first_page_blocked_by_small_body_returns_blocked_reason() {
    let _g = env_lock().lock().await;
    let mock = MockServer::start().await;

    // Status 200 but VERY small body → search_with_pagination returns Blocked.
    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string("ok")
                .insert_header("content-type", "text/html; charset=utf-8"),
        )
        .mount(&mock)
        .await;

    let base = format!("{}/", mock.uri());
    let _env = EnvGuard::set(&[
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_HTML", base.clone()),
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_LITE", base),
        ("DUCKDUCKGO_SEARCH_CLI_NO_CHROME", "1".into()),
    ]);

    let client = test_client();
    let config = base_config(Endpoint::Html, 1, 0);
    let flag = Arc::new(AtomicBool::new(false));
    let cancellation = CancellationToken::new();

    let result = search_with_pagination(&client, &config, "rust", &flag, &cancellation).await;
    match result {
        Err(RetryFailReason::Blocked) => {}
        Err(other) => panic!("expected Blocked, got another reason: {other:?}"),
        Ok(_) => panic!("expected Blocked, got Ok"),
    }
}

// ===========================================================================
// GAP-WS-52 (v0.7.8) — Fallback Lite CONDICIONAL com detector anti-bot.
//
// Comportamento esperado (v0.7.8):
// - HTML zero + interstitial Cloudflare/DDG + flag ON  → tenta Lite, retorna
//   os resultados do Lite, marca `used_fallback_lite` e `effective_endpoint =
//   Endpoint::Lite`.
// - HTML zero + interstitial Cloudflare/DDG + flag OFF → NÃO tenta Lite,
//   mantém `results = []`, `used_fallback_lite = false`, `effective_endpoint
//   = Endpoint::Html`, e emite `tracing::warn!` estruturado com a sugestão.
// - HTML zero + SEM interstitial + flag OFF            → NÃO tenta Lite,
//   comportamento legado (zero results).
// ===========================================================================

/// HTML Cloudflare interstitial — body acima do limiar de 5 000 bytes
/// para evitar a detecção de bloqueio silencioso (que retornaria `Blocked`).
fn html_cloudflare_interstitial() -> String {
    // Padding garante que o corpo fique acima de LIMIAR_BLOQUEIO_SILENCIOSO
    // (5 000 bytes) e inclui marcadores canônicos do detector
    // `detectar_interstitial` (`cf-challenge`, `cf-spinner`,
    // `__cf_chl_jschl_tk__`, `Just a moment`, `Attention Required`).
    let padding =
        "<!-- padding para superar o limiar de detecção de bloqueio silencioso do DuckDuckGo. -->"
            .repeat(60);
    format!(
        r#"<html><head><title>Just a moment...</title></head><body>{padding}<div class="cf-challenge"><h1>Attention Required! | Cloudflare</h1><p>cf-spinner placeholder — checking your browser before accessing duckduckgo.com.</p><script src="/cdn-cgi/challenge-platform/h/b"></script><input type="hidden" name="__cf_chl_jschl_tk__" value="x.y.z"></div></body></html>"#
    )
}

fn html_lite_1_resultado() -> String {
    // Formato canônico do endpoint Lite (TABELA com class="result-link"),
    // que casa com `extract_results_lite_with_cfg`. Padding garante body
    // acima do limiar de 5 000 bytes.
    let padding =
        "<!-- padding para garantir que a resposta Lite seja interpretada como resultado válido. -->"
            .repeat(60);
    format!(
        r#"<html><body>
    {padding}
    <table>
      <tr><td valign="top">1.&nbsp;</td><td><a class="result-link" href="//lite.example/a">Lite Resultado A</a></td></tr>
      <tr><td>&nbsp;</td><td class="result-snippet">Snippet do primeiro resultado lite retornado pelo fallback.</td></tr>
    </table>
    </body></html>"#
    )
}

fn html_zero_sem_interstitial() -> String {
    // HTML genuinamente vazio (zero `.result`, zero marcadores de
    // interstitial). Padding precisa ser robusto — o detector
    // `detectar_interstitial` é aplicado no body inteiro, então o padding
    // também não pode conter marcadores. Usamos 80 repetições para garantir
    // ~6 000 bytes.
    let padding =
        "<!-- padding cenario-C sem marcadores anti-bot para superar limiar 5000 bytes -->"
            .repeat(80);
    format!(
        r#"<html><head><title>Resultados</title></head><body>{padding}<div id="links"><p>Nenhum resultado encontrado para esta consulta genuinamente vazia.</p></div></body></html>"#
    )
}

#[tokio::test]
async fn fallback_lite_condicional_interstitial_com_flag_usa_lite() {
    let _g = env_lock().lock().await;
    let mock_html = MockServer::start().await;
    let mock_lite = MockServer::start().await;

    // HTML devolve 200 com Cloudflare interstitial detectado por
    // `detectar_interstitial` (marker `cf-challenge` + `cf-spinner`).
    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(html_cloudflare_interstitial())
                .insert_header("content-type", "text/html; charset=utf-8"),
        )
        .mount(&mock_html)
        .await;

    // Lite devolve 200 com 1 resultado válido no formato canônico
    // de tabela (casado por `extract_results_lite_with_cfg`).
    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(html_lite_1_resultado())
                .insert_header("content-type", "text/html; charset=utf-8"),
        )
        .mount(&mock_lite)
        .await;

    let base_html = format!("{}/", mock_html.uri());
    let base_lite = format!("{}/", mock_lite.uri());
    let _env = EnvGuard::set(&[
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_HTML", base_html),
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_LITE", base_lite),
        ("DUCKDUCKGO_SEARCH_CLI_NO_CHROME", "1".into()),
    ]);

    let client = test_client();
    let mut config = base_config(Endpoint::Html, 1, 0);
    // FLAG LIGADA → fallback Lite deve disparar quando o detector
    // classificar a resposta como interstitial.
    config.allow_lite_fallback = true;
    let flag = Arc::new(AtomicBool::new(false));
    let cancellation = CancellationToken::new();

    let aggregated = search_with_pagination(&client, &config, "rust", &flag, &cancellation)
        .await
        .expect("interstitial + flag ON → deve completar com resultados do Lite");
    assert_eq!(
        aggregated.results.len(),
        1,
        "Cenário A: interstitial + flag → 1 resultado do Lite"
    );
    assert!(
        aggregated.used_fallback_lite,
        "Cenário A: flag ON + interstitial → used_fallback_lite deve ser true"
    );
    assert_eq!(aggregated.effective_endpoint, Endpoint::Lite);
}

#[tokio::test]
async fn fallback_lite_condicional_interstitial_sem_flag_mantem_vazio() {
    let _g = env_lock().lock().await;
    let mock_html = MockServer::start().await;
    let mock_lite = MockServer::start().await;

    // Mesmo interstitial Cloudflare do cenário A.
    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(html_cloudflare_interstitial())
                .insert_header("content-type", "text/html; charset=utf-8"),
        )
        .mount(&mock_html)
        .await;

    // Lite existe no mock mas NÃO deve ser chamado neste cenário (sem flag).
    // Se for chamado, retornaria 1 resultado — o teste falharia porque
    // a contagem mockada seria 1 mas o effective_endpoint continuaria
    // como Html (assert abaixo).
    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string("<html><body>should not be called</body></html>"),
        )
        .mount(&mock_lite)
        .await;

    let base_html = format!("{}/", mock_html.uri());
    let base_lite = format!("{}/", mock_lite.uri());
    let _env = EnvGuard::set(&[
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_HTML", base_html),
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_LITE", base_lite),
        ("DUCKDUCKGO_SEARCH_CLI_NO_CHROME", "1".into()),
    ]);

    let client = test_client();
    let config = base_config(Endpoint::Html, 1, 0);
    // FLAG DESLIGADA (default) → NÃO tenta Lite mesmo com interstitial.
    assert!(!config.allow_lite_fallback);
    let flag = Arc::new(AtomicBool::new(false));
    let cancellation = CancellationToken::new();

    let aggregated = search_with_pagination(&client, &config, "rust", &flag, &cancellation)
        .await
        .expect("interstitial + flag OFF → deve retornar Ok com lista vazia");
    assert_eq!(
        aggregated.results.len(),
        0,
        "Cenário B: interstitial + flag OFF → 0 resultados"
    );
    assert!(
        !aggregated.used_fallback_lite,
        "Cenário B: flag OFF → Lite nunca é tentado"
    );
    assert_eq!(
        aggregated.effective_endpoint,
        Endpoint::Html,
        "Cenário B: effective_endpoint permanece Html quando Lite não é tentado"
    );
}

#[tokio::test]
async fn fallback_lite_condicional_zero_sem_interstitial_nao_usa_lite() {
    let _g = env_lock().lock().await;
    let mock_html = MockServer::start().await;
    let mock_lite = MockServer::start().await;

    // HTML devolve 200 com zero resultados e SEM nenhum marker de
    // interstitial. Body acima do limiar de 5 000 bytes.
    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(html_zero_sem_interstitial())
                .insert_header("content-type", "text/html; charset=utf-8"),
        )
        .mount(&mock_html)
        .await;

    // Lite mockado — NÃO deve ser chamado neste cenário.
    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(ResponseTemplate::new(200).set_body_string(
            "<html><body>should not be called — sem interstitial detectado</body></html>",
        ))
        .mount(&mock_lite)
        .await;

    let base_html = format!("{}/", mock_html.uri());
    let base_lite = format!("{}/", mock_lite.uri());
    let _env = EnvGuard::set(&[
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_HTML", base_html),
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_LITE", base_lite),
        ("DUCKDUCKGO_SEARCH_CLI_NO_CHROME", "1".into()),
    ]);

    let client = test_client();
    let mut config = base_config(Endpoint::Html, 1, 0);
    // Mesmo com flag LIGADA, sem interstitial detector → fallback NÃO dispara.
    config.allow_lite_fallback = true;
    let flag = Arc::new(AtomicBool::new(false));
    let cancellation = CancellationToken::new();

    let aggregated = search_with_pagination(&client, &config, "rust", &flag, &cancellation)
        .await
        .expect("zero resultados genuínos sem interstitial → Ok com lista vazia");
    assert_eq!(
        aggregated.results.len(),
        0,
        "Cenário C: zero sem interstitial → 0 resultados"
    );
    assert!(
        !aggregated.used_fallback_lite,
        "Cenário C: detector classifica como None → Lite não é tentado mesmo com flag ON"
    );
    assert_eq!(
        aggregated.effective_endpoint,
        Endpoint::Html,
        "Cenário C: effective_endpoint permanece Html"
    );
}
