// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload: I/O-light (one-shot config file creation)
//! Implements the `init-config` subcommand — copies TOMLs embedded in the binary
//! to the user's configuration directory, allowing local editing without
//! recompiling.
//!
//! - `selectors.toml` — CSS selectors for SERP extraction.
//! - `user-agents.toml` — cross-platform User-Agent pool.
//!
//! Existing files are preserved unless `--force` is passed.
//! In `--dry-run` mode, only reports planned actions without touching disk.

use crate::error::CliError;
use crate::platform;
use crate::selectors::DEFAULT_SELECTORS_TOML;
use serde::Serialize;
use std::path::{Path, PathBuf};

/// Embedded TOML with the default User-Agents. `config/user-agents.toml` is the
/// source of truth during the build (`include_str!`).
pub const DEFAULT_USER_AGENTS_TOML: &str = include_str!("../config/user-agents.toml");

/// Action applied to a file during initialization.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case", tag = "action")]
pub enum ConfigFileAction {
    /// File did not exist — will be/was created.
    #[serde(rename = "criado")]
    Created,
    /// File already existed and `--force` was not passed — no change.
    #[serde(rename = "ignorado")]
    Skipped,
    /// File was overwritten (only with `--force`).
    #[serde(rename = "sobrescrito")]
    Overwritten,
    /// `--dry-run` active: file would be created.
    #[serde(rename = "criaria_se_executasse")]
    WouldCreate,
    /// `--dry-run` active: file would be overwritten.
    #[serde(rename = "sobrescreveria_se_executasse")]
    WouldOverwrite,
    /// Write failure — contains a human-readable message.
    #[serde(rename = "erro")]
    Error {
        /// Human-readable error description.
        #[serde(rename = "mensagem")]
        message: String,
    },
}

/// Individual report for a processed file.
#[derive(Debug, Clone, Serialize)]
pub struct FileReport {
    /// Absolute path of the file.
    #[serde(rename = "caminho")]
    pub path: PathBuf,
    /// Applied/planned action.
    #[serde(flatten)]
    pub action_taken: ConfigFileAction,
}

/// Complete initialization report.
#[derive(Debug, Clone, Serialize)]
pub struct InitConfigReport {
    /// `true` if `--dry-run` mode was active (no write I/O).
    pub dry_run: bool,
    /// `true` if `--force` mode was active (overwrites existing files).
    pub force: bool,
    /// Base directory used (XDG / Apple / APPDATA).
    #[serde(rename = "diretorio_base")]
    pub base_directory: Option<PathBuf>,
    /// Per-file actions — stable order.
    #[serde(rename = "arquivos")]
    pub files: Vec<FileReport>,
}

/// Executes initialization with the provided flags.
///
/// Always returns `Ok` — individual failures are stored in `ConfigFileAction::Error`.
/// Returns `Err` only if no configuration directory can be determined.
///
/// # Errors
///
/// Returns an error if the OS configuration directory cannot be determined, or if
/// creating the configuration directory on disk fails.
pub fn initialize_config(force: bool, dry_run: bool) -> Result<InitConfigReport, CliError> {
    let diretorio_base = platform::config_directory().ok_or_else(|| CliError::InvalidConfig {
        message: "could not determine configuration directory (HOME/APPDATA missing?)".into(),
    })?;

    if !dry_run && !diretorio_base.exists() {
        std::fs::create_dir_all(&diretorio_base).map_err(|e| CliError::PathError {
            message: format!(
                "failed to create config directory {}: {e}",
                diretorio_base.display()
            ),
        })?;
    }

    let arquivos = vec![
        (
            diretorio_base.join("selectors.toml"),
            DEFAULT_SELECTORS_TOML,
        ),
        (
            diretorio_base.join("user-agents.toml"),
            DEFAULT_USER_AGENTS_TOML,
        ),
    ];

    let mut file_reports = Vec::with_capacity(arquivos.len());

    for (path, content) in arquivos {
        let action = process_file(&path, content, force, dry_run);
        file_reports.push(FileReport {
            path,
            action_taken: action,
        });
    }

    Ok(InitConfigReport {
        dry_run,
        force,
        base_directory: Some(diretorio_base),
        files: file_reports,
    })
}

/// Processes ONE file — decides the correct action based on existence and flags.
fn process_file(path: &Path, content: &str, force: bool, dry_run: bool) -> ConfigFileAction {
    let exists = path.exists();

    match (exists, force, dry_run) {
        (true, false, _) => ConfigFileAction::Skipped,
        (false, _, true) => ConfigFileAction::WouldCreate,
        (true, true, true) => ConfigFileAction::WouldOverwrite,
        (false, _, false) => match write_file(path, content) {
            Ok(_) => ConfigFileAction::Created,
            Err(erro) => ConfigFileAction::Error {
                message: format!("{erro:#}"),
            },
        },
        (true, true, false) => match write_file(path, content) {
            Ok(_) => ConfigFileAction::Overwritten,
            Err(erro) => ConfigFileAction::Error {
                message: format!("{erro:#}"),
            },
        },
    }
}

fn write_file(path: &Path, content: &str) -> Result<(), CliError> {
    if let Some(parent_dir) = path.parent() {
        if !parent_dir.as_os_str().is_empty() && !parent_dir.exists() {
            std::fs::create_dir_all(parent_dir).map_err(|e| CliError::PathError {
                message: format!("failed to create directory {}: {e}", parent_dir.display()),
            })?;
        }
    }
    std::fs::write(path, content).map_err(|e| CliError::PathError {
        message: format!("failed to write {}: {e}", path.display()),
    })?;

    #[cfg(unix)]
    apply_permissions_600(path)?;

    Ok(())
}

#[cfg(unix)]
fn apply_permissions_600(path: &Path) -> Result<(), CliError> {
    use std::os::unix::fs::PermissionsExt;
    let permissions = std::fs::Permissions::from_mode(0o600);
    std::fs::set_permissions(path, permissions).map_err(|e| CliError::PathError {
        message: format!(
            "failed to apply 0o600 permissions on {}: {e}",
            path.display()
        ),
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn prepare_directory(nome: &str) -> PathBuf {
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
        std::fs::create_dir_all(&dir).expect("create dir");
        dir
    }

    #[test]
    fn user_agents_toml_default_not_empty() {
        assert!(!DEFAULT_USER_AGENTS_TOML.is_empty());
        assert!(DEFAULT_USER_AGENTS_TOML.contains("Mozilla"));
    }

    #[test]
    fn selectors_toml_default_not_empty() {
        assert!(!DEFAULT_SELECTORS_TOML.is_empty());
        assert!(DEFAULT_SELECTORS_TOML.contains("[html_endpoint]"));
    }

    #[test]
    fn process_file_creates_when_not_exists() {
        let dir = prepare_directory("novo");
        let caminho = dir.join("arq.toml");
        let acao = process_file(&caminho, "x = 1", false, false);
        assert_eq!(acao, ConfigFileAction::Created);
        assert!(caminho.exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn process_file_skips_when_exists_without_force() {
        let dir = prepare_directory("ignora");
        let caminho = dir.join("arq.toml");
        std::fs::write(&caminho, "original").expect("prepare file");
        let acao = process_file(&caminho, "novo conteudo", false, false);
        assert_eq!(acao, ConfigFileAction::Skipped);
        let conteudo = std::fs::read_to_string(&caminho).expect("read");
        assert_eq!(conteudo, "original", "arquivo não deve ser sobrescrito");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn process_file_overwrites_when_force_and_exists() {
        let dir = prepare_directory("force");
        let caminho = dir.join("arq.toml");
        std::fs::write(&caminho, "original").expect("prepare file");
        let acao = process_file(&caminho, "novo conteudo", true, false);
        assert_eq!(acao, ConfigFileAction::Overwritten);
        let conteudo = std::fs::read_to_string(&caminho).expect("read");
        assert_eq!(conteudo, "novo conteudo");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn process_file_dry_run_does_not_write() {
        let dir = prepare_directory("dryrun");
        let caminho = dir.join("arq.toml");
        let acao = process_file(&caminho, "x = 1", false, true);
        assert_eq!(acao, ConfigFileAction::WouldCreate);
        assert!(!caminho.exists(), "dry-run não deve criar arquivo");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn process_file_dry_run_force_over_existing() {
        let dir = prepare_directory("dryforce");
        let caminho = dir.join("arq.toml");
        std::fs::write(&caminho, "original").expect("prepare file");
        let acao = process_file(&caminho, "novo conteudo", true, true);
        assert_eq!(acao, ConfigFileAction::WouldOverwrite);
        let conteudo = std::fs::read_to_string(&caminho).expect("read");
        assert_eq!(conteudo, "original", "dry-run não deve sobrescrever");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn init_report_serializes_as_stable_json() {
        let rel = InitConfigReport {
            dry_run: true,
            force: false,
            base_directory: Some(PathBuf::from("/tmp/x")),
            files: vec![FileReport {
                path: PathBuf::from("/tmp/x/selectors.toml"),
                action_taken: ConfigFileAction::WouldCreate,
            }],
        };
        let json = serde_json::to_string(&rel).expect("serialize");
        assert!(json.contains("\"dry_run\":true"));
        assert!(json.contains("\"action\":\"criaria_se_executasse\""));
    }
}
