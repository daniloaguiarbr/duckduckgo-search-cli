// SPDX-License-Identifier: MIT OR Apache-2.0
//! Testes de integração para `content_fetch::enrich_with_content` via `wiremock`.
//!
//! Cobrem o CAMINHO FELIZ (HTTP retorna HTML válido → snippet preenchido) que não
//! é exercitado pelos testes unitários inline do módulo (que cobrem no-op e cancelado).

use duckduckgo_search_cli::content_fetch::enrich_with_content;
use duckduckgo_search_cli::types::{
    Config, Endpoint, OutputFormat, SafeSearch, SearchMetadata, SearchOutput, SearchResult,
};
use reqwest::Client;
use std::time::Duration;
use tokio_util::sync::CancellationToken;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn cfg(parallelism: u32) -> Config {
    Config {
        query: "q".into(),
        queries: vec!["q".into()],
        num_results: None,
        format: OutputFormat::Json,
        timeout_seconds: 5,
        language: "pt".into(),
        country: "br".into(),
        verbose: 0,
        quiet: true,
        user_agent: "Mozilla/5.0 (teste)".into(),
        browser_profile: duckduckgo_search_cli::http::create_browser_profile("Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/146.0.0.0 Safari/537.36"),
        parallelism,
        pages: 1,
        retries: 0,
        endpoint: Endpoint::Html,
        time_filter: None,
        safe_search: SafeSearch::Moderate,
        stream_mode: false,
        output_file: None,
        fetch_content: true,
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

fn output_with_urls(urls: &[&str]) -> SearchOutput {
    let results: Vec<SearchResult> = urls
        .iter()
        .enumerate()
        .map(|(i, u)| SearchResult {
            position: (i + 1) as u32,
            title: format!("Titulo {i}"),
            url: (*u).to_string(),
            display_url: None,
            snippet: Some(format!("snippet {i}")),
            original_title: None,
            content: None,
            content_size: None,
            content_extraction_method: None,
        })
        .collect();
    SearchOutput {
        query: "q".into(),
        engine: "duckduckgo".into(),
        endpoint: "html".into(),
        timestamp: "2026-04-14T00:00:00Z".into(),
        region: "br-pt".into(),
        result_count: results.len() as u32,
        results,
        pages_fetched: 1,
        error: None,
        message: None,
        metadata: SearchMetadata {
            execution_time_ms: 0,
            selectors_hash: "x".into(),
            retries: 0,
            retries_configured: None,
            used_fallback_endpoint: false,
            concurrent_fetches: 0,
            fetch_successes: 0,
            fetch_failures: 0,
            used_chrome: false,
            chrome_attempted: false,
            user_agent: "ua".into(),
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
    }
}

fn artigo_html(titulo: &str) -> String {
    // Realistic HTML for readability: <article> + several long paragraphs.
    let paragrafos: Vec<String> = (0..5)
        .map(|i| {
            format!(
                "<p>Este é o parágrafo número {i} do artigo sobre {titulo}, \
                 com texto suficiente para ultrapassar o threshold de 200 caracteres \
                 e convencer o extrator de que há conteúdo relevante a preservar.</p>"
            )
        })
        .collect();
    format!(
        "<html><head><title>{titulo}</title></head><body>\
         <nav>menu</nav>\
         <article>{}</article>\
         <footer>rodapé</footer>\
         </body></html>",
        paragrafos.join("")
    )
}

// ---------------------------------------------------------------------------
// T1: caminho feliz — 2 URLs distintas, HTTP retorna HTML com artigo → ambos enriquecidos.
// ---------------------------------------------------------------------------
#[tokio::test]
async fn enriquece_duas_urls_via_http_puro_e_marca_metodo_http() {
    std::env::set_var("DUCKDUCKGO_SEARCH_CLI_SKIP_SSRF", "1");
    let mock = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/a"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_raw(artigo_html("Rust").into_bytes(), "text/html; charset=utf-8"),
        )
        .mount(&mock)
        .await;

    Mock::given(method("GET"))
        .and(path("/b"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            artigo_html("Tokio").into_bytes(),
            "text/html; charset=utf-8",
        ))
        .mount(&mock)
        .await;

    let url_a = format!("{}/a", mock.uri());
    let url_b = format!("{}/b", mock.uri());
    let mut output = output_with_urls(&[&url_a, &url_b]);

    let client = Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .unwrap();
    let config = cfg(2);
    let cancellation = CancellationToken::new();

    enrich_with_content(&mut output, &client, &config, &cancellation).await;

    assert_eq!(output.metadata.concurrent_fetches, 2);
    assert_eq!(output.metadata.fetch_successes, 2);
    assert_eq!(output.metadata.fetch_failures, 0);
    for r in &output.results {
        let content = r.content.as_ref().expect("content present");
        assert!(content.len() > 100, "non-trivial content");
        assert_eq!(
            r.content_extraction_method.as_deref(),
            Some("http"),
            "method should be http (no chrome feature active or fallback triggered)"
        );
    }
    // Without the `chrome` feature active, this field remains false.
    assert!(!output.metadata.used_chrome);
}

// ---------------------------------------------------------------------------
// T2: endpoint returns non-HTML content-type → must register failure, not crash.
// ---------------------------------------------------------------------------
#[tokio::test]
async fn enriches_with_non_html_content_type_records_failure() {
    std::env::set_var("DUCKDUCKGO_SEARCH_CLI_SKIP_SSRF", "1");
    let mock = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/img"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(b"PNGDATA".to_vec(), "image/png"))
        .mount(&mock)
        .await;

    let url = format!("{}/img", mock.uri());
    let mut output = output_with_urls(&[&url]);

    let client = Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .unwrap();
    let config = cfg(1);
    let cancellation = CancellationToken::new();

    enrich_with_content(&mut output, &client, &config, &cancellation).await;

    assert_eq!(output.metadata.concurrent_fetches, 1);
    assert_eq!(output.metadata.fetch_successes, 0, "non-HTML = 0 successes");
    assert_eq!(
        output.metadata.fetch_failures, 1,
        "non-HTML counts as failure"
    );
    assert!(output.results[0].content.is_none());
    assert!(output.results[0].content_extraction_method.is_none());
}

// ---------------------------------------------------------------------------
// T3: two results on the SAME host — exercises per-host semaphore without serializing everything.
// ---------------------------------------------------------------------------
#[tokio::test]
async fn enriches_same_host_respecting_per_host_limit() {
    std::env::set_var("DUCKDUCKGO_SEARCH_CLI_SKIP_SSRF", "1");
    let mock = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/p1"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_raw(artigo_html("A").into_bytes(), "text/html; charset=utf-8"),
        )
        .mount(&mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/p2"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_raw(artigo_html("B").into_bytes(), "text/html; charset=utf-8"),
        )
        .mount(&mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/p3"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_raw(artigo_html("C").into_bytes(), "text/html; charset=utf-8"),
        )
        .mount(&mock)
        .await;

    let u1 = format!("{}/p1", mock.uri());
    let u2 = format!("{}/p2", mock.uri());
    let u3 = format!("{}/p3", mock.uri());
    let mut output = output_with_urls(&[&u1, &u2, &u3]);

    let client = Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .unwrap();
    let mut config = cfg(3);
    config.per_host_limit = 2;
    let cancellation = CancellationToken::new();

    enrich_with_content(&mut output, &client, &config, &cancellation).await;

    assert_eq!(output.metadata.fetch_successes, 3);
    assert_eq!(output.metadata.fetch_failures, 0);
}
