// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload: I/O-bound (reqwest client construction and UA management)
//! `reqwest::Client` construction and User-Agent selection.
//!
//! The HTTP client is configured with:
//! - TLS via `rustls-tls` (no OpenSSL dependency on any platform).
//! - Cookie store enabled (required for pagination with `vqd` token).
//! - `gzip` + `brotli` compression (reduces bandwidth).
//! - Redirect policy limited to 5 hops.
//! - Headers that mimic a real browser with full family profile (Chrome, Firefox, Safari, Edge).
//! - Configurable total timeout.
//! - Optional HTTP/HTTPS/SOCKS5 proxy.
//! - User-Agents loaded from external `user-agents.toml` OR built-in defaults.
//!
//! ## Browser Profiles (v0.6.0)
//!
//! Each loaded UA receives a [`BrowserProfile`] that encapsulates the detected family
//! (`Chrome`, `Firefox`, `Safari`, `Edge`) and generates complete Sec-Fetch headers.
//! Chrome and Edge also emit Client Hints (`Sec-CH-UA*`), exactly replicating
//! the behavior of real browsers and reducing anti-bot detection.

use crate::error::CliError;
use crate::platform;
use rand::seq::{IteratorRandom, SliceRandom};
use reqwest::{
    header::{
        HeaderMap, HeaderName, HeaderValue, ACCEPT, ACCEPT_ENCODING, ACCEPT_LANGUAGE, CACHE_CONTROL,
    },
    redirect::Policy,
    Client,
};
use serde::Deserialize;
use std::time::Duration;

/// Built-in User-Agent list embedded in the binary as fallback when `config/user-agents.toml`
/// is not available.
///
/// v0.3.0 — POOL UPDATE (2026-04-14):
/// The old text browser UAs (Lynx, w3m, Links, `ELinks`) were REMOVED.
/// Empirically they still return HTTP 200, but `DuckDuckGo` serves DEGRADED HTML
/// for those agents: the layout lacks consistent `.result__snippet` classes,
/// forcing the extractor to fall back to Strategy 2 and return empty/incorrect snippets.
///
/// Final empirical validation (2026-04-14, real requests to /html/):
///   Chrome 146 Win/Mac/Linux → 200 OK ✓
///   Edge   145 Windows       → 200 OK ✓
///   Safari 17.6 macOS        → 200 OK ✓
///   Firefox 134 Linux        → 200 OK ✓
///   Firefox 134 Windows      → 202 ANOMALY ✗ (REMOVED)
///   Firefox 134 macOS        → 202 ANOMALY ✗ (REMOVED)
///
/// `DuckDuckGo` blocks Firefox desktop Win/Mac on the `/html/` endpoint
/// (anti-bot heuristic: UA claiming full browser without JS). Linux Firefox
/// passes because it is a minority desktop — DDG's filter is less aggressive.
const USER_AGENTS_DEFAULT: &[&str] = &[
    // Chrome desktop (Windows / macOS / Linux) — abril 2026
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/146.0.0.0 Safari/537.36",
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/146.0.0.0 Safari/537.36",
    "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/146.0.0.0 Safari/537.36",
    // Edge Windows
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/145.0.0.0 Safari/537.36 Edg/145.0.3800.97",
    // Firefox desktop (Linux only — Win/Mac return HTTP 202 on /html/)
    "Mozilla/5.0 (X11; Linux x86_64; rv:134.0) Gecko/20100101 Firefox/134.0",
    // Safari macOS
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.6 Safari/605.1.15",
];

// ---------------------------------------------------------------------------
// Browser family
// ---------------------------------------------------------------------------

/// Detected browser family from the User-Agent string.
///
/// Used to generate family-specific headers (Client Hints, Accept, etc.).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrowserFamily {
    /// Google Chrome or Chromium derivatives (except Edge).
    Chrome,
    /// Mozilla Firefox.
    Firefox,
    /// Apple Safari (no Chrome indicator in the UA).
    Safari,
    /// Microsoft Edge (Chromium-based, contains `Edg/`).
    Edge,
}

// ---------------------------------------------------------------------------
// Perfil de browser
// ---------------------------------------------------------------------------

/// Complete browser profile derived from its User-Agent.
///
/// Encapsulates family, major version, and platform to generate correct
/// Sec-Fetch and Client Hints headers per family.
#[derive(Debug, Clone)]
pub struct BrowserProfile {
    /// Detected browser family.
    pub family: BrowserFamily,
    /// Full User-Agent string.
    pub user_agent: String,
    /// Browser major version (e.g. 146 for Chrome 146).
    pub major_version: u32,
    /// Platform normalized for Client Hints (e.g. `"Windows"`, `"macOS"`, `"Linux"`).
    pub ua_platform: &'static str,
}

/// Detects the browser family from a User-Agent string.
///
/// Detection priority:
/// 1. `Edg/` → Edge
/// 2. `Chrome/` → Chrome
/// 3. `Firefox/` → Firefox
/// 4. `Safari/` without `Chrome/` → Safari
/// 5. Fallback → Firefox
///
/// # Exemplos
///
/// ```
/// use duckduckgo_search_cli::http::{detect_family, BrowserFamily};
/// assert_eq!(detect_family("Mozilla/5.0 ... Chrome/146 ... Edg/145"), BrowserFamily::Edge);
/// assert_eq!(detect_family("Mozilla/5.0 ... Chrome/146 ..."), BrowserFamily::Chrome);
/// ```
pub fn detect_family(ua: &str) -> BrowserFamily {
    if ua.contains("Edg/") {
        BrowserFamily::Edge
    } else if ua.contains("Chrome/") {
        BrowserFamily::Chrome
    } else if ua.contains("Firefox/") {
        BrowserFamily::Firefox
    } else if ua.contains("Safari/") {
        BrowserFamily::Safari
    } else {
        BrowserFamily::Firefox
    }
}

/// Extracts the major version of the browser from the UA and the detected family.
///
/// Supported patterns: `Chrome/146`, `Firefox/134`, `Version/17` (Safari), `Edg/145`.
/// Returns `0` if no pattern is found.
fn extract_major_version(ua: &str, family: BrowserFamily) -> u32 {
    let prefix = match family {
        BrowserFamily::Chrome => "Chrome/",
        BrowserFamily::Firefox => "Firefox/",
        BrowserFamily::Safari => "Version/",
        BrowserFamily::Edge => "Edg/",
    };

    if let Some(pos) = ua.find(prefix) {
        let rest = &ua[pos + prefix.len()..];
        return rest
            .chars()
            .take_while(|c| c.is_ascii_digit())
            .fold(0u32, |acc, c| acc * 10 + c.to_digit(10).unwrap_or(0));
    }
    0
}

/// Extracts the platform from the UA and normalizes it to the Client Hints format.
///
/// Mappings:
/// - `Windows NT` → `"Windows"`
/// - `Macintosh` → `"macOS"`
/// - Fallback → `"Linux"`
fn extract_ua_platform(ua: &str) -> &'static str {
    if ua.contains("Windows NT") {
        "Windows"
    } else if ua.contains("Macintosh") {
        "macOS"
    } else {
        "Linux"
    }
}

/// Builds a complete [`BrowserProfile`] from a User-Agent string.
///
/// Combines `detect_family`, `extract_major_version`, and `extract_ua_platform`.
///
/// The resulting profile automatically emits the correct `Sec-Fetch-*` and Client Hints
/// headers for the detected family — **do not inject custom Sec-Fetch or Accept
/// headers on top of this profile** (see rule R33 in `AGENT_RULES.md`).
///
/// # Exemplos
///
/// ```
/// use duckduckgo_search_cli::http::{create_browser_profile, BrowserFamily};
///
/// // Chrome UA → Chrome family, major version extracted, Linux platform
/// let ua_chrome = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 \
///                  (KHTML, like Gecko) Chrome/146.0.0.0 Safari/537.36";
/// let profile = create_browser_profile(ua_chrome);
/// assert_eq!(profile.family, BrowserFamily::Chrome);
/// assert_eq!(profile.major_version, 146);
/// assert_eq!(profile.ua_platform, "Linux");
///
/// // Edge UA → Edge family (Sec-CH-UA* headers emitted automatically)
/// let ua_edge = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 \
///                (KHTML, like Gecko) Chrome/145.0.0.0 Safari/537.36 Edg/145.0.0.0";
/// let profile_edge = create_browser_profile(ua_edge);
/// assert_eq!(profile_edge.family, BrowserFamily::Edge);
/// assert_eq!(profile_edge.ua_platform, "Windows");
/// ```
pub fn create_browser_profile(ua: &str) -> BrowserProfile {
    let family = detect_family(ua);
    let major_version = extract_major_version(ua, family);
    let ua_platform = extract_ua_platform(ua);
    BrowserProfile {
        family,
        user_agent: ua.to_string(),
        major_version,
        ua_platform,
    }
}

impl BrowserProfile {
    /// Generates the full initial headers for the first request of the session.
    ///
    /// Includes universal headers (Accept, Accept-Language, Accept-Encoding,
    /// Upgrade-Insecure-Requests, Sec-Fetch-*) and, for Chrome/Edge, Client Hints
    /// (Sec-CH-UA, Sec-CH-UA-Mobile, Sec-CH-UA-Platform, Cache-Control).
    ///
    /// # Arguments
    /// * `language` — BCP-47 language code (e.g. `"pt"`, `"en"`).
    /// * `country` — ISO 3166-1 alpha-2 country code (e.g. `"br"`, `"us"`).
    ///
    /// # Errors
    /// Returns an error if any header value contains invalid bytes.
    pub fn initial_headers(&self, language: &str, country: &str) -> Result<HeaderMap, CliError> {
        let mut headers = HeaderMap::new();

        // Accept by browser family
        let accept_value = match self.family {
            BrowserFamily::Chrome | BrowserFamily::Edge => {
                "text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,image/apng,*/*;q=0.8"
            }
            BrowserFamily::Firefox => {
                "text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,*/*;q=0.8"
            }
            BrowserFamily::Safari => {
                "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8"
            }
        };
        headers.insert(ACCEPT, HeaderValue::from_static(accept_value));

        // Accept-Language with q-values
        let language_lower = language.to_ascii_lowercase();
        let country_upper = country.to_ascii_uppercase();
        let accept_language = if language_lower == "en" {
            "en-US,en;q=0.9".to_string()
        } else {
            format!("{language_lower}-{country_upper},{language_lower};q=0.9,en-US;q=0.8,en;q=0.7")
        };
        headers.insert(
            ACCEPT_LANGUAGE,
            HeaderValue::from_str(&accept_language).map_err(|e| CliError::InvalidConfig {
                message: format!("Accept-Language contains invalid characters: {e}"),
            })?,
        );

        // Accept-Encoding
        headers.insert(
            ACCEPT_ENCODING,
            HeaderValue::from_static("gzip, deflate, br"),
        );

        // Upgrade-Insecure-Requests
        headers.insert(
            HeaderName::from_static("upgrade-insecure-requests"),
            HeaderValue::from_static("1"),
        );

        // Sec-Fetch universais
        headers.insert(
            HeaderName::from_static("sec-fetch-dest"),
            HeaderValue::from_static("document"),
        );
        headers.insert(
            HeaderName::from_static("sec-fetch-mode"),
            HeaderValue::from_static("navigate"),
        );
        headers.insert(
            HeaderName::from_static("sec-fetch-site"),
            HeaderValue::from_static("none"),
        );
        headers.insert(
            HeaderName::from_static("sec-fetch-user"),
            HeaderValue::from_static("?1"),
        );

        // Client Hints — exclusivo Chrome e Edge
        if matches!(self.family, BrowserFamily::Chrome | BrowserFamily::Edge) {
            let sec_ch_ua = match self.family {
                BrowserFamily::Edge => format!(
                    r#""Chromium";v="{v}", "Microsoft Edge";v="{v}", "Not-A.Brand";v="99""#,
                    v = self.major_version
                ),
                _ => format!(
                    r#""Chromium";v="{v}", "Google Chrome";v="{v}", "Not-A.Brand";v="99""#,
                    v = self.major_version
                ),
            };
            headers.insert(
                HeaderName::from_static("sec-ch-ua"),
                HeaderValue::from_str(&sec_ch_ua).map_err(|e| CliError::InvalidConfig {
                    message: format!("Sec-CH-UA contains invalid characters: {e}"),
                })?,
            );
            headers.insert(
                HeaderName::from_static("sec-ch-ua-mobile"),
                HeaderValue::from_static("?0"),
            );
            let platform_quoted = format!(r#""{}""#, self.ua_platform);
            headers.insert(
                HeaderName::from_static("sec-ch-ua-platform"),
                HeaderValue::from_str(&platform_quoted).map_err(|e| CliError::InvalidConfig {
                    message: format!("Sec-CH-UA-Platform contains invalid characters: {e}"),
                })?,
            );
            headers.insert(CACHE_CONTROL, HeaderValue::from_static("max-age=0"));
        }

        Ok(headers)
    }

    /// Generates headers for pagination requests (same session, site already known).
    ///
    /// Difference from `construir_headers`: `Sec-Fetch-Site` becomes
    /// `same-origin` instead of `none`.
    pub fn pagination_headers(&self) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(
            HeaderName::from_static("sec-fetch-dest"),
            HeaderValue::from_static("document"),
        );
        headers.insert(
            HeaderName::from_static("sec-fetch-mode"),
            HeaderValue::from_static("navigate"),
        );
        headers.insert(
            HeaderName::from_static("sec-fetch-site"),
            HeaderValue::from_static("same-origin"),
        );
        headers.insert(
            HeaderName::from_static("sec-fetch-user"),
            HeaderValue::from_static("?1"),
        );
        headers
    }
}

// ---------------------------------------------------------------------------
// Entry TOML do arquivo user-agents.toml externo
// ---------------------------------------------------------------------------

/// TOML entry from the external `user-agents.toml` file.
#[derive(Debug, Clone, Deserialize)]
struct ExternalTomlAgent {
    ua: String,
    #[serde(default = "platform_any")]
    platform: String,
    /// Optional field: browser family (`"chrome"`, `"firefox"`, `"safari"`, `"edge"`).
    /// If absent, the family is detected automatically in `create_browser_profile()`.
    #[serde(default)]
    #[allow(dead_code)]
    browser: Option<String>,
}

fn platform_any() -> String {
    "any".to_string()
}

#[derive(Debug, Clone, Deserialize)]
struct UserAgentsFile {
    #[serde(default)]
    agents: Vec<ExternalTomlAgent>,
}

// ---------------------------------------------------------------------------
// User-Agent loading
// ---------------------------------------------------------------------------

/// Loads the User-Agent list combining the external file (if it exists) with defaults.
///
/// If `corresponde_plataforma` is true, filters by current platform (`linux`/`macos`/`windows`)
/// OR `any`. Always returns a non-empty list — on failure, uses `USER_AGENTS_DEFAULT`.
pub fn load_user_agents(match_platform: bool) -> Vec<String> {
    let Some(path) = platform::user_agents_toml_path() else {
        tracing::debug!("no config directory — using built-in UAs");
        return default_user_agents_vec();
    };

    if path.metadata().map(|m| m.len()).unwrap_or(0) > 1_048_576 {
        tracing::warn!(
            path = %path.display(),
            "user-agents.toml exceeds 1 MB limit — using built-in UAs"
        );
        return default_user_agents_vec();
    }

    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(err) => {
            tracing::info!(
                path = %path.display(),
                ?err,
                "user-agents.toml not found — using built-in UAs"
            );
            return default_user_agents_vec();
        }
    };

    let file_data: UserAgentsFile = match toml::from_str(&content) {
        Ok(a) => a,
        Err(err) => {
            tracing::warn!(
                path = %path.display(),
                ?err,
                "user-agents.toml invalid — using built-in UAs"
            );
            return default_user_agents_vec();
        }
    };

    let current_platform = platform::platform_name();
    let filtered: Vec<String> = file_data
        .agents
        .into_iter()
        .filter(|a| {
            if !match_platform {
                return true;
            }
            a.platform == "any" || a.platform == current_platform
        })
        .map(|a| a.ua)
        .filter(|ua| !ua.is_empty())
        .collect();

    if filtered.is_empty() {
        tracing::warn!("user-agents.toml produced no applicable UA — using defaults");
        return default_user_agents_vec();
    }

    tracing::info!(
        path = %path.display(),
        total = filtered.len(),
        match_platform,
        "User-Agents loaded from external user-agents.toml"
    );
    filtered
}

fn default_user_agents_vec() -> Vec<String> {
    use std::sync::OnceLock;
    static CACHE: OnceLock<Vec<String>> = OnceLock::new();
    CACHE
        .get_or_init(|| {
            USER_AGENTS_DEFAULT
                .iter()
                .map(|s| (*s).to_string())
                .collect()
        })
        .clone()
}

// ---------------------------------------------------------------------------
// User-Agent / BrowserProfile selection
// ---------------------------------------------------------------------------

/// Selects a random User-Agent from the built-in list.
pub fn select_user_agent() -> String {
    let mut rng = rand::thread_rng();
    USER_AGENTS_DEFAULT
        .choose(&mut rng)
        .copied()
        .unwrap_or(USER_AGENTS_DEFAULT[0])
        .to_string()
}

/// Selects a random User-Agent from the provided list (useful after `load_user_agents`).
///
/// If the list is empty, falls back to the built-in default.
pub fn select_user_agent_from_list(list: &[String]) -> String {
    let mut rng = rand::thread_rng();
    list.choose(&mut rng)
        .cloned()
        .unwrap_or_else(select_user_agent)
}

/// Selects a random [`BrowserProfile`] from the provided list.
///
/// Each string in the list is converted into a [`BrowserProfile`] via [`create_browser_profile`].
/// If the list is empty, creates a profile from the built-in default.
///
/// # Exemplos
///
/// ```
/// use duckduckgo_search_cli::http::{select_profile_from_list, BrowserFamily};
///
/// // Single Chrome UA list → always returns Chrome profile
/// let list = vec![
///     "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 \
///      (KHTML, like Gecko) Chrome/146.0.0.0 Safari/537.36"
///         .to_string(),
/// ];
/// let profile = select_profile_from_list(&list);
/// assert_eq!(profile.family, BrowserFamily::Chrome);
///
/// // Empty list → falls back to built-in default (returns a valid profile)
/// let profile_default = select_profile_from_list(&[]);
/// // family is one of the known BrowserFamily values
/// let _ = profile_default.family;
/// ```
pub fn select_profile_from_list(list: &[String]) -> BrowserProfile {
    let ua = select_user_agent_from_list(list);
    create_browser_profile(&ua)
}

/// Selects a [`BrowserProfile`] from the provided list using a deterministic seed.
///
/// When `seed` is `Some`, uses `SmallRng::seed_from_u64` for reproducible selection.
/// When `None`, delegates to [`select_profile_from_list`] (random).
pub fn select_profile_from_list_seeded(list: &[String], seed: Option<u64>) -> BrowserProfile {
    match seed {
        Some(s) => {
            use rand::SeedableRng;
            let mut rng = rand::rngs::StdRng::seed_from_u64(s);
            let ua = if list.is_empty() {
                USER_AGENTS_DEFAULT
                    .choose(&mut rng)
                    .copied()
                    .unwrap_or(USER_AGENTS_DEFAULT[0])
                    .to_string()
            } else {
                list.choose(&mut rng)
                    .cloned()
                    .unwrap_or_else(select_user_agent)
            };
            create_browser_profile(&ua)
        }
        None => select_profile_from_list(list),
    }
}

/// Selects a random User-Agent different from the one provided in `excluding` (when possible).
///
/// Used by the retry mechanism when HTTP 403 is detected — rotating UA reduces the chance
/// of consistent fingerprinting. If all UAs in the list match `excluding`
/// (or the list has a single item), returns any UA from the list.
pub fn select_random_user_agent(excluding: Option<&str>) -> String {
    let mut rng = rand::thread_rng();
    let chosen = USER_AGENTS_DEFAULT
        .iter()
        .filter(|ua| match excluding {
            Some(excl) => **ua != excl,
            None => true,
        })
        .choose(&mut rng);

    match chosen {
        Some(ua) => ua.to_string(),
        None => select_user_agent(),
    }
}

// ---------------------------------------------------------------------------
// Proxy configuration
// ---------------------------------------------------------------------------

/// Proxy configuration for the HTTP client.
///
/// - `Unset` → reqwest respects `HTTP_PROXY/HTTPS_PROXY/ALL_PROXY` env vars automatically.
/// - `Disabled` → `.no_proxy()` — ignores env vars.
/// - `Url(u)` → `Proxy::all(u)` with basic-auth extracted from userinfo, if present.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProxyConfig {
    /// No explicit proxy — respects `HTTP_PROXY`/`HTTPS_PROXY` env vars.
    Unset,
    /// Proxy explicitly disabled — ignores env vars.
    Disabled,
    /// Explicit proxy URL (HTTP/HTTPS/SOCKS5).
    Url(String),
}

impl ProxyConfig {
    /// Builds the configuration from the `--proxy` and `--no-proxy` flags.
    pub fn from_options(proxy: Option<&str>, no_proxy: bool) -> Self {
        if no_proxy {
            return Self::Disabled;
        }
        match proxy {
            Some(u) if !u.is_empty() => Self::Url(u.to_string()),
            _ => Self::Unset,
        }
    }

    /// Returns `true` when an explicit proxy URL is configured.
    pub fn is_active(&self) -> bool {
        matches!(self, Self::Url(_))
    }
}

// ---------------------------------------------------------------------------
// Client construction
// ---------------------------------------------------------------------------

/// Builds a `reqwest::Client` ready to make requests to `DuckDuckGo`.
///
/// # Arguments
/// * `user_agent` — User-Agent string to be sent on all requests.
/// * `timeout_secs` — total timeout (including body read).
/// * `language` — language code for the `Accept-Language` header (e.g. `"pt"`).
/// * `country` — country code for the `Accept-Language` header (e.g. `"br"`).
///
/// # Errors
/// Returns an error if the `ClientBuilder` build fails.
pub fn build_client(
    user_agent: &str,
    timeout_secs: u64,
    language: &str,
    country: &str,
) -> Result<Client, CliError> {
    let profile = create_browser_profile(user_agent);
    build_client_with_proxy(
        &profile,
        timeout_secs,
        language,
        country,
        &ProxyConfig::Unset,
    )
}

/// Masks credentials in a proxy URL for safe use in logs and error messages.
///
/// Transforms `http://user:password@proxy:8080` into `http://us***@proxy:8080`.
/// If the URL contains no credentials, returns the safe representation without userinfo.
fn mask_proxy_url(raw_url: &str) -> String {
    match reqwest::Url::parse(raw_url) {
        Ok(parsed) => {
            let user = parsed.username();
            let has_password = parsed.password().is_some();

            if user.is_empty() && !has_password {
                return format!(
                    "{}://{}{}",
                    parsed.scheme(),
                    parsed.host_str().unwrap_or("?"),
                    parsed.port().map(|p| format!(":{p}")).unwrap_or_default()
                );
            }

            let masked_user = if user.len() > 2 {
                format!("{}***", &user[..2])
            } else {
                format!("{user}***")
            };

            format!(
                "{}://{}@{}{}",
                parsed.scheme(),
                masked_user,
                parsed.host_str().unwrap_or("?"),
                parsed.port().map(|p| format!(":{p}")).unwrap_or_default()
            )
        }
        Err(_) => "***URL_MALFORMADA***".to_string(),
    }
}

/// Builds a `reqwest::Client` with a browser profile and proxy configuration.
///
/// Uses [`BrowserProfile::initial_headers`] to generate family-specific headers,
/// including complete Sec-Fetch and Client Hints (Chrome/Edge).
///
/// # Arguments
/// * `profile` — browser profile that defines headers per family.
/// * `timeout_secs` — total timeout.
/// * `language` — language code (e.g. `"pt"`).
/// * `country` — country code (e.g. `"br"`).
/// * `proxy` — proxy configuration.
///
/// # Errors
/// Returns an error if the headers are invalid or the proxy configuration fails.
pub fn build_client_with_proxy(
    profile: &BrowserProfile,
    timeout_secs: u64,
    language: &str,
    country: &str,
    proxy: &ProxyConfig,
) -> Result<Client, CliError> {
    let headers = profile.initial_headers(language, country)?;

    let mut builder = Client::builder()
        .user_agent(&profile.user_agent)
        .default_headers(headers)
        .cookie_store(true)
        .tcp_nodelay(true)
        .tcp_keepalive(Duration::from_secs(60))
        .pool_max_idle_per_host(10)
        .connect_timeout(Duration::from_secs(10))
        .gzip(true)
        .brotli(true)
        .redirect(Policy::limited(5))
        .timeout(Duration::from_secs(timeout_secs));

    match proxy {
        ProxyConfig::Unset => {}
        ProxyConfig::Disabled => {
            builder = builder.no_proxy();
            tracing::info!("proxy explicitly disabled via --no-proxy");
        }
        ProxyConfig::Url(url) => {
            let parsed_url = reqwest::Url::parse(url).map_err(|e| CliError::ProxyError {
                message: format!("invalid proxy URL {}: {e}", mask_proxy_url(url)),
            })?;
            let user = parsed_url.username().to_string();
            let password = parsed_url
                .password()
                .map(|s| s.to_string())
                .unwrap_or_default();

            let mut proxy_rq = reqwest::Proxy::all(url).map_err(|e| CliError::ProxyError {
                message: format!(
                    "failed to configure Proxy::all({}): {e}",
                    mask_proxy_url(url)
                ),
            })?;

            if !user.is_empty() {
                proxy_rq = proxy_rq.basic_auth(&user, &password);
            }
            builder = builder.proxy(proxy_rq);
            tracing::info!(
                host = parsed_url.host_str(),
                scheme = parsed_url.scheme(),
                "proxy configured"
            );
        }
    }

    let client = builder.build().map_err(|e| CliError::HttpError {
        message: format!("failed to build reqwest::Client: {e}"),
        cause: None,
    })?;

    Ok(client)
}

// ---------------------------------------------------------------------------
// Testes
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // --- Testes existentes ---------------------------------------------------

    #[test]
    fn choose_user_agent_returns_non_empty_string() {
        let ua = select_user_agent();
        assert!(!ua.is_empty());
    }

    #[test]
    fn choose_user_agent_returns_modern_ua_from_pool() {
        let ua = select_user_agent();
        assert!(
            USER_AGENTS_DEFAULT.contains(&ua.as_str()),
            "UA selecionado deve estar na lista padrão: {ua}"
        );
        assert!(
            ua.starts_with("Mozilla/5.0 ("),
            "UAs padrão v0.3.0 iniciam com 'Mozilla/5.0 (' (browser real): {ua}"
        );
    }

    #[test]
    fn default_pool_contains_modern_browsers_in_all_families() {
        let pool = USER_AGENTS_DEFAULT;
        assert!(pool.iter().any(|ua| ua.contains("Chrome/")));
        assert!(pool.iter().any(|ua| ua.contains("Firefox/")));
        assert!(pool.iter().any(|ua| ua.contains("Edg/")));
        assert!(pool
            .iter()
            .any(|ua| ua.contains("Safari/") && !ua.contains("Chrome/")));
    }

    #[test]
    fn default_pool_does_not_contain_removed_text_browsers() {
        for ua in USER_AGENTS_DEFAULT {
            assert!(!ua.contains("Lynx"), "UA banido detectado (Lynx): {ua}");
            assert!(!ua.contains("w3m"), "UA banido detectado (w3m): {ua}");
            assert!(
                !ua.starts_with("Links ("),
                "UA banido detectado (Links): {ua}"
            );
            assert!(!ua.contains("ELinks"), "UA banido detectado (ELinks): {ua}");
            assert!(
                !ua.starts_with("duckduckgo-search-cli"),
                "UA banido detectado (self-cli): {ua}"
            );
            assert_ne!(
                *ua, "Mozilla/5.0",
                "UA minimalista 'Mozilla/5.0' deve ter sido removido"
            );
        }
        assert!(!USER_AGENTS_DEFAULT.is_empty());
    }

    #[test]
    fn select_random_user_agent_without_exclusion_returns_valid() {
        let ua = select_random_user_agent(None);
        assert!(!ua.is_empty());
    }

    #[test]
    fn select_random_user_agent_avoids_excluded_when_possible() {
        let excluded = USER_AGENTS_DEFAULT[0];
        for _ in 0..20 {
            let ua = select_random_user_agent(Some(excluded));
            assert_ne!(ua, excluded);
            assert!(!ua.is_empty());
        }
    }

    #[test]
    fn build_client_with_valid_values_works() {
        let client = build_client("Mozilla/5.0 teste", 15, "pt", "br");
        assert!(client.is_ok(), "cliente deve ser construído sem erro");
    }

    #[test]
    fn build_client_with_http_proxy_works() {
        let profile = create_browser_profile("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/146.0.0.0 Safari/537.36");
        let proxy = ProxyConfig::Url("http://user:pass@proxy.local:8080".to_string());
        let client = build_client_with_proxy(&profile, 10, "pt", "br", &proxy);
        assert!(client.is_ok(), "client with HTTP proxy should build");
    }

    #[test]
    fn build_client_with_socks5_proxy_works() {
        let profile = create_browser_profile(
            "Mozilla/5.0 (X11; Linux x86_64; rv:134.0) Gecko/20100101 Firefox/134.0",
        );
        let proxy = ProxyConfig::Url("socks5://127.0.0.1:9050".to_string());
        let client = build_client_with_proxy(&profile, 10, "pt", "br", &proxy);
        assert!(client.is_ok(), "client with SOCKS5 should build");
    }

    #[test]
    fn build_client_with_no_proxy_works() {
        let profile = create_browser_profile(
            "Mozilla/5.0 (X11; Linux x86_64; rv:134.0) Gecko/20100101 Firefox/134.0",
        );
        let proxy = ProxyConfig::Disabled;
        let client = build_client_with_proxy(&profile, 10, "pt", "br", &proxy);
        assert!(client.is_ok(), "client with no_proxy should build");
    }

    #[test]
    fn build_client_with_invalid_proxy_url_fails() {
        let profile = create_browser_profile(
            "Mozilla/5.0 (X11; Linux x86_64; rv:134.0) Gecko/20100101 Firefox/134.0",
        );
        let proxy = ProxyConfig::Url("nao eh uma url".to_string());
        let client = build_client_with_proxy(&profile, 10, "pt", "br", &proxy);
        assert!(client.is_err(), "invalid URL should be rejected");
    }

    #[test]
    fn proxy_config_from_flags() {
        assert_eq!(ProxyConfig::from_options(None, false), ProxyConfig::Unset);
        assert_eq!(ProxyConfig::from_options(None, true), ProxyConfig::Disabled);
        assert_eq!(
            ProxyConfig::from_options(Some("http://x:9"), false),
            ProxyConfig::Url("http://x:9".to_string())
        );
        assert_eq!(
            ProxyConfig::from_options(Some("http://x:9"), true),
            ProxyConfig::Disabled
        );
    }

    #[test]
    fn proxy_config_is_active_only_for_url() {
        assert!(!ProxyConfig::Unset.is_active());
        assert!(!ProxyConfig::Disabled.is_active());
        assert!(ProxyConfig::Url("http://x".to_string()).is_active());
    }

    #[test]
    fn mask_proxy_url_with_credentials() {
        let result = mask_proxy_url("http://admin:s3cret@proxy.local:8080");
        assert!(!result.contains("s3cret"), "password vazou: {result}");
        assert!(
            !result.contains("admin"),
            "username completo vazou: {result}"
        );
        assert!(
            result.contains("ad***"),
            "username mascarado ausente: {result}"
        );
        assert!(result.contains("proxy.local"));
        assert!(result.contains("8080"));
    }

    #[test]
    fn mask_proxy_url_without_credentials() {
        let result = mask_proxy_url("http://proxy.local:8080");
        assert_eq!(result, "http://proxy.local:8080");
    }

    #[test]
    fn mask_proxy_url_username_only() {
        let result = mask_proxy_url("http://user@proxy.local:3128");
        assert!(result.contains("us***"));
        assert!(!result.contains("user@"));
    }

    #[test]
    fn mask_proxy_url_malformed() {
        let result = mask_proxy_url("not-a-url");
        assert_eq!(result, "***URL_MALFORMADA***");
    }

    #[test]
    fn mask_proxy_url_socks5() {
        let result = mask_proxy_url("socks5://root:toor@127.0.0.1:1080");
        assert!(!result.contains("toor"));
        assert!(result.contains("socks5://"));
        assert!(result.contains("127.0.0.1"));
    }

    #[test]
    fn mask_proxy_url_short_username() {
        let result = mask_proxy_url("http://a:pass@proxy:80");
        assert!(result.contains("a***"));
        assert!(!result.contains("pass"));
    }

    #[test]
    fn load_user_agents_returns_at_least_one_default() {
        let agents = load_user_agents(false);
        assert!(!agents.is_empty());
        for ua in &agents {
            assert!(!ua.is_empty());
        }
    }

    #[test]
    fn choose_user_agent_from_list_returns_list_item() {
        let agents = vec!["A".to_string(), "B".to_string(), "C".to_string()];
        for _ in 0..10 {
            let selected = select_user_agent_from_list(&agents);
            assert!(agents.contains(&selected));
        }
    }

    // --- Testes novos: BrowserProfile -----------------------------------------

    #[test]
    fn detect_family_chrome() {
        let uas_chrome = [
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/146.0.0.0 Safari/537.36",
            "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/146.0.0.0 Safari/537.36",
            "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/146.0.0.0 Safari/537.36",
        ];
        for ua in &uas_chrome {
            assert_eq!(
                detect_family(ua),
                BrowserFamily::Chrome,
                "esperado Chrome para: {ua}"
            );
        }
    }

    #[test]
    fn detect_family_edge_before_chrome() {
        // Edge UA contains "Chrome/" but must return Edge because it has "Edg/" first
        let ua = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/145.0.0.0 Safari/537.36 Edg/145.0.3800.97";
        assert_eq!(detect_family(ua), BrowserFamily::Edge);
    }

    #[test]
    fn detect_family_firefox() {
        let ua = "Mozilla/5.0 (X11; Linux x86_64; rv:134.0) Gecko/20100101 Firefox/134.0";
        assert_eq!(detect_family(ua), BrowserFamily::Firefox);
    }

    #[test]
    fn detect_family_safari() {
        // Pure Safari does not contain "Chrome/"
        let ua = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.6 Safari/605.1.15";
        assert_eq!(detect_family(ua), BrowserFamily::Safari);
    }

    #[test]
    fn extract_major_version_chrome_146() {
        let ua = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/146.0.0.0 Safari/537.36";
        let version = extract_major_version(ua, BrowserFamily::Chrome);
        assert_eq!(version, 146, "versão major Chrome deve ser 146");
    }

    #[test]
    fn extract_major_version_firefox_134() {
        let ua = "Mozilla/5.0 (X11; Linux x86_64; rv:134.0) Gecko/20100101 Firefox/134.0";
        let version = extract_major_version(ua, BrowserFamily::Firefox);
        assert_eq!(version, 134, "versão major Firefox deve ser 134");
    }

    #[test]
    fn initial_chrome_headers_include_sec_fetch() {
        let ua = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/146.0.0.0 Safari/537.36";
        let profile = create_browser_profile(ua);
        let headers = profile
            .initial_headers("pt", "br")
            .expect("should build headers");
        assert!(
            headers.contains_key("sec-fetch-dest"),
            "sec-fetch-dest ausente"
        );
        assert!(
            headers.contains_key("sec-fetch-mode"),
            "sec-fetch-mode ausente"
        );
        assert!(
            headers.contains_key("sec-fetch-site"),
            "sec-fetch-site ausente"
        );
        assert!(
            headers.contains_key("sec-fetch-user"),
            "sec-fetch-user ausente"
        );
    }

    #[test]
    fn initial_chrome_headers_include_client_hints() {
        let ua = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/146.0.0.0 Safari/537.36";
        let profile = create_browser_profile(ua);
        let headers = profile
            .initial_headers("pt", "br")
            .expect("should build headers");
        assert!(headers.contains_key("sec-ch-ua"), "sec-ch-ua ausente");
        assert!(
            headers.contains_key("sec-ch-ua-mobile"),
            "sec-ch-ua-mobile ausente"
        );
        assert!(
            headers.contains_key("sec-ch-ua-platform"),
            "sec-ch-ua-platform ausente"
        );
    }

    #[test]
    fn initial_firefox_headers_omit_client_hints() {
        let ua = "Mozilla/5.0 (X11; Linux x86_64; rv:134.0) Gecko/20100101 Firefox/134.0";
        let profile = create_browser_profile(ua);
        let headers = profile
            .initial_headers("pt", "br")
            .expect("should build headers");
        assert!(
            !headers.contains_key("sec-ch-ua"),
            "Firefox NÃO deve ter sec-ch-ua"
        );
        assert!(
            !headers.contains_key("sec-ch-ua-mobile"),
            "Firefox NÃO deve ter sec-ch-ua-mobile"
        );
        assert!(
            !headers.contains_key("sec-ch-ua-platform"),
            "Firefox NÃO deve ter sec-ch-ua-platform"
        );
    }

    #[test]
    fn pagination_headers_sec_fetch_site_same_origin() {
        let ua = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/146.0.0.0 Safari/537.36";
        let profile = create_browser_profile(ua);
        let headers = profile.pagination_headers();
        let value = headers
            .get("sec-fetch-site")
            .expect("sec-fetch-site should be present");
        assert_eq!(value.to_str().unwrap(), "same-origin");
    }

    #[test]
    fn accept_language_with_q_values_pt() {
        let ua = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/146.0.0.0 Safari/537.36";
        let profile = create_browser_profile(ua);
        let headers = profile
            .initial_headers("pt", "br")
            .expect("should build headers");
        let al = headers
            .get(ACCEPT_LANGUAGE)
            .expect("Accept-Language present");
        let al_str = al.to_str().unwrap();
        assert!(al_str.contains("pt-BR"), "deve conter pt-BR: {al_str}");
        assert!(
            al_str.contains("pt;q=0.9"),
            "deve conter pt;q=0.9: {al_str}"
        );
        assert!(
            al_str.contains("en-US;q=0.8"),
            "deve conter en-US;q=0.8: {al_str}"
        );
        assert!(
            al_str.contains("en;q=0.7"),
            "deve conter en;q=0.7: {al_str}"
        );
    }

    #[test]
    fn accept_language_with_q_values_en() {
        let ua = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/146.0.0.0 Safari/537.36";
        let profile = create_browser_profile(ua);
        let headers = profile
            .initial_headers("en", "us")
            .expect("should build headers");
        let al = headers
            .get(ACCEPT_LANGUAGE)
            .expect("Accept-Language present");
        let al_str = al.to_str().unwrap();
        assert_eq!(
            al_str, "en-US,en;q=0.9",
            "formato en deve ser simplificado: {al_str}"
        );
    }

    // Testes existentes atualizados para usar BrowserProfile

    #[test]
    fn default_headers_include_accept_and_language() {
        // Teste atualizado para usar BrowserProfile em vez de headers_padrao()
        let ua = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/146.0.0.0 Safari/537.36";
        let profile = create_browser_profile(ua);
        let headers = profile
            .initial_headers("pt", "br")
            .expect("should build headers");
        let accept = headers.get(ACCEPT).expect("ACCEPT present");
        assert!(accept.to_str().unwrap().contains("text/html"));
        let al = headers
            .get(ACCEPT_LANGUAGE)
            .expect("ACCEPT_LANGUAGE present");
        assert!(al.to_str().unwrap().contains("pt-BR"));
    }

    #[test]
    fn default_headers_omit_dnt_and_referer() {
        // Empirical finding iter. 4: persistent DNT + Referer reveal fingerprint.
        // Updated to use BrowserProfile.
        let ua = "Mozilla/5.0 (X11; Linux x86_64; rv:134.0) Gecko/20100101 Firefox/134.0";
        let profile = create_browser_profile(ua);
        let headers = profile
            .initial_headers("en", "us")
            .expect("should build headers");
        assert!(headers.get(reqwest::header::DNT).is_none());
        assert!(headers.get(reqwest::header::REFERER).is_none());
    }
}
