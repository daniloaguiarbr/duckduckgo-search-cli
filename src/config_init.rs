//! Implementa o subcomando `init-config` — copia TOMLs embutidos no binário
//! para o diretório de configuração do usuário, permitindo edição local sem
//! recompilar.
//!
//! - `selectors.toml` — seletores CSS para extração da SERP.
//! - `user-agents.toml` — pool de User-Agents cross-platform.
//!
//! Arquivos já existentes são preservados a menos que `--force` seja passado.
//! Em modo `--dry-run` apenas reporta as ações planejadas sem tocar no disco.

use crate::platform;
use crate::selectors::SELECTORS_TOML_PADRAO;
use anyhow::{Context, Result};
use serde::Serialize;
use std::path::{Path, PathBuf};

/// TOML embutido com os User-Agents padrão. O `config/user-agents.toml` é a
/// fonte de verdade durante o build (`include_str!`).
pub const USER_AGENTS_TOML_PADRAO: &str = include_str!("../config/user-agents.toml");

/// Ação aplicada a um arquivo durante a inicialização.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case", tag = "action")]
pub enum AcaoArquivoConfig {
    /// Arquivo não existia — será/foi criado.
    Criado,
    /// Arquivo já existia e `--force` não foi passado — sem alteração.
    Ignorado,
    /// Arquivo foi sobrescrito (apenas com `--force`).
    Sobrescrito,
    /// `--dry-run` ativo: arquivo seria criado.
    CriariaSeExecutasse,
    /// `--dry-run` ativo: arquivo seria sobrescrito.
    SobrescreveriaSeExecutasse,
    /// Falha ao gravar — contém mensagem humana.
    Erro { mensagem: String },
}

/// Relatório individual por arquivo processado.
#[derive(Debug, Clone, Serialize)]
pub struct RelatorioArquivo {
    /// Caminho absoluto do arquivo.
    pub caminho: PathBuf,
    /// Ação aplicada/planejada.
    #[serde(flatten)]
    pub acao: AcaoArquivoConfig,
}

/// Relatório completo da inicialização.
#[derive(Debug, Clone, Serialize)]
pub struct RelatorioInitConfig {
    /// `true` se o modo foi `--dry-run` (nenhum I/O de escrita).
    pub dry_run: bool,
    /// `true` se o modo foi `--force` (sobrescreve existentes).
    pub force: bool,
    /// Diretório base usado (XDG / Apple / APPDATA).
    pub diretorio_base: Option<PathBuf>,
    /// Ações por arquivo — ordem estável.
    pub arquivos: Vec<RelatorioArquivo>,
}

/// Executa a inicialização com as flags fornecidas.
///
/// Retorna sempre `Ok` — falhas individuais ficam em `AcaoArquivoConfig::Erro`.
/// Retorna `Err` apenas se nenhum diretório de configuração puder ser determinado.
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

/// Processa UM arquivo — decide a ação correta conforme existência + flags.
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
