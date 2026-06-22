// SPDX-License-Identifier: MIT OR Apache-2.0
//! GAP-NEW-003 v0.8.0 — regression tests for the stealth shell classifier branch.
//!
//! The `classify_zero_result` function in `src/pipeline.rs` now has a `CR4b`
//! branch that detects when DDG returns a 14KB+ HTML shell (no `result__a`
//! markers, no interstitial markers, but contains DDG home page signature).
//! Without this branch, the classifier returns `Legitimo` for stealth
//! blocks, hiding the fact that the IP is being throttled.
use duckduckgo_search_cli::pipeline::{classify_zero_result, ZeroClassificationInputs};
use duckduckgo_search_cli::probe_deep::has_result_page_signal;

/// Builds a 14KB+ DDG home page stealth shell — no `result__a`, no interstitial
/// markers, but contains `search_form` and `DuckDuckGo` brand signature.
fn stealth_shell_body() -> String {
    let mut body = String::with_capacity(15_000);
    body.push_str("<!DOCTYPE html><html><head><title>DuckDuckGo</title></head>");
    body.push_str("<body><div id=\"header\"><form id=\"search_form\" action=\"/html/\">");
    body.push_str("<input type=\"text\" name=\"q\" autocomplete=\"off\">");
    body.push_str("<button type=\"submit\">S</button></form></div>");
    // Padding that mimics DDG home page content (categories, footer, etc.)
    let padding: String = std::iter::repeat_n("DuckDuckGo privacy search engine. ", 450).collect();
    body.push_str(&padding);
    body.push_str("</body></html>");
    body
}

#[test]
fn has_result_page_signal_returns_false_for_stealth_shell() {
    let body = stealth_shell_body();
    assert!(
        !has_result_page_signal(&body),
        "stealth shell has no result__a selectors"
    );
}

#[test]
fn classify_stealth_shell_14kb_returns_ghost_block() {
    let body = stealth_shell_body();
    assert!(
        body.len() >= 14_000,
        "fixture must be 14KB+ for the stealth shell branch"
    );
    let inputs = ZeroClassificationInputs {
        body: &body,
        pre_flight_enabled: false,
        pre_flight_fired: false,
        execution_time_ms: 500,
        retries: 0,
        concurrent_fetches: 0,
        last_probe_cascade_level: None,
    };
    let cause = classify_zero_result(&inputs);
    assert_eq!(
        cause,
        duckduckgo_search_cli::types::ZeroCause::GhostBlock,
        "14KB DDG home page shell must classify as GhostBlock (stealth block), not Legitimo"
    );
}

#[test]
fn classify_stealth_shell_with_anomaly_modal_still_anti_bot() {
    let mut body = stealth_shell_body();
    body.push_str("<div class=\"anomaly-modal__mask\"></div>");
    let inputs = ZeroClassificationInputs {
        body: &body,
        pre_flight_enabled: false,
        pre_flight_fired: false,
        execution_time_ms: 500,
        retries: 0,
        concurrent_fetches: 0,
        last_probe_cascade_level: None,
    };
    let cause = classify_zero_result(&inputs);
    assert_eq!(
        cause,
        duckduckgo_search_cli::types::ZeroCause::AntiBot,
        "anomaly-modal must still trigger AntiBot (not stealth shell branch)"
    );
}

#[test]
fn classify_legit_short_query_with_signal_returns_legitimo() {
    let body = "<a class=\"result__a\" href=\"https://example.com\">x</a>";
    let inputs = ZeroClassificationInputs {
        body,
        pre_flight_enabled: false,
        pre_flight_fired: false,
        execution_time_ms: 500,
        retries: 0,
        concurrent_fetches: 0,
        last_probe_cascade_level: None,
    };
    assert_eq!(
        classify_zero_result(&inputs),
        duckduckgo_search_cli::types::ZeroCause::Legitimo
    );
}
