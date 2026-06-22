// SPDX-License-Identifier: MIT OR Apache-2.0
//! Transparent decompression of HTTP response bodies.
//!
//! The HTTP client enables the `gzip` and `deflate` features but does NOT
//! always decompress the body returned by `Response::text()` or `Response::bytes()`.
//! The DDG upstream always replies with `Content-Encoding: gzip` for HTML
//! responses, so the body reaching the caller is a stream of gzip-compressed
//! bytes (~9 KB instead of the ~14 KB plain text body). Downstream consumers
//! like `detectar_interstitial_com_match` perform literal substring searches
//! (e.g. `body.contains("anomaly-modal")`) and fail on compressed bytes — the
//! root cause of GAP-AUD-003 being inoperant in production.
//!
//! This module wraps `Response::bytes()` and inspects the
//! `Content-Encoding` header to dispatch to the correct decoder before
//! returning the decoded UTF-8 string to the caller. A safety cap of
//! [`DECOMPRESSION_MAX_OUTPUT`] bytes protects against gzip bombs: a small
//! compressed payload that decompresses to gigabytes of data.

use std::io::Read;

use crate::error::CliError;

/// Maximum number of bytes accepted after decompression.
///
/// Protects against gzip bombs: an attacker serves a small body that
/// decompresses to gigabytes. Set to 32 MiB which is well above the
/// largest legitimate HTML result page but bounded enough to abort a
/// bomb attempt before exhausting memory.
pub const DECOMPRESSION_MAX_OUTPUT: usize = 32 * 1024 * 1024;

/// Reads the response body and returns the decoded UTF-8 string.
///
/// Inspects the `Content-Encoding` header and dispatches:
/// - `identity` (or absent) — passes bytes through unchanged.
/// - `gzip` — decodes via [`flate2::read::MultiGzDecoder`] (handles
///   concatenated gzip streams transparently).
/// - `deflate` — decodes via [`flate2::read::ZlibDecoder`].
/// - `br` — returns [`CliError::UnsupportedEncoding`] (brotli removed in
///   v0.8.6 with the wreq-to-reqwest migration; `DuckDuckGo` never serves brotli for HTML endpoints).
/// - Anything else — [`CliError::UnsupportedEncoding`].
///
/// Returns [`CliError::PayloadTooLarge`] if decompression exceeds
/// [`DECOMPRESSION_MAX_OUTPUT`] bytes. Returns [`CliError::InvalidUtf8`]
/// if the decoded bytes are not valid UTF-8.
///
/// # Errors
///
/// - [`CliError::HttpClient`] if reading the response body fails at the
///   transport layer (DNS, TLS, connection reset).
/// - [`CliError::UnsupportedEncoding`] if the `Content-Encoding` header
///   carries an encoding this module does not handle (e.g. `zstd`).
/// - [`CliError::PayloadTooLarge`] if decompression exceeds the 32 MiB
///   safety cap (gzip bomb protection).
/// - [`CliError::DecompressionIo`] if the decoder returns an I/O error
///   (corrupt stream, truncated payload).
/// - [`CliError::InvalidUtf8`] if the decoded bytes are not valid UTF-8.
pub async fn response_body_string(response: reqwest::Response) -> Result<String, CliError> {
    let encoding = response
        .headers()
        .get(reqwest::header::CONTENT_ENCODING)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("identity")
        .to_ascii_lowercase();

    let bytes = response.bytes().await?;
    // Dispatch the synchronous `flate2` / `brotli-decompressor` work to the
    // blocking thread pool. These decoders are CPU-bound and would otherwise
    // stall the tokio worker thread; `spawn_blocking` returns a future that
    // resolves when the blocking task completes. See ADR (planned) for the
    // measurement that motivated this.
    let decoded = tokio::task::spawn_blocking(move || decode_bytes(&bytes, &encoding))
        .await
        .map_err(|e| {
            CliError::DecompressionIo(std::io::Error::other(format!(
                "decompression task panicked: {e}"
            )))
        })??;
    String::from_utf8(decoded).map_err(CliError::from)
}

/// Decompresses raw bytes using the given `Content-Encoding` value.
///
/// Public so call sites that already hold a `Vec<u8>` (e.g.
/// [`crate::content`]) can reuse the same decoder dispatch without
/// re-reading the response. `encoding` is expected to already be
/// lowercased by the caller; if not, this function lowercases it.
///
/// # Errors
///
/// - [`CliError::UnsupportedEncoding`] for encodings this module does
///   not handle.
/// - [`CliError::PayloadTooLarge`] if decompression exceeds the 32 MiB
///   safety cap.
/// - [`CliError::DecompressionIo`] for corrupt or truncated streams.
pub fn decode_bytes(bytes: &[u8], encoding: &str) -> Result<Vec<u8>, CliError> {
    let encoding = encoding.to_ascii_lowercase();
    let decoded = match encoding.as_str() {
        "identity" | "" => bytes.to_vec(),
        "gzip" => decode_with_cap(bytes, |slice| {
            let mut out = Vec::with_capacity(bytes.len() * 3);
            flate2::read::MultiGzDecoder::new(slice)
                .take(u64::try_from(DECOMPRESSION_MAX_OUTPUT).unwrap_or(u64::MAX) + 1)
                .read_to_end(&mut out)?;
            Ok(out)
        })?,
        "deflate" => decode_with_cap(bytes, |slice| {
            let mut out = Vec::with_capacity(bytes.len() * 3);
            flate2::read::ZlibDecoder::new(slice)
                .take(u64::try_from(DECOMPRESSION_MAX_OUTPUT).unwrap_or(u64::MAX) + 1)
                .read_to_end(&mut out)?;
            Ok(out)
        })?,
        "br" => {
            return Err(CliError::UnsupportedEncoding(
                "br (brotli removed in v0.8.6)".to_string(),
            ))
        }
        other => return Err(CliError::UnsupportedEncoding(other.to_string())),
    };

    let bytes_in = bytes.len();
    let bytes_out = decoded.len();
    tracing::info!(
        encoding = %encoding,
        bytes_in,
        bytes_out,
        "decompressed response body"
    );

    Ok(decoded)
}

/// Helper that runs a decoder closure and enforces the [`DECOMPRESSION_MAX_OUTPUT`] cap.
///
/// The closure MUST honor the `.take(cap + 1)` semantic so we can detect
/// when the stream exceeds the cap and abort cleanly. If the returned
/// `Vec` is exactly `cap + 1` long, the cap was hit and we return
/// [`CliError::PayloadTooLarge`] with the actual size reported as
/// `cap + 1` (a lower bound on the true size).
fn decode_with_cap<F>(bytes: &[u8], decoder: F) -> Result<Vec<u8>, CliError>
where
    F: FnOnce(&[u8]) -> std::io::Result<Vec<u8>>,
{
    let out = decoder(bytes)?;
    if out.len() > DECOMPRESSION_MAX_OUTPUT {
        return Err(CliError::PayloadTooLarge {
            max: DECOMPRESSION_MAX_OUTPUT,
            actual: out.len(),
        });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cap_constant_is_32_mib() {
        assert_eq!(DECOMPRESSION_MAX_OUTPUT, 32 * 1024 * 1024);
    }

    #[test]
    fn decoder_helper_rejects_oversize_payload() {
        // Build a payload that the decoder closure reports as 2x the cap.
        let oversize = vec![0u8; DECOMPRESSION_MAX_OUTPUT + 1024];
        let result = decode_with_cap(&[], |_| Ok(oversize.clone()));
        match result {
            Err(CliError::PayloadTooLarge { max, actual }) => {
                assert_eq!(max, DECOMPRESSION_MAX_OUTPUT);
                assert_eq!(actual, oversize.len());
            }
            other => panic!("expected PayloadTooLarge, got {other:?}"),
        }
    }

    #[test]
    fn decoder_helper_accepts_undersize_payload() {
        let small = vec![0u8; 1024];
        let result = decode_with_cap(&[], |_| Ok(small.clone()));
        assert_eq!(result.unwrap().len(), 1024);
    }
}
