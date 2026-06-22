// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload: declarative (cookie jar wrapper for reqwest + JSON persistence)
//! v0.7.3 PR2 / v0.8.6 — Bridge between `reqwest::cookie::Jar` (the in-memory
//! cookie store used by `reqwest::Client::cookie_provider`) and the JSON file
//! format produced by [`crate::session_warmup::default_cookies_path`].
//!
//! `reqwest::cookie::Jar` implements `reqwest::cookie::CookieStore` natively.
//! However, `Jar` does not expose iteration over stored cookies. We persist
//! cookies by extracting the `Cookie` header via `CookieStore::cookies()` for
//! the `DuckDuckGo` domain and rebuild the jar from the file on each invocation
//! using `Jar::add_cookie_str()`.

use crate::error::CliError;
use std::path::Path;

/// A `reqwest::cookie::Jar` paired with a backing JSON file path.
///
/// Constructed once per CLI invocation. The jar is the active cookie
/// store passed to `reqwest::Client::cookie_provider`; the file path is
/// the on-disk projection used by [`SessionWarmup`] to read and write
/// the persistent jar.
///
/// [`SessionWarmup`]: crate::session_warmup::default_cookies_path
#[derive(Clone, Debug)]
pub struct PersistentJar {
    /// The active in-memory jar shared with `reqwest::Client`.
    pub jar: std::sync::Arc<reqwest::cookie::Jar>,
    /// Path to the JSON file on disk. `None` disables persistence.
    pub path: Option<std::path::PathBuf>,
}

impl PersistentJar {
    /// Creates a new empty persistent jar at the given path.
    pub fn empty(path: Option<std::path::PathBuf>) -> Self {
        Self {
            jar: std::sync::Arc::new(reqwest::cookie::Jar::default()),
            path,
        }
    }

    /// Loads the JSON projection from disk into a fresh jar.
    ///
    /// Returns an empty jar if persistence is disabled, the file is
    /// missing, or the file is malformed. Malformed files are logged
    /// and treated as empty (so a corrupt jar does not break the CLI).
    pub fn load(path: Option<std::path::PathBuf>) -> Self {
        let jar = match path.as_ref() {
            Some(p) if p.exists() => match std::fs::read_to_string(p) {
                Ok(content) => match Self::parse_json(&content) {
                    Ok(jar) => jar,
                    Err(e) => {
                        tracing::warn!(
                            error = %e,
                            path = %p.display(),
                            "cookie jar is malformed — starting with empty jar"
                        );
                        reqwest::cookie::Jar::default()
                    }
                },
                Err(e) => {
                    tracing::warn!(
                        error = %e,
                        path = %p.display(),
                        "failed to read cookie jar — starting with empty jar"
                    );
                    reqwest::cookie::Jar::default()
                }
            },
            _ => reqwest::cookie::Jar::default(),
        };
        Self {
            jar: std::sync::Arc::new(jar),
            path,
        }
    }

    /// Persists the current jar to disk in JSON format.
    ///
    /// On Unix, the file is written with mode `0o600` (owner read+write
    /// only) because it contains session credentials. Errors are logged
    /// but never fatal — a failed save does not break the current
    /// invocation.
    pub fn save(&self) {
        let Some(path) = &self.path else {
            return;
        };
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() && !parent.exists() {
                if let Err(e) = std::fs::create_dir_all(parent) {
                    tracing::warn!(
                        error = %e,
                        path = %parent.display(),
                        "failed to create cookie jar parent dir"
                    );
                    return;
                }
            }
        }
        let json = match Self::to_json(&self.jar) {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!(error = %e, "failed to serialize cookie jar");
                return;
            }
        };
        if let Err(e) = std::fs::write(path, json.as_bytes()) {
            tracing::warn!(
                error = %e,
                path = %path.display(),
                "failed to persist cookie jar to disk"
            );
            return;
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let perms = std::fs::Permissions::from_mode(0o600);
            if let Err(e) = std::fs::set_permissions(path, perms) {
                tracing::warn!(
                    error = %e,
                    "failed to set 0o600 permissions on cookie jar"
                );
            }
        }
    }

    /// Returns a shared reference suitable for `reqwest::Client::cookie_provider`.
    pub fn as_provider(&self) -> std::sync::Arc<reqwest::cookie::Jar> {
        self.jar.clone()
    }

    /// Performs a `GET <url>` warm-up request to populate session cookies.
    ///
    /// Returns silently on any error — the warm-up is best-effort and
    /// failure here must not break the real query.
    ///
    /// # Errors
    ///
    /// Returns `Err` if the warm-up HTTP request itself fails (network
    /// error, TLS error, etc.). The pipeline logs and continues.
    pub async fn warm_up(&self, client: &reqwest::Client) -> Result<(), CliError> {
        client
            .get("https://duckduckgo.com/")
            .send()
            .await
            .map_err(|e| CliError::HttpError {
                message: format!("warm-up request failed: {e}"),
                cause: None,
            })?;
        Ok(())
    }

    /// Serializes the jar to a stable JSON projection.
    ///
    /// Since `reqwest::cookie::Jar` does not expose iteration, we extract
    /// cookies via `CookieStore::cookies()` for the `DuckDuckGo` domain and
    /// parse the combined `Cookie` header into individual `name=value` pairs.
    ///
    /// # Errors
    ///
    /// Returns `Err` if `serde_json::to_string` fails.
    #[allow(clippy::missing_panics_doc)]
    pub fn to_json(jar: &reqwest::cookie::Jar) -> serde_json::Result<String> {
        use reqwest::cookie::CookieStore;
        // SAFETY: this URL is a compile-time constant and always parses successfully.
        let ddg_url: url::Url = "https://duckduckgo.com/".parse().expect("hardcoded URL");
        let cookies: Vec<serde_json::Value> = match jar.cookies(&ddg_url) {
            Some(header_value) => {
                let header_str = header_value.to_str().unwrap_or("");
                header_str
                    .split("; ")
                    .filter_map(|pair| {
                        let mut parts = pair.splitn(2, '=');
                        let name = parts.next()?.trim();
                        let value = parts.next().unwrap_or("").trim();
                        if name.is_empty() {
                            return None;
                        }
                        Some(serde_json::json!({
                            "name": name,
                            "value": value,
                            "domain": "duckduckgo.com",
                        }))
                    })
                    .collect()
            }
            None => Vec::new(),
        };
        serde_json::to_string(&cookies)
    }

    /// Parses a JSON projection (as written by `to_json`) into a fresh
    /// `reqwest::cookie::Jar`. Each entry is converted to a cookie string
    /// and added via `Jar::add_cookie_str()`.
    ///
    /// # Errors
    ///
    /// Returns `Err` if the JSON is malformed. Malformed entries inside a
    /// valid JSON array are skipped silently.
    pub fn parse_json(content: &str) -> serde_json::Result<reqwest::cookie::Jar> {
        let entries: Vec<serde_json::Value> = serde_json::from_str(content)?;
        let jar = reqwest::cookie::Jar::default();
        for entry in entries {
            let name = entry
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or_default();
            let value = entry
                .get("value")
                .and_then(|v| v.as_str())
                .unwrap_or_default();
            let domain = entry
                .get("domain")
                .and_then(|v| v.as_str())
                .unwrap_or("duckduckgo.com");
            let secure = entry
                .get("secure")
                .and_then(|v| v.as_bool())
                .unwrap_or(true);
            let scheme = if secure { "https" } else { "http" };
            let url_str = format!("{scheme}://{domain}/");
            if let Ok(url) = url_str.parse::<url::Url>() {
                let cookie_str = format!("{name}={value}");
                jar.add_cookie_str(&cookie_str, &url);
            }
        }
        Ok(jar)
    }
}

/// Computes the default cookie jar file path under the XDG config directory.
///
/// # Errors
///
/// Returns `Err` if `dirs::config_dir()` returns `None`.
pub fn default_cookies_path() -> Result<std::path::PathBuf, CliError> {
    let base = dirs::config_dir().ok_or_else(|| CliError::PathError {
        message: "could not determine user config directory for cookie jar".into(),
    })?;
    Ok(base
        .join("duckduckgo-search-cli")
        .join(crate::session_warmup::DEFAULT_COOKIES_FILENAME))
}

/// Returns the XDG-relative cookie path for use with `Path::new`.
pub fn default_cookies_path_for(path: &Path) -> &Path {
    path
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn empty_jar_serializes_to_empty_array() {
        let jar = reqwest::cookie::Jar::default();
        let json = PersistentJar::to_json(&jar).expect("serialize");
        assert_eq!(json, "[]");
    }

    #[test]
    fn malformed_json_yields_parse_error() {
        let result = PersistentJar::parse_json("not json {{{{");
        assert!(result.is_err(), "expected parse error, got {result:?}");
    }

    #[test]
    fn round_trip_preserves_cookie() {
        let jar = reqwest::cookie::Jar::default();
        let url: url::Url = "https://duckduckgo.com/".parse().unwrap();
        jar.add_cookie_str("kl=br-pt", &url);
        let json = PersistentJar::to_json(&jar).expect("serialize");
        assert!(json.contains("kl"));
        assert!(json.contains("br-pt"));
    }

    #[test]
    fn persistent_jar_empty_creates_empty_jar() {
        let pj = PersistentJar::empty(None);
        assert!(
            pj.path.is_none(),
            "path should be None when constructed with None"
        );
        let json = PersistentJar::to_json(&pj.jar).expect("serialize empty");
        assert_eq!(json, "[]", "empty jar serializes to empty JSON array");
    }

    #[test]
    fn persistent_jar_load_missing_file_returns_empty() {
        let pj = PersistentJar::load(Some(PathBuf::from("/nonexistent/path/cookies.json")));
        let json = PersistentJar::to_json(&pj.jar).expect("serialize empty fallback");
        assert_eq!(json, "[]", "missing file should fallback to empty jar");
    }

    #[test]
    fn persistent_jar_save_writes_to_disk() {
        let tmp = tempfile::tempdir().expect("create tempdir");
        let path = tmp.path().join("cookies.json");
        let pj = PersistentJar::empty(Some(path.clone()));
        pj.save();
        assert!(path.exists(), "save() must create the cookies file");
        let content = std::fs::read_to_string(&path).expect("read file");
        assert_eq!(content, "[]", "empty jar written to disk as empty array");
    }

    #[test]
    fn default_cookies_path_returns_path_under_config_dir() {
        let path = default_cookies_path().expect("default path should resolve");
        let s = path.to_string_lossy();
        assert!(
            s.contains("duckduckgo-search-cli") && s.ends_with("cookies.json"),
            "default path must be <config>/duckduckgo-search-cli/cookies.json, got {s}"
        );
    }
}
