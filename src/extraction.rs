// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload: CPU-bound (HTML parsing and text extraction via scraper)
//! Extraction of search results from `DuckDuckGo` HTML.
//!
//! In the MVP implements ONLY Strategy 1 (stable class selectors):
//! - Container: `#links`.
//! - Items: `.result` (multiple alternative selectors).
//! - Title + URL: `.result__a`.
//! - Snippet: `.result__snippet`.
//! - Display URL: `.result__url`.
//!
//! Ad filtering:
//! - Removes elements with class `.result--ad` or `.badge--ad`.
//! - Removes elements with attribute `data-nrn="ad"`.
//! - Removes results whose URL contains `duckduckgo.com/y.js`.
//!
//! URL resolution:
//! - Protocol-relative URLs (`//example.com`) are prefixed with `https:`.
//! - URLs containing a `DuckDuckGo` internal redirect (`/l/?uddg=...&rut=...`) are
//!   unwrapped via URL-decoding of the `uddg` parameter.
//! - URLs on the `duckduckgo.com` domain itself are filtered out.

use crate::types::{SearchResult, SelectorConfig};
use scraper::{ElementRef, Html, Selector};
use std::sync::OnceLock;

fn sel_tr() -> &'static Selector {
    static C: OnceLock<Selector> = OnceLock::new();
    C.get_or_init(|| Selector::parse("tr").unwrap())
}

fn sel_strategy2_links() -> &'static Selector {
    static C: OnceLock<Selector> = OnceLock::new();
    C.get_or_init(|| Selector::parse("#links a[href], .result a[href]").unwrap())
}

pub(crate) struct CompiledSelectors {
    pub result_item: Selector,
    pub ad_class: Option<Selector>,
    pub title_sel: Option<Selector>,
    pub snippet: Option<Selector>,
    pub display_url_sel: Option<Selector>,
    pub ad_classes_raw: Vec<String>,
    pub ad_attributes: Vec<(String, String)>,
    pub url_patterns: Vec<String>,
}

impl CompiledSelectors {
    pub fn compile(cfg: &SelectorConfig) -> Option<Self> {
        let result_item = match Selector::parse(&cfg.html_endpoint.result_item) {
            Ok(s) => s,
            Err(error) => {
                tracing::error!(
                    ?error,
                    selector = %cfg.html_endpoint.result_item,
                    "Result selector invalid — cannot extract"
                );
                return None;
            }
        };
        let join_ad = cfg.html_endpoint.ads_filter.ad_classes.join(", ");
        let ad_class = if join_ad.is_empty() {
            None
        } else {
            Selector::parse(&join_ad).ok()
        };
        let title_sel = Selector::parse(&cfg.html_endpoint.title_and_url).ok();
        let snippet = Selector::parse(&cfg.html_endpoint.snippet).ok();
        let display_url_sel = Selector::parse(&cfg.html_endpoint.display_url).ok();
        let ad_classes_raw = cfg
            .html_endpoint
            .ads_filter
            .ad_classes
            .iter()
            .map(|c| c.trim_start_matches('.').to_string())
            .collect();
        let ad_attributes = cfg
            .html_endpoint
            .ads_filter
            .ad_attributes
            .iter()
            .filter_map(|e| {
                let mut parts = e.splitn(2, '=');
                let key = parts.next()?.trim().to_string();
                let value = parts.next()?.trim().to_string();
                Some((key, value))
            })
            .collect();
        let url_patterns = cfg.html_endpoint.ads_filter.ad_url_patterns.to_vec();
        Some(Self {
            result_item,
            ad_class,
            title_sel,
            snippet,
            display_url_sel,
            ad_classes_raw,
            ad_attributes,
            url_patterns,
        })
    }
}

pub(crate) struct CompiledLiteSelectors {
    pub link: Selector,
    pub snippet_td: Selector,
}

impl CompiledLiteSelectors {
    pub fn compile(cfg: &SelectorConfig) -> Option<Self> {
        let link = Selector::parse(&cfg.lite_endpoint.result_link)
            .or_else(|_| Selector::parse("a.result-link, a"))
            .ok()?;
        let snippet_td = Selector::parse(&cfg.lite_endpoint.result_snippet)
            .or_else(|_| Selector::parse("td.result-snippet, td"))
            .ok()?;
        Some(Self { link, snippet_td })
    }
}

/// Bounded limits to prevent absurdly large payloads (section 5.4 — rule 4).
const TITLE_LIMIT: usize = 200;
const URL_LIMIT: usize = 2000;
const SNIPPET_LIMIT: usize = 500;

fn join_text(el: &ElementRef<'_>) -> String {
    let mut out = String::with_capacity(128);
    let mut need_space = false;
    for frag in el.text() {
        for word in frag.split_whitespace() {
            if need_space {
                out.push(' ');
            }
            out.push_str(word);
            need_space = true;
        }
    }
    out
}

/// Extracts the organic results from a `DuckDuckGo` HTML page using Strategy 1.
///
/// Returns results already filtered (no ads), with resolved URLs and positions
/// numbered sequentially from 1.
///
/// If no results are found, returns an empty `Vec` (not an error — the query may simply
/// have no results; actual malformed-HTML errors are handled further up the call stack).
pub fn extract_results(raw_html: &str) -> Vec<SearchResult> {
    let cfg = SelectorConfig::default();
    extract_results_with_cfg(raw_html, &cfg)
}

/// Same as `extract_results`, but accepts a custom `SelectorConfig`.
///
/// Iteration 6: allows selectors loaded from an external TOML file to be applied.
pub fn extract_results_with_cfg(raw_html: &str, cfg: &SelectorConfig) -> Vec<SearchResult> {
    let document = Html::parse_document(raw_html);
    let Some(compiled) = CompiledSelectors::compile(cfg) else {
        return Vec::new();
    };
    extract_with_document(&document, &compiled)
}

/// Applies Strategy 1 and, if it returns empty, applies Strategy 2 (semantic fallback).
///
/// Strategy 2 searches all `<a href="...">` links inside `#links` that point to
/// an external domain; for each one it extracts the link text as the title, unwraps
/// the href with `resolve_url`, and attempts to extract a snippet from the parent
/// element (looks for the ancestor with substantial text).
pub fn extract_results_with_strategies(raw_html: &str) -> Vec<SearchResult> {
    let cfg = SelectorConfig::default();
    extract_results_with_strategies_cfg(raw_html, &cfg)
}

/// Same as `extract_results_with_strategies`, but accepts external selectors.
pub fn extract_results_with_strategies_cfg(
    raw_html: &str,
    cfg: &SelectorConfig,
) -> Vec<SearchResult> {
    let document = Html::parse_document(raw_html);
    let mut results = match CompiledSelectors::compile(cfg) {
        Some(compiled) => extract_with_document(&document, &compiled),
        None => Vec::new(),
    };
    if !results.is_empty() {
        return results;
    }

    tracing::debug!("Strategy 1 returned empty — trying Strategy 2 (semantic fallback)");
    results = extract_strategy_2(&document);
    if !results.is_empty() {
        tracing::info!(total = results.len(), "Strategy 2 recovered results");
    }
    results
}

/// Strategy 2: semantic fallback. Searches all external `<a href>` links inside
/// the results container (`#links`) and extracts title, URL and snippet.
fn extract_strategy_2(document: &Html) -> Vec<SearchResult> {
    let links_selector = sel_strategy2_links();

    let mut results = Vec::with_capacity(16);
    let mut position: u32 = 0;
    let mut seen_urls: std::collections::HashSet<String> =
        std::collections::HashSet::with_capacity(16);

    for link in document.select(links_selector) {
        let href = match link.value().attr("href") {
            Some(h) if !h.is_empty() => h,
            _ => continue,
        };
        let resolved_url = match resolve_url(href) {
            Some(u) => u,
            None => continue,
        };
        if resolved_url.contains("duckduckgo.com/y.js") || resolved_url.len() > URL_LIMIT {
            continue;
        }
        // Deduplicate by URL.
        if !seen_urls.insert(resolved_url.clone()) {
            continue;
        }

        let raw_title = join_text(&link);
        let title = normalize_text(&raw_title, TITLE_LIMIT);
        if title.is_empty() {
            continue;
        }

        // Look for an ancestor with substantial text to extract as snippet.
        let snippet = extract_snippet_from_ancestor(&link, &title);

        position += 1;
        results.push(SearchResult {
            position,
            title,
            url: resolved_url,
            display_url: None,
            snippet,
            original_title: None,
            content: None,
            content_size: None,
            content_extraction_method: None,
        });

        // Sanity limit to avoid pages that explode the list.
        if results.len() >= 50 {
            break;
        }
    }

    results
}

/// Walks the link's ancestors looking for the first one with "substantial" text
/// (at least 40 characters distinct from the title itself).
fn extract_snippet_from_ancestor(link: &ElementRef<'_>, title: &str) -> Option<String> {
    let mut atual = link.parent();
    let mut nivel = 0;
    while let Some(no) = atual {
        nivel += 1;
        if nivel > 5 {
            break;
        }
        if let Some(el) = ElementRef::wrap(no) {
            let text = join_text(&el);
            let normalized = normalize_text(&text, SNIPPET_LIMIT);
            // Remove the title from the text to isolate the "rest" that may be a snippet.
            let without_title = normalized.replacen(title, "", 1);
            let without_title_tr = without_title.trim();
            if without_title_tr.chars().count() >= 40 {
                return Some(normalize_text(without_title_tr, SNIPPET_LIMIT));
            }
        }
        atual = no.parent();
    }
    None
}

/// Strategy 3: extraction for the Lite endpoint (`https://lite.duckduckgo.com/lite/`).
///
/// Lite returns tabular HTML. We iterate over `<tr>` elements capturing pairs:
/// 1. `<tr>` with `<a class="result-link">` (or any `<a>` in `<td>`) → title/URL.
/// 2. The following `<tr>` with `td.result-snippet` (or a `<td>` with substantial text) → snippet.
pub fn extract_results_lite(raw_html: &str) -> Vec<SearchResult> {
    let cfg = SelectorConfig::default();
    extract_results_lite_with_cfg(raw_html, &cfg)
}

/// Same as `extract_results_lite`, but accepts external selectors.
pub fn extract_results_lite_with_cfg(raw_html: &str, cfg: &SelectorConfig) -> Vec<SearchResult> {
    let document = Html::parse_document(raw_html);
    let Some(compiled_lite) = CompiledLiteSelectors::compile(cfg) else {
        return Vec::new();
    };
    let sel_link = &compiled_lite.link;
    let sel_snippet_td = &compiled_lite.snippet_td;

    let mut results: Vec<SearchResult> = Vec::with_capacity(16);
    let mut position: u32 = 0;
    let mut pending_title: Option<(String, String)> = None;

    for tr in document.select(sel_tr()) {
        // Try the result link in the first <a> of the row (class result-link preferred).
        let link_candidate = tr.select(sel_link).next();
        if let Some(link) = link_candidate {
            let is_result_link = link
                .value()
                .attr("class")
                .map(|c| c.contains("result-link"))
                .unwrap_or(false);

            if is_result_link || pending_title.is_none() {
                if let Some(href) = link.value().attr("href") {
                    if let Some(resolved_url) = resolve_url(href) {
                        if resolved_url.contains("duckduckgo.com/y.js") {
                            continue;
                        }
                        let raw_title = join_text(&link);
                        let title = normalize_text(&raw_title, TITLE_LIMIT);
                        if !title.is_empty() && !resolved_url.contains("duckduckgo.com") {
                            // Flush any pending title without snippet.
                            if let Some((pending_t, pending_u)) = pending_title.take() {
                                position += 1;
                                results.push(SearchResult {
                                    position,
                                    title: pending_t,
                                    url: pending_u,
                                    display_url: None,
                                    snippet: None,
                                    original_title: None,
                                    content: None,
                                    content_size: None,
                                    content_extraction_method: None,
                                });
                            }
                            pending_title = Some((title, resolved_url));
                            continue;
                        }
                    }
                }
            }
        }

        // Snippet row: look for td.result-snippet or td with substantial text.
        if let Some((title, url)) = pending_title.take() {
            let snippet_text = tr
                .select(sel_snippet_td)
                .map(|td| join_text(&td))
                .find(|t| t.split_whitespace().count() > 5);
            let snippet = snippet_text.map(|t| normalize_text(&t, SNIPPET_LIMIT));

            position += 1;
            results.push(SearchResult {
                position,
                title,
                url,
                display_url: None,
                snippet,
                original_title: None,
                content: None,
                content_size: None,
                content_extraction_method: None,
            });
        }

        if results.len() >= 50 {
            break;
        }
    }

    // Final flush of any pending title.
    if let Some((title, url)) = pending_title {
        position += 1;
        results.push(SearchResult {
            position,
            title,
            url,
            display_url: None,
            snippet: None,
            original_title: None,
            content: None,
            content_size: None,
            content_extraction_method: None,
        });
    }

    results
}

fn extract_with_document(document: &Html, compiled: &CompiledSelectors) -> Vec<SearchResult> {
    let mut results = Vec::with_capacity(16);
    let mut position: u32 = 0;

    for result_element in document.select(&compiled.result_item) {
        // --- Ad filter by class (descendant or element itself) ---
        if let Some(ref ad_sel) = compiled.ad_class {
            if result_element.select(ad_sel).next().is_some()
                || contem_classe_anuncio_dinamico(&result_element, &compiled.ad_classes_raw)
            {
                tracing::trace!("Result filtered by ad class");
                continue;
            }
        }

        // --- Filter by attributes (configured key=value pairs) ---
        let mut filtered_by_attribute = false;
        for (key, value) in &compiled.ad_attributes {
            if result_element.value().attr(key.as_str()) == Some(value.as_str()) {
                tracing::trace!(attribute = %key, "Result filtered by ad attribute");
                filtered_by_attribute = true;
                break;
            }
        }
        if filtered_by_attribute {
            continue;
        }

        // --- Title + URL extraction ---
        let Some(ref title_selector) = compiled.title_sel else {
            continue;
        };
        let title_element = match result_element.select(title_selector).next() {
            Some(e) => e,
            None => {
                tracing::trace!("Result missing title element — skipping");
                continue;
            }
        };

        let raw_title = join_text(&title_element);
        let title = normalize_text(&raw_title, TITLE_LIMIT);
        if title.is_empty() {
            continue;
        }

        let raw_url = match title_element.value().attr("href") {
            Some(href) => href.to_string(),
            None => {
                tracing::trace!("Title missing href attribute — skipping");
                continue;
            }
        };
        let resolved_url = match resolve_url(&raw_url) {
            Some(u) => u,
            None => {
                tracing::trace!(url = %raw_url, "URL filtered or invalid");
                continue;
            }
        };
        // Filter by ad URL patterns (configurable).
        if compiled
            .url_patterns
            .iter()
            .any(|p| resolved_url.contains(p))
        {
            tracing::trace!(url = %resolved_url, "URL filtered by ad pattern");
            continue;
        }
        if resolved_url.len() > URL_LIMIT {
            tracing::trace!(size = resolved_url.len(), "URL exceeds limit — skipping");
            continue;
        }

        // --- Snippet extraction (optional) ---
        let snippet = compiled.snippet.as_ref().and_then(|sel| {
            result_element
                .select(sel)
                .next()
                .map(|el| normalize_text(&join_text(&el), SNIPPET_LIMIT))
                .filter(|s| !s.is_empty())
        });

        // --- Display URL extraction (optional) ---
        let display_url = compiled.display_url_sel.as_ref().and_then(|sel| {
            result_element
                .select(sel)
                .next()
                .map(|el| normalize_text(&join_text(&el), URL_LIMIT))
                .filter(|s| !s.is_empty())
        });

        // --- "Official site" heuristic (v0.3.0) ---
        // DDG renders the literal "Official site" as the title for verified domains
        // (e.g., wikipedia.org, rust-lang.org). We replace it with
        // `display_url` when available and preserve the literal in
        // `original_title` for auditing.
        let (final_title, original_title) =
            apply_official_site_heuristic(title, display_url.as_deref());

        position += 1;
        results.push(SearchResult {
            position,
            title: final_title,
            url: resolved_url,
            display_url,
            snippet,
            original_title,
            content: None,
            content_size: None,
            content_extraction_method: None,
        });
    }

    tracing::debug!(
        total = results.len(),
        "Extraction complete after ad filtering"
    );
    results
}

/// Dynamic version: accepts the list of ad classes configured in the TOML file.
fn contem_classe_anuncio_dinamico(element: &ElementRef<'_>, raw_classes: &[String]) -> bool {
    element
        .value()
        .classes()
        .any(|class| raw_classes.iter().any(|c| c == class))
}

/// Applies the "Official site" replacement heuristic (v0.3.0).
///
/// `DuckDuckGo` renders the literal text `"Official site"` (case-insensitive)
/// as the title when the result's domain is verified (e.g. rust-lang.org,
/// wikipedia.org). That title is not useful for API consumers — we replace it
/// with `url_exibicao` and preserve the literal in `original_title` for auditing.
///
/// Returns `(final_title, original_title)`:
/// - If the title matches exactly "Official site" (case-insensitive) AND a non-empty
///   `url_exibicao` exists, returns `(url_exibicao, Some("Official site"))`.
/// - Otherwise returns `(title, None)` unchanged.
fn apply_official_site_heuristic(
    title: String,
    display_url: Option<&str>,
) -> (String, Option<String>) {
    if title.eq_ignore_ascii_case("Official site") {
        if let Some(friendly_url) = display_url.map(str::trim).filter(|s| !s.is_empty()) {
            return (friendly_url.to_string(), Some(title));
        }
    }
    (title, None)
}

/// Normalises extracted text: collapses whitespace, trims and truncates at `limit` characters
/// respecting UTF-8 character boundaries.
fn normalize_text(raw: &str, limit: usize) -> String {
    let mut result_buf = String::with_capacity(raw.len().min(limit + 64));
    let mut needs_space = false;
    let mut chars_written: usize = 0;

    for word in raw.split_whitespace() {
        let separator = usize::from(needs_space);
        let word_len = word.chars().count();

        if chars_written + separator + word_len > limit {
            let remaining = limit.saturating_sub(chars_written + separator);
            if remaining > 0 {
                if needs_space {
                    result_buf.push(' ');
                }
                for ch in word.chars().take(remaining) {
                    result_buf.push(ch);
                }
            }
            break;
        }

        if needs_space {
            result_buf.push(' ');
            chars_written += 1;
        }
        result_buf.push_str(word);
        chars_written += word_len;
        needs_space = true;
    }

    result_buf
}

/// Resolves a URL found in the `DuckDuckGo` DOM to the final URL.
///
/// Handled cases:
/// 1. `//example.com/path` → `https://example.com/path` (protocol-relative).
/// 2. `/l/?uddg=<REAL_URL>&rut=...` → decodes `uddg` and returns the real URL.
/// 3. `//duckduckgo.com/l/?uddg=...` → same logic after normalisation.
/// 4. Absolute external URLs are returned as-is.
/// 5. URLs on the `duckduckgo.com` domain itself (except `/l/?uddg=`) are filtered.
///
/// Returns `None` if the URL is invalid or belongs to `DuckDuckGo`.
pub fn resolve_url(href: &str) -> Option<String> {
    let href_trim = href.trim();
    if href_trim.is_empty() {
        return None;
    }

    // Case 1: protocol-relative.
    let normalized = if let Some(rest) = href_trim.strip_prefix("//") {
        format!("https://{rest}")
    } else if href_trim.starts_with('/') {
        // Case 2: relative DuckDuckGo path (e.g., "/l/?uddg=...").
        format!("https://duckduckgo.com{href_trim}")
    } else {
        href_trim.to_string()
    };

    // Case 3: DuckDuckGo redirect with `uddg` parameter.
    if let Some(uddg_decoded) = extract_uddg(&normalized) {
        return Some(uddg_decoded);
    }

    // Case 4: filter URLs from DuckDuckGo itself (without uddg).
    if eh_url_duckduckgo(&normalized) {
        return None;
    }

    Some(normalized)
}

/// If the URL is a `DuckDuckGo` redirect (`/l/?uddg=<REAL_URL>`), extracts and
/// URL-decodes `uddg`. Returns `None` if it is not a redirect or the parameter is absent.
fn extract_uddg(url: &str) -> Option<String> {
    // Search for "uddg=" in the query string.
    let idx_uddg = url.find("uddg=")?;
    let after_equals = &url[idx_uddg + "uddg=".len()..];
    // The uddg value extends to the next `&` or end of string.
    let encoded_value = match after_equals.find('&') {
        Some(end) => &after_equals[..end],
        None => after_equals,
    };
    urlencoding::decode(encoded_value)
        .ok()
        .map(|cow| cow.into_owned())
}

/// Checks whether the URL points to any subdomain of `DuckDuckGo`.
fn eh_url_duckduckgo(url: &str) -> bool {
    let after_proto = if let Some(pos) = url.find("://") {
        &url[pos + 3..]
    } else {
        url
    };
    let host = after_proto
        .split('/')
        .next()
        .unwrap_or(after_proto)
        .split('?')
        .next()
        .unwrap_or(after_proto);
    host.eq_ignore_ascii_case("duckduckgo.com")
        || host.eq_ignore_ascii_case("html.duckduckgo.com")
        || host.eq_ignore_ascii_case("lite.duckduckgo.com")
        || (host.len() > ".duckduckgo.com".len()
            && host[host.len() - ".duckduckgo.com".len()..].eq_ignore_ascii_case(".duckduckgo.com"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolver_url_prefixa_protocol_relative() {
        assert_eq!(
            resolve_url("//exemplo.com/caminho"),
            Some("https://exemplo.com/caminho".to_string())
        );
    }

    #[test]
    fn resolver_url_desencapsula_redirect_uddg() {
        let href = "//duckduckgo.com/l/?uddg=https%3A%2F%2Fexemplo.com%2Fnoticia&rut=abc123";
        let resolvida = resolve_url(href).expect("should decode uddg");
        assert_eq!(resolvida, "https://exemplo.com/noticia");
    }

    #[test]
    fn resolve_url_unwraps_uddg_with_absolute_path() {
        let href = "/l/?uddg=https%3A%2F%2Fexemplo.com%2Farticle";
        let resolvida = resolve_url(href).expect("should decode uddg");
        assert_eq!(resolvida, "https://exemplo.com/article");
    }

    #[test]
    fn resolve_url_filters_duckduckgo_without_uddg() {
        assert_eq!(resolve_url("https://duckduckgo.com/settings"), None);
        assert_eq!(resolve_url("//html.duckduckgo.com/html/?q=teste"), None);
    }

    #[test]
    fn resolver_url_mantem_absolutas_externas() {
        assert_eq!(
            resolve_url("https://exemplo.com.br/noticia"),
            Some("https://exemplo.com.br/noticia".to_string())
        );
    }

    #[test]
    fn resolve_url_returns_none_for_empty_string() {
        assert_eq!(resolve_url(""), None);
        assert_eq!(resolve_url("   "), None);
    }

    #[test]
    fn normalize_text_colapsa_whitespace() {
        assert_eq!(
            normalize_text("  olá   mundo\n\n\ttexto  ", 100),
            "olá mundo texto"
        );
    }

    #[test]
    fn normalize_text_trunca_respeitando_char_boundary() {
        let long_text = "á".repeat(300);
        let truncated = normalize_text(&long_text, 200);
        assert_eq!(truncated.chars().count(), 200);
    }

    #[test]
    fn extract_results_works_with_minimal_html() {
        let html = r#"
            <html><body>
            <div id="links">
              <div class="result">
                <a class="result__a" href="//exemplo.com/pagina">Título Exemplo</a>
                <a class="result__snippet">Esta é uma descrição de exemplo.</a>
                <span class="result__url">exemplo.com</span>
              </div>
              <div class="result result--ad">
                <a class="result__a" href="//anuncio.com">Anúncio Pago</a>
              </div>
              <div class="result">
                <a class="result__a" href="//duckduckgo.com/l/?uddg=https%3A%2F%2Fwikipedia.org%2Fwiki%2FRust">Rust</a>
                <a class="result__snippet">Linguagem de programação Rust.</a>
              </div>
            </div>
            </body></html>
        "#;
        let results = extract_results(html);
        assert_eq!(results.len(), 2, "deve filtrar o anúncio");
        assert_eq!(results[0].position, 1);
        assert_eq!(results[0].title, "Título Exemplo");
        assert_eq!(results[0].url, "https://exemplo.com/pagina");
        assert_eq!(
            results[0].snippet.as_deref(),
            Some("Esta é uma descrição de exemplo.")
        );
        assert_eq!(results[1].position, 2);
        assert_eq!(results[1].title, "Rust");
        assert_eq!(results[1].url, "https://wikipedia.org/wiki/Rust");
    }

    #[test]
    fn extract_results_filters_js_urls() {
        let html = r#"
            <div id="links">
              <div class="result">
                <a class="result__a" href="//duckduckgo.com/y.js?ad=1">Tracker</a>
              </div>
              <div class="result">
                <a class="result__a" href="//site-valido.com/pagina">Válido</a>
              </div>
            </div>
        "#;
        let results = extract_results(html);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "Válido");
    }

    #[test]
    fn extract_results_respects_data_nrn_ad_attribute() {
        let html = r#"
            <div id="links">
              <div class="result" data-nrn="ad">
                <a class="result__a" href="//anuncio.com">Patrocinado</a>
              </div>
              <div class="result" data-nrn="organic">
                <a class="result__a" href="//organico.com">Orgânico</a>
              </div>
            </div>
        "#;
        let results = extract_results(html);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].url, "https://organico.com");
    }

    #[test]
    fn extract_results_empty_returns_empty_vec() {
        let html = "<html><body>Sem results</body></html>";
        let results = extract_results(html);
        assert!(results.is_empty());
    }

    #[test]
    fn strategy_2_recovers_when_classes_absent() {
        let html = r#"
            <html><body>
            <div id="links">
              <div>
                <a href="//exemplo.com/artigo">Título do Artigo de Exemplo</a>
                <p>Este é o snippet descritivo do artigo que precisa ter texto suficiente para ser considerado substancial e assim ser capturado como snippet pela heurística de extração.</p>
              </div>
              <div>
                <a href="//outro-site.com/noticia">Notícia Externa Importante</a>
                <p>Descrição relevante da notícia com mais de quarenta caracteres para garantir captura pela heurística de snippet.</p>
              </div>
            </div>
            </body></html>
        "#;
        let results = extract_results_with_strategies(html);
        assert!(
            results.len() >= 2,
            "Estratégia 2 deve recuperar pelo menos 2 results"
        );
        assert_eq!(results[0].title, "Título do Artigo de Exemplo");
        assert_eq!(results[0].url, "https://exemplo.com/artigo");
    }

    #[test]
    fn strategy_2_does_not_run_if_strategy_1_worked() {
        let html = r#"
            <html><body>
            <div id="links">
              <div class="result">
                <a class="result__a" href="//valido.com">Válido via Estratégia 1</a>
                <a class="result__snippet">Snippet curto.</a>
              </div>
            </div>
            </body></html>
        "#;
        let results = extract_results_with_strategies(html);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "Válido via Estratégia 1");
    }

    #[test]
    fn extract_results_lite_parses_duckduckgo_lite_table() {
        let html = r#"
            <html><body>
            <table>
              <tr>
                <td valign="top">1.&nbsp;</td>
                <td><a rel="nofollow" href="//exemplo.com/pagina1" class="result-link">Primeiro Resultado Lite</a></td>
              </tr>
              <tr>
                <td>&nbsp;</td>
                <td class="result-snippet">Esta é a descrição do primeiro resultado com texto suficiente para ser reconhecido.</td>
              </tr>
              <tr>
                <td valign="top">2.&nbsp;</td>
                <td><a rel="nofollow" href="//exemplo.com/pagina2" class="result-link">Segundo Resultado Lite</a></td>
              </tr>
              <tr>
                <td>&nbsp;</td>
                <td class="result-snippet">Descrição do segundo resultado com bastante texto também.</td>
              </tr>
            </table>
            </body></html>
        "#;
        let results = extract_results_lite(html);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].position, 1);
        assert_eq!(results[0].title, "Primeiro Resultado Lite");
        assert_eq!(results[0].url, "https://exemplo.com/pagina1");
        assert!(results[0].snippet.is_some());
        assert_eq!(results[1].title, "Segundo Resultado Lite");
    }

    #[test]
    fn extract_results_lite_empty_returns_empty_vec() {
        let html = "<html><body><p>Nada aqui</p></body></html>";
        let results = extract_results_lite(html);
        assert!(results.is_empty());
    }

    #[test]
    fn extract_results_with_custom_cfg_uses_alternate_selector() {
        // HTML sem `.result` original, mas com `.custom-result` — extrator default falharia.
        let html = r#"
            <div id="custom-links">
              <div class="custom-result">
                <a class="custom-title" href="//site.com/a">Título A</a>
                <span class="custom-snippet">Snippet A</span>
              </div>
              <div class="custom-result">
                <a class="custom-title" href="//site.com/b">Título B</a>
                <span class="custom-snippet">Snippet B</span>
              </div>
            </div>
        "#;

        // Default finds nothing.
        let padrao = extract_results(html);
        assert!(
            padrao.is_empty(),
            "default não deve casar com .custom-result"
        );

        // Config customizada deve funcionar.
        let mut cfg = SelectorConfig::default();
        cfg.html_endpoint.result_item = "#custom-links .custom-result".to_string();
        cfg.html_endpoint.title_and_url = ".custom-title".to_string();
        cfg.html_endpoint.snippet = ".custom-snippet".to_string();

        let results = extract_results_with_cfg(html, &cfg);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].title, "Título A");
        assert_eq!(results[1].title, "Título B");
    }

    #[test]
    fn extract_results_with_cfg_filters_custom_classes() {
        let html = r#"
            <div id="links">
              <div class="result organic">
                <a class="result__a" href="//a.com">Orgânico</a>
              </div>
              <div class="result my-custom-ad">
                <a class="result__a" href="//ad.com">Anúncio Custom</a>
              </div>
            </div>
        "#;

        let mut cfg = SelectorConfig::default();
        cfg.html_endpoint.ads_filter.ad_classes = vec![".my-custom-ad".to_string()];

        let results = extract_results_with_cfg(html, &cfg);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].url, "https://a.com");
    }

    #[test]
    fn extract_results_lite_filters_duckduckgo_links() {
        let html = r#"
            <table>
              <tr><td><a href="//duckduckgo.com/about" class="result-link">Sobre DDG</a></td></tr>
              <tr><td class="result-snippet">Snippet do DDG não deve aparecer.</td></tr>
              <tr><td><a href="//externo.com/doc" class="result-link">Doc Externa</a></td></tr>
              <tr><td class="result-snippet">Descrição da documentação externa relevante.</td></tr>
            </table>
        "#;
        let results = extract_results_lite(html);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].url, "https://externo.com/doc");
    }
}
