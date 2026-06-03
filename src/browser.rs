// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload: I/O-bound (Chrome CDP connection, feature-gated)
//! Cross-platform detection and launch of headless Chrome via `chromiumoxide`.
//!
//! This module is only compiled with the `chrome` feature, enabled via
//! `cargo build --features chrome`. In default mode (without feature) the binary
//! has NO dependency on chromiumoxide/tempfile/futures — zero overhead.
//!
//! ## Responsibilities
//!
//! 1. [`detect_chrome`] — detects the Chrome/Chromium executable path on the
//!    system, with a 3-layer hierarchy (manual flag → env var → auto-detection).
//! 2. [`ChromeBrowser`] — safe wrapper over `chromiumoxide::Browser` that
//!    ensures process cleanup and handler-task via `impl Drop`.
//! 3. [`extract_text_with_chrome`] — navigation + extraction of `document.body.innerText`
//!    with configurable timeout.
//!
//! ## Process Cleanup and Safety (rules_rust.md — Memory Management)
//!
//! `chromiumoxide::Browser` starts a child Chrome process. Without explicit cleanup,
//! the process becomes a zombie. The [`Drop`] implementation on [`ChromeBrowser`]
//! aborts the handler task and signals `kill_on_drop` internally. For complete
//! synchronous cleanup, prefer calling [`ChromeBrowser::shutdown`] before drop.

#![cfg(feature = "chrome")]

use crate::error::CliError;
use chromiumoxide::browser::{Browser, BrowserConfig, HeadlessMode};
use futures::StreamExt;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::task::JoinHandle;

/// Minimum character count per line kept by the cleaning pipeline.
const MIN_LINE_LENGTH: usize = 20;

/// Returns an ordered list of candidate paths for Chrome/Chromium by platform.
///
/// Includes native installations, Flatpak, and Snap. Windows consults
/// environment variables (`%PROGRAMFILES%`, `%LOCALAPPDATA%`) when available.
pub fn chrome_candidate_paths() -> Vec<PathBuf> {
    let mut candidates: Vec<PathBuf> = Vec::new();

    #[cfg(target_os = "linux")]
    {
        for base in [
            "/usr/bin/google-chrome",
            "/usr/bin/google-chrome-stable",
            "/usr/bin/chromium",
            "/usr/bin/chromium-browser",
            "/usr/local/bin/chromium",
            "/usr/local/bin/google-chrome",
            "/opt/google/chrome/chrome",
            "/snap/bin/chromium",
            "/snap/bin/google-chrome",
            "/var/lib/flatpak/exports/bin/com.google.Chrome",
            "/var/lib/flatpak/exports/bin/org.chromium.Chromium",
        ] {
            candidates.push(PathBuf::from(base));
        }
        if let Some(home) = dirs::home_dir() {
            candidates.push(home.join(".local/share/flatpak/exports/bin/com.google.Chrome"));
            candidates.push(home.join(".local/share/flatpak/exports/bin/org.chromium.Chromium"));
        }
    }

    #[cfg(target_os = "macos")]
    {
        for base in [
            "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
            "/Applications/Chromium.app/Contents/MacOS/Chromium",
            "/opt/homebrew/bin/chromium",
            "/opt/homebrew/bin/google-chrome",
            "/usr/local/bin/chromium",
            "/usr/local/bin/google-chrome",
        ] {
            candidates.push(PathBuf::from(base));
        }
    }

    #[cfg(target_os = "windows")]
    {
        // Known base paths.
        for base in [
            r"C:\Program Files\Google\Chrome\Application\chrome.exe",
            r"C:\Program Files (x86)\Google\Chrome\Application\chrome.exe",
            r"C:\Program Files\Chromium\Application\chrome.exe",
        ] {
            candidates.push(PathBuf::from(base));
        }
        // User-dependent paths via %LOCALAPPDATA%.
        if let Ok(localappdata) = std::env::var("LOCALAPPDATA") {
            let base = PathBuf::from(&localappdata);
            candidates.push(base.join(r"Google\Chrome\Application\chrome.exe"));
            candidates.push(base.join(r"Chromium\Application\chrome.exe"));
        }
    }

    candidates
}

/// Detects the Chrome/Chromium executable with a 3-layer hierarchy.
///
/// Resolution order:
/// 1. `manual_path` (typically `--chrome-path`). If provided but invalid,
///    returns an error — does NOT fall back silently.
/// 2. `CHROME_PATH` environment variable (if set and points to an existing file).
/// 3. Auto-detection via [`chrome_candidate_paths`] — first found wins.
///
/// Returns `Err` if no candidate is found.
///
/// # Errors
///
/// Returns an error if `manual_path` is provided but does not point to an
/// existing file, or if no Chrome/Chromium executable is found on the system.
pub fn detect_chrome(manual_path: Option<&Path>) -> Result<PathBuf, CliError> {
    if let Some(p) = manual_path {
        if p.is_file() {
            tracing::debug!(path = %p.display(), "Chrome found via --chrome-path");
            return Ok(p.to_path_buf());
        }
        return Err(CliError::PathError {
            message: format!(
                "--chrome-path {:?} does not exist or is not a file",
                p.display()
            ),
        });
    }

    if let Ok(env_path) = std::env::var("CHROME_PATH") {
        let p = PathBuf::from(&env_path);
        if p.is_file() {
            tracing::debug!(path = %p.display(), "Chrome found via CHROME_PATH");
            return Ok(p);
        }
        tracing::warn!(
            path = env_path,
            "CHROME_PATH set but file does not exist — trying auto-detection"
        );
    }

    for candidate in chrome_candidate_paths() {
        if candidate.is_file() {
            tracing::debug!(path = %candidate.display(), "Chrome detected automatically");
            return Ok(candidate);
        }
    }

    return Err(CliError::PathError { message: "Chrome/Chromium not found. Install via your package manager or provide --chrome-path or CHROME_PATH.".into() });
}

/// Indicates whether we are running inside a container or Flatpak/Snap wrapper, which
/// requires `--no-sandbox` for Chrome to work.
pub fn needs_no_sandbox(chrome_path: &Path) -> bool {
    #[cfg(target_os = "linux")]
    {
        // Wrapper Flatpak ou Snap.
        let s = chrome_path.to_string_lossy();
        if s.contains("flatpak/exports/bin") || s.starts_with("/snap/") {
            return true;
        }
        // Rodando como root (comum em Docker).
        // SAFETY: libc::geteuid is thread-safe and has no side effects.
        #[cfg(unix)]
        {
            // Simplification: detect via Docker environment variable.
            if std::env::var("DOCKER_CONTAINER").is_ok()
                || std::path::Path::new("/.dockerenv").exists()
            {
                return true;
            }
        }
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = chrome_path;
    }
    false
}

/// Monta a lista de flags stealth cross-platform para o Chrome headless.
pub fn flags_stealth(precisa_sandbox_off: bool, proxy: Option<&str>) -> Vec<String> {
    let mut flags: Vec<String> = vec![
        "--disable-blink-features=AutomationControlled".to_string(),
        "--window-size=1920,1080".to_string(),
        "--disable-background-networking".to_string(),
        "--disable-default-apps".to_string(),
        "--disable-extensions".to_string(),
        "--disable-sync".to_string(),
        "--metrics-recording-only".to_string(),
        "--no-first-run".to_string(),
    ];

    #[cfg(target_os = "linux")]
    {
        flags.push("--disable-dev-shm-usage".to_string());
        if precisa_sandbox_off {
            flags.push("--no-sandbox".to_string());
        }
    }
    #[cfg(target_os = "windows")]
    {
        let _ = precisa_sandbox_off;
        flags.push("--disable-gpu".to_string());
    }
    #[cfg(target_os = "macos")]
    {
        let _ = precisa_sandbox_off;
    }

    if let Some(url_proxy) = proxy {
        flags.push(format!("--proxy-server={url_proxy}"));
    }

    flags
}

/// RAII wrapper over `chromiumoxide::Browser`. Keeps the browser and handler-task alive.
///
/// **Cleanup:** prefer calling [`ChromeBrowser::shutdown`] explicitly (async).
/// [`Drop`] only aborts the handler task — the Chrome process may take a few ms
/// to terminate. For long-running applications, ALWAYS use `shutdown`.
pub struct ChromeBrowser {
    browser: Browser,
    handler: Option<JoinHandle<()>>,
    /// Keeps TempDir alive to ensure user-data-dir is removed on drop.
    _user_data: tempfile::TempDir,
}

impl ChromeBrowser {
    /// Launches headless Chrome with the stealth configuration.
    ///
    /// - `path`: Chrome executable (use [`detect_chrome`] to obtain it).
    /// - `proxy`: optional proxy URL (propagated to the browser process).
    /// - `timeout_launch`: time limit for process initialization.
    ///
    /// # Errors
    ///
    /// Returns an error if the temporary user-data directory cannot be created,
    /// if `BrowserConfig` construction fails, or if the Chrome process fails to
    /// launch within `timeout_launch`.
    ///
    /// # Cancel safety
    ///
    /// This function is cancel-safe. If the future is dropped before Chrome
    /// finishes launching, the spawned handler task is aborted and the temporary
    /// directory is cleaned up via [`Drop`].
    pub async fn launch(
        path: &Path,
        proxy: Option<&str>,
        timeout_launch: Duration,
    ) -> Result<Self, CliError> {
        tracing::info!(
            path = %path.display(),
            proxy = proxy.unwrap_or(""),
            "Launching headless Chrome"
        );

        let sandbox_off = needs_no_sandbox(path);
        let flags = flags_stealth(sandbox_off, proxy);
        let user_data = tempfile::tempdir().map_err(|e| CliError::PathError {
            message: format!("failed to create user-data-dir TempDir: {e}"),
        })?;

        let mut builder = BrowserConfig::builder()
            .chrome_executable(path)
            .user_data_dir(user_data.path())
            .headless_mode(HeadlessMode::New)
            .launch_timeout(timeout_launch)
            .args(flags);

        if sandbox_off {
            builder = builder.no_sandbox();
        }

        let config = builder.build().map_err(|e| CliError::InvalidConfig {
            message: format!("invalid BrowserConfig: {e}"),
        })?;

        let (browser, mut handler) =
            Browser::launch(config)
                .await
                .map_err(|e| CliError::HttpError {
                    message: format!("failed to launch Chrome process: {e}"),
                    cause: None,
                })?;

        // Handler task: pumps events until handler returns None (closed).
        let handler_task = tokio::spawn(async move {
            while let Some(event) = handler.next().await {
                if let Err(err) = event {
                    tracing::debug!(?err, "CDP handler event with error — continuing");
                }
            }
        });

        Ok(Self {
            browser,
            handler: Some(handler_task),
            _user_data: user_data,
        })
    }

    /// Accesses the internal `Browser` to create pages.
    pub fn browser_mut(&mut self) -> &mut Browser {
        &mut self.browser
    }

    /// Shuts down the browser and awaits handler cleanup. Prefer this over Drop.
    ///
    /// # Errors
    ///
    /// Returns an error only if the underlying `close()` or `wait()` calls
    /// propagate a fatal CDP protocol error; transient errors are logged and
    /// swallowed so cleanup always completes.
    ///
    /// # Cancel safety
    ///
    /// This function is cancel-safe. If dropped before completion, the handler
    /// task is aborted and the Chrome process is terminated via `kill_on_drop`.
    pub async fn shutdown(mut self) -> Result<(), CliError> {
        tracing::debug!("shutting down Chrome via close() + wait()");
        if let Err(err) = self.browser.close().await {
            tracing::debug!(?err, "error closing browser — continuing");
        }
        if let Err(err) = self.browser.wait().await {
            tracing::debug!(?err, "error awaiting browser wait()");
        }
        if let Some(h) = self.handler.take() {
            h.abort();
            let _ = h.await;
        }
        Ok(())
    }
}

impl Drop for ChromeBrowser {
    fn drop(&mut self) {
        if let Some(h) = self.handler.take() {
            h.abort();
        }
        tracing::debug!(
            "ChromeBrowser dropped — chromiumoxide Browser::drop handles remaining cleanup"
        );
    }
}

/// Extracts the main text from a URL using headless Chrome.
///
/// Strategy:
/// 1. Opens a new page (`new_page`).
/// 2. Awaits navigation completion.
/// 3. Executes JS `document.body.innerText` and collects as `String`.
/// 4. Cleans whitespace + short lines + truncates at `max_size`.
/// 5. Closes the page immediately.
///
/// The `timeout` applies to the entire operation via `tokio::time::timeout`.
///
/// # Errors
///
/// Returns an error if the page cannot be opened, JS evaluation fails,
/// or the operation exceeds `timeout`.
///
/// # Cancel safety
///
/// This function is cancel-safe. The outer `tokio::time::timeout` wraps
/// the entire navigation, so dropping the future aborts the CDP session
/// and releases the browser tab.
pub async fn extract_text_with_chrome(
    browser: &mut ChromeBrowser,
    url: &str,
    max_size: usize,
    timeout: Duration,
) -> Result<String, CliError> {
    let work = async {
        let page = browser
            .browser_mut()
            .new_page(url)
            .await
            .map_err(|e| CliError::HttpError {
                message: format!("failed to open page {url:?}: {e}"),
                cause: None,
            })?;

        // Wait for full navigation to complete (respects redirects).
        let _ = page.wait_for_navigation().await;

        let js_result = page
            .evaluate("document.body ? document.body.innerText : ''")
            .await
            .map_err(|e| CliError::HttpError {
                message: format!("failed to execute innerText on {url:?}: {e}"),
                cause: None,
            })?;

        let raw_text: String = js_result.into_value().unwrap_or_else(|_| String::new());

        // Close the page immediately to release the target.
        let _ = page.close().await;

        Ok::<String, CliError>(clean_text(&raw_text, max_size))
    };

    tokio::time::timeout(timeout, work)
        .await
        .map_err(|_| CliError::HttpError {
            message: format!("Chrome timeout exceeded for {url:?}"),
            cause: None,
        })?
}

/// Cleans raw text: normalizes whitespace, discards short lines, truncates at `max_size`.
fn clean_text(raw: &str, max_size: usize) -> String {
    let lines: Vec<String> = raw
        .lines()
        .map(|line| line.split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|line| line.chars().count() >= MIN_LINE_LENGTH)
        .collect();
    let joined = lines.join("\n");
    truncate_at_word(&joined, max_size)
}

/// Truncates respecting word boundary. Mirrors the implementation in `content.rs`.
fn truncate_at_word(text: &str, max_size: usize) -> String {
    if max_size == 0 {
        return String::new();
    }
    let total: usize = text.chars().count();
    if total <= max_size {
        return text.to_string();
    }
    let prefix: String = text.chars().take(max_size).collect();
    if let Some(pos) = prefix.rfind(char::is_whitespace) {
        return prefix[..pos].trim_end().to_string();
    }
    prefix
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chrome_candidate_paths_not_empty() {
        let paths = chrome_candidate_paths();
        assert!(!paths.is_empty(), "deve retornar ao menos um candidato");
    }

    #[test]
    fn detect_chrome_manual_path_nonexistent_fails() {
        let p = Path::new("/tmp/caminho/absolutamente/inexistente/chrome-xyz");
        assert!(
            detect_chrome(Some(p)).is_err(),
            "caminho manual inválido deve falhar"
        );
    }

    #[test]
    fn stealth_flags_include_anti_detection() {
        let f = flags_stealth(false, None);
        assert!(f.iter().any(|x| x.contains("AutomationControlled")));
        assert!(f.iter().any(|x| x == "--window-size=1920,1080"));
    }

    #[test]
    fn stealth_flags_include_proxy_when_provided() {
        let f = flags_stealth(false, Some("http://proxy:8080"));
        assert!(f.iter().any(|x| x == "--proxy-server=http://proxy:8080"));
    }

    #[test]
    fn stealth_flags_no_sandbox_only_when_required_on_linux() {
        let f_com = flags_stealth(true, None);
        let f_sem = flags_stealth(false, None);
        #[cfg(target_os = "linux")]
        {
            assert!(f_com.iter().any(|x| x == "--no-sandbox"));
            assert!(!f_sem.iter().any(|x| x == "--no-sandbox"));
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = (f_com, f_sem);
        }
    }

    #[test]
    fn clean_text_removes_short_lines() {
        let raw = "ok\noutra linha com tamanho bastante suficiente de vinte chars\ncurta\n";
        let clean = clean_text(raw, 1000);
        assert!(clean.contains("outra linha"));
        assert!(!clean.contains("ok\n"));
    }

    #[test]
    fn clean_text_truncates_at_word() {
        let raw =
            "linha um com mais de vinte caracteres definitivamente aqui presentes\n".repeat(10);
        let clean = clean_text(&raw, 50);
        assert!(clean.chars().count() <= 50);
    }

    #[test]
    fn precisa_no_sandbox_flatpak_path() {
        let p = Path::new("/var/lib/flatpak/exports/bin/com.google.Chrome");
        #[cfg(target_os = "linux")]
        assert!(needs_no_sandbox(p));
        #[cfg(not(target_os = "linux"))]
        {
            let _ = p;
        }
    }

    #[test]
    fn precisa_no_sandbox_snap_path() {
        let p = Path::new("/snap/bin/chromium");
        #[cfg(target_os = "linux")]
        assert!(needs_no_sandbox(p));
        #[cfg(not(target_os = "linux"))]
        {
            let _ = p;
        }
    }

    #[test]
    fn needs_no_sandbox_default_returns_false() {
        let p = Path::new("/usr/bin/chromium");
        #[cfg(target_os = "linux")]
        {
            // False UNLESS DOCKER_CONTAINER or /.dockerenv is present.
            let expected = std::env::var("DOCKER_CONTAINER").is_ok()
                || std::path::Path::new("/.dockerenv").exists();
            assert_eq!(needs_no_sandbox(p), expected);
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = p;
        }
    }
}
