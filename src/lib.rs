// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload: orchestrator (config assembly, delegation to pipeline)
#![doc(html_root_url = "https://docs.rs/duckduckgo-search-cli/0.7.7")]
#![doc(html_playground_url = "https://play.rust-lang.org")]
#![warn(missing_docs)]
#![warn(missing_debug_implementations)]
#![warn(rustdoc::broken_intra_doc_links)]
#![warn(rustdoc::private_intra_doc_links)]
#![warn(rustdoc::missing_crate_level_docs)]
#![warn(rustdoc::invalid_codeblock_attributes)]
#![warn(rustdoc::invalid_html_tags)]
#![warn(rustdoc::bare_urls)]
#![warn(rustdoc::redundant_explicit_links)]
#![warn(clippy::undocumented_unsafe_blocks)]
#![warn(clippy::multiple_unsafe_ops_per_block)]
#![deny(unsafe_op_in_unsafe_fn)]
//! # duckduckgo-search-cli
//!
//! Rust CLI for searching `DuckDuckGo` via pure HTTP, with structured JSON output
//! for LLM consumption. No paid API. No Chrome (during the search phase).
//! No cache. Universal cross-platform (Linux including Alpine/NixOS/Flatpak/Snap,
//! macOS including Apple Silicon, Windows including cmd.exe and `PowerShell`).
//!
//! ## Module Structure
//!
//! | Module        | Responsibility                                               |
//! |---------------|--------------------------------------------------------------|
//! | [`cli`]       | Clap structs (command-line argument parsing).                |
//! | [`http`]      | `wreq::Client` construction and User-Agent selection.     |
//! | [`search`]    | URL building and HTTP request to the `DuckDuckGo` endpoint.    |
//! | [`extraction`]| HTML parsing with `scraper` and ad filtering.                |
//! | [`pipeline`]  | Single/multi orchestration, deduplication and source reading.|
//! | [`parallel`]  | Multi-query fan-out with `JoinSet`, Semaphore, `CancellationToken`.|
//! | [`output`]    | JSON serialization and stdout writing (ONLY module with `println!`).|
//! | [`platform`]  | Cross-platform initialization (UTF-8 on Windows, TTY detect).|
//! | [`types`]     | Shared structs and enums.                                    |
//! | [`error`]     | Error codes and exit codes.                                  |
//! | [`content`]   | HTTP + readability extraction for `--fetch-content` (iter. 5).|
//! | [`content_fetch`] | Parallel fan-out + per-host rate-limit (iter. 5 / 6).   |
//! | [`selectors`] | Loading of external `SelectorConfig` (iter. 6).      |
//! | [`signals`]   | Cross-platform signal handlers (SIGPIPE, Ctrl+C).            |
//! | [`config_init`] | `init-config` subcommand (iter. 6).                       |
//! | [`paths`]     | Path validation and sanitization for I/O.                    |
//! | `browser`     | Headless Chrome cross-platform under feature `chrome` (iter.7).|
//!
//! ## Entry Point
//!
//! The public function [`run`] is called by `main.rs` and returns an exit code
//! as specified in section 17.7 of the specification.

pub mod aggregation;
pub mod cli;
pub mod config_init;
pub mod content;
pub mod content_fetch;
pub mod decompress;
pub mod decomposition;
pub mod deep_research;
pub mod error;
pub mod extraction;
pub mod http;
pub mod identity;
pub mod output;
pub mod parallel;
pub mod paths;
pub mod pipeline;
pub mod platform;
pub mod probe_deep;
pub mod search;
pub mod selectors;
pub mod session_warmup;
pub mod signals;
pub mod synthesis;
pub mod types;
pub mod wreq_cookie_adapter;

// browser.rs declares `#![cfg(feature = "chrome")]` at the module root (line 25),
// which already excludes the entire module when the feature is off. Re-declaring
// `#[cfg(feature = "chrome")]` here is redundant and triggers clippy::duplicated_attributes.
// The previous `#[cfg_attr(docsrs, doc(cfg(...)))]` was removed in v0.6.6 because
// `doc(cfg)` is unstable and requires `#![feature(doc_cfg)]` since doc_auto_cfg
// was merged into doc_cfg in Oct 2025 (see rust-lang/rust#43781).
pub mod browser;

// Query de calibração longa para o probe-deep (GAP-WS-51).
//
// DuckDuckGo trata queries curtas e longas de forma diferente: queries
// de 1 palavra raramente acionam o sistema de bot detection, fazendo
// com que `--probe-deep` retorne "ok" mesmo quando uma query real de
// produção seria bloqueada. Esta string de 43 caracteres garante que
// o payload HTTP tenha tamanho realista, replicando o cenário real.
const PROBE_CALIBRATION_QUERY: &str = "the quick brown fox jumps over the lazy dog";

use crate::cli::{
    CliArgs, CliEndpoint, CliSafeSearch, CliTimeFilter, CompletionsArgs, InitConfigArgs, RootArgs,
    Subcommand,
};
use crate::error::exit_codes;
use crate::error::CliError;
use crate::types::{Config, Endpoint, OutputFormat, SafeSearch, TimeFilter};
use clap::Parser;
use tokio_util::sync::CancellationToken;
use tracing_subscriber::{fmt, EnvFilter};

/// Library entry point. Called by `main.rs`.
///
/// Returns the appropriate exit code (0 success, 1 generic error, 2 invalid config, etc.).
///
/// # Cancel safety
///
/// This function is cancel-safe. Dropping the future cancels all
/// in-flight HTTP requests via the [`CancellationToken`].
pub async fn run(cancellation: CancellationToken) -> i32 {
    // Parse command-line arguments — clap terminates the process with exit code 2 on error.
    let root = RootArgs::parse();

    // Dispatch subcommand (or fall through to default = Buscar).
    // v0.7.9 GAP-WS-59: capture the global flags before any potential
    // partial move of `root.buscar` so we can pass them to `build_config`
    // after the match.
    // v0.7.10 B3 fix: also capture `global_timeout_seconds` here — it
    // lives on `RootArgs` and must be hoisted out before consuming
    // `root.buscar` (which is a `Box<CliArgs>`).
    let allow_lite_fallback = root.allow_lite_fallback;
    let pre_flight = root.pre_flight;
    let root_global_timeout_seconds = root.global_timeout_seconds;
    // v0.7.10 GAP-WS-60 fix: capture `identity_profile` from `root.buscar`
    // before consuming `args` via the `match`. `Box<CliArgs>` is dereferenced
    // to read the field without moving the box itself.
    let identity_profile = root.buscar.identity_profile;

    // Initialize logging BEFORE subcommand dispatch so deep-research
    // respects -q/--verbose (fixes tracing leaking to stdout).
    let disable_colors = platform::should_disable_color(root.buscar.no_color);
    initialize_logging(root.buscar.verbose, root.buscar.quiet, disable_colors);
    platform::init();

    let args = match root.subcomando {
        Some(Subcommand::InitConfig(args)) => {
            return execute_init_config(args);
        }
        Some(Subcommand::Completions(args)) => {
            return execute_completions(args);
        }
        Some(Subcommand::Buscar(args)) => *args,
        Some(Subcommand::DeepResearch(dr_args)) => {
            let search_defaults = root.buscar;
            return execute_deep_research(
                dr_args,
                root_global_timeout_seconds,
                &search_defaults,
                allow_lite_fallback,
                pre_flight,
                identity_profile,
            )
            .await;
        }
        None => root.buscar,
    };

    // Logging and platform already initialized before subcommand dispatch.

    // v0.6.4 WS-26: Intercept --probe BEFORE query validation. The probe
    // is a pre-flight health check that does NOT require a query — it sends
    // 1 minimal request to the configured endpoint and reports status as JSON.
    if args.probe {
        return execute_probe(&args).await;
    }

    // v0.7.3 PR3: deep probe — runs a real query and detects CAPTCHA
    // interstitials in the response body. Emits a JSON report on stdout.
    if args.probe_deep {
        return execute_probe_deep(&args).await;
    }

    // Convert CliArgs into internal Config.
    let mut config = match build_config(&args) {
        Ok(c) => c,
        Err(err) => {
            tracing::error!(?err, "Invalid configuration");
            output::emit_stderr(&format!("Configuration error: {err:#}"));
            return exit_codes::INVALID_CONFIG;
        }
    };
    // v0.7.9 GAP-WS-59: inject the hoisted global flags into the
    // locally-built `Config`. `build_config` is `&CliArgs`-based and
    // the globals live on `RootArgs`; we apply them here so the
    // function signature stays minimal for the unit tests.
    // v0.7.10 B3 fix: also override `global_timeout_seconds` from the
    // hoisted `root_global_timeout_seconds` so the user-supplied value
    // is honored (the default value of 60 lives on `RootArgs`, not in
    // `CliArgs`).
    config.allow_lite_fallback = allow_lite_fallback;
    config.pre_flight = pre_flight;
    config.global_timeout_seconds = root_global_timeout_seconds;
    // v0.7.10 GAP-WS-60 fix: propagate `--identity-profile` into the Config
    // so the pipeline can fix the selected identity on the `IdentityPool`.
    config.identity_profile = identity_profile;

    let format = config.format;
    let output_file = config.output_file.clone();
    let global_timeout = std::time::Duration::from_secs(config.global_timeout_seconds);

    // GAP-AUD-004 v0.8.0: --allow-lite-fallback é precondição que FORÇA o
    // endpoint Lite independente do resultado do classificador. Antes era
    // apenas consultada no gate de auto-fallback (depois da busca falhar),
    // o que fazia a flag ser ignorada quando a busca html inicial obtinha
    // 0-result sem disparar causa não-legítima (race com classificador).
    if config.allow_lite_fallback && config.endpoint == crate::types::Endpoint::Html {
        tracing::info!(
            "GAP-AUD-004: --allow-lite-fallback ativo — forçando Endpoint::Lite na busca principal"
        );
        config.endpoint = crate::types::Endpoint::Lite;
    }

    // Wrap the pipeline in `tokio::time::timeout` — if it expires, cancel everything
    // and return exit code 4 (TIMEOUT_GLOBAL).
    let internal_cancellation = cancellation.clone();
    let pipeline_future = pipeline::execute_pipeline(config, internal_cancellation);

    let pipeline_result = match tokio::time::timeout(global_timeout, pipeline_future).await {
        Ok(result) => result,
        Err(_elapsed) => {
            // Propagate cancellation to any task still in-flight.
            cancellation.cancel();
            tracing::error!(
                seconds = global_timeout.as_secs(),
                "global timeout exceeded — execution aborted"
            );
            output::emit_stderr(&format!(
                "Error: global timeout of {}s exceeded",
                global_timeout.as_secs()
            ));
            return exit_codes::GLOBAL_TIMEOUT;
        }
    };

    match pipeline_result {
        Ok(output) => {
            // B2 fix: surface anti-bot (pre_flight_blocked) as exit 3
            // instead of exit 5 (zero results). The payload still travels
            // through `emit_result` so consumers see a single, well-formed
            // JSON object — the exit code is the only thing that changes.
            //
            // PipelineResult has 3 variants: Single(SearchOutput),
            // Multi(MultiSearchOutput), and Stream(StreamStats). We
            // inspect the inner SearchOutput / MultiSearchOutput for the
            // `error: "pre_flight_blocked"` marker when available.
            let pre_flight_blocked = match &output {
                crate::pipeline::PipelineResult::Single(s) => {
                    s.error.as_deref() == Some("pre_flight_blocked")
                }
                crate::pipeline::PipelineResult::Multi(m) => m
                    .searches
                    .iter()
                    .any(|b| b.error.as_deref() == Some("pre_flight_blocked")),
                crate::pipeline::PipelineResult::Stream(_) => false,
            };
            let total = output.total_results();

            // GAP-AUD-003 v0.8.0: causal classification of zero-result.
            // Stream variant returns false because stream emits incrementally
            // and the histogram per sub-query already carries the classification.
            let zero_cause_non_legitimo = match &output {
                crate::pipeline::PipelineResult::Single(s) => matches!(
                    s.metadata.zero_cause,
                    Some(crate::types::ZeroCause::GhostBlock)
                        | Some(crate::types::ZeroCause::AntiBot)
                        | Some(crate::types::ZeroCause::RespostaInvalida)
                        | Some(crate::types::ZeroCause::FiltroSilencioso)
                ),
                crate::pipeline::PipelineResult::Multi(m) => m.searches.iter().any(|b| {
                    matches!(
                        b.metadata.zero_cause,
                        Some(crate::types::ZeroCause::GhostBlock)
                            | Some(crate::types::ZeroCause::AntiBot)
                            | Some(crate::types::ZeroCause::RespostaInvalida)
                            | Some(crate::types::ZeroCause::FiltroSilencioso)
                    )
                }),
                crate::pipeline::PipelineResult::Stream(_) => false,
            };

            // BC opt-out: DUCKDUCKGO_ZERO_CAUSE_STRICT=false mapeia exit 6 → exit 5.
            // Default ON (strict). Aceita false/0/no/off como opt-out.
            let strict = parse_zero_cause_strict_env(std::env::var("DUCKDUCKGO_ZERO_CAUSE_STRICT").ok().as_deref());

            // GAP-AUD-005 + GAP-AUD-006 v0.8.0: reordenar lógica de exit code.
            // ANTES: pre_flight_blocked sempre saía com exit 3, ignorando a
            // BC opt-out. DEPOIS: pre_flight_blocked && !strict → exit 5
            // (legacy); pre_flight_blocked && strict → exit 3 (RATE_LIMITED).
            // Isso garante que pipelines de retry legacy continuam funcionando
            // quando opt-out ativo, mesmo quando pre-flight dispara.
            let exit_code = if pre_flight_blocked && !strict {
                tracing::warn!(
                    "pre-flight detected anti-bot block + BC opt-out; emitting exit 5 (ZERO_RESULTS)"
                );
                exit_codes::ZERO_RESULTS
            } else if pre_flight_blocked {
                tracing::warn!("pre-flight detected anti-bot block; emitting exit 3");
                exit_codes::RATE_LIMITED_OR_BLOCKED
            } else if total == 0 && strict && zero_cause_non_legitimo {
                tracing::warn!(
                    "Zero results with non-legitimo causa_zero; emitting exit 6 (SUSPECTED_BLOCK)"
                );
                tracing::warn!(
                    "  opt-out via DUCKDUCKGO_ZERO_CAUSE_STRICT=false to restore exit 5"
                );
                exit_codes::SUSPECTED_BLOCK
            } else if total == 0 {
                tracing::warn!("Zero results returned across all queries");
                exit_codes::ZERO_RESULTS
            } else {
                exit_codes::SUCCESS
            };

            if let Err(err) = output::emit_result(&output, format, output_file.as_deref()) {
                if output::is_broken_pipe(&err) {
                    // Pipe closed by consumer (e.g. `| jaq`, `| head`).
                    // Standard Unix behavior — exit 0 silently.
                    return exit_codes::SUCCESS;
                }
                tracing::error!(?err, "Failed to emit result");
                output::emit_stderr(&format!("Error writing output: {err:#}"));
                return exit_codes::GENERIC_ERROR;
            }

            exit_code
        }
        Err(err) => {
            tracing::error!(?err, "Pipeline execution failed");
            output::emit_stderr(&format!("Error: {err:#}"));
            exit_codes::GENERIC_ERROR
        }
    }
}

/// Executes the `deep-research` subcommand (v0.7.0).
///
/// Builds a default [`Config`] (15 results per sub-query, `--parallel`
/// inherited from the global `MAX_PARALLELISM` floor), then delegates to
/// [`crate::deep_research::run_deep_research`].
///
/// v0.7.10 B3 fix: takes `root_global_timeout_seconds` so the user's
/// `--global-timeout` (now global) is honored by this subcommand.
async fn execute_deep_research(
    args: crate::cli::DeepResearchArgs,
    root_global_timeout_seconds: u64,
    search_defaults: &crate::cli::CliArgs,
    allow_lite_fallback: bool,
    pre_flight: bool,
    identity_profile: crate::cli::CliIdentityProfile,
) -> i32 {
    use crate::deep_research::{run_deep_research, DeepResearchArgs as DrArgs};

    let require_results = args.require_results;
    let query_for_error = args.query.clone();

    let dr = DrArgs {
        query: args.query.clone(),
        max_sub_queries: args.max_sub_queries,
        sub_query_strategy: args.sub_query_strategy.into(),
        sub_queries_file: args.sub_queries_file.clone(),
        aggregation: args.aggregation.into(),
        depth: args.depth,
        fetch_content: args.fetch_content,
        synthesize: args.synthesize,
        budget_tokens: args.budget_tokens,
        synth_format: args.synth_format.into(),
    };

    let ua_list = http::load_user_agents(search_defaults.match_platform_ua);
    let browser_profile =
        http::select_profile_from_list_seeded(&ua_list, search_defaults.seed);
    let user_agent = browser_profile.user_agent.clone();
    let selectors = selectors::load_selectors();
    let effective_num = search_defaults.num_results.unwrap_or(15);
    let effective_endpoint = match search_defaults.endpoint {
        crate::cli::CliEndpoint::Html => Endpoint::Html,
        crate::cli::CliEndpoint::Lite => Endpoint::Lite,
    };

    let config = Config {
        query: dr.query.clone(),
        queries: vec![dr.query.clone()],
        num_results: Some(effective_num),
        format: OutputFormat::Json,
        timeout_seconds: search_defaults.timeout_seconds,
        language: search_defaults.language.clone(),
        country: search_defaults.country.clone(),
        pre_flight,
        verbose: search_defaults.verbose,
        quiet: search_defaults.quiet,
        user_agent,
        browser_profile,
        parallelism: search_defaults.parallelism,
        pages: 1,
        retries: search_defaults.retries,
        endpoint: effective_endpoint,
        time_filter: None,
        safe_search: SafeSearch::Moderate,
        stream_mode: false,
        output_file: None,
        fetch_content: dr.fetch_content,
        max_content_length: search_defaults.max_content_length,
        proxy: search_defaults.proxy.clone(),
        no_proxy: search_defaults.no_proxy,
        global_timeout_seconds: root_global_timeout_seconds,
        match_platform_ua: search_defaults.match_platform_ua,
        per_host_limit: search_defaults.per_host_limit as usize,
        chrome_path: search_defaults.chrome_path.clone(),
        selectors,
        cookie_provider: None,
        persistent_jar: None,
        warmup_enabled: false,
        allow_lite_fallback,
        identity_profile,
        last_probe_cascade_level: None,
    };

    let token = CancellationToken::new();
    let result = run_deep_research(dr, &config, token.clone()).await;

    match result {
        Ok(output) => {
            // v0.7.10 P4: when --require-results is set and the fan-out
            // aggregated zero results, surface an explicit error instead
            // of returning exit 0 with an empty payload. Closes the
            // GAP-WS-1114 silent-discard pattern.
            if require_results && output.metadata.unique_result_count == 0 {
                output::emit_stderr(&format!(
                    "deep-research produced zero results for query {:?}; \
                     --require-results set → exiting non-zero",
                    query_for_error
                ));
                return exit_codes::GLOBAL_TIMEOUT;
            }

            // Emit the report as JSON on stdout, single line.
            match serde_json::to_string(&output) {
                Ok(json) => match output::print_line_stdout(&json) {
                    Ok(()) => exit_codes::SUCCESS,
                    Err(CliError::BrokenPipe) => exit_codes::SUCCESS,
                    Err(err) => {
                        output::emit_stderr(&format!("stdout write failed: {err:#}"));
                        exit_codes::GENERIC_ERROR
                    }
                },
                Err(err) => {
                    output::emit_stderr(&format!("Error serializing deep-research output: {err}"));
                    exit_codes::GENERIC_ERROR
                }
            }
        }
        Err(err) => {
            output::emit_stderr(&format!("deep-research failed: {err:#}"));
            match err {
                CliError::InvalidConfig { .. } => exit_codes::INVALID_CONFIG,
                CliError::Cancelled => exit_codes::GLOBAL_TIMEOUT,
                _ => exit_codes::GENERIC_ERROR,
            }
        }
    }
}

/// Executes the `init-config` subcommand and prints the report in JSON format.
///
/// Returns `SUCESSO` if all files were processed (including skipped ones);
/// returns `ERRO_GENERICO` on fatal failure (e.g., config directory undetermined).
fn execute_init_config(args: InitConfigArgs) -> i32 {
    initialize_logging(0, false, false);
    platform::init();

    let report = match config_init::initialize_config(args.force, args.dry_run) {
        Ok(r) => r,
        Err(err) => {
            tracing::error!(?err, "failed to initialize config");
            output::emit_stderr(&format!("Error: {err:#}"));
            return exit_codes::GENERIC_ERROR;
        }
    };

    match serde_json::to_string_pretty(&report) {
        Ok(json) => {
            if let Err(err) = output::print_line_stdout(&json) {
                if output::is_broken_pipe(&err) {
                    return exit_codes::SUCCESS;
                }
                tracing::error!(?err, "failed to emit report");
                return exit_codes::GENERIC_ERROR;
            }
        }
        Err(err) => {
            tracing::error!(?err, "failed to serialize JSON report");
            return exit_codes::GENERIC_ERROR;
        }
    }

    // Was there an error in any individual file? Return a generic error regardless.
    let had_error = report.files.iter().any(|a| {
        matches!(
            a.action_taken,
            crate::config_init::ConfigFileAction::Error { .. }
        )
    });
    if had_error {
        return exit_codes::GENERIC_ERROR;
    }

    exit_codes::SUCCESS
}

/// Executes the v0.6.4 --probe pre-flight health check.
///
/// Sends ONE minimal GET request to the configured endpoint and emits a
/// JSON report on stdout with `status`, `latency_ms`, `has_set_cookie`,
/// `endpoint`, and `identity` fields. Exits 0 if the request succeeded
/// (any HTTP status, including 202/403/429 — the probe reports but does
/// not retry), 1 if the network/TLS/DNS layer failed.
async fn execute_probe(args: &crate::cli::CliArgs) -> i32 {
    use crate::error::exit_codes;
    use std::time::Instant;

    let endpoint = match args.endpoint {
        crate::cli::CliEndpoint::Html => "html",
        crate::cli::CliEndpoint::Lite => "lite",
    };
    let probe_url = if endpoint == "lite" {
        crate::search::lite_base_url()
    } else {
        crate::search::html_base_url()
    };

    // Build a minimal client. Use the same UA + Accept-Language defaults
    // the main pipeline uses (no --probe-specific profile).
    // Pick a User-Agent (rotated, seeded if --seed is set) — keeps probe
    // behavior consistent with the main pipeline.
    // Pick a User-Agent (rotated, seeded if --seed is set) — keeps probe
    // behavior consistent with the main pipeline.
    let ua = match args.seed {
        Some(seed) => {
            crate::http::select_profile_from_list_seeded(
                &crate::http::load_user_agents(args.match_platform_ua),
                Some(seed),
            )
            .user_agent
        }
        None => crate::http::select_user_agent(),
    };
    let client =
        match crate::http::build_client(&ua, args.timeout_seconds, &args.language, &args.country) {
            Ok(c) => c,
            Err(err) => {
                let payload = serde_json::json!({
                    "type": "probe",
                    "endpoint": endpoint,
                    "status": 0u16,
                    "latency_ms": 0u64,
                    "has_set_cookie": false,
                    "error": format!("client build failed: {err}"),
                });
                let _ = crate::output::print_line_stdout(&payload.to_string());
                return exit_codes::GENERIC_ERROR;
            }
        };

    let started = Instant::now();
    let result = client.get(&probe_url).send().await;
    let latency_ms = started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64;

    match result {
        Ok(response) => {
            let status = response.status().as_u16();
            let has_set_cookie = response.headers().contains_key("set-cookie");
            let payload = serde_json::json!({
                "type": "probe",
                "endpoint": endpoint,
                "status": status,
                "latency_ms": latency_ms,
                "has_set_cookie": has_set_cookie,
                "url": probe_url,
            });
            // Emit single JSON object to stdout.
            if let Err(err) = crate::output::print_line_stdout(&payload.to_string()) {
                if !crate::output::is_broken_pipe(&err) {
                    tracing::error!(?err, "failed to emit probe report");
                    return exit_codes::GENERIC_ERROR;
                }
            }
            // Probe succeeds on ANY HTTP response (even 202/403/429) — caller
            // decides what to do based on the status field.
            exit_codes::SUCCESS
        }
        Err(err) => {
            let payload = serde_json::json!({
                "type": "probe",
                "endpoint": endpoint,
                "status": 0u16,
                "latency_ms": latency_ms,
                "has_set_cookie": false,
                "url": probe_url,
                "error": format!("network error: {err}"),
            });
            let _ = crate::output::print_line_stdout(&payload.to_string());
            exit_codes::GENERIC_ERROR
        }
    }
}

/// Executes the v0.7.3 PR3 `--probe-deep` health check.
///
/// Runs one real query against the configured endpoint, reads the
/// response body, and classifies it as `captcha | ok` based on the
/// presence of Cloudflare or DDG bot-detection markers. Emits a JSON
/// report on stdout with `status`, `endpoint`, `cascade_level`,
/// `cascata_motivo`, and `sugestao_mitigacao`. Exits 0 on success
/// (including when the probe detected a captcha — the caller is
/// expected to act on the JSON), 1 on network failure.
async fn execute_probe_deep(args: &crate::cli::CliArgs) -> i32 {
    use crate::error::exit_codes;
    use crate::probe_deep::{
        detectar_interstitial_com_match, sugestao_mitigacao_com_marker, InterstitialKind,
    };
    use std::time::Instant;

    let endpoint = match args.endpoint {
        crate::cli::CliEndpoint::Html => "html",
        crate::cli::CliEndpoint::Lite => "lite",
    };
    let probe_url = if endpoint == "lite" {
        crate::search::lite_base_url()
    } else {
        crate::search::html_base_url()
    };

    // Build a minimal client. The deep probe does not use the persistent
    // cookie jar or the warm-up — it tests the endpoint raw.
    let ua = crate::http::select_user_agent();
    let client =
        match crate::http::build_client(&ua, args.timeout_seconds, &args.language, &args.country) {
            Ok(c) => c,
            Err(err) => {
                let payload = serde_json::json!({
                    "type": "probe_deep",
                    "endpoint": endpoint,
                    "status": "error",
                    "error": format!("client build failed: {err}"),
                });
                let _ = output::print_line_stdout(&payload.to_string());
                return exit_codes::GENERIC_ERROR;
            }
        };

    // Build a minimal form with just `q=`. The HTML endpoint requires
    // POST with a form body, so we send a one-field form.
    let form_data: Vec<(String, String)> =
        vec![("q".to_string(), PROBE_CALIBRATION_QUERY.to_string())];
    let started = Instant::now();
    let result = client.post(&probe_url).form(&form_data).send().await;
    let latency_ms = started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64;

    match result {
        Ok(response) => {
            let status = response.status().as_u16();
            let body = crate::decompress::response_body_string(response)
                .await
                .unwrap_or_default();
            let (marker, kind) = detectar_interstitial_com_match(&body);
            let status_str = match kind {
                InterstitialKind::None => "ok",
                _ => "captcha",
            };
            let payload = serde_json::json!({
                "type": "probe_deep",
                "endpoint": endpoint,
                "status": status_str,
                "http_status": status,
                "latency_ms": latency_ms,
                "cascade_level": 0,
                "cascata_motivo": kind.as_str(),
                "sugestao_mitigacao": sugestao_mitigacao_com_marker(kind, marker),
                "url": probe_url,
            });
            if let Err(err) = output::print_line_stdout(&payload.to_string()) {
                if !output::is_broken_pipe(&err) {
                    tracing::error!(?err, "failed to emit probe_deep report");
                    return exit_codes::GENERIC_ERROR;
                }
            }
            // B4 fix: when the probe detects a captcha / interstitial,
            // surface exit 3 (DuckDuckGo 202 block anomaly) so consumers
            // can branch on the exit code instead of parsing the JSON
            // status field. The JSON payload above already carries
            // `status: "captcha"` and the marker hint for downstream use.
            if kind != InterstitialKind::None {
                exit_codes::RATE_LIMITED_OR_BLOCKED
            } else {
                exit_codes::SUCCESS
            }
        }
        Err(err) => {
            let payload = serde_json::json!({
                "type": "probe_deep",
                "endpoint": endpoint,
                "status": "error",
                "latency_ms": latency_ms,
                "error": format!("network error: {err}"),
            });
            let _ = output::print_line_stdout(&payload.to_string());
            exit_codes::GENERIC_ERROR
        }
    }
}

/// Executes the `completions` subcommand — generates shell completion scripts.
fn execute_completions(args: CompletionsArgs) -> i32 {
    use clap::CommandFactory;
    let mut cmd = RootArgs::command();
    clap_complete::generate(
        args.shell,
        &mut cmd,
        "duckduckgo-search-cli",
        &mut std::io::stdout(),
    );
    exit_codes::SUCCESS
}

/// Initializes the tracing subscriber writing to stderr.
///
/// - `--quiet` → `ERROR` only.
/// - `verbose == 0` → `INFO` (default), respects `RUST_LOG` when set.
/// - `verbose == 1` → `DEBUG`, respects `RUST_LOG` when set.
/// - `verbose >= 2` → `TRACE`, respects `RUST_LOG` when set.
fn initialize_logging(verbose: u8, quiet: bool, disable_colors: bool) {
    // GAP-NEW-001 v0.8.0: detect timeout-cli Rust wrapper that shadows GNU
    // coreutils  and breaks  flag parsing for the subprocess.
    // The Rust wrapper sets CARGO_BIN_EXE_timeout when invoked via
    // ; for end users running the installed binary under the
    // wrapper, the env var is propagated by the wrapper itself.
    if std::env::var_os("CARGO_BIN_EXE_timeout").is_some() {
        tracing::warn!(
            "timeout-cli Rust crate detected as parent process;              use /usr/bin/timeout GNU coreutils to avoid -v flag interception.              Run scripts/detect-timeout-wrapper.sh to verify."
        );
    }

    let filter = if quiet {
        EnvFilter::new("error")
    } else if verbose >= 2 {
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("trace"))
    } else if verbose >= 1 {
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("debug"))
    } else {
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"))
    };

    let subscriber = fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .with_target(false)
        .with_ansi(!disable_colors)
        .compact()
        .finish();

    let _ = tracing::subscriber::set_global_default(subscriber);
}

/// Converts raw CLI arguments into validated `Config`.
///
/// Combines queries from: (1) positional arguments, (2) file via
/// `--queries-file`, (3) stdin when it is not a TTY. Deduplicates while
/// preserving the order of first occurrence.
fn build_config(args: &CliArgs) -> Result<Config, CliError> {
    let format =
        OutputFormat::from_str_value(&args.format).ok_or_else(|| CliError::InvalidConfig {
            message: format!("unknown format: {:?}", args.format),
        })?;

    args.validate_parallelism()
        .map_err(|e| CliError::InvalidConfig { message: e })?;
    args.validate_pages()
        .map_err(|e| CliError::InvalidConfig { message: e })?;
    args.validate_retries()
        .map_err(|e| CliError::InvalidConfig { message: e })?;
    args.validate_max_content_length()
        .map_err(|e| CliError::InvalidConfig { message: e })?;
    // v0.7.10 B3 fix: `global_timeout_seconds` validation happens on
    // `RootArgs` in `run()`. The unit tests that call `build_config`
    // directly bypass `run`, so they exercise the default
    // `DEFAULT_GLOBAL_TIMEOUT` which always validates as `Ok`.
    let _ = crate::cli::DEFAULT_GLOBAL_TIMEOUT;
    args.validate_proxy()
        .map_err(|e| CliError::InvalidConfig { message: e })?;
    args.validate_per_host_limit()
        .map_err(|e| CliError::InvalidConfig { message: e })?;
    args.validate_timeout_seconds()
        .map_err(|e| CliError::InvalidConfig { message: e })?;
    if let Some(path) = &args.output_file {
        crate::paths::validate_output_path(path)?;
    }

    let file_queries = match &args.queries_file {
        Some(path) => pipeline::read_queries_from_file(path)?,
        None => Vec::new(),
    };

    let queries_stdin = if args.queries.is_empty() && args.queries_file.is_none() {
        pipeline::read_queries_from_stdin_if_pipe()?
    } else {
        Vec::new()
    };

    let queries =
        pipeline::combine_and_dedup_queries(args.queries.clone(), file_queries, queries_stdin);

    if queries.is_empty() {
        return Err(CliError::InvalidConfig {
            message:
                "no query provided (positional arguments, --queries-file, and stdin are all empty)"
                    .into(),
        });
    }

    let first_query = queries[0].clone();

    // Load UA list — tries external file, falls back to embedded defaults.
    let ua_list = http::load_user_agents(args.match_platform_ua);
    let browser_profile = http::select_profile_from_list_seeded(&ua_list, args.seed);
    let user_agent = browser_profile.user_agent.clone();

    // Load CSS selectors — tries external TOML file, falls back to embedded defaults.
    // --config overrides the default config directory.
    let selectors = if let Some(ref dir) = args.config_path {
        selectors::load_selectors_from_dir(dir)
    } else {
        selectors::load_selectors()
    };

    // --- Default for --num and auto-pagination (v0.4.0) ---
    //
    // Semantics (decided in v0.4.0):
    // - If the user does NOT pass `--num`, we use 15 as the effective default.
    // - If the effective `num` is > 10 and the user did NOT customize `--pages`
    //   (i.e., `paginas == 1`, which is the clap default), we auto-raise
    //   `paginas` to `ceil(num/10)`, capped at 5 (MAX_PAGES
    //   validated in `validar_paginas`).
    // - If the user passes `--pages > 1` explicitly, we RESPECT that value
    //   without overriding (edge case: `--pages 1` explicit is
    //   indistinguishable from the default; accepted trade-off).
    let effective_num = args.num_results.unwrap_or(15);
    let effective_pages = if args.pages > 1 {
        args.pages
    } else if effective_num > 10 {
        effective_num.div_ceil(10).min(5)
    } else {
        1
    };

    // v0.7.3 PR2: build the cookie jar / warm-up machinery.
    let (persistent_jar, warmup_enabled) = if args.no_cookie_persistence {
        (
            crate::wreq_cookie_adapter::PersistentJar::empty(None),
            !args.no_warmup,
        )
    } else {
        let path = match args.cookies_path.as_ref() {
            Some(p) => p.clone(),
            None => crate::wreq_cookie_adapter::default_cookies_path()?,
        };
        (
            crate::wreq_cookie_adapter::PersistentJar::load(Some(path)),
            !args.no_warmup,
        )
    };
    let cookie_provider = persistent_jar.as_provider();

    Ok(Config {
        query: first_query,
        queries,
        num_results: Some(effective_num),
        format,
        timeout_seconds: args.timeout_seconds,
        language: args.language.clone(),
        allow_lite_fallback: false,
        pre_flight: false,
        last_probe_cascade_level: None,
        country: args.country.clone(),
        verbose: args.verbose,
        quiet: args.quiet,
        user_agent,
        browser_profile,
        parallelism: args.parallelism,
        pages: effective_pages,
        retries: args.retries,
        endpoint: convert_endpoint(args.endpoint),
        time_filter: args.time_filter.map(convert_time_filter),
        safe_search: convert_safe_search(args.safe_search),
        stream_mode: args.stream_mode,
        output_file: args.output_file.clone(),
        fetch_content: args.fetch_content,
        max_content_length: args.max_content_length,
        proxy: args.proxy.clone(),
        no_proxy: args.no_proxy,
        // v0.7.10 B3 fix: `global_timeout_seconds` lives on `RootArgs`,
        // not on `CliArgs`. The caller (`run`) hoists the value and
        // overrides this field right after `build_config` returns.
        // The default below only runs in unit tests that bypass `run`.
        global_timeout_seconds: crate::cli::DEFAULT_GLOBAL_TIMEOUT,
        match_platform_ua: args.match_platform_ua,
        per_host_limit: args.per_host_limit as usize,
        chrome_path: args.chrome_path.clone(),
        selectors,
        cookie_provider: Some(cookie_provider),
        persistent_jar: Some(persistent_jar),
        warmup_enabled,
        identity_profile: args.identity_profile,
    })
}

/// Converts the `CliEndpoint` enum (clap) into the internal `Endpoint` type.
fn convert_endpoint(source: CliEndpoint) -> Endpoint {
    match source {
        CliEndpoint::Html => Endpoint::Html,
        CliEndpoint::Lite => Endpoint::Lite,
    }
}

/// Converts the `CliTimeFilter` enum (clap) into the internal `TimeFilter` type.
fn convert_time_filter(source: CliTimeFilter) -> TimeFilter {
    match source {
        CliTimeFilter::D => TimeFilter::Day,
        CliTimeFilter::W => TimeFilter::Week,
        CliTimeFilter::M => TimeFilter::Month,
        CliTimeFilter::Y => TimeFilter::Year,
    }
}

/// Converts the `CliSafeSearch` enum (clap) into the internal `SafeSearch` type.
fn convert_safe_search(source: CliSafeSearch) -> SafeSearch {
    match source {
        CliSafeSearch::Off => SafeSearch::Off,
        CliSafeSearch::Moderate => SafeSearch::Moderate,
        CliSafeSearch::On => SafeSearch::Strict,
    }
}

/// Parses the `DUCKDUCKGO_ZERO_CAUSE_STRICT` env var to decide whether
/// zero-result queries with a non-`Legitimo` causa should emit exit code
/// 6 (`SUSPECTED_BLOCK`) or fall back to exit code 5 (legacy `ZERO_RESULTS`).
///
/// Pure function so unit tests can validate the opt-out contract
/// without touching the process environment. Accepts the conventional
/// falsy spellings `false`, `0`, `no`, `off`, and the empty string.
/// Any other value (including `true`) is treated as strict. When the
/// env var is missing, strict is the default (v0.8.0 introduces this
/// exit code — pre-v0.8 callers must opt out explicitly to preserve
/// v0.7.x behavior).
fn parse_zero_cause_strict_env(value: Option<&str>) -> bool {
    match value {
        None => true,
        Some(v) => !matches!(v, "false" | "0" | "no" | "off" | ""),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base_args() -> CliArgs {
        CliArgs {
            queries: vec!["rust async".to_string()],
            num_results: Some(5),
            format: "json".to_string(),
            output_file: None,
            timeout_seconds: 15,
            language: "pt".to_string(),
            country: "br".to_string(),
            parallelism: 5,
            queries_file: None,
            pages: 1,
            retries: 2,
            endpoint: CliEndpoint::Html,
            time_filter: None,
            safe_search: CliSafeSearch::Moderate,
            probe: false,
            identity_profile: crate::cli::CliIdentityProfile::Auto,
            stream_mode: false,
            verbose: 0,
            quiet: false,
            fetch_content: false,
            max_content_length: crate::cli::DEFAULT_MAX_CONTENT_LENGTH,
            proxy: None,
            no_proxy: false,
            // v0.7.10 B3 fix: `global_timeout_seconds` is no longer on
            // `CliArgs`; it lives on `RootArgs` and is hoisted in `run`.
            match_platform_ua: false,
            per_host_limit: crate::cli::DEFAULT_PER_HOST_LIMIT,
            chrome_path: None,
            no_color: false,
            seed: None,
            config_path: None,
            no_warmup: false,
            no_cookie_persistence: false,
            cookies_path: None,
            probe_deep: false,
        }
    }

    #[test]
    fn build_config_with_valid_args() {
        let args = base_args();
        let cfg = build_config(&args).expect("should build config");
        assert_eq!(cfg.query, "rust async");
        assert_eq!(cfg.queries, vec!["rust async".to_string()]);
        assert_eq!(cfg.format, OutputFormat::Json);
        assert_eq!(cfg.num_results, Some(5));
        assert_eq!(cfg.parallelism, 5);
        assert_eq!(cfg.pages, 1);
        assert!(!cfg.stream_mode);
    }

    // v0.7.10 GAP-WS-60 regression: `build_config` must propagate
    // `args.identity_profile` into `Config.identity_profile` so the
    // pipeline can pin to a fixed identity.
    #[test]
    fn build_config_propagates_identity_profile_default_auto() {
        let args = base_args();
        let cfg = build_config(&args).expect("should build config");
        assert_eq!(
            cfg.identity_profile,
            crate::cli::CliIdentityProfile::Auto,
            "default identity_profile must be Auto"
        );
    }

    #[test]
    fn build_config_propagates_identity_profile_chrome_linux() {
        let mut args = base_args();
        args.identity_profile = crate::cli::CliIdentityProfile::ChromeLinux;
        let cfg = build_config(&args).expect("should build config");
        assert_eq!(
            cfg.identity_profile,
            crate::cli::CliIdentityProfile::ChromeLinux,
            "ChromeLinux flag must reach Config"
        );
    }

    #[test]
    fn build_config_rejects_all_empty_queries() {
        let mut args = base_args();
        args.queries = vec!["   ".to_string(), "".to_string()];
        let result = build_config(&args);
        assert!(result.is_err());
    }

    #[test]
    fn build_config_rejects_unknown_format() {
        let mut args = base_args();
        args.format = "xml".to_string();
        assert!(build_config(&args).is_err());
    }

    #[test]
    fn build_config_rejects_zero_parallelism() {
        let mut args = base_args();
        args.parallelism = 0;
        assert!(build_config(&args).is_err());
    }

    #[test]
    fn build_config_rejects_parallelism_above_max() {
        let mut args = base_args();
        args.parallelism = 50;
        assert!(build_config(&args).is_err());
    }

    #[test]
    fn build_config_applies_default_num_15_when_omitted() {
        // v0.4.0: when `--num` is omitted (None), the effective default is 15
        // and this auto-raises `--pages` to 2 (since 15 > 10 and pages=1 is the default).
        let mut args = base_args();
        args.num_results = None;
        args.pages = 1;
        let cfg = build_config(&args).expect("should build");
        assert_eq!(cfg.num_results, Some(15), "default 15 quando None");
        assert_eq!(cfg.pages, 2, "auto-eleva para ceil(15/10) = 2");
    }

    #[test]
    fn build_config_respects_explicit_pages_above_1() {
        // If the user passes `--pages 3` explicitly, do NOT override with
        // auto-pagination, even if the effective num would require fewer.
        let mut args = base_args();
        args.num_results = Some(20);
        args.pages = 3;
        let cfg = build_config(&args).expect("should build");
        assert_eq!(cfg.num_results, Some(20));
        assert_eq!(cfg.pages, 3, "respeita --pages explícito do usuário");
    }

    #[test]
    fn build_config_auto_paginates_when_num_above_10() {
        // Casos de fronteira do auto-paginador.
        let casos = [
            (11u32, 2u32), // ceil(11/10) = 2
            (15, 2),       // ceil(15/10) = 2
            (20, 2),       // ceil(20/10) = 2
            (21, 3),       // ceil(21/10) = 3
            (45, 5),       // ceil(45/10) = 5
            (60, 5),       // ceil(60/10) = 6 mas clamp em 5
        ];
        for (num, expected_pages) in casos {
            let mut args = base_args();
            args.num_results = Some(num);
            args.pages = 1;
            let cfg =
                build_config(&args).unwrap_or_else(|e| panic!("should build for num={num}: {e}"));
            assert_eq!(
                cfg.pages, expected_pages,
                "para num={num}, paginas deveria ser {expected_pages}"
            );
        }
    }

    #[test]
    fn build_config_no_auto_paginate_when_num_10_or_less() {
        // If effective num <= 10, keep paginas=1 (no auto-pagination).
        for num in [1u32, 5, 10] {
            let mut args = base_args();
            args.num_results = Some(num);
            args.pages = 1;
            let cfg = build_config(&args).expect("should build");
            assert_eq!(cfg.pages, 1, "num={num} não deveria auto-paginar");
        }
    }

    #[test]
    fn build_config_combines_multiple_positional_queries() {
        let mut args = base_args();
        args.queries = vec![
            "alfa".to_string(),
            "beta".to_string(),
            "alfa".to_string(), // duplicata
            "gama".to_string(),
        ];
        let cfg = build_config(&args).expect("should build config");
        assert_eq!(cfg.queries, vec!["alfa", "beta", "gama"]);
        assert_eq!(cfg.query, "alfa");
    }

    // --- v0.8.0 Bug #2 BC opt-out regression coverage ---

    #[test]
    fn parse_zero_cause_strict_env_missing_defaults_to_strict() {
        // Env var not set → strict mode (default for v0.8.0+).
        assert!(parse_zero_cause_strict_env(None));
    }

    #[test]
    fn parse_zero_cause_strict_env_explicit_true_remains_strict() {
        assert!(parse_zero_cause_strict_env(Some("true")));
    }

    #[test]
    fn parse_zero_cause_strict_env_arbitrary_value_remains_strict() {
        // Unknown spellings MUST NOT accidentally trigger opt-out.
        assert!(parse_zero_cause_strict_env(Some("yes")));
        assert!(parse_zero_cause_strict_env(Some("1")));
        assert!(parse_zero_cause_strict_env(Some("enabled")));
    }

    #[test]
    fn parse_zero_cause_strict_env_false_triggers_opt_out() {
        assert!(!parse_zero_cause_strict_env(Some("false")));
    }

    #[test]
    fn parse_zero_cause_strict_env_zero_triggers_opt_out() {
        assert!(!parse_zero_cause_strict_env(Some("0")));
    }

    #[test]
    fn parse_zero_cause_strict_env_no_triggers_opt_out() {
        assert!(!parse_zero_cause_strict_env(Some("no")));
    }

    #[test]
    fn parse_zero_cause_strict_env_off_triggers_opt_out() {
        assert!(!parse_zero_cause_strict_env(Some("off")));
    }

    #[test]
    fn parse_zero_cause_strict_env_empty_string_triggers_opt_out() {
        // Empty value is treated as opt-out (mirrors shell unset semantics
        // where a typo like `DUCKDUCKGO_ZERO_CAUSE_STRICT=` should not
        // accidentally lock the caller into strict mode).
        assert!(!parse_zero_cause_strict_env(Some("")));
    }

    #[test]
    fn parse_zero_cause_strict_env_case_sensitive() {
        // Mixed case spellings of "False" / "OFF" / "No" do NOT trigger
        // opt-out. This matches shell convention where case matters and
        // avoids accidentally disabling strict mode.
        assert!(parse_zero_cause_strict_env(Some("False")));
        assert!(parse_zero_cause_strict_env(Some("OFF")));
        assert!(parse_zero_cause_strict_env(Some("No")));
    }
}
