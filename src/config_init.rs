//! Implements the `init-config` subcommand — copies TOMLs embedded in the binary
//! to the user's configuration directory, allowing local editing without
//! recompiling.
//!
//! - `selectors.toml` — CSS selectors for SERP extraction.
//! - `user-agents.toml` — cross-platform User-Agent pool.
//!
//! Existing files are preserved unless `--force` is passed.
//! In `--dry-run` mode, only reports planned actions without touching disk.

use crate::platform;
use crate::selectors::SELECTORS_TOML_PADRAO;
use anyhow::{Context, Result};
use serde::Serialize;
use std::path::{Path, PathBuf};

/// Embedded TOML with the default User-Agents. `config/user-agents.toml` is the
/// source of truth during the build (`include_str!`).
pub const USER_AGENTS_TOML_PADRAO: &str = include_str!("../config/user-agents.toml");

/// Action applied to a file during initialization.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case", tag = "action")]
pub enum AcaoArquivoConfig {
    /// File did not exist — will be/was created.
    Criado,
    /// File already existed and `--force` was not passed — no change.
    Ignorado,
    /// File was overwritten (only with `--force`).
    Sobrescrito,
    /// `--dry-run` active: file would be created.
    CriariaSeExecutasse,
    /// `--dry-run` active: file would be overwritten.
    SobrescreveriaSeExecutasse,
    /// Write failure — contains a human-readable message.
    Erro { mensagem: String },
}

/// Individual report for a processed file.
#[derive(Debug, Clone, Serialize)]
pub struct RelatorioArquivo {
    /// Absolute path of the file.
    pub caminho: PathBuf,
    /// Applied/planned action.
    #[serde(flatten)]
    pub acao: AcaoArquivoConfig,
}

/// Complete initialization report.
#[derive(Debug, Clone, Serialize)]
pub struct RelatorioInitConfig {
    /// `true` if `--dry-run` mode was active (no write I/O).
    pub dry_run: bool,
    /// `true` if `--force` mode was active (overwrites existing files).
    pub force: bool,
    /// Base directory used (XDG / Apple / APPDATA).
    pub diretorio_base: Option<PathBuf>,
    /// Per-file actions — stable order.
    pub arquivos: Vec<RelatorioArquivo>,
}

/// Executes initialization with the provided flags.
///
/// Always returns `Ok` — individual failures are stored in `AcaoArquivoConfig::Erro`.
/// Returns `Err` only if no configuration directory can be determined.
pub fn inicializar_config(forcar: bool, dry_run: bool) -> Result<RelatorioInitConfig> {
    let diretorio_base = platform::diretorio_configuracao().context(
        "não foi possível determinar o diretório de configuração (HOME/APPDATA ausentes?)",
    )?;

    // Criação de diretório: apenas se não for dry-run.
    if !dry_run && !diretorio_base.exists() {
        std::fs::create_dir_all(&diretorio_base).with_context(|| {
            format!(
                "falha ao criar diretório de configuração {}",
                diretorio_base.display()
            )
        })?;
    }

    let arquivos = vec![
        (diretorio_base.join("selectors.toml"), SELECTORS_TOML_PADRAO),
        (
            diretorio_base.join("user-agents.toml"),
            USER_AGENTS_TOML_PADRAO,
        ),
    ];

    let mut relatorio_arquivos = Vec::with_capacity(arquivos.len());

    for (caminho, conteudo) in arquivos {
        let acao = processar_arquivo(&caminho, conteudo, forcar, dry_run);
        relatorio_arquivos.push(RelatorioArquivo { caminho, acao });
    }

    Ok(RelatorioInitConfig {
        dry_run,
        force: forcar,
        diretorio_base: Some(diretorio_base),
        arquivos: relatorio_arquivos,
    })
}

/// Processes ONE file — decides the correct action based on existence and flags.
fn processar_arquivo(
    caminho: &Path,
    conteudo: &str,
    forcar: bool,
    dry_run: bool,
) -> AcaoArquivoConfig {
    let existe = caminho.exists();

    match (existe, forcar, dry_run) {
        (true, false, _) => AcaoArquivoConfig::Ignorado,
        (false, _, true) => AcaoArquivoConfig::CriariaSeExecutasse,
        (true, true, true) => AcaoArquivoConfig::SobrescreveriaSeExecutasse,
        (false, _, false) => match gravar_arquivo(caminho, conteudo) {
            Ok(_) => AcaoArquivoConfig::Criado,
            Err(erro) => AcaoArquivoConfig::Erro {
                mensagem: format!("{erro:#}"),
            },
        },
        (true, true, false) => match gravar_arquivo(caminho, conteudo) {
            Ok(_) => AcaoArquivoConfig::Sobrescrito,
            Err(erro) => AcaoArquivoConfig::Erro {
                mensagem: format!("{erro:#}"),
            },
        },
    }
}

fn gravar_arquivo(caminho: &Path, conteudo: &str) -> Result<()> {
    if let Some(pai) = caminho.parent() {
        if !pai.as_os_str().is_empty() && !pai.exists() {
            std::fs::create_dir_all(pai)
                .with_context(|| format!("falha ao criar diretório {}", pai.display()))?;
        }
    }
    std::fs::write(caminho, conteudo)
        .with_context(|| format!("falha ao gravar {}", caminho.display()))?;

    #[cfg(unix)]
    aplicar_permissoes_600(caminho)?;

    Ok(())
}

#[cfg(unix)]
fn aplicar_permissoes_600(caminho: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let permissoes = std::fs::Permissions::from_mode(0o600);
    std::fs::set_permissions(caminho, permissoes)
        .with_context(|| format!("falha ao aplicar permissões 0o600 em {}", caminho.display()))?;
    Ok(())
}

#[cfg(test)]
mod testes {
    use super::*;

    fn preparar_diretorio(nome: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "ddgcli-init-{}-{}-{}",
            nome,
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("criar dir");
        dir
    }

    #[test]
    fn user_agents_toml_padrao_nao_vazio() {
        assert!(!USER_AGENTS_TOML_PADRAO.is_empty());
        assert!(USER_AGENTS_TOML_PADRAO.contains("Mozilla"));
    }

    #[test]
    fn selectors_toml_padrao_nao_vazio() {
        assert!(!SELECTORS_TOML_PADRAO.is_empty());
        assert!(SELECTORS_TOML_PADRAO.contains("[html_endpoint]"));
    }

    #[test]
    fn processar_arquivo_criando_quando_nao_existe() {
        let dir = preparar_diretorio("novo");
        let caminho = dir.join("arq.toml");
        let acao = processar_arquivo(&caminho, "x = 1", false, false);
        assert_eq!(acao, AcaoArquivoConfig::Criado);
        assert!(caminho.exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn processar_arquivo_ignora_quando_existe_sem_force() {
        let dir = preparar_diretorio("ignora");
        let caminho = dir.join("arq.toml");
        std::fs::write(&caminho, "original").expect("preparar arquivo");
        let acao = processar_arquivo(&caminho, "novo conteudo", false, false);
        assert_eq!(acao, AcaoArquivoConfig::Ignorado);
        let conteudo = std::fs::read_to_string(&caminho).expect("ler");
        assert_eq!(conteudo, "original", "arquivo não deve ser sobrescrito");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn processar_arquivo_sobrescreve_quando_force_e_existe() {
        let dir = preparar_diretorio("force");
        let caminho = dir.join("arq.toml");
        std::fs::write(&caminho, "original").expect("preparar arquivo");
        let acao = processar_arquivo(&caminho, "novo conteudo", true, false);
        assert_eq!(acao, AcaoArquivoConfig::Sobrescrito);
        let conteudo = std::fs::read_to_string(&caminho).expect("ler");
        assert_eq!(conteudo, "novo conteudo");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn processar_arquivo_dry_run_nao_grava() {
        let dir = preparar_diretorio("dryrun");
        let caminho = dir.join("arq.toml");
        let acao = processar_arquivo(&caminho, "x = 1", false, true);
        assert_eq!(acao, AcaoArquivoConfig::CriariaSeExecutasse);
        assert!(!caminho.exists(), "dry-run não deve criar arquivo");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn processar_arquivo_dry_run_force_sobre_existente() {
        let dir = preparar_diretorio("dryforce");
        let caminho = dir.join("arq.toml");
        std::fs::write(&caminho, "original").expect("preparar arquivo");
        let acao = processar_arquivo(&caminho, "novo conteudo", true, true);
        assert_eq!(acao, AcaoArquivoConfig::SobrescreveriaSeExecutasse);
        let conteudo = std::fs::read_to_string(&caminho).expect("ler");
        assert_eq!(conteudo, "original", "dry-run não deve sobrescrever");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn relatorio_init_serializa_como_json_estavel() {
        let rel = RelatorioInitConfig {
            dry_run: true,
            force: false,
            diretorio_base: Some(PathBuf::from("/tmp/x")),
            arquivos: vec![RelatorioArquivo {
                caminho: PathBuf::from("/tmp/x/selectors.toml"),
                acao: AcaoArquivoConfig::CriariaSeExecutasse,
            }],
        };
        let json = serde_json::to_string(&rel).expect("serializar");
        assert!(json.contains("\"dry_run\":true"));
        assert!(json.contains("\"action\":\"criaria_se_executasse\""));
    }
}
