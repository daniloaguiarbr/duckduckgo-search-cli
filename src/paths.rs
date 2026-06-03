// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload: declarative (path validation and sanitization)
//! Path validation and sanitization for I/O operations.
//!
//! This module centralizes validation of output paths provided by the
//! user via `--output`, preventing path traversal and writes to system
//! directories. Also encapsulates parent directory creation and Unix
//! permissions application.

use crate::error::CliError;
use std::path::{Component, Path, PathBuf};

const PROTECTED_UNIX: &[&str] = &[
    "/etc", "/usr", "/bin", "/sbin", "/boot", "/proc", "/sys", "/dev",
];
const PROTECTED_WINDOWS: &[&str] = &[
    "C:\\Windows",
    "C:\\Program Files",
    "C:\\Program Files (x86)",
];

const WINDOWS_RESERVED_NAMES: &[&str] = &[
    "CON", "PRN", "AUX", "NUL", "COM0", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7",
    "COM8", "COM9", "LPT0", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

/// Validates an output path provided by the user.
///
/// Rejects paths containing `..` components (path traversal), absolute
/// paths pointing to protected system directories, and filenames using
/// Windows reserved device names (CON, PRN, AUX, NUL, COM0-9, LPT0-9).
///
/// On Windows, paths longer than 260 characters may fail unless the
/// application manifest includes `longPathAware`. This is a known limitation.
///
/// # Errors
///
/// Returns [`CliError::PathError`] if the path contains `..` components,
/// points to a protected system directory, or uses a Windows reserved name.
pub fn validate_output_path(path: &Path) -> Result<PathBuf, CliError> {
    for component in path.components() {
        if matches!(component, Component::ParentDir) {
            return Err(CliError::PathError {
                message: format!(
                    "output path rejected — contains '..' (path traversal): {}",
                    path.display()
                ),
            });
        }
    }

    if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
        let upper = stem.to_ascii_uppercase();
        if WINDOWS_RESERVED_NAMES.contains(&upper.as_str()) {
            return Err(CliError::PathError {
                message: format!(
                    "output path rejected — uses Windows reserved name: {}",
                    path.display()
                ),
            });
        }
    }

    if path.is_absolute() {
        let path_str = path.to_string_lossy();

        for dir in PROTECTED_UNIX {
            if path_str.starts_with(dir) {
                return Err(CliError::PathError {
                    message: format!(
                        "output path rejected — points to system directory: {}",
                        path.display()
                    ),
                });
            }
        }
        for dir in PROTECTED_WINDOWS {
            if path_str.to_lowercase().starts_with(&dir.to_lowercase()) {
                return Err(CliError::PathError {
                    message: format!(
                        "output path rejected — points to system directory: {}",
                        path.display()
                    ),
                });
            }
        }
    }

    Ok(path.to_path_buf())
}

/// Creates parent directories of a path, if needed.
///
/// # Errors
///
/// Returns [`CliError::PathError`] if the underlying filesystem call to
/// create directories fails.
pub fn create_parent_dirs(path: &Path) -> Result<(), CliError> {
    if let Some(parent_dir) = path.parent() {
        if !parent_dir.as_os_str().is_empty() && !parent_dir.exists() {
            std::fs::create_dir_all(parent_dir).map_err(|e| CliError::PathError {
                message: format!(
                    "failed to create parent directories: {}: {e}",
                    parent_dir.display()
                ),
            })?;
        }
    }
    Ok(())
}

/// Applies 0o644 permissions to a file on Unix (owner reads+writes, others read).
/// No-op on non-Unix platforms.
///
/// # Errors
///
/// Returns [`CliError::PathError`] if setting file permissions fails
/// (e.g. permission denied or the file no longer exists).
#[cfg(unix)]
pub fn apply_permissions_644(path: &Path) -> Result<(), CliError> {
    use std::os::unix::fs::PermissionsExt;
    let permissions = std::fs::Permissions::from_mode(0o644);
    std::fs::set_permissions(path, permissions).map_err(|e| CliError::PathError {
        message: format!(
            "failed to apply 0o644 permissions on {}: {e}",
            path.display()
        ),
    })?;
    Ok(())
}

/// # Errors
///
/// Always returns `Ok(())` on non-Unix platforms.
#[cfg(not(unix))]
pub fn apply_permissions_644(_path: &Path) -> Result<(), CliError> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn rejects_path_with_parent_dir() {
        let result = validate_output_path(Path::new("../../etc/passwd"));
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("path traversal"), "mensagem: {msg}");
    }

    #[test]
    fn rejects_path_with_parent_dir_in_middle() {
        let result = validate_output_path(Path::new("output/../../../evil.json"));
        assert!(result.is_err());
    }

    #[test]
    fn aceita_path_relativo_simples() {
        let result = validate_output_path(Path::new("output/resultado.json"));
        assert!(result.is_ok());
    }

    #[test]
    fn accepts_relative_path_with_single_dot() {
        let result = validate_output_path(Path::new("./resultado.json"));
        assert!(result.is_ok());
    }

    #[test]
    fn aceita_path_absoluto_tmp() {
        let result = validate_output_path(Path::new("/tmp/ddg_resultado.json"));
        assert!(result.is_ok());
    }

    #[test]
    fn rejeita_path_absoluto_etc() {
        let result = validate_output_path(Path::new("/etc/shadow"));
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("system directory"), "message: {msg}");
    }

    #[test]
    fn rejeita_path_absoluto_usr() {
        let result = validate_output_path(Path::new("/usr/bin/evil"));
        assert!(result.is_err());
    }

    #[test]
    fn aceita_path_absoluto_home() {
        let result = validate_output_path(Path::new("/home/user/resultado.json"));
        assert!(result.is_ok());
    }

    #[test]
    fn create_parent_dirs_with_tempdir() {
        let tmp = tempfile::tempdir().expect("create tempdir");
        let path = tmp.path().join("sub").join("resultado.json");
        let result = create_parent_dirs(&path);
        assert!(result.is_ok());
        assert!(path.parent().expect("has parent").exists());
    }

    #[test]
    fn simple_filename_without_parent() {
        let result = validate_output_path(Path::new("resultado.json"));
        assert!(result.is_ok());
    }

    #[test]
    fn rejeita_nome_reservado_windows_nul() {
        let result = validate_output_path(Path::new("NUL.json"));
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("Windows reserved name"), "mensagem: {msg}");
    }

    #[test]
    fn rejeita_nome_reservado_windows_con_case_insensitive() {
        let result = validate_output_path(Path::new("con.txt"));
        assert!(result.is_err());
    }

    #[test]
    fn rejeita_nome_reservado_windows_com1() {
        let result = validate_output_path(Path::new("output/COM1.json"));
        assert!(result.is_err());
    }

    #[test]
    fn accepts_non_reserved_name_content() {
        let result = validate_output_path(Path::new("conteudo.json"));
        assert!(result.is_ok());
    }
}
