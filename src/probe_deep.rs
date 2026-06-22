// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload: pure (HTML string classification, no I/O)
//! v0.7.3 PR3 — CAPTCHA interstitial detection for the `html` endpoint.
//!
//! The `DuckDuckGo` HTML endpoint sometimes returns a Cloudflare or DDG
//! bot-detection interstitial instead of real results, even when the
//! HTTP status is 200 (so the existing `html -> lite` cascade does not
//! trigger). This module classifies the response body so the pipeline
//! can detect the interstitial and fall back to the `lite` endpoint
//! when appropriate.
//!
//! ## Detection strategy
//!
//! The detection is string-based: we look for known interstitial markers
//! in the raw HTML. This is intentionally simple — we do not parse the
//! DOM, we do not execute JavaScript, and we do not load the page in a
//! headless browser. False positives are acceptable as long as the
//! fallback is opt-in (the user must pass `--allow-lite-fallback` to
//! actually trigger it).

/// Classification of a `DuckDuckGo` HTML response body.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InterstitialKind {
    /// No interstitial detected — the body contains search results.
    None,
    /// Cloudflare bot-management challenge page (HTTP 200 but no
    /// results). Markers: `cf-chl-bypass`, `cf-challenge`,
    /// `challenge-platform`, `Attention Required`, `__cf_chl_jschl_tk__`.
    Cloudflare,
    /// `DuckDuckGo` in-house bot detection (`robot-detected`,
    /// "bots, we have detected..."). Markers: `robot-detected`,
    /// `bots, we have detected`.
    DuckDuckGo,
}

impl InterstitialKind {
    /// Short string label used in JSON metadata and log lines.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Cloudflare => "cloudflare",
            Self::DuckDuckGo => "duckduckgo",
        }
    }
}

/// String markers that indicate a Cloudflare interstitial.
///
/// Includes both legacy (pre-2026) and post-2026 markers. Cloudflare
/// rolled out Turnstile, the `cf-spinner` placeholder and the
/// `Just a moment` interstitial while `cf-mitigated` appears in
/// post-mitigation pages. We match on any of these to be safe.
const CLOUDFLARE_MARKERS: &[&str] = &[
    "cf-chl-bypass",
    "cf-challenge",
    "challenge-platform",
    "Attention Required",
    "__cf_chl_jschl_tk__",
    "anomaly-modal",
    "anomaly-modal__mask",
    "anomaly-modal__title",
    "anomaly.js?cc=botnet",
    "cf-turnstile",
    "cf-spinner",
    "Just a moment",
    "cf-mitigated",
    // v0.7.9 GAP-WS-59: 2026 markers backported.
    "anomaly.js",
    "botnet",
    "cf-error-code",
    "cf-ray",
    "Performance & Security by Cloudflare",
];

/// String markers that indicate a `DuckDuckGo` bot-detection interstitial.
///
/// The pre-2026 `robot-detected` template is still observed in some
/// locales, while the post-2026 anomaly-modal copy replaces the older
/// "bots, we have detected" sentence on the main path.
const DDG_MARKERS: &[&str] = &[
    "robot-detected",
    "bots, we have detected",
    "Unfortunately, bots use DuckDuckGo too.",
    // v0.7.9 GAP-WS-59: prefix-only match for truncated DDG responses.
    "Unfortunately, bots",
];

/// Selectors that indicate a real DDG result page. Used to distinguish
/// a legitimately short results page from a Cloudflare ghost-block
/// (HTTP 200, sub-4KB body, no markers). v0.7.9 GAP-WS-58 + v0.7.10 P12.
pub const RESULT_PAGE_SELECTORS: &[&str] = &[
    "result__a",
    "class=\"result\"",
    "class='result'",
    "result-link",
    "result__title",
    "result__snippet",
    "nrn-react-div",
    "data-testid=\"result\"",
    // v0.7.10 P12: alternative DDG template classes (2026 rotation).
    "react-article",
    "module--results",
    "js-react-aria-results",
];

/// Returns `true` when the HTML body contains at least one DDG
/// result-page selector. Used by `detectar_interstitial` to avoid
/// false ghost-block classifications on legitimately short results
/// pages. v0.7.9 GAP-WS-58.
pub fn has_result_page_signal(html: &str) -> bool {
    RESULT_PAGE_SELECTORS.iter().any(|s| html.contains(s))
}

/// Detects whether the given HTML body is a bot-detection interstitial
/// or a real search result page.
///
/// The detection is order-sensitive: Cloudflare markers are checked
/// first because the DDG interstitial sometimes embeds Cloudflare
/// fragments. A single match in either list triggers the corresponding
/// classification. An empty body is classified as `None` (transport
/// error, not a block). A body shorter than 4KB with no result-page
/// selector is classified as `Cloudflare` (ghost-block) — v0.7.9
/// GAP-WS-58.
pub fn detectar_interstitial(html: &str) -> InterstitialKind {
    if html.is_empty() {
        return InterstitialKind::None;
    }
    for marker in CLOUDFLARE_MARKERS {
        if html.contains(marker) {
            return InterstitialKind::Cloudflare;
        }
    }
    for marker in DDG_MARKERS {
        if html.contains(marker) {
            return InterstitialKind::DuckDuckGo;
        }
    }
    // v0.7.9 GAP-WS-58: ghost-block — short body with no result-page signal.
    // Cloudflare serves HTTP 200 with a sub-4KB body and no markers in
    // the 2026 ghost-block pattern. Without this branch, the search
    // path treats the response as a legitimate empty results page and
    // exits with code 0.
    const GHOST_BLOCK_THRESHOLD: usize = 4_000;
    if html.len() < GHOST_BLOCK_THRESHOLD && !has_result_page_signal(html) {
        return InterstitialKind::Cloudflare;
    }
    InterstitialKind::None
}

/// Sentinel marker returned when a detection is triggered by the size
/// heuristic (ghost-block) rather than by a literal Cloudflare marker.
/// Wrapped in `<>` so callers can detect it via `starts_with('<')` and
/// omit it from user-facing marker lists. v0.7.10.
pub const GHOST_BLOCK_SENTINEL: &str = "<ghost-block-no-marker>";

/// Sentinel marker returned when the body is empty (transport error,
/// not a Cloudflare/DDG interstitial). v0.7.10.
pub const EMPTY_BODY_SENTINEL: &str = "<empty-body>";

/// Sentinel marker returned when the body is classified as `None`
/// (legitimate response). v0.7.10.
pub const NO_MARKER_SENTINEL: &str = "<no-marker>";

/// Like [`detectar_interstitial`], but also returns the matched marker.
///
/// Returns `(marker, InterstitialKind)`:
/// - For marker-based detection, `marker` is the literal string from
///   `CLOUDFLARE_MARKERS` or `DDG_MARKERS` that matched.
/// - For ghost-block detection, `marker` is [`GHOST_BLOCK_SENTINEL`].
/// - For empty body, `marker` is [`EMPTY_BODY_SENTINEL`].
/// - For `None` (no interstitial), `marker` is [`NO_MARKER_SENTINEL`].
///
/// Callers can detect sentinels via `marker.starts_with('<')` and
/// suppress them from user-facing output. v0.7.10.
pub fn detectar_interstitial_com_match(html: &str) -> (&'static str, InterstitialKind) {
    if html.is_empty() {
        return (EMPTY_BODY_SENTINEL, InterstitialKind::None);
    }
    // GAP-NEW-005 v0.8.0: se o body contém sinal positivo de result page
    // (qualquer `RESULT_PAGE_SELECTORS`), os markers de interstitial só
    // contam se aparecerem em contexto de DOM, não em URLs/asset paths
    // embutidos no HTML legítimo. Resultado: o probe para de reportar
    // `anomaly-modal` quando o DDG serve a SERP completa mas a string
    // aparece em `src=".../anomaly.js"` ou `<a href="...anomaly-modal...">`.
    // Audit E2E 2026-06-19 confirmou que DDG serve a SERP normal (5+
    // resultados reais via Firefox) mas a string "anomaly-modal" também
    // aparece em referências de assets/scripts.
    let tem_resultado_real = has_result_page_signal(html);
    if !tem_resultado_real {
        for marker in CLOUDFLARE_MARKERS {
            if html.contains(marker) {
                return (marker, InterstitialKind::Cloudflare);
            }
        }
        for marker in DDG_MARKERS {
            if html.contains(marker) {
                return (marker, InterstitialKind::DuckDuckGo);
            }
        }
    }
    const GHOST_BLOCK_THRESHOLD: usize = 4_000;
    if html.len() < GHOST_BLOCK_THRESHOLD && !has_result_page_signal(html) {
        return (GHOST_BLOCK_SENTINEL, InterstitialKind::Cloudflare);
    }
    (NO_MARKER_SENTINEL, InterstitialKind::None)
}

/// Returns a human-readable suggestion for the operator when an
/// interstitial is detected. The message is informational only — the
/// fallback decision is the caller's responsibility.
///
/// **Deprecated since v0.7.10**: use [`sugestao_mitigacao_com_marker`]
/// for messages that include the specific matched marker (e.g.
/// `cf-challenge`, `robot-detected`). This function remains available
/// for BC but will be removed in v0.8.0.
#[deprecated(
    since = "0.7.10",
    note = "Use sugestao_mitigacao_com_marker for marker-specific messages. This function remains for BC."
)]
pub fn sugestao_mitigacao(kind: InterstitialKind) -> &'static str {
    match kind {
        InterstitialKind::None => "no interstitial detected",
        InterstitialKind::Cloudflare => {
            // v0.7.9 GAP-WS-58 UX: include the new --pre-flight flag and the
            // canonical 2026 marker set so the operator can act on the hint
            // without consulting the docs.
            "Cloudflare challenge detected (markers: cf-challenge, anomaly-modal, \
             cf-turnstile, etc.). Re-run with --pre-flight to enable automatic \
             Lite fallback, or with --allow-lite-fallback for explicit opt-in."
        }
        InterstitialKind::DuckDuckGo => {
            "DuckDuckGo bot detection triggered. Re-run with \
             --allow-lite-fallback to use the lite endpoint, or wait a few \
             minutes and retry."
        }
    }
}

/// Returns a human-readable suggestion for the operator when an
/// interstitial is detected, including the matched marker. The message
/// is informational only — the fallback decision is the caller's
/// responsibility. v0.7.10.
///
/// When `marker` is a sentinel (starts with `<`, e.g. `<ghost-block-no-marker>`
/// or `<empty-body>`), the suggestion omits the marker name and
/// describes the heuristic that triggered the classification instead.
pub fn sugestao_mitigacao_com_marker(kind: InterstitialKind, marker: &str) -> String {
    match kind {
        InterstitialKind::None => "no interstitial detected".to_string(),
        InterstitialKind::Cloudflare => {
            if marker.starts_with('<') {
                "Cloudflare ghost-block detected (heuristic: short body with no \
                 result-page signal). Re-run with --pre-flight to enable automatic \
                 Lite fallback, or with --allow-lite-fallback for explicit opt-in."
                    .to_string()
            } else {
                format!(
                    "Cloudflare challenge detected (marker: {marker}). Re-run with \
                     --pre-flight to enable automatic Lite fallback, or with \
                     --allow-lite-fallback for explicit opt-in."
                )
            }
        }
        InterstitialKind::DuckDuckGo => {
            if marker.starts_with('<') {
                "DuckDuckGo bot detection triggered. Re-run with \
                 --allow-lite-fallback to use the lite endpoint, or wait a few \
                 minutes and retry."
                    .to_string()
            } else {
                format!(
                    "DuckDuckGo bot detection triggered (marker: {marker}). Re-run \
                     with --allow-lite-fallback to use the lite endpoint, or wait a \
                     few minutes and retry."
                )
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_body_is_none() {
        assert_eq!(detectar_interstitial(""), InterstitialKind::None);
    }

    // v0.7.9 GAP-WS-58: ghost-block — a sub-4KB body with no result-page
    // signal is classified as Cloudflare. Companion to `empty_body_is_none`
    // which remains the transport-error contract for `""`.
    #[test]
    fn ghost_block_short_no_signal_is_cloudflare() {
        // 2KB of pure lorem ipsum — too short to be a real results page,
        // contains no `result__a`/`class="result"`/etc.
        let mut html = String::with_capacity(2_048);
        while html.len() < 2_048 {
            html.push_str("lorem ipsum dolor sit amet ");
        }
        assert_eq!(detectar_interstitial(&html), InterstitialKind::Cloudflare);
    }

    #[test]
    fn normal_results_are_none() {
        let html = r#"
            <html><body>
            <div class="result">
                <a class="result__a" href="https://example.com">Example</a>
                <a class="result__snippet">A snippet</a>
            </div>
            </body></html>
        "#;
        assert_eq!(detectar_interstitial(html), InterstitialKind::None);
    }

    #[test]
    fn cloudflare_challenge_detected() {
        let html = r#"<html><body>
            <div id="cf-chl-bypass">
                <form action="/challenge">
                    <input name="__cf_chl_jschl_tk__" value="...">
                </form>
            </div>
        </body></html>"#;
        assert_eq!(detectar_interstitial(html), InterstitialKind::Cloudflare);
    }

    #[test]
    fn cloudflare_attention_required_detected() {
        let html = "<html><body><h1>Attention Required! | Cloudflare</h1></body></html>";
        assert_eq!(detectar_interstitial(html), InterstitialKind::Cloudflare);
    }

    #[test]
    fn duckduckgo_bots_detected() {
        let html = "<html><body>Sorry, bots, we have detected unusual activity from your network.</body></html>";
        assert_eq!(detectar_interstitial(html), InterstitialKind::DuckDuckGo);
    }

    #[test]
    fn duckduckgo_robot_detected() {
        let html = "<html><body>robot-detected from your network</body></html>";
        assert_eq!(detectar_interstitial(html), InterstitialKind::DuckDuckGo);
    }

    #[test]
    fn cloudflare_takes_precedence_over_ddg() {
        let html = "<html><body>cf-challenge robot-detected</body></html>";
        assert_eq!(detectar_interstitial(html), InterstitialKind::Cloudflare);
    }

    // v0.7.10 P6/P17: snapshot test capturing the full enumeration of
    // Cloudflare 2026 markers + the matched marker from `com_match`.
    // On first run, insta writes `tests/snapshots/*.snap`. Subsequent
    // runs compare against the committed snapshot — drift indicates
    // someone removed a marker string by accident.
    #[test]
    fn cloudflare_markers_snapshot_v0_7_10() {
        let fixtures = [
            (
                "anomaly.js",
                "<script src=\"/.well-known/anomaly.js\"></script>",
            ),
            ("botnet", "<div id=\"botnet-banner\">blocked</div>"),
            (
                "cf-error-code",
                "<h1 data-cf-error-code=\"1020\">Access Denied</h1>",
            ),
            ("cf-ray", "cf-ray: 8a1b2c3d4e5f6789-EWR"),
            (
                "Performance & Security by Cloudflare",
                "<footer>Performance & Security by Cloudflare</footer>",
            ),
            (
                "cf-turnstile",
                "<div class=\"cf-turnstile\" data-sitekey=\"0x4AAA\"></div>",
            ),
            ("cf-spinner", "<div class=\"cf-spinner\"></div>"),
            ("cf-mitigated", "<div id=\"cf-mitigated\">success</div>"),
        ];
        for (expected_marker, html) in &fixtures {
            let (marker, kind) = detectar_interstitial_com_match(html);
            assert_eq!(marker, *expected_marker, "fixture: {html}");
            assert_eq!(kind, InterstitialKind::Cloudflare);
        }

        // Aggregate snapshot for regression coverage. Insta produces
        // a textual diff when the set of markers changes.
        let markers: Vec<&str> = CLOUDFLARE_MARKERS.to_vec();
        insta::assert_snapshot!("cloudflare_markers_v0_7_10", format!("{markers:#?}"));
    }

    #[test]
    fn as_str_matches_variant() {
        assert_eq!(InterstitialKind::None.as_str(), "none");
        assert_eq!(InterstitialKind::Cloudflare.as_str(), "cloudflare");
        assert_eq!(InterstitialKind::DuckDuckGo.as_str(), "duckduckgo");
    }

    #[test]
    fn cloudflare_anomaly_modal_detected() {
        let html = "<html><body><div class=\"anomaly-modal__mask\"></div></body></html>";
        assert_eq!(detectar_interstitial(html), InterstitialKind::Cloudflare);
    }

    #[test]
    fn cloudflare_anomaly_modal_title_detected() {
        let html =
            "<html><body><h1 class=\"anomaly-modal__title\">Verify you are human</h1></body></html>";
        assert_eq!(detectar_interstitial(html), InterstitialKind::Cloudflare);
    }

    #[test]
    fn cloudflare_anomaly_js_botnet_detected() {
        let html =
            "<html><body><script src=\"/.well-known/anomaly.js?cc=botnet\"></script></body></html>";
        assert_eq!(detectar_interstitial(html), InterstitialKind::Cloudflare);
    }

    // v0.7.9 GAP-WS-59: backport of 5 Cloudflare markers missing in v0.7.8.
    #[test]
    fn cloudflare_2026_markers_detected() {
        let fixtures = [
            "<html><body><script src=\"/.well-known/anomaly.js\"></script></body></html>",
            "<html><body><div id=\"botnet-banner\">blocked</div></body></html>",
            "<html><body><h1 data-cf-error-code=\"1020\">Access Denied</h1></body></html>",
            "<html><body>cf-ray: 8a1b2c3d4e5f6789-EWR</body></html>",
            "<html><body><footer>Performance & Security by Cloudflare</footer></body></html>",
        ];
        for html in &fixtures {
            assert_eq!(
                detectar_interstitial(html),
                InterstitialKind::Cloudflare,
                "expected Cloudflare for fixture: {html}"
            );
        }
    }

    #[test]
    fn cloudflare_turnstile_detected() {
        let html =
            "<html><body><div class=\"cf-turnstile\" data-sitekey=\"0x4AAA\"></div></body></html>";
        assert_eq!(detectar_interstitial(html), InterstitialKind::Cloudflare);
    }

    #[test]
    fn cloudflare_spinner_detected() {
        let html = "<html><body><div class=\"cf-spinner\"></div></body></html>";
        assert_eq!(detectar_interstitial(html), InterstitialKind::Cloudflare);
    }

    #[test]
    fn cloudflare_just_a_moment_detected() {
        let html = "<html><body><h1>Just a moment...</h1></body></html>";
        assert_eq!(detectar_interstitial(html), InterstitialKind::Cloudflare);
    }

    #[test]
    fn cloudflare_mitigated_detected() {
        let html = "<html><body><div id=\"cf-mitigated\">success</div></body></html>";
        assert_eq!(detectar_interstitial(html), InterstitialKind::Cloudflare);
    }

    #[test]
    fn duckduckgo_unfortunately_bots_detected() {
        let html =
            "<html><body>Unfortunately, bots use DuckDuckGo too. Please complete the challenge below.</body></html>";
        assert_eq!(detectar_interstitial(html), InterstitialKind::DuckDuckGo);
    }

    // v0.7.9 GAP-WS-59: prefix-only match for truncated DDG responses.
    #[test]
    fn duckduckgo_unfortunately_bots_prefix_detected() {
        let html = "<html><body>Unfortunately, bots.</body></html>";
        assert_eq!(detectar_interstitial(html), InterstitialKind::DuckDuckGo);
    }

    #[test]
    #[allow(deprecated)]
    fn sugestao_is_informative() {
        assert!(sugestao_mitigacao(InterstitialKind::Cloudflare).contains("Cloudflare"));
        assert!(sugestao_mitigacao(InterstitialKind::DuckDuckGo).contains("DuckDuckGo"));
        assert_eq!(
            sugestao_mitigacao(InterstitialKind::None),
            "no interstitial detected"
        );
    }

    // v0.7.9 GAP-WS-58: has_result_page_signal is the BC guard that
    // prevents `detectar_interstitial` from classifying a legitimately
    // short results page as a ghost-block.
    #[test]
    fn result_page_signal_recognizes_legit_short() {
        let legit = "<a class=\"result__a\" href=\"https://example.com\">x</a>";
        assert!(has_result_page_signal(legit), "result__a must be detected");
        let no_signal = "lorem ipsum dolor sit amet, consectetur adipiscing elit";
        assert!(
            !has_result_page_signal(no_signal),
            "pure text without selectors must NOT be detected"
        );
    }

    // v0.7.9 GAP-WS-58 UX: the Cloudflare suggestion must mention the new
    // --pre-flight flag so the operator can act on the hint without
    // consulting the docs.
    #[test]
    #[allow(deprecated)]
    fn sugestao_cloudflare_cita_pre_flight() {
        let msg = sugestao_mitigacao(InterstitialKind::Cloudflare);
        assert!(msg.contains("--pre-flight"));
        assert!(msg.contains("--allow-lite-fallback"));
    }

    // v0.7.10 P1 #1: detectar_interstitial_com_match returns Cloudflare marker
    // when a literal Cloudflare marker matches.
    #[test]
    fn detectar_interstitial_com_match_returns_cloudflare_marker_on_challenge() {
        let html = r#"<html><body><div id="cf-challenge">x</div></body></html>"#;
        let (marker, kind) = detectar_interstitial_com_match(html);
        assert_eq!(marker, "cf-challenge");
        assert_eq!(kind, InterstitialKind::Cloudflare);
    }

    // v0.7.10 P1 #2: detectar_interstitial_com_match returns DDG marker.
    #[test]
    fn detectar_interstitial_com_match_returns_ddg_marker_on_anomaly() {
        let html = "<html><body>robot-detected from your network</body></html>";
        let (marker, kind) = detectar_interstitial_com_match(html);
        assert_eq!(marker, "robot-detected");
        assert_eq!(kind, InterstitialKind::DuckDuckGo);
    }

    // v0.7.10 P1 #3: detectar_interstitial_com_match returns ghost-block sentinel.
    #[test]
    fn detectar_interstitial_com_match_returns_ghost_block_sentinel_on_short_body() {
        let mut html = String::with_capacity(2_048);
        while html.len() < 2_048 {
            html.push_str("lorem ipsum dolor sit amet ");
        }
        let (marker, kind) = detectar_interstitial_com_match(&html);
        assert_eq!(marker, GHOST_BLOCK_SENTINEL);
        assert_eq!(kind, InterstitialKind::Cloudflare);
    }

    // v0.7.10 P1 #4: detectar_interstitial_com_match returns empty-body sentinel.
    #[test]
    fn detectar_interstitial_com_match_returns_none_marker_on_empty_body() {
        let (marker, kind) = detectar_interstitial_com_match("");
        assert_eq!(marker, EMPTY_BODY_SENTINEL);
        assert_eq!(kind, InterstitialKind::None);
    }

    // v0.7.10 P12: classes DDG alternativas (rotação de templates 2026).
    #[test]
    fn ddg_template_rotation_fixtures() {
        let fixtures = [
            "nrn-react-div",
            "react-article",
            "module--results",
            "js-react-aria-results",
        ];
        for selector in &fixtures {
            let html = format!(
                r#"<html><body><div class="{selector}"><a href="/x">link</a></div></body></html>"#
            );
            assert!(
                has_result_page_signal(&html),
                "fixture class {selector} must be detected as result-page signal"
            );
        }
    }

    // v0.7.10 P2 #5: sugestao_mitigacao_com_marker includes Cloudflare marker.
    #[test]
    #[allow(deprecated)]
    fn sugestao_mitigacao_com_marker_cloudflare_includes_marker_name() {
        let msg = sugestao_mitigacao_com_marker(InterstitialKind::Cloudflare, "cf-challenge");
        assert!(msg.contains("cf-challenge"), "msg = {msg}");
        assert!(msg.contains("--pre-flight"));
        assert!(msg.contains("--allow-lite-fallback"));
    }

    // v0.7.10 P2 #6: sugestao_mitigacao_com_marker includes DDG marker.
    #[test]
    fn sugestao_mitigacao_com_marker_ddg_includes_marker_name() {
        let msg = sugestao_mitigacao_com_marker(InterstitialKind::DuckDuckGo, "robot-detected");
        assert!(msg.contains("robot-detected"), "msg = {msg}");
    }

    // v0.7.10 P11: ghost-block sentinel produces heuristic-aware message.
    #[test]
    fn sugestao_mitigacao_com_marker_handles_ghost_block_sentinel() {
        let msg = sugestao_mitigacao_com_marker(InterstitialKind::Cloudflare, GHOST_BLOCK_SENTINEL);
        assert!(msg.contains("ghost-block"), "msg = {msg}");
        assert!(!msg.contains("<"), "sentinel must not leak to user");
    }
}

/// Outcome of a single probe-deep health check, used by the v0.7.10 P5
/// probe-deep scheduler to decide whether to short-circuit `execute_pipeline`.
#[derive(Debug, Clone)]
pub struct ProbeOutcome {
    /// `true` when the endpoint is reachable AND no interstitial was
    /// detected. `false` when captcha/ghost-block tripped the detector.
    pub healthy: bool,
    /// Matched marker (or sentinel) returned by `detectar_interstitial_com_match`.
    pub marker: &'static str,
    /// Classified `InterstitialKind`.
    pub kind: InterstitialKind,
    /// HTTP status code returned by the endpoint.
    pub http_status: u16,
    /// Wall-clock latency in milliseconds.
    pub latency_ms: u64,
}

/// Lightweight, pure-Rust health check used by the P5 scheduler. Given
/// the response body and HTTP status, returns a `ProbeOutcome`. This
/// function does NOT perform network I/O — the caller fetches the
/// response via `reqwest::Client`, then passes the body here.
///
/// `expected_marker_hint` is the marker the scheduler expects to see
/// (informational only, for log correlation).
pub fn classify_probe_outcome(body: &str, http_status: u16, latency_ms: u64) -> ProbeOutcome {
    let (marker, kind) = detectar_interstitial_com_match(body);
    let healthy = matches!(kind, InterstitialKind::None) && http_status < 400;
    ProbeOutcome {
        healthy,
        marker,
        kind,
        http_status,
        latency_ms,
    }
}

#[cfg(test)]
mod tests_probe_outcome {
    use super::*;

    #[test]
    fn probe_outcome_classifies_clean_response_as_healthy() {
        let body = r#"<html><body><div class="result__a">x</div></body></html>"#;
        let outcome = classify_probe_outcome(body, 200, 150);
        assert!(outcome.healthy);
        assert_eq!(outcome.kind, InterstitialKind::None);
        assert_eq!(outcome.marker, NO_MARKER_SENTINEL);
        assert_eq!(outcome.http_status, 200);
        assert_eq!(outcome.latency_ms, 150);
    }

    #[test]
    fn probe_outcome_classifies_captcha_as_unhealthy() {
        let body = "<html><body>cf-challenge</body></html>";
        let outcome = classify_probe_outcome(body, 403, 230);
        assert!(!outcome.healthy);
        assert_eq!(outcome.kind, InterstitialKind::Cloudflare);
        assert_eq!(outcome.marker, "cf-challenge");
    }

    #[test]
    fn probe_outcome_classifies_ghost_block_as_unhealthy() {
        let mut body = String::with_capacity(2_048);
        while body.len() < 2_048 {
            body.push_str("lorem ipsum dolor sit amet ");
        }
        let outcome = classify_probe_outcome(&body, 200, 90);
        assert!(!outcome.healthy);
        assert_eq!(outcome.kind, InterstitialKind::Cloudflare);
        assert_eq!(outcome.marker, GHOST_BLOCK_SENTINEL);
    }

    #[test]
    fn probe_outcome_classifies_http_500_as_unhealthy() {
        // Long-enough body with a result-page signal so the detector
        // classifies it as `None` — only then the 500 status flips
        // healthy to false.
        let body = r#"<html><body><div class="result__a">link</div><div class="result__snippet">a</div></div></body></html>"#;
        let outcome = classify_probe_outcome(body, 500, 100);
        assert!(!outcome.healthy);
        assert_eq!(outcome.kind, InterstitialKind::None);
        assert_eq!(outcome.http_status, 500);
    }
}
