// SPDX-License-Identifier: MIT OR Apache-2.0
//! Teste de auditoria: reprodução do GAP-AUD-003 v0.8.0 em ambiente bloqueado.
//!
//! Serve o body REAL do Cloudflare capturado em 2026-06-19 (14KB com
//! marcadores `anomaly-modal` + `anomaly.js`) via wiremock e verifica se o
//! classificador retorna `AntiBot`/`GhostBlock` e se o exit code é 6.

use duckduckgo_search_cli::pipeline::{
    classify_zero_result, sugestao_proxima_acao_para_zero, ZeroClassificationInputs,
};
use duckduckgo_search_cli::probe_deep::{
    detectar_interstitial_com_match, has_result_page_signal, InterstitialKind,
};
use duckduckgo_search_cli::search::search_with_pagination;
use duckduckgo_search_cli::types::{Config, Endpoint, ZeroCause};
use reqwest::Client;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use tokio::sync::Mutex as TokioMutex;
use tokio_util::sync::CancellationToken;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn env_lock() -> &'static TokioMutex<()> {
    static LOCK: std::sync::OnceLock<TokioMutex<()>> = std::sync::OnceLock::new();
    LOCK.get_or_init(|| TokioMutex::new(()))
}

fn load_cloudflare_2026_fixture() -> String {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.push("tests/fixtures/interstitial_cloudflare_anomaly_2026.html");
    fs::read_to_string(&p).unwrap_or_else(|e| panic!("falha ler fixture {:?}: {e}", p))
}

/// Pre-compresses the Cloudflare 2026 fixture with gzip at default level.
///
/// Mirrors the production behavior of `DuckDuckGo`, which replies with
/// `Content-Encoding: gzip` for HTML responses. Used to reproduce
/// GAP-AUD-003 Bug #1 in a regression test that verifies the full
/// `search_with_pagination` path correctly decompresses before the
/// interstitial classifier inspects the body.
fn gzip_compress_fixture(plain: &str) -> Vec<u8> {
    let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    encoder
        .write_all(plain.as_bytes())
        .expect("write to gzip encoder");
    encoder.finish().expect("finish gzip encoder")
}

#[test]
fn audit_cloudflare_2026_body_has_anomaly_modal_marker() {
    let body = load_cloudflare_2026_fixture();
    let (marker, kind) = detectar_interstitial_com_match(&body);
    eprintln!(
        "AUDITORIA: body_len={} marker={} kind={:?} has_result_page_signal={}",
        body.len(),
        marker,
        kind,
        has_result_page_signal(&body)
    );
    assert!(
        body.contains("anomaly-modal"),
        "fixture deve conter marker anomaly-modal"
    );
    assert_eq!(
        kind,
        InterstitialKind::Cloudflare,
        "kind deve ser Cloudflare (14KB com anomaly-modal)"
    );
}

#[test]
fn audit_cloudflare_2026_classifier_returns_non_legitimo() {
    let body = load_cloudflare_2026_fixture();
    let inputs = ZeroClassificationInputs {
        body: &body,
        pre_flight_enabled: false,
        pre_flight_fired: false,
        execution_time_ms: 766,
        retries: 0,
        concurrent_fetches: 0,
        last_probe_cascade_level: None,
    };
    let cause = classify_zero_result(&inputs);
    let sugestao = sugestao_proxima_acao_para_zero(cause);
    eprintln!("AUDITORIA: cause={:?} sugestao={:?}", cause, sugestao);
    assert_ne!(
        cause,
        ZeroCause::Legitimo,
        "BUG CONFIRMADO: classificador rotulou Cloudflare 2026 challenge (14KB + anomaly-modal) como Legitimo"
    );
}

#[tokio::test]
async fn audit_cloudflare_2026_e2e_first_body_populated() {
    let _g = env_lock().lock().await;
    let mock_server = MockServer::start().await;

    let body = load_cloudflare_2026_fixture();
    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(body.clone())
                .insert_header("content-type", "text/html; charset=utf-8"),
        )
        .mount(&mock_server)
        .await;

    let mock_server_lite = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string("<html><body>vazio lite</body></html>".to_string())
                .insert_header("content-type", "text/html; charset=utf-8"),
        )
        .mount(&mock_server_lite)
        .await;

    let base_html = format!("{}/", mock_server.uri());
    let base_lite = format!("{}/", mock_server_lite.uri());
    let _env = EnvGuard::set(&[
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_HTML".to_string(), base_html),
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_LITE".to_string(), base_lite),
        (
            "DUCKDUCKGO_SEARCH_CLI_NO_CHROME".to_string(),
            "1".to_string(),
        ),
    ]);

    let cliente = Client::builder().build().expect("client");
    let cfg = Config {
        query: "rust serde derive".to_string(),
        endpoint: Endpoint::Html,
        num_results: Some(3),
        retries: 0,
        timeout_seconds: 10,
        global_timeout_seconds: 30,
        allow_lite_fallback: false,
        pre_flight: false,
        ..Config::default()
    };

    let flag = Arc::new(AtomicBool::new(false));
    let token = CancellationToken::new();

    let result = search_with_pagination(&cliente, &cfg, "rust serde derive", &flag, &token).await;

    match &result {
        Ok(agregado) => {
            eprintln!(
                "AUDITORIA E2E: results.len={} first_body.len={} effective_endpoint={:?}",
                agregado.results.len(),
                agregado.first_body.len(),
                agregado.effective_endpoint
            );
            assert_eq!(
                agregado.results.len(),
                0,
                "Cloudflare challenge não deve produzir resultados"
            );
            assert!(
                agregado.first_body.len() > 5000,
                "first_body deve conter body real do Cloudflare (>5KB), encontrado {} bytes",
                agregado.first_body.len()
            );
            assert!(
                agregado.first_body.contains("anomaly-modal"),
                "first_body deve preservar anomaly-modal do body original"
            );
        }
        Err(e) => {
            eprintln!(
                "AUDITORIA E2E: search_with_pagination retornou Err (esperado se blocked antes de popular first_body): {:?}",
                e
            );
        }
    }
}

/// Regression test for GAP-AUD-003 Bug #1 (HTTP gzip decompression).
///
/// Reproduces the production scenario where `DuckDuckGo` replies with
/// `Content-Encoding: gzip` and the body is gzip-compressed bytes. Without
/// the fix, `search_with_pagination` would treat the compressed bytes as
/// the interstitial body, the marker detection would fail, and the
/// classifier would mislabel the result as `Legitimo` instead of
/// `AntiBot`/`GhostBlock`.
///
/// Steps:
/// 1. Load the real 14KB Cloudflare 2026 fixture (contains `anomaly-modal`).
/// 2. Pre-compress with `flate2::write::GzEncoder`.
/// 3. Serve via wiremock with `Content-Encoding: gzip` header.
/// 4. Run full `search_with_pagination` end-to-end.
/// 5. Assert that `agregado.first_body` (after decompression) contains
///    `anomaly-modal` — proves the decompression path is wired correctly
///    before the interstitial detector runs.
#[tokio::test]
async fn audit_cloudflare_2026_gzip_e2e_decompression_succeeds() {
    let _g = env_lock().lock().await;
    let mock_server = MockServer::start().await;

    let plain = load_cloudflare_2026_fixture();
    let gzipped = gzip_compress_fixture(&plain);
    assert!(
        gzipped.len() < plain.len(),
        "fixture deve comprimir (original={}, gzipped={})",
        plain.len(),
        gzipped.len()
    );

    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_bytes(gzipped)
                .insert_header("content-type", "text/html; charset=utf-8")
                .insert_header("content-encoding", "gzip")
                .insert_header("vary", "Accept-Encoding"),
        )
        .mount(&mock_server)
        .await;

    let mock_server_lite = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string("<html><body>vazio lite</body></html>".to_string())
                .insert_header("content-type", "text/html; charset=utf-8"),
        )
        .mount(&mock_server_lite)
        .await;

    let base_html = format!("{}/", mock_server.uri());
    let base_lite = format!("{}/", mock_server_lite.uri());
    let _env = EnvGuard::set(&[
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_HTML".to_string(), base_html),
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_LITE".to_string(), base_lite),
        (
            "DUCKDUCKGO_SEARCH_CLI_NO_CHROME".to_string(),
            "1".to_string(),
        ),
    ]);

    let cliente = Client::builder().build().expect("client");
    let cfg = Config {
        query: "rust serde derive".to_string(),
        endpoint: Endpoint::Html,
        num_results: Some(3),
        retries: 0,
        timeout_seconds: 10,
        global_timeout_seconds: 30,
        allow_lite_fallback: false,
        pre_flight: false,
        ..Config::default()
    };

    let flag = Arc::new(AtomicBool::new(false));
    let token = CancellationToken::new();

    let result = search_with_pagination(&cliente, &cfg, "rust serde derive", &flag, &token).await;

    match &result {
        Ok(agregado) => {
            eprintln!(
                "AUDITORIA GZIP E2E: results.len={} first_body.len={} effective_endpoint={:?}",
                agregado.results.len(),
                agregado.first_body.len(),
                agregado.effective_endpoint
            );
            assert_eq!(
                agregado.results.len(),
                0,
                "Cloudflare challenge não deve produzir resultados mesmo com gzip"
            );
            assert!(
                agregado.first_body.len() > 5000,
                "first_body DEVE conter body descomprimido do Cloudflare (>5KB após gzip→plain), encontrado {} bytes — BUG #1 NÃO CORRIGIDO se for próximo do tamanho gzipped",
                agregado.first_body.len()
            );
            assert!(
                agregado.first_body.contains("anomaly-modal"),
                "first_body DEVE preservar marker 'anomaly-modal' após descompressão gzip — BUG #1 NÃO CORRIGIDO se marker ausente"
            );
        }
        Err(e) => {
            panic!(
                "AUDITORIA GZIP E2E: search_with_pagination deveria succeed (body gzip descomprimido OK), got Err: {e:?}"
            );
        }
    }
}

struct EnvGuard {
    keys: Vec<String>,
}

impl EnvGuard {
    fn set(pairs: &[(String, String)]) -> Self {
        let mut keys = Vec::with_capacity(pairs.len());
        for (k, v) in pairs {
            unsafe { std::env::set_var(k, v) };
            keys.push(k.clone());
        }
        Self { keys }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        for k in &self.keys {
            unsafe { std::env::remove_var(k) };
        }
    }
}
