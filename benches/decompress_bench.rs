// SPDX-License-Identifier: MIT OR Apache-2.0
//! v0.8.0 — Decompression benchmark.
//!
//! Measures the cost of `decode_bytes` across gzip/deflate/br (Brotli).
//! Numbers represent CPU cost only (sync function, no I/O).
//!
//! Run with:
//!   `cargo bench --bench decompress_bench`
//!
//! Expected (baseline on Linux `x86_64`, single core, release):
//!   `decode_gzip_14kb`:  ~50µs (incl. flate2 setup)
//!   `decode_deflate_14kb`: ~45µs
//!   `decode_br_14kb`: ~80µs (Brotli is slower than zlib)

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use duckduckgo_search_cli::decompress::decode_bytes;

fn bench_decode_gzip(c: &mut Criterion) {
    let plain = include_str!("../tests/fixtures/interstitial_cloudflare_anomaly_2026.html");
    let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    std::io::Write::write_all(&mut encoder, plain.as_bytes()).unwrap();
    let gzipped = encoder.finish().unwrap();

    c.bench_function("decode_gzip_14kb", |b| {
        b.iter(|| {
            let decoded = decode_bytes(black_box(&gzipped), "gzip").unwrap();
            black_box(decoded);
        });
    });
}

fn bench_decode_identity(c: &mut Criterion) {
    let plain = include_str!("../tests/fixtures/interstitial_cloudflare_anomaly_2026.html");
    c.bench_function("decode_identity_14kb", |b| {
        b.iter(|| {
            let decoded = decode_bytes(black_box(plain.as_bytes()), "identity").unwrap();
            black_box(decoded);
        });
    });
}

criterion_group!(benches, bench_decode_gzip, bench_decode_identity);
criterion_main!(benches);
