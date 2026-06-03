// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload: declarative (error enum definition via thiserror)
//! Structured error codes as defined in specification section 14.3.
//!
//! The typed [`CliError`] enum maps each failure mode to a specific exit
//! code and JSON error code. Library consumers should match on the enum
//! variants; binary callers can use [`CliError::exit_code`] directly.

/// Error codes that may appear in the `error` field of the JSON output.
/// Correspond to the values listed in section 14.3 of the blueprint.
pub mod codes {
    /// HTTP-level failure (timeout, connection refused, non-2xx status).
    pub const HTTP_ERROR: &str = "http_error";
    /// Persistent rate limiting (HTTP 429 after exhausting retries).
    pub const RATE_LIMITED: &str = "rate_limited";
    /// Anti-bot blocking detected (HTTP 202 anomaly or persistent 403).
    pub const BLOCKED: &str = "blocked";
    /// Zero organic results across all queries.
    pub const NO_RESULTS_FOUND: &str = "no_results_found";
    /// Selector configuration file is invalid or unparseable.
    pub const SELECTOR_CONFIG_INVALID: &str = "selector_config_invalid";
    /// Pagination token extraction failed.
    pub const PAGINATION_FAILED: &str = "pagination_failed";
    /// Global timeout exceeded.
    pub const TIMEOUT: &str = "timeout";
    /// Operation cancelled via SIGINT / Ctrl-C.
    pub const CANCELLED: &str = "cancelled";
    /// Chrome/Chromium executable not found on the system.
    pub const CHROME_NOT_FOUND: &str = "chrome_not_found";
    /// Low-level network error (DNS, TLS, connection reset).
    pub const NETWORK_ERROR: &str = "network_error";
    /// Proxy configuration or connection failure.
    pub const PROXY_ERROR: &str = "proxy_error";
}

/// Exit codes defined in specification section 17.7.
pub mod exit_codes {
    /// At least one query returned results.
    pub const SUCCESS: i32 = 0;
    /// Generic error (configuration failure, IO, etc.).
    pub const GENERIC_ERROR: i32 = 1;
    /// Invalid configuration (incompatible CLI arguments).
    pub const INVALID_CONFIG: i32 = 2;
    /// Rate limiting or blocking on all queries.
    pub const RATE_LIMITED_OR_BLOCKED: i32 = 3;
    /// Global timeout exceeded.
    pub const GLOBAL_TIMEOUT: i32 = 4;
    /// Zero results on all queries.
    pub const ZERO_RESULTS: i32 = 5;
}

/// Typed error enum for the CLI domain.
///
/// Each variant maps to a specific exit code and JSON error code.
#[derive(thiserror::Error, Debug)]
#[non_exhaustive]
pub enum CliError {
    /// HTTP-level failure with optional source chain.
    #[error("HTTP error: {message}")]
    HttpError {
        /// Human-readable description of the HTTP failure.
        message: String,
        /// Underlying cause, when available.
        #[source]
        cause: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    /// Persistent rate limiting after exhausting retries (HTTP 429).
    #[error("rate limiting detected by DuckDuckGo")]
    RateLimited,

    /// Anti-bot blocking detected (HTTP 202 anomaly or persistent 403).
    #[error("anti-bot blocking detected (HTTP 202 anomaly)")]
    Blocked,

    /// Zero organic results across all queries.
    #[error("zero results across all queries")]
    NoResults,

    /// Invalid CLI configuration (incompatible arguments, bad values).
    #[error("invalid configuration: {message}")]
    InvalidConfig {
        /// Description of the configuration problem.
        message: String,
    },

    /// Global timeout exceeded.
    #[error("global timeout exceeded ({seconds}s)")]
    GlobalTimeout {
        /// Configured timeout in seconds.
        seconds: u64,
    },

    /// Operation cancelled via SIGINT / Ctrl-C.
    #[error("operation cancelled via SIGINT")]
    Cancelled,

    /// Proxy configuration or connection failure.
    #[error("proxy error: {message}")]
    ProxyError {
        /// Description of the proxy problem.
        message: String,
    },

    /// Low-level network error (DNS, TLS, connection reset).
    #[error("network error: {message}")]
    NetworkError {
        /// Description of the network failure.
        message: String,
    },

    /// Consumer closed the pipe (SIGPIPE / `BrokenPipe`).
    #[error("pipe closed by consumer (BrokenPipe)")]
    BrokenPipe,

    /// Output path is invalid (path traversal, system directory).
    #[error("invalid output path: {message}")]
    PathError {
        /// Description of why the path was rejected.
        message: String,
    },
}

impl CliError {
    /// Returns the exit code corresponding to this error variant.
    pub fn exit_code(&self) -> i32 {
        match self {
            Self::HttpError { .. } | Self::NetworkError { .. } => exit_codes::GENERIC_ERROR,
            Self::InvalidConfig { .. } | Self::ProxyError { .. } | Self::PathError { .. } => {
                exit_codes::INVALID_CONFIG
            }
            Self::RateLimited | Self::Blocked => exit_codes::RATE_LIMITED_OR_BLOCKED,
            Self::GlobalTimeout { .. } => exit_codes::GLOBAL_TIMEOUT,
            Self::NoResults => exit_codes::ZERO_RESULTS,
            Self::Cancelled => exit_codes::GENERIC_ERROR,
            Self::BrokenPipe => exit_codes::SUCCESS,
        }
    }

    /// Returns the string error code for use in the `error` field of the JSON output.
    pub fn error_code(&self) -> &'static str {
        match self {
            Self::HttpError { .. } => codes::HTTP_ERROR,
            Self::RateLimited => codes::RATE_LIMITED,
            Self::Blocked => codes::BLOCKED,
            Self::NoResults => codes::NO_RESULTS_FOUND,
            Self::InvalidConfig { .. } => codes::SELECTOR_CONFIG_INVALID,
            Self::GlobalTimeout { .. } => codes::TIMEOUT,
            Self::Cancelled => codes::CANCELLED,
            Self::ProxyError { .. } => codes::PROXY_ERROR,
            Self::NetworkError { .. } => codes::NETWORK_ERROR,
            Self::BrokenPipe => codes::HTTP_ERROR,
            Self::PathError { .. } => codes::SELECTOR_CONFIG_INVALID,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_codes_are_non_empty_strings() {
        assert!(!codes::HTTP_ERROR.is_empty());
        assert!(!codes::BLOCKED.is_empty());
        assert!(!codes::NO_RESULTS_FOUND.is_empty());
    }

    #[test]
    fn exit_codes_have_correct_values() {
        assert_eq!(exit_codes::SUCCESS, 0);
        assert_eq!(exit_codes::GENERIC_ERROR, 1);
        assert_eq!(exit_codes::INVALID_CONFIG, 2);
        assert_eq!(exit_codes::RATE_LIMITED_OR_BLOCKED, 3);
        assert_eq!(exit_codes::GLOBAL_TIMEOUT, 4);
        assert_eq!(exit_codes::ZERO_RESULTS, 5);
    }

    #[test]
    fn cli_error_exit_codes_are_correct() {
        assert_eq!(
            CliError::RateLimited.exit_code(),
            exit_codes::RATE_LIMITED_OR_BLOCKED
        );
        assert_eq!(
            CliError::Blocked.exit_code(),
            exit_codes::RATE_LIMITED_OR_BLOCKED
        );
        assert_eq!(CliError::NoResults.exit_code(), exit_codes::ZERO_RESULTS);
        assert_eq!(
            CliError::GlobalTimeout { seconds: 60 }.exit_code(),
            exit_codes::GLOBAL_TIMEOUT
        );
        assert_eq!(
            CliError::InvalidConfig {
                message: "test".into()
            }
            .exit_code(),
            exit_codes::INVALID_CONFIG
        );
        assert_eq!(CliError::BrokenPipe.exit_code(), exit_codes::SUCCESS);
    }

    #[test]
    fn cli_error_display_is_not_empty() {
        let err = CliError::HttpError {
            message: "timeout".into(),
            cause: None,
        };
        let text = format!("{err}");
        assert!(!text.is_empty());
        assert!(text.contains("timeout"));
    }

    #[test]
    fn cli_error_codes_are_correct_strings() {
        assert_eq!(CliError::RateLimited.error_code(), "rate_limited");
        assert_eq!(CliError::Blocked.error_code(), "blocked");
        assert_eq!(CliError::NoResults.error_code(), "no_results_found");
    }
}
