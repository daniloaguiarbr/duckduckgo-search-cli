// SPDX-License-Identifier: MIT OR Apache-2.0
//! Testes de integração para `pipeline::execute_pipeline` e `parallel::*`.
//!
//! Cobre os caminhos de maior custo do fluxo multi-query:
//! - Barrier (`JoinSet`) quando `modo_stream = false`.
//! - Streaming (mpsc) quando `modo_stream = true`.
//! - Single-query com `modo_stream = true` (warn + fallback).
//! - Erros de lista vazia.
//! - Helpers puros de dedup e leitura de arquivo.
//!
//! Todos os testes usam `wiremock` — ZERO chamadas HTTP reais.

use duckduckgo_search_cli::pipeline::{
    combine_and_dedup_queries, execute_pipeline, read_queries_from_file, PipelineResult,
};
use duckduckgo_search_cli::types::{Config, Endpoint, OutputFormat, SafeSearch};
use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::Duration;
use tokio::sync::Mutex as TokioMutex;
use tokio_util::sync::CancellationToken;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// Async mutex to serialize tests that manipulate env vars (`std::env` is not thread-safe).
fn env_lock() -> &'static TokioMutex<()> {
    static LOCK: OnceLock<TokioMutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| TokioMutex::new(()))
}

/// RAII guard for env vars — cleans up on drop.
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

fn cfg_multi(queries: Vec<String>, format: OutputFormat, stream: bool) -> Config {
    Config {
        query: queries.first().cloned().unwrap_or_default(),
        queries,
        num_results: None,
        format,
        timeout_seconds: 5,
        language: "pt".to_string(),
        country: "br".to_string(),
        verbose: false,
        quiet: true,
        user_agent: "Mozilla/5.0 (teste)".to_string(),
        browser_profile: duckduckgo_search_cli::http::create_browser_profile("Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/146.0.0.0 Safari/537.36"),
        parallelism: 2,
        pages: 1,
        retries: 0,
        endpoint: Endpoint::Html,
        time_filter: None,
        safe_search: SafeSearch::Moderate,
        stream_mode: stream,
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
        selectors: std::sync::Arc::new(
            duckduckgo_search_cli::types::SelectorConfig::default(),
        ),
    }
}

/// HTML com 2 resultados — corpo acima de 5 000 bytes (limiar anti-bloqueio silencioso).
fn html_2_resultados(titulo_a: &str, titulo_b: &str) -> String {
    // Padding ensures the body stays above LIMIAR_BLOQUEIO_SILENCIOSO (5,000 bytes).
    let padding =
        "<!-- padding para superar o limiar de detecção de bloqueio silencioso do DuckDuckGo. -->"
            .repeat(60);
    format!(
        r#"<html><body>
        {padding}
        <div id="links">
          <div class="result">
            <a class="result__a" href="//exemplo.com/a">{titulo_a}</a>
            <a class="result__snippet">snippet A</a>
            <span class="result__url">exemplo.com/a</span>
          </div>
          <div class="result">
            <a class="result__a" href="//exemplo.com/b">{titulo_b}</a>
            <a class="result__snippet">snippet B</a>
            <span class="result__url">exemplo.com/b</span>
          </div>
        </div></body></html>"#
    )
}

// ---------------------------------------------------------------------------
// T1: multi-query em modo barrier — exercita `execute_parallel_searches`
//     e JoinSet com staggered launch.
// ---------------------------------------------------------------------------
#[tokio::test]
async fn pipeline_multi_query_barrier_agrega_resultados() {
    let _g = env_lock().lock().await;
    let mock = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(html_2_resultados("Primeiro", "Segundo"))
                .insert_header("content-type", "text/html; charset=utf-8"),
        )
        .mount(&mock)
        .await;

    let base = format!("{}/", mock.uri());
    let _env = EnvGuard::set(&[
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_HTML", base.clone()),
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_LITE", base),
    ]);

    let cfg = cfg_multi(
        vec!["rust".to_string(), "tokio".to_string()],
        OutputFormat::Json,
        false,
    );
    let token = CancellationToken::new();

    let res = execute_pipeline(cfg, token)
        .await
        .expect("pipeline multi-query barrier deve ter sucesso");

    match res {
        PipelineResult::Multi(multi) => {
            assert_eq!(multi.query_count, 2, "2 queries executadas");
            assert_eq!(multi.searches.len(), 2);
            assert!(multi.searches.iter().all(|s| s.result_count >= 2));
        }
        other => panic!("expected Multi, got: {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// T2: multi-query in streaming mode — exercises `execute_parallel_searches_streaming`
//     + consumer via mpsc + NDJSON emission.
// ---------------------------------------------------------------------------
#[tokio::test]
async fn pipeline_multi_query_streaming_drains_and_returns_stats() {
    let _g = env_lock().lock().await;
    let mock = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(html_2_resultados("Alpha", "Beta"))
                .insert_header("content-type", "text/html; charset=utf-8"),
        )
        .mount(&mock)
        .await;

    let base = format!("{}/", mock.uri());
    let _env = EnvGuard::set(&[
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_HTML", base.clone()),
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_LITE", base),
    ]);

    // Output file to avoid polluting stdout during the test.
    let tmp = tempfile::NamedTempFile::new().expect("tempfile");
    let mut cfg = cfg_multi(
        vec!["a".to_string(), "b".to_string(), "c".to_string()],
        OutputFormat::Json,
        true,
    );
    cfg.output_file = Some(tmp.path().to_path_buf());

    let token = CancellationToken::new();

    let res = tokio::time::timeout(Duration::from_secs(30), execute_pipeline(cfg, token))
        .await
        .expect("pipeline não deve pendurar")
        .expect("pipeline streaming deve ter sucesso");

    match res {
        PipelineResult::Stream(stats) => {
            assert_eq!(stats.total, 3, "3 queries processadas no stream");
            assert!(stats.successes + stats.errors == stats.total);
        }
        other => panic!("expected Stream, got: {other:?}"),
    }

    // Validate that NDJSON was written: 3 valid JSON lines.
    let content = std::fs::read_to_string(tmp.path()).expect("read output file");
    let lines: Vec<&str> = content.lines().filter(|l| !l.trim().is_empty()).collect();
    assert_eq!(lines.len(), 3, "3 NDJSON lines (one per query)");
    for line in &lines {
        let _: serde_json::Value = serde_json::from_str(line).expect("valid NDJSON line");
    }
}

// ---------------------------------------------------------------------------
// T3: single-query com modo_stream=true — branch que emite warn + fallback agregado.
// ---------------------------------------------------------------------------
#[tokio::test]
async fn pipeline_single_query_with_stream_warns_and_falls_back_to_aggregate() {
    let _g = env_lock().lock().await;
    let mock = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(html_2_resultados("Único", "Segundo"))
                .insert_header("content-type", "text/html; charset=utf-8"),
        )
        .mount(&mock)
        .await;

    let base = format!("{}/", mock.uri());
    let _env = EnvGuard::set(&[
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_HTML", base.clone()),
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_LITE", base),
    ]);

    let cfg = cfg_multi(vec!["solo".to_string()], OutputFormat::Json, true);
    let token = CancellationToken::new();

    let res = execute_pipeline(cfg, token)
        .await
        .expect("single + stream deve cair em Unica com warn");

    match res {
        PipelineResult::Single(output) => {
            assert_eq!(output.query, "solo");
            assert!(output.result_count >= 2);
        }
        other => panic!("expected Single, got: {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// T4: empty queries — must return error, not panic.
// ---------------------------------------------------------------------------
#[tokio::test]
async fn pipeline_with_empty_queries_returns_error() {
    let cfg = cfg_multi(vec![], OutputFormat::Json, false);
    let token = CancellationToken::new();
    let res = execute_pipeline(cfg, token).await;
    assert!(res.is_err(), "lista vazia deve produzir erro");
    let msg = format!("{}", res.unwrap_err());
    assert!(
        msg.contains("no queries to execute"),
        "message should mention 'no queries to execute', got: {msg}"
    );
}

// ---------------------------------------------------------------------------
// T5: combine_and_dedup_queries — dedup preservando ordem e filtrando vazios.
// ---------------------------------------------------------------------------
#[test]
fn combinar_queries_preserva_ordem_dedup_e_filtra_vazios() {
    let r = combine_and_dedup_queries(
        vec!["rust".into(), "  ".into(), "tokio".into()],
        vec!["rust".into(), "serde".into()],
        vec!["".into(), "serde".into(), "axum".into()],
    );
    assert_eq!(r, vec!["rust", "tokio", "serde", "axum"]);
}

#[test]
fn combine_queries_fully_empty_list_returns_empty_vec() {
    let r = combine_and_dedup_queries(vec![], vec![], vec!["   ".into(), "\n".into()]);
    assert!(r.is_empty());
}

#[test]
fn combine_queries_trims_each_entry() {
    let r = combine_and_dedup_queries(
        vec!["  rust  ".into()],
        vec!["\ttokio\n".into()],
        vec![" rust ".into()],
    );
    // "  rust  " and " rust " after trim are equal → dedup.
    assert_eq!(r, vec!["rust", "tokio"]);
}

// ---------------------------------------------------------------------------
// T6: read_queries_from_file — LF, CRLF e linhas em branco.
// ---------------------------------------------------------------------------
#[test]
fn read_queries_from_file_handles_crlf_and_empty_lines() {
    let tmp = tempfile::NamedTempFile::new().expect("tempfile");
    // Mistura LF e CRLF + linhas vazias.
    std::fs::write(tmp.path(), "rust\r\n\r\n  tokio  \nserde\n\n").expect("escrever");
    let qs = read_queries_from_file(tmp.path()).expect("ler ok");
    assert_eq!(qs, vec!["rust", "tokio", "serde"]);
}

#[test]
fn read_queries_from_nonexistent_file_returns_error() {
    let inexistente = PathBuf::from("/tmp/duckduckgo-search-cli-file-nao-existe-xyz-123.txt");
    let r = read_queries_from_file(&inexistente);
    assert!(r.is_err(), "arquivo inexistente deve falhar");
}

#[test]
fn read_queries_from_empty_file_returns_empty_vec() {
    let tmp = tempfile::NamedTempFile::new().expect("tempfile");
    std::fs::write(tmp.path(), "").expect("escrever");
    let qs = read_queries_from_file(tmp.path()).expect("ok");
    assert!(qs.is_empty());
}
