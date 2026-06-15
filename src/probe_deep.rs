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
];

/// Detects whether the given HTML body is a bot-detection interstitial
/// or a real search result page.
///
/// The detection is order-sensitive: Cloudflare markers are checked
/// first because the DDG interstitial sometimes embeds Cloudflare
/// fragments. A single match in either list triggers the corresponding
/// classification. An empty body or a body that matches no marker is
/// classified as `None` (treated as a normal result page, even if
/// `quantidade_resultados` is 0).
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
    InterstitialKind::None
}

/// Returns a human-readable suggestion for the operator when an
/// interstitial is detected. The message is informational only — the
/// fallback decision is the caller's responsibility.
pub fn sugestao_mitigacao(kind: InterstitialKind) -> &'static str {
    match kind {
        InterstitialKind::None => "no interstitial detected",
        InterstitialKind::Cloudflare => {
            "Cloudflare challenge detected. Re-run with --allow-lite-fallback \
             to use the lite endpoint, or wait a few minutes and retry."
        }
        InterstitialKind::DuckDuckGo => {
            "DuckDuckGo bot detection triggered. Re-run with \
             --allow-lite-fallback to use the lite endpoint, or wait a few \
             minutes and retry."
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

    #[test]
    fn sugestao_is_informative() {
        assert!(sugestao_mitigacao(InterstitialKind::Cloudflare).contains("Cloudflare"));
        assert!(sugestao_mitigacao(InterstitialKind::DuckDuckGo).contains("DuckDuckGo"));
        assert_eq!(
            sugestao_mitigacao(InterstitialKind::None),
            "no interstitial detected"
        );
    }
}
