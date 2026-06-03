// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload: I/O-bound + CPU (HTTP fetch + readability extraction)
//! Full text content extraction from URLs (flag `--fetch-content`).
//!
//! Pure HTTP implementation (iteration 5). For each URL:
//! 1. Makes an HTTP request with `reqwest::Client`.
//! 2. Checks `Content-Type` — accepts only `text/html` and variants.
//! 3. Reads body as `Vec<u8>`, detects charset from header and converts to UTF-8
//!    with `encoding_rs` (fallback `from_utf8_lossy` for UTF-8/absent).
//! 4. Parses with `scraper` and applies simplified readability (5 steps):
//!    - Removes chrome elements (nav, header, footer, script, style, aside, forms).
//!    - Identifies main container (article → main → [role=main] → body).
//!    - Extracts text from relevant blocks (p, h1-6, li, blockquote, pre, td).
//!    - Cleans up (excessive whitespace, short lines).
//!    - Truncates at `max_size` respecting word boundaries.
//! 5. If clean text < 200 chars → returns empty string signalling that
//!    Chrome is likely needed (iteration 6).
//!
//! Headless Chrome fallback will come in iteration 6 under feature `chrome`.

use crate::error::CliError;
use reqwest::Client;
use scraper::{Html, Selector};
use std::net::IpAddr;
use tokio_util::sync::CancellationToken;

/// Threshold below which we consider content "insufficient" (Chrome fallback candidate).
const MIN_CONTENT_THRESHOLD: usize = 200;

/// Character threshold per line to discard very short lines (e.g. navigation boilerplate).
const MIN_LINE_LENGTH: usize = 20;

/// Hard cap on HTTP response body size before allocation (5 MB).
const MAX_BODY_BYTES: usize = 5 * 1024 * 1024;

/// Validates that a URL is safe to fetch (SSRF protection).
///
/// Rejects non-HTTP schemes (`file://`, `ftp://`, `data:`, etc.) and
/// hosts that resolve to private/loopback IP ranges (RFC 1918, RFC 4193).
fn is_safe_url(url: &str) -> bool {
    let parsed = match reqwest::Url::parse(url) {
        Ok(u) => u,
        Err(_) => return false,
    };

    match parsed.scheme() {
        "http" | "https" => {}
        _ => return false,
    }

    let host = match parsed.host_str() {
        Some(h) => h,
        None => return false,
    };

    if host == "localhost" {
        return false;
    }

    let host_clean = host.trim_start_matches('[').trim_end_matches(']');
    if let Ok(ip) = host_clean.parse::<IpAddr>() {
        return !is_private_ip(ip);
    }

    true
}

fn is_private_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            v4.is_loopback()
                || v4.is_private()
                || v4.is_link_local()
                || v4.octets()[0] == 169 && v4.octets()[1] == 254
                || v4.is_broadcast()
                || v4.is_unspecified()
        }
        IpAddr::V6(v6) => v6.is_loopback() || v6.is_unspecified(),
    }
}

/// Extracts the main text content from a URL via pure HTTP.
///
/// Returns:
/// - `Ok(Some((clean_text, original_size_in_bytes)))` on success.
/// - `Ok(None)` if the `Content-Type` is not HTML (pdf, image, etc.).
/// - `Err` on unrecoverable network/parse failure.
///
/// The returned text may be empty if extraction produced no content > 200 chars —
/// in that case the caller knows a Chrome fallback would be needed.
///
/// # Errors
///
/// Returns an error if the HTTP request fails, the response body
/// cannot be read, or the operation is cancelled via the token.
///
/// # Cancel safety
///
/// This function is cancel-safe. Each `.await` point races against
/// the cancellation token via `tokio::select!`, so dropping the
/// future does not leak resources.
pub async fn extract_http_content(
    client: &Client,
    url: &str,
    max_size: usize,
    token: &CancellationToken,
) -> Result<Option<(String, u32)>, CliError> {
    if token.is_cancelled() {
        return Err(CliError::NetworkError {
            message: format!("extraction cancelled for {url:?}"),
        });
    }

    if std::env::var("DUCKDUCKGO_SEARCH_CLI_SKIP_SSRF").is_err() && !is_safe_url(url) {
        tracing::warn!(
            url,
            "URL rejected by SSRF filter — unsafe scheme or private host"
        );
        return Ok(None);
    }

    tracing::debug!(url, "starting HTTP content extraction");

    let response = tokio::select! {
        biased;
        _ = token.cancelled() => {
            return Err(CliError::NetworkError {
                message: format!("extraction cancelled during request for {url:?}"),
            });
        }
        res = client.get(url).send() => res.map_err(|e| CliError::HttpError {
            message: format!("HTTP request failed for {url}: {e}"),
            cause: Some(e.into()),
        })?
    };

    if !response.status().is_success() {
        tracing::debug!(url, status = %response.status(), "non-success HTTP status — discarding");
        return Ok(None);
    }

    // Extrai charset do Content-Type ANTES de consumir o body.
    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();

    if !is_html(&content_type) {
        tracing::debug!(url, content_type, "Content-Type is not HTML — discarding");
        return Ok(None);
    }

    let charset = extract_charset(&content_type);

    if let Some(cl) = response.headers().get(reqwest::header::CONTENT_LENGTH) {
        if let Ok(size_str) = cl.to_str() {
            if let Ok(size) = size_str.parse::<u64>() {
                if size > MAX_BODY_BYTES as u64 {
                    tracing::warn!(
                        url,
                        content_length = size,
                        limit = MAX_BODY_BYTES,
                        "response body exceeds size limit — skipping"
                    );
                    return Ok(None);
                }
            }
        }
    }

    let bytes = tokio::select! {
        biased;
        _ = token.cancelled() => {
            return Err(CliError::NetworkError {
                message: format!("extraction cancelled during body read for {url:?}"),
            });
        }
        res = response.bytes() => res.map_err(|e| CliError::HttpError {
            message: format!("failed to read response body for {url}: {e}"),
            cause: Some(e.into()),
        })?
    };

    if bytes.len() > MAX_BODY_BYTES {
        tracing::warn!(
            url,
            actual_size = bytes.len(),
            limit = MAX_BODY_BYTES,
            "downloaded body exceeds size limit — discarding"
        );
        return Ok(None);
    }

    let size_original = u32::try_from(bytes.len()).unwrap_or(u32::MAX);
    tracing::debug!(url, size = bytes.len(), "body downloaded");

    // Decodifica para UTF-8 usando encoding_rs + fallback lossy.
    let html_utf8 = decode_to_utf8(&bytes, charset.as_deref());

    // Parse + readability run in the blocking pool: scraper uses Rc<_> internally
    // (html5ever) and is NOT Send. spawn_blocking moves us to a dedicated thread pool.
    // spawn_blocking concurrency is bounded indirectly: callers in
    // content_fetch::enrich_with_content hold a global semaphore permit,
    // so at most `parallelism` tasks reach this point concurrently.
    let max_size_local = max_size;
    let clean_text =
        tokio::task::spawn_blocking(move || apply_readability(&html_utf8, max_size_local))
            .await
            .map_err(|err| CliError::NetworkError {
                message: format!("readability task panicked: {err}"),
            })?;

    if clean_text.len() < MIN_CONTENT_THRESHOLD {
        tracing::debug!(
            url,
            len = clean_text.len(),
            "extracted content below threshold — signalling possible Chrome need"
        );
        // Return empty string + original size for signaling (iteration 6 will fallback).
        return Ok(Some((String::new(), size_original)));
    }

    tracing::debug!(url, clean_size = clean_text.len(), "extraction complete");
    Ok(Some((clean_text, size_original)))
}

/// Checks whether the Content-Type corresponds to HTML (flexible for `text/html; charset=...`).
fn is_html(content_type: &str) -> bool {
    let b = content_type.as_bytes();
    (b.len() >= 9 && b[..9].eq_ignore_ascii_case(b"text/html"))
        || (b.len() >= 21 && b[..21].eq_ignore_ascii_case(b"application/xhtml+xml"))
}

/// Extrai o valor de `charset=` de um Content-Type (se presente).
fn extract_charset(content_type: &str) -> Option<String> {
    for part in content_type.split(';') {
        let trimmed = part.trim();
        if let Some(value) = trimmed.strip_prefix("charset=") {
            let clean = value.trim_matches(|c: char| c == '"' || c == '\'');
            if !clean.is_empty() {
                return Some(clean.to_ascii_lowercase());
            }
        }
    }
    None
}

/// Decodes bytes to a UTF-8 `String` using the declared charset (if provided).
///
/// - If `charset` is UTF-8 or absent → `from_utf8_lossy` (fast path).
/// - Otherwise → `Encoding::for_label().decode()` with WINDOWS-1252 fallback on unknown label.
pub fn decode_to_utf8(bytes: &[u8], charset: Option<&str>) -> String {
    let label = charset.unwrap_or("utf-8");
    if label == "utf-8" || label == "utf8" || label.is_empty() {
        return match std::str::from_utf8(bytes) {
            Ok(valido) => valido.to_string(),
            Err(_) => String::from_utf8_lossy(bytes).into_owned(),
        };
    }

    match encoding_rs::Encoding::for_label(label.as_bytes()) {
        Some(enc) => {
            let (cow, _used, _had_errors) = enc.decode(bytes);
            cow.into_owned()
        }
        None => {
            tracing::debug!(
                charset = label,
                "unknown charset label — fallback to UTF-8 lossy"
            );
            String::from_utf8_lossy(bytes).into_owned()
        }
    }
}

/// Applies simplified readability in 5 steps over UTF-8 HTML.
///
/// Returns clean text truncated at `max_size` characters (respecting word boundaries).
/// Called from within `spawn_blocking` because `scraper::Html` is not `Send`.
fn apply_readability(html: &str, max_size: usize) -> String {
    let document = Html::parse_document(html);

    // Step 1: list of CSS selectors that MUST be IGNORED (chrome/navigation/scripts).
    // scraper doesn't support easy in-place removal, so instead we collect SEMANTICS of
    // "valid elements within the main container ignoring descendants of chrome".
    // Strategy: find the main container, iterate text blocks PROVIDED THAT
    // no ancestor is a chrome element.

    // Step 2: identify main container.
    let mut container_ref = None;
    for sel in cached_sel_containers() {
        if let Some(first_match) = document.select(sel).next() {
            container_ref = Some(first_match);
            break;
        }
    }

    // Fallback: body inteiro.
    let container = match container_ref {
        Some(c) => c,
        None => match document.select(cached_sel_body()).next() {
            Some(b) => b,
            None => return String::new(),
        },
    };

    // Step 3: extract text from relevant blocks within the container.
    let blocks = cached_sel_blocks();

    // IGNORED element selectors — if any ancestor is of this type, we skip.
    // `scraper` doesn't give us direct ancestor iteration — we simulate by checking parent tags.
    // Simple strategy: for each block, walk up the chain to the root and discard if
    // a forbidden tag is found.
    let excluded_tags: &[&str] = &[
        "nav", "header", "footer", "aside", "script", "style", "noscript", "iframe", "svg", "form",
    ];
    let excluded_classes: &[&str] = &[
        "sidebar",
        "nav",
        "menu",
        "footer",
        "header",
        "ad",
        "advertisement",
        "social-share",
    ];
    let excluded_roles: &[&str] = &["navigation", "banner", "contentinfo"];

    let mut lines_vec: Vec<String> = Vec::with_capacity(64);
    for block in container.select(blocks) {
        if has_excluded_ancestor(block, excluded_tags, excluded_classes, excluded_roles) {
            continue;
        }
        let mut text = String::with_capacity(256);
        let mut needs_space = false;
        for fragment in block.text() {
            for word in fragment.split_whitespace() {
                if needs_space {
                    text.push(' ');
                }
                text.push_str(word);
                needs_space = true;
            }
        }
        if !text.is_empty() {
            lines_vec.push(text);
        }
    }

    // Step 4: cleanup — short lines discarded, normalize whitespace between lines.
    let mut content = String::with_capacity(lines_vec.len() * 100);
    let mut is_first = true;
    for l in lines_vec {
        if l.chars().count() >= MIN_LINE_LENGTH {
            if !is_first {
                content.push('\n');
            }
            content.push_str(&l);
            is_first = false;
        }
    }

    // Step 5: truncate at max_size characters respecting word boundaries.
    truncate_at_word(&content, max_size)
}

/// Checks whether an element (or any ancestor) belongs to the "chrome" categories.
///
/// Uses tree traversal via `parent()` up to the root (Document).
fn has_excluded_ancestor(
    element: scraper::ElementRef<'_>,
    tags: &[&str],
    classes: &[&str],
    roles: &[&str],
) -> bool {
    // The element itself matched the block selector (p/h1/etc), but may be
    // nested inside a nav/header. Walk up the parent chain.
    let mut current_node = element.parent();
    while let Some(node) = current_node {
        if let Some(el) = scraper::ElementRef::wrap(node) {
            let nome = el.value().name();
            if tags.iter().any(|t| t.eq_ignore_ascii_case(nome)) {
                return true;
            }
            if let Some(class_attr) = el.value().attr("class") {
                for c in class_attr.split_ascii_whitespace() {
                    if classes
                        .iter()
                        .any(|excluded| c.eq_ignore_ascii_case(excluded))
                    {
                        return true;
                    }
                }
            }
            if let Some(role) = el.value().attr("role") {
                if roles.iter().any(|r| r.eq_ignore_ascii_case(role)) {
                    return true;
                }
            }
        }
        current_node = node.parent();
    }
    false
}

/// Truncates `text` at `max_size` characters respecting word boundaries.
///
/// If the cut falls in the middle of a word, backs up to the last whitespace.
/// If there is no whitespace, performs a hard cut at the nearest valid character boundary.
fn truncate_at_word(text: &str, max_size: usize) -> String {
    if max_size == 0 {
        return String::new();
    }
    let byte_pos = text.char_indices().nth(max_size).map(|(i, _)| i);
    let Some(cut) = byte_pos else {
        return text.to_string();
    };
    let prefix = &text[..cut];
    if let Some(pos) = prefix.rfind(char::is_whitespace) {
        return prefix[..pos].trim_end().to_string();
    }
    prefix.to_string()
}

fn cached_sel_containers() -> &'static [Selector] {
    use std::sync::OnceLock;
    static C: OnceLock<Vec<Selector>> = OnceLock::new();
    C.get_or_init(|| {
        [
            "article",
            "main",
            "[role=\"main\"]",
            ".post-content",
            ".article-body",
            ".entry-content",
            "#content",
            ".content",
        ]
        .iter()
        .filter_map(|s| Selector::parse(s).ok())
        .collect()
    })
}

fn cached_sel_body() -> &'static Selector {
    use std::sync::OnceLock;
    static C: OnceLock<Selector> = OnceLock::new();
    C.get_or_init(|| Selector::parse("body").unwrap())
}

fn cached_sel_blocks() -> &'static Selector {
    use std::sync::OnceLock;
    static C: OnceLock<Selector> = OnceLock::new();
    C.get_or_init(|| {
        Selector::parse("p, h1, h2, h3, h4, h5, h6, li, blockquote, pre, td, th").unwrap()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_html_accepts_text_html_and_variants() {
        assert!(is_html("text/html"));
        assert!(is_html("text/html; charset=utf-8"));
        assert!(is_html("application/xhtml+xml"));
        assert!(is_html("TEXT/HTML"));
    }

    #[test]
    fn is_html_rejects_non_html() {
        assert!(!is_html("application/pdf"));
        assert!(!is_html("image/png"));
        assert!(!is_html("application/json"));
        assert!(!is_html(""));
    }

    #[test]
    fn extract_charset_identifies_utf8() {
        assert_eq!(
            extract_charset("text/html; charset=UTF-8"),
            Some("utf-8".to_string())
        );
        assert_eq!(
            extract_charset("text/html; charset=\"iso-8859-1\""),
            Some("iso-8859-1".to_string())
        );
    }

    #[test]
    fn extract_charset_absent_returns_none() {
        assert_eq!(extract_charset("text/html"), None);
        assert_eq!(extract_charset(""), None);
    }

    #[test]
    fn decodificar_utf8_puro() {
        let bytes = "olá mundo".as_bytes();
        let s = decode_to_utf8(bytes, None);
        assert_eq!(s, "olá mundo");
        let s2 = decode_to_utf8(bytes, Some("utf-8"));
        assert_eq!(s2, "olá mundo");
    }

    #[test]
    fn decode_latin1_to_utf8() {
        // 'a-acute' (U+00E1) in Latin-1 is byte 0xE1.
        let bytes: &[u8] = &[0xE1, 0x6C, 0x6F];
        let s = decode_to_utf8(bytes, Some("iso-8859-1"));
        assert_eq!(s, "álo");
    }

    #[test]
    fn decode_windows1252_to_utf8() {
        // 'c-cedilla' (U+00E7) in Windows-1252 is byte 0xE7.
        let bytes: &[u8] = &[0xE7];
        let s = decode_to_utf8(bytes, Some("windows-1252"));
        assert_eq!(s, "ç");
    }

    #[test]
    fn decode_unknown_charset_falls_back_to_utf8_lossy() {
        let bytes = "teste".as_bytes();
        let s = decode_to_utf8(bytes, Some("charset-que-nao-existe"));
        assert_eq!(s, "teste");
    }

    #[test]
    fn truncate_at_word_preserves_boundary() {
        let text = "uma frase qualquer com várias palavras";
        let t = truncate_at_word(text, 10);
        assert!(t.len() <= 10);
        assert!(!t.ends_with(' '));
        // Must not cut in the middle of a word.
        assert!(
            text.starts_with(&t),
            "truncated ({t:?}) must be a prefix of the original"
        );
    }

    #[test]
    fn truncate_at_word_short_text_returns_original() {
        assert_eq!(truncate_at_word("oi", 100), "oi");
        assert_eq!(truncate_at_word("", 100), "");
    }

    #[test]
    fn truncate_at_word_no_whitespace_cuts_hard() {
        let t = truncate_at_word("palavraSemEspacoNenhum", 10);
        assert_eq!(t.chars().count(), 10);
    }

    #[test]
    fn readability_extrai_artigo_simples() {
        let html = r#"<html><body>
            <nav><a href="/">Menu</a></nav>
            <article>
              <h1>Título do Artigo</h1>
              <p>Este é o primeiro parágrafo do artigo com pelo menos vinte caracteres de conteúdo substantivo.</p>
              <p>Segundo parágrafo também com conteúdo suficiente para passar do limiar de linha mínima.</p>
            </article>
            <footer>Copyright</footer>
            </body></html>"#;
        let text = apply_readability(html, 1000);
        assert!(text.contains("primeiro parágrafo"));
        assert!(text.contains("Segundo parágrafo"));
        // Navigation and footer must be omitted.
        assert!(!text.contains("Menu"));
        assert!(!text.contains("Copyright"));
    }

    #[test]
    fn readability_uses_main_when_no_article() {
        let html = r#"<html><body>
            <header>Cabeçalho irrelevante</header>
            <main>
              <p>Conteúdo principal via tag main, com mais de vinte caracteres de texto útil aqui.</p>
              <p>Outro parágrafo relevante com conteúdo suficiente para não ser descartado.</p>
            </main>
            </body></html>"#;
        let text = apply_readability(html, 1000);
        assert!(text.contains("Conteúdo principal"));
        assert!(text.contains("Outro parágrafo"));
        assert!(!text.contains("Cabeçalho"));
    }

    #[test]
    fn readability_remove_script_style_nav() {
        let html = r#"<html><body>
            <nav><p>Este parágrafo dentro da nav deve ser descartado porque é chrome.</p></nav>
            <article>
              <script>var x = 1;</script>
              <style>.a { color: red; }</style>
              <p>Parágrafo legítimo dentro de article com conteúdo o bastante para passar o limiar.</p>
            </article>
            </body></html>"#;
        let text = apply_readability(html, 1000);
        assert!(text.contains("Parágrafo legítimo"));
        assert!(!text.contains("dentro da nav"));
        assert!(!text.contains("var x = 1"));
        assert!(!text.contains("color: red"));
    }

    #[test]
    fn readability_truncates_at_max_size() {
        let long_content = "Parágrafo um com pelo menos vinte caracteres aqui.\n".repeat(100);
        let html = format!("<html><body><article><p>{long_content}</p></article></body></html>");
        let text = apply_readability(&html, 200);
        assert!(text.chars().count() <= 200);
    }

    #[test]
    fn readability_returns_empty_without_enough_content() {
        // Apenas nav e footer — nada no main/article.
        let html = r#"<html><body>
            <nav>Menu curto</nav>
            <footer>Rodapé breve.</footer>
            </body></html>"#;
        let text = apply_readability(html, 1000);
        // Should be an empty string (or very short), signaling that fallback is needed.
        assert!(
            text.len() < MIN_CONTENT_THRESHOLD,
            "sem conteúdo substantivo esperado, obtido: {text:?}"
        );
    }

    #[test]
    fn ssrf_rejects_file_scheme() {
        assert!(!is_safe_url("file:///etc/passwd"));
    }

    #[test]
    fn ssrf_rejects_ftp_scheme() {
        assert!(!is_safe_url("ftp://internal.corp/data"));
    }

    #[test]
    fn ssrf_rejects_data_scheme() {
        assert!(!is_safe_url("data:text/html,<h1>hi</h1>"));
    }

    #[test]
    fn ssrf_rejects_loopback_ipv4() {
        assert!(!is_safe_url("http://127.0.0.1/secret"));
    }

    #[test]
    fn ssrf_rejects_private_range_10() {
        assert!(!is_safe_url("http://10.0.0.1/internal"));
    }

    #[test]
    fn ssrf_rejects_private_range_192() {
        assert!(!is_safe_url("http://192.168.1.1/admin"));
    }

    #[test]
    fn ssrf_rejects_link_local() {
        assert!(!is_safe_url("http://169.254.169.254/metadata"));
    }

    #[test]
    fn ssrf_rejects_localhost() {
        assert!(!is_safe_url("http://localhost/admin"));
    }

    #[test]
    fn ssrf_accepts_public_https() {
        assert!(is_safe_url("https://www.example.com/page"));
    }

    #[test]
    fn ssrf_accepts_public_http() {
        assert!(is_safe_url("http://example.com/page"));
    }

    #[test]
    fn ssrf_rejects_ipv6_loopback() {
        assert!(!is_safe_url("http://[::1]/secret"));
    }
}
