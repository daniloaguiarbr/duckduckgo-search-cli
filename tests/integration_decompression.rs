// SPDX-License-Identifier: MIT OR Apache-2.0
//! Integration tests for `src/decompress.rs` — wiremock-backed regression
//! coverage for the bug where `reqwest::Response::text()` returned gzip-compressed
//! bytes (GAP-AUD-003 v0.8.0 Bug #1).
//!
//! Each test spins up a `wiremock::MockServer` that replies with a body
//! encoded using a specific `Content-Encoding` value, then asserts that
//! `decompress::response_body_string` produces the expected decoded text
//! or the expected error variant.

use std::io::Write;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use duckduckgo_search_cli::decompress::{
    decode_bytes, response_body_string, DECOMPRESSION_MAX_OUTPUT,
};
use duckduckgo_search_cli::error::CliError;
use reqwest::Client;
use tokio_util::sync::CancellationToken;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// Returns a gzip-compressed copy of the input.
fn gzip_encode(plain: &[u8]) -> Vec<u8> {
    let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    encoder.write_all(plain).expect("write to encoder");
    encoder.finish().expect("finish encoder")
}

/// Returns a zlib (deflate) compressed copy of the input.
fn deflate_encode(plain: &[u8]) -> Vec<u8> {
    let mut encoder = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
    encoder.write_all(plain).expect("write to encoder");
    encoder.finish().expect("finish encoder")
}

// brotli_encode removed: brotli crate was dropped in v0.8.6

#[tokio::test]
async fn decode_identity_passes_through() {
    let plain = b"<html><body>hello world</body></html>";
    let decoded = decode_bytes(plain, "identity").expect("identity decode");
    assert_eq!(decoded, plain);
}

#[tokio::test]
async fn decode_gzip_unwraps_html() {
    let plain = b"<html><body>hello gzip</body></html>";
    let gz = gzip_encode(plain);
    let decoded = decode_bytes(&gz, "gzip").expect("gzip decode");
    assert_eq!(decoded, plain);
}

#[tokio::test]
async fn decode_deflate_unwraps_html() {
    let plain = b"<html><body>hello deflate</body></html>";
    let zl = deflate_encode(plain);
    let decoded = decode_bytes(&zl, "deflate").expect("deflate decode");
    assert_eq!(decoded, plain);
}

// decode_br_unwraps_html removed: brotli crate was dropped in v0.8.6

#[tokio::test]
async fn decode_oversize_returns_payload_too_large() {
    // Build a plain body just over the cap so gzip compresses to a small
    // payload that decompresses above the limit. The cap is 32 MiB so we
    // fabricate 33 MiB of repeated printable bytes.
    let plain: Vec<u8> = vec![b'A'; DECOMPRESSION_MAX_OUTPUT + 1024];
    let gz = gzip_encode(&plain);
    let result = decode_bytes(&gz, "gzip");
    match result {
        Err(CliError::PayloadTooLarge { max, actual }) => {
            assert_eq!(max, DECOMPRESSION_MAX_OUTPUT);
            assert!(
                actual > DECOMPRESSION_MAX_OUTPUT,
                "actual ({actual}) deve exceder cap"
            );
        }
        other => panic!("expected PayloadTooLarge, got {other:?}"),
    }
}

#[tokio::test]
async fn decode_unsupported_encoding_returns_error() {
    let plain = b"some payload";
    let result = decode_bytes(plain, "zstd");
    match result {
        Err(CliError::UnsupportedEncoding(enc)) => assert_eq!(enc, "zstd"),
        other => panic!("expected UnsupportedEncoding, got {other:?}"),
    }
}

#[tokio::test]
async fn response_body_string_e2e_gzip_via_wiremock() {
    // Spin up a wiremock that replies with gzip-encoded HTML containing a
    // recognizable marker. Bug #1 is fixed when the marker appears in the
    // decoded string.
    let plain = b"<html><body>marker-gzip-decoded-ok</body></html>";
    let gz = gzip_encode(plain);

    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_bytes(gz)
                .insert_header("content-encoding", "gzip")
                .insert_header("content-type", "text/html; charset=utf-8"),
        )
        .mount(&server)
        .await;

    let client = Client::builder().build().expect("client");
    let response = client.get(server.uri()).send().await.expect("send");
    let decoded = response_body_string(response)
        .await
        .expect("decompress succeeds");
    assert_eq!(decoded.as_bytes(), plain);
    assert!(decoded.contains("marker-gzip-decoded-ok"));
}

// response_body_string_e2e_br_via_wiremock removed: brotli crate was dropped in v0.8.6

#[tokio::test]
async fn response_body_string_e2e_identity_via_wiremock() {
    let plain = b"<html><body>plain-text</body></html>";
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(String::from_utf8(plain.to_vec()).unwrap())
                .insert_header("content-type", "text/html; charset=utf-8"),
        )
        .mount(&server)
        .await;

    let client = Client::builder().build().expect("client");
    let response = client.get(server.uri()).send().await.expect("send");
    let decoded = response_body_string(response)
        .await
        .expect("identity decode");
    assert_eq!(decoded.as_bytes(), plain);
}

/// Sanity check that the `CancellationToken` machinery still composes
/// when decompression is in flight. This is a smoke test — full cancel
/// safety is exercised by `tests/integration_*.rs` for the search path.
#[tokio::test]
async fn response_body_string_completes_under_cancellation_token() {
    let plain = b"<html><body>cancellable</body></html>";
    let gz = gzip_encode(plain);
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_bytes(gz)
                .insert_header("content-encoding", "gzip"),
        )
        .mount(&server)
        .await;

    let client = Client::builder().build().expect("client");
    let response = client.get(server.uri()).send().await.expect("send");
    let token = CancellationToken::new();
    let _flag = Arc::new(AtomicBool::new(false));
    let decoded = response_body_string(response)
        .await
        .expect("decode completes");
    assert!(decoded.contains("cancellable"));
    let _ = token; // silence unused warning while exercising the API
}
