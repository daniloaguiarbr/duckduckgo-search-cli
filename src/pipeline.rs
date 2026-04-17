//! Orchestration of the CLI execution flow.
//!
//! In iteration 2, decides between single-query and multi-query flow based on
//! the number of effective queries (after combining positional + file + stdin,
//! dedup and empty-string filtering).
//!
//! - Single-query (1 query): uses the legacy `executar_busca_unica` flow and emits `SaidaBusca`.
//! - Multi-query (>=2 queries): delegates to `parallel::executar_buscas_paralelas`
//!   and emits `SaidaBuscaMultipla`.

use crate::fetch_conteudo;
use crate::http;
use crate::http::ConfiguracaoProxy;
use crate::parallel;
use crate::search;
use crate::types::{
    ConfiguracaoSeletores, Configuracoes, MetadadosBusca, SaidaBusca, SaidaBuscaMultipla,
};
use anyhow::{Context, Result};
use std::collections::HashSet;
use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::Instant;
use tokio_util::sync::CancellationToken;

/// Result emitted by the pipeline — may be a single output, aggregated multi output, or an already-emitted stream.
///
/// The `Stream` variant indicates that output was already emitted incrementally by
/// the consumer; the final `output` step MUST NOT re-emit anything. Only the
/// aggregated statistics are available for logging / exit-code decisions.
#[derive(Debug, Clone)]
pub enum ResultadoPipeline {
    Unica(Box<SaidaBusca>),
    Multipla(Box<SaidaBuscaMultipla>),
    Stream(crate::parallel::EstatisticasStream),
}

impl ResultadoPipeline {
    /// Total results summed across all queries (used for exit-code decisions).
    ///
    /// For `Stream` returns `sucessos` — a sufficient approximation for exit codes 0/5
    /// (success vs zero-results). Fine-grained `quantidade_resultados` precision in
    /// streaming mode would require duplicate aggregation by the consumer, which is not worth the cost.
    pub fn total_resultados(&self) -> u32 {
        match self {
            ResultadoPipeline::Unica(s) => s.quantidade_resultados,
            ResultadoPipeline::Multipla(m) => m
                .buscas
                .iter()
                .map(|b| b.quantidade_resultados)
                .fold(0u32, |acc, v| acc.saturating_add(v)),
            ResultadoPipeline::Stream(e) => e.sucessos,
        }
    }
}

/// Entry point for iteration 2: decides single vs multi based on `configuracoes.queries`.
///
/// `cancelamento` is the token that signals SIGINT (ctrl+c). In single-query mode
/// cancellation only affects the request via `reqwest` timeout; in multi-query mode it
/// is propagated explicitly to each task.
pub async fn executar_pipeline(
    configuracoes: Configuracoes,
    cancelamento: CancellationToken,
) -> Result<ResultadoPipeline> {
    match configuracoes.queries.len() {
        0 => anyhow::bail!("nenhuma query para executar (lista vazia após filtragem)"),
        1 => {
            if configuracoes.modo_stream {
                tracing::warn!(
                    "--stream ignorado em modo single-query (apenas 1 query efetiva); \
                     emitindo saída agregada padrão"
                );
            }
            // Fluxo single-query preserva compatibilidade do MVP.
            let mut cfg_single = configuracoes.clone();
            cfg_single.query = cfg_single.queries[0].clone();
            let saida = executar_busca_unica(&cfg_single, &cancelamento).await?;
            Ok(ResultadoPipeline::Unica(Box::new(saida)))
        }
        _ => {
            if configuracoes.modo_stream {
                return executar_pipeline_streaming(configuracoes, cancelamento).await;
            }
            let queries = configuracoes.queries.clone();
            let multi = parallel::executar_buscas_paralelas(queries, configuracoes, cancelamento)
                .await
                .context("falha na execução multi-query paralela")?;
            Ok(ResultadoPipeline::Multipla(Box::new(multi)))
        }
    }
}

/// Pipeline in streaming mode — emits results as tasks complete.
///
/// The spawned consumer drains the mpsc channel and emits NDJSON/text/markdown line by line.
/// Returns `ResultadoPipeline::Stream` at the end, indicating there is nothing left to emit.
async fn executar_pipeline_streaming(
    configuracoes: Configuracoes,
    cancelamento: CancellationToken,
) -> Result<ResultadoPipeline> {
    use crate::types::FormatoSaida;
    use tokio::sync::mpsc;

    let formato = configuracoes.formato;
    let arquivo_saida = configuracoes.arquivo_saida.clone();
    let queries = configuracoes.queries.clone();
    let paralelismo = configuracoes.paralelismo.max(1) as usize;

    // Buffer = paralelismo * 2, conforme spec. Min 2 para evitar starvation trivial.
    let (tx, mut rx) = mpsc::channel::<(usize, SaidaBusca)>(paralelismo.saturating_mul(2).max(2));

    // Spawn consumer: consome itens e emite conforme formato.
    let consumer = tokio::spawn(async move {
        let mut emitidos: u64 = 0;
        while let Some((indice, saida)) = rx.recv().await {
            let formato_resolvido = match formato {
                FormatoSaida::Auto | FormatoSaida::Json => FormatoSaida::Json,
                outro => outro,
            };
            let res = match formato_resolvido {
                FormatoSaida::Json | FormatoSaida::Auto => {
                    crate::output::emitir_ndjson(&saida, arquivo_saida.as_deref())
                }
                FormatoSaida::Text => {
                    crate::output::emitir_stream_text(indice, &saida, arquivo_saida.as_deref())
                }
                FormatoSaida::Markdown => {
                    crate::output::emitir_stream_markdown(indice, &saida, arquivo_saida.as_deref())
                }
            };
            if let Err(erro) = res {
                if crate::output::eh_broken_pipe(&erro) {
                    tracing::debug!("BrokenPipe em streaming — encerrando consumer");
                    return Ok(());
                }
                tracing::error!(
                    ?erro,
                    "falha ao emitir item de streaming — abortando consumer"
                );
                return Err(erro);
            }
            emitidos = emitidos.saturating_add(1);
        }
        tracing::info!(emitidos, "consumer de streaming finalizado");
        Ok::<(), anyhow::Error>(())
    });

    let stats =
        parallel::executar_buscas_paralelas_streaming(queries, configuracoes, cancelamento, tx)
            .await
            .context("falha na execução multi-query streaming")?;

    // Aguardar consumer drenar canal.
    match consumer.await {
        Ok(Ok(())) => {}
        Ok(Err(erro)) => return Err(erro.context("consumer de streaming falhou")),
        Err(erro_join) => {
            tracing::error!(?erro_join, "consumer panicou");
            anyhow::bail!("consumer de streaming panicou: {erro_join}");
        }
    }

    Ok(ResultadoPipeline::Stream(stats))
}

/// Executes the full flow for a single-query search with pagination, retry and Lite fallback.
pub async fn executar_busca_unica(
    cfg: &Configuracoes,
    cancelamento: &CancellationToken,
) -> Result<SaidaBusca> {
    let inicio = Instant::now();

    let config_proxy = ConfiguracaoProxy::a_partir_de(cfg.proxy.as_deref(), cfg.sem_proxy);
    let cliente = http::construir_cliente_com_proxy(
        &cfg.perfil_browser,
        cfg.timeout_segundos,
        &cfg.idioma,
        &cfg.pais,
        &config_proxy,
    )
    .context("falha ao construir cliente HTTP")?;

    tracing::info!(query = %cfg.query, endpoint = cfg.endpoint.como_str(), "Executando busca");

    let flag_rate_limit = Arc::new(AtomicBool::new(false));

    let agregado = match search::buscar_com_paginacao(
        &cliente,
        cfg,
        &cfg.query,
        &flag_rate_limit,
        cancelamento,
    )
    .await
    {
        Ok(a) => a,
        Err(motivo) => {
            return Ok(saida_de_falha(cfg, &motivo, inicio));
        }
    };

    let quantidade = u32::try_from(agregado.resultados.len()).unwrap_or(u32::MAX);
    let hash_seletores = calcular_hash_seletores(&cfg.seletores);
    let tempo_ms = inicio.elapsed().as_millis().min(u64::MAX as u128) as u64;
    let timestamp = chrono::Utc::now().to_rfc3339();
    // Retries = tentativas - 1 (o primeiro request não conta como retry).
    let retentativas = agregado.tentativas.saturating_sub(1);

    let metadados = MetadadosBusca {
        tempo_execucao_ms: tempo_ms,
        hash_seletores,
        retentativas,
        usou_endpoint_fallback: agregado.usou_fallback_lite,
        fetches_simultaneos: 0,
        sucessos_fetch: 0,
        falhas_fetch: 0,
        usou_chrome: false,
        user_agent: cfg.user_agent.clone(),
        usou_proxy: config_proxy.esta_ativo(),
    };

    let mut saida = SaidaBusca {
        query: cfg.query.clone(),
        motor: "duckduckgo".to_string(),
        endpoint: agregado.endpoint_efetivo.como_str().to_string(),
        timestamp,
        regiao: search::formatar_kl(&cfg.idioma, &cfg.pais),
        quantidade_resultados: quantidade,
        resultados: agregado.resultados,
        paginas_buscadas: agregado.paginas_buscadas,
        erro: None,
        mensagem: None,
        metadados,
    };

    // Enriquecimento opcional via --fetch-content (iter. 5).
    fetch_conteudo::enriquecer_com_conteudo(&mut saida, &cliente, cfg, cancelamento).await;

    tracing::info!(
        total = saida.quantidade_resultados,
        paginas = saida.paginas_buscadas,
        fallback = saida.metadados.usou_endpoint_fallback,
        fetch_content = cfg.buscar_conteudo,
        sucessos_fetch = saida.metadados.sucessos_fetch,
        "Busca concluída com sucesso"
    );
    Ok(saida)
}

/// Generates a `SaidaBusca` from a retry failure, preserving the structured error code
/// and partial metrics.
fn saida_de_falha(
    cfg: &Configuracoes,
    motivo: &search::MotivoFalhaRetry,
    inicio: Instant,
) -> SaidaBusca {
    let tempo_ms = inicio.elapsed().as_millis().min(u64::MAX as u128) as u64;
    let timestamp = chrono::Utc::now().to_rfc3339();
    let hash_seletores = calcular_hash_seletores(&cfg.seletores);
    let usou_proxy =
        ConfiguracaoProxy::a_partir_de(cfg.proxy.as_deref(), cfg.sem_proxy).esta_ativo();

    SaidaBusca {
        query: cfg.query.clone(),
        motor: "duckduckgo".to_string(),
        endpoint: cfg.endpoint.como_str().to_string(),
        timestamp,
        regiao: search::formatar_kl(&cfg.idioma, &cfg.pais),
        quantidade_resultados: 0,
        resultados: Vec::new(),
        paginas_buscadas: 0,
        erro: Some(motivo.como_codigo_erro().to_string()),
        mensagem: Some(motivo.mensagem()),
        metadados: MetadadosBusca {
            tempo_execucao_ms: tempo_ms,
            hash_seletores,
            retentativas: cfg.retries,
            usou_endpoint_fallback: false,
            fetches_simultaneos: 0,
            sucessos_fetch: 0,
            falhas_fetch: 0,
            usou_chrome: false,
            user_agent: cfg.user_agent.clone(),
            usou_proxy,
        },
    }
}

/// Backwards-compatible alias — preserves the `executar` name used in the original `lib.rs`.
pub async fn executar(cfg: &Configuracoes) -> Result<SaidaBusca> {
    executar_busca_unica(cfg, &CancellationToken::new()).await
}

/// Combines queries from three sources (positional, file, stdin), deduplicates
/// preserving the ORDER of the first occurrence, and filters empty strings after trim.
///
/// Performs no I/O: expects the caller to have already collected the lines (useful for tests).
///
/// # Example
///
/// ```
/// use duckduckgo_search_cli::pipeline::combinar_e_deduplicar_queries;
///
/// let resultado = combinar_e_deduplicar_queries(
///     vec!["rust".into(), "  ".into(), "tokio".into()],
///     vec!["rust".into(), "serde".into()],
///     vec!["".into(), "serde".into(), "axum".into()],
/// );
///
/// // Dedup preserves order of first occurrence; empty strings (after trim) are removed.
/// assert_eq!(resultado, vec!["rust", "tokio", "serde", "axum"]);
/// ```
pub fn combinar_e_deduplicar_queries(
    posicionais: Vec<String>,
    de_arquivo: Vec<String>,
    de_stdin: Vec<String>,
) -> Vec<String> {
    let mut vistos: HashSet<String> = HashSet::new();
    let mut resultado: Vec<String> = Vec::new();

    let todas = posicionais.into_iter().chain(de_arquivo).chain(de_stdin);

    for bruta in todas {
        let limpa = bruta.trim().to_string();
        if limpa.is_empty() {
            continue;
        }
        if vistos.insert(limpa.clone()) {
            resultado.push(limpa);
        }
    }

    resultado
}

/// Reads a queries file — one query per line, ignoring empty lines after trim.
///
/// Correctly handles both `\n` and `\r\n` (Windows) via `BufRead::lines`.
pub fn ler_queries_de_arquivo(caminho: &Path) -> Result<Vec<String>> {
    use std::io::BufRead;
    let arquivo = std::fs::File::open(caminho)
        .with_context(|| format!("falha ao abrir arquivo de queries {}", caminho.display()))?;
    let leitor = std::io::BufReader::new(arquivo);
    let mut linhas: Vec<String> = Vec::new();
    for (indice, linha) in leitor.lines().enumerate() {
        let linha = linha.with_context(|| {
            format!("falha ao ler linha {} de {}", indice + 1, caminho.display())
        })?;
        let tratada = linha.trim().to_string();
        if !tratada.is_empty() {
            linhas.push(tratada);
        }
    }
    Ok(linhas)
}

/// Reads queries from stdin — one per line — ONLY if stdin is not a TTY.
/// Returns an empty `Vec` when stdin is a TTY (i.e. the user did not pipe/redirect input).
pub fn ler_queries_de_stdin_se_pipe() -> Result<Vec<String>> {
    use std::io::{BufRead, IsTerminal};
    if std::io::stdin().is_terminal() {
        return Ok(Vec::new());
    }
    let leitor = std::io::stdin().lock();
    let mut linhas: Vec<String> = Vec::new();
    for (indice, linha) in leitor.lines().enumerate() {
        let linha = linha.with_context(|| format!("falha ao ler linha {} de stdin", indice + 1))?;
        let tratada = linha.trim().to_string();
        if !tratada.is_empty() {
            linhas.push(tratada);
        }
    }
    Ok(linhas)
}

/// Computes a blake3 hash (hex, first 16 chars) of the serialised selector configuration.
/// Useful for versioning changes to the `selectors.toml` file in future iterations.
fn calcular_hash_seletores(cfg: &ConfiguracaoSeletores) -> String {
    match toml::to_string(cfg) {
        Ok(serializado) => {
            let hash = blake3::hash(serializado.as_bytes());
            hash.to_hex().chars().take(16).collect()
        }
        Err(erro) => {
            tracing::warn!(?erro, "falha ao serializar config de seletores para hash");
            "unknown".to_string()
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn calcular_hash_seletores_retorna_16_chars() {
        let cfg = ConfiguracaoSeletores::default();
        let hash = calcular_hash_seletores(&cfg);
        assert_eq!(hash.len(), 16);
        assert!(hash.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn calcular_hash_seletores_eh_deterministico() {
        let cfg = ConfiguracaoSeletores::default();
        let h1 = calcular_hash_seletores(&cfg);
        let h2 = calcular_hash_seletores(&cfg);
        assert_eq!(h1, h2);
    }

    #[test]
    fn combinar_deduplica_preservando_ordem_da_primeira_ocorrencia() {
        let posicionais = vec!["alfa".to_string(), "beta".to_string()];
        let de_arquivo = vec!["beta".to_string(), "gama".to_string()];
        let de_stdin = vec!["alfa".to_string(), "delta".to_string()];
        let combinado = combinar_e_deduplicar_queries(posicionais, de_arquivo, de_stdin);
        assert_eq!(
            combinado,
            vec!["alfa", "beta", "gama", "delta"],
            "ordem deve ser da primeira ocorrência; duplicatas devem ser removidas"
        );
    }

    #[test]
    fn combinar_remove_strings_vazias_e_apenas_espacos() {
        let posicionais = vec!["   ".to_string(), "rust".to_string(), "".to_string()];
        let de_arquivo = vec!["\t\t".to_string(), "tokio".to_string()];
        let de_stdin = vec![];
        let combinado = combinar_e_deduplicar_queries(posicionais, de_arquivo, de_stdin);
        assert_eq!(combinado, vec!["rust", "tokio"]);
    }

    #[test]
    fn combinar_trimma_whitespace_antes_de_comparar() {
        let posicionais = vec!["  alfa  ".to_string()];
        let de_arquivo = vec!["alfa".to_string()];
        let de_stdin = vec!["alfa\t".to_string()];
        let combinado = combinar_e_deduplicar_queries(posicionais, de_arquivo, de_stdin);
        assert_eq!(
            combinado,
            vec!["alfa"],
            "queries equivalentes após trim devem ser deduplicadas"
        );
    }

    #[test]
    fn combinar_vazio_retorna_vazio() {
        let combinado = combinar_e_deduplicar_queries(vec![], vec![], vec![]);
        assert!(combinado.is_empty());
    }

    #[test]
    fn ler_queries_de_arquivo_aceita_linhas_windows_e_vazias() {
        use std::io::Write;
        let dir = std::env::temp_dir().join("ddg_cli_iter2_queries_test");
        std::fs::create_dir_all(&dir).unwrap();
        let caminho = dir.join("queries.txt");
        let conteudo = "rust\r\ntokio\r\n\r\n  axum  \n\nhttp://exemplo.com\n";
        let mut arquivo = std::fs::File::create(&caminho).unwrap();
        arquivo.write_all(conteudo.as_bytes()).unwrap();
        drop(arquivo);

        let linhas = ler_queries_de_arquivo(&caminho).expect("deve ler arquivo");
        assert_eq!(linhas, vec!["rust", "tokio", "axum", "http://exemplo.com"]);
        // Cleanup best-effort.
        let _ = std::fs::remove_file(&caminho);
    }

    #[test]
    fn total_resultados_em_saida_unica() {
        let saida = SaidaBusca {
            query: "q".into(),
            motor: "duckduckgo".into(),
            endpoint: "html".into(),
            timestamp: "t".into(),
            regiao: "br-pt".into(),
            quantidade_resultados: 7,
            resultados: vec![],
            paginas_buscadas: 1,
            erro: None,
            mensagem: None,
            metadados: MetadadosBusca {
                tempo_execucao_ms: 0,
                hash_seletores: "x".into(),
                retentativas: 0,
                usou_endpoint_fallback: false,
                fetches_simultaneos: 0,
                sucessos_fetch: 0,
                falhas_fetch: 0,
                usou_chrome: false,
                user_agent: "ua".into(),
                usou_proxy: false,
            },
        };
        assert_eq!(
            ResultadoPipeline::Unica(Box::new(saida)).total_resultados(),
            7
        );
    }

    #[test]
    fn total_resultados_em_saida_multipla_soma_todas() {
        let nova_saida = |n: u32| SaidaBusca {
            query: "q".into(),
            motor: "duckduckgo".into(),
            endpoint: "html".into(),
            timestamp: "t".into(),
            regiao: "br-pt".into(),
            quantidade_resultados: n,
            resultados: vec![],
            paginas_buscadas: 1,
            erro: None,
            mensagem: None,
            metadados: MetadadosBusca {
                tempo_execucao_ms: 0,
                hash_seletores: "x".into(),
                retentativas: 0,
                usou_endpoint_fallback: false,
                fetches_simultaneos: 0,
                sucessos_fetch: 0,
                falhas_fetch: 0,
                usou_chrome: false,
                user_agent: "ua".into(),
                usou_proxy: false,
            },
        };
        let multi = SaidaBuscaMultipla {
            quantidade_queries: 3,
            timestamp: "t".into(),
            paralelismo: 3,
            buscas: vec![nova_saida(2), nova_saida(5), nova_saida(0)],
        };
        assert_eq!(
            ResultadoPipeline::Multipla(Box::new(multi)).total_resultados(),
            7
        );
    }
}
