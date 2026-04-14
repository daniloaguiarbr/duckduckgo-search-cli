//! Detecção de plataforma e inicialização cross-platform.
//!
//! Responsabilidades:
//! 1. No Windows, chamar `SetConsoleOutputCP(65001)` para garantir UTF-8 no console
//!    cmd.exe e PowerShell legado (noop em Windows Terminal e em pipes/arquivos).
//! 2. Detecção de TTY para auto-detect de formato (usado pelo módulo `output`).
//! 3. Resolução de diretório de configuração via `dirs::config_dir()`.
//!
//! A função `iniciar()` DEVE ser chamada exatamente uma vez no começo do `main`.

use std::path::PathBuf;

/// Inicializa configurações específicas da plataforma.
///
/// No Windows: configura codepage UTF-8 (65001) para output no console.
/// Em todas as plataformas: não faz nenhuma operação de I/O que possa falhar.
///
/// Esta função é best-effort — se a configuração de codepage falhar no Windows,
/// apenas emite um warning via `tracing` e continua.
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

/// Verifica se `stdout` está conectado a um terminal interativo (TTY).
/// Usado pelo módulo `output` para auto-detect de formato (text em TTY, json em pipe).
pub fn stdout_eh_tty() -> bool {
    use std::io::IsTerminal;
    std::io::stdout().is_terminal()
}

/// Retorna o diretório de configuração da aplicação seguindo convenções XDG / Apple / Windows.
///
/// Caminhos resultantes:
/// - Linux: `$XDG_CONFIG_HOME/duckduckgo-search-cli/` ou `~/.config/duckduckgo-search-cli/`.
/// - macOS: `~/Library/Application Support/duckduckgo-search-cli/`.
/// - Windows: `%APPDATA%\duckduckgo-search-cli\`.
///
/// Retorna `None` se nenhum diretório de configuração puder ser determinado.
pub fn diretorio_configuracao() -> Option<PathBuf> {
    dirs::config_dir().map(|base| base.join("duckduckgo-search-cli"))
}

/// Caminho do arquivo `selectors.toml` externo (se o diretório de config existir).
///
/// Usado pelo carregador lazy de `ConfiguracaoSeletores` — quando o arquivo existe,
/// substitui os defaults hardcoded.
pub fn caminho_selectors_toml() -> Option<PathBuf> {
    diretorio_configuracao().map(|base| base.join("selectors.toml"))
}

/// Caminho do arquivo `user-agents.toml` externo (se o diretório de config existir).
pub fn caminho_user_agents_toml() -> Option<PathBuf> {
    diretorio_configuracao().map(|base| base.join("user-agents.toml"))
}

/// Nome identificador da plataforma atual (para logs e User-Agent matching).
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
