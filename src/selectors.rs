//! Carregamento lazy de `ConfiguracaoSeletores` com precedência:
//! 1. Arquivo `selectors.toml` em `$XDG_CONFIG_HOME/duckduckgo-search-cli/` (se existir e parsear).
//! 2. Defaults embutidos via `ConfiguracaoSeletores::default()`.
//!
//! Esta iteração 6 externaliza completamente os seletores — permite hotfix de
//! breakages de layout sem recompilar a CLI. O TOML embutido (`config/selectors.toml`)
//! é idêntico aos defaults; é copiado para o filesystem pelo subcomando `init-config`.
//!
//! Falhas de parsing NÃO abortam a execução — caem silenciosamente para defaults
//! com log `tracing::warn!` para diagnóstico.

use crate::platform;
use crate::types::ConfiguracaoSeletores;
use anyhow::{Context, Result};
use std::path::Path;
use std::sync::Arc;

/// TOML embutido com os seletores padrão. Usado por `init-config` para criar o
/// arquivo no filesystem sem depender de conexão de rede.
pub const SELECTORS_TOML_PADRAO: &str = include_str!("../config/selectors.toml");

/// Parseia um arquivo TOML arbitrário em `ConfiguracaoSeletores`.
///
/// Retorna `Err(anyhow)` se o arquivo não existe, não for legível ou não parsear.
/// Usado tanto internamente quanto em testes para validar arquivos customizados.
pub fn carregar_do_toml(caminho: &Path) -> Result<ConfiguracaoSeletores> {
    let conteudo = std::fs::read_to_string(caminho)
        .with_context(|| format!("falha ao ler arquivo de seletores {}", caminho.display()))?;
    let cfg: ConfiguracaoSeletores = toml::from_str(&conteudo)
        .with_context(|| format!("falha ao parsear TOML {}", caminho.display()))?;
    Ok(cfg)
}

/// Carrega os seletores aplicando a precedência TOML externo → defaults embutidos.
///
/// Tenta em ordem:
/// 1. Caminho retornado por [`platform::caminho_selectors_toml`].
/// 2. Fallback para [`ConfiguracaoSeletores::default`] embutido.
///
/// Sempre retorna um `Arc<ConfiguracaoSeletores>` válido — nunca panica.
pub fn carregar_seletores() -> Arc<ConfiguracaoSeletores> {
    if let Some(caminho) = platform::caminho_selectors_toml() {
        if caminho.exists() {
            match carregar_do_toml(&caminho) {
                Ok(cfg) => {
                    tracing::info!(caminho = %caminho.display(), "Seletores carregados de arquivo TOML externo");
                    return Arc::new(cfg);
                }
                Err(erro) => {
                    tracing::warn!(
                        caminho = %caminho.display(),
                        ?erro,
                        "falha ao carregar seletores.toml externo — caindo para defaults embutidos"
                    );
                }
            }
        } else {
            tracing::debug!(caminho = %caminho.display(), "arquivo selectors.toml não existe — usando defaults embutidos");
        }
    }
    tracing::info!("Usando seletores padrão embutidos");
    Arc::new(ConfiguracaoSeletores::default())
}

#[cfg(test)]
mod testes {
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
        std::fs::create_dir_all(&dir).expect("criar tempdir");
        dir
    }

    #[test]
    fn carregar_do_toml_valido_parseia_todos_os_grupos() {
        let dir = novo_tempdir("valido");
        let caminho = dir.join("selectors.toml");
        let mut arquivo = std::fs::File::create(&caminho).expect("criar arquivo");
        arquivo
            .write_all(SELECTORS_TOML_PADRAO.as_bytes())
            .expect("escrever");
        drop(arquivo);

        let cfg = carregar_do_toml(&caminho).expect("deve parsear TOML padrão");
        assert_eq!(cfg.html_endpoint.results_container, "#links");
        assert_eq!(cfg.lite_endpoint.results_table, "table, body table");
        assert_eq!(cfg.related_searches.links, "a");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn carregar_do_toml_invalido_retorna_erro() {
        let dir = novo_tempdir("invalido");
        let caminho = dir.join("broken.toml");
        std::fs::write(&caminho, "[html_endpoint\nresult_item = ").expect("escrever");
        let resultado = carregar_do_toml(&caminho);
        assert!(
            resultado.is_err(),
            "TOML sintaticamente inválido deve falhar"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn carregar_do_toml_ausente_retorna_erro() {
        let inexistente = std::env::temp_dir().join("ddgcli-nao-existe-xyz987654321.toml");
        let _ = std::fs::remove_file(&inexistente);
        assert!(carregar_do_toml(&inexistente).is_err());
    }

    #[test]
    fn carregar_do_toml_parcial_usa_defaults_para_campos_ausentes() {
        let dir = novo_tempdir("parcial");
        let caminho = dir.join("selectors.toml");
        // Somente o grupo `html_endpoint` com campo customizado — resto vira default.
        let conteudo = r#"
            [html_endpoint]
            result_item = ".custom-result"

            [html_endpoint.ads_filter]
            ad_classes = [".custom-ad"]
            ad_attributes = ["data-custom=1"]
            ad_url_patterns = ["tracking.example/track"]
        "#;
        std::fs::write(&caminho, conteudo).expect("escrever");
        let cfg = carregar_do_toml(&caminho).expect("parcial deve parsear");
        assert_eq!(cfg.html_endpoint.result_item, ".custom-result");
        // Demais campos devem vir do default.
        assert_eq!(cfg.html_endpoint.results_container, "#links");
        assert_eq!(cfg.lite_endpoint.results_table, "table, body table");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn carregar_seletores_retorna_defaults_quando_nao_ha_arquivo() {
        // O caminho real do usuário pode variar; no mínimo, a função sempre retorna algo.
        let arc = carregar_seletores();
        assert!(!arc.html_endpoint.results_container.is_empty());
    }

    #[test]
    fn selectors_toml_embutido_eh_valido() {
        let cfg: ConfiguracaoSeletores =
            toml::from_str(SELECTORS_TOML_PADRAO).expect("TOML embutido deve parsear");
        assert_eq!(cfg.html_endpoint.results_container, "#links");
    }
}
