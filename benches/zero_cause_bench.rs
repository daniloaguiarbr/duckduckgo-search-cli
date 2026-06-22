// SPDX-License-Identifier: MIT OR Apache-2.0
//! v0.8.0 — Zero-cause classification benchmark.
//!
//! Measures the cost of `classify_zero_result` across the 5
//! `ZeroCause` variants. The classifier is a pure function (no I/O),
//! so reported numbers represent CPU cost only — useful for
//! regression detection if the classification chain grows in
//! future versions.
//!
//! Run with:
//!   `cargo bench --bench zero_cause_bench`
//!
//! Expected (baseline on Linux `x86_64`, single core, release):
//!   `classify_resposta_invalida_empty_body`: ~50ns
//!   `classify_ghost_block_short_no_marker`: ~500ns (marker scan)
//!   `classify_anti_bot_cloudflare_interstitial`: ~700ns (marker scan + variant detection)
//!   `classify_legitimo_with_result_page_signal`: ~400ns
//!   `classify_filtro_silencioso_short_no_signal`: ~500ns

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use duckduckgo_search_cli::pipeline::{classify_zero_result, ZeroClassificationInputs};
use duckduckgo_search_cli::types::ZeroCause;

fn bench_classify_resposta_invalida(c: &mut Criterion) {
    c.bench_function("classify_resposta_invalida_empty_body", |b| {
        let inputs = ZeroClassificationInputs {
            body: "",
            pre_flight_enabled: false,
            pre_flight_fired: false,
            execution_time_ms: 0,
            retries: 0,
            concurrent_fetches: 0,
            last_probe_cascade_level: None,
        };
        b.iter(|| {
            let cause = classify_zero_result(black_box(&inputs));
            black_box(cause);
        });
    });
}

fn bench_classify_ghost_block(c: &mut Criterion) {
    c.bench_function("classify_ghost_block_short_no_marker", |b| {
        // 2KB lorem-ipsum — exercises the size-heuristic branch
        // (body.len() < 4000 && !has_result_page_signal).
        let body: String = "Lorem ipsum dolor sit amet, consectetur adipiscing elit. \
            Sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. \
            Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris \
            nisi ut aliquip ex ea commodo consequat. Duis aute irure dolor in \
            reprehenderit in voluptate velit esse cillum dolore eu fugiat nulla \
            pariatur. Excepteur sint occaecat cupidatat non proident, sunt in \
            culpa qui officia deserunt mollit anim id est laborum. "
            .repeat(20);
        let inputs = ZeroClassificationInputs {
            body: &body,
            pre_flight_enabled: false,
            pre_flight_fired: false,
            execution_time_ms: 500,
            retries: 0,
            concurrent_fetches: 0,
            last_probe_cascade_level: None,
        };
        b.iter(|| {
            let cause = classify_zero_result(black_box(&inputs));
            black_box(cause);
        });
    });
}

fn bench_classify_anti_bot(c: &mut Criterion) {
    c.bench_function("classify_anti_bot_cloudflare_interstitial", |b| {
        // Load the real 14KB Cloudflare 2026 fixture — exercises the
        // interstitial detection branch (markers in body).
        let body = include_str!("../tests/fixtures/interstitial_cloudflare_anomaly_2026.html");
        let inputs = ZeroClassificationInputs {
            body,
            pre_flight_enabled: false,
            pre_flight_fired: false,
            execution_time_ms: 766,
            retries: 0,
            concurrent_fetches: 0,
            last_probe_cascade_level: None,
        };
        b.iter(|| {
            let cause = classify_zero_result(black_box(&inputs));
            black_box(cause);
        });
    });
}

fn bench_classify_legitimo(c: &mut Criterion) {
    c.bench_function("classify_legitimo_with_result_page_signal", |b| {
        // 800B body with `result__a` (real result page signal).
        let body = r#"<html><body>
            <div class="result"><a class="result__a" href="https://example.com">Title 1</a></div>
            <div class="result"><a class="result__a" href="https://example.com/2">Title 2</a></div>
            <div class="result"><a class="result__a" href="https://example.com/3">Title 3</a></div>
        </body></html>"#;
        let inputs = ZeroClassificationInputs {
            body,
            pre_flight_enabled: false,
            pre_flight_fired: false,
            execution_time_ms: 250,
            retries: 0,
            concurrent_fetches: 0,
            last_probe_cascade_level: None,
        };
        b.iter(|| {
            let cause = classify_zero_result(black_box(&inputs));
            black_box(cause);
        });
    });
}

criterion_group!(
    benches,
    bench_classify_resposta_invalida,
    bench_classify_ghost_block,
    bench_classify_anti_bot,
    bench_classify_legitimo,
);
criterion_main!(benches);

// Suppress unused import warning for ZeroCause (re-exported via classify_zero_result).
#[allow(dead_code)]
const _ZERO_CAUSE: ZeroCause = ZeroCause::Legitimo;
