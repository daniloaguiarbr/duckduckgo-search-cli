// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload: orchestrator (config assembly, delegation to pipeline)
#![doc(html_root_url = "https://docs.rs/duckduckgo-search-cli/0.7.3")]
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
    if let Some(Subcommand::DeepResearch(dr_args)) = root.subcomando {
        return execute_deep_research(dr_args).await;
    }
    let args = match root.subcomando {
        Some(Subcommand::InitConfig(args)) => {
            return execute_init_config(args);
        }
        Some(Subcommand::Completions(args)) => {
            return execute_completions(args);
        }
        Some(Subcommand::Buscar(args)) => *args,
        Some(Subcommand::DeepResearch(_)) => unreachable!("handled above"),
        None => root.buscar,
    };

    // Initialize logging to stderr (before any operation that might emit logs).
    let disable_colors = platform::should_disable_color(args.no_color);
    initialize_logging(args.verbose, args.quiet, disable_colors);

    // Initialize platform (UTF-8 on Windows, etc.).
    platform::init();

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
    let config = match build_config(&args) {
        Ok(c) => c,
        Err(err) => {
            tracing::error!(?err, "Invalid configuration");
            output::emit_stderr(&format!("Configuration error: {err:#}"));
            return exit_codes::INVALID_CONFIG;
        }
    };

    let format = config.format;
    let output_file = config.output_file.clone();
    let global_timeout = std::time::Duration::from_secs(config.global_timeout_seconds);

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
            let total = output.total_results();
            let exit_code = if total == 0 {
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
async fn execute_deep_research(args: crate::cli::DeepResearchArgs) -> i32 {
    use crate::cli::DEFAULT_PARALLELISM;
    use crate::deep_research::{run_deep_research, DeepResearchArgs as DrArgs};

    initialize_logging(false, false, false);
    platform::init();

    // Translate the CLI struct into the library-level struct.
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

    // Build a default config for the sub-queries. The deep-research pipeline
    // does not need a file output or a custom format — it always emits JSON
    // via stdout so LLMs can consume it directly.
    let ua_list = http::load_user_agents(false);
    let browser_profile = http::select_profile_from_list_seeded(&ua_list, None);
    let user_agent = browser_profile.user_agent.clone();
    let selectors = selectors::load_selectors();

    let config = Config {
        query: dr.query.clone(),
        queries: vec![dr.query.clone()],
        num_results: Some(10),
        format: OutputFormat::Json,
        timeout_seconds: 15,
        language: "en".to_string(),
        country: "us".to_string(),
        verbose: false,
        quiet: false,
        user_agent,
        browser_profile,
        parallelism: DEFAULT_PARALLELISM,
        pages: 1,
        retries: 2,
        endpoint: Endpoint::Html,
        time_filter: None,
        safe_search: SafeSearch::Moderate,
        stream_mode: false,
        output_file: None,
        fetch_content: dr.fetch_content,
        max_content_length: 10_000,
        proxy: None,
        no_proxy: false,
        global_timeout_seconds: 120,
        match_platform_ua: false,
        per_host_limit: 2,
        chrome_path: None,
        selectors,
        // Deep research uses an in-memory cookie jar with no warm-up.
        cookie_provider: None,
        persistent_jar: None,
        warmup_enabled: false,
        allow_lite_fallback: false,
    };

    let token = CancellationToken::new();
    let result = run_deep_research(dr, &config, token.clone()).await;

    match result {
        Ok(output) => {
            // Emit the report as JSON on stdout, single line.
            match serde_json::to_string(&output) {
                Ok(json) => {
                    println!("{json}");
                    exit_codes::SUCCESS
                }
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
    initialize_logging(false, false, false);
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
    use crate::probe_deep::{detectar_interstitial, sugestao_mitigacao, InterstitialKind};
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
    let form_data: Vec<(String, String)> = vec![("q".to_string(), "rust".to_string())];
    let started = Instant::now();
    let result = client.post(&probe_url).form(&form_data).send().await;
    let latency_ms = started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64;

    match result {
        Ok(response) => {
            let status = response.status().as_u16();
            let body = response.text().await.unwrap_or_default();
            let kind = detectar_interstitial(&body);
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
                "sugestao_mitigacao": sugestao_mitigacao(kind),
                "url": probe_url,
            });
            if let Err(err) = output::print_line_stdout(&payload.to_string()) {
                if !output::is_broken_pipe(&err) {
                    tracing::error!(?err, "failed to emit probe_deep report");
                    return exit_codes::GENERIC_ERROR;
                }
            }
            exit_codes::SUCCESS
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
/// - `--verbose` → `DEBUG` and above.
/// - Default → `INFO` and above (but respects `RUST_LOG` if set).
fn initialize_logging(verbose: bool, quiet: bool, disable_colors: bool) {
    let filter = if quiet {
        EnvFilter::new("error")
    } else if verbose {
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
    args.validate_global_timeout()
        .map_err(|e| CliError::InvalidConfig { message: e })?;
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
        global_timeout_seconds: args.global_timeout_seconds,
        match_platform_ua: args.match_platform_ua,
        per_host_limit: args.per_host_limit as usize,
        chrome_path: args.chrome_path.clone(),
        selectors,
        cookie_provider: Some(cookie_provider),
        persistent_jar: Some(persistent_jar),
        warmup_enabled,
        allow_lite_fallback: args.allow_lite_fallback,
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
            verbose: false,
            quiet: false,
            fetch_content: false,
            max_content_length: crate::cli::DEFAULT_MAX_CONTENT_LENGTH,
            proxy: None,
            no_proxy: false,
            global_timeout_seconds: crate::cli::DEFAULT_GLOBAL_TIMEOUT,
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
            allow_lite_fallback: false,
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
}
