// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload: declarative (platform detection and XDG path resolution)
//! Platform detection and cross-platform initialization.
//!
//! Responsibilities:
//! 1. On Windows, call `SetConsoleOutputCP(65001)` to ensure UTF-8 in the console
//!    for cmd.exe and legacy `PowerShell` (noop in Windows Terminal and in pipes/files).
//! 2. TTY detection for format auto-detect (used by the `output` module).
//! 3. Configuration directory resolution via `dirs::config_dir()`.
//!
//! The `init()` function MUST be called exactly once at the start of `main`.

use std::path::PathBuf;

/// Initializes platform-specific settings.
///
/// On Windows: configures UTF-8 codepage (65001) for console output.
/// On all platforms: performs no I/O operation that could fail.
///
/// This function is best-effort — if codepage configuration fails on Windows,
/// it only emits a warning via `tracing` and continues.
pub fn init() {
    #[cfg(windows)]
    iniciar_windows();
}

#[cfg(windows)]
fn iniciar_windows() {
    use windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE;
    use windows_sys::Win32::System::Console::{
        GetConsoleMode, GetStdHandle, SetConsoleCP, SetConsoleMode, SetConsoleOutputCP,
        ENABLE_VIRTUAL_TERMINAL_PROCESSING, STD_OUTPUT_HANDLE,
    };

    // SAFETY: SetConsoleOutputCP/SetConsoleCP accept a UINT codepage id
    // and return BOOL. No pointer dereference. 65001 = UTF-8.
    let resultado_output = unsafe { SetConsoleOutputCP(65001) };
    if resultado_output == 0 {
        tracing::warn!("Failed to configure UTF-8 output codepage (65001) on Windows console.");
    }

    // MP-01: SetConsoleCP for stdin UTF-8 (queries with accents via pipe).
    // SAFETY: SetConsoleCP is a Windows API that takes a u32 codepage identifier;
    // 65001 is the standard UTF-8 codepage constant. No preconditions beyond
    // being called on a Windows process with a console attached.
    let resultado_input = unsafe { SetConsoleCP(65001) };
    if resultado_input == 0 {
        tracing::warn!("Failed to configure UTF-8 input codepage (65001) on Windows console.");
    }

    if resultado_output != 0 || resultado_input != 0 {
        tracing::debug!("UTF-8 codepage (65001) configured on Windows console.");
    }

    // MP-02: Enable ANSI escape sequences (Virtual Terminal Processing).
    // SAFETY: GetStdHandle returns a HANDLE which is a transparent `*mut c_void`
    // in `windows-sys 0.59+`. On failure it returns `INVALID_HANDLE_VALUE`
    // (defined as `(HANDLE)-1`, i.e. `isize::MAX as *mut c_void`). We test
    // the handle against both `is_null()` and the documented sentinel before
    // passing it to `GetConsoleMode`/`SetConsoleMode`, which take a HANDLE
    // by value (no pointer arithmetic on the Rust side).
    let handle = unsafe { GetStdHandle(STD_OUTPUT_HANDLE) };
    if !handle.is_null() && handle != INVALID_HANDLE_VALUE {
        let mut mode: u32 = 0;
        // SAFETY: GetConsoleMode writes a u32 mode value into the provided
        // mutable reference. The handle was validated above (not null,
        // not INVALID_HANDLE_VALUE) and `mode` is a stack-allocated u32
        // with proper alignment. Win32 guarantees no other access.
        if unsafe { GetConsoleMode(handle, &mut mode) } != 0 {
            let novo = mode | ENABLE_VIRTUAL_TERMINAL_PROCESSING;
            // SAFETY: SetConsoleMode takes a handle and a u32 mode. The handle
            // was validated above and the mode is the existing mode OR'd with
            // ENABLE_VIRTUAL_TERMINAL_PROCESSING, which only enables an
            // opt-in feature. No memory is dereferenced.
            if unsafe { SetConsoleMode(handle, novo) } == 0 {
                tracing::debug!("ANSI VTP not available on this Windows console.");
            }
        }
    }
}

/// Checks whether `stdout` is connected to an interactive terminal (TTY).
/// Used by the `output` module for format auto-detect (text in TTY, json in pipe).
pub fn stdout_is_tty() -> bool {
    use std::io::IsTerminal;
    std::io::stdout().is_terminal()
}

/// Returns the application configuration directory following XDG / Apple / Windows conventions.
///
/// Resulting paths:
/// - Linux: `$XDG_CONFIG_HOME/duckduckgo-search-cli/` or `~/.config/duckduckgo-search-cli/`.
/// - macOS: `~/Library/Application Support/duckduckgo-search-cli/`.
/// - Windows: `%APPDATA%\duckduckgo-search-cli\`.
///
/// The environment variable `DUCKDUCKGO_SEARCH_CLI_HOME` overrides the default
/// path when set (rejected if it contains `..` for path traversal safety).
///
/// Returns `None` if no configuration directory can be determined.
pub fn config_directory() -> Option<PathBuf> {
    if let Some(home) = std::env::var_os("DUCKDUCKGO_SEARCH_CLI_HOME") {
        let p = PathBuf::from(home);
        if p.to_string_lossy().contains("..") {
            tracing::warn!("DUCKDUCKGO_SEARCH_CLI_HOME contains '..', ignoring");
        } else {
            return Some(p);
        }
    }
    dirs::config_dir().map(|base| base.join("duckduckgo-search-cli"))
}

/// Returns `true` if color output should be suppressed.
///
/// Checks (in order): `--no-color` flag, `NO_COLOR` env var (any value per
/// no-color.org), `CLICOLOR_FORCE=0`.
pub fn should_disable_color(flag_no_color: bool) -> bool {
    flag_no_color
        || std::env::var_os("NO_COLOR").is_some()
        || std::env::var("CLICOLOR_FORCE").ok().as_deref() == Some("0")
}

/// Path to the external `selectors.toml` file (if the config directory exists).
///
/// Used by the lazy loader of `SelectorConfig` — when the file exists,
/// it overrides the hardcoded defaults.
pub fn selectors_toml_path() -> Option<PathBuf> {
    config_directory().map(|base| base.join("selectors.toml"))
}

/// Path to the external `user-agents.toml` file (if the config directory exists).
pub fn user_agents_toml_path() -> Option<PathBuf> {
    config_directory().map(|base| base.join("user-agents.toml"))
}

/// Identifier name of the current platform (for logs and User-Agent matching).
pub fn platform_name() -> &'static str {
    if cfg!(target_os = "linux") {
        "linux"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else if cfg!(target_os = "windows") {
        "windows"
    } else {
        "outro"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn platform_name_returns_known_value() {
        let nome = platform_name();
        assert!(matches!(nome, "linux" | "macos" | "windows" | "outro"));
    }

    #[test]
    fn init_should_not_panic() {
        // Smoke test — on non-Windows platforms, this is a no-op.
        // On Windows, the call is best-effort and must not panic.
        init();
    }

    #[test]
    fn config_directory_not_empty_on_systems_with_home() {
        // Em sistemas CI sem HOME, pode retornar None. Apenas verificamos tipagem.
        let _ = config_directory();
    }

    #[test]
    fn toml_paths_end_with_expected_names() {
        if let Some(selectors) = selectors_toml_path() {
            assert!(selectors.ends_with("selectors.toml"));
        }
        if let Some(uas) = user_agents_toml_path() {
            assert!(uas.ends_with("user-agents.toml"));
        }
    }
}
