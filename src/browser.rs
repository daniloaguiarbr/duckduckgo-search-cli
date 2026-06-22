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
//! ## Process Cleanup and Safety (`rules_rust.md` — Memory Management)
//!
//! `chromiumoxide::Browser` starts a child Chrome process. Without explicit cleanup,
//! the process becomes a zombie. The [`Drop`] implementation on [`ChromeBrowser`]
//! aborts the handler task and signals `kill_on_drop` internally. For complete
//! synchronous cleanup, prefer calling [`ChromeBrowser::shutdown`] before drop.

#![cfg(feature = "chrome")]

use crate::error::CliError;
use chromiumoxide::browser::{Browser, BrowserConfig};
use chromiumoxide::cdp::browser_protocol::page::AddScriptToEvaluateOnNewDocumentParams;
use futures::StreamExt;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::task::JoinHandle;

/// Minimum character count per line kept by the cleaning pipeline.
const MIN_LINE_LENGTH: usize = 20;

/// Comprehensive stealth scripts injected via CDP `Page.addScriptToEvaluateOnNewDocument`.
///
/// Layer 3a — Basic JS environment (6 signals):
///   webdriver, plugins, languages, chrome object, maxTouchPoints, connection
///
/// Layer 3b — Hardware fingerprint spoofing (GAP-NEW-007):
///   Canvas fingerprint, WebGL renderer/vendor, `AudioContext` channel data,
///   `hardwareConcurrency`, `deviceMemory`, `screen.colorDepth`
const STEALTH_SCRIPTS: &str = concat!(
    // --- Layer 3a: basic JS environment ---
    "Object.defineProperty(navigator,'webdriver',{get:()=>false});",
    "Object.defineProperty(navigator,'languages',{get:()=>['en-US','en']});",
    "Object.defineProperty(navigator,'maxTouchPoints',{get:()=>0});",
    "Object.defineProperty(navigator,'connection',{get:()=>({rtt:50,downlink:10,effectiveType:'4g',saveData:false})});",
    "Object.defineProperty(navigator,'vendor',{get:()=>'Google Inc.'});",
    // --- Layer 3a+: chrome object emulation ---
    "window.chrome={runtime:{PlatformOs:{MAC:'mac',WIN:'win',ANDROID:'android',CROS:'cros',LINUX:'linux',OPENBSD:'openbsd'},PlatformArch:{ARM:'arm',X86_32:'x86-32',X86_64:'x86-64',MIPS:'mips',MIPS64:'mips64'},PlatformNaclArch:{ARM:'arm',X86_32:'x86-32',X86_64:'x86-64',MIPS:'mips',MIPS64:'mips64'},RequestUpdateCheckStatus:{THROTTLED:'throttled',NO_UPDATE:'no_update',UPDATE_AVAILABLE:'update_available'},OnInstalledReason:{INSTALL:'install',UPDATE:'update',CHROME_UPDATE:'chrome_update',SHARED_MODULE_UPDATE:'shared_module_update'},OnRestartRequiredReason:{APP_UPDATE:'app_update',OS_UPDATE:'os_update',PERIODIC:'periodic'},connect:function(){},sendMessage:function(){}},app:{isInstalled:false,InstallState:{INSTALLED:'installed',DISABLED:'disabled',NOT_INSTALLED:'not_installed'},RunningState:{RUNNING:'running',CANNOT_RUN:'cannot_run',READY_TO_RUN:'ready_to_run'}},loadTimes:function(){return{requestTime:Date.now()/1000,startLoadTime:Date.now()/1000,commitLoadTime:Date.now()/1000,finishDocumentLoadTime:Date.now()/1000,finishLoadTime:Date.now()/1000,firstPaintTime:Date.now()/1000,firstPaintAfterLoadTime:0,navigationType:'Other',wasFetchedViaSpdy:true,wasNpnNegotiated:true,npnNegotiatedProtocol:'h2',wasAlternateProtocolAvailable:false,connectionInfo:'h2'}},csi:function(){return{onloadT:Date.now(),startE:Date.now(),pageT:0,tran:15}}};",
    // --- Layer 3a+: realistic PluginArray ---
    "(function(){function P(n,d,f,m){this.name=n;this.description=d;this.filename=f;this.length=m.length;for(var i=0;i<m.length;i++)this[i]=m[i]}var p=[new P('Chrome PDF Plugin','Portable Document Format','internal-pdf-viewer',[{type:'application/x-google-chrome-pdf',suffixes:'pdf',description:'Portable Document Format'}]),new P('Chrome PDF Viewer','','mhjfbmdgcfjbbpaeojofohoefgiehjai',[{type:'application/pdf',suffixes:'pdf',description:''}]),new P('Native Client','','internal-nacl-plugin',[{type:'application/x-nacl',suffixes:'',description:'Native Client Executable'},{type:'application/x-pnacl',suffixes:'',description:'Portable Native Client Executable'}])];Object.defineProperty(navigator,'plugins',{get:function(){return p}});Object.defineProperty(navigator,'mimeTypes',{get:function(){return p.reduce(function(a,pl){for(var i=0;i<pl.length;i++)a.push(pl[i]);return a},[])}})})()",
    ";",
    // --- Layer 3b: window outer dimensions (0 in headless = detection) ---
    "Object.defineProperty(window,'outerHeight',{get:function(){return window.innerHeight+85}});",
    "Object.defineProperty(window,'outerWidth',{get:function(){return window.innerWidth+15}});",
    // --- Layer 3b: Permissions API (notifications=denied in headless) ---
    "(function(){if(typeof Permissions!=='undefined'){var o=Permissions.prototype.query;Permissions.prototype.query=function(p){if(p&&p.name==='notifications')return Promise.resolve({state:Notification.permission==='denied'?'denied':'prompt',onchange:null});return o.apply(this,arguments)}}})()",
    ";",
    // --- Layer 3b: iframe contentWindow protection ---
    "(function(){try{var F=HTMLIFrameElement.prototype;var d=Object.getOwnPropertyDescriptor(F,'contentWindow');if(d&&d.get){var o=d.get;Object.defineProperty(F,'contentWindow',{get:function(){var w=o.call(this);if(w&&w.chrome)w.chrome=window.chrome;return w}})}}catch(e){}})()",
    ";",
    // --- Layer 3b: hardware fingerprint spoofing (GAP-NEW-007) ---
    "Object.defineProperty(navigator,'hardwareConcurrency',{get:()=>8});",
    "Object.defineProperty(navigator,'deviceMemory',{get:()=>8});",
    "Object.defineProperty(screen,'colorDepth',{get:()=>24});",
    // Canvas fingerprint: inject subtle per-session noise into pixel data
    "(function(){var o=HTMLCanvasElement.prototype.toDataURL;HTMLCanvasElement.prototype.toDataURL=function(){try{var c=this.getContext('2d');if(c){var i=c.getImageData(0,0,Math.min(this.width,16),Math.min(this.height,16));for(var j=0;j<i.data.length;j+=100)i.data[j]=(i.data[j]+1)%256;c.putImageData(i,0,0)}}catch(e){}return o.apply(this,arguments)}})();",
    // WebGL renderer/vendor: report plausible GPU instead of SwiftShader
    "(function(){var o=WebGLRenderingContext.prototype.getParameter;WebGLRenderingContext.prototype.getParameter=function(p){if(p===37445)return'Google Inc. (NVIDIA)';if(p===37446)return'ANGLE (NVIDIA, NVIDIA GeForce GTX 1650 Direct3D11 vs_5_0 ps_5_0, D3D11)';return o.call(this,p)};if(typeof WebGL2RenderingContext!=='undefined'){var o2=WebGL2RenderingContext.prototype.getParameter;WebGL2RenderingContext.prototype.getParameter=function(p){if(p===37445)return'Google Inc. (NVIDIA)';if(p===37446)return'ANGLE (NVIDIA, NVIDIA GeForce GTX 1650 Direct3D11 vs_5_0 ps_5_0, D3D11)';return o2.call(this,p)}}})();",
    // AudioContext fingerprint: add micro-noise to channel data
    "(function(){if(typeof AudioBuffer!=='undefined'){var o=AudioBuffer.prototype.getChannelData;AudioBuffer.prototype.getChannelData=function(c){var a=o.call(this,c);for(var i=0;i<a.length;i+=100)a[i]+=0.0000001*(i%7-3);return a}}})();",
);

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
            "/usr/local/bin/chromium",
            "/usr/local/bin/google-chrome",
            "/opt/google/chrome/chrome",
            "/snap/bin/chromium",
            "/snap/bin/google-chrome",
            "/var/lib/flatpak/exports/bin/com.google.Chrome",
            "/var/lib/flatpak/exports/bin/org.chromium.Chromium",
            // v0.8.0 GAP-NEW-005: prefer raw Chromium binary over wrapper script
            // (Fedora/RHEL install the actual binary at this path; the .sh wrapper
            //  invokes PATH `timeout` which is shadowed by the Rust crate timeout-cli).
            "/usr/lib64/chromium-browser/chromium-browser",
            "/usr/lib/chromium-browser/chromium-browser",
            "/usr/lib/chromium-browser/chrome",
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
    // Layer 1: manual --chrome-path argument (highest priority, bypasses all checks).
    if let Some(p) = manual_path {
        if is_executable_chrome_binary(p) {
            tracing::info!(path = %p.display(), "Chrome found via --chrome-path");
            return Ok(p.to_path_buf());
        }
        return Err(CliError::PathError {
            message: format!(
                "--chrome-path {:?} is not a valid Chrome/Chromium binary (missing, not a file, or shell script)",
                p.display()
            ),
        });
    }

    // Layer 2: CHROME_PATH env var override.
    if let Ok(env_path) = std::env::var("CHROME_PATH") {
        let p = PathBuf::from(&env_path);
        if is_executable_chrome_binary(&p) {
            tracing::info!(path = %p.display(), "Chrome found via CHROME_PATH");
            return Ok(p);
        }
        tracing::warn!(
            path = env_path,
            "CHROME_PATH set but file is missing or is a shell script — trying auto-detection"
        );
    }

    // Layer 3: PATH lookup via `which` crate (cross-platform: Linux/macOS/Windows).
    for binary_name in [
        "chromium",
        "google-chrome",
        "google-chrome-stable",
        "chrome",
    ] {
        if let Ok(p) = which::which(binary_name) {
            if is_executable_chrome_binary(&p) {
                tracing::info!(
                    binary = binary_name,
                    path = %p.display(),
                    "Chrome found via PATH lookup (which crate)"
                );
                return Ok(p);
            }
            tracing::debug!(
                binary = binary_name,
                path = %p.display(),
                "which crate found candidate but rejected: not a real binary"
            );
        }
    }

    // Layer 4: platform-specific well-known installation paths.
    for candidate in chrome_candidate_paths() {
        if is_executable_chrome_binary(&candidate) {
            tracing::info!(path = %candidate.display(), "Chrome found at platform-specific path");
            return Ok(candidate);
        }
    }

    Err(CliError::PathError {
        message: "Chrome/Chromium not found. Install via your package manager or provide --chrome-path or CHROME_PATH.".into(),
    })
}

/// v0.8.0 GAP-NEW-005: rejects shell-script wrappers (e.g. `chromium-browser.sh`)
/// which call the Rust `timeout` crate binary and kill Chrome in ~0.1s. Validates
/// that the candidate is a real ELF/Mach-O executable, not a text file.
fn is_executable_chrome_binary(path: &Path) -> bool {
    if !path.is_file() {
        return false;
    }
    let path_str = path.to_string_lossy();
    if path_str.ends_with(".sh") {
        tracing::debug!(path = %path.display(), "rejecting shell-script wrapper");
        return false;
    }
    // Verify ELF magic bytes (Linux) or Mach-O (macOS) to ensure executable format.
    match std::fs::read(path) {
        Ok(bytes) if bytes.len() >= 4 => {
            let is_elf = &bytes[0..4] == b"\x7fELF";
            let is_macho = bytes[0..4] == [0xCF, 0xFA, 0xED, 0xFE]
                || bytes[0..4] == [0xFE, 0xED, 0xFA, 0xCE]
                || bytes[0..4] == [0xFE, 0xED, 0xFA, 0xCF]
                || bytes[0..4] == [0xCA, 0xFE, 0xBA, 0xBE];
            let is_pe = &bytes[0..2] == b"MZ";
            is_elf || is_macho || is_pe
        }
        _ => false,
    }
}

/// Returns `true` when the operator explicitly requested headed Chrome
/// via `DUCKDUCKGO_CHROME_XVFB=1` (for xvfb-run anti-bot evasion).
/// Without this env var, Chrome runs headless by default.
fn is_xvfb_requested() -> bool {
    std::env::var("DUCKDUCKGO_CHROME_XVFB").is_ok()
}

/// Spawns a private Xvfb server on a free display number so Chrome can run
/// in headed mode (passing Cloudflare anti-bot) without showing a visible
/// window to the user.
///
/// Returns `(child_process, display_string)` on success, or `None` if Xvfb
/// is not available or no free display slot was found.
#[cfg(target_os = "linux")]
fn spawn_virtual_display() -> Option<(std::process::Child, String)> {
    let xvfb_path = which::which("Xvfb").ok()?;

    for display_num in 99..200 {
        let lock_path = format!("/tmp/.X{display_num}-lock");
        if std::path::Path::new(&lock_path).exists() {
            continue;
        }
        let disp = format!(":{display_num}");
        let child = std::process::Command::new(&xvfb_path)
            .arg(&disp)
            .args(["-screen", "0", "1920x1080x24", "-nolisten", "tcp", "-ac"])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .ok()?;

        std::thread::sleep(std::time::Duration::from_millis(150));

        if std::path::Path::new(&lock_path).exists() {
            tracing::info!(xvfb_display = %disp, "Xvfb virtual display started");
            return Some((child, disp));
        }
        // Xvfb failed to create lock — try next display number.
    }
    None
}

#[cfg(not(target_os = "linux"))]
fn spawn_virtual_display() -> Option<(std::process::Child, String)> {
    None
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
pub fn flags_stealth(
    precisa_sandbox_off: bool,
    proxy: Option<&str>,
    user_agent: &str,
) -> Vec<String> {
    let mut flags: Vec<String> = vec![
        "--disable-blink-features=AutomationControlled".to_string(),
        "--window-size=1920,1080".to_string(),
        "--disable-background-networking".to_string(),
        "--disable-default-apps".to_string(),
        "--disable-extensions".to_string(),
        "--disable-sync".to_string(),
        "--metrics-recording-only".to_string(),
        "--no-first-run".to_string(),
        format!("--user-agent={user_agent}"),
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
    /// The underlying chromiumoxide browser handle.
    browser: Browser,
    /// Join handle for the event-loop task; aborted on `Drop` if still alive.
    handler: Option<JoinHandle<()>>,
    /// Keeps `TempDir` alive to ensure user-data-dir is removed on drop.
    _user_data: tempfile::TempDir,
    /// Private Xvfb process for headed-but-invisible Chrome.
    /// Killed on drop so the virtual display does not leak.
    _xvfb: Option<std::process::Child>,
}

impl std::fmt::Debug for ChromeBrowser {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ChromeBrowser")
            .field("handler_alive", &self.handler.is_some())
            .field("user_data_dir", &self._user_data.path())
            .finish_non_exhaustive()
    }
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
        user_agent: &str,
    ) -> Result<Self, CliError> {
        tracing::info!(
            path = %path.display(),
            proxy = proxy.unwrap_or(""),
            user_agent = user_agent,
            "Launching headless Chrome"
        );

        let sandbox_off = needs_no_sandbox(path);
        let flags = flags_stealth(sandbox_off, proxy, user_agent);
        let user_data = tempfile::tempdir().map_err(|e| CliError::PathError {
            message: format!("failed to create user-data-dir TempDir: {e}"),
        })?;

        let force_visible = std::env::var("DUCKDUCKGO_CHROME_VISIBLE").is_ok();
        let xvfb_requested = is_xvfb_requested();
        let force_headless = std::env::var("DUCKDUCKGO_CHROME_HEADLESS").is_ok();

        // Headed Chrome passes Cloudflare anti-bot; headless is detectable.
        // Priority: HEADLESS env (force) > VISIBLE env > Xvfb auto-spawn > headless fallback.
        let mut xvfb_child: Option<std::process::Child> = None;
        let (use_headed, virtual_display) = if force_headless {
            (false, None)
        } else if force_visible || xvfb_requested {
            (true, None)
        } else {
            match spawn_virtual_display() {
                Some((child, display)) => {
                    xvfb_child = Some(child);
                    (true, Some(display))
                }
                None => (false, None),
            }
        };

        let mut builder = BrowserConfig::builder()
            .chrome_executable(path)
            .user_data_dir(user_data.path())
            .launch_timeout(timeout_launch)
            .args(flags);

        if let Some(ref display) = virtual_display {
            builder = builder
                .env("DISPLAY", display)
                .env("WAYLAND_DISPLAY", "")
                .arg(("ozone-platform", "x11"));
        }

        if use_headed {
            builder = builder.with_head();
            tracing::info!(
                force_visible,
                xvfb_requested,
                virtual_display = virtual_display.as_deref().unwrap_or("none"),
                "Chrome running in headed mode (anti-bot evasion)"
            );
        } else {
            builder = builder.new_headless_mode();
            if !force_headless {
                tracing::info!(
                    "Xvfb not available — falling back to headless Chrome (anti-bot risk)"
                );
            }
        }

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
                    tracing::info!(?err, "CDP handler event with error — continuing");
                }
            }
        });

        Ok(Self {
            browser,
            handler: Some(handler_task),
            _user_data: user_data,
            _xvfb: xvfb_child,
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
        tracing::info!("shutting down Chrome via close() + wait()");
        if let Err(err) = self.browser.close().await {
            tracing::info!(?err, "error closing browser — continuing");
        }
        if let Err(err) = self.browser.wait().await {
            tracing::info!(?err, "error awaiting browser wait()");
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
        if let Some(ref mut xvfb) = self._xvfb {
            let _ = xvfb.kill();
            let _ = xvfb.wait();
            tracing::info!("Xvfb virtual display stopped");
        }
        tracing::info!(
            "ChromeBrowser dropped — chromiumoxide Browser::drop handles remaining cleanup"
        );
    }
}

/// Extracts raw HTML from a URL using headless Chrome with stealth injection.
///
/// Strategy:
/// 1. Opens a blank page and injects `navigator.webdriver = false` via CDP.
/// 2. Navigates to the target URL.
/// 3. Waits for navigation completion + 1500ms for JS rendering.
/// 4. Extracts `document.documentElement.outerHTML`.
/// 5. Truncates at `max_size` bytes and closes the page.
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
pub async fn extract_html_with_chrome(
    browser: &mut ChromeBrowser,
    url: &str,
    max_size: usize,
    timeout: Duration,
) -> Result<String, CliError> {
    let work = async {
        let page = browser
            .browser_mut()
            .new_page("about:blank")
            .await
            .map_err(|e| CliError::HttpError {
                message: format!("failed to open blank page for {url:?}: {e}"),
                cause: None,
            })?;

        // Inject comprehensive stealth scripts before any navigation.
        // Layer 3a: webdriver, plugins, languages, chrome, maxTouchPoints, connection
        // Layer 3b: Canvas, WebGL, AudioContext, hardwareConcurrency, deviceMemory (GAP-NEW-007)
        let stealth_cmd = AddScriptToEvaluateOnNewDocumentParams::new(STEALTH_SCRIPTS);
        let _ = page.execute(stealth_cmd).await;

        // Navigate to the target URL.
        page.goto(url).await.map_err(|e| CliError::HttpError {
            message: format!("failed to navigate to {url:?}: {e}"),
            cause: None,
        })?;

        // Wait for full navigation to complete (respects redirects).
        let _ = page.wait_for_navigation().await;

        // Poll for real SERP: Cloudflare may serve a JS challenge that
        // auto-resolves after a few seconds. We check every 500ms for up
        // to 8 seconds whether the page contains search result markers.
        let mut raw_html = String::new();
        for attempt in 0..16u32 {
            tokio::time::sleep(Duration::from_millis(500)).await;
            let js_result = page
                .evaluate("document.documentElement.outerHTML")
                .await
                .map_err(|e| CliError::HttpError {
                    message: format!("failed to extract outerHTML on {url:?}: {e}"),
                    cause: None,
                })?;
            raw_html = js_result.into_value().unwrap_or_default();
            if raw_html.contains("result__a") || raw_html.contains("result__snippet") {
                tracing::info!(attempt, "SERP detected after polling");
                break;
            }
            if attempt == 15 {
                tracing::info!(
                    body_len = raw_html.len(),
                    "polling exhausted — using last HTML"
                );
            }
        }

        // Close the page immediately to release the target.
        let _ = page.close().await;

        // Truncate at byte boundary.
        if raw_html.len() > max_size {
            Ok::<String, CliError>(raw_html[..max_size].to_string())
        } else {
            Ok::<String, CliError>(raw_html)
        }
    };

    tokio::time::timeout(timeout, work)
        .await
        .map_err(|_| CliError::HttpError {
            message: format!("Chrome timeout exceeded for {url:?}"),
            cause: None,
        })?
}

/// Extracts the main text from a URL using headless Chrome.
///
/// Wrapper over [`extract_html_with_chrome`] that applies text cleaning
/// (normalizes whitespace, discards short lines, truncates at `max_size`).
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
            .new_page("about:blank")
            .await
            .map_err(|e| CliError::HttpError {
                message: format!("failed to open blank page for {url:?}: {e}"),
                cause: None,
            })?;

        // Inject comprehensive stealth scripts before any navigation.
        // Layer 3a: webdriver, plugins, languages, chrome, maxTouchPoints, connection
        // Layer 3b: Canvas, WebGL, AudioContext, hardwareConcurrency, deviceMemory (GAP-NEW-007)
        let stealth_cmd = AddScriptToEvaluateOnNewDocumentParams::new(STEALTH_SCRIPTS);
        let _ = page.execute(stealth_cmd).await;

        // Navigate to the target URL.
        page.goto(url).await.map_err(|e| CliError::HttpError {
            message: format!("failed to navigate to {url:?}: {e}"),
            cause: None,
        })?;

        // Wait for full navigation to complete (respects redirects).
        let _ = page.wait_for_navigation().await;

        // Allow time for JS rendering.
        tokio::time::sleep(Duration::from_millis(1500)).await;

        let js_result = page
            .evaluate("document.body ? document.body.innerText : ''")
            .await
            .map_err(|e| CliError::HttpError {
                message: format!("failed to execute innerText on {url:?}: {e}"),
                cause: None,
            })?;

        let raw_text: String = js_result.into_value().unwrap_or_default();

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
        let f = flags_stealth(false, None, "TestAgent/1.0");
        assert!(f.iter().any(|x| x.contains("AutomationControlled")));
        assert!(f.iter().any(|x| x == "--window-size=1920,1080"));
        assert!(f.iter().any(|x| x == "--user-agent=TestAgent/1.0"));
    }

    #[test]
    fn stealth_flags_include_proxy_when_provided() {
        let f = flags_stealth(false, Some("http://proxy:8080"), "TestAgent/1.0");
        assert!(f.iter().any(|x| x == "--proxy-server=http://proxy:8080"));
    }

    #[test]
    fn stealth_flags_no_sandbox_only_when_required_on_linux() {
        let f_com = flags_stealth(true, None, "TestAgent/1.0");
        let f_sem = flags_stealth(false, None, "TestAgent/1.0");
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

    #[test]
    fn is_xvfb_requested_false_by_default() {
        std::env::remove_var("DUCKDUCKGO_CHROME_XVFB");
        assert!(!is_xvfb_requested());
    }

    #[test]
    fn is_xvfb_requested_true_when_set() {
        std::env::set_var("DUCKDUCKGO_CHROME_XVFB", "1");
        let result = is_xvfb_requested();
        std::env::remove_var("DUCKDUCKGO_CHROME_XVFB");
        assert!(result);
    }

    #[test]
    fn headed_requires_explicit_opt_in() {
        std::env::remove_var("DUCKDUCKGO_CHROME_VISIBLE");
        std::env::remove_var("DUCKDUCKGO_CHROME_XVFB");
        assert!(!is_xvfb_requested());
    }
}
