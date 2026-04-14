//! Definições de argumentos da CLI via `clap` derive.
//!
//! Este módulo contém APENAS as structs declarativas do clap. ZERO lógica de negócio.
//! A conversão de `ArgumentosCli` para `Configuracoes` usadas pelo pipeline ocorre
//! no módulo `lib.rs` (função `run`).
//!
//! Na iteração 6 foi adicionado o subcomando `init-config` — backward-compatible,
//! pois quando nenhum subcomando é passado, o comportamento de busca (anterior)
//! é preservado via `#[command(subcommand)]` com `Option<Subcomando>`.

use clap::{Args, Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

/// Limite superior rígido para `--per-host-limit` (fetch-content).
pub const PER_HOST_LIMIT_PADRAO: u32 = 2;
pub const PER_HOST_LIMIT_MAXIMO: u32 = 10;

/// Limite superior rígido para o grau de paralelismo, conforme seção 4.2 e 17.4.
pub const PARALELISMO_MAXIMO: u32 = 20;

/// Grau de paralelismo padrão quando o usuário não especifica `-p`.
pub const PARALELISMO_PADRAO: u32 = 5;

/// Limite superior rígido para o número de páginas (evita loops caros).
pub const PAGINAS_MAXIMO: u32 = 5;

/// Limite superior rígido para retries (evita travamentos em 429 infinito).
pub const RETRIES_MAXIMO: u32 = 10;

/// Limite superior rígido para `--max-content-length` (100_000 chars — ~100KB de texto limpo).
pub const MAX_CONTENT_LENGTH_PADRAO: usize = 10_000;
pub const MAX_CONTENT_LENGTH_MAXIMO: usize = 100_000;

/// Valores mín/máx da flag `--global-timeout` em segundos.
pub const GLOBAL_TIMEOUT_PADRAO: u64 = 60;
pub const GLOBAL_TIMEOUT_MAXIMO: u64 = 3600;

/// Endpoint do DuckDuckGo selecionável via `--endpoint`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum EndpointCli {
    Html,
    Lite,
}

/// Filtro temporal aceito em `--time-filter` (parâmetro `df` do DDG).
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum FiltroTemporalCli {
    /// Último dia.
    D,
    /// Última semana.
    W,
    /// Último mês.
    M,
    /// Último ano.
    Y,
}

/// Safe-search aceito em `--safe-search` (parâmetro `kp` do DDG).
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum SafeSearchCli {
    Off,
    Moderate,
    On,
}

/// CLI para pesquisa no DuckDuckGo via HTTP puro, com saída estruturada para consumo por LLM.
///
/// Raiz aceita subcomando opcional. Quando nenhum subcomando é passado, o
/// comportamento default é `buscar` — mantém total compatibilidade com versões
/// anteriores da CLI.
#[derive(Debug, Clone, Parser)]
#[command(
    name = "duckduckgo-search-cli",
    version,
    about = "Pesquisa no DuckDuckGo via HTTP puro, saída JSON para LLMs.",
    long_about = "CLI em Rust que consulta o endpoint HTML estático do DuckDuckGo \
                  (https://html.duckduckgo.com/html/) usando requests HTTP puros, \
                  sem Chrome, sem APIs pagas e sem cache. Retorna resultados \
                  orgânicos estruturados em JSON prontos para consumo por LLMs."
)]
pub struct ArgumentosRaiz {
    /// Subcomando opcional (`init-config`). Sem subcomando = buscar (default).
    #[command(subcommand)]
    pub subcomando: Option<Subcomando>,

    /// Argumentos de busca (também aceitos sem subcomando para retrocompatibilidade).
    #[command(flatten)]
    pub buscar: ArgumentosCli,
}

/// Subcomandos suportados. Arquitetura escolhida: `Option<Subcomando>` na raiz
/// permite invocar sem subcomando (busca direta) OU com subcomando explícito.
///
/// `Buscar` é `Box`ado para evitar variante gigante na enum (ArgumentosCli tem
/// muitos campos derivados do clap).
#[derive(Debug, Clone, Subcommand)]
pub enum Subcomando {
    /// Busca no DuckDuckGo (equivalente ao modo sem subcomando).
    Buscar(Box<ArgumentosCli>),
    /// Inicializa arquivos de configuração (`selectors.toml`, `user-agents.toml`)
    /// no diretório padrão do sistema operacional.
    InitConfig(ArgumentosInitConfig),
}

/// Argumentos específicos do subcomando `init-config`.
#[derive(Debug, Clone, Args)]
pub struct ArgumentosInitConfig {
    /// Sobrescreve arquivos existentes. Sem esta flag, arquivos já presentes
    /// são preservados intactos.
    #[arg(long = "force")]
    pub forcar: bool,

    /// Simula a execução sem gravar nenhum arquivo no disco. Reporta as ações
    /// que seriam tomadas.
    #[arg(long = "dry-run")]
    pub dry_run: bool,
}

/// Argumentos de busca (compartilhados entre o modo direto e o subcomando `buscar`).
#[derive(Debug, Clone, Args)]
pub struct ArgumentosCli {
    /// Queries de busca (texto livre). Aceita múltiplos valores separados por espaço
    /// ou via stdin (uma por linha) se nenhum for passado aqui nem via `--queries-file`.
    #[arg(value_name = "QUERY")]
    pub queries: Vec<String>,

    /// Número máximo de resultados a retornar por query (default: todos da primeira página).
    #[arg(short = 'n', long = "num", value_name = "N")]
    pub num_resultados: Option<u32>,

    /// Formato de saída: `json`, `text`, `markdown` (`md`) ou `auto`.
    /// `auto` usa `text` em TTY e `json` em pipe (e força `json` quando
    /// `--output` é fornecido).
    #[arg(
        short = 'f',
        long = "format",
        value_name = "FMT",
        default_value = "auto"
    )]
    pub formato: String,

    /// Grava a saída no arquivo informado em vez de imprimir em stdout.
    /// Diretórios pai inexistentes são criados. No Unix aplica permissões 0o644.
    #[arg(short = 'o', long = "output", value_name = "PATH")]
    pub arquivo_saida: Option<PathBuf>,

    /// Timeout per-query em segundos (default: 15).
    #[arg(
        short = 't',
        long = "timeout",
        value_name = "SECS",
        default_value_t = 15
    )]
    pub timeout_segundos: u64,

    /// Idioma do parâmetro `kl` do DuckDuckGo (default: `pt`).
    #[arg(short = 'l', long = "lang", value_name = "LANG", default_value = "pt")]
    pub idioma: String,

    /// País do parâmetro `kl` do DuckDuckGo (default: `br`).
    #[arg(short = 'c', long = "country", value_name = "CC", default_value = "br")]
    pub pais: String,

    /// Número de requests simultâneos (default 5, máximo 20).
    #[arg(
        short = 'p',
        long = "parallel",
        value_name = "N",
        default_value_t = PARALELISMO_PADRAO
    )]
    pub paralelismo: u32,

    /// Arquivo contendo queries adicionais (uma por linha). Linhas vazias são ignoradas.
    #[arg(long = "queries-file", value_name = "PATH")]
    pub arquivo_queries: Option<PathBuf>,

    /// Número de páginas a buscar por query (1..=5). Default 1.
    #[arg(long = "pages", value_name = "N", default_value_t = 1)]
    pub paginas: u32,

    /// Número de retries adicionais em caso de 429/403/timeout (0..=10). Default 2.
    #[arg(long = "retries", value_name = "N", default_value_t = 2)]
    pub retries: u32,

    /// Endpoint preferido: `html` (default) ou `lite` (força o endpoint sem JavaScript).
    #[arg(long = "endpoint", value_enum, default_value_t = EndpointCli::Html)]
    pub endpoint: EndpointCli,

    /// Filtro temporal: `d` (dia), `w` (semana), `m` (mês), `y` (ano). Default sem filtro.
    #[arg(long = "time-filter", value_enum)]
    pub filtro_temporal: Option<FiltroTemporalCli>,

    /// Safe-search: `off`, `moderate` (default) ou `on`.
    #[arg(long = "safe-search", value_enum, default_value_t = SafeSearchCli::Moderate)]
    pub safe_search: SafeSearchCli,

    /// Placeholder — emite resultados conforme completam. Não implementado na iteração 2.
    #[arg(long = "stream")]
    pub modo_stream: bool,

    /// Habilita logs detalhados em stderr (`tracing::debug` e `tracing::info`).
    #[arg(short = 'v', long = "verbose", conflicts_with = "silencioso")]
    pub verboso: bool,

    /// Suprime todos os logs em stderr, mantendo apenas o output principal em stdout.
    #[arg(short = 'q', long = "quiet", conflicts_with = "verboso")]
    pub silencioso: bool,

    /// Ativa extração de conteúdo textual completo de cada URL (HTTP puro + readability).
    /// Faz um request adicional por resultado, em paralelo (limitado por --parallel).
    #[arg(long = "fetch-content")]
    pub buscar_conteudo: bool,

    /// Tamanho máximo (em caracteres) do conteúdo extraído por página (1..=100_000).
    /// Efeito apenas com `--fetch-content`. Default 10_000.
    #[arg(
        long = "max-content-length",
        value_name = "N",
        default_value_t = MAX_CONTENT_LENGTH_PADRAO
    )]
    pub max_tamanho_conteudo: usize,

    /// URL de proxy HTTP/HTTPS/SOCKS5 (ex: `http://user:pass@host:port`, `socks5://host:port`).
    /// Tem precedência sobre as variáveis de ambiente HTTP_PROXY/HTTPS_PROXY/ALL_PROXY.
    #[arg(long = "proxy", value_name = "URL", conflicts_with = "sem_proxy")]
    pub proxy: Option<String>,

    /// Desabilita qualquer proxy — ignora `--proxy` e env vars HTTP_PROXY/HTTPS_PROXY/ALL_PROXY.
    #[arg(long = "no-proxy", conflicts_with = "proxy")]
    pub sem_proxy: bool,

    /// Timeout global da execução inteira em segundos (1..=3600). Default 60.
    /// Diferente de `--timeout`, que é per-request.
    #[arg(
        long = "global-timeout",
        value_name = "SECS",
        default_value_t = GLOBAL_TIMEOUT_PADRAO
    )]
    pub timeout_global_segundos: u64,

    /// Restringe os UAs carregados de `user-agents.toml` à plataforma atual (linux/macos/windows).
    /// Só tem efeito se o arquivo TOML externo for encontrado; senão usa defaults embutidos.
    #[arg(long = "match-platform-ua")]
    pub corresponde_plataforma_ua: bool,

    /// Limite de fetches simultâneos POR HOST em `--fetch-content` (1..=10, default 2).
    /// Protege hosts de burst — complementa o `--parallel` global com gate per-host.
    #[arg(
        long = "per-host-limit",
        value_name = "N",
        default_value_t = PER_HOST_LIMIT_PADRAO
    )]
    pub limite_por_host: u32,

    /// Caminho manual para o executável Chrome/Chromium (feature `chrome`).
    /// Apenas útil com `--fetch-content` e feature `chrome` compilada;
    /// senão é ignorada com warning em stderr.
    #[arg(long = "chrome-path", value_name = "PATH")]
    pub caminho_chrome: Option<PathBuf>,
}

impl ArgumentosCli {
    /// Valida o grau de paralelismo está dentro do intervalo `[1, PARALELISMO_MAXIMO]`.
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

    /// Valida número de páginas está no intervalo `[1, PAGINAS_MAXIMO]`.
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

    /// Valida `--max-content-length` está no intervalo `[1, MAX_CONTENT_LENGTH_MAXIMO]`.
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

    /// Valida `--global-timeout` está no intervalo `[1, GLOBAL_TIMEOUT_MAXIMO]`.
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

    /// Valida que `--proxy`, quando informado, é uma URL parseável com scheme suportado.
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

    /// Valida número de retries está no intervalo `[0, RETRIES_MAXIMO]`.
    pub fn validar_retries(&self) -> Result<(), String> {
        if self.retries > RETRIES_MAXIMO {
            return Err(format!(
                "--retries não pode exceder {} (recebido {})",
                RETRIES_MAXIMO, self.retries
            ));
        }
        Ok(())
    }

    /// Valida `--per-host-limit` está no intervalo `[1, PER_HOST_LIMIT_MAXIMO]`.
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
}

#[cfg(test)]
mod testes {
    use super::*;
    use clap::CommandFactory;

    /// Helper: parseia argumentos via raiz e extrai `ArgumentosCli` (fluxo default = Buscar).
    /// Replica o comportamento de conveniência dos testes anteriores à introdução do subcomando.
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
}
