// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload: declarative (CLI parsing via clap derive, zero runtime)
//! CLI argument definitions via `clap` derive.
//!
//! This module contains ONLY declarative clap structs. ZERO business logic.
//! Conversion of `CliArgs` into `Config` used by the pipeline occurs
//! in the `lib.rs` module (`run` function).
//!
//! In iteration 6 the `init-config` subcommand was added — backward-compatible,
//! since when no subcommand is passed, the previous search behavior is preserved
//! via `#[command(subcommand)]` with `Option<Subcommand>`.

use clap::{Args, Parser, Subcommand as ClapSubcommand, ValueEnum};
use std::path::PathBuf;

// Shell completion generation (MP-04).
pub use clap_complete::Shell as CompletionShell;

/// Default value for `--per-host-limit` (concurrent fetches per host in `--fetch-content`).
pub const DEFAULT_PER_HOST_LIMIT: u32 = 2;
/// Hard upper bound for `--per-host-limit`.
pub const MAX_PER_HOST_LIMIT: u32 = 10;

/// Hard upper bound for parallelism degree, per sections 4.2 and 17.4.
pub const MAX_PARALLELISM: u32 = 20;

/// Default parallelism degree when the user does not specify `-p`.
pub const DEFAULT_PARALLELISM: u32 = 5;

/// Hard upper bound for the number of pages (avoids expensive loops).
pub const MAX_PAGES: u32 = 5;

/// Hard upper bound for retries (avoids infinite-429 hangs).
pub const MAX_RETRIES: u32 = 10;

/// Default value for `--max-content-length` (characters of extracted page text).
pub const DEFAULT_MAX_CONTENT_LENGTH: usize = 10_000;
/// Hard upper bound for `--max-content-length` (`100_000` chars ~100 KB of clean text).
pub const MAX_CONTENT_LENGTH_LIMIT: usize = 100_000;

/// Default value for `--global-timeout` in seconds.
pub const DEFAULT_GLOBAL_TIMEOUT: u64 = 60;
/// Hard upper bound for `--global-timeout` (1 hour).
pub const MAX_GLOBAL_TIMEOUT: u64 = 3600;

/// Selectable `DuckDuckGo` endpoint via `--endpoint`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum CliEndpoint {
    /// Full HTML endpoint (`html.duckduckgo.com`).
    Html,
    /// Lightweight endpoint (`lite.duckduckgo.com`).
    Lite,
}

/// Time filter accepted by `--time-filter` (DDG `df` parameter).
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum CliTimeFilter {
    /// Last day.
    D,
    /// Last week.
    W,
    /// Last month.
    M,
    /// Last year.
    Y,
}

/// Safe-search accepted by `--safe-search` (DDG `kp` parameter).
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum CliSafeSearch {
    /// Disable all content filters.
    Off,
    /// DDG default moderate filtering.
    Moderate,
    /// Strict filtering of adult content.
    On,
}

/// Browser identity profile accepted by `--identity-profile`.
///
/// `Auto` (default) selects from the 12-identity pool adaptively, rotating on
/// detected blocks. The other variants pin the session to a single identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum CliIdentityProfile {
    /// Adaptive selection from the 12-identity pool (default).
    Auto,
    /// Chrome on Windows.
    ChromeWin,
    /// Chrome on macOS.
    ChromeMac,
    /// Chrome on Linux.
    ChromeLinux,
    /// Edge on Windows.
    EdgeWin,
    /// Firefox on Linux.
    FirefoxLinux,
    /// Safari on macOS.
    SafariMac,
}

impl CliIdentityProfile {
    /// Returns the family and platform tuple for the profile, or `None` for `Auto`.
    pub fn family_and_platform(
        self,
    ) -> Option<(crate::identity::BrowserFamily, crate::identity::Platform)> {
        use crate::identity::{BrowserFamily, Platform};
        match self {
            Self::Auto => None,
            Self::ChromeWin => Some((BrowserFamily::Chrome, Platform::Windows)),
            Self::ChromeMac => Some((BrowserFamily::Chrome, Platform::MacOS)),
            Self::ChromeLinux => Some((BrowserFamily::Chrome, Platform::Linux)),
            Self::EdgeWin => Some((BrowserFamily::Edge, Platform::Windows)),
            Self::FirefoxLinux => Some((BrowserFamily::Firefox, Platform::Linux)),
            Self::SafariMac => Some((BrowserFamily::Safari, Platform::MacOS)),
        }
    }
}

/// CLI for searching `DuckDuckGo` via pure HTTP, with structured output for LLM consumption.
///
/// Root accepts an optional subcommand. When no subcommand is passed, the
/// default behavior is `buscar` — maintains full backward compatibility with
/// previous versions of the CLI.
#[derive(Debug, Clone, Parser)]
#[command(
    name = "duckduckgo-search-cli",
    version,
    about = "DuckDuckGo search via pure HTTP, JSON output for LLMs.",
    long_about = "Rust CLI that queries the static DuckDuckGo HTML endpoint \
                  (https://html.duckduckgo.com/html/) using pure HTTP requests, \
                  no Chrome, no paid APIs, and no cache. Returns structured organic \
                  results as JSON ready for LLM consumption.",
    after_long_help = "\
EXIT CODES:\n\
    0    Success — at least one query returned results\n\
    1    Runtime error (network, parse, I/O)\n\
    2    Invalid configuration (flag out of range, bad proxy)\n\
    3    DuckDuckGo 202 block anomaly (soft-rate-limit)\n\
    4    Global timeout exceeded\n\
    5    Zero results across all queries\n\
\n\
PIPE USAGE:\n\
    duckduckgo-search-cli -q -f json \"query\" | jaq '.resultados[].url'\n\
    Logs go to stderr (-q suppresses them). JSON goes to stdout."
)]
pub struct RootArgs {
    /// Optional subcommand (`init-config`). No subcommand = search (default).
    #[command(subcommand)]
    pub subcomando: Option<Subcommand>,

    /// Search arguments (also accepted without a subcommand for backward compatibility).
    #[command(flatten)]
    pub buscar: CliArgs,
}

/// Supported subcommands. Chosen architecture: `Option<Subcommand>` at the root
/// allows invocation without a subcommand (direct search) OR with an explicit subcommand.
///
/// `Buscar` is `Box`ed to avoid a large enum variant (`CliArgs` has
/// many clap-derived fields).
#[derive(Debug, Clone, ClapSubcommand)]
pub enum Subcommand {
    /// Search on `DuckDuckGo` (equivalent to the no-subcommand mode).
    Buscar(Box<CliArgs>),
    /// Initializes configuration files (`selectors.toml`, `user-agents.toml`)
    /// in the default OS configuration directory.
    InitConfig(InitConfigArgs),
    /// Generates shell completion scripts for the specified shell.
    Completions(CompletionsArgs),
    /// Runs a deep research pipeline: query fan-out, aggregation, and
    /// optional synthesis into a Markdown/PlainText/Json report.
    DeepResearch(DeepResearchArgs),
}

/// Arguments for the `deep-research` subcommand (v0.7.0).
#[derive(Debug, Clone, Args)]
pub struct DeepResearchArgs {
    /// The original user query to research.
    #[arg(value_name = "QUERY")]
    pub query: String,

    /// Maximum number of sub-queries to produce by decomposition (1..=12, default 5).
    #[arg(
        long = "max-sub-queries",
        value_name = "N",
        default_value_t = crate::deep_research::DEFAULT_MAX_SUB_QUERIES
    )]
    pub max_sub_queries: usize,

    /// Decomposition strategy: `heuristic` (5 templates, default) or `manual`.
    #[arg(
        long = "sub-query-strategy",
        value_enum,
        default_value_t = CliSubQueryStrategy::Heuristic
    )]
    pub sub_query_strategy: CliSubQueryStrategy,

    /// File with one sub-query per line (only used with `--sub-query-strategy manual`).
    #[arg(long = "sub-queries-file", value_name = "PATH")]
    pub sub_queries_file: Option<PathBuf>,

    /// Aggregation strategy: `rrf` (default, K=60) or `dedupe-by-url`.
    #[arg(
        long = "aggregate",
        value_enum,
        default_value_t = CliAggregationStrategy::Rrf
    )]
    pub aggregation: CliAggregationStrategy,

    /// Reflection depth (0..=3). 0 = single pass. Each round plans a
    /// follow-up sub-query from the top results; v0.7.0 plans but does
    /// not execute the follow-up.
    #[arg(long = "depth", value_name = "N", default_value_t = 0)]
    pub depth: u32,

    /// Enables content extraction from the top-K aggregated URLs.
    #[arg(long = "fetch-content")]
    pub fetch_content: bool,

    /// Produces a synthesised report at the end of the pipeline.
    #[arg(long = "synthesize")]
    pub synthesize: bool,

    /// Approximate token budget for the synthesised report (default 4000).
    /// 1 token ≈ 4 characters (English text heuristic).
    #[arg(long = "budget-tokens", value_name = "N", default_value_t = 4000)]
    pub budget_tokens: usize,

    /// Format of the synthesised report.
    #[arg(
        long = "synth-format",
        value_enum,
        default_value_t = CliSynthFormat::Markdown
    )]
    pub synth_format: CliSynthFormat,
}

/// CLI wrapper for the decomposition strategy enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum CliSubQueryStrategy {
    /// Heuristic fan-out using 5 canonical templates (default).
    Heuristic,
    /// Read sub-queries from a file or stdin.
    Manual,
}

/// CLI wrapper for the aggregation strategy enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum CliAggregationStrategy {
    /// Reciprocal Rank Fusion with K=60 (default).
    Rrf,
    /// Canonical-URL deduplication, keep first occurrence.
    DedupeByUrl,
}

/// CLI wrapper for the synthesis format enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum CliSynthFormat {
    /// Markdown with H2/H3 headings and `[n](url)` links.
    Markdown,
    /// Linear numbered list without markup.
    PlainText,
    /// Structured JSON tree.
    Json,
}

impl From<CliSubQueryStrategy> for crate::deep_research::SubQueryStrategy {
    fn from(value: CliSubQueryStrategy) -> Self {
        match value {
            CliSubQueryStrategy::Heuristic => Self::Heuristic,
            CliSubQueryStrategy::Manual => Self::Manual,
        }
    }
}

impl From<CliAggregationStrategy> for crate::deep_research::AggregationStrategyKind {
    fn from(value: CliAggregationStrategy) -> Self {
        match value {
            CliAggregationStrategy::Rrf => Self::Rrf,
            CliAggregationStrategy::DedupeByUrl => Self::DedupeByUrl,
        }
    }
}

impl From<CliSynthFormat> for crate::synthesis::SynthFormat {
    fn from(value: CliSynthFormat) -> Self {
        match value {
            CliSynthFormat::Markdown => Self::Markdown,
            CliSynthFormat::PlainText => Self::PlainText,
            CliSynthFormat::Json => Self::Json,
        }
    }
}

/// Arguments for the `completions` subcommand (MP-04).
#[derive(Debug, Clone, Args)]
pub struct CompletionsArgs {
    /// Shell to generate completions for (bash, zsh, fish, powershell, elvish).
    #[arg(value_enum)]
    pub shell: CompletionShell,
}

/// Arguments specific to the `init-config` subcommand.
#[derive(Debug, Clone, Args)]
pub struct InitConfigArgs {
    /// Overwrites existing files. Without this flag, files already present
    /// are kept intact.
    #[arg(long = "force")]
    pub force: bool,

    /// Simulates execution without writing any file to disk. Reports the actions
    /// that would be taken.
    #[arg(long = "dry-run")]
    pub dry_run: bool,
}

/// Search arguments (shared between the direct mode and the `buscar` subcommand).
#[derive(Debug, Clone, Args)]
pub struct CliArgs {
    /// Search queries (free text). Accepts multiple space-separated values
    /// or via stdin (one per line) if none are passed here or via `--queries-file`.
    #[arg(value_name = "QUERY")]
    pub queries: Vec<String>,

    /// Maximum number of results to return per query (default: 15, with
    /// auto-pagination to 2 pages when `--pages` is not customized).
    /// If omitted, uses 15; if `--num > 10` and `--pages == 1` (default),
    /// `--pages` is auto-elevated to `ceil(num/10)` up to a maximum of 5.
    #[arg(short = 'n', long = "num", value_name = "N")]
    pub num_results: Option<u32>,

    /// Output format: `json`, `text`, `markdown` (`md`) or `auto`.
    /// `auto` uses `text` in a TTY and `json` in a pipe (and forces `json` when
    /// `--output` is provided).
    #[arg(
        short = 'f',
        long = "format",
        value_name = "FMT",
        default_value = "auto"
    )]
    pub format: String,

    /// Writes output to the specified file instead of printing to stdout.
    /// Missing parent directories are created. On Unix, permissions 0o644 are applied.
    #[arg(short = 'o', long = "output", value_name = "PATH")]
    pub output_file: Option<PathBuf>,

    /// Per-query timeout in seconds (default: 15).
    #[arg(
        short = 't',
        long = "timeout",
        value_name = "SECS",
        default_value_t = 15
    )]
    pub timeout_seconds: u64,

    /// Language for `DuckDuckGo`'s `kl` parameter (default: `pt`).
    #[arg(short = 'l', long = "lang", value_name = "LANG", default_value = "pt")]
    pub language: String,

    /// Country for `DuckDuckGo`'s `kl` parameter (default: `br`).
    #[arg(short = 'c', long = "country", value_name = "CC", default_value = "br")]
    pub country: String,

    /// Number of concurrent requests (default 5, maximum 20).
    #[arg(
        short = 'p',
        long = "parallel",
        value_name = "N",
        default_value_t = DEFAULT_PARALLELISM
    )]
    pub parallelism: u32,

    /// File containing additional queries (one per line). Empty lines are ignored.
    #[arg(long = "queries-file", value_name = "PATH")]
    pub queries_file: Option<PathBuf>,

    /// Number of pages to fetch per query (1..=5). Default 1.
    #[arg(long = "pages", value_name = "N", default_value_t = 1)]
    pub pages: u32,

    /// Number of additional retries on 429/403/timeout (0..=10). Default 2.
    #[arg(long = "retries", value_name = "N", default_value_t = 2)]
    pub retries: u32,

    /// Preferred endpoint: `html` (default) or `lite` (forces the no-JavaScript endpoint).
    #[arg(long = "endpoint", value_enum, default_value_t = CliEndpoint::Html)]
    pub endpoint: CliEndpoint,

    /// Time filter: `d` (day), `w` (week), `m` (month), `y` (year). Default: no filter.
    #[arg(long = "time-filter", value_enum)]
    pub time_filter: Option<CliTimeFilter>,

    /// Safe-search: `off`, `moderate` (default) or `on`.
    #[arg(long = "safe-search", value_enum, default_value_t = CliSafeSearch::Moderate)]
    pub safe_search: CliSafeSearch,

    /// Pre-flight probe: sends one minimal request to the endpoint and reports
    /// status + latency + Set-Cookie presence as JSON, then exits.
    /// Useful for diagnosing whether `DuckDuckGo` is reachable from this IP/UA
    /// before launching a real query.
    #[arg(long = "probe")]
    pub probe: bool,

    /// Forces a specific browser identity profile from the 12-identity pool.
    /// Default `auto` rotates adaptively on block (HTTP 202/403/429).
    /// When set, the chosen identity is used for the whole session.
    #[arg(long = "identity-profile", value_enum, default_value_t = CliIdentityProfile::Auto)]
    pub identity_profile: CliIdentityProfile,

    /// Placeholder — streams results as they complete. Not implemented in iteration 2.
    #[arg(long = "stream")]
    pub stream_mode: bool,

    /// Enables detailed logs on stderr (`tracing::debug` and `tracing::info`).
    #[arg(short = 'v', long = "verbose", conflicts_with = "quiet")]
    pub verbose: bool,

    /// Suppresses all stderr logs, keeping only the main output on stdout.
    #[arg(short = 'q', long = "quiet", conflicts_with = "verbose")]
    pub quiet: bool,

    /// Enables full text content extraction from each result URL (pure HTTP + readability).
    /// Makes one additional request per result, in parallel (limited by --parallel).
    #[arg(long = "fetch-content")]
    pub fetch_content: bool,

    /// Maximum size (in characters) of the extracted content per page (`1..=100_000`).
    /// Only effective with `--fetch-content`. Default `10_000`.
    #[arg(
        long = "max-content-length",
        value_name = "N",
        default_value_t = DEFAULT_MAX_CONTENT_LENGTH
    )]
    pub max_content_length: usize,

    /// HTTP/HTTPS/SOCKS5 proxy URL (e.g., `http://user:pass@host:port`, `socks5://host:port`).
    /// Takes precedence over the `HTTP_PROXY/HTTPS_PROXY/ALL_PROXY` environment variables.
    #[arg(long = "proxy", value_name = "URL", conflicts_with = "no_proxy")]
    pub proxy: Option<String>,

    /// Disables any proxy — ignores `--proxy` and the `HTTP_PROXY/HTTPS_PROXY/ALL_PROXY` env vars.
    #[arg(long = "no-proxy", conflicts_with = "proxy")]
    pub no_proxy: bool,

    /// Global timeout for the entire execution in seconds (1..=3600). Default 60.
    /// Different from `--timeout`, which is per-request.
    #[arg(
        long = "global-timeout",
        value_name = "SECS",
        default_value_t = DEFAULT_GLOBAL_TIMEOUT
    )]
    pub global_timeout_seconds: u64,

    /// Restricts UAs loaded from `user-agents.toml` to the current platform (linux/macos/windows).
    /// Only takes effect if the external TOML file is found; otherwise uses built-in defaults.
    #[arg(long = "match-platform-ua")]
    pub match_platform_ua: bool,

    /// Concurrent fetch limit PER HOST in `--fetch-content` mode (1..=10, default 2).
    /// Protects hosts from bursts — complements the global `--parallel` with a per-host gate.
    #[arg(
        long = "per-host-limit",
        value_name = "N",
        default_value_t = DEFAULT_PER_HOST_LIMIT
    )]
    pub per_host_limit: u32,

    /// Manual path to the Chrome/Chromium executable (`chrome` feature).
    /// Only useful with `--fetch-content` and the `chrome` feature compiled in;
    /// otherwise ignored with a stderr warning.
    #[arg(long = "chrome-path", value_name = "PATH")]
    pub chrome_path: Option<PathBuf>,

    /// Disables colored output (respects `NO_COLOR` env var per no-color.org).
    #[arg(long = "no-color")]
    pub no_color: bool,

    /// Disables the warm-up `GET https://duckduckgo.com/` request that
    /// populates session cookies before the first real query. v0.7.3 PR2.
    /// Default `false` (warm-up enabled). Disabling saves one request
    /// per invocation but increases CAPTCHA risk on macOS.
    #[arg(long = "no-warmup")]
    pub no_warmup: bool,

    /// Disables persistence of the cookie jar to disk. Cookies live only
    /// in memory for the duration of the process. v0.7.3 PR2.
    /// Default `false` (cookies are persisted to
    /// `~/.config/duckduckgo-search-cli/cookies.json` on Unix or
    /// `%APPDATA%\duckduckgo-search-cli\cookies.json` on Windows).
    #[arg(long = "no-cookie-persistence")]
    pub no_cookie_persistence: bool,

    /// Overrides the cookie jar file path. v0.7.3 PR2.
    /// Default is the XDG config dir joined with `cookies.json`.
    #[arg(long = "cookies-path", value_name = "PATH")]
    pub cookies_path: Option<PathBuf>,

    /// Performs a deep health check on the configured endpoint, including
    /// interstitial detection (Cloudflare / DDG bot challenge). v0.7.3 PR3.
    /// Emits a JSON report on stdout and exits before running the real query.
    /// Default `false`.
    #[arg(long = "probe-deep")]
    pub probe_deep: bool,

    /// Allows automatic fallback to the `lite` endpoint when the
    /// `html` endpoint returns a bot-detection interstitial (HTTP 200
    /// with zero results). v0.7.3 PR3. Default `false` — without this
    /// flag, the CLI emits the zero-result output and exits with code 5
    /// as before, so users are not surprised by content changes.
    #[arg(long = "allow-lite-fallback")]
    pub allow_lite_fallback: bool,

    /// Seed for deterministic User-Agent selection (debugging reproducibility).
    #[arg(long = "seed", value_name = "N")]
    pub seed: Option<u64>,

    /// Path to configuration directory (overrides default OS config path).
    #[arg(long = "config", value_name = "PATH")]
    pub config_path: Option<PathBuf>,
}

impl CliArgs {
    /// Validates that the parallelism degree is within the range `[1, MAX_PARALLELISM]`.
    ///
    /// # Errors
    ///
    /// Returns an error string if `--parallel` is zero or exceeds [`MAX_PARALLELISM`].
    pub fn validate_parallelism(&self) -> Result<(), String> {
        if self.parallelism == 0 {
            return Err(format!(
                "--parallel must be at least 1 (got {})",
                self.parallelism
            ));
        }
        if self.parallelism > MAX_PARALLELISM {
            return Err(format!(
                "--parallel cannot exceed {} (got {})",
                MAX_PARALLELISM, self.parallelism
            ));
        }
        Ok(())
    }

    /// Validates that the number of pages is within the range `[1, MAX_PAGES]`.
    ///
    /// # Errors
    ///
    /// Returns an error string if `--pages` is zero or exceeds [`MAX_PAGES`].
    pub fn validate_pages(&self) -> Result<(), String> {
        if self.pages == 0 {
            return Err(format!("--pages must be at least 1 (got {})", self.pages));
        }
        if self.pages > MAX_PAGES {
            return Err(format!(
                "--pages cannot exceed {} (got {})",
                MAX_PAGES, self.pages
            ));
        }
        Ok(())
    }

    /// Validates that `--max-content-length` is within the range `[1, MAX_CONTENT_LENGTH_LIMIT]`.
    ///
    /// # Errors
    ///
    /// Returns an error string if `--max-content-length` is zero or exceeds
    /// [`MAX_CONTENT_LENGTH_LIMIT`].
    pub fn validate_max_content_length(&self) -> Result<(), String> {
        if self.max_content_length == 0 {
            return Err(format!(
                "--max-content-length must be at least 1 (got {})",
                self.max_content_length
            ));
        }
        if self.max_content_length > MAX_CONTENT_LENGTH_LIMIT {
            return Err(format!(
                "--max-content-length cannot exceed {} (got {})",
                MAX_CONTENT_LENGTH_LIMIT, self.max_content_length
            ));
        }
        Ok(())
    }

    /// Validates that `--global-timeout` is within the range `[1, MAX_GLOBAL_TIMEOUT]`.
    ///
    /// # Errors
    ///
    /// Returns an error string if `--global-timeout` is zero or exceeds
    /// [`MAX_GLOBAL_TIMEOUT`].
    pub fn validate_global_timeout(&self) -> Result<(), String> {
        if self.global_timeout_seconds == 0 {
            return Err(format!(
                "--global-timeout must be at least 1 (got {})",
                self.global_timeout_seconds
            ));
        }
        if self.global_timeout_seconds > MAX_GLOBAL_TIMEOUT {
            return Err(format!(
                "--global-timeout cannot exceed {} seconds (got {})",
                MAX_GLOBAL_TIMEOUT, self.global_timeout_seconds
            ));
        }
        Ok(())
    }

    /// Validates that `--proxy`, when provided, is a parseable URL with a supported scheme.
    ///
    /// # Errors
    ///
    /// Returns an error string if `--proxy` is not a valid URL or uses an unsupported
    /// scheme (only `http`, `https`, `socks5`, and `socks5h` are accepted).
    pub fn validate_proxy(&self) -> Result<(), String> {
        let Some(url) = self.proxy.as_deref() else {
            return Ok(());
        };
        let parsed =
            url::Url::parse(url).map_err(|e| format!("invalid --proxy URL ({url:?}): {e}"))?;
        match parsed.scheme() {
            "http" | "https" | "socks5" | "socks5h" => Ok(()),
            other => Err(format!(
                "scheme {other:?} not supported in --proxy (use http/https/socks5)"
            )),
        }
    }

    /// Validates that the number of retries is within the range `[0, MAX_RETRIES]`.
    ///
    /// # Errors
    ///
    /// Returns an error string if `--retries` exceeds [`MAX_RETRIES`].
    pub fn validate_retries(&self) -> Result<(), String> {
        if self.retries > MAX_RETRIES {
            return Err(format!(
                "--retries cannot exceed {} (got {})",
                MAX_RETRIES, self.retries
            ));
        }
        Ok(())
    }

    /// Validates that `--per-host-limit` is within the range `[1, MAX_PER_HOST_LIMIT]`.
    ///
    /// # Errors
    ///
    /// Returns an error string if `--per-host-limit` is zero or exceeds
    /// [`MAX_PER_HOST_LIMIT`].
    pub fn validate_per_host_limit(&self) -> Result<(), String> {
        if self.per_host_limit == 0 {
            return Err(format!(
                "--per-host-limit must be at least 1 (got {})",
                self.per_host_limit
            ));
        }
        if self.per_host_limit > MAX_PER_HOST_LIMIT {
            return Err(format!(
                "--per-host-limit cannot exceed {} (got {})",
                MAX_PER_HOST_LIMIT, self.per_host_limit
            ));
        }
        Ok(())
    }

    /// Validates that `--timeout` is at least 1 second.
    ///
    /// # Errors
    ///
    /// Returns an error string if `--timeout` is zero.
    pub fn validate_timeout_seconds(&self) -> Result<(), String> {
        if self.timeout_seconds == 0 {
            return Err(format!(
                "--timeout must be at least 1 (got {})",
                self.timeout_seconds
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    /// Helper: parses arguments via root and extracts `CliArgs` (default flow = Buscar).
    /// Replicates the convenience behavior of tests prior to the introduction of the subcommand.
    fn parse_buscar(argv: &[&str]) -> Result<CliArgs, clap::Error> {
        let root = RootArgs::try_parse_from(argv)?;
        match root.subcomando {
            Some(Subcommand::Buscar(a)) => Ok(*a),
            Some(Subcommand::InitConfig(_))
            | Some(Subcommand::Completions(_))
            | Some(Subcommand::DeepResearch(_)) => Err(clap::Error::raw(
                clap::error::ErrorKind::InvalidSubcommand,
                "subcomando nao-busca retornado em contexto que esperava busca",
            )),
            None => Ok(root.buscar),
        }
    }

    #[test]
    fn cli_passes_schema_validation() {
        // `debug_assert` do clap valida a struct em tempo de chamada.
        RootArgs::command().debug_assert();
    }

    #[test]
    fn parseia_query_simples() {
        let args = parse_buscar(&["bin", "rust async"]).expect("should parse");
        assert_eq!(args.queries, vec!["rust async".to_string()]);
        // Default is now "auto" (resolved at runtime via TTY detection).
        assert_eq!(args.format, "auto");
        assert!(args.output_file.is_none());
        assert_eq!(args.timeout_seconds, 15);
        assert_eq!(args.language, "pt");
        assert_eq!(args.country, "br");
        assert_eq!(args.parallelism, DEFAULT_PARALLELISM);
        assert_eq!(args.pages, 1);
        assert_eq!(args.retries, 2);
        assert_eq!(args.endpoint, CliEndpoint::Html);
        assert!(args.time_filter.is_none());
        assert_eq!(args.safe_search, CliSafeSearch::Moderate);
        assert!(!args.stream_mode);
        assert!(args.queries_file.is_none());
        assert!(!args.verbose);
        assert!(!args.quiet);
        assert!(!args.fetch_content);
        assert_eq!(args.max_content_length, DEFAULT_MAX_CONTENT_LENGTH);
        assert!(args.proxy.is_none());
        assert!(!args.no_proxy);
        assert_eq!(args.global_timeout_seconds, DEFAULT_GLOBAL_TIMEOUT);
        assert!(!args.match_platform_ua);
    }

    #[test]
    fn parseia_fetch_content_e_max_content_length() {
        let args = parse_buscar(&[
            "bin",
            "--fetch-content",
            "--max-content-length",
            "500",
            "rust",
        ])
        .expect("should parse --fetch-content");
        assert!(args.fetch_content);
        assert_eq!(args.max_content_length, 500);
    }

    #[test]
    fn parseia_proxy_e_no_proxy_mutuamente_exclusivos() {
        let ok = parse_buscar(&[
            "bin",
            "--proxy",
            "http://user:pass@proxy.local:8080",
            "rust",
        ])
        .expect("should parse --proxy");
        assert_eq!(
            ok.proxy.as_deref(),
            Some("http://user:pass@proxy.local:8080")
        );
        assert!(!ok.no_proxy);

        let no = parse_buscar(&["bin", "--no-proxy", "rust"]).expect("should parse --no-proxy");
        assert!(no.no_proxy);
        assert!(no.proxy.is_none());

        let err = parse_buscar(&["bin", "--proxy", "http://x", "--no-proxy", "rust"]);
        assert!(err.is_err(), "--proxy + --no-proxy deve conflitar");
    }

    #[test]
    fn parseia_global_timeout() {
        let args = parse_buscar(&["bin", "--global-timeout", "30", "rust"]).unwrap();
        assert_eq!(args.global_timeout_seconds, 30);
    }

    #[test]
    fn validate_max_content_length_range() {
        let mut args = parse_buscar(&["bin", "q"]).unwrap();
        args.max_content_length = 0;
        assert!(args.validate_max_content_length().is_err());
        args.max_content_length = MAX_CONTENT_LENGTH_LIMIT + 1;
        assert!(args.validate_max_content_length().is_err());
        args.max_content_length = 5000;
        assert!(args.validate_max_content_length().is_ok());
    }

    #[test]
    fn validate_global_timeout_range() {
        let mut args = parse_buscar(&["bin", "q"]).unwrap();
        args.global_timeout_seconds = 0;
        assert!(args.validate_global_timeout().is_err());
        args.global_timeout_seconds = MAX_GLOBAL_TIMEOUT + 1;
        assert!(args.validate_global_timeout().is_err());
        args.global_timeout_seconds = 120;
        assert!(args.validate_global_timeout().is_ok());
    }

    #[test]
    fn validate_proxy_accepts_supported_schemes() {
        let mut args = parse_buscar(&["bin", "q"]).unwrap();
        for ok in [
            "http://proxy:8080",
            "https://user:pass@proxy:8443",
            "socks5://127.0.0.1:9050",
            "socks5h://host:1080",
        ] {
            args.proxy = Some(ok.to_string());
            assert!(
                args.validate_proxy().is_ok(),
                "proxy {ok:?} deveria ser aceito"
            );
        }
        args.proxy = Some("ftp://proxy".to_string());
        assert!(args.validate_proxy().is_err());
        args.proxy = Some("nao-eh-uma-url".to_string());
        assert!(args.validate_proxy().is_err());
        args.proxy = None;
        assert!(args.validate_proxy().is_ok());
    }

    #[test]
    fn parses_resilience_and_filter_flags() {
        let args = parse_buscar(&[
            "bin",
            "--pages",
            "3",
            "--retries",
            "5",
            "--endpoint",
            "lite",
            "--time-filter",
            "w",
            "--safe-search",
            "on",
            "rust",
        ])
        .expect("should parse resilience flags");
        assert_eq!(args.pages, 3);
        assert_eq!(args.retries, 5);
        assert_eq!(args.endpoint, CliEndpoint::Lite);
        assert_eq!(args.time_filter, Some(CliTimeFilter::W));
        assert_eq!(args.safe_search, CliSafeSearch::On);
    }

    #[test]
    fn validate_pages_accepts_range_and_rejects_invalid() {
        let mut args = parse_buscar(&["bin", "qualquer"]).unwrap();
        for v in [1u32, 2, 5] {
            args.pages = v;
            assert!(args.validate_pages().is_ok(), "pages {v}");
        }
        args.pages = 0;
        assert!(args.validate_pages().is_err());
        args.pages = 6;
        assert!(args.validate_pages().is_err());
    }

    #[test]
    fn validate_retries_rejects_above_max() {
        let mut args = parse_buscar(&["bin", "qualquer"]).unwrap();
        args.retries = 0;
        assert!(args.validate_retries().is_ok());
        args.retries = 10;
        assert!(args.validate_retries().is_ok());
        args.retries = 11;
        assert!(args.validate_retries().is_err());
    }

    #[test]
    fn parseia_multiplas_queries_posicionais() {
        let args = parse_buscar(&["bin", "rust async", "tokio runtime", "async channels"])
            .expect("should parse multiple queries");
        assert_eq!(
            args.queries,
            vec![
                "rust async".to_string(),
                "tokio runtime".to_string(),
                "async channels".to_string(),
            ]
        );
    }

    #[test]
    fn parseia_flags_customizadas() {
        let args = parse_buscar(&[
            "bin",
            "--num",
            "10",
            "--format",
            "json",
            "--timeout",
            "30",
            "--lang",
            "en",
            "--country",
            "us",
            "--parallel",
            "8",
            "--verbose",
            "teste de busca",
        ])
        .expect("should parse with flags");
        assert_eq!(args.queries, vec!["teste de busca".to_string()]);
        assert_eq!(args.num_results, Some(10));
        assert_eq!(args.timeout_seconds, 30);
        assert_eq!(args.language, "en");
        assert_eq!(args.country, "us");
        assert_eq!(args.parallelism, 8);
        assert!(args.verbose);
    }

    #[test]
    fn parseia_flag_output_curta_e_longa() {
        let args = parse_buscar(&["bin", "-o", "/tmp/saida.json", "q"]).expect("should parse -o");
        assert_eq!(
            args.output_file.as_deref(),
            Some(std::path::Path::new("/tmp/saida.json"))
        );

        let args2 = parse_buscar(&["bin", "--output", "/tmp/x.md", "--format", "markdown", "q"])
            .expect("should parse --output");
        assert_eq!(
            args2.output_file.as_deref(),
            Some(std::path::Path::new("/tmp/x.md"))
        );
        assert_eq!(args2.format, "markdown");
    }

    #[test]
    fn parseia_arquivo_queries_e_stream() {
        let args = parse_buscar(&["bin", "--queries-file", "queries.txt", "--stream"])
            .expect("should parse --queries-file and --stream");
        assert!(args.stream_mode);
        assert_eq!(
            args.queries_file.as_deref(),
            Some(std::path::Path::new("queries.txt"))
        );
        assert!(args.queries.is_empty());
    }

    #[test]
    fn verbose_e_quiet_sao_mutuamente_exclusivos() {
        let result = parse_buscar(&["bin", "--verbose", "--quiet", "query qualquer"]);
        assert!(result.is_err(), "verbose + quiet deve falhar a validação");
    }

    #[test]
    fn validate_parallelism_accepts_allowed_range() {
        let mut args = parse_buscar(&["bin", "qualquer"]).unwrap();
        for value in [1u32, 5, 10, MAX_PARALLELISM] {
            args.parallelism = value;
            assert!(
                args.validate_parallelism().is_ok(),
                "--parallel {value} deveria ser aceito"
            );
        }
    }

    #[test]
    fn validate_parallelism_rejects_invalid_values() {
        let mut args = parse_buscar(&["bin", "qualquer"]).unwrap();
        args.parallelism = 0;
        assert!(args.validate_parallelism().is_err());
        args.parallelism = MAX_PARALLELISM + 1;
        assert!(args.validate_parallelism().is_err());
        args.parallelism = 100;
        assert!(args.validate_parallelism().is_err());
    }

    #[test]
    fn parses_init_config_subcommand_with_flags() {
        let root = RootArgs::try_parse_from(["bin", "init-config", "--force", "--dry-run"])
            .expect("should parse init-config");
        let Some(Subcommand::InitConfig(args)) = root.subcomando else {
            panic!("esperava subcomando InitConfig");
        };
        assert!(args.force);
        assert!(args.dry_run);
    }

    #[test]
    fn parses_init_config_subcommand_without_flags() {
        let root = RootArgs::try_parse_from(["bin", "init-config"])
            .expect("should parse init-config without flags");
        let Some(Subcommand::InitConfig(args)) = root.subcomando else {
            panic!("esperava subcomando InitConfig");
        };
        assert!(!args.force);
        assert!(!args.dry_run);
    }

    #[test]
    fn parseia_subcomando_buscar_explicito() {
        let root = RootArgs::try_parse_from(["bin", "buscar", "rust"])
            .expect("should parse buscar subcommand");
        let Some(Subcommand::Buscar(args)) = root.subcomando else {
            panic!("esperava subcomando Buscar");
        };
        assert_eq!(args.queries, vec!["rust".to_string()]);
    }

    #[test]
    fn search_subcommand_stays_small_when_boxed() {
        // Regression guarantee: Subcommand::Buscar is still Box — clippy lint large_enum.
        // v0.7.0 added DeepResearchArgs; the largest variant dictates the enum
        // size. We assert the enum stays under 256 bytes (deep-research fields
        // include 5 strings + 1 PathBuf, well below that cap).
        let enum_size = std::mem::size_of::<Subcommand>();
        assert!(
            enum_size <= 256,
            "Subcommand grew unexpectedly: {enum_size} bytes"
        );
    }

    #[test]
    fn parse_without_subcommand_uses_search_flatten() {
        let root = RootArgs::try_parse_from(["bin", "rust async"])
            .expect("should parse without subcommand");
        assert!(root.subcomando.is_none());
        assert_eq!(root.buscar.queries, vec!["rust async".to_string()]);
    }

    #[test]
    fn parseia_per_host_limit() {
        let args = parse_buscar(&["bin", "--per-host-limit", "5", "q"]).unwrap();
        assert_eq!(args.per_host_limit, 5);
        let default = parse_buscar(&["bin", "q"]).unwrap();
        assert_eq!(default.per_host_limit, DEFAULT_PER_HOST_LIMIT);
    }

    #[test]
    fn validate_per_host_limit_range() {
        let mut args = parse_buscar(&["bin", "q"]).unwrap();
        args.per_host_limit = 0;
        assert!(args.validate_per_host_limit().is_err());
        args.per_host_limit = MAX_PER_HOST_LIMIT + 1;
        assert!(args.validate_per_host_limit().is_err());
        args.per_host_limit = 2;
        assert!(args.validate_per_host_limit().is_ok());
    }

    #[test]
    fn validate_timeout_seconds_rejects_zero() {
        let mut args = parse_buscar(&["bin", "q"]).unwrap();
        args.timeout_seconds = 0;
        assert!(args.validate_timeout_seconds().is_err());
        args.timeout_seconds = 1;
        assert!(args.validate_timeout_seconds().is_ok());
        args.timeout_seconds = 15;
        assert!(args.validate_timeout_seconds().is_ok());
    }
}
