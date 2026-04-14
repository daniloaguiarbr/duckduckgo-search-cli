//! Detecção cross-platform e lançamento do Chrome headless via `chromiumoxide`.
//!
//! Este módulo só é compilado com a feature `chrome`, habilitada via
//! `cargo build --features chrome`. No modo default (sem feature) o binário
//! não tem NENHUMA dependência de chromiumoxide/tempfile/futures — zero overhead.
//!
//! ## Responsabilidades
//!
//! 1. [`detectar_chrome`] — detecta o caminho do executável Chrome/Chromium no
//!    sistema, com hierarquia de 3 camadas (flag manual → env var → auto-detecção).
//! 2. [`NavegadorChrome`] — wrapper seguro sobre `chromiumoxide::Browser` que
//!    garante cleanup do processo e handler-task via `impl Drop`.
//! 3. [`extrair_texto_com_chrome`] — navegação + extração de `document.body.innerText`
//!    com timeout configurável.
//!
//! ## Cleanup e Segurança de Processos (rules_rust.md — Gestão de Memória)
//!
//! `chromiumoxide::Browser` inicia um processo filho Chrome. Sem cleanup explícito,
//! o processo vira zumbi. A implementação de [`Drop`] em [`NavegadorChrome`]
//! aborta a tarefa handler e sinaliza `kill_on_drop` internamente. Para cleanup
//! síncrono completo, prefira chamar [`NavegadorChrome::desligar`] antes do drop.

#![cfg(feature = "chrome")]

use anyhow::{Context, Result};
use chromiumoxide::browser::{Browser, BrowserConfig, HeadlessMode};
use futures::StreamExt;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::task::JoinHandle;

/// Limite de caracteres por linha descartado pelo pipeline de limpeza.
const LIMIAR_LINHA_MINIMA: usize = 20;

/// Retorna uma lista ordenada de caminhos candidatos para Chrome/Chromium por plataforma.
///
/// Inclui instalações nativas, Flatpak e Snap. Windows consulta
/// variáveis de ambiente (`%PROGRAMFILES%`, `%LOCALAPPDATA%`) quando disponíveis.
pub fn caminhos_candidatos_chrome() -> Vec<PathBuf> {
    let mut candidatos: Vec<PathBuf> = Vec::new();

    #[cfg(target_os = "linux")]
    {
        for base in [
            "/usr/bin/google-chrome",
            "/usr/bin/google-chrome-stable",
            "/usr/bin/chromium",
            "/usr/bin/chromium-browser",
            "/usr/local/bin/chromium",
            "/usr/local/bin/google-chrome",
            "/opt/google/chrome/chrome",
            "/snap/bin/chromium",
            "/snap/bin/google-chrome",
            "/var/lib/flatpak/exports/bin/com.google.Chrome",
            "/var/lib/flatpak/exports/bin/org.chromium.Chromium",
        ] {
            candidatos.push(PathBuf::from(base));
        }
        if let Some(home) = dirs::home_dir() {
            candidatos.push(home.join(".local/share/flatpak/exports/bin/com.google.Chrome"));
            candidatos.push(home.join(".local/share/flatpak/exports/bin/org.chromium.Chromium"));
        }
    }

    #[cfg(target_os = "macos")]
    {
        for base in [
            "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
            "/Applications/Chromium.app/Contents/MacOS/Chromium",
            "/opt/homebrew/bin/chromium",
            "/opt/homebrew/bin/google-chrome",
            "/usr/local/bin/chromium",
            "/usr/local/bin/google-chrome",
        ] {
            candidatos.push(PathBuf::from(base));
        }
    }

    #[cfg(target_os = "windows")]
    {
        // Bases conhecidas.
        for base in [
            r"C:\Program Files\Google\Chrome\Application\chrome.exe",
            r"C:\Program Files (x86)\Google\Chrome\Application\chrome.exe",
            r"C:\Program Files\Chromium\Application\chrome.exe",
        ] {
            candidatos.push(PathBuf::from(base));
        }
        // Caminhos dependentes do usuário via %LOCALAPPDATA%.
        if let Ok(localappdata) = std::env::var("LOCALAPPDATA") {
            let base = PathBuf::from(&localappdata);
            candidatos.push(base.join(r"Google\Chrome\Application\chrome.exe"));
            candidatos.push(base.join(r"Chromium\Application\chrome.exe"));
        }
    }

    candidatos
}

/// Detecta o executável Chrome/Chromium com hierarquia de 3 camadas.
///
/// Ordem de resolução:
/// 1. `caminho_manual` (normalmente `--chrome-path`). Se fornecido mas inválido,
///    retorna erro — NÃO cai em fallback silencioso.
/// 2. Variável de ambiente `CHROME_PATH` (se definida e aponta para arquivo existente).
/// 3. Auto-detecção via [`caminhos_candidatos_chrome`] — primeiro que existir vence.
///
/// Retorna `Err` se nenhum candidato for encontrado.
pub fn detectar_chrome(caminho_manual: Option<&Path>) -> Result<PathBuf> {
    if let Some(p) = caminho_manual {
        if p.is_file() {
            tracing::debug!(path = %p.display(), "Chrome encontrado via --chrome-path");
            return Ok(p.to_path_buf());
        }
        anyhow::bail!(
            "--chrome-path {:?} não existe ou não é arquivo",
            p.display()
        );
    }

    if let Ok(env_path) = std::env::var("CHROME_PATH") {
        let p = PathBuf::from(&env_path);
        if p.is_file() {
            tracing::debug!(path = %p.display(), "Chrome encontrado via CHROME_PATH");
            return Ok(p);
        }
        tracing::warn!(
            path = env_path,
            "CHROME_PATH definido mas arquivo inexistente — tentando auto-detecção"
        );
    }

    for candidato in caminhos_candidatos_chrome() {
        if candidato.is_file() {
            tracing::debug!(path = %candidato.display(), "Chrome detectado automaticamente");
            return Ok(candidato);
        }
    }

    anyhow::bail!(
        "Chrome/Chromium não encontrado. Instale via gerenciador de pacotes \
        ou forneça --chrome-path ou CHROME_PATH."
    )
}

/// Indica se estamos rodando dentro de container ou wrapper Flatpak/Snap, o que
/// exige `--no-sandbox` para Chrome funcionar.
pub fn precisa_no_sandbox(caminho_chrome: &Path) -> bool {
    #[cfg(target_os = "linux")]
    {
        // Wrapper Flatpak ou Snap.
        let s = caminho_chrome.to_string_lossy();
        if s.contains("flatpak/exports/bin") || s.starts_with("/snap/") {
            return true;
        }
        // Rodando como root (comum em Docker).
        // SAFETY: libc::geteuid é thread-safe e não tem efeitos colaterais.
        #[cfg(unix)]
        {
            // Simplificação: detecta via variável de ambiente do Docker.
            if std::env::var("DOCKER_CONTAINER").is_ok()
                || std::path::Path::new("/.dockerenv").exists()
            {
                return true;
            }
        }
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = caminho_chrome;
    }
    false
}

/// Monta a lista de flags stealth cross-platform para o Chrome headless.
pub fn flags_stealth(precisa_sandbox_off: bool, proxy: Option<&str>) -> Vec<String> {
    let mut flags: Vec<String> = vec![
        "--disable-blink-features=AutomationControlled".to_string(),
        "--window-size=1920,1080".to_string(),
        "--disable-background-networking".to_string(),
        "--disable-default-apps".to_string(),
        "--disable-extensions".to_string(),
        "--disable-sync".to_string(),
        "--metrics-recording-only".to_string(),
        "--no-first-run".to_string(),
    ];

    #[cfg(target_os = "linux")]
    {
        flags.push("--disable-dev-shm-usage".to_string());
        if precisa_sandbox_off {
            flags.push("--no-sandbox".to_string());
        }
    }
    #[cfg(target_os = "windows")]
    {
        let _ = precisa_sandbox_off;
        flags.push("--disable-gpu".to_string());
    }
    #[cfg(target_os = "macos")]
    {
        let _ = precisa_sandbox_off;
    }

    if let Some(url_proxy) = proxy {
        flags.push(format!("--proxy-server={url_proxy}"));
    }

    flags
}

/// Wrapper RAII sobre `chromiumoxide::Browser`. Mantém o navegador e o handler-task vivos.
///
/// **Cleanup:** prefira chamar [`NavegadorChrome::desligar`] explicitamente (async).
/// O [`Drop`] apenas aborta a tarefa handler — o processo do Chrome pode demorar
/// alguns ms para encerrar. Para aplicações de longa duração, SEMPRE use `desligar`.
pub struct NavegadorChrome {
    browser: Browser,
    handler: Option<JoinHandle<()>>,
    /// Guarda o TempDir para garantir que o user-data-dir seja apagado no drop.
    _user_data: tempfile::TempDir,
}

impl NavegadorChrome {
    /// Lança o Chrome headless com a configuração stealth.
    ///
    /// - `caminho`: executável do Chrome (use [`detectar_chrome`] para obtê-lo).
    /// - `proxy`: URL opcional de proxy (propagada para o processo do navegador).
    /// - `timeout_launch`: limite para a inicialização do processo.
    pub async fn lancar(
        caminho: &Path,
        proxy: Option<&str>,
        timeout_launch: Duration,
    ) -> Result<Self> {
        tracing::info!(
            path = %caminho.display(),
            proxy = proxy.unwrap_or(""),
            "Lançando Chrome headless"
        );

        let sandbox_off = precisa_no_sandbox(caminho);
        let flags = flags_stealth(sandbox_off, proxy);
        let user_data = tempfile::tempdir().context("falha ao criar TempDir de user-data-dir")?;

        let mut builder = BrowserConfig::builder()
            .chrome_executable(caminho)
            .user_data_dir(user_data.path())
            .headless_mode(HeadlessMode::New)
            .launch_timeout(timeout_launch)
            .args(flags);

        if sandbox_off {
            builder = builder.no_sandbox();
        }

        let config = builder
            .build()
            .map_err(|e| anyhow::anyhow!("BrowserConfig inválido: {e}"))?;

        let (browser, mut handler) = Browser::launch(config)
            .await
            .context("falha ao lançar processo Chrome")?;

        // Handler-task: bombeia eventos até handler retornar None (encerrado).
        let tarefa_handler = tokio::spawn(async move {
            while let Some(evento) = handler.next().await {
                if let Err(erro) = evento {
                    tracing::debug!(?erro, "evento do handler CDP com erro — seguindo");
                }
            }
        });

        Ok(Self {
            browser,
            handler: Some(tarefa_handler),
            _user_data: user_data,
        })
    }

    /// Acessa o `Browser` interno para criar páginas.
    pub fn browser_mut(&mut self) -> &mut Browser {
        &mut self.browser
    }

    /// Encerra o navegador e aguarda cleanup do handler. Prefira este sobre Drop.
    pub async fn desligar(mut self) -> Result<()> {
        tracing::debug!("desligando Chrome via close() + wait()");
        if let Err(erro) = self.browser.close().await {
            tracing::debug!(?erro, "erro ao fechar browser — prosseguindo");
        }
        if let Err(erro) = self.browser.wait().await {
            tracing::debug!(?erro, "erro ao aguardar wait() do browser");
        }
        if let Some(h) = self.handler.take() {
            h.abort();
            let _ = h.await;
        }
        Ok(())
    }
}

impl Drop for NavegadorChrome {
    fn drop(&mut self) {
        if let Some(h) = self.handler.take() {
            h.abort();
        }
        tracing::debug!(
            "NavegadorChrome dropado — Browser::drop do chromiumoxide assume cleanup restante"
        );
    }
}

/// Extrai o texto principal de uma URL usando Chrome headless.
///
/// Estratégia:
/// 1. Abre nova página (`new_page`).
/// 2. Aguarda navegação concluída.
/// 3. Executa JS `document.body.innerText` e coleta como `String`.
/// 4. Limpa whitespace + linhas curtas + trunca em `tamanho_max`.
/// 5. Fecha página imediatamente.
///
/// O `timeout` aplica-se à operação completa via `tokio::time::timeout`.
pub async fn extrair_texto_com_chrome(
    navegador: &mut NavegadorChrome,
    url: &str,
    tamanho_max: usize,
    timeout: Duration,
) -> Result<String> {
    let trabalho = async {
        let pagina = navegador
            .browser_mut()
            .new_page(url)
            .await
            .with_context(|| format!("falha ao abrir página {url:?}"))?;

        // Aguarda navegação completa (respeita redirects).
        let _ = pagina.wait_for_navigation().await;

        let resultado_js = pagina
            .evaluate("document.body ? document.body.innerText : ''")
            .await
            .with_context(|| format!("falha ao executar innerText em {url:?}"))?;

        let texto_bruto: String = resultado_js.into_value().unwrap_or_else(|_| String::new());

        // Fecha a página imediatamente para liberar target.
        let _ = pagina.close().await;

        Ok::<String, anyhow::Error>(limpar_texto(&texto_bruto, tamanho_max))
    };

    tokio::time::timeout(timeout, trabalho)
        .await
        .with_context(|| format!("timeout de Chrome excedido em {url:?}"))?
}

/// Limpa texto bruto: normaliza whitespace, descarta linhas curtas, trunca em `tamanho_max`.
fn limpar_texto(bruto: &str, tamanho_max: usize) -> String {
    let linhas: Vec<String> = bruto
        .lines()
        .map(|linha| linha.split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|linha| linha.chars().count() >= LIMIAR_LINHA_MINIMA)
        .collect();
    let juntado = linhas.join("\n");
    truncar_em_palavra(&juntado, tamanho_max)
}

/// Trunca respeitando fronteira de palavra. Espelha a implementação de `content.rs`.
fn truncar_em_palavra(texto: &str, tamanho_max: usize) -> String {
    if tamanho_max == 0 {
        return String::new();
    }
    let total: usize = texto.chars().count();
    if total <= tamanho_max {
        return texto.to_string();
    }
    let prefixo: String = texto.chars().take(tamanho_max).collect();
    if let Some(pos) = prefixo.rfind(char::is_whitespace) {
        return prefixo[..pos].trim_end().to_string();
    }
    prefixo
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn caminhos_candidatos_chrome_nao_vazio() {
        let lista = caminhos_candidatos_chrome();
        assert!(!lista.is_empty(), "deve retornar ao menos um candidato");
    }

    #[test]
    fn detectar_chrome_caminho_manual_inexistente_falha() {
        let p = Path::new("/tmp/caminho/absolutamente/inexistente/chrome-xyz");
        assert!(
            detectar_chrome(Some(p)).is_err(),
            "caminho manual inválido deve falhar"
        );
    }

    #[test]
    fn flags_stealth_inclui_anti_detection() {
        let f = flags_stealth(false, None);
        assert!(f.iter().any(|x| x.contains("AutomationControlled")));
        assert!(f.iter().any(|x| x == "--window-size=1920,1080"));
    }

    #[test]
    fn flags_stealth_inclui_proxy_quando_fornecido() {
        let f = flags_stealth(false, Some("http://proxy:8080"));
        assert!(f.iter().any(|x| x == "--proxy-server=http://proxy:8080"));
    }

    #[test]
    fn flags_stealth_no_sandbox_apenas_quando_requerido_no_linux() {
        let f_com = flags_stealth(true, None);
        let f_sem = flags_stealth(false, None);
        #[cfg(target_os = "linux")]
        {
            assert!(f_com.iter().any(|x| x == "--no-sandbox"));
            assert!(!f_sem.iter().any(|x| x == "--no-sandbox"));
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = (f_com, f_sem);
        }
    }

    #[test]
    fn limpar_texto_remove_linhas_curtas() {
        let bruto = "ok\noutra linha com tamanho bastante suficiente de vinte chars\ncurta\n";
        let limpo = limpar_texto(bruto, 1000);
        assert!(limpo.contains("outra linha"));
        assert!(!limpo.contains("ok\n"));
    }

    #[test]
    fn limpar_texto_trunca_em_palavra() {
        let bruto =
            "linha um com mais de vinte caracteres definitivamente aqui presentes\n".repeat(10);
        let limpo = limpar_texto(&bruto, 50);
        assert!(limpo.chars().count() <= 50);
    }

    #[test]
    fn precisa_no_sandbox_flatpak_path() {
        let p = Path::new("/var/lib/flatpak/exports/bin/com.google.Chrome");
        #[cfg(target_os = "linux")]
        assert!(precisa_no_sandbox(p));
        #[cfg(not(target_os = "linux"))]
        {
            let _ = p;
        }
    }

    #[test]
    fn precisa_no_sandbox_snap_path() {
        let p = Path::new("/snap/bin/chromium");
        #[cfg(target_os = "linux")]
        assert!(precisa_no_sandbox(p));
        #[cfg(not(target_os = "linux"))]
        {
            let _ = p;
        }
    }

    #[test]
    fn precisa_no_sandbox_padrao_retorna_false() {
        let p = Path::new("/usr/bin/chromium");
        #[cfg(target_os = "linux")]
        {
            // False EXCETO se DOCKER_CONTAINER ou /.dockerenv presente.
            let esperado = std::env::var("DOCKER_CONTAINER").is_ok()
                || std::path::Path::new("/.dockerenv").exists();
            assert_eq!(precisa_no_sandbox(p), esperado);
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = p;
        }
    }
}
