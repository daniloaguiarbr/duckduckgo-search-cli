// SPDX-License-Identifier: MIT OR Apache-2.0
//! Testes E2E do binário compilado via `assert_cmd` + `predicates`.
//!
//! Estes testes exercitam o CLI do ponto de vista externo — validações de flags,
//! help, version, exit codes — SEM fazer chamadas HTTP reais. Testes que precisam
//! de HTTP já estão cobertos em `tests/integration_wiremock.rs`.
//!
//! Conforme `rules_rust.md` seção 20.2:
//! - `assert_cmd::Command::cargo_bin(<BIN_NAME>)` para testar binário compilado.
//! - `predicates` para assertions composáveis.
//! - `tempfile::NamedTempFile` e `TempDir` para isolamento.

use assert_cmd::Command;
use predicates::prelude::*;
use std::io::Write;

const BIN_NAME: &str = "duckduckgo-search-cli";

#[test]
fn help_returns_success_and_contains_usage() {
    Command::cargo_bin(BIN_NAME)
        .expect("binário compilado")
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("Usage:"));
}

#[test]
fn version_returns_name_and_version() {
    Command::cargo_bin(BIN_NAME)
        .expect("binário compilado")
        .arg("--version")
        .assert()
        .success()
        .stdout(
            predicate::str::contains(BIN_NAME)
                .and(predicate::str::contains(env!("CARGO_PKG_VERSION"))),
        );
}

#[test]
fn init_config_help_returns_success() {
    Command::cargo_bin(BIN_NAME)
        .expect("binário compilado")
        .args(["init-config", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--force").and(predicate::str::contains("--dry-run")));
}

#[test]
fn init_config_dry_run_returns_valid_json() {
    // Force an isolated DIR for XDG via temporary HOME (effective in dirs crate).
    let temp = tempfile::tempdir().expect("tempdir");
    let output = Command::cargo_bin(BIN_NAME)
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
    // Valid JSON with a known field.
    let value: serde_json::Value =
        serde_json::from_str(&stdout).expect("stdout deve ser JSON válido");
    assert!(
        value.get("arquivos").is_some(),
        "deve conter chave arquivos"
    );
}

#[test]
fn no_query_no_stdin_no_file_returns_exit_2() {
    // With empty/redirected stdin to /dev/null, no query is provided.
    let output = Command::cargo_bin(BIN_NAME)
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
fn invalid_parallelism_returns_exit_2() {
    let output = Command::cargo_bin(BIN_NAME)
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
fn invalid_pages_returns_exit_2() {
    let output = Command::cargo_bin(BIN_NAME)
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
fn invalid_max_content_length_returns_exit_2() {
    let output = Command::cargo_bin(BIN_NAME)
        .expect("binário compilado")
        .args(["--max-content-length", "999999", "query"])
        .output()
        .expect("executar");
    assert_eq!(output.status.code(), Some(2));
}

#[test]
fn invalid_global_timeout_returns_exit_2() {
    let output = Command::cargo_bin(BIN_NAME)
        .expect("binário compilado")
        .args(["--global-timeout", "99999", "query"])
        .output()
        .expect("executar");
    assert_eq!(output.status.code(), Some(2));
}

#[test]
fn proxy_with_invalid_scheme_returns_exit_2() {
    let output = Command::cargo_bin(BIN_NAME)
        .expect("binário compilado")
        .args(["--proxy", "ftp://naovalidos", "query"])
        .output()
        .expect("executar");
    assert_eq!(output.status.code(), Some(2));
}

#[test]
fn verbose_e_quiet_conflitam_retornam_exit_2() {
    Command::cargo_bin(BIN_NAME)
        .expect("binário compilado")
        .args(["--verbose", "--quiet", "query"])
        .assert()
        .failure()
        .code(2);
}

#[test]
fn proxy_e_noproxy_conflitam_retornam_exit_2() {
    Command::cargo_bin(BIN_NAME)
        .expect("binário compilado")
        .args(["--proxy", "http://x", "--no-proxy", "query"])
        .assert()
        .failure()
        .code(2);
}

#[test]
fn nonexistent_queries_file_returns_exit_2() {
    let output = Command::cargo_bin(BIN_NAME)
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
fn valid_queries_file_is_read_correctly() {
    // Create a temporary file with 3 queries.
    let mut file = tempfile::NamedTempFile::new().expect("tempfile");
    writeln!(file, "foo bar").unwrap();
    writeln!(file).unwrap(); // linha vazia ignorada
    writeln!(file, "baz qux").unwrap();
    writeln!(file, "quux").unwrap();
    let path = file.path().to_path_buf();

    // Run with a short global-timeout to avoid the test going to the network
    // for too long; the point is to exercise file READING, not validate HTTP.
    let output = Command::cargo_bin(BIN_NAME)
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

    // Expected exit: 0/1/3/4/5 (NOT 2, which is invalid config).
    // The key point: code != 2 means the configuration was accepted.
    let code = output.status.code().unwrap_or(-1);
    assert!(
        code != 2,
        "queries-file válido deve ser ACEITO (code != 2), mas veio {code}; \
         stderr={:?}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn unknown_format_returns_exit_2() {
    let output = Command::cargo_bin(BIN_NAME)
        .expect("binário compilado")
        .args(["--format", "xml", "query"])
        .output()
        .expect("executar");
    assert_eq!(output.status.code(), Some(2));
}

#[test]
fn unknown_flag_returns_exit_2() {
    Command::cargo_bin(BIN_NAME)
        .expect("binário compilado")
        .arg("--flag-que-nao-existe-xyz-12345")
        .assert()
        .failure()
        .code(2);
}

// ---------------------------------------------------------------------------
// Pipe and exit code tests for --help (SIGPIPE regression prevention)
// ---------------------------------------------------------------------------

#[test]
fn long_help_contains_exit_codes_section() {
    // Verify that `--help` (long help) shows the EXIT CODES section added
    // via after_long_help in clap. Prevents regression if someone removes the attribute.
    Command::cargo_bin(BIN_NAME)
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
fn short_help_does_not_contain_exit_codes() {
    // `-h` (short help) NÃO deve exibir after_long_help — apenas `--help` exibe.
    Command::cargo_bin(BIN_NAME)
        .expect("binário compilado")
        .arg("-h")
        .assert()
        .success()
        .stdout(predicate::str::contains("EXIT CODES:").not());
}

#[test]
fn help_stdout_does_not_lose_bytes() {
    // Capture stdout from --help and validate it has a reasonable size.
    // Prevents SIGPIPE/BrokenPipe regression that could truncate output.
    let output = Command::cargo_bin(BIN_NAME)
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
fn retries_above_max_returns_exit_2() {
    let output = Command::cargo_bin(BIN_NAME)
        .expect("binário compilado")
        .args(["--retries", "99", "query"])
        .output()
        .expect("executar");
    assert_eq!(output.status.code(), Some(2));
}

#[test]
fn per_host_limit_above_max_returns_exit_2() {
    let output = Command::cargo_bin(BIN_NAME)
        .expect("binário compilado")
        .args(["--per-host-limit", "99", "query"])
        .output()
        .expect("executar");
    assert_eq!(output.status.code(), Some(2));
}

#[test]
fn timeout_zero_returns_exit_2() {
    let output = Command::cargo_bin(BIN_NAME)
        .expect("binário compilado")
        .args(["--timeout", "0", "query"])
        .output()
        .expect("executar com --timeout 0");
    assert_eq!(
        output.status.code(),
        Some(2),
        "--timeout 0 deve retornar exit 2 (configuração inválida)"
    );
}

#[test]
fn output_with_path_traversal_returns_exit_2() {
    let output = Command::cargo_bin(BIN_NAME)
        .expect("binário compilado")
        .args(["--output", "/tmp/../../etc/passwd", "query"])
        .output()
        .expect("executar com --output path traversal");
    assert_eq!(
        output.status.code(),
        Some(2),
        "--output com path traversal deve retornar exit 2 (configuração inválida)"
    );
}

// =============================================================================
// Teste do handler SIGINT instalado em src/main.rs
// =============================================================================
//
// The binary installs a SIGINT/Ctrl+C handler in `tokio::spawn` (lines 22-27
// of `src/main.rs`). This handler awaits `tokio::signal::ctrl_c()`, logs a
// warning via `tracing` and calls `cancelamento.cancel()` propagating the signal
// to the in-flight pipeline.
//
// To exercise these lines we need to:
//   1. Launch the REAL binary (not the lib) with a slow load (mock HTTP that
//      takes a long time before responding).
//   2. Wait long enough for the handler to be installed AND an HTTP request
//      to be in-flight (otherwise SIGINT arrives before the handler).
//   3. Send SIGINT via `kill(pid, SIGINT)`.
//   4. Confirm the process terminates in a reasonable time (cancellation
//      propagated) with exit code != 0.
//
// Gated on `#[cfg(unix)]` — Windows has different semantics for Ctrl+C.
#[cfg(unix)]
mod sigint_handler {
    use super::BIN_NAME;
    use std::io::Read;
    use std::process::{Command as StdCommand, Stdio};
    use std::time::{Duration, Instant};

    // Direct FFI for `kill(2)` — avoids adding `libc` or `nix` dep to
    // [dev-dependencies] just for this test.
    extern "C" {
        fn kill(pid: i32, sig: i32) -> i32;
    }
    const SIGINT: i32 = 2;

    /// Waits for the `Child` to terminate up to `timeout_total`, polling every 50ms.
    /// Returns `Ok(status)` if it terminated, `Err(())` if the timeout was exceeded.
    fn wait_with_timeout(
        child: &mut std::process::Child,
        timeout_total: Duration,
    ) -> Result<std::process::ExitStatus, ()> {
        let start = Instant::now();
        loop {
            match child.try_wait() {
                Ok(Some(status)) => return Ok(status),
                Ok(None) => {
                    if start.elapsed() > timeout_total {
                        return Err(());
                    }
                    std::thread::sleep(Duration::from_millis(50));
                }
                Err(_) => return Err(()),
            }
        }
    }

    /// Starts wiremock returning 200 with a 30s delay and launches the binary
    /// pointing the HTML endpoint at that mock. Then sends SIGINT and
    /// validates that the process terminates within the timeout (cancellation occurred).
    ///
    /// NOTE: this test depends on timing (the signal handler must be installed
    /// before SIGINT arrives). 600ms warm-up is comfortable on most CIs,
    /// but on EXTREMELY saturated runners it may occasionally fail — increase `WARMUP_MS`.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn sigint_dispara_cancelamento_e_termina_processo() {
        use wiremock::matchers::method;
        use wiremock::{Mock, MockServer, ResponseTemplate};

        const WARMUP_MS: u64 = 600;
        const HARD_TIMEOUT_PROCESSO: Duration = Duration::from_secs(8);

        // 1. Mock HTTP that NEVER responds quickly — forces the request to stay
        // in-flight while we send SIGINT.
        let mock_server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(200).set_delay(Duration::from_secs(30)))
            .mount(&mock_server)
            .await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(200).set_delay(Duration::from_secs(30)))
            .mount(&mock_server)
            .await;

        // 2. Locate the compiled binary via assert_cmd.
        let bin_path = assert_cmd::cargo::cargo_bin(BIN_NAME);
        assert!(bin_path.exists(), "binário deve existir: {bin_path:?}");

        // 3. Spawn process via std::process::Command (we need the PID).
        let mut child = StdCommand::new(&bin_path)
            .arg("rust async")
            .arg("--global-timeout")
            .arg("60") // high enough to NOT be the reason for termination
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

        // 4. Warm-up: tokio must register the handler BEFORE SIGINT.
        std::thread::sleep(Duration::from_millis(WARMUP_MS));

        // Sanity: still running? If already terminated, the test is inconclusive.
        if let Some(status) = child.try_wait().expect("try_wait") {
            panic!(
                "processo terminou ANTES do SIGINT (status={:?}); teste \
                 inválido — possivelmente o mock não foi atingido",
                status.code()
            );
        }

        // 5. Send SIGINT.
        let kill_result = unsafe { kill(pid, SIGINT) };
        assert_eq!(
            kill_result, 0,
            "kill(pid={pid}, SIGINT) deve retornar 0, retornou {kill_result}"
        );

        // 6. Wait for termination within a reasonable time.
        let status = match wait_with_timeout(&mut child, HARD_TIMEOUT_PROCESSO) {
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

        // 7. Collect stderr for diagnosis on failure.
        let mut stderr_buf = String::new();
        if let Some(mut stderr) = child.stderr.take() {
            let _ = stderr.read_to_string(&mut stderr_buf);
        }

        // The process MUST have terminated due to our action (not with success 0).
        // Accept any exit code != 0 or termination by signal.
        // - If `tokio::signal::ctrl_c()` intercepted (normal path):
        //   `run()` returns with some error/cancel exit code.
        // - If SIGINT arrived BEFORE the handler was ready: process dies
        //   by signal and `code()` is None — also evidence of SIGINT.
        let code = status.code();
        assert!(
            code != Some(0),
            "processo terminou com SUCESSO (0) após SIGINT; esperado != 0. \
             stderr={stderr_buf:?}"
        );
    }
}
