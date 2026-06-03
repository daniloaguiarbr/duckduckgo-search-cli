// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload: I/O-light (TOML config loading, one-shot at startup)
//! Lazy loading of `SelectorConfig` with precedence:
//! 1. `selectors.toml` file in `$XDG_CONFIG_HOME/duckduckgo-search-cli/` (if it exists and parses).
//! 2. Embedded defaults via `SelectorConfig::default()`.
//!
//! This iteration 6 fully externalizes the selectors — allows hotfixing layout
//! breakages without recompiling the CLI. The embedded TOML (`config/selectors.toml`)
//! is identical to the defaults; it is copied to the filesystem by the `init-config` subcommand.
//!
//! Parsing failures do NOT abort execution — they silently fall back to defaults
//! with a `tracing::warn!` log for diagnostics.

use crate::error::CliError;
use crate::platform;
use crate::types::SelectorConfig;
use std::path::Path;
use std::sync::Arc;

/// Embedded TOML with the default CSS selectors. Used by `init-config` to create the
/// file on the filesystem without requiring a network connection.
pub const DEFAULT_SELECTORS_TOML: &str = include_str!("../config/selectors.toml");

/// Parses an arbitrary TOML file into `SelectorConfig`.
///
/// # Errors
///
/// Returns [`CliError::InvalidConfig`] if the file cannot be read or if TOML
/// deserialization fails.
pub fn load_from_toml(path: &Path) -> Result<SelectorConfig, CliError> {
    let meta = std::fs::metadata(path).map_err(|e| CliError::InvalidConfig {
        message: format!("failed to stat selector file {}: {e}", path.display()),
    })?;
    if meta.len() > 1_048_576 {
        return Err(CliError::InvalidConfig {
            message: format!(
                "selector file {} exceeds 1 MB limit ({} bytes)",
                path.display(),
                meta.len()
            ),
        });
    }
    let content = std::fs::read_to_string(path).map_err(|e| CliError::InvalidConfig {
        message: format!("failed to read selector file {}: {e}", path.display()),
    })?;
    let cfg: SelectorConfig = toml::from_str(&content).map_err(|e| CliError::InvalidConfig {
        message: format!("failed to parse TOML {}: {e}", path.display()),
    })?;
    Ok(cfg)
}

/// Loads selectors applying precedence rules: external TOML → embedded defaults.
///
/// Tries in order:
/// 1. Path returned by [`platform::selectors_toml_path`].
/// 2. Fallback to embedded [`SelectorConfig::default`].
///
/// Always returns a valid `Arc<SelectorConfig>` — never panics.
pub fn load_selectors() -> Arc<SelectorConfig> {
    if let Some(path) = platform::selectors_toml_path() {
        if path.exists() {
            match load_from_toml(&path) {
                Ok(cfg) => {
                    tracing::info!(path = %path.display(), "Selectors loaded from external TOML file");
                    return Arc::new(cfg);
                }
                Err(erro) => {
                    tracing::warn!(
                        path = %path.display(),
                        ?erro,
                        "failed to load external selectors.toml — falling back to built-in defaults"
                    );
                }
            }
        } else {
            tracing::debug!(path = %path.display(), "selectors.toml file does not exist — using built-in defaults");
        }
    }
    tracing::info!("Using built-in default selectors");
    Arc::new(SelectorConfig::default())
}

/// Loads selectors from a specific directory (used with `--config`).
///
/// Falls back to built-in defaults if the file does not exist or fails to parse.
pub fn load_selectors_from_dir(dir: &std::path::Path) -> Arc<SelectorConfig> {
    let path = dir.join("selectors.toml");
    if path.exists() {
        match load_from_toml(&path) {
            Ok(cfg) => {
                tracing::info!(path = %path.display(), "Selectors loaded from --config directory");
                return Arc::new(cfg);
            }
            Err(erro) => {
                tracing::warn!(path = %path.display(), ?erro, "failed to load selectors.toml from --config dir");
            }
        }
    }
    Arc::new(SelectorConfig::default())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn novo_tempdir(nome: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "ddgcli-selectors-{}-{}-{}",
            nome,
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create tempdir");
        dir
    }

    #[test]
    fn load_from_toml_valid_parses_all_groups() {
        let dir = novo_tempdir("valido");
        let path = dir.join("selectors.toml");
        let mut file = std::fs::File::create(&path).expect("create file");
        file.write_all(DEFAULT_SELECTORS_TOML.as_bytes())
            .expect("write");
        drop(file);

        let cfg = load_from_toml(&path).expect("should parse default TOML");
        assert_eq!(cfg.html_endpoint.results_container, "#links");
        assert_eq!(cfg.lite_endpoint.results_table, "table, body table");
        assert_eq!(cfg.related_searches.links, "a");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_from_toml_invalid_returns_error() {
        let dir = novo_tempdir("invalido");
        let path = dir.join("broken.toml");
        std::fs::write(&path, "[html_endpoint\nresult_item = ").expect("write");
        let result = load_from_toml(&path);
        assert!(result.is_err(), "TOML sintaticamente inválido deve falhar");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_from_toml_absent_returns_error() {
        let inexistente = std::env::temp_dir().join("ddgcli-nao-existe-xyz987654321.toml");
        let _ = std::fs::remove_file(&inexistente);
        assert!(load_from_toml(&inexistente).is_err());
    }

    #[test]
    fn load_from_toml_partial_uses_defaults_for_missing_fields() {
        let dir = novo_tempdir("parcial");
        let path = dir.join("selectors.toml");
        // Somente o grupo `html_endpoint` com campo customizado — resto vira default.
        let content = r#"
            [html_endpoint]
            result_item = ".custom-result"

            [html_endpoint.ads_filter]
            ad_classes = [".custom-ad"]
            ad_attributes = ["data-custom=1"]
            ad_url_patterns = ["tracking.example/track"]
        "#;
        std::fs::write(&path, content).expect("write");
        let cfg = load_from_toml(&path).expect("partial should parse");
        assert_eq!(cfg.html_endpoint.result_item, ".custom-result");
        // Demais campos devem vir do default.
        assert_eq!(cfg.html_endpoint.results_container, "#links");
        assert_eq!(cfg.lite_endpoint.results_table, "table, body table");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_selectors_returns_defaults_when_no_file() {
        // The real user path may vary; at minimum, the function always returns something.
        let arc = load_selectors();
        assert!(!arc.html_endpoint.results_container.is_empty());
    }

    #[test]
    fn embedded_selectors_toml_is_valid() {
        let cfg: SelectorConfig =
            toml::from_str(DEFAULT_SELECTORS_TOML).expect("embedded TOML should parse");
        assert_eq!(cfg.html_endpoint.results_container, "#links");
    }
}
