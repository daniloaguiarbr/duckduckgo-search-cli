// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload: declarative (string transformations, no I/O).
//! Query decomposition for deep-research fan-out.
//!
//! Splits an original user query into 1..=`max_sub_queries` sub-queries using
//! one of three strategies:
//!
//! - `Heuristic` — applies five canonical templates:
//!   aspect, comparison, timeline, opinion, cause. Pure local computation.
//! - `Manual` — reads a list of sub-queries from a file
//!   or stdin, one per line. Empty lines and `#`-comments are ignored.
//!
//! All templates are deterministic for a given input — no LLM is invoked.
//! When the heuristic strategy produces fewer templates than `max_sub_queries`,
//! the function tops up by emitting focused refinements of the original query.

use crate::error::CliError;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tokio_util::sync::CancellationToken;

/// A single sub-query produced by decomposition.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SubQuery {
    /// The sub-query text to be sent to `DuckDuckGo`.
    pub text: String,
    /// Origin label — `heuristic:<template>`, `manual`, or `heuristic:refine`.
    pub origin: SubQueryOrigin,
}

impl SubQuery {
    /// Returns a short, stable label for logs and JSON output.
    pub fn strategy_label(&self) -> String {
        match &self.origin {
            SubQueryOrigin::Heuristic { template } => format!("heuristic:{}", template.as_str()),
            SubQueryOrigin::Manual => "manual".to_string(),
            SubQueryOrigin::HeuristicRefine => "heuristic:refine".to_string(),
        }
    }
}

/// Origin of a sub-query, used for observability and JSON reporting.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SubQueryOrigin {
    /// Produced by a named heuristic template.
    Heuristic {
        /// The template that produced this sub-query.
        template: HeuristicTemplate,
    },
    /// Loaded from a manual file/stdin list.
    Manual,
    /// Generated as a refinement of the original query when the templates did
    /// not fill the `max_sub_queries` budget.
    HeuristicRefine,
}

/// The five canonical heuristic templates for query fan-out.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum HeuristicTemplate {
    /// Focused aspect of the topic (e.g. `"<q> key components"`).
    Aspect,
    /// Comparison framing (e.g. `"<q> vs alternatives"`).
    Comparison,
    /// Timeline framing (e.g. `"<q> history timeline"`).
    Timeline,
    /// Opinion framing (e.g. `"<q> reviews opinions"`).
    Opinion,
    /// Causal framing (e.g. `"<q> causes effects"`).
    Cause,
}

impl HeuristicTemplate {
    /// Returns the kebab-case name of the template (used in labels and logs).
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Aspect => "aspect",
            Self::Comparison => "comparison",
            Self::Timeline => "timeline",
            Self::Opinion => "opinion",
            Self::Cause => "cause",
        }
    }

    /// Returns the suffix appended to the original query for this template.
    ///
    /// # Examples
    ///
    /// ```
    /// use duckduckgo_search_cli::decomposition::HeuristicTemplate;
    ///
    /// assert_eq!(HeuristicTemplate::Aspect.suffix(), "main aspects components");
    /// assert_eq!(HeuristicTemplate::Comparison.suffix(), "vs alternatives comparison");
    /// assert_eq!(HeuristicTemplate::Cause.suffix(), "causes effects consequences");
    /// assert_eq!(HeuristicTemplate::all().len(), 5);
    /// ```
    pub fn suffix(self) -> &'static str {
        match self {
            Self::Aspect => "main aspects components",
            Self::Comparison => "vs alternatives comparison",
            Self::Timeline => "history timeline evolution",
            Self::Opinion => "reviews opinions expert",
            Self::Cause => "causes effects consequences",
        }
    }

    /// Returns the list of all five templates in canonical order.
    pub fn all() -> [Self; 5] {
        [
            Self::Aspect,
            Self::Comparison,
            Self::Timeline,
            Self::Opinion,
            Self::Cause,
        ]
    }
}

/// Decomposes a user query into a list of sub-queries.
///
/// # Arguments
///
/// * `query` — the original user query.
/// * `strategy` — heuristic or manual.
/// * `manual_path` — when `Some`, used as the source of manual sub-queries
///   (only honoured when `strategy == Manual`).
/// * `max_sub_queries` — upper bound on the number of returned sub-queries.
/// * `cancel` — cooperative cancellation token.
///
/// # Errors
///
/// Returns [`CliError::InvalidConfig`] when `manual_path` is `Some` but the
/// file cannot be read, or when the manual list is empty.
///
/// # Cancel safety
///
/// This function is cancel-safe — it never holds resources across `await`
/// points in a way that would leak on cancellation.
pub async fn decompose(
    query: &str,
    strategy: crate::deep_research::SubQueryStrategy,
    manual_path: Option<&Path>,
    max_sub_queries: usize,
    cancel: &CancellationToken,
) -> Result<Vec<SubQuery>, CliError> {
    if cancel.is_cancelled() {
        return Err(CliError::Cancelled);
    }
    let trimmed = query.trim();
    if trimmed.is_empty() {
        return Err(CliError::InvalidConfig {
            message: "deep-research query is empty".to_string(),
        });
    }
    if max_sub_queries == 0 {
        return Err(CliError::InvalidConfig {
            message: "max_sub_queries must be at least 1".to_string(),
        });
    }

    match strategy {
        crate::deep_research::SubQueryStrategy::Manual => {
            load_manual(manual_path, max_sub_queries).await
        }
        crate::deep_research::SubQueryStrategy::Heuristic => {
            Ok(heuristic_decompose(trimmed, max_sub_queries))
        }
    }
}

/// Heuristic signals that a query is already a compound (multi-concept) one.
/// When matched, the corresponding [`HeuristicTemplate`] is suppressed from
/// the fan-out to avoid emitting redundant or self-contradictory sub-queries
/// like `rust vs go vs alternatives comparison`.
///
/// The detection is conservative: a single match on the high-signal
/// patterns below is enough to mark the query as composite for that
/// dimension. False positives are tolerable (the worst case is one
/// fewer sub-query); false negatives are not (the worst case is a
/// nonsensical `"X vs Y vs alternatives comparison"` sub-query).
///
/// # Panics
///
/// Panics only if one of the static regexes fails to compile, which
/// cannot happen with the current literals — the `expect` calls are
/// kept as a defence-in-depth marker for future edits.
pub fn is_composite_query(query: &str, signal: CompositeSignal) -> bool {
    static RE_VS: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    static RE_AND: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    static RE_COLON: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    static RE_TIMELINE: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    static RE_OPINION: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    static RE_CAUSE: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();

    let re = match signal {
        CompositeSignal::Comparison => RE_VS.get_or_init(|| {
            // "X vs Y", "X versus Y", "X or Y" — case-insensitive.
            Regex::new(r"(?i)\b(vs\.?|versus|or)\b").expect("static regex")
        }),
        CompositeSignal::Aspect => RE_AND.get_or_init(|| {
            // "X and Y", "X & Y", "X, Y" — case-insensitive.
            Regex::new(r"(?i)\b(and)\b|\s&\s|,\s").expect("static regex")
        }),
        CompositeSignal::Timeline => RE_TIMELINE.get_or_init(|| {
            // "history of", "evolution of", "from ... to ...", year ranges.
            Regex::new(r"(?i)\b(history|evolution|timeline|chronolog)|(\b\d{4}\b.*\b\d{4}\b)")
                .expect("static regex")
        }),
        CompositeSignal::Opinion => RE_OPINION.get_or_init(|| {
            // "reviews", "opinions", "best", "worst", "rating".
            Regex::new(r"(?i)\b(review|opinion|rating|best|worst)\b").expect("static regex")
        }),
        CompositeSignal::Cause => RE_CAUSE.get_or_init(|| {
            // "causes", "effects", "consequences", "why", "impact".
            Regex::new(r"(?i)(causes?|effects?|consequences?|why|impact)\b").expect("static regex")
        }),
        CompositeSignal::Topic => RE_COLON.get_or_init(|| {
            // "topic: subtopic" — explicit hierarchical decomposition.
            Regex::new(r":\s|\s-\s").expect("static regex")
        }),
    };
    re.is_match(query)
}

/// High-level decomposition signal a query might already encode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompositeSignal {
    /// Query is already a comparison (`X vs Y`).
    Comparison,
    /// Query is already a multi-aspect listing (`X and Y`).
    Aspect,
    /// Query is already timeline-shaped (`history of X`).
    Timeline,
    /// Query is already opinion-shaped (`best X`).
    Opinion,
    /// Query is already cause-shaped (`why X`).
    Cause,
    /// Query is already topic-decomposed (`topic: subtopic`).
    Topic,
}

fn heuristic_decompose(query: &str, max_sub_queries: usize) -> Vec<SubQuery> {
    let template_for_signal = [
        (CompositeSignal::Aspect, HeuristicTemplate::Aspect),
        (CompositeSignal::Comparison, HeuristicTemplate::Comparison),
        (CompositeSignal::Timeline, HeuristicTemplate::Timeline),
        (CompositeSignal::Opinion, HeuristicTemplate::Opinion),
        (CompositeSignal::Cause, HeuristicTemplate::Cause),
    ];

    let mut out: Vec<SubQuery> = template_for_signal
        .into_iter()
        .filter(|(sig, _)| !is_composite_query(query, *sig))
        .take(max_sub_queries)
        .map(|(_, t)| SubQuery {
            text: format!("{} {}", query, t.suffix()),
            origin: SubQueryOrigin::Heuristic { template: t },
        })
        .collect();

    // If the user requested more sub-queries than templates, top up with
    // language refinements of the original query.
    let mut refine_index: usize = 0;
    let refinements = ["tutorial guide", "examples use cases", "best practices"];
    while out.len() < max_sub_queries {
        let suffix = refinements[refine_index % refinements.len()];
        out.push(SubQuery {
            text: format!("{} {}", query, suffix),
            origin: SubQueryOrigin::HeuristicRefine,
        });
        refine_index += 1;
        if refine_index >= refinements.len() * 4 {
            // Defensive: never loop forever even if a caller passes a huge value.
            break;
        }
    }
    out
}

async fn load_manual(
    path: Option<&Path>,
    max_sub_queries: usize,
) -> Result<Vec<SubQuery>, CliError> {
    let path = path.ok_or_else(|| CliError::InvalidConfig {
        message: "manual sub-query strategy requires --sub-queries-file".to_string(),
    })?;
    let contents = tokio::fs::read_to_string(path)
        .await
        .map_err(|e| CliError::InvalidConfig {
            message: format!("read sub-queries file: {e}"),
        })?;
    let mut out: Vec<SubQuery> = Vec::new();
    for line in contents.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        out.push(SubQuery {
            text: trimmed.to_string(),
            origin: SubQueryOrigin::Manual,
        });
        if out.len() >= max_sub_queries {
            break;
        }
    }
    if out.is_empty() {
        return Err(CliError::InvalidConfig {
            message: format!(
                "no sub-queries found in {} (file empty or only comments)",
                path.display()
            ),
        });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tok() -> CancellationToken {
        CancellationToken::new()
    }

    #[tokio::test]
    async fn heuristic_produces_five_when_no_cap() {
        let out = decompose(
            "rust async",
            crate::deep_research::SubQueryStrategy::Heuristic,
            None,
            5,
            &tok(),
        )
        .await
        .expect("ok");
        assert_eq!(out.len(), 5);
        for (i, sq) in out.iter().enumerate() {
            assert_eq!(
                sq.origin,
                SubQueryOrigin::Heuristic {
                    template: HeuristicTemplate::all()[i]
                }
            );
        }
    }

    #[tokio::test]
    async fn heuristic_caps_at_max() {
        let out = decompose(
            "rust",
            crate::deep_research::SubQueryStrategy::Heuristic,
            None,
            2,
            &tok(),
        )
        .await
        .expect("ok");
        assert_eq!(out.len(), 2);
    }

    #[tokio::test]
    async fn heuristic_top_up_with_refinements() {
        let out = decompose(
            "rust",
            crate::deep_research::SubQueryStrategy::Heuristic,
            None,
            7,
            &tok(),
        )
        .await
        .expect("ok");
        assert_eq!(out.len(), 7);
        assert_eq!(out[5].origin, SubQueryOrigin::HeuristicRefine);
        assert_eq!(out[6].origin, SubQueryOrigin::HeuristicRefine);
    }

    #[tokio::test]
    async fn empty_query_rejected() {
        let err = decompose(
            "   ",
            crate::deep_research::SubQueryStrategy::Heuristic,
            None,
            5,
            &tok(),
        )
        .await
        .expect_err("must fail");
        assert!(matches!(err, CliError::InvalidConfig { .. }));
    }

    #[tokio::test]
    async fn manual_without_path_rejected() {
        let err = decompose(
            "x",
            crate::deep_research::SubQueryStrategy::Manual,
            None,
            5,
            &tok(),
        )
        .await
        .expect_err("must fail");
        assert!(matches!(err, CliError::InvalidConfig { .. }));
    }

    #[tokio::test]
    async fn manual_reads_file_and_skips_comments() {
        let tmp = tempfile::NamedTempFile::new().expect("tmpfile");
        std::fs::write(
            tmp.path(),
            "# header comment\n\nalpha beta\n# another comment\ngamma delta\n",
        )
        .expect("write");
        let out = decompose(
            "ignored",
            crate::deep_research::SubQueryStrategy::Manual,
            Some(tmp.path()),
            10,
            &tok(),
        )
        .await
        .expect("ok");
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].text, "alpha beta");
        assert_eq!(out[1].text, "gamma delta");
        assert_eq!(out[0].origin, SubQueryOrigin::Manual);
    }

    #[tokio::test]
    async fn cancel_aborts_decomposition() {
        let token = CancellationToken::new();
        token.cancel();
        let err = decompose(
            "x",
            crate::deep_research::SubQueryStrategy::Heuristic,
            None,
            5,
            &token,
        )
        .await
        .expect_err("must fail");
        assert!(matches!(err, CliError::Cancelled));
    }

    #[test]
    fn template_labels_match_suffixes() {
        for t in HeuristicTemplate::all() {
            assert!(!t.as_str().is_empty());
            assert!(!t.suffix().is_empty());
        }
    }

    #[test]
    fn composite_query_detects_comparison() {
        assert!(is_composite_query(
            "rust vs go",
            CompositeSignal::Comparison
        ));
        assert!(is_composite_query(
            "PostgreSQL versus MySQL",
            CompositeSignal::Comparison
        ));
        assert!(is_composite_query(
            "read or write",
            CompositeSignal::Comparison
        ));
        assert!(!is_composite_query(
            "rust async runtime",
            CompositeSignal::Comparison
        ));
    }

    #[test]
    fn composite_query_detects_aspect() {
        assert!(is_composite_query(
            "cargo and clippy",
            CompositeSignal::Aspect
        ));
        assert!(is_composite_query(
            "cargo & clippy",
            CompositeSignal::Aspect
        ));
        assert!(is_composite_query(
            "cargo, clippy, rustfmt",
            CompositeSignal::Aspect
        ));
        assert!(!is_composite_query("rust async", CompositeSignal::Aspect));
    }

    #[test]
    fn composite_query_detects_timeline() {
        assert!(is_composite_query(
            "history of rust",
            CompositeSignal::Timeline
        ));
        assert!(is_composite_query(
            "evolution of async runtimes",
            CompositeSignal::Timeline
        ));
        assert!(is_composite_query(
            "rust 2015 to 2024",
            CompositeSignal::Timeline
        ));
        assert!(!is_composite_query(
            "rust async runtime",
            CompositeSignal::Timeline
        ));
    }

    #[test]
    fn composite_query_detects_opinion() {
        assert!(is_composite_query(
            "best rust web framework",
            CompositeSignal::Opinion
        ));
        assert!(is_composite_query(
            "actix vs axum review",
            CompositeSignal::Opinion
        ));
        assert!(!is_composite_query(
            "rust async runtime",
            CompositeSignal::Opinion
        ));
    }

    #[test]
    fn composite_query_detects_cause() {
        assert!(is_composite_query(
            "why is rust hard",
            CompositeSignal::Cause
        ));
        assert!(is_composite_query(
            "causes of memory unsafety",
            CompositeSignal::Cause
        ));
        assert!(!is_composite_query(
            "rust async runtime",
            CompositeSignal::Cause
        ));
    }

    #[test]
    fn composite_query_detects_topic() {
        assert!(is_composite_query(
            "rust: async runtime",
            CompositeSignal::Topic
        ));
        assert!(is_composite_query(
            "postgres - replication setup",
            CompositeSignal::Topic
        ));
        assert!(!is_composite_query(
            "rust async runtime",
            CompositeSignal::Topic
        ));
    }

    #[test]
    fn heuristic_skips_redundant_comparison_template() {
        let subs = heuristic_decompose("rust vs go", 5);
        // Comparison template is suppressed; we should NOT see the literal
        // suffix "vs alternatives comparison" in any sub-query.
        for s in &subs {
            assert!(
                !s.text.contains("vs alternatives comparison"),
                "redundant template: {}",
                s.text
            );
        }
    }

    #[test]
    fn heuristic_skips_redundant_cause_template() {
        let subs = heuristic_decompose("why is rust hard", 5);
        for s in &subs {
            assert!(
                !s.text.contains("causes effects consequences"),
                "redundant template: {}",
                s.text
            );
        }
    }

    #[tokio::test]
    async fn decompose_rejects_empty_query() {
        let result = decompose(
            "   ",
            crate::deep_research::SubQueryStrategy::Heuristic,
            None,
            3,
            &tok(),
        )
        .await;
        assert!(matches!(result, Err(CliError::InvalidConfig { .. })));
    }

    #[tokio::test]
    async fn decompose_rejects_zero_max() {
        let result = decompose(
            "rust",
            crate::deep_research::SubQueryStrategy::Heuristic,
            None,
            0,
            &tok(),
        )
        .await;
        assert!(matches!(result, Err(CliError::InvalidConfig { .. })));
    }

    #[tokio::test]
    async fn decompose_respects_cancellation() {
        let token = CancellationToken::new();
        token.cancel();
        let result = decompose(
            "rust",
            crate::deep_research::SubQueryStrategy::Heuristic,
            None,
            3,
            &token,
        )
        .await;
        assert!(matches!(result, Err(CliError::Cancelled)));
    }

    #[tokio::test]
    async fn heuristic_with_single_token_query() {
        let out = decompose(
            "rust",
            crate::deep_research::SubQueryStrategy::Heuristic,
            None,
            2,
            &tok(),
        )
        .await
        .expect("ok");
        assert_eq!(out.len(), 2);
        for s in &out {
            assert!(s.text.starts_with("rust "));
        }
    }

    #[tokio::test]
    async fn manual_strategy_skips_blank_and_comment_lines() {
        let dir = std::env::temp_dir();
        let unique = format!(
            "dr-sub-queries-{}.txt",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        );
        let path = dir.join(unique);
        std::fs::write(
            &path,
            "# header comment\n\
             \n\
             # blank line above\n\
             rust async runtime\n\
             tokio vs async-std\n\
             \n\
             best rust web framework 2026\n",
        )
        .expect("write temp file");
        let result = decompose(
            "rust",
            crate::deep_research::SubQueryStrategy::Manual,
            Some(&path),
            10,
            &tok(),
        )
        .await
        .expect("ok");
        assert_eq!(result.len(), 3, "comments and blank lines must be ignored");
        assert_eq!(result[0].text, "rust async runtime");
        assert_eq!(result[1].text, "tokio vs async-std");
        assert_eq!(result[2].text, "best rust web framework 2026");
        let _ = std::fs::remove_file(&path);
    }

    #[tokio::test]
    async fn manual_strategy_rejects_file_with_only_comments() {
        let dir = std::env::temp_dir();
        let unique = format!(
            "dr-empty-{}.txt",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        );
        let path = dir.join(unique);
        std::fs::write(&path, "# only comments\n# nothing else\n").expect("write temp file");
        let result = decompose(
            "rust",
            crate::deep_research::SubQueryStrategy::Manual,
            Some(&path),
            10,
            &tok(),
        )
        .await;
        assert!(matches!(result, Err(CliError::InvalidConfig { .. })));
        let _ = std::fs::remove_file(&path);
    }

    #[tokio::test]
    async fn manual_strategy_requires_path() {
        let result = decompose(
            "rust",
            crate::deep_research::SubQueryStrategy::Manual,
            None,
            3,
            &tok(),
        )
        .await;
        assert!(matches!(result, Err(CliError::InvalidConfig { .. })));
    }
}
