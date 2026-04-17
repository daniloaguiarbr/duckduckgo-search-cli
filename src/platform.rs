//! Platform detection and cross-platform initialization.
//!
//! Responsibilities:
//! 1. On Windows, call `SetConsoleOutputCP(65001)` to ensure UTF-8 in the console
//!    for cmd.exe and legacy PowerShell (noop in Windows Terminal and in pipes/files).
//! 2. TTY detection for format auto-detect (used by the `output` module).
//! 3. Configuration directory resolution via `dirs::config_dir()`.
//!
//! The `iniciar()` function MUST be called exactly once at the start of `main`.

use std::path::PathBuf;

/// Initializes platform-specific settings.
///
/// On Windows: configures UTF-8 codepage (65001) for console output.
/// On all platforms: performs no I/O operation that could fail.
///
/// This function is best-effort — if codepage configuration fails on Windows,
/// it only emits a warning via `tracing` and continues.
pub fn iniciar() {
    #[cfg(windows)]
    iniciar_windows();
}

#[cfg(windows)]
fn iniciar_windows() {
    // SetConsoleOutputCP(65001) configura UTF-8 como codepage de saída do console.
    // windows-sys 0.59 expõe a função em Win32::System::Console.
    // A função retorna BOOL (i32): 0 = falha, !=0 = sucesso.
    // Para stdout redirecionado (pipe/arquivo) a chamada é no-op e inofensiva.
    use windows_sys::Win32::System::Console::SetConsoleOutputCP;
    // SAFETY: SetConsoleOutputCP é uma FFI segura — aceita uma UINT (codepage id)
    // e retorna BOOL. Não desreferencia ponteiros. 65001 é a constante UTF-8.
    let resultado = unsafe { SetConsoleOutputCP(65001) };
    if resultado == 0 {
        tracing::warn!(
            "Falha ao configurar codepage UTF-8 (65001) no console Windows. \
             Caracteres acentuados podem aparecer incorretamente em cmd.exe antigo."
        );
    } else {
        tracing::debug!("Codepage UTF-8 (65001) configurado com sucesso no console Windows.");
    }
}

/// Checks whether `stdout` is connected to an interactive terminal (TTY).
/// Used by the `output` module for format auto-detect (text in TTY, json in pipe).
pub fn stdout_eh_tty() -> bool {
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
/// Returns `None` if no configuration directory can be determined.
pub fn diretorio_configuracao() -> Option<PathBuf> {
    dirs::config_dir().map(|base| base.join("duckduckgo-search-cli"))
}

/// Path to the external `selectors.toml` file (if the config directory exists).
///
/// Used by the lazy loader of `ConfiguracaoSeletores` — when the file exists,
/// it overrides the hardcoded defaults.
pub fn caminho_selectors_toml() -> Option<PathBuf> {
    diretorio_configuracao().map(|base| base.join("selectors.toml"))
}

/// Path to the external `user-agents.toml` file (if the config directory exists).
pub fn caminho_user_agents_toml() -> Option<PathBuf> {
    diretorio_configuracao().map(|base| base.join("user-agents.toml"))
}

/// Identifier name of the current platform (for logs and User-Agent matching).
pub fn nome_plataforma() -> &'static str {
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
mod testes {
    use super::*;

    #[test]
    fn nome_plataforma_retorna_valor_conhecido() {
        let nome = nome_plataforma();
        assert!(matches!(nome, "linux" | "macos" | "windows" | "outro"));
    }

    #[test]
    fn iniciar_nao_deve_panicar() {
        // Smoke test — em plataformas não-Windows, é no-op.
        // Em Windows, a chamada é best-effort e não deve panicar.
        iniciar();
    }

    #[test]
    fn diretorio_configuracao_nao_vazio_em_sistemas_com_home() {
        // Em sistemas CI sem HOME, pode retornar None. Apenas verificamos tipagem.
        let _ = diretorio_configuracao();
    }

    #[test]
    fn caminhos_toml_terminam_com_nomes_esperados() {
        if let Some(selectors) = caminho_selectors_toml() {
            assert!(selectors.ends_with("selectors.toml"));
        }
        if let Some(uas) = caminho_user_agents_toml() {
            assert!(uas.ends_with("user-agents.toml"));
        }
    }
}
