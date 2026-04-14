//! Fan-out paralelo de extração de conteúdo (flag `--fetch-content`).
//!
//! Para cada resultado de uma `SaidaBusca`, spawn uma task async limitada por
//! `Semaphore` (mesma capacidade de `--parallel`). Cada task chama
//! [`crate::content::extrair_conteudo_http`] e preenche `ResultadoBusca.conteudo`,
//! `.tamanho_conteudo` e `.metodo_extracao_conteudo` quando bem-sucedida.
//!
//! Também atualiza os campos de `MetadadosBusca`:
//! - `fetches_simultaneos` = total de tasks spawnadas.
//! - `sucessos_fetch` = tasks com `conteudo` não-vazio retornado.
//! - `falhas_fetch` = tasks que retornaram erro ou conteúdo vazio.
//!
//! A extração respeita `CancellationToken` — cancelamento global aborta todas
//! as tasks em voo rapidamente.

use crate::content;
use crate::types::{Configuracoes, SaidaBusca};
use reqwest::Client;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{Mutex, Semaphore};
use tokio::task::JoinSet;
use tokio_util::sync::CancellationToken;

#[cfg(feature = "chrome")]
use crate::browser::{detectar_chrome, extrair_texto_com_chrome, NavegadorChrome};

/// Mapa `host → Semaphore` para rate-limit per-host compartilhado entre tasks.
pub type MapaSemaforosPorHost = Arc<Mutex<HashMap<String, Arc<Semaphore>>>>;

/// Obtém (ou cria sob lock) o semáforo para o `host` dado com capacidade `limite`.
///
/// A lookup primeira sob lock, criando lazy se o host não existe. O `Arc<Semaphore>`
/// retornado é clonado e emprestado pelas tasks — o lock não é mantido durante
/// `.acquire_owned().await`.
pub async fn obter_semaforo_para_host(
    mapa: &MapaSemaforosPorHost,
    host: &str,
    limite: usize,
) -> Arc<Semaphore> {
    let mut guarda = mapa.lock().await;
    guarda
        .entry(host.to_string())
        .or_insert_with(|| Arc::new(Semaphore::new(limite.max(1))))
        .clone()
}

/// Extrai o host de uma URL. Retorna `"unknown"` quando a URL é malformada —
/// todas as URLs malformadas compartilham o mesmo slot (é uma fallback segura).
///
/// Hosts são normalizados para minúsculas para que `Exemplo.COM` e `exemplo.com`
/// compartilhem o mesmo `Semaphore` per-host.
///
/// # Exemplo
///
/// ```
/// use duckduckgo_search_cli::fetch_conteudo::extrair_host;
///
/// assert_eq!(extrair_host("https://www.example.com/path?q=1"), "www.example.com");
/// assert_eq!(extrair_host("https://API.test/x"), "api.test"); // minúsculas
/// assert_eq!(extrair_host("não-é-url"), "unknown");            // malformada
/// assert_eq!(extrair_host(""), "unknown");                      // vazia
/// ```
pub fn extrair_host(url: &str) -> String {
    reqwest::Url::parse(url)
        .ok()
        .and_then(|u| u.host_str().map(|s| s.to_lowercase()))
        .unwrap_or_else(|| "unknown".to_string())
}

/// Enriquece uma `SaidaBusca` com conteúdo textual de cada URL em paralelo.
///
/// Modifica `saida` IN-PLACE. Não retorna erro fatal — falhas individuais são
/// registradas em `metadados.falhas_fetch` e o campo `content` fica ausente no
/// `ResultadoBusca` correspondente.
pub async fn enriquecer_com_conteudo(
    saida: &mut SaidaBusca,
    cliente: &Client,
    configuracoes: &Configuracoes,
    cancelamento: &CancellationToken,
) {
    if !configuracoes.buscar_conteudo || saida.resultados.is_empty() {
        return;
    }

    let total = saida.resultados.len();
    tracing::info!(
        total,
        parallel = configuracoes.paralelismo,
        "iniciando enriquecimento paralelo com --fetch-content"
    );

    let semaforo = Arc::new(Semaphore::new(configuracoes.paralelismo.max(1) as usize));
    let mapa_por_host: MapaSemaforosPorHost = Arc::new(Mutex::new(HashMap::new()));
    let limite_por_host = configuracoes.limite_por_host.max(1);
    let tamanho_max = configuracoes.max_tamanho_conteudo;

    // Feature chrome: tenta lançar o navegador UMA vez antes do fan-out.
    // Se falhar (Chrome ausente), seguimos apenas com HTTP — sem quebrar a execução.
    #[cfg(feature = "chrome")]
    let navegador_chrome: Option<Arc<Mutex<NavegadorChrome>>> = {
        let caminho_manual = configuracoes.caminho_chrome.as_deref();
        match detectar_chrome(caminho_manual) {
            Ok(path) => {
                tracing::info!(path = %path.display(), "Chrome detectado — habilitando fallback");
                let timeout_launch = std::time::Duration::from_secs(30);
                match NavegadorChrome::lancar(&path, configuracoes.proxy.as_deref(), timeout_launch)
                    .await
                {
                    Ok(n) => Some(Arc::new(Mutex::new(n))),
                    Err(erro) => {
                        tracing::warn!(?erro, "falha ao lançar Chrome — seguindo apenas com HTTP");
                        None
                    }
                }
            }
            Err(erro) => {
                tracing::info!(?erro, "Chrome não detectado — seguindo apenas com HTTP");
                None
            }
        }
    };

    #[cfg(not(feature = "chrome"))]
    {
        if configuracoes.caminho_chrome.is_some() {
            tracing::warn!(
                "--chrome-path fornecido mas o binário não foi compilado com --features chrome — ignorando"
            );
        }
    }

    // Tipo retornado: (indice, texto, tamanho, metodo). texto vazio = falha.
    type ResultadoFetch = (usize, Option<(String, u32, String)>);
    let mut tasks: JoinSet<ResultadoFetch> = JoinSet::new();

    for (indice, resultado) in saida.resultados.iter().enumerate() {
        if cancelamento.is_cancelled() {
            tracing::warn!("cancelamento detectado — abortando spawn de fetches");
            break;
        }
        let url = resultado.url.clone();
        let cliente_task = cliente.clone();
        let semaforo_task = Arc::clone(&semaforo);
        let mapa_task = Arc::clone(&mapa_por_host);
        let cancelamento_task = cancelamento.clone();

        #[cfg(feature = "chrome")]
        let nav_task: Option<Arc<Mutex<NavegadorChrome>>> =
            navegador_chrome.as_ref().map(Arc::clone);

        tasks.spawn(async move {
            // Adquire permit global PRIMEIRO (controla concorrência total).
            let Ok(permit_global) = semaforo_task.acquire_owned().await else {
                tracing::debug!(indice, "semáforo global fechado — pulando");
                return (indice, None);
            };

            if cancelamento_task.is_cancelled() {
                drop(permit_global);
                return (indice, None);
            }

            // Agora adquire permit per-host (evita burst contra um único domínio).
            let host = extrair_host(&url);
            let semaforo_host = obter_semaforo_para_host(&mapa_task, &host, limite_por_host).await;
            let Ok(permit_host) = semaforo_host.acquire_owned().await else {
                tracing::debug!(indice, host, "semáforo por host fechado — pulando");
                drop(permit_global);
                return (indice, None);
            };

            if cancelamento_task.is_cancelled() {
                drop(permit_host);
                drop(permit_global);
                return (indice, None);
            }

            let resultado = content::extrair_conteudo_http(
                &cliente_task,
                &url,
                tamanho_max,
                &cancelamento_task,
            )
            .await;

            let retorno = match resultado {
                Ok(Some((texto, tamanho))) if !texto.is_empty() => {
                    (indice, Some((texto, tamanho, "http".to_string())))
                }
                Ok(Some((_vazio, _tamanho_original))) => {
                    // HTTP retornou conteúdo insuficiente — tentar Chrome se disponível.
                    #[cfg(feature = "chrome")]
                    {
                        if let Some(nav) = nav_task {
                            tracing::debug!(
                                indice,
                                url,
                                "conteúdo HTTP insuficiente — tentando Chrome"
                            );
                            let mut guarda = nav.lock().await;
                            match extrair_texto_com_chrome(
                                &mut guarda,
                                &url,
                                tamanho_max,
                                std::time::Duration::from_secs(30),
                            )
                            .await
                            {
                                Ok(texto) if !texto.is_empty() => {
                                    let tamanho_cast =
                                        u32::try_from(texto.len()).unwrap_or(u32::MAX);
                                    drop(permit_host);
                                    drop(permit_global);
                                    return (
                                        indice,
                                        Some((texto, tamanho_cast, "chrome".to_string())),
                                    );
                                }
                                Ok(_) => {
                                    tracing::debug!(indice, url, "Chrome também retornou vazio");
                                }
                                Err(erro) => {
                                    tracing::debug!(indice, url, ?erro, "Chrome falhou");
                                }
                            }
                        }
                    }
                    (indice, None)
                }
                Ok(None) => {
                    tracing::debug!(indice, url, "content-type não HTML — sem conteúdo");
                    (indice, None)
                }
                Err(erro) => {
                    tracing::debug!(indice, url, ?erro, "falha ao extrair conteúdo HTTP");
                    (indice, None)
                }
            };

            drop(permit_host);
            drop(permit_global);
            retorno
        });
    }

    let mut sucessos: u32 = 0;
    let mut falhas: u32 = 0;
    let mut usou_chrome: bool = false;

    while let Some(join_res) = tasks.join_next().await {
        match join_res {
            Ok((indice, Some((texto, tamanho, metodo)))) => {
                if indice < saida.resultados.len() && !texto.is_empty() {
                    let res = &mut saida.resultados[indice];
                    if metodo == "chrome" {
                        usou_chrome = true;
                    }
                    res.conteudo = Some(texto);
                    res.tamanho_conteudo = Some(tamanho);
                    res.metodo_extracao_conteudo = Some(metodo);
                    sucessos = sucessos.saturating_add(1);
                } else {
                    falhas = falhas.saturating_add(1);
                }
            }
            Ok((_, None)) => {
                falhas = falhas.saturating_add(1);
            }
            Err(erro_join) => {
                tracing::warn!(?erro_join, "task de fetch panicou");
                falhas = falhas.saturating_add(1);
            }
        }
    }

    saida.metadados.fetches_simultaneos = u32::try_from(total).unwrap_or(u32::MAX);
    saida.metadados.sucessos_fetch = sucessos;
    saida.metadados.falhas_fetch = falhas;
    if usou_chrome {
        saida.metadados.usou_chrome = true;
    }

    // Cleanup explícito do navegador (feature chrome).
    #[cfg(feature = "chrome")]
    if let Some(nav_arc) = navegador_chrome {
        drop(nav_arc); // Drop releases Mutex e o NavegadorChrome::drop aborta handler.
        tracing::debug!("Chrome dropped após enriquecimento");
    }

    tracing::info!(
        total,
        sucessos,
        falhas,
        "enriquecimento com conteúdo concluído"
    );
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::types::{Endpoint, FormatoSaida, MetadadosBusca, ResultadoBusca, SafeSearch};

    fn configuracoes_fetch(paralelismo: u32, max_tam: usize) -> Configuracoes {
        Configuracoes {
            query: "q".to_string(),
            queries: vec!["q".to_string()],
            num_resultados: None,
            formato: FormatoSaida::Json,
            timeout_segundos: 5,
            idioma: "pt".to_string(),
            pais: "br".to_string(),
            modo_verboso: false,
            modo_silencioso: true,
            user_agent: "Mozilla/5.0".to_string(),
            paralelismo,
            paginas: 1,
            retries: 0,
            endpoint: Endpoint::Html,
            filtro_temporal: None,
            safe_search: SafeSearch::Moderate,
            modo_stream: false,
            arquivo_saida: None,
            buscar_conteudo: true,
            max_tamanho_conteudo: max_tam,
            proxy: None,
            sem_proxy: false,
            timeout_global_segundos: 60,
            corresponde_plataforma_ua: false,
            limite_por_host: 2,
            caminho_chrome: None,
            seletores: std::sync::Arc::new(crate::types::ConfiguracaoSeletores::default()),
        }
    }

    fn saida_vazia() -> SaidaBusca {
        SaidaBusca {
            query: "q".to_string(),
            motor: "duckduckgo".to_string(),
            endpoint: "html".to_string(),
            timestamp: "t".to_string(),
            regiao: "br-pt".to_string(),
            quantidade_resultados: 0,
            resultados: vec![],
            paginas_buscadas: 1,
            erro: None,
            mensagem: None,
            metadados: MetadadosBusca {
                tempo_execucao_ms: 0,
                hash_seletores: "x".to_string(),
                retentativas: 0,
                usou_endpoint_fallback: false,
                fetches_simultaneos: 0,
                sucessos_fetch: 0,
                falhas_fetch: 0,
                usou_chrome: false,
                user_agent: "ua".to_string(),
                usou_proxy: false,
            },
        }
    }

    #[tokio::test]
    async fn enriquecer_com_conteudo_no_op_quando_flag_false() {
        let cliente = reqwest::Client::new();
        let mut cfg = configuracoes_fetch(3, 1000);
        cfg.buscar_conteudo = false;
        let mut saida = saida_vazia();
        saida.resultados.push(ResultadoBusca {
            posicao: 1,
            titulo: "Um".to_string(),
            url: "http://inexistente.local/a".to_string(),
            url_exibicao: None,
            snippet: None,
            titulo_original: None,
            conteudo: None,
            tamanho_conteudo: None,
            metodo_extracao_conteudo: None,
        });

        let token = CancellationToken::new();
        enriquecer_com_conteudo(&mut saida, &cliente, &cfg, &token).await;

        // Nada deve ter sido modificado (flag false).
        assert!(saida.resultados[0].conteudo.is_none());
        assert_eq!(saida.metadados.fetches_simultaneos, 0);
    }

    #[test]
    fn extrair_host_url_valida_retorna_host() {
        assert_eq!(extrair_host("https://www.example.com/a"), "www.example.com");
        assert_eq!(extrair_host("https://API.test/x"), "api.test");
    }

    #[test]
    fn extrair_host_url_invalida_retorna_unknown() {
        assert_eq!(extrair_host("nao-eh-url"), "unknown");
        assert_eq!(extrair_host(""), "unknown");
    }

    #[tokio::test]
    async fn obter_semaforo_para_host_cria_uma_vez_por_host() {
        let mapa: MapaSemaforosPorHost = Arc::new(Mutex::new(HashMap::new()));
        let sema_a1 = obter_semaforo_para_host(&mapa, "a.com", 3).await;
        let sema_a2 = obter_semaforo_para_host(&mapa, "a.com", 99).await;
        // O segundo acesso deve retornar o MESMO semáforo (limite inicial 3 preservado).
        assert!(Arc::ptr_eq(&sema_a1, &sema_a2));
        assert_eq!(sema_a1.available_permits(), 3);

        let sema_b = obter_semaforo_para_host(&mapa, "b.com", 5).await;
        assert!(!Arc::ptr_eq(&sema_a1, &sema_b));
        assert_eq!(sema_b.available_permits(), 5);

        let mapa_guardado = mapa.lock().await;
        assert_eq!(mapa_guardado.len(), 2);
    }

    #[tokio::test]
    async fn obter_semaforo_limita_concorrencia_simultanea_no_mesmo_host() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        let mapa: MapaSemaforosPorHost = Arc::new(Mutex::new(HashMap::new()));
        let contador_simultaneo = Arc::new(AtomicUsize::new(0));
        let pico_simultaneo = Arc::new(AtomicUsize::new(0));

        let mut tarefas = Vec::new();
        for _ in 0..10 {
            let mapa = Arc::clone(&mapa);
            let contador = Arc::clone(&contador_simultaneo);
            let pico = Arc::clone(&pico_simultaneo);
            tarefas.push(tokio::spawn(async move {
                let sema = obter_semaforo_para_host(&mapa, "same-host.com", 2).await;
                let _permit = sema.acquire_owned().await.expect("permit");
                let atual = contador.fetch_add(1, Ordering::SeqCst) + 1;
                let mut p = pico.load(Ordering::SeqCst);
                while atual > p {
                    match pico.compare_exchange(p, atual, Ordering::SeqCst, Ordering::SeqCst) {
                        Ok(_) => break,
                        Err(novo) => p = novo,
                    }
                }
                tokio::time::sleep(std::time::Duration::from_millis(30)).await;
                contador.fetch_sub(1, Ordering::SeqCst);
            }));
        }
        for t in tarefas {
            let _ = t.await;
        }
        assert!(
            pico_simultaneo.load(Ordering::SeqCst) <= 2,
            "pico simultâneo {} excedeu limite 2",
            pico_simultaneo.load(Ordering::SeqCst)
        );
    }

    #[tokio::test]
    async fn enriquecer_com_conteudo_cancelado_marca_falhas() {
        let cliente = reqwest::Client::builder()
            .timeout(std::time::Duration::from_millis(100))
            .build()
            .unwrap();
        let cfg = configuracoes_fetch(2, 1000);
        let mut saida = saida_vazia();
        for i in 0..3 {
            saida.resultados.push(ResultadoBusca {
                posicao: (i + 1) as u32,
                titulo: format!("r{i}"),
                url: format!("http://127.0.0.1:1/{i}"),
                url_exibicao: None,
                snippet: None,
                titulo_original: None,
                conteudo: None,
                tamanho_conteudo: None,
                metodo_extracao_conteudo: None,
            });
        }

        let token = CancellationToken::new();
        token.cancel();
        enriquecer_com_conteudo(&mut saida, &cliente, &cfg, &token).await;

        // Nenhum sucesso esperado (cancelado antes).
        assert_eq!(saida.metadados.sucessos_fetch, 0);
    }
}
