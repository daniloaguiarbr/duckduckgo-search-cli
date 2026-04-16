//! Validação e sanitização de paths para operações de I/O.
//!
//! Este módulo centraliza a validação de caminhos de saída fornecidos pelo
//! usuário via `--output`, prevenindo path traversal e escrita em diretórios
//! de sistema. Também encapsula criação de diretórios pai e aplicação de
//! permissões Unix.

use anyhow::{bail, Context, Result};
use std::path::{Component, Path, PathBuf};

/// Valida um caminho de saída fornecido pelo usuário.
///
/// Rejeita paths que contenham componentes `..` (path traversal) e paths
/// absolutos que apontem para diretórios de sistema protegidos.
/// Retorna o caminho validado como `PathBuf`.
pub fn validar_caminho_saida(caminho: &Path) -> Result<PathBuf> {
    // Rejeitar componentes ".." em qualquer posição
    for componente in caminho.components() {
        if matches!(componente, Component::ParentDir) {
            bail!(
                "caminho de saída rejeitado — contém '..' (path traversal): {}",
                caminho.display()
            );
        }
    }

    // Rejeitar paths absolutos que apontem para diretórios de sistema
    if caminho.is_absolute() {
        let caminho_str = caminho.to_string_lossy();
        let diretorios_protegidos_unix = [
            "/etc", "/usr", "/bin", "/sbin", "/boot", "/proc", "/sys", "/dev",
        ];
        let diretorios_protegidos_windows = [
            "C:\\Windows",
            "C:\\Program Files",
            "C:\\Program Files (x86)",
        ];

        for dir in &diretorios_protegidos_unix {
            if caminho_str.starts_with(dir) {
                bail!(
                    "caminho de saída rejeitado — aponta para diretório de sistema: {}",
                    caminho.display()
                );
            }
        }
        for dir in &diretorios_protegidos_windows {
            if caminho_str.to_lowercase().starts_with(&dir.to_lowercase()) {
                bail!(
                    "caminho de saída rejeitado — aponta para diretório de sistema: {}",
                    caminho.display()
                );
            }
        }
    }

    Ok(caminho.to_path_buf())
}

/// Cria diretórios pai de um caminho, se necessário.
pub fn criar_diretorios_pai(caminho: &Path) -> Result<()> {
    if let Some(pai) = caminho.parent() {
        if !pai.as_os_str().is_empty() && !pai.exists() {
            std::fs::create_dir_all(pai)
                .with_context(|| format!("falha ao criar diretórios pai: {}", pai.display()))?;
        }
    }
    Ok(())
}

/// Aplica permissões 0o644 em um arquivo no Unix (dono lê+escreve, outros leem).
/// No-op em plataformas não-Unix.
#[cfg(unix)]
pub fn aplicar_permissoes_644(caminho: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let permissoes = std::fs::Permissions::from_mode(0o644);
    std::fs::set_permissions(caminho, permissoes)
        .with_context(|| format!("falha ao aplicar permissões 0o644 em {}", caminho.display()))?;
    Ok(())
}

#[cfg(not(unix))]
pub fn aplicar_permissoes_644(_caminho: &Path) -> Result<()> {
    Ok(())
}

#[cfg(test)]
mod testes {
    use super::*;
    use std::path::Path;

    #[test]
    fn rejeita_path_com_parent_dir() {
        let resultado = validar_caminho_saida(Path::new("../../etc/passwd"));
        assert!(resultado.is_err());
        let msg = resultado.unwrap_err().to_string();
        assert!(msg.contains("path traversal"), "mensagem: {msg}");
    }

    #[test]
    fn rejeita_path_com_parent_dir_no_meio() {
        let resultado = validar_caminho_saida(Path::new("output/../../../evil.json"));
        assert!(resultado.is_err());
    }

    #[test]
    fn aceita_path_relativo_simples() {
        let resultado = validar_caminho_saida(Path::new("output/resultado.json"));
        assert!(resultado.is_ok());
    }

    #[test]
    fn aceita_path_relativo_com_ponto_simples() {
        let resultado = validar_caminho_saida(Path::new("./resultado.json"));
        assert!(resultado.is_ok());
    }

    #[test]
    fn aceita_path_absoluto_tmp() {
        let resultado = validar_caminho_saida(Path::new("/tmp/ddg_resultado.json"));
        assert!(resultado.is_ok());
    }

    #[test]
    fn rejeita_path_absoluto_etc() {
        let resultado = validar_caminho_saida(Path::new("/etc/shadow"));
        assert!(resultado.is_err());
        let msg = resultado.unwrap_err().to_string();
        assert!(msg.contains("diretório de sistema"), "mensagem: {msg}");
    }

    #[test]
    fn rejeita_path_absoluto_usr() {
        let resultado = validar_caminho_saida(Path::new("/usr/bin/evil"));
        assert!(resultado.is_err());
    }

    #[test]
    fn aceita_path_absoluto_home() {
        let resultado = validar_caminho_saida(Path::new("/home/user/resultado.json"));
        assert!(resultado.is_ok());
    }

    #[test]
    fn criar_diretorios_pai_com_tempdir() {
        let tmp = tempfile::tempdir().expect("falha ao criar tempdir");
        let caminho = tmp.path().join("sub").join("resultado.json");
        let resultado = criar_diretorios_pai(&caminho);
        assert!(resultado.is_ok());
        assert!(caminho.parent().expect("tem pai").exists());
    }

    #[test]
    fn nome_arquivo_simples_sem_pai() {
        let resultado = validar_caminho_saida(Path::new("resultado.json"));
        assert!(resultado.is_ok());
    }
}
