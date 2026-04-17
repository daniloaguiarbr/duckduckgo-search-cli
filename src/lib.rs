//! # duckduckgo-search-cli
//!
//! CLI em Rust para pesquisa no DuckDuckGo via HTTP puro, com output estruturado
//! em JSON para consumo por LLMs. Sem API paga. Sem Chrome (na fase de busca).
//! Sem cache. Cross-platform universal (Linux incluindo Alpine/NixOS/Flatpak/Snap,
//! macOS incluindo Apple Silicon, Windows incluindo cmd.exe e PowerShell).
//!
//! ## Estrutura de Módulos
//!
//! | Módulo        | Responsabilidade                                             |
//! |---------------|--------------------------------------------------------------|
//! | [`cli`]       | Structs clap (parsing de argumentos da linha de comando).    |
//! | [`http`]      | Construção do `reqwest::Client` e seleção de User-Agent.     |
//! | [`search`]    | URL e request HTTP ao endpoint do DuckDuckGo.                |
//! | [`extraction`]| Parsing HTML com `scraper` e filtragem de anúncios.          |
//! | [`pipeline`]  | Orquestração single/multi, deduplicação e leitura de fontes. |
//! | [`parallel`]  | Fan-out multi-query com JoinSet, Semaphore, CancellationToken.|
//! | [`output`]    | Serialização JSON e escrita em stdout (ÚNICO com `println!`).|
//! | [`platform`]  | Inicialização cross-platform (UTF-8 no Windows, TTY detect). |
//! | [`types`]     | Structs e enums compartilhados.                              |
//! | [`error`]     | Códigos de erro e exit codes.                                |
//! | [`content`]   | Extração HTTP + readability para `--fetch-content` (iter. 5).|
//! | [`fetch_conteudo`] | Fan-out paralelo + rate-limit per-host (iter. 5 / 6).  |
//! | [`selectors`] | Carregamento de `ConfiguracaoSeletores` externa (iter. 6).  |
//! | [`signals`]   | Handlers de sinais cross-platform (SIGPIPE, Ctrl+C).         |
//! | [`config_init`] | Subcomando `init-config` (iter. 6).                       |
//! | [`paths`]     | Validação e sanitização de paths para I/O.                   |
//! | `browser`     | Chrome headless cross-platform sob feature `chrome` (iter.7).|
//!
//! ## Ponto de Entrada
//!
//! A função pública [`run`] é chamada por `main.rs` e retorna um exit code
//! conforme seção 17.7 da especificação.

pub mod cli;
pub mod config_init;
pub mod content;
pub mod error;
pub mod extraction;
pub mod fetch_conteudo;
pub mod http;
pub mod output;
pub mod parallel;
pub mod paths;
pub mod pipeline;
pub mod platform;
pub mod search;
pub mod selectors;
pub mod signals;
pub mod types;

// browser.rs só compila com a feature `chrome` (zero overhead no MVP).
#[cfg(feature = "chrome")]
pub mod browser;

use crate::cli::{
    ArgumentosCli, ArgumentosInitConfig, ArgumentosRaiz, EndpointCli, FiltroTemporalCli,
    SafeSearchCli, Subcomando,
};
use crate::error::exit_codes;
use crate::types::{Configuracoes, Endpoint, FiltroTemporal, FormatoSaida, SafeSearch};
use anyhow::{Context, Result};
use clap::Parser;
use tokio_util::sync::CancellationToken;
use tracing_subscriber::{fmt, EnvFilter};

/// Ponto de entrada da biblioteca. Chamado por `main.rs`.
///
/// Retorna o exit code apropriado (0 sucesso, 1 erro genérico, 2 config inválida, etc.).
pub async fn run(cancelamento: CancellationToken) -> i32 {
    // Parse da linha de comando — clap termina o processo com código 2 em caso de erro.
    let raiz = ArgumentosRaiz::parse();

    // Despacha subcomando (ou cai no default = Buscar).
    let argumentos = match raiz.subcomando {
        Some(Subcomando::InitConfig(args)) => {
            return executar_init_config(args);
        }
        Some(Subcomando::Buscar(args)) => *args,
        None => raiz.buscar,
    };

    // Inicializa logging em stderr (antes de qualquer operação que possa emitir logs).
    inicializar_logging(argumentos.verboso, argumentos.silencioso);

    // Inicializa plataforma (UTF-8 no Windows, etc.).
    platform::iniciar();

    // Converte ArgumentosCli em Configuracoes internas.
    let configuracoes = match montar_configuracoes(&argumentos) {
        Ok(c) => c,
        Err(erro) => {
            tracing::error!(?erro, "Configuração inválida");
            eprintln!("Erro de configuração: {erro:#}");
            return exit_codes::CONFIGURACAO_INVALIDA;
        }
    };

    let formato = configuracoes.formato;
    let arquivo_saida = configuracoes.arquivo_saida.clone();
    let timeout_global = std::time::Duration::from_secs(configuracoes.timeout_global_segundos);

    // Envolve o pipeline em `tokio::time::timeout` — se expirar, cancela tudo
    // e retorna exit code 4 (TIMEOUT_GLOBAL).
    let cancelamento_interno = cancelamento.clone();
    let futuro_pipeline = pipeline::executar_pipeline(configuracoes, cancelamento_interno);

    let resultado_pipeline = match tokio::time::timeout(timeout_global, futuro_pipeline).await {
        Ok(resultado) => resultado,
        Err(_elapsed) => {
            // Propaga cancelamento para qualquer task que ainda esteja em voo.
            cancelamento.cancel();
            tracing::error!(
                segundos = timeout_global.as_secs(),
                "timeout global excedido — execução abortada"
            );
            eprintln!(
                "Erro: timeout global de {}s excedido",
                timeout_global.as_secs()
            );
            return exit_codes::TIMEOUT_GLOBAL;
        }
    };

    match resultado_pipeline {
        Ok(resultado) => {
            let total = resultado.total_resultados();
            let codigo_saida = if total == 0 {
                tracing::warn!("Zero resultados retornados em todas as queries");
                exit_codes::ZERO_RESULTADOS
            } else {
                exit_codes::SUCESSO
            };

            if let Err(erro) =
                output::emitir_resultado(&resultado, formato, arquivo_saida.as_deref())
            {
                if output::eh_broken_pipe(&erro) {
                    // Pipe fechado pelo consumidor (ex: `| jaq`, `| head`).
                    // Comportamento Unix padrão — exit 0 silenciosamente.
                    return exit_codes::SUCESSO;
                }
                tracing::error!(?erro, "Falha ao emitir resultado");
                eprintln!("Erro ao escrever output: {erro:#}");
                return exit_codes::ERRO_GENERICO;
            }

            codigo_saida
        }
        Err(erro) => {
            tracing::error!(?erro, "Falha na execução do pipeline");
            eprintln!("Erro: {erro:#}");
            exit_codes::ERRO_GENERICO
        }
    }
}

/// Executa o subcomando `init-config` e imprime o relatório em formato JSON.
///
/// Retorna `SUCESSO` se todos os arquivos foram processados (inclusive ignorados);
/// retorna `ERRO_GENERICO` se falha fatal (ex: diretório de config indeterminado).
fn executar_init_config(args: ArgumentosInitConfig) -> i32 {
    // Inicializa logging mínimo (info) para o relatório.
    inicializar_logging(false, false);
    platform::iniciar();

    let relatorio = match config_init::inicializar_config(args.forcar, args.dry_run) {
        Ok(r) => r,
        Err(erro) => {
            tracing::error!(?erro, "falha ao inicializar config");
            eprintln!("Erro: {erro:#}");
            return exit_codes::ERRO_GENERICO;
        }
    };

    match serde_json::to_string_pretty(&relatorio) {
        Ok(json) => {
            if let Err(erro) = output::imprimir_linha_stdout(&json) {
                if output::eh_broken_pipe(&erro) {
                    return exit_codes::SUCESSO;
                }
                tracing::error!(?erro, "falha ao emitir relatório");
                return exit_codes::ERRO_GENERICO;
            }
        }
        Err(erro) => {
            tracing::error!(?erro, "falha ao serializar relatório JSON");
            return exit_codes::ERRO_GENERICO;
        }
    }

    // Houve erro em algum arquivo individual? Retornar erro genérico mesmo assim.
    let houve_erro = relatorio
        .arquivos
        .iter()
        .any(|a| matches!(a.acao, crate::config_init::AcaoArquivoConfig::Erro { .. }));
    if houve_erro {
        return exit_codes::ERRO_GENERICO;
    }

    exit_codes::SUCESSO
}

/// Inicializa o subscriber de tracing escrevendo em stderr.
///
/// - `--quiet` → apenas `ERROR`.
/// - `--verbose` → `DEBUG` e acima.
/// - Default → `INFO` e acima (mas respeita `RUST_LOG` se definido).
fn inicializar_logging(verboso: bool, silencioso: bool) {
    let filtro = if silencioso {
        EnvFilter::new("error")
    } else if verboso {
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("debug"))
    } else {
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"))
    };

    // Escrevemos em stderr para NÃO contaminar o output JSON em stdout.
    let subscriber = fmt()
        .with_env_filter(filtro)
        .with_writer(std::io::stderr)
        .with_target(false)
        .compact()
        .finish();

    // try_init permite que testes instalem seu próprio subscriber sem conflito.
    let _ = tracing::subscriber::set_global_default(subscriber);
}

/// Converte argumentos brutos da CLI em `Configuracoes` com validação.
///
/// Combina queries vindas de: (1) argumentos posicionais, (2) arquivo em
/// `--queries-file`, (3) stdin quando este não é TTY. Deduplica preservando
/// a ordem da primeira ocorrência.
fn montar_configuracoes(argumentos: &ArgumentosCli) -> Result<Configuracoes> {
    let formato = FormatoSaida::a_partir_de_str(&argumentos.formato)
        .with_context(|| format!("formato desconhecido: {:?}", argumentos.formato))?;

    argumentos
        .validar_paralelismo()
        .map_err(|e| anyhow::anyhow!(e))?;
    argumentos
        .validar_paginas()
        .map_err(|e| anyhow::anyhow!(e))?;
    argumentos
        .validar_retries()
        .map_err(|e| anyhow::anyhow!(e))?;
    argumentos
        .validar_max_tamanho_conteudo()
        .map_err(|e| anyhow::anyhow!(e))?;
    argumentos
        .validar_global_timeout()
        .map_err(|e| anyhow::anyhow!(e))?;
    argumentos.validar_proxy().map_err(|e| anyhow::anyhow!(e))?;
    argumentos
        .validar_limite_por_host()
        .map_err(|e| anyhow::anyhow!(e))?;
    argumentos
        .validar_timeout_segundos()
        .map_err(|e| anyhow::anyhow!(e))?;
    if let Some(caminho) = &argumentos.arquivo_saida {
        crate::paths::validar_caminho_saida(caminho)?;
    }

    let queries_arquivo = match &argumentos.arquivo_queries {
        Some(caminho) => pipeline::ler_queries_de_arquivo(caminho)
            .with_context(|| format!("falha ao processar --queries-file {}", caminho.display()))?,
        None => Vec::new(),
    };

    // Lê stdin apenas se nenhum argumento posicional foi fornecido E nenhum
    // arquivo foi passado. Isso reproduz o comportamento Unix clássico.
    let queries_stdin = if argumentos.queries.is_empty() && argumentos.arquivo_queries.is_none() {
        pipeline::ler_queries_de_stdin_se_pipe().context("falha ao ler queries de stdin")?
    } else {
        Vec::new()
    };

    let queries = pipeline::combinar_e_deduplicar_queries(
        argumentos.queries.clone(),
        queries_arquivo,
        queries_stdin,
    );

    if queries.is_empty() {
        anyhow::bail!(
            "nenhuma query fornecida (argumentos posicionais, --queries-file ou stdin vazios)"
        );
    }

    let primeira = queries[0].clone();

    // Carrega lista de UAs — tenta arquivo externo, cai em defaults embutidos.
    let lista_uas = http::carregar_user_agents(argumentos.corresponde_plataforma_ua);
    let perfil_browser = http::escolher_perfil_da_lista(&lista_uas);
    let user_agent = perfil_browser.user_agent.clone();

    // Carrega seletores CSS — tenta arquivo TOML externo, cai em defaults embutidos.
    let seletores = selectors::carregar_seletores();

    // --- Default de --num e auto-paginação (v0.4.0) ---
    //
    // Semântica (decidida em v0.4.0):
    // - Se o usuário NÃO passa `--num`, usamos 15 como default efetivo.
    // - Se o `num` efetivo for > 10 e o usuário NÃO customizou `--pages`
    //   (ou seja, `paginas == 1`, que é o default do clap), auto-elevamos
    //   `paginas` para `ceil(num/10)`, limitado ao teto de 5 (PAGINAS_MAXIMO
    //   validado em `validar_paginas`).
    // - Se o usuário passa `--pages > 1` explicitamente, RESPEITAMOS o valor
    //   dele sem sobrescrever (caso raro: `--pages 1` explícito é
    //   indistinguível do default; trade-off aceito).
    let num_efetivo = argumentos.num_resultados.unwrap_or(15);
    let paginas_efetivas = if argumentos.paginas > 1 {
        argumentos.paginas
    } else if num_efetivo > 10 {
        num_efetivo.div_ceil(10).min(5)
    } else {
        1
    };

    Ok(Configuracoes {
        query: primeira,
        queries,
        num_resultados: Some(num_efetivo),
        formato,
        timeout_segundos: argumentos.timeout_segundos,
        idioma: argumentos.idioma.clone(),
        pais: argumentos.pais.clone(),
        modo_verboso: argumentos.verboso,
        modo_silencioso: argumentos.silencioso,
        user_agent,
        perfil_browser,
        paralelismo: argumentos.paralelismo,
        paginas: paginas_efetivas,
        retries: argumentos.retries,
        endpoint: converter_endpoint(argumentos.endpoint),
        filtro_temporal: argumentos.filtro_temporal.map(converter_filtro_temporal),
        safe_search: converter_safe_search(argumentos.safe_search),
        modo_stream: argumentos.modo_stream,
        arquivo_saida: argumentos.arquivo_saida.clone(),
        buscar_conteudo: argumentos.buscar_conteudo,
        max_tamanho_conteudo: argumentos.max_tamanho_conteudo,
        proxy: argumentos.proxy.clone(),
        sem_proxy: argumentos.sem_proxy,
        timeout_global_segundos: argumentos.timeout_global_segundos,
        corresponde_plataforma_ua: argumentos.corresponde_plataforma_ua,
        limite_por_host: argumentos.limite_por_host as usize,
        caminho_chrome: argumentos.caminho_chrome.clone(),
        seletores,
    })
}

/// Converte o enum `EndpointCli` (clap) em `Endpoint` (tipo interno).
fn converter_endpoint(origem: EndpointCli) -> Endpoint {
    match origem {
        EndpointCli::Html => Endpoint::Html,
        EndpointCli::Lite => Endpoint::Lite,
    }
}

/// Converte o enum `FiltroTemporalCli` (clap) em `FiltroTemporal` (tipo interno).
fn converter_filtro_temporal(origem: FiltroTemporalCli) -> FiltroTemporal {
    match origem {
        FiltroTemporalCli::D => FiltroTemporal::Dia,
        FiltroTemporalCli::W => FiltroTemporal::Semana,
        FiltroTemporalCli::M => FiltroTemporal::Mes,
        FiltroTemporalCli::Y => FiltroTemporal::Ano,
    }
}

/// Converte o enum `SafeSearchCli` (clap) em `SafeSearch` (tipo interno).
fn converter_safe_search(origem: SafeSearchCli) -> SafeSearch {
    match origem {
        SafeSearchCli::Off => SafeSearch::Off,
        SafeSearchCli::Moderate => SafeSearch::Moderate,
        SafeSearchCli::On => SafeSearch::Strict,
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    fn argumentos_base() -> ArgumentosCli {
        ArgumentosCli {
            queries: vec!["rust async".to_string()],
            num_resultados: Some(5),
            formato: "json".to_string(),
            arquivo_saida: None,
            timeout_segundos: 15,
            idioma: "pt".to_string(),
            pais: "br".to_string(),
            paralelismo: 5,
            arquivo_queries: None,
            paginas: 1,
            retries: 2,
            endpoint: EndpointCli::Html,
            filtro_temporal: None,
            safe_search: SafeSearchCli::Moderate,
            modo_stream: false,
            verboso: false,
            silencioso: false,
            buscar_conteudo: false,
            max_tamanho_conteudo: crate::cli::MAX_CONTENT_LENGTH_PADRAO,
            proxy: None,
            sem_proxy: false,
            timeout_global_segundos: crate::cli::GLOBAL_TIMEOUT_PADRAO,
            corresponde_plataforma_ua: false,
            limite_por_host: crate::cli::PER_HOST_LIMIT_PADRAO,
            caminho_chrome: None,
        }
    }

    #[test]
    fn montar_configuracoes_com_argumentos_validos() {
        let argumentos = argumentos_base();
        let cfg = montar_configuracoes(&argumentos).expect("deve montar configurações");
        assert_eq!(cfg.query, "rust async");
        assert_eq!(cfg.queries, vec!["rust async".to_string()]);
        assert_eq!(cfg.formato, FormatoSaida::Json);
        assert_eq!(cfg.num_resultados, Some(5));
        assert_eq!(cfg.paralelismo, 5);
        assert_eq!(cfg.paginas, 1);
        assert!(!cfg.modo_stream);
    }

    #[test]
    fn montar_configuracoes_rejeita_queries_todas_vazias() {
        let mut argumentos = argumentos_base();
        argumentos.queries = vec!["   ".to_string(), "".to_string()];
        let resultado = montar_configuracoes(&argumentos);
        assert!(resultado.is_err());
    }

    #[test]
    fn montar_configuracoes_rejeita_formato_desconhecido() {
        let mut argumentos = argumentos_base();
        argumentos.formato = "xml".to_string();
        assert!(montar_configuracoes(&argumentos).is_err());
    }

    #[test]
    fn montar_configuracoes_rejeita_paralelismo_zero() {
        let mut argumentos = argumentos_base();
        argumentos.paralelismo = 0;
        assert!(montar_configuracoes(&argumentos).is_err());
    }

    #[test]
    fn montar_configuracoes_rejeita_paralelismo_acima_do_maximo() {
        let mut argumentos = argumentos_base();
        argumentos.paralelismo = 50;
        assert!(montar_configuracoes(&argumentos).is_err());
    }

    #[test]
    fn montar_configuracoes_aplica_default_num_15_quando_omitido() {
        // v0.4.0: quando `--num` é omitido (None), o default efetivo é 15
        // E isso auto-eleva `--pages` para 2 (já que 15 > 10 e pages=1 é o default).
        let mut argumentos = argumentos_base();
        argumentos.num_resultados = None;
        argumentos.paginas = 1;
        let cfg = montar_configuracoes(&argumentos).expect("deve montar");
        assert_eq!(cfg.num_resultados, Some(15), "default 15 quando None");
        assert_eq!(cfg.paginas, 2, "auto-eleva para ceil(15/10) = 2");
    }

    #[test]
    fn montar_configuracoes_respeita_pages_explicito_acima_de_1() {
        // Se o usuário passa `--pages 3` explícito, NÃO sobrescrever com
        // auto-paginação, mesmo que num efetivo exigisse menos.
        let mut argumentos = argumentos_base();
        argumentos.num_resultados = Some(20);
        argumentos.paginas = 3;
        let cfg = montar_configuracoes(&argumentos).expect("deve montar");
        assert_eq!(cfg.num_resultados, Some(20));
        assert_eq!(cfg.paginas, 3, "respeita --pages explícito do usuário");
    }

    #[test]
    fn montar_configuracoes_auto_pagina_quando_num_maior_que_10() {
        // Casos de fronteira do auto-paginador.
        let casos = [
            (11u32, 2u32), // ceil(11/10) = 2
            (15, 2),       // ceil(15/10) = 2
            (20, 2),       // ceil(20/10) = 2
            (21, 3),       // ceil(21/10) = 3
            (45, 5),       // ceil(45/10) = 5
            (60, 5),       // ceil(60/10) = 6 mas clamp em 5
        ];
        for (num, paginas_esperadas) in casos {
            let mut argumentos = argumentos_base();
            argumentos.num_resultados = Some(num);
            argumentos.paginas = 1;
            let cfg = montar_configuracoes(&argumentos)
                .unwrap_or_else(|e| panic!("deve montar para num={num}: {e}"));
            assert_eq!(
                cfg.paginas, paginas_esperadas,
                "para num={num}, paginas deveria ser {paginas_esperadas}"
            );
        }
    }

    #[test]
    fn montar_configuracoes_nao_auto_pagina_quando_num_10_ou_menos() {
        // Se num efetivo <= 10, mantém paginas=1 (sem auto-paginação).
        for num in [1u32, 5, 10] {
            let mut argumentos = argumentos_base();
            argumentos.num_resultados = Some(num);
            argumentos.paginas = 1;
            let cfg = montar_configuracoes(&argumentos).expect("deve montar");
            assert_eq!(cfg.paginas, 1, "num={num} não deveria auto-paginar");
        }
    }

    #[test]
    fn montar_configuracoes_combina_multiplas_queries_posicionais() {
        let mut argumentos = argumentos_base();
        argumentos.queries = vec![
            "alfa".to_string(),
            "beta".to_string(),
            "alfa".to_string(), // duplicata
            "gama".to_string(),
        ];
        let cfg = montar_configuracoes(&argumentos).expect("deve montar configurações");
        assert_eq!(cfg.queries, vec!["alfa", "beta", "gama"]);
        assert_eq!(cfg.query, "alfa");
    }
}
