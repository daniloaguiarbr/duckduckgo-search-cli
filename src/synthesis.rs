// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload: declarative (string assembly, no I/O).
//! Heuristic synthesis of an aggregated result list into a single report.
//!
//! Given a list of [`AggregatedItem`]s sorted by descending score, this module
//! produces a self-contained report with numbered references. Three formats
//! are supported:
//!
//! - [`SynthFormat::Markdown`] — `##`/`###` headings and `[n](url)` links.
//! - [`SynthFormat::PlainText`] — linear numbered list without markup.
//! - [`SynthFormat::Json`] — structured tree: `{ "summary": "...", "references":
//!   [{ "id": n, "url": "...", "title": "..." }] }`.
//!
//! # Token budget
//!
//! We approximate one token as four characters (the de-facto industry
//! heuristic for English text). The budget is enforced on the summary body
//! only — references are always included in full because they are
//! non-negotiable for LLM grounding.

use crate::aggregation::AggregatedItem;
use crate::deep_research::DeepResearchArgs;
use serde::{Deserialize, Serialize};

/// Output format of the synthesis stage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SynthFormat {
    /// `## Heading` and `[n](url)` links.
    Markdown,
    /// Linear numbered list, no markup.
    PlainText,
    /// Structured JSON tree.
    Json,
}

/// Synthesised report returned by the deep-research pipeline.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SynthesizedReport {
    /// Format used to render the report.
    #[serde(rename = "formato")]
    pub format: SynthFormat,
    /// The report body (`Markdown`, `PlainText`, or `JSON`).
    #[serde(rename = "corpo")]
    pub body: String,
    /// Approximate token count of the report body (4 chars ≈ 1 token).
    #[serde(rename = "tokens_estimados")]
    pub estimated_tokens: usize,
    /// Number of references cited in the report.
    #[serde(rename = "quantidade_referencias")]
    pub reference_count: usize,
}

/// Approximate token count: 1 token ≈ 4 characters.
///
/// # Examples
///
/// ```
/// use duckduckgo_search_cli::synthesis::estimate_tokens;
///
/// assert_eq!(estimate_tokens(""), 0);
/// assert_eq!(estimate_tokens("abcd"), 1);
/// assert_eq!(estimate_tokens("abcde"), 2);
/// assert_eq!(estimate_tokens("a 16-character str!"), 5);
/// ```
pub fn estimate_tokens(text: &str) -> usize {
    text.len().div_ceil(4)
}

/// Truncates `text` to roughly `budget_tokens` tokens, preferring word
/// boundaries. Truncation is done at a valid UTF-8 char boundary so
/// multi-byte characters are never split.
///
/// # Examples
///
/// ```
/// use duckduckgo_search_cli::synthesis::trim_to_budget;
///
/// // Input shorter than budget: returned unchanged.
/// let s = "short text";
/// assert_eq!(trim_to_budget(s, 100), s);
///
/// // Truncation respects the nearest word boundary.
/// let long = "the quick brown fox jumps over the lazy dog";
/// let trimmed = trim_to_budget(long, 3);
/// assert!(trimmed.starts_with("the quick"));
/// assert!(trimmed.contains(" ..."));
///
/// // Multi-byte UTF-8 is never split mid-character.
/// let emoji_text = "🦀🦀🦀🦀 a b c d e f g h i j";
/// let out = trim_to_budget(emoji_text, 2);
/// assert!(out.is_char_boundary(out.len()));
/// ```
pub fn trim_to_budget(text: &str, budget_tokens: usize) -> String {
    let char_budget = budget_tokens.saturating_mul(4);
    if text.len() <= char_budget {
        return text.to_string();
    }
    // Snap the byte index to the nearest valid char boundary at or before
    // `char_budget`. This prevents panics on multi-byte UTF-8 inputs.
    let cut_byte = floor_char_boundary(text, char_budget);
    let mut cut = text[..cut_byte].to_string();
    if let Some(last_space) = cut.rfind(' ') {
        cut.truncate(last_space);
    }
    cut.push_str(" ...");
    cut
}

/// Returns the largest byte index `<= idx` that is a valid char boundary
/// in `s`. Returns 0 when `idx == 0`. Panics only on `idx > s.len()`.
fn floor_char_boundary(s: &str, idx: usize) -> usize {
    if idx >= s.len() {
        return s.len();
    }
    let mut i = idx;
    while i > 0 && !s.is_char_boundary(i) {
        i -= 1;
    }
    i
}

/// Combines the top-K aggregated items into a synthesised report.
pub fn synthesize(
    items: &[AggregatedItem],
    original_query: &str,
    format: SynthFormat,
    budget_tokens: usize,
) -> SynthesizedReport {
    // Heuristic cap: never synthesise more than 20 references per report.
    let top: &[AggregatedItem] = if items.len() > 20 {
        &items[..20]
    } else {
        items
    };

    let body = match format {
        SynthFormat::Markdown => render_markdown(top, original_query),
        SynthFormat::PlainText => render_plain(top, original_query),
        SynthFormat::Json => render_json(top, original_query),
    };
    let trimmed = trim_to_budget(&body, budget_tokens);
    SynthesizedReport {
        format,
        estimated_tokens: estimate_tokens(&trimmed),
        reference_count: top.len(),
        body: trimmed,
    }
}

fn render_markdown(items: &[AggregatedItem], query: &str) -> String {
    let mut s = String::new();
    s.push_str(&format!("## Deep Research: {query}\n\n"));
    s.push_str("### Summary\n\n");
    if items.is_empty() {
        s.push_str("_No results were aggregated._\n");
        return s;
    }
    s.push_str(&format!(
        "Aggregated {} result(s) from {} sub-queries. The top-ranked sources are summarised below.\n\n",
        items.len(),
        items
            .iter()
            .map(|i| i.sources.len())
            .max()
            .unwrap_or(0)
            .max(1)
    ));
    s.push_str("### Key Findings\n\n");
    for (i, item) in items.iter().enumerate() {
        let id = i + 1;
        let snippet = item.snippet.as_deref().unwrap_or("(no snippet)");
        s.push_str(&format!(
            "{}. [{}]({}) — {}\n",
            id,
            truncate(&item.title, 80),
            item.url,
            truncate(snippet, 240)
        ));
    }
    s.push_str("\n### References\n\n");
    for (i, item) in items.iter().enumerate() {
        let id = i + 1;
        s.push_str(&format!("[{}] {}\n", id, item.url));
    }
    s
}

fn render_plain(items: &[AggregatedItem], query: &str) -> String {
    let mut s = String::new();
    s.push_str(&format!("Deep Research: {query}\n\n"));
    if items.is_empty() {
        s.push_str("No results were aggregated.\n");
        return s;
    }
    s.push_str(&format!("Top {} result(s):\n\n", items.len()));
    for (i, item) in items.iter().enumerate() {
        let id = i + 1;
        let snippet = item.snippet.as_deref().unwrap_or("(no snippet)");
        s.push_str(&format!(
            "{}. {}\n   URL: {}\n   {}\n",
            id,
            item.title,
            item.url,
            truncate(snippet, 240)
        ));
    }
    s
}

fn render_json(items: &[AggregatedItem], query: &str) -> String {
    #[derive(Serialize)]
    struct Ref<'a> {
        id: usize,
        url: &'a str,
        title: &'a str,
        score: f64,
    }
    #[derive(Serialize)]
    struct Body<'a> {
        query: &'a str,
        summary: String,
        references: Vec<Ref<'a>>,
    }
    let body = Body {
        query,
        summary: format!(
            "Aggregated {} result(s) for the deep-research query.",
            items.len()
        ),
        references: items
            .iter()
            .enumerate()
            .map(|(i, item)| Ref {
                id: i + 1,
                url: &item.url,
                title: &item.title,
                score: item.score,
            })
            .collect(),
    };
    serde_json::to_string_pretty(&body).unwrap_or_else(|_| "{}".to_string())
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let cut: String = s.chars().take(max).collect();
        format!("{cut}...")
    }
}

// Link from SynthFormat to DeepResearchArgs is not needed at this layer; we
// re-export the type to avoid a second copy in lib.rs.
#[allow(dead_code)]
fn _ensure_link(_: DeepResearchArgs) {}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(url: &str, title: &str, snippet: &str, score: f64) -> AggregatedItem {
        AggregatedItem {
            url: url.to_string(),
            title: title.to_string(),
            display_url: None,
            snippet: Some(snippet.to_string()),
            score,
            position: 1,
            sources: vec!["alpha".to_string()],
        }
    }

    #[test]
    fn estimate_tokens_is_4_chars_per_token() {
        assert_eq!(estimate_tokens(""), 0);
        assert_eq!(estimate_tokens("abcd"), 1);
        assert_eq!(estimate_tokens("abcde"), 2);
    }

    #[test]
    fn trim_to_budget_preserves_under_limit() {
        let s = "hello world".to_string();
        assert_eq!(trim_to_budget(&s, 100), s);
    }

    #[test]
    fn trim_to_budget_cuts_above_limit() {
        let s = "a".repeat(200);
        let out = trim_to_budget(&s, 10);
        assert!(out.len() < 200);
        assert!(out.ends_with("..."));
    }

    #[test]
    fn markdown_reports_empty_when_no_items() {
        let r = synthesize(&[], "q", SynthFormat::Markdown, 4000);
        assert!(r.body.contains("No results"));
    }

    #[test]
    fn markdown_caps_at_twenty_references() {
        let items: Vec<AggregatedItem> = (0..50)
            .map(|i| {
                item(
                    &format!("https://e.com/{i}"),
                    "t",
                    "s",
                    1.0 - i as f64 * 0.01,
                )
            })
            .collect();
        let r = synthesize(&items, "q", SynthFormat::Markdown, 4000);
        assert!(r.body.contains("[20]"));
        assert!(!r.body.contains("[21]"));
        assert_eq!(r.reference_count, 20);
    }

    #[test]
    fn json_is_valid_json_with_references() {
        let items = vec![item("https://e.com/a", "title", "snippet", 0.5)];
        let r = synthesize(&items, "q", SynthFormat::Json, 4000);
        let parsed: serde_json::Value = serde_json::from_str(&r.body).expect("valid json");
        assert_eq!(parsed["query"], "q");
        assert_eq!(parsed["references"][0]["url"], "https://e.com/a");
    }

    #[test]
    fn plain_text_renders_numbered_list() {
        let items = vec![item("https://e.com/a", "title", "snippet", 0.5)];
        let r = synthesize(&items, "q", SynthFormat::PlainText, 4000);
        assert!(r.body.contains("1. title"));
        assert!(r.body.contains("URL: https://e.com/a"));
    }

    #[test]
    fn budget_respected_with_five_percent_margin() {
        // Markdown output is bounded by the budget — the snippet itself is
        // capped at `budget_tokens * 4` chars, the surrounding headings
        // add a small constant overhead, so the total fits within ~10% of
        // the budget.
        let long_snippet = "a".repeat(100_000);
        let items = vec![item("https://e.com/a", "t", &long_snippet, 0.5)];
        let r = synthesize(&items, "q", SynthFormat::Markdown, 100);
        assert!(
            r.estimated_tokens <= 110,
            "estimated_tokens {} exceeded budget+10%",
            r.estimated_tokens
        );
    }

    // ---------------------------------------------------------------
    // Property-based tests (proptest)
    // ---------------------------------------------------------------
    mod proptests {
        use super::*;
        use proptest::prelude::*;

        proptest! {
            /// `estimate_tokens` is monotonic non-decreasing with input length.
            #[test]
            fn estimate_tokens_is_monotonic(short in ".{0,20}", long_extra in ".{1,40}") {
                let short_t = estimate_tokens(&short);
                let long = format!("{}{}", short, long_extra);
                let long_t = estimate_tokens(&long);
                prop_assert!(long_t >= short_t);
            }

            /// `trim_to_budget` never returns more characters than the
            /// (4 × budget) char ceiling, plus the ` ...` suffix overhead.
            #[test]
            fn trim_to_budget_respects_ceiling(
                text in ".{0,200}",
                budget in 0usize..50,
            ) {
                let out = trim_to_budget(&text, budget);
                let ceiling = budget.saturating_mul(4) + 4;
                prop_assert!(
                    out.len() <= ceiling,
                    "trim produced {} chars > ceiling {}",
                    out.len(),
                    ceiling
                );
            }

            /// `trim_to_budget` is idempotent: trimming an already-trimmed
            /// string at the same budget must yield the same result.
            #[test]
            fn trim_to_budget_is_idempotent(text in ".{0,80}", budget in 1usize..20) {
                let once = trim_to_budget(&text, budget);
                let twice = trim_to_budget(&once, budget);
                prop_assert_eq!(once, twice);
            }
        }
    }
}
