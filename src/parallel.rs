//! Paralelismo multi-query com `JoinSet`, `Semaphore`, staggered launch e `CancellationToken`.
//!
//! Implementação da iteração 2 conforme seções 4.1–4.6, 13 e 15.8 da especificação.
//!
//! Contratos importantes:
//! - `Semaphore` limita concorrência ao valor de `--parallel` (1..=20).
//! - Staggered launch adiciona `indice * 200ms + jitter(0..300ms)` ANTES do spawn
//!   para evitar burst síncrono que dispararia rate-limit.
//! - `CancellationToken` é verificado entre estágios de cada task; quando SIGINT
//!   dispara, tasks em voo abortam gracefulmente com erro `cancelled`.
//! - Falha de uma task NÃO aborta o `JoinSet` inteiro. Outras tasks continuam.
//!   Queries falhas produzem `SaidaBusca` com campo `error` preenchido.
//! - Decisão de Client por query (isolamento de cookie jar) segue seção 4.3:
//!   `paginas == 1` → compartilhado; `paginas > 1` → novo Client por query.

use crate::fetch_conteudo;
use crate::http;
use crate::http::ConfiguracaoProxy;
use crate::search;
use crate::types::{
    ConfiguracaoSeletores, Configuracoes, MetadadosBusca, SaidaBusca, SaidaBuscaMultipla,
};
use anyhow::{Context, Result};
use rand::Rng;
use reqwest::Client;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, Semaphore};
use tokio::task::JoinSet;
use tokio_util::sync::CancellationToken;

/// Delay base por índice (milissegundos) para staggered launch.
const DELAY_BASE_STAGGERED_MS: u64 = 200;

/// Jitter máximo adicional (milissegundos) para staggered launch.
const JITTER_MAXIMO_STAGGERED_MS: u64 = 300;

/// Executa múltiplas queries em paralelo respeitando o limite `--parallel`.
///
/// # Argumentos
/// * `queries` — lista já deduplicada/filtrada de queries.
/// * `configuracoes` — template de configuração (query individual será sobrescrita).
/// * `cancelamento` — token que sinaliza SIGINT / timeout global.
///
/// # Comportamento em falha
/// Se uma query falhar, sua `SaidaBusca` é gerada com `error` preenchido e
/// `results_count = 0`. O processo NÃO aborta demais queries em voo.
pub async fn executar_buscas_paralelas(
    queries: Vec<String>,
    configuracoes: Configuracoes,
    cancelamento: CancellationToken,
) -> Result<SaidaBuscaMultipla> {
    let quantidade_queries = u32::try_from(queries.len()).unwrap_or(u32::MAX);
    let paralelismo_efetivo = configuracoes.paralelismo.max(1);
    let timestamp_inicio = chrono::Utc::now().to_rfc3339();

    tracing::info!(
        queries = quantidade_queries,
        parallel = paralelismo_efetivo,
        paginas = configuracoes.paginas,
        "Iniciando execução multi-query paralela"
    );

    let semaforo = Arc::new(Semaphore::new(paralelismo_efetivo as usize));
    let configuracoes = Arc::new(configuracoes);
    // Flag compartilhada que sinaliza rate-limit para todas as tasks.
    let flag_rate_limit = Arc::new(AtomicBool::new(false));

    let config_proxy = Arc::new(ConfiguracaoProxy::a_partir_de(
        configuracoes.proxy.as_deref(),
        configuracoes.sem_proxy,
    ));

    // Decide se o Client é compartilhado ou construído por task.
    // Conforme seção 4.3: paginas == 1 → compartilhado; paginas > 1 → isolado.
    let cliente_compartilhado: Option<Client> = if configuracoes.paginas <= 1 {
        let cliente = http::construir_cliente_com_proxy(
            &configuracoes.perfil_browser,
            configuracoes.timeout_segundos,
            &configuracoes.idioma,
            &configuracoes.pais,
            &config_proxy,
        )
        .context("falha ao construir cliente HTTP compartilhado para multi-query")?;
        Some(cliente)
    } else {
        None
    };

    let mut conjunto_tasks: JoinSet<(usize, Result<SaidaBusca>)> = JoinSet::new();

    for (indice, query) in queries.into_iter().enumerate() {
        // Captura clones/refs para mover para a task.
        let semaforo_task = Arc::clone(&semaforo);
        let configuracoes_task = Arc::clone(&configuracoes);
        let cancelamento_task = cancelamento.clone();
        let cliente_para_task = cliente_compartilhado.clone();
        let flag_rate_limit_task = Arc::clone(&flag_rate_limit);
        let config_proxy_task = Arc::clone(&config_proxy);

        conjunto_tasks.spawn(async move {
            // Staggered launch: atrasa antes de adquirir permit para evitar burst síncrono.
            let jitter_ms = rand::thread_rng().gen_range(0..JITTER_MAXIMO_STAGGERED_MS);
            let delay_total = Duration::from_millis(
                DELAY_BASE_STAGGERED_MS.saturating_mul(indice as u64) + jitter_ms,
            );

            tokio::select! {
                biased;
                _ = cancelamento_task.cancelled() => {
                    return (indice, Err(anyhow::anyhow!("execução cancelada antes do início da query {indice}")));
                }
                _ = tokio::time::sleep(delay_total) => {}
            }

            // Adquire permit do semaphore (owned — liberado no drop ao fim da task).
            let permit = match semaforo_task.acquire_owned().await {
                Ok(p) => p,
                Err(erro) => {
                    return (
                        indice,
                        Err(anyhow::anyhow!("semáforo fechado: {erro}")),
                    );
                }
            };

            tracing::debug!(indice, query = %query, "permit adquirido, iniciando task");

            if cancelamento_task.is_cancelled() {
                drop(permit);
                return (indice, Err(anyhow::anyhow!("execução cancelada após aquisição de permit")));
            }

            // Decisão de Client por task.
            let resultado_cliente = match cliente_para_task {
                Some(compartilhado) => Ok(compartilhado),
                None => http::construir_cliente_com_proxy(
                    &configuracoes_task.perfil_browser,
                    configuracoes_task.timeout_segundos,
                    &configuracoes_task.idioma,
                    &configuracoes_task.pais,
                    &config_proxy_task,
                )
                .context("falha ao construir Client isolado para query"),
            };

            let resultado = match resultado_cliente {
                Ok(cliente) => {
                    executar_query_com_cancelamento(
                        &query,
                        &cliente,
                        &configuracoes_task,
                        &flag_rate_limit_task,
                        &cancelamento_task,
                    )
                    .await
                }
                Err(erro) => Err(erro),
            };

            drop(permit);
            (indice, resultado)
        });
    }

    // Coleta todas as tasks — preservando a ordem original das queries.
    let mut resultados_ordenados: Vec<Option<SaidaBusca>> =
        (0..quantidade_queries).map(|_| None).collect();

    while let Some(resultado_task) = conjunto_tasks.join_next().await {
        match resultado_task {
            Ok((indice, Ok(saida))) => {
                resultados_ordenados[indice] = Some(saida);
            }
            Ok((indice, Err(erro))) => {
                tracing::warn!(indice, ?erro, "query falhou, gerando SaidaBusca de erro");
                resultados_ordenados[indice] = Some(saida_de_erro(indice, erro, &configuracoes));
            }
            Err(erro_join) => {
                // Panic dentro da task. Recuperamos o índice se disponível.
                tracing::error!(?erro_join, "task panicou ou foi abortada");
                // Busca o primeiro slot vazio para registrar o erro; sem perda de outros resultados.
                if let Some(slot) = resultados_ordenados.iter_mut().find(|s| s.is_none()) {
                    *slot = Some(saida_de_erro(
                        0,
                        anyhow::anyhow!("task panicou: {erro_join}"),
                        &configuracoes,
                    ));
                }
            }
        }
    }

    // Converte Option<SaidaBusca> em Vec<SaidaBusca> (todos os slots devem estar preenchidos).
    let buscas: Vec<SaidaBusca> = resultados_ordenados
        .into_iter()
        .enumerate()
        .map(|(indice, slot)| {
            slot.unwrap_or_else(|| {
                saida_de_erro(
                    indice,
                    anyhow::anyhow!("resultado ausente para query {indice}"),
                    &configuracoes,
                )
            })
        })
        .collect();

    tracing::info!(total = buscas.len(), "multi-query concluída");

    Ok(SaidaBuscaMultipla {
        quantidade_queries,
        timestamp: timestamp_inicio,
        paralelismo: paralelismo_efetivo,
        buscas,
    })
}

/// Estatísticas agregadas de uma execução multi-query em modo streaming.
#[derive(Debug, Clone, Default)]
pub struct EstatisticasStream {
    /// Total de queries submetidas.
    pub total: u32,
    /// Queries finalizadas com sucesso (sem campo `error`).
    pub sucessos: u32,
    /// Queries finalizadas com erro.
    pub erros: u32,
    /// Timestamp (RFC 3339) do início da execução.
    pub timestamp_inicio: String,
    /// Paralelismo efetivo.
    pub paralelismo: u32,
}

/// Executa múltiplas queries em paralelo EMITINDO resultados via `mpsc::Sender`
/// conforme cada task termina. O consumer (em `pipeline`) recebe os resultados e
/// emite NDJSON / text / markdown incrementalmente.
///
/// Retorna `EstatisticasStream` após todas as tasks terminarem.
///
/// Os resultados chegam em ORDEM DE CONCLUSÃO (não a ordem das queries de entrada).
/// Cada item enviado é `(indice_original, SaidaBusca)` para que o consumer saiba
/// a qual query a saída corresponde.
pub async fn executar_buscas_paralelas_streaming(
    queries: Vec<String>,
    configuracoes: Configuracoes,
    cancelamento: CancellationToken,
    canal_saida: mpsc::Sender<(usize, SaidaBusca)>,
) -> Result<EstatisticasStream> {
    let quantidade_queries = u32::try_from(queries.len()).unwrap_or(u32::MAX);
    let paralelismo_efetivo = configuracoes.paralelismo.max(1);
    let timestamp_inicio = chrono::Utc::now().to_rfc3339();

    tracing::info!(
        queries = quantidade_queries,
        parallel = paralelismo_efetivo,
        "Iniciando execução multi-query paralela streaming"
    );

    let semaforo = Arc::new(Semaphore::new(paralelismo_efetivo as usize));
    let configuracoes = Arc::new(configuracoes);
    let flag_rate_limit = Arc::new(AtomicBool::new(false));

    let config_proxy = Arc::new(ConfiguracaoProxy::a_partir_de(
        configuracoes.proxy.as_deref(),
        configuracoes.sem_proxy,
    ));

    let cliente_compartilhado: Option<Client> = if configuracoes.paginas <= 1 {
        let cliente = http::construir_cliente_com_proxy(
            &configuracoes.perfil_browser,
            configuracoes.timeout_segundos,
            &configuracoes.idioma,
            &configuracoes.pais,
            &config_proxy,
        )
        .context("falha ao construir cliente HTTP compartilhado para streaming")?;
        Some(cliente)
    } else {
        None
    };

    let mut conjunto_tasks: JoinSet<(usize, SaidaBusca)> = JoinSet::new();

    for (indice, query) in queries.into_iter().enumerate() {
        let semaforo_task = Arc::clone(&semaforo);
        let configuracoes_task = Arc::clone(&configuracoes);
        let cancelamento_task = cancelamento.clone();
        let cliente_para_task = cliente_compartilhado.clone();
        let flag_rate_limit_task = Arc::clone(&flag_rate_limit);
        let config_proxy_task = Arc::clone(&config_proxy);

        conjunto_tasks.spawn(async move {
            let jitter_ms = rand::thread_rng().gen_range(0..JITTER_MAXIMO_STAGGERED_MS);
            let delay_total = Duration::from_millis(
                DELAY_BASE_STAGGERED_MS.saturating_mul(indice as u64) + jitter_ms,
            );

            tokio::select! {
                biased;
                _ = cancelamento_task.cancelled() => {
                    return (
                        indice,
                        saida_de_erro(
                            indice,
                            anyhow::anyhow!("execução cancelada antes da query {indice}"),
                            &configuracoes_task,
                        ),
                    );
                }
                _ = tokio::time::sleep(delay_total) => {}
            }

            let permit = match semaforo_task.acquire_owned().await {
                Ok(p) => p,
                Err(erro) => {
                    return (
                        indice,
                        saida_de_erro(
                            indice,
                            anyhow::anyhow!("semáforo fechado: {erro}"),
                            &configuracoes_task,
                        ),
                    );
                }
            };

            if cancelamento_task.is_cancelled() {
                drop(permit);
                return (
                    indice,
                    saida_de_erro(
                        indice,
                        anyhow::anyhow!("execução cancelada após permit"),
                        &configuracoes_task,
                    ),
                );
            }

            let resultado_cliente = match cliente_para_task {
                Some(c) => Ok(c),
                None => http::construir_cliente_com_proxy(
                    &configuracoes_task.perfil_browser,
                    configuracoes_task.timeout_segundos,
                    &configuracoes_task.idioma,
                    &configuracoes_task.pais,
                    &config_proxy_task,
                )
                .context("falha ao construir Client isolado"),
            };

            let resultado = match resultado_cliente {
                Ok(cliente) => {
                    executar_query_com_cancelamento(
                        &query,
                        &cliente,
                        &configuracoes_task,
                        &flag_rate_limit_task,
                        &cancelamento_task,
                    )
                    .await
                }
                Err(erro) => Err(erro),
            };

            drop(permit);
            match resultado {
                Ok(saida) => (indice, saida),
                Err(erro) => (indice, saida_de_erro(indice, erro, &configuracoes_task)),
            }
        });
    }

    let mut sucessos: u32 = 0;
    let mut erros: u32 = 0;

    while let Some(resultado_task) = conjunto_tasks.join_next().await {
        match resultado_task {
            Ok((indice, saida)) => {
                if saida.erro.is_some() {
                    erros = erros.saturating_add(1);
                } else {
                    sucessos = sucessos.saturating_add(1);
                }
                if let Err(erro_send) = canal_saida.send((indice, saida)).await {
                    tracing::warn!(
                        ?erro_send,
                        "consumer de streaming fechou o canal — abortando envio"
                    );
                    // Recolher tasks remanescentes para evitar zumbis.
                    conjunto_tasks.abort_all();
                    break;
                }
            }
            Err(erro_join) => {
                tracing::error!(?erro_join, "task panicou em streaming");
                erros = erros.saturating_add(1);
            }
        }
    }

    tracing::info!(
        total = quantidade_queries,
        sucessos,
        erros,
        "streaming concluído"
    );

    Ok(EstatisticasStream {
        total: quantidade_queries,
        sucessos,
        erros,
        timestamp_inicio,
        paralelismo: paralelismo_efetivo,
    })
}

/// Executa UMA query com paginação, retry, fallback Lite e fetch-content (se ativo).
async fn executar_query_com_cancelamento(
    query: &str,
    cliente: &Client,
    configuracoes: &Configuracoes,
    flag_rate_limit: &Arc<AtomicBool>,
    cancelamento: &CancellationToken,
) -> Result<SaidaBusca> {
    let inicio = Instant::now();

    if cancelamento.is_cancelled() {
        anyhow::bail!("execução cancelada antes do request de {query:?}");
    }

    tracing::info!(query = %query, endpoint = configuracoes.endpoint.como_str(), "enviando request");

    // Cria uma cópia com a query sobrescrita para `buscar_com_paginacao`.
    let mut cfg_task = configuracoes.clone();
    cfg_task.query = query.to_string();

    let agregado = match search::buscar_com_paginacao(
        cliente,
        &cfg_task,
        query,
        flag_rate_limit,
        cancelamento,
    )
    .await
    {
        Ok(a) => a,
        Err(motivo) => {
            let tempo_ms = inicio.elapsed().as_millis().min(u64::MAX as u128) as u64;
            let timestamp = chrono::Utc::now().to_rfc3339();
            let hash_seletores = calcular_hash_seletores_pt(&configuracoes.seletores);
            let usou_proxy = ConfiguracaoProxy::a_partir_de(
                configuracoes.proxy.as_deref(),
                configuracoes.sem_proxy,
            )
            .esta_ativo();
            return Ok(SaidaBusca {
                query: query.to_string(),
                motor: "duckduckgo".to_string(),
                endpoint: configuracoes.endpoint.como_str().to_string(),
                timestamp,
                regiao: search::formatar_kl(&configuracoes.idioma, &configuracoes.pais),
                quantidade_resultados: 0,
                resultados: Vec::new(),
                paginas_buscadas: 0,
                erro: Some(motivo.como_codigo_erro().to_string()),
                mensagem: Some(motivo.mensagem()),
                metadados: MetadadosBusca {
                    tempo_execucao_ms: tempo_ms,
                    hash_seletores,
                    retentativas: configuracoes.retries,
                    usou_endpoint_fallback: false,
                    fetches_simultaneos: 0,
                    sucessos_fetch: 0,
                    falhas_fetch: 0,
                    usou_chrome: false,
                    user_agent: configuracoes.user_agent.clone(),
                    usou_proxy,
                },
            });
        }
    };

    let quantidade = u32::try_from(agregado.resultados.len()).unwrap_or(u32::MAX);
    let hash_seletores = calcular_hash_seletores_pt(&configuracoes.seletores);
    let tempo_ms = inicio.elapsed().as_millis().min(u64::MAX as u128) as u64;
    let timestamp = chrono::Utc::now().to_rfc3339();
    let retentativas = agregado.tentativas.saturating_sub(1);

    let usou_proxy =
        ConfiguracaoProxy::a_partir_de(configuracoes.proxy.as_deref(), configuracoes.sem_proxy)
            .esta_ativo();
    let metadados = MetadadosBusca {
        tempo_execucao_ms: tempo_ms,
        hash_seletores,
        retentativas,
        usou_endpoint_fallback: agregado.usou_fallback_lite,
        fetches_simultaneos: 0,
        sucessos_fetch: 0,
        falhas_fetch: 0,
        usou_chrome: false,
        user_agent: configuracoes.user_agent.clone(),
        usou_proxy,
    };

    let mut saida = SaidaBusca {
        query: query.to_string(),
        motor: "duckduckgo".to_string(),
        endpoint: agregado.endpoint_efetivo.como_str().to_string(),
        timestamp,
        regiao: search::formatar_kl(&configuracoes.idioma, &configuracoes.pais),
        quantidade_resultados: quantidade,
        resultados: agregado.resultados,
        paginas_buscadas: agregado.paginas_buscadas,
        erro: None,
        mensagem: None,
        metadados,
    };

    // Enriquecimento opcional via --fetch-content (iter. 5).
    fetch_conteudo::enriquecer_com_conteudo(&mut saida, cliente, configuracoes, cancelamento).await;

    Ok(saida)
}

/// Gera uma `SaidaBusca` representando uma query com falha.
///
/// Mantém a posição no output multi mesmo quando uma query individual falhou.
fn saida_de_erro(indice: usize, erro: anyhow::Error, configuracoes: &Configuracoes) -> SaidaBusca {
    let query_ref = configuracoes
        .queries
        .get(indice)
        .cloned()
        .unwrap_or_default();
    let mensagem = format!("{erro:#}");
    let timestamp = chrono::Utc::now().to_rfc3339();
    let hash_seletores = calcular_hash_seletores_pt(&configuracoes.seletores);

    SaidaBusca {
        query: query_ref,
        motor: "duckduckgo".to_string(),
        endpoint: "html".to_string(),
        timestamp,
        regiao: search::formatar_kl(&configuracoes.idioma, &configuracoes.pais),
        quantidade_resultados: 0,
        resultados: Vec::new(),
        paginas_buscadas: 0,
        erro: Some(crate::error::codigos::NETWORK_ERROR.to_string()),
        mensagem: Some(mensagem),
        metadados: MetadadosBusca {
            tempo_execucao_ms: 0,
            hash_seletores,
            retentativas: 0,
            usou_endpoint_fallback: false,
            fetches_simultaneos: 0,
            sucessos_fetch: 0,
            falhas_fetch: 0,
            usou_chrome: false,
            user_agent: configuracoes.user_agent.clone(),
            usou_proxy: false,
        },
    }
}

/// Duplica a lógica de `pipeline::calcular_hash_seletores` para evitar dependência
/// circular de visibilidade. Mantida privada a este módulo.
fn calcular_hash_seletores_pt(cfg: &ConfiguracaoSeletores) -> String {
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
    use crate::types::{Endpoint, FormatoSaida, SafeSearch};

    fn configuracoes_de_teste(queries: Vec<String>, paralelismo: u32) -> Configuracoes {
        let primeira = queries.first().cloned().unwrap_or_default();
        Configuracoes {
            query: primeira,
            queries,
            num_resultados: None,
            formato: FormatoSaida::Json,
            timeout_segundos: 15,
            idioma: "pt".to_string(),
            pais: "br".to_string(),
            modo_verboso: false,
            modo_silencioso: true,
            user_agent: "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/146.0.0.0 Safari/537.36".to_string(),
            perfil_browser: crate::http::criar_perfil_browser("Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/146.0.0.0 Safari/537.36"),
            paralelismo,
            paginas: 1,
            retries: 0,
            endpoint: Endpoint::Html,
            filtro_temporal: None,
            safe_search: SafeSearch::Moderate,
            modo_stream: false,
            arquivo_saida: None,
            buscar_conteudo: false,
            max_tamanho_conteudo: 10_000,
            proxy: None,
            sem_proxy: false,
            timeout_global_segundos: 60,
            corresponde_plataforma_ua: false,
            limite_por_host: 2,
            caminho_chrome: None,
            seletores: std::sync::Arc::new(ConfiguracaoSeletores::default()),
        }
    }

    #[test]
    fn saida_de_erro_preenche_campos_obrigatorios() {
        let cfg = configuracoes_de_teste(vec!["alfa".into(), "beta".into()], 2);
        let erro = anyhow::anyhow!("falha sintética de teste");
        let saida = saida_de_erro(1, erro, &cfg);
        assert_eq!(saida.query, "beta");
        assert_eq!(saida.quantidade_resultados, 0);
        assert!(saida.resultados.is_empty());
        assert!(saida.erro.is_some());
        assert!(saida.mensagem.is_some());
        assert_eq!(saida.regiao, "br-pt");
    }

    #[test]
    fn saida_de_erro_indice_fora_da_lista_usa_string_vazia() {
        let cfg = configuracoes_de_teste(vec!["apenas uma".into()], 1);
        let saida = saida_de_erro(99, anyhow::anyhow!("falha fora de bounds"), &cfg);
        // Sem query disponível para o índice → string vazia, mas não panica.
        assert!(saida.query.is_empty());
        assert!(saida.erro.is_some());
    }

    #[tokio::test]
    async fn executar_buscas_paralelas_cancelado_antes_do_spawn_retorna_erros() {
        // Cancelamos ANTES de chamar, todas as tasks devem retornar falha controlada.
        let token = CancellationToken::new();
        token.cancel();
        let cfg = configuracoes_de_teste(
            vec!["query-a".into(), "query-b".into(), "query-c".into()],
            3,
        );
        let queries = cfg.queries.clone();
        let resultado = executar_buscas_paralelas(queries, cfg, token).await;
        let saida = resultado.expect("função deve retornar Ok mesmo com todas falhando");
        assert_eq!(saida.quantidade_queries, 3);
        assert_eq!(saida.buscas.len(), 3);
        assert_eq!(saida.paralelismo, 3);
        // Todas devem estar marcadas com erro.
        for busca in &saida.buscas {
            assert!(
                busca.erro.is_some(),
                "query {:?} deveria ter falhado com cancelamento",
                busca.query
            );
        }
    }

    #[test]
    fn calcular_hash_seletores_pt_retorna_16_chars() {
        let cfg = ConfiguracaoSeletores::default();
        let hash = calcular_hash_seletores_pt(&cfg);
        assert_eq!(hash.len(), 16);
        assert!(hash.chars().all(|c| c.is_ascii_hexdigit()));
    }
}
