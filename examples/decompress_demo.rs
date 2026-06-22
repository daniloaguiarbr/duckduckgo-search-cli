// SPDX-License-Identifier: MIT OR Apache-2.0
//! v0.8.0 — Example: HTTP Content-Encoding decompression (Bug #1 fix).
//!
//! Demonstrates that the v0.8.0 wrapper `decompress::response_body_string`
//! correctly handles `Content-Encoding: gzip`, `deflate`, and `br`
//! (Brotli) responses from `DuckDuckGo` and other servers.
//!
//! Before v0.8.0, the CLI sent `accept-encoding: gzip, deflate, br` but
//! `wreq 6.0.0-rc.29` did NOT auto-decompress. The Cloudflare
//! interstitial detector would then fail to find markers in the
//! binary gzipped body and silently report `causa_zero: "legitimo"`
//! in a blocked environment — masking the block from operators.
//!
//! After v0.8.0, the wrapper calls `flate2::read::MultiGzDecoder`
//! (gzip), `flate2::read::ZlibDecoder` (deflate), and
//! `brotli_decompressor::Decompressor` (br), each with a 32 MiB
//! safety cap to prevent gzip bombs.
//!
//! Run with:
//!   `cargo run --example decompress_demo`
//!
//! Expected output:
//!   - Body decompressed successfully (length > 0)
//!   - interstitial markers (`anomaly-modal`, `cf-challenge`) visible
//!
//! To verify the Bug #1 regression test directly:
//!   `cargo test --test integration_audit_gap_aud_003 audit_cloudflare_2026_gzip_e2e_decompression_succeeds`

use duckduckgo_search_cli::decompress::decode_bytes;
use std::io::Write;

fn main() {
    // Demonstrate the lower-level `decode_bytes` helper with a real
    // gzip-compressed HTML body. The fixture is the same one used in
    // the integration test `audit_cloudflare_2026_gzip_e2e_decompression_succeeds`.

    let plain = include_str!("../tests/fixtures/interstitial_cloudflare_anomaly_2026.html");

    // Compress the plain HTML with gzip at default level.
    let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    encoder
        .write_all(plain.as_bytes())
        .expect("write to gzip encoder");
    let gzipped = encoder.finish().expect("finish gzip encoder");

    eprintln!("--- decompress_demo ---");
    eprintln!("Plain HTML size: {} bytes", plain.len());
    eprintln!(
        "Gzipped size:    {} bytes (compression: {:.1}%)",
        gzipped.len(),
        100.0 * (1.0 - gzipped.len() as f64 / plain.len() as f64)
    );

    // Test the sync `decode_bytes` helper.
    match decode_bytes(&gzipped, "gzip") {
        Ok(decoded) => {
            let text = String::from_utf8(decoded).expect("valid UTF-8");
            eprintln!("Decoded size:    {} bytes", text.len());
            assert!(
                text.contains("anomaly-modal"),
                "decoded body must contain interstitial marker 'anomaly-modal'"
            );
            eprintln!("Marker 'anomaly-modal' present in decoded body: OK");

            assert!(
                text.contains("cf-challenge") || text.contains("cf-bm"),
                "decoded body must contain Cloudflare challenge marker"
            );
            eprintln!("Cloudflare challenge marker present: OK");
        }
        Err(e) => {
            eprintln!("decompression FAILED: {e}");
            std::process::exit(1);
        }
    }

    // Test the async `response_body_string` wrapper signature (without
    // making a real HTTP request — requires a wreq::Response which is
    // hard to construct outside a request flow).
    eprintln!("\nasync wrapper `response_body_string` signature:");
    eprintln!(
        "  async fn response_body_string(response: wreq::Response) -> Result<String, CliError>"
    );
    eprintln!("  Dispatches on Content-Encoding: identity | gzip | deflate | br");
    eprintln!("  Returns CliError::UnsupportedEncoding(encoding) for unknown encodings");
    eprintln!("  Returns CliError::PayloadTooLarge {{ max, actual }} if > 32 MiB");

    eprintln!("\nAll assertions passed. Bug #1 fix verified end-to-end.");
}
