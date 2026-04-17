//! CLI argument definitions via `clap` derive.
//!
//! This module contains ONLY declarative clap structs. ZERO business logic.
//! Conversion of `ArgumentosCli` into `Configuracoes` used by the pipeline occurs
//! in the `lib.rs` module (`run` function).
//!
//! In iteration 6 the `init-config` subcommand was added — backward-compatible,
//! since when no subcommand is passed, the previous search behavior is preserved
//! via `#[command(subcommand)]` with `Option<Subcomando>`.

use clap::{Args, Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

/// Hard upper bound for `--per-host-limit` (fetch-content).
pub const PER_HOST_LIMIT_PADRAO: u32 = 2;
pub const PER_HOST_LIMIT_MAXIMO: u32 = 10;

/// Hard upper bound for parallelism degree, per sections 4.2 and 17.4.
pub const PARALELISMO_MAXIMO: u32 = 20;

/// Default parallelism degree when the user does not specify `-p`.
pub const PARALELISMO_PADRAO: u32 = 5;

/// Hard upper bound for the number of pages (avoids expensive loops).
pub const PAGINAS_MAXIMO: u32 = 5;

/// Hard upper bound for retries (avoids infinite-429 hangs).
pub const RETRIES_MAXIMO: u32 = 10;

/// Hard upper bound for `--max-content-length` (100_000 chars — ~100KB of clean text).
pub const MAX_CONTENT_LENGTH_PADRAO: usize = 10_000;
pub const MAX_CONTENT_LENGTH_MAXIMO: usize = 100_000;

/// Min/max values for the `--global-timeout` flag in seconds.
pub const GLOBAL_TIMEOUT_PADRAO: u64 = 60;
pub const GLOBAL_TIMEOUT_MAXIMO: u64 = 3600;

/// Selectable DuckDuckGo endpoint via `--endpoint`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum EndpointCli {
    Html,
    Lite,
}

/// Time filter accepted by `--time-filter` (DDG `df` parameter).
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum FiltroTemporalCli {
    /// Last day.
    D,
    /// Last week.
    W,
    /// Last month.
    M,
    /// Last year.
    Y,
}

/// Safe-search accepted by `--safe-search` (DDG `kp` parameter).
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum SafeSearchCli {
    Off,
    Moderate,
    On,
}

/// CLI for searching DuckDuckGo via pure HTTP, with structured output for LLM consumption.
///
/// Root accepts an optional subcommand. When no subcommand is passed, the
/// default behavior is `buscar` — maintains full backward compatibility with
/// previous versions of the CLI.
#[derive(Debug, Clone, Parser)]
#[command(
    name = "duckduckgo-search-cli",
    version,
    about = "DuckDuckGo search via pure HTTP, JSON output for LLMs.",
    long_about = "Rust CLI that queries the static DuckDuckGo HTML endpoint \
                  (https://html.duckduckgo.com/html/) using pure HTTP requests, \
                  no Chrome, no paid APIs, and no cache. Returns structured organic \
                  results as JSON ready for LLM consumption.",
    after_long_help = "\
EXIT CODES:\n\
    0    Success — at least one query returned results\n\
    1    Runtime error (network, parse, I/O)\n\
    2    Invalid configuration (flag out of range, bad proxy)\n\
    3    DuckDuckGo 202 block anomaly (soft-rate-limit)\n\
    4    Global timeout exceeded\n\
    5    Zero results across all queries\n\
\n\
PIPE USAGE:\n\
    duckduckgo-search-cli -q -f json \"query\" | jaq '.resultados[].url'\n\
    Logs go to stderr (-q suppresses them). JSON goes to stdout."
)]
pub struct ArgumentosRaiz {
    /// Optional subcommand (`init-config`). No subcommand = search (default).
    #[command(subcommand)]
    pub subcomando: Option<Subcomando>,

    /// Search arguments (also accepted without a subcommand for backward compatibility).
    #[command(flatten)]
    pub buscar: ArgumentosCli,
}

/// Supported subcommands. Chosen architecture: `Option<Subcomando>` at the root
/// allows invocation without a subcommand (direct search) OR with an explicit subcommand.
///
/// `Buscar` is `Box`ed to avoid a large enum variant (ArgumentosCli has
/// many clap-derived fields).
#[derive(Debug, Clone, Subcommand)]
pub enum Subcomando {
    /// Search on DuckDuckGo (equivalent to the no-subcommand mode).
    Buscar(Box<ArgumentosCli>),
    /// Initializes configuration files (`selectors.toml`, `user-agents.toml`)
    /// in the default OS configuration directory.
    InitConfig(ArgumentosInitConfig),
}

/// Arguments specific to the `init-config` subcommand.
#[derive(Debug, Clone, Args)]
pub struct ArgumentosInitConfig {
    /// Overwrites existing files. Without this flag, files already present
    /// are kept intact.
    #[arg(long = "force")]
    pub forcar: bool,

    /// Simulates execution without writing any file to disk. Reports the actions
    /// that would be taken.
    #[arg(long = "dry-run")]
    pub dry_run: bool,
}

/// Search arguments (shared between the direct mode and the `buscar` subcommand).
#[derive(Debug, Clone, Args)]
pub struct ArgumentosCli {
    /// Search queries (free text). Accepts multiple space-separated values
    /// or via stdin (one per line) if none are passed here or via `--queries-file`.
    #[arg(value_name = "QUERY")]
    pub queries: Vec<String>,

    /// Maximum number of results to return per query (default: 15, with
    /// auto-pagination to 2 pages when `--pages` is not customized).
    /// If omitted, uses 15; if `--num > 10` and `--pages == 1` (default),
    /// `--pages` is auto-elevated to `ceil(num/10)` up to a maximum of 5.
    #[arg(short = 'n', long = "num", value_name = "N")]
    pub num_resultados: Option<u32>,

    /// Output format: `json`, `text`, `markdown` (`md`) or `auto`.
    /// `auto` uses `text` in a TTY and `json` in a pipe (and forces `json` when
    /// `--output` is provided).
    #[arg(
        short = 'f',
        long = "format",
        value_name = "FMT",
        default_value = "auto"
    )]
    pub formato: String,

    /// Writes output to the specified file instead of printing to stdout.
    /// Missing parent directories are created. On Unix, permissions 0o644 are applied.
    #[arg(short = 'o', long = "output", value_name = "PATH")]
    pub arquivo_saida: Option<PathBuf>,

    /// Per-query timeout in seconds (default: 15).
    #[arg(
        short = 't',
        long = "timeout",
        value_name = "SECS",
        default_value_t = 15
    )]
    pub timeout_segundos: u64,

    /// Language for DuckDuckGo's `kl` parameter (default: `pt`).
    #[arg(short = 'l', long = "lang", value_name = "LANG", default_value = "pt")]
    pub idioma: String,

    /// Country for DuckDuckGo's `kl` parameter (default: `br`).
    #[arg(short = 'c', long = "country", value_name = "CC", default_value = "br")]
    pub pais: String,

    /// Number of concurrent requests (default 5, maximum 20).
    #[arg(
        short = 'p',
        long = "parallel",
        value_name = "N",
        default_value_t = PARALELISMO_PADRAO
    )]
    pub paralelismo: u32,

    /// File containing additional queries (one per line). Empty lines are ignored.
    #[arg(long = "queries-file", value_name = "PATH")]
    pub arquivo_queries: Option<PathBuf>,

    /// Number of pages to fetch per query (1..=5). Default 1.
    #[arg(long = "pages", value_name = "N", default_value_t = 1)]
    pub paginas: u32,

    /// Number of additional retries on 429/403/timeout (0..=10). Default 2.
    #[arg(long = "retries", value_name = "N", default_value_t = 2)]
    pub retries: u32,

    /// Preferred endpoint: `html` (default) or `lite` (forces the no-JavaScript endpoint).
    #[arg(long = "endpoint", value_enum, default_value_t = EndpointCli::Html)]
    pub endpoint: EndpointCli,

    /// Time filter: `d` (day), `w` (week), `m` (month), `y` (year). Default: no filter.
    #[arg(long = "time-filter", value_enum)]
    pub filtro_temporal: Option<FiltroTemporalCli>,

    /// Safe-search: `off`, `moderate` (default) or `on`.
    #[arg(long = "safe-search", value_enum, default_value_t = SafeSearchCli::Moderate)]
    pub safe_search: SafeSearchCli,

    /// Placeholder — streams results as they complete. Not implemented in iteration 2.
    #[arg(long = "stream")]
    pub modo_stream: bool,

    /// Enables detailed logs on stderr (`tracing::debug` and `tracing::info`).
    #[arg(short = 'v', long = "verbose", conflicts_with = "silencioso")]
    pub verboso: bool,

    /// Suppresses all stderr logs, keeping only the main output on stdout.
    #[arg(short = 'q', long = "quiet", conflicts_with = "verboso")]
    pub silencioso: bool,

    /// Enables full text content extraction from each result URL (pure HTTP + readability).
    /// Makes one additional request per result, in parallel (limited by --parallel).
    #[arg(long = "fetch-content")]
    pub buscar_conteudo: bool,

    /// Maximum size (in characters) of the extracted content per page (1..=100_000).
    /// Only effective with `--fetch-content`. Default 10_000.
    #[arg(
        long = "max-content-length",
        value_name = "N",
        default_value_t = MAX_CONTENT_LENGTH_PADRAO
    )]
    pub max_tamanho_conteudo: usize,

    /// HTTP/HTTPS/SOCKS5 proxy URL (e.g., `http://user:pass@host:port`, `socks5://host:port`).
    /// Takes precedence over the HTTP_PROXY/HTTPS_PROXY/ALL_PROXY environment variables.
    #[arg(long = "proxy", value_name = "URL", conflicts_with = "sem_proxy")]
    pub proxy: Option<String>,

    /// Disables any proxy — ignores `--proxy` and the HTTP_PROXY/HTTPS_PROXY/ALL_PROXY env vars.
    #[arg(long = "no-proxy", conflicts_with = "proxy")]
    pub sem_proxy: bool,

    /// Global timeout for the entire execution in seconds (1..=3600). Default 60.
    /// Different from `--timeout`, which is per-request.
    #[arg(
        long = "global-timeout",
        value_name = "SECS",
        default_value_t = GLOBAL_TIMEOUT_PADRAO
    )]
    pub timeout_global_segundos: u64,

    /// Restricts UAs loaded from `user-agents.toml` to the current platform (linux/macos/windows).
    /// Only takes effect if the external TOML file is found; otherwise uses built-in defaults.
    #[arg(long = "match-platform-ua")]
    pub corresponde_plataforma_ua: bool,

    /// Concurrent fetch limit PER HOST in `--fetch-content` mode (1..=10, default 2).
    /// Protects hosts from bursts — complements the global `--parallel` with a per-host gate.
    #[arg(
        long = "per-host-limit",
        value_name = "N",
        default_value_t = PER_HOST_LIMIT_PADRAO
    )]
    pub limite_por_host: u32,

    /// Manual path to the Chrome/Chromium executable (`chrome` feature).
    /// Only useful with `--fetch-content` and the `chrome` feature compiled in;
    /// otherwise ignored with a stderr warning.
    #[arg(long = "chrome-path", value_name = "PATH")]
    pub caminho_chrome: Option<PathBuf>,
}

impl ArgumentosCli {
    /// Validates that the parallelism degree is within the range `[1, PARALELISMO_MAXIMO]`.
    pub fn validar_paralelismo(&self) -> Result<(), String> {
        if self.paralelismo == 0 {
            return Err(format!(
                "--parallel deve ser pelo menos 1 (recebido {})",
                self.paralelismo
            ));
        }
        if self.paralelismo > PARALELISMO_MAXIMO {
            return Err(format!(
                "--parallel não pode exceder {} (recebido {})",
                PARALELISMO_MAXIMO, self.paralelismo
            ));
        }
        Ok(())
    }

    /// Validates that the number of pages is within the range `[1, PAGINAS_MAXIMO]`.
    pub fn validar_paginas(&self) -> Result<(), String> {
        if self.paginas == 0 {
            return Err(format!(
                "--pages deve ser pelo menos 1 (recebido {})",
                self.paginas
            ));
        }
        if self.paginas > PAGINAS_MAXIMO {
            return Err(format!(
                "--pages não pode exceder {} (recebido {})",
                PAGINAS_MAXIMO, self.paginas
            ));
        }
        Ok(())
    }

    /// Validates that `--max-content-length` is within the range `[1, MAX_CONTENT_LENGTH_MAXIMO]`.
    pub fn validar_max_tamanho_conteudo(&self) -> Result<(), String> {
        if self.max_tamanho_conteudo == 0 {
            return Err(format!(
                "--max-content-length deve ser pelo menos 1 (recebido {})",
                self.max_tamanho_conteudo
            ));
        }
        if self.max_tamanho_conteudo > MAX_CONTENT_LENGTH_MAXIMO {
            return Err(format!(
                "--max-content-length não pode exceder {} (recebido {})",
                MAX_CONTENT_LENGTH_MAXIMO, self.max_tamanho_conteudo
            ));
        }
        Ok(())
    }

    /// Validates that `--global-timeout` is within the range `[1, GLOBAL_TIMEOUT_MAXIMO]`.
    pub fn validar_global_timeout(&self) -> Result<(), String> {
        if self.timeout_global_segundos == 0 {
            return Err(format!(
                "--global-timeout deve ser pelo menos 1 (recebido {})",
                self.timeout_global_segundos
            ));
        }
        if self.timeout_global_segundos > GLOBAL_TIMEOUT_MAXIMO {
            return Err(format!(
                "--global-timeout não pode exceder {} segundos (recebido {})",
                GLOBAL_TIMEOUT_MAXIMO, self.timeout_global_segundos
            ));
        }
        Ok(())
    }

    /// Validates that `--proxy`, when provided, is a parseable URL with a supported scheme.
    pub fn validar_proxy(&self) -> Result<(), String> {
        let Some(url) = self.proxy.as_deref() else {
            return Ok(());
        };
        let parseada = reqwest::Url::parse(url)
            .map_err(|e| format!("URL de --proxy inválida ({url:?}): {e}"))?;
        match parseada.scheme() {
            "http" | "https" | "socks5" | "socks5h" => Ok(()),
            outro => Err(format!(
                "scheme {outro:?} não suportado em --proxy (use http/https/socks5)"
            )),
        }
    }

    /// Validates that the number of retries is within the range `[0, RETRIES_MAXIMO]`.
    pub fn validar_retries(&self) -> Result<(), String> {
        if self.retries > RETRIES_MAXIMO {
            return Err(format!(
                "--retries não pode exceder {} (recebido {})",
                RETRIES_MAXIMO, self.retries
            ));
        }
        Ok(())
    }

    /// Validates that `--per-host-limit` is within the range `[1, PER_HOST_LIMIT_MAXIMO]`.
    pub fn validar_limite_por_host(&self) -> Result<(), String> {
        if self.limite_por_host == 0 {
            return Err(format!(
                "--per-host-limit deve ser pelo menos 1 (recebido {})",
                self.limite_por_host
            ));
        }
        if self.limite_por_host > PER_HOST_LIMIT_MAXIMO {
            return Err(format!(
                "--per-host-limit não pode exceder {} (recebido {})",
                PER_HOST_LIMIT_MAXIMO, self.limite_por_host
            ));
        }
        Ok(())
    }

    /// Validates that `--timeout` is at least 1 second.
    pub fn validar_timeout_segundos(&self) -> Result<(), String> {
        if self.timeout_segundos == 0 {
            return Err(format!(
                "--timeout deve ser pelo menos 1 (recebido {})",
                self.timeout_segundos
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use clap::CommandFactory;

    /// Helper: parseia argumentos via raiz e extrai `ArgumentosCli` (fluxo default = Buscar).
    /// Replicates the convenience behavior of tests prior to the introduction of the subcommand.
    fn parse_buscar(argv: &[&str]) -> Result<ArgumentosCli, clap::Error> {
        let raiz = ArgumentosRaiz::try_parse_from(argv)?;
        match raiz.subcomando {
            Some(Subcomando::Buscar(a)) => Ok(*a),
            Some(Subcomando::InitConfig(_)) => {
                // Em testes chamamos parse_buscar apenas com args de busca — se cair aqui,
                // é bug do teste.
                Err(clap::Error::raw(
                    clap::error::ErrorKind::InvalidSubcommand,
                    "subcomando init-config retornado em contexto que esperava busca",
                ))
            }
            None => Ok(raiz.buscar),
        }
    }

    #[test]
    fn cli_passa_validacao_de_schema() {
        // `debug_assert` do clap valida a struct em tempo de chamada.
        ArgumentosRaiz::command().debug_assert();
    }

    #[test]
    fn parseia_query_simples() {
        let argumentos = parse_buscar(&["bin", "rust async"]).expect("deve parsear");
        assert_eq!(argumentos.queries, vec!["rust async".to_string()]);
        // Default agora é "auto" (resolvido em runtime via TTY detection).
        assert_eq!(argumentos.formato, "auto");
        assert!(argumentos.arquivo_saida.is_none());
        assert_eq!(argumentos.timeout_segundos, 15);
        assert_eq!(argumentos.idioma, "pt");
        assert_eq!(argumentos.pais, "br");
        assert_eq!(argumentos.paralelismo, PARALELISMO_PADRAO);
        assert_eq!(argumentos.paginas, 1);
        assert_eq!(argumentos.retries, 2);
        assert_eq!(argumentos.endpoint, EndpointCli::Html);
        assert!(argumentos.filtro_temporal.is_none());
        assert_eq!(argumentos.safe_search, SafeSearchCli::Moderate);
        assert!(!argumentos.modo_stream);
        assert!(argumentos.arquivo_queries.is_none());
        assert!(!argumentos.verboso);
        assert!(!argumentos.silencioso);
        assert!(!argumentos.buscar_conteudo);
        assert_eq!(argumentos.max_tamanho_conteudo, MAX_CONTENT_LENGTH_PADRAO);
        assert!(argumentos.proxy.is_none());
        assert!(!argumentos.sem_proxy);
        assert_eq!(argumentos.timeout_global_segundos, GLOBAL_TIMEOUT_PADRAO);
        assert!(!argumentos.corresponde_plataforma_ua);
    }

    #[test]
    fn parseia_fetch_content_e_max_content_length() {
        let argumentos = parse_buscar(&[
            "bin",
            "--fetch-content",
            "--max-content-length",
            "500",
            "rust",
        ])
        .expect("deve parsear --fetch-content");
        assert!(argumentos.buscar_conteudo);
        assert_eq!(argumentos.max_tamanho_conteudo, 500);
    }

    #[test]
    fn parseia_proxy_e_no_proxy_mutuamente_exclusivos() {
        let ok = parse_buscar(&[
            "bin",
            "--proxy",
            "http://user:pass@proxy.local:8080",
            "rust",
        ])
        .expect("deve parsear --proxy");
        assert_eq!(
            ok.proxy.as_deref(),
            Some("http://user:pass@proxy.local:8080")
        );
        assert!(!ok.sem_proxy);

        let sem = parse_buscar(&["bin", "--no-proxy", "rust"]).expect("deve parsear --no-proxy");
        assert!(sem.sem_proxy);
        assert!(sem.proxy.is_none());

        let erro = parse_buscar(&["bin", "--proxy", "http://x", "--no-proxy", "rust"]);
        assert!(erro.is_err(), "--proxy + --no-proxy deve conflitar");
    }

    #[test]
    fn parseia_global_timeout() {
        let argumentos = parse_buscar(&["bin", "--global-timeout", "30", "rust"]).unwrap();
        assert_eq!(argumentos.timeout_global_segundos, 30);
    }

    #[test]
    fn validar_max_tamanho_conteudo_faixa() {
        let mut argumentos = parse_buscar(&["bin", "q"]).unwrap();
        argumentos.max_tamanho_conteudo = 0;
        assert!(argumentos.validar_max_tamanho_conteudo().is_err());
        argumentos.max_tamanho_conteudo = MAX_CONTENT_LENGTH_MAXIMO + 1;
        assert!(argumentos.validar_max_tamanho_conteudo().is_err());
        argumentos.max_tamanho_conteudo = 5000;
        assert!(argumentos.validar_max_tamanho_conteudo().is_ok());
    }

    #[test]
    fn validar_global_timeout_faixa() {
        let mut argumentos = parse_buscar(&["bin", "q"]).unwrap();
        argumentos.timeout_global_segundos = 0;
        assert!(argumentos.validar_global_timeout().is_err());
        argumentos.timeout_global_segundos = GLOBAL_TIMEOUT_MAXIMO + 1;
        assert!(argumentos.validar_global_timeout().is_err());
        argumentos.timeout_global_segundos = 120;
        assert!(argumentos.validar_global_timeout().is_ok());
    }

    #[test]
    fn validar_proxy_aceita_schemes_suportados() {
        let mut argumentos = parse_buscar(&["bin", "q"]).unwrap();
        for ok in [
            "http://proxy:8080",
            "https://user:pass@proxy:8443",
            "socks5://127.0.0.1:9050",
            "socks5h://host:1080",
        ] {
            argumentos.proxy = Some(ok.to_string());
            assert!(
                argumentos.validar_proxy().is_ok(),
                "proxy {ok:?} deveria ser aceito"
            );
        }
        argumentos.proxy = Some("ftp://proxy".to_string());
        assert!(argumentos.validar_proxy().is_err());
        argumentos.proxy = Some("nao-eh-uma-url".to_string());
        assert!(argumentos.validar_proxy().is_err());
        argumentos.proxy = None;
        assert!(argumentos.validar_proxy().is_ok());
    }

    #[test]
    fn parseia_flags_de_resiliencia_e_filtros() {
        let argumentos = parse_buscar(&[
            "bin",
            "--pages",
            "3",
            "--retries",
            "5",
            "--endpoint",
            "lite",
            "--time-filter",
            "w",
            "--safe-search",
            "on",
            "rust",
        ])
        .expect("deve parsear flags de resiliência");
        assert_eq!(argumentos.paginas, 3);
        assert_eq!(argumentos.retries, 5);
        assert_eq!(argumentos.endpoint, EndpointCli::Lite);
        assert_eq!(argumentos.filtro_temporal, Some(FiltroTemporalCli::W));
        assert_eq!(argumentos.safe_search, SafeSearchCli::On);
    }

    #[test]
    fn validar_paginas_aceita_faixa_e_rejeita_invalidos() {
        let mut argumentos = parse_buscar(&["bin", "qualquer"]).unwrap();
        for v in [1u32, 2, 5] {
            argumentos.paginas = v;
            assert!(argumentos.validar_paginas().is_ok(), "paginas {v}");
        }
        argumentos.paginas = 0;
        assert!(argumentos.validar_paginas().is_err());
        argumentos.paginas = 6;
        assert!(argumentos.validar_paginas().is_err());
    }

    #[test]
    fn validar_retries_rejeita_acima_do_maximo() {
        let mut argumentos = parse_buscar(&["bin", "qualquer"]).unwrap();
        argumentos.retries = 0;
        assert!(argumentos.validar_retries().is_ok());
        argumentos.retries = 10;
        assert!(argumentos.validar_retries().is_ok());
        argumentos.retries = 11;
        assert!(argumentos.validar_retries().is_err());
    }

    #[test]
    fn parseia_multiplas_queries_posicionais() {
        let argumentos = parse_buscar(&["bin", "rust async", "tokio runtime", "async channels"])
            .expect("deve parsear múltiplas queries");
        assert_eq!(
            argumentos.queries,
            vec![
                "rust async".to_string(),
                "tokio runtime".to_string(),
                "async channels".to_string(),
            ]
        );
    }

    #[test]
    fn parseia_flags_customizadas() {
        let argumentos = parse_buscar(&[
            "bin",
            "--num",
            "10",
            "--format",
            "json",
            "--timeout",
            "30",
            "--lang",
            "en",
            "--country",
            "us",
            "--parallel",
            "8",
            "--verbose",
            "teste de busca",
        ])
        .expect("deve parsear com flags");
        assert_eq!(argumentos.queries, vec!["teste de busca".to_string()]);
        assert_eq!(argumentos.num_resultados, Some(10));
        assert_eq!(argumentos.timeout_segundos, 30);
        assert_eq!(argumentos.idioma, "en");
        assert_eq!(argumentos.pais, "us");
        assert_eq!(argumentos.paralelismo, 8);
        assert!(argumentos.verboso);
    }

    #[test]
    fn parseia_flag_output_curta_e_longa() {
        let argumentos =
            parse_buscar(&["bin", "-o", "/tmp/saida.json", "q"]).expect("deve parsear -o");
        assert_eq!(
            argumentos.arquivo_saida.as_deref(),
            Some(std::path::Path::new("/tmp/saida.json"))
        );

        let argumentos2 =
            parse_buscar(&["bin", "--output", "/tmp/x.md", "--format", "markdown", "q"])
                .expect("deve parsear --output");
        assert_eq!(
            argumentos2.arquivo_saida.as_deref(),
            Some(std::path::Path::new("/tmp/x.md"))
        );
        assert_eq!(argumentos2.formato, "markdown");
    }

    #[test]
    fn parseia_arquivo_queries_e_stream() {
        let argumentos = parse_buscar(&["bin", "--queries-file", "queries.txt", "--stream"])
            .expect("deve parsear --queries-file e --stream");
        assert!(argumentos.modo_stream);
        assert_eq!(
            argumentos.arquivo_queries.as_deref(),
            Some(std::path::Path::new("queries.txt"))
        );
        assert!(argumentos.queries.is_empty());
    }

    #[test]
    fn verbose_e_quiet_sao_mutuamente_exclusivos() {
        let resultado = parse_buscar(&["bin", "--verbose", "--quiet", "query qualquer"]);
        assert!(
            resultado.is_err(),
            "verbose + quiet deve falhar a validação"
        );
    }

    #[test]
    fn validar_paralelismo_aceita_faixa_permitida() {
        let mut argumentos = parse_buscar(&["bin", "qualquer"]).unwrap();
        for valor in [1u32, 5, 10, PARALELISMO_MAXIMO] {
            argumentos.paralelismo = valor;
            assert!(
                argumentos.validar_paralelismo().is_ok(),
                "--parallel {valor} deveria ser aceito"
            );
        }
    }

    #[test]
    fn validar_paralelismo_rejeita_valores_invalidos() {
        let mut argumentos = parse_buscar(&["bin", "qualquer"]).unwrap();
        argumentos.paralelismo = 0;
        assert!(argumentos.validar_paralelismo().is_err());
        argumentos.paralelismo = PARALELISMO_MAXIMO + 1;
        assert!(argumentos.validar_paralelismo().is_err());
        argumentos.paralelismo = 100;
        assert!(argumentos.validar_paralelismo().is_err());
    }

    #[test]
    fn parseia_subcomando_init_config_com_flags() {
        let raiz = ArgumentosRaiz::try_parse_from(["bin", "init-config", "--force", "--dry-run"])
            .expect("deve parsear init-config");
        let Some(Subcomando::InitConfig(args)) = raiz.subcomando else {
            panic!("esperava subcomando InitConfig");
        };
        assert!(args.forcar);
        assert!(args.dry_run);
    }

    #[test]
    fn parseia_subcomando_init_config_sem_flags() {
        let raiz = ArgumentosRaiz::try_parse_from(["bin", "init-config"])
            .expect("deve parsear init-config sem flags");
        let Some(Subcomando::InitConfig(args)) = raiz.subcomando else {
            panic!("esperava subcomando InitConfig");
        };
        assert!(!args.forcar);
        assert!(!args.dry_run);
    }

    #[test]
    fn parseia_subcomando_buscar_explicito() {
        let raiz = ArgumentosRaiz::try_parse_from(["bin", "buscar", "rust"])
            .expect("deve parsear subcomando buscar");
        let Some(Subcomando::Buscar(args)) = raiz.subcomando else {
            panic!("esperava subcomando Buscar");
        };
        assert_eq!(args.queries, vec!["rust".to_string()]);
    }

    #[test]
    fn subcomando_buscar_continua_pequeno_quando_boxed() {
        // Garantia de regressão: Subcomando::Buscar ainda é Box — clippy lint large_enum.
        let tamanho_enum = std::mem::size_of::<Subcomando>();
        let tamanho_init = std::mem::size_of::<ArgumentosInitConfig>();
        // Enum ≤ max(variant + discriminant) — Buscar é Box (ptr size).
        assert!(
            tamanho_enum <= tamanho_init.max(std::mem::size_of::<usize>()) * 4,
            "Subcomando cresceu inesperadamente: {tamanho_enum} bytes"
        );
    }

    #[test]
    fn parse_sem_subcomando_usa_buscar_flatten() {
        let raiz = ArgumentosRaiz::try_parse_from(["bin", "rust async"])
            .expect("deve parsear sem subcomando");
        assert!(raiz.subcomando.is_none());
        assert_eq!(raiz.buscar.queries, vec!["rust async".to_string()]);
    }

    #[test]
    fn parseia_per_host_limit() {
        let argumentos = parse_buscar(&["bin", "--per-host-limit", "5", "q"]).unwrap();
        assert_eq!(argumentos.limite_por_host, 5);
        let default = parse_buscar(&["bin", "q"]).unwrap();
        assert_eq!(default.limite_por_host, PER_HOST_LIMIT_PADRAO);
    }

    #[test]
    fn validar_limite_por_host_faixa() {
        let mut argumentos = parse_buscar(&["bin", "q"]).unwrap();
        argumentos.limite_por_host = 0;
        assert!(argumentos.validar_limite_por_host().is_err());
        argumentos.limite_por_host = PER_HOST_LIMIT_MAXIMO + 1;
        assert!(argumentos.validar_limite_por_host().is_err());
        argumentos.limite_por_host = 2;
        assert!(argumentos.validar_limite_por_host().is_ok());
    }

    #[test]
    fn validar_timeout_segundos_rejeita_zero() {
        let mut argumentos = parse_buscar(&["bin", "q"]).unwrap();
        argumentos.timeout_segundos = 0;
        assert!(argumentos.validar_timeout_segundos().is_err());
        argumentos.timeout_segundos = 1;
        assert!(argumentos.validar_timeout_segundos().is_ok());
        argumentos.timeout_segundos = 15;
        assert!(argumentos.validar_timeout_segundos().is_ok());
    }
}
