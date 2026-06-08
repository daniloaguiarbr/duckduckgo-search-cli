// SPDX-License-Identifier: MIT OR Apache-2.0
//! Integration tests for the deep-research pipeline using `wiremock`.
//!
//! ZERO real HTTP calls. Each test boots a `MockServer` on a random port
//! and exercises the public surface of the deep-research modules.

use duckduckgo_search_cli::aggregation::{aggregate, AggregationStrategy};
use duckduckgo_search_cli::decomposition::{
    is_composite_query, CompositeSignal, HeuristicTemplate,
};
use duckduckgo_search_cli::deep_research::{
    AggregationStrategyKind, DeepResearchArgs, MAX_SUB_QUERIES,
};
use duckduckgo_search_cli::synthesis::{estimate_tokens, trim_to_budget, SynthFormat};
use duckduckgo_search_cli::types::{SearchMetadata, SearchOutput, SearchResult};
use std::time::Duration;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// Minimal HTML payload. The DDG HTML parser is lenient — it will produce
/// zero results from any non-empty HTML, which is what we want for these
/// unit-level integration tests.
const MOCK_HTML: &str = r#"<!DOCTYPE html>
<html>
<head><title>DuckDuckGo</title></head>
<body><h1>No results</h1></body>
</html>
"#;

fn make_result(position: u32, url: &str) -> SearchResult {
    SearchResult {
        position,
        title: format!("Mock title {position}"),
        url: url.to_string(),
        display_url: None,
        snippet: Some(format!("Mock snippet {position}")),
        original_title: None,
        content: None,
        content_size: None,
        content_extraction_method: None,
    }
}

fn make_metadata() -> SearchMetadata {
    SearchMetadata {
        execution_time_ms: 100,
        selectors_hash: "deadbeef".to_string(),
        retries: 0,
        used_fallback_endpoint: false,
        concurrent_fetches: 0,
        fetch_successes: 0,
        fetch_failures: 0,
        used_chrome: false,
        user_agent: "test-ua".to_string(),
        identity_used: None,
        cascade_level: None,
        used_proxy: false,
    }
}

fn make_output(query: &str, urls: &[&str]) -> SearchOutput {
    SearchOutput {
        query: query.to_string(),
        engine: "duckduckgo".to_string(),
        endpoint: "html".to_string(),
        timestamp: "2026-06-07T00:00:00Z".to_string(),
        region: "us-en".to_string(),
        result_count: urls.len() as u32,
        results: urls
            .iter()
            .enumerate()
            .map(|(i, u)| make_result(i as u32 + 1, u))
            .collect(),
        pages_fetched: 1,
        error: None,
        message: None,
        metadata: make_metadata(),
    }
}

// =====================================================================
// Public-surface tests (no network)
// =====================================================================

#[test]
fn deep_research_args_default_validates() {
    let args = DeepResearchArgs::default();
    assert!(args.validate().is_ok());
    assert_eq!(args.max_sub_queries, 5);
    assert_eq!(args.depth, 0);
    assert!(!args.synthesize);
}

#[test]
fn deep_research_args_rejects_zero_sub_queries() {
    let args = DeepResearchArgs {
        max_sub_queries: 0,
        ..Default::default()
    };
    assert!(args.validate().is_err());
}

#[test]
fn deep_research_args_rejects_excessive_sub_queries() {
    let args = DeepResearchArgs {
        max_sub_queries: MAX_SUB_QUERIES + 1,
        ..Default::default()
    };
    assert!(args.validate().is_err());
}

#[test]
fn aggregation_rrf_dedupes_repeated_urls() {
    let a = make_output("q1", &["https://example.com/a", "https://example.com/b"]);
    let b = make_output("q2", &["https://example.com/a", "https://example.com/c"]);
    let agg = aggregate(&[a, b], AggregationStrategy::Rrf(60));
    let urls: Vec<&str> = agg.iter().map(|i| i.url.as_str()).collect();
    assert_eq!(urls.len(), 3, "expected 3 unique URLs");
    // The URL that appears in both queries should rank first.
    assert_eq!(urls[0], "https://example.com/a");
}

#[test]
fn aggregation_dedupe_keeps_first_occurrence() {
    let a = make_output("q1", &["https://example.com/x"]);
    let b = make_output("q2", &["https://example.com/y", "https://example.com/x"]);
    let agg = aggregate(&[a, b], AggregationStrategy::DedupeByUrl);
    assert_eq!(agg.len(), 2, "x appears in both, must be deduped");
    let urls: std::collections::HashSet<&str> = agg.iter().map(|i| i.url.as_str()).collect();
    assert!(urls.contains("https://example.com/x"));
    assert!(urls.contains("https://example.com/y"));
}

#[test]
fn aggregation_handles_url_canonicalization() {
    let a = make_output("q1", &["https://Example.com/p?utm_source=x&id=1"]);
    let b = make_output("q2", &["https://example.com/p?id=1"]);
    let agg = aggregate(&[a, b], AggregationStrategy::DedupeByUrl);
    assert_eq!(agg.len(), 1, "canonical URLs must dedupe");
}

#[test]
fn synthesis_format_variants_exist() {
    let _m = SynthFormat::Markdown;
    let _p = SynthFormat::PlainText;
    let _j = SynthFormat::Json;
    assert!(!format!("{:?}", SynthFormat::Markdown).is_empty());
}

#[test]
fn estimate_tokens_handles_empty_and_unicode() {
    assert_eq!(estimate_tokens(""), 0);
    assert_eq!(estimate_tokens("abcd"), 1);
    // 16 bytes (4 emojis) -> ceil(16/4) = 4
    assert_eq!(estimate_tokens("🦀🦀🦀🦀"), 4);
}

#[test]
fn trim_to_budget_respects_utf8_boundaries() {
    let s = "🦀🦀🦀🦀 hello world";
    let out = trim_to_budget(s, 2);
    // Must not panic on char boundaries.
    assert!(out.len() <= 8 + 4);
}

#[test]
fn composite_query_detects_all_signals() {
    assert!(is_composite_query(
        "rust vs go",
        CompositeSignal::Comparison
    ));
    assert!(is_composite_query(
        "why is rust hard",
        CompositeSignal::Cause
    ));
    assert!(is_composite_query(
        "best rust web framework",
        CompositeSignal::Opinion
    ));
    assert!(is_composite_query(
        "history of rust",
        CompositeSignal::Timeline
    ));
    assert!(is_composite_query(
        "cargo and clippy",
        CompositeSignal::Aspect
    ));
}

#[test]
fn all_five_templates_have_unique_suffixes() {
    let mut seen = std::collections::HashSet::new();
    for t in HeuristicTemplate::all() {
        assert!(seen.insert(t.suffix()), "duplicate suffix: {}", t.suffix());
    }
}

#[test]
fn aggregation_strategy_kind_variants() {
    let _ = AggregationStrategyKind::Rrf;
    let _ = AggregationStrategyKind::DedupeByUrl;
}

#[test]
fn heuristic_template_comparison_suffix_format() {
    let suffix = HeuristicTemplate::Comparison.suffix();
    assert!(suffix.contains("vs alternatives comparison"));
}

// =====================================================================
// wiremock tests (network, but mocked)
// =====================================================================

#[tokio::test]
async fn wiremock_deep_research_pipeline_smoke() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/html/"))
        .respond_with(ResponseTemplate::new(200).set_body_string(MOCK_HTML))
        .mount(&server)
        .await;

    let client = wreq::Client::builder()
        .timeout(Duration::from_secs(5))
        .user_agent("Mozilla/5.0 (test)")
        .build()
        .unwrap();

    let urls: Vec<String> = (0..2)
        .map(|i| format!("{}/html/?q=test{}", server.uri(), i))
        .collect();
    let mut handles = Vec::new();
    for url in urls {
        let c = client.clone();
        handles.push(tokio::spawn(async move { c.get(&url).send().await }));
    }
    for h in handles {
        let resp = h.await.expect("join").expect("send");
        assert!(resp.status().is_success());
    }
}

#[tokio::test]
async fn wiremock_404_observable() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/missing"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;

    let client = wreq::Client::builder()
        .timeout(Duration::from_secs(2))
        .build()
        .unwrap();
    let resp = client
        .get(format!("{}/missing", server.uri()))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status().as_u16(), 404);
}

#[tokio::test]
async fn wiremock_202_anomaly_path() {
    // DDG returns HTTP 202 (Accepted) when rate-limiting. The CLI must
    // detect this and back off; at the wiremock level we just confirm
    // the response is observable.
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/blocked"))
        .respond_with(ResponseTemplate::new(202).set_body_string(""))
        .mount(&server)
        .await;

    let client = wreq::Client::builder()
        .timeout(Duration::from_secs(2))
        .build()
        .unwrap();
    let resp = client
        .get(format!("{}/blocked", server.uri()))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status().as_u16(), 202);
}

#[tokio::test]
async fn wiremock_query_param_matching() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/html/"))
        .and(query_param("q", "rust"))
        .respond_with(ResponseTemplate::new(200).set_body_string(MOCK_HTML))
        .mount(&server)
        .await;

    let client = wreq::Client::builder()
        .timeout(Duration::from_secs(2))
        .build()
        .unwrap();
    let resp = client
        .get(format!("{}/html/?q=rust", server.uri()))
        .send()
        .await
        .unwrap();
    assert!(resp.status().is_success());
}
