// GAP-NEW-002 v0.8.0 — criterion bench para tracing::info! overhead em hot path.
//
// Compara 3 cenários de instrumentação para detectar regressão de performance:
// - baseline (sem log)
// - tracing::info! (sempre emitido, sobrevive release com release_max_level_info)
// - tracing::debug! (removido em release por release_max_level_info; aqui é stub para referência)
//
// Invocação: cargo bench --bench tracing_overhead_bench

use criterion::{black_box, criterion_group, criterion_main, Criterion};

/// Simula hot path do classificador com 3 estratégias de logging.
fn bench_tracing_overhead(c: &mut Criterion) {
    // Body realista para forçar trabalho não-trivial: 5000 chars + assinatura DDG.
    let body_with_marker = "<html><body><form id=\"search_form\">".to_string()
        + &"x".repeat(5000)
        + "</form></body></html>";

    c.bench_function("classify_no_log_baseline", |b| {
        b.iter(|| {
            let body = black_box(&body_with_marker);
            let len = body.len();
            let has_marker = body.contains("search_form");
            black_box(len);
            black_box(has_marker);
        });
    });

    c.bench_function("classify_with_tracing_info", |b| {
        b.iter(|| {
            let body = black_box(&body_with_marker);
            tracing::info!(body_len = body.len(), "classify invoked");
            let has_marker = body.contains("search_form");
            black_box(has_marker);
        });
    });

    c.bench_function("classify_with_tracing_debug_disabled_in_release", |b| {
        // tracing::debug! é estaticamente removido quando feature release_max_level_info
        // está ativa no build release. Aqui incluímos para medir custo do macro expandido
        // (que em release vira no-op).
        b.iter(|| {
            let body = black_box(&body_with_marker);
            tracing::debug!(
                body_len = body.len(),
                "classify invoked (stripped in release)"
            );
            let has_marker = body.contains("search_form");
            black_box(has_marker);
        });
    });
}

criterion_group!(benches, bench_tracing_overhead);
criterion_main!(benches);
