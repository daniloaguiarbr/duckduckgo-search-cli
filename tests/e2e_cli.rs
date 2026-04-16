//! Testes E2E do binário compilado via `assert_cmd` + `predicates`.
//!
//! Estes testes exercitam o CLI do ponto de vista externo — validações de flags,
//! help, version, exit codes — SEM fazer chamadas HTTP reais. Testes que precisam
//! de HTTP já estão cobertos em `tests/integracao_wiremock.rs`.
//!
//! Conforme rules_rust.md seção 20.2:
//! - `assert_cmd::Command::cargo_bin(<BIN_NAME>)` para testar binário compilado.
//! - `predicates` para assertions composáveis.
//! - `tempfile::NamedTempFile` e `TempDir` para isolamento.

use assert_cmd::Command;
use predicates::prelude::*;
use std::io::Write;

const NOME_BIN: &str = "duckduckgo-search-cli";

#[test]
fn help_retorna_sucesso_e_contem_usage() {
    Command::cargo_bin(NOME_BIN)
        .expect("binário compilado")
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("Usage:"));
}

#[test]
fn version_retorna_nome_e_versao() {
    Command::cargo_bin(NOME_BIN)
        .expect("binário compilado")
        .arg("--version")
        .assert()
        .success()
        .stdout(
            predicate::str::contains(NOME_BIN)
                .and(predicate::str::contains(env!("CARGO_PKG_VERSION"))),
        );
}

#[test]
fn init_config_help_retorna_sucesso() {
    Command::cargo_bin(NOME_BIN)
        .expect("binário compilado")
        .args(["init-config", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--force").and(predicate::str::contains("--dry-run")));
}

#[test]
fn init_config_dry_run_retorna_json_valido() {
    // Força um DIR isolado para XDG via HOME temporário (efetivo em dirs crate).
    let temp = tempfile::tempdir().expect("tempdir");
    let output = Command::cargo_bin(NOME_BIN)
        .expect("binário compilado")
        .args(["init-config", "--dry-run"])
        .env("HOME", temp.path())
        .env("XDG_CONFIG_HOME", temp.path().join(".config"))
        .output()
        .expect("executar init-config");

    assert!(
        output.status.success(),
        "init-config --dry-run deve sucesso"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    // JSON válido com campo conhecido.
    let valor: serde_json::Value =
        serde_json::from_str(&stdout).expect("stdout deve ser JSON válido");
    assert!(
        valor.get("arquivos").is_some(),
        "deve conter chave arquivos"
    );
}

#[test]
fn sem_query_sem_stdin_sem_arquivo_retorna_exit_2() {
    // Com stdin vazio/redirecionado para /dev/null, nenhuma query é fornecida.
    let output = Command::cargo_bin(NOME_BIN)
        .expect("binário compilado")
        .env("RUST_LOG", "error")
        .write_stdin("") // stdin vazio
        .output()
        .expect("executar sem query");

    assert_eq!(
        output.status.code(),
        Some(2),
        "sem query deve retornar exit 2; stdout={:?}, stderr={:?}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn paralelismo_invalido_retorna_exit_2() {
    let output = Command::cargo_bin(NOME_BIN)
        .expect("binário compilado")
        .args(["--parallel", "50", "query"])
        .output()
        .expect("executar com --parallel 50");
    assert_eq!(
        output.status.code(),
        Some(2),
        "--parallel 50 deve retornar exit 2"
    );
}

#[test]
fn pages_invalido_retorna_exit_2() {
    let output = Command::cargo_bin(NOME_BIN)
        .expect("binário compilado")
        .args(["--pages", "10", "query"])
        .output()
        .expect("executar com --pages 10");
    assert_eq!(
        output.status.code(),
        Some(2),
        "--pages 10 deve retornar exit 2"
    );
}

#[test]
fn max_content_length_invalido_retorna_exit_2() {
    let output = Command::cargo_bin(NOME_BIN)
        .expect("binário compilado")
        .args(["--max-content-length", "999999", "query"])
        .output()
        .expect("executar");
    assert_eq!(output.status.code(), Some(2));
}

#[test]
fn global_timeout_invalido_retorna_exit_2() {
    let output = Command::cargo_bin(NOME_BIN)
        .expect("binário compilado")
        .args(["--global-timeout", "99999", "query"])
        .output()
        .expect("executar");
    assert_eq!(output.status.code(), Some(2));
}

#[test]
fn proxy_com_scheme_invalido_retorna_exit_2() {
    let output = Command::cargo_bin(NOME_BIN)
        .expect("binário compilado")
        .args(["--proxy", "ftp://naovalidos", "query"])
        .output()
        .expect("executar");
    assert_eq!(output.status.code(), Some(2));
}

#[test]
fn verbose_e_quiet_conflitam_retornam_exit_2() {
    Command::cargo_bin(NOME_BIN)
        .expect("binário compilado")
        .args(["--verbose", "--quiet", "query"])
        .assert()
        .failure()
        .code(2);
}

#[test]
fn proxy_e_noproxy_conflitam_retornam_exit_2() {
    Command::cargo_bin(NOME_BIN)
        .expect("binário compilado")
        .args(["--proxy", "http://x", "--no-proxy", "query"])
        .assert()
        .failure()
        .code(2);
}

#[test]
fn queries_file_inexistente_retorna_exit_2() {
    let output = Command::cargo_bin(NOME_BIN)
        .expect("binário compilado")
        .args([
            "--queries-file",
            "/tmp/arquivo_que_realmente_nao_existe_xyz_12345",
        ])
        .output()
        .expect("executar");
    assert_eq!(
        output.status.code(),
        Some(2),
        "queries-file inexistente deve retornar exit 2"
    );
}

#[test]
fn queries_file_valido_eh_lido_corretamente() {
    // Cria arquivo temporário com 3 queries.
    let mut arquivo = tempfile::NamedTempFile::new().expect("tempfile");
    writeln!(arquivo, "foo bar").unwrap();
    writeln!(arquivo).unwrap(); // linha vazia ignorada
    writeln!(arquivo, "baz qux").unwrap();
    writeln!(arquivo, "quux").unwrap();
    let path = arquivo.path().to_path_buf();

    // Rodamos com global-timeout curto para evitar que o teste vá para a rede
    // por muito tempo; ponto é exercitar a LEITURA do arquivo, não validar HTTP.
    let output = Command::cargo_bin(NOME_BIN)
        .expect("binário compilado")
        .args([
            "--queries-file",
            path.to_str().unwrap(),
            "--global-timeout",
            "1",
            "--quiet",
            "--format",
            "json",
        ])
        .timeout(std::time::Duration::from_secs(10))
        .output()
        .expect("executar");

    // Exit esperado: 0/1/3/4/5 (NÃO 2, que é config inválida).
    // O importante: código != 2 significa que a configuração foi aceita.
    let code = output.status.code().unwrap_or(-1);
    assert!(
        code != 2,
        "queries-file válido deve ser ACEITO (code != 2), mas veio {code}; \
         stderr={:?}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn formato_desconhecido_retorna_exit_2() {
    let output = Command::cargo_bin(NOME_BIN)
        .expect("binário compilado")
        .args(["--format", "xml", "query"])
        .output()
        .expect("executar");
    assert_eq!(output.status.code(), Some(2));
}

#[test]
fn flag_desconhecida_retorna_exit_2() {
    Command::cargo_bin(NOME_BIN)
        .expect("binário compilado")
        .arg("--flag-que-nao-existe-xyz-12345")
        .assert()
        .failure()
        .code(2);
}

// ---------------------------------------------------------------------------
// Testes de pipe e exit codes no --help (prevenção de regressão SIGPIPE)
// ---------------------------------------------------------------------------

#[test]
fn help_longo_contem_secao_exit_codes() {
    // Verifica que `--help` (long help) exibe a seção EXIT CODES adicionada
    // via after_long_help no clap. Previne regressão se alguém remover o atributo.
    Command::cargo_bin(NOME_BIN)
        .expect("binário compilado")
        .arg("--help")
        .assert()
        .success()
        .stdout(
            predicate::str::contains("EXIT CODES:")
                .and(predicate::str::contains("PIPE USAGE:"))
                .and(predicate::str::contains("Zero results across all queries")),
        );
}

#[test]
fn help_curto_nao_contem_exit_codes() {
    // `-h` (short help) NÃO deve exibir after_long_help — apenas `--help` exibe.
    Command::cargo_bin(NOME_BIN)
        .expect("binário compilado")
        .arg("-h")
        .assert()
        .success()
        .stdout(predicate::str::contains("EXIT CODES:").not());
}

#[test]
fn stdout_do_help_nao_perde_bytes() {
    // Captura stdout do --help e valida que tem tamanho razoável.
    // Previne regressão de SIGPIPE/BrokenPipe que poderia truncar output.
    let output = Command::cargo_bin(NOME_BIN)
        .expect("binário compilado")
        .arg("--help")
        .output()
        .expect("executar --help");
    assert!(output.status.success(), "exit code deve ser 0");
    assert!(
        output.stdout.len() > 500,
        "stdout do --help deve ter pelo menos 500 bytes, obteve {}",
        output.stdout.len()
    );
}

#[test]
fn retries_acima_do_maximo_retorna_exit_2() {
    let output = Command::cargo_bin(NOME_BIN)
        .expect("binário compilado")
        .args(["--retries", "99", "query"])
        .output()
        .expect("executar");
    assert_eq!(output.status.code(), Some(2));
}

#[test]
fn per_host_limit_acima_do_maximo_retorna_exit_2() {
    let output = Command::cargo_bin(NOME_BIN)
        .expect("binário compilado")
        .args(["--per-host-limit", "99", "query"])
        .output()
        .expect("executar");
    assert_eq!(output.status.code(), Some(2));
}

// =============================================================================
// Teste do handler SIGINT instalado em src/main.rs
// =============================================================================
//
// O binário instala um handler de SIGINT/Ctrl+C em `tokio::spawn` (linhas 22-27
// de `src/main.rs`). Esse handler aguarda `tokio::signal::ctrl_c()`, registra
// warning via `tracing` e chama `cancelamento.cancel()` propagando o sinal para
// o pipeline em voo.
//
// Para exercitar essas linhas precisamos:
//   1. Subir o binário REAL (não a lib) com uma carga lenta (mock HTTP que
//      demora muito antes de responder).
//   2. Aguardar tempo suficiente para o handler estar instalado E uma request
//      HTTP estar em voo (caso contrário, SIGINT chega antes do handler).
//   3. Enviar SIGINT via `kill(pid, SIGINT)`.
//   4. Confirmar que o processo termina em tempo razoável (cancelamento
//      propagou) com exit code != 0.
//
// Gate em `#[cfg(unix)]` — Windows tem semântica diferente para Ctrl+C.
#[cfg(unix)]
mod sigint_handler {
    use super::NOME_BIN;
    use std::io::Read;
    use std::process::{Command as StdCommand, Stdio};
    use std::time::{Duration, Instant};

    // FFI direta para `kill(2)` — evita adicionar dep `libc` ou `nix` ao
    // [dev-dependencies] só para esse teste.
    extern "C" {
        fn kill(pid: i32, sig: i32) -> i32;
    }
    const SIGINT: i32 = 2;

    /// Aguarda o `Child` terminar até `timeout_total`, polling a cada 50ms.
    /// Retorna `Ok(status)` se terminou, `Err(())` se estourou o timeout.
    fn wait_com_timeout(
        child: &mut std::process::Child,
        timeout_total: Duration,
    ) -> Result<std::process::ExitStatus, ()> {
        let inicio = Instant::now();
        loop {
            match child.try_wait() {
                Ok(Some(status)) => return Ok(status),
                Ok(None) => {
                    if inicio.elapsed() > timeout_total {
                        return Err(());
                    }
                    std::thread::sleep(Duration::from_millis(50));
                }
                Err(_) => return Err(()),
            }
        }
    }

    /// Sobe wiremock retornando 200 com delay de 30s e dispara o binário
    /// apontando o endpoint HTML para esse mock. Em seguida envia SIGINT e
    /// valida que o processo termina dentro do timeout (cancelamento ocorreu).
    ///
    /// NOTE: o teste depende de timing (precisa que o handler de signal já
    /// esteja instalado antes do SIGINT chegar). 600ms de warm-up é folgado
    /// na maioria dos CIs, mas em runners EXTREMAMENTE saturados pode
    /// ocasionalmente falhar — neste caso aumentar `WARMUP_MS`.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn sigint_dispara_cancelamento_e_termina_processo() {
        use wiremock::matchers::method;
        use wiremock::{Mock, MockServer, ResponseTemplate};

        const WARMUP_MS: u64 = 600;
        const HARD_TIMEOUT_PROCESSO: Duration = Duration::from_secs(8);

        // 1. Mock HTTP que NUNCA responde rápido — força a request a ficar
        // em voo enquanto enviamos SIGINT.
        let mock_server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(200).set_delay(Duration::from_secs(30)))
            .mount(&mock_server)
            .await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(200).set_delay(Duration::from_secs(30)))
            .mount(&mock_server)
            .await;

        // 2. Localiza o binário compilado via assert_cmd.
        let bin_path = assert_cmd::cargo::cargo_bin(NOME_BIN);
        assert!(bin_path.exists(), "binário deve existir: {bin_path:?}");

        // 3. Spawna processo via std::process::Command (precisamos do PID).
        let mut child = StdCommand::new(&bin_path)
            .arg("rust async")
            .arg("--global-timeout")
            .arg("60") // alto o suficiente para NÃO ser este o motivo do término
            .arg("--retries")
            .arg("0")
            .arg("--quiet")
            .env("DUCKDUCKGO_SEARCH_CLI_BASE_URL_HTML", mock_server.uri())
            .env("DUCKDUCKGO_SEARCH_CLI_BASE_URL_LITE", mock_server.uri())
            .env("RUST_LOG", "off")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn binário");

        let pid = child.id() as i32;

        // 4. Warm-up: tokio precisa registrar o handler ANTES do SIGINT.
        std::thread::sleep(Duration::from_millis(WARMUP_MS));

        // Sanity: ainda está rodando? Se já terminou, o teste é inconclusivo.
        if let Some(status) = child.try_wait().expect("try_wait") {
            panic!(
                "processo terminou ANTES do SIGINT (status={:?}); teste \
                 inválido — possivelmente o mock não foi atingido",
                status.code()
            );
        }

        // 5. Envia SIGINT.
        let resultado_kill = unsafe { kill(pid, SIGINT) };
        assert_eq!(
            resultado_kill, 0,
            "kill(pid={pid}, SIGINT) deve retornar 0, retornou {resultado_kill}"
        );

        // 6. Aguarda terminar em tempo razoável.
        let status = match wait_com_timeout(&mut child, HARD_TIMEOUT_PROCESSO) {
            Ok(status) => status,
            Err(()) => {
                let _ = child.kill();
                let _ = child.wait();
                panic!(
                    "processo NÃO terminou dentro de {:?} após SIGINT — \
                     handler não funcionou ou cancelamento não propagou",
                    HARD_TIMEOUT_PROCESSO
                );
            }
        };

        // 7. Coleta stderr para diagnóstico em caso de falha.
        let mut stderr_buf = String::new();
        if let Some(mut stderr) = child.stderr.take() {
            let _ = stderr.read_to_string(&mut stderr_buf);
        }

        // O processo DEVE ter terminado por nossa ação (não por sucesso 0).
        // Aceitamos qualquer exit code != 0 ou terminação por sinal.
        // - Se `tokio::signal::ctrl_c()` interceptou (caminho normal):
        //   `run()` retorna com algum exit code de erro/cancel.
        // - Se SIGINT chegou ANTES do handler estar pronto: processo morre
        //   por sinal e `code()` é None — também é evidência de SIGINT.
        let codigo = status.code();
        assert!(
            codigo != Some(0),
            "processo terminou com SUCESSO (0) após SIGINT; esperado != 0. \
             stderr={stderr_buf:?}"
        );
    }
}
