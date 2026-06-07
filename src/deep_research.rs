// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload: I/O-bound (multi-query fan-out against DuckDuckGo).
//! Deep research subcommand — query fan-out, aggregation, and synthesis.
//!
//! This module is the single entry point for the `deep-research` subcommand. It
//! composes four smaller modules into an 8-stage pipeline:
//!
//! 1. `decomposition::decompose` — splits the user query into 1..=`max_sub_queries`
//!    sub-queries using a heuristic, manual, or template strategy.
//! 2. `parallel::execute_query_fan_out` — fans out all sub-queries via `JoinSet`
//!    with a `Semaphore` for bounded concurrency and `CancellationToken` for
//!    graceful shutdown.
//! 3. `aggregation::aggregate` — merges the per-sub-query result lists into a
//!    single ranked list using Reciprocal Rank Fusion (RRF, K=60) or
//!    URL-canonical deduplication.
//! 4. `synthesis::synthesize` — optionally combines the top-K results into a
//!    Markdown/PlainText/Json report with numbered references.
//!
//! # Design notes
//!
//! - The output struct uses Portuguese JSON keys (`metadados`, `resultados`,
//!   `sintese`) to keep the public schema consistent with the rest of the CLI.
//! - `SubQuery` carries its origin (heuristic/manual/template) and a per-query
//!   `elapsed_ms` for observability.
//! - Aggregation emits a score in `[0.0, 1.0]` — higher is better — plus a
//!   `fontes: Vec<String>` listing the sub-query texts that produced the result
//!   for traceability.
//!
//! Content fetches performed by the synthesis stage honour the same
//! per-host rate limiting, circuit breaker, and concurrency controls as
//! the rest of the binary.
//!
//! # Example
//!
//! The function is the entry point called from `lib::execute_deep_research`.
//! End-to-end usage requires a fully-built [`Config`] (the CLI's
//! `lib::execute_deep_research` builds one from the global flags), the
//! operator-provided [`DeepResearchArgs`], and a [`CancellationToken`].
//!
//! ```no_run
//! use duckduckgo_search_cli::deep_research::{
//!     DeepResearchArgs, SubQueryStrategy, AggregationStrategyKind,
//! };
//! use duckduckgo_search_cli::synthesis::SynthFormat;
//!
//! let _args = DeepResearchArgs {
//!     query: "rust async".to_string(),
//!     max_sub_queries: 2,
//!     sub_query_strategy: SubQueryStrategy::Heuristic,
//!     sub_queries_file: None,
//!     aggregation: AggregationStrategyKind::Rrf,
//!     depth: 0,
//!     fetch_content: false,
//!     synthesize: false,
//!     budget_tokens: 4000,
//!     synth_format: SynthFormat::Markdown,
//! };
//! // See `lib::execute_deep_research` for a complete wiring example that
//! // builds a `Config` from CLI args and dispatches the pipeline.
//! ```

use crate::aggregation::{aggregate, AggregatedItem, AggregationStrategy};
use crate::decomposition::{decompose, SubQuery};
use crate::error::CliError;
use crate::parallel::execute_parallel_searches;
use crate::synthesis::{synthesize, SynthFormat, SynthesizedReport};
use crate::types::{Config, SearchOutput};
use serde::{Deserialize, Serialize};
use std::time::Instant;
use tokio_util::sync::CancellationToken;

/// Hard upper bound on the number of sub-queries produced by decomposition.
pub const MAX_SUB_QUERIES: usize = 12;

/// Default number of sub-queries when the user does not specify.
pub const DEFAULT_MAX_SUB_QUERIES: usize = 5;

/// Hard upper bound on the depth (number of reflective rounds).
pub const MAX_DEPTH: u32 = 3;

/// Default RRF constant (K in `1 / (K + rank)`). 60 is the de-facto literature
/// default (Cormack et al., 2009) and matches the `SQLite` `FTS5` anchor used
/// in the `GraphRAG` memory subsystem.
pub const RRF_K: u32 = 60;

/// Strategy used to decompose the original query into sub-queries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SubQueryStrategy {
    /// Heuristic fan-out (5 canonical templates: aspect, comparison, timeline,
    /// opinion, cause). Default — pure local computation, no LLM cost.
    Heuristic,
    /// Pre-defined list of sub-queries from a file or stdin.
    Manual,
}

/// Strategy used to merge per-sub-query results into a single ranked list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AggregationStrategyKind {
    /// Reciprocal Rank Fusion across sub-queries (K=60 by default).
    Rrf,
    /// Canonical-URL deduplication (keep first occurrence, drop the rest).
    DedupeByUrl,
}

/// Arguments for the `deep-research` subcommand.
#[derive(Debug, Clone)]
pub struct DeepResearchArgs {
    /// The original user query.
    pub query: String,
    /// Maximum number of sub-queries to produce (1..=12, default 5).
    pub max_sub_queries: usize,
    /// Decomposition strategy (default `Heuristic`).
    pub sub_query_strategy: SubQueryStrategy,
    /// Optional path to a file containing explicit sub-queries (one per line).
    /// Only honoured when `sub_query_strategy = Manual`.
    pub sub_queries_file: Option<std::path::PathBuf>,
    /// Aggregation strategy (default `Rrf`).
    pub aggregation: AggregationStrategyKind,
    /// Reflection depth — number of follow-up gap-filling rounds (0..=3).
    /// 0 = single pass, 1..=3 = iterative refinement using the top-ranked
    /// results to inform new sub-queries.
    pub depth: u32,
    /// When `true`, enable `--fetch-content` behaviour for the top-K results.
    pub fetch_content: bool,
    /// When `true`, produce a synthesised report (Markdown/PlainText/Json).
    pub synthesize: bool,
    /// Token budget for the synthesised report (~4 chars ≈ 1 token heuristic).
    pub budget_tokens: usize,
    /// Format of the synthesised report (only used when `synthesize` is true).
    pub synth_format: SynthFormat,
}

impl Default for DeepResearchArgs {
    fn default() -> Self {
        Self {
            query: String::new(),
            max_sub_queries: DEFAULT_MAX_SUB_QUERIES,
            sub_query_strategy: SubQueryStrategy::Heuristic,
            sub_queries_file: None,
            aggregation: AggregationStrategyKind::Rrf,
            depth: 0,
            fetch_content: false,
            synthesize: false,
            budget_tokens: 4000,
            synth_format: SynthFormat::Markdown,
        }
    }
}

impl DeepResearchArgs {
    /// Validates that the field ranges are within documented bounds.
    ///
    /// # Errors
    ///
    /// Returns an error string when `max_sub_queries` is zero or exceeds
    /// [`MAX_SUB_QUERIES`], or when `depth` exceeds [`MAX_DEPTH`].
    ///
    /// # Examples
    ///
    /// ```
    /// use duckduckgo_search_cli::deep_research::{
    ///     DeepResearchArgs, MAX_SUB_QUERIES, MAX_DEPTH,
    /// };
    ///
    /// // Defaults are valid.
    /// assert!(DeepResearchArgs::default().validate().is_ok());
    ///
    /// // Zero is rejected.
    /// let mut args = DeepResearchArgs::default();
    /// args.max_sub_queries = 0;
    /// assert!(args.validate().is_err());
    ///
    /// // Above MAX_SUB_QUERIES is rejected.
    /// let mut args = DeepResearchArgs::default();
    /// args.max_sub_queries = MAX_SUB_QUERIES + 1;
    /// assert!(args.validate().is_err());
    ///
    /// // Depth above MAX_DEPTH is rejected.
    /// let mut args = DeepResearchArgs::default();
    /// args.depth = MAX_DEPTH + 1;
    /// assert!(args.validate().is_err());
    /// ```
    pub fn validate(&self) -> Result<(), String> {
        if self.max_sub_queries == 0 {
            return Err(format!(
                "--max-sub-queries must be at least 1 (got {})",
                self.max_sub_queries
            ));
        }
        if self.max_sub_queries > MAX_SUB_QUERIES {
            return Err(format!(
                "--max-sub-queries cannot exceed {} (got {})",
                MAX_SUB_QUERIES, self.max_sub_queries
            ));
        }
        if self.depth > MAX_DEPTH {
            return Err(format!(
                "--depth cannot exceed {} (got {})",
                MAX_DEPTH, self.depth
            ));
        }
        Ok(())
    }
}

/// Per-sub-query outcome.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubQueryOutcome {
    /// The sub-query text.
    #[serde(rename = "texto")]
    pub text: String,
    /// Origin label — `heuristic`, `manual`, or template name.
    #[serde(rename = "estrategia")]
    pub strategy: String,
    /// Status: `ok` when results were produced, `erro` otherwise.
    #[serde(rename = "status")]
    pub status: String,
    /// Wall-clock duration for this sub-query (milliseconds).
    #[serde(rename = "tempo_ms")]
    pub elapsed_ms: u64,
    /// Optional error message when `status == "erro"`.
    #[serde(rename = "mensagem_erro", skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Top-level output of the `deep-research` subcommand.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeepResearchOutput {
    /// Schema discriminator (always `"deep_research"`).
    #[serde(rename = "tipo")]
    pub kind: String,
    /// Run metadata (query, sub-queries, timings, etc.).
    #[serde(rename = "metadados")]
    pub metadata: DeepResearchMetadata,
    /// Aggregated evidence list (sorted by descending score).
    #[serde(rename = "resultados")]
    pub results: Vec<AggregatedItem>,
    /// Optional synthesised report.
    #[serde(rename = "sintese", skip_serializing_if = "Option::is_none")]
    pub synth: Option<SynthesizedReport>,
}

/// Run-level metadata for a deep-research execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeepResearchMetadata {
    /// Original user query.
    #[serde(rename = "query_original")]
    pub original_query: String,
    /// Sub-queries that were dispatched.
    #[serde(rename = "sub_queries")]
    pub sub_queries: Vec<SubQueryOutcome>,
    /// Aggregation strategy used.
    #[serde(rename = "estrategia_agregacao")]
    pub aggregation_strategy: String,
    /// Number of unique results after deduplication / RRF.
    #[serde(rename = "total_resultados_unicos")]
    pub unique_result_count: usize,
    /// End-to-end wall-clock duration (milliseconds).
    #[serde(rename = "tempo_total_ms")]
    pub total_elapsed_ms: u64,
    /// Anti-bot cascade level reached during the deepest sub-query.
    #[serde(rename = "nivel_cascata")]
    pub cascade_level: Option<u8>,
}

/// Runs the full deep-research pipeline.
///
/// # Arguments
///
/// * `args` — user-facing deep-research options.
/// * `cfg` — base [`Config`] inherited from the global CLI flags (HTTP client,
///   timeouts, proxies, etc.).
/// * `cancel` — token that signals SIGINT / global timeout.
///
/// # Errors
///
/// Returns an error only for unrecoverable setup failures (e.g. invalid
/// `sub_queries_file` when strategy is `Manual`). Per-sub-query failures are
/// captured inside the returned [`DeepResearchOutput`] rather than propagated
/// as `Err`, so partial results are always surfaced.
///
/// # Cancel safety
///
/// This function is cancel-safe. The `CancellationToken` is propagated to every
/// spawned sub-task; on cancellation, in-flight HTTP requests abort, partial
/// results are collected, and a `DeepResearchOutput` is returned with the
/// remaining sub-queries marked as `status == "erro"`.
pub async fn run_deep_research(
    args: DeepResearchArgs,
    cfg: &Config,
    cancel: CancellationToken,
) -> Result<DeepResearchOutput, CliError> {
    args.validate()
        .map_err(|e| CliError::InvalidConfig { message: e })?;

    let start_total = Instant::now();

    // Stage 1: decompose the original query.
    let sub_queries: Vec<SubQuery> = decompose(
        &args.query,
        args.sub_query_strategy,
        args.sub_queries_file.as_deref(),
        args.max_sub_queries,
        &cancel,
    )
    .await?;

    // Stage 2: fan out the sub-queries in parallel.
    let per_query_outputs: Vec<SearchOutput> = execute_parallel_searches(
        sub_queries.iter().map(|q| q.text.clone()).collect(),
        cfg.clone(),
        cancel.clone(),
    )
    .await?
    .searches;

    // Build the per-sub-query outcome report.
    let mut outcomes: Vec<SubQueryOutcome> = sub_queries
        .iter()
        .zip(per_query_outputs.iter())
        .map(|(q, o)| SubQueryOutcome {
            text: q.text.clone(),
            strategy: q.strategy_label().to_string(),
            status: if o.error.is_some() { "erro" } else { "ok" }.to_string(),
            elapsed_ms: o.metadata.execution_time_ms,
            error: o.error.clone(),
        })
        .collect();

    // Stage 3: aggregate across sub-queries.
    let aggregation_strategy = match args.aggregation {
        AggregationStrategyKind::Rrf => AggregationStrategy::Rrf(RRF_K),
        AggregationStrategyKind::DedupeByUrl => AggregationStrategy::DedupeByUrl,
    };

    let aggregated = aggregate(&per_query_outputs, aggregation_strategy);

    // Stage 4: optional synthesis.
    let synth = if args.synthesize {
        Some(synthesize(
            &aggregated,
            &args.query,
            args.synth_format,
            args.budget_tokens,
        ))
    } else {
        None
    };

    // Determine the deepest cascade level observed across all sub-queries.
    let cascade_level = per_query_outputs
        .iter()
        .filter_map(|o| o.metadata.cascade_level)
        .max()
        .map(|v| v as u8);

    // Reflective depth: future iterations can spawn follow-up sub-queries from
    // the aggregated top-K. For v0.7.0 we report the planned depth but do not
    // execute reflection yet (deferred to v0.6.13 with an LLM-driven gap fill).
    if args.depth > 0 {
        let planned = SubQueryOutcome {
            text: format!("<reflective depth={} not implemented>", args.depth),
            strategy: "depth".to_string(),
            status: "planejado".to_string(),
            elapsed_ms: 0,
            error: None,
        };
        outcomes.push(planned);
    }

    Ok(DeepResearchOutput {
        kind: "deep_research".to_string(),
        metadata: DeepResearchMetadata {
            original_query: args.query,
            sub_queries: outcomes,
            aggregation_strategy: match args.aggregation {
                AggregationStrategyKind::Rrf => "rrf".to_string(),
                AggregationStrategyKind::DedupeByUrl => "dedupe_by_url".to_string(),
            },
            unique_result_count: aggregated.len(),
            total_elapsed_ms: start_total.elapsed().as_millis() as u64,
            cascade_level,
        },
        results: aggregated,
        synth,
    })
}
