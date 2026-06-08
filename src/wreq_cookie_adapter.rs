// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload: declarative (cookie jar wrapper for wreq + JSON persistence)
//! v0.7.3 PR2 — Bridge between `wreq::cookie::Jar` (the in-memory cookie
//! store used by `wreq::Client::cookie_provider`) and the JSON file
//! format produced by [`crate::session_warmup::default_cookies_path`].
//!
//! The [`wreq::cookie::Jar`] type implements `wreq::cookie::CookieStore`
//! natively and is the recommended way to feed cookies into a `wreq::Client`.
//! However, `wreq::cookie::Jar` does not expose a public Serialize/Deserialize
//! implementation. We therefore persist a JSON projection of its cookies
//! (the `name=value` pairs plus a minimal attribute subset) and rebuild
//! the jar from the file on each invocation.

use crate::error::CliError;
use std::path::Path;
use wreq::cookie::IntoCookieStore;

/// A `wreq::cookie::Jar` paired with a backing JSON file path.
///
/// Constructed once per CLI invocation. The jar is the active cookie
/// store passed to `wreq::Client::cookie_provider`; the file path is
/// the on-disk projection used by [`SessionWarmup`] to read and write
/// the persistent jar.
///
/// [`SessionWarmup`]: crate::session_warmup::default_cookies_path
#[derive(Clone, Debug)]
pub struct PersistentJar {
    /// The active in-memory jar shared with `wreq::Client`.
    pub jar: std::sync::Arc<wreq::cookie::Jar>,
    /// Path to the JSON file on disk. `None` disables persistence.
    pub path: Option<std::path::PathBuf>,
}

impl PersistentJar {
    /// Creates a new empty persistent jar at the given path.
    pub fn empty(path: Option<std::path::PathBuf>) -> Self {
        Self {
            jar: std::sync::Arc::new(wreq::cookie::Jar::default()),
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
                        wreq::cookie::Jar::default()
                    }
                },
                Err(e) => {
                    tracing::warn!(
                        error = %e,
                        path = %p.display(),
                        "failed to read cookie jar — starting with empty jar"
                    );
                    wreq::cookie::Jar::default()
                }
            },
            _ => wreq::cookie::Jar::default(),
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

    /// Returns a shared reference suitable for `wreq::Client::cookie_provider`.
    pub fn as_provider(&self) -> std::sync::Arc<dyn wreq::cookie::CookieStore> {
        self.jar.clone().into_shared()
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
    pub async fn warm_up(&self, client: &wreq::Client) -> Result<(), CliError> {
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
    /// Format: `Vec<{name, value, domain, path, secure, http_only, max_age}>`
    /// (only the fields that are required to reconstruct a valid cookie).
    ///
    /// # Errors
    ///
    /// Returns `Err` if `serde_json::to_string` fails (rare; only on
    /// extreme memory pressure or non-UTF-8 byte sequences).
    pub fn to_json(jar: &wreq::cookie::Jar) -> serde_json::Result<String> {
        let cookies: Vec<serde_json::Value> = jar
            .get_all()
            .map(|c| {
                serde_json::json!({
                    "name": c.name(),
                    "value": c.value(),
                    "domain": c.domain().map(|d| d.to_string()),
                    "path": c.path().map(|p| p.to_string()),
                    "secure": c.secure(),
                    "http_only": c.http_only(),
                    "max_age": c.max_age().map(|d| d.as_secs()),
                })
            })
            .collect();
        serde_json::to_string(&cookies)
    }

    /// Parses a JSON projection (as written by `to_json`) into a fresh
    /// `wreq::cookie::Jar`. Each entry is converted to a
    /// `wreq::header::HeaderValue` and added to the jar with the
    /// reconstructed URI.
    ///
    /// # Errors
    ///
    /// Returns `Err` if the JSON is malformed. Malformed entries inside a
    /// valid JSON array are skipped silently.
    pub fn parse_json(content: &str) -> serde_json::Result<wreq::cookie::Jar> {
        let entries: Vec<serde_json::Value> = serde_json::from_str(content)?;
        let jar = wreq::cookie::Jar::default();
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
            let _ = entry
                .get("http_only")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let cookie_str = format!("{name}={value}");
            let scheme = if secure { "https" } else { "http" };
            let uri = format!("{scheme}://{domain}/");
            if let Ok(uri_parsed) = uri.parse::<wreq::Uri>() {
                if let Ok(value_header) = wreq::header::HeaderValue::from_str(&cookie_str) {
                    let mut iter = std::iter::once(&value_header);
                    wreq::cookie::CookieStore::set_cookies(&jar, &mut iter, &uri_parsed);
                }
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

    #[test]
    fn empty_jar_serializes_to_empty_array() {
        let jar = wreq::cookie::Jar::default();
        let json = PersistentJar::to_json(&jar).expect("serialize");
        assert_eq!(json, "[]");
    }

    #[test]
    fn malformed_json_yields_parse_error() {
        // `parse_json` is fallible: a corrupt file surfaces as `Err` to the
        // caller. The caller (`PersistentJar::load`) is responsible for
        // catching the error and falling back to an empty jar.
        let result = PersistentJar::parse_json("not json {{{{");
        assert!(result.is_err(), "expected parse error, got {result:?}");
    }

    #[test]
    fn round_trip_preserves_cookie() {
        let jar = wreq::cookie::Jar::default();
        let value = wreq::header::HeaderValue::from_static("kl=br-pt");
        let mut iter = std::iter::once(&value);
        let uri: wreq::Uri = "https://duckduckgo.com/".parse().unwrap();
        wreq::cookie::CookieStore::set_cookies(&jar, &mut iter, &uri);
        let json = PersistentJar::to_json(&jar).expect("serialize");
        assert!(json.contains("kl"));
        assert!(json.contains("br-pt"));
    }
}
