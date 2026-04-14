//! Testes de integração para `pipeline::executar_pipeline` e `parallel::*`.
//!
//! Cobre os caminhos de maior custo do fluxo multi-query:
//! - Barrier (JoinSet) quando `modo_stream = false`.
//! - Streaming (mpsc) quando `modo_stream = true`.
//! - Single-query com `modo_stream = true` (warn + fallback).
//! - Erros de lista vazia.
//! - Helpers puros de dedup e leitura de arquivo.
//!
//! Todos os testes usam `wiremock` — ZERO chamadas HTTP reais.

use duckduckgo_search_cli::pipeline::{
    combinar_e_deduplicar_queries, executar_pipeline, ler_queries_de_arquivo, ResultadoPipeline,
};
use duckduckgo_search_cli::types::{Configuracoes, Endpoint, FormatoSaida, SafeSearch};
use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::Duration;
use tokio::sync::Mutex as TokioMutex;
use tokio_util::sync::CancellationToken;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// Mutex async para serializar testes que manipulam env vars (std::env não é thread-safe).
fn env_lock() -> &'static TokioMutex<()> {
    static LOCK: OnceLock<TokioMutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| TokioMutex::new(()))
}

/// RAII guard para env vars — limpa ao sair.
struct GuardaEnv {
    chaves: Vec<&'static str>,
}
impl GuardaEnv {
    fn set(pares: &[(&'static str, String)]) -> Self {
        let mut chs = Vec::new();
        for (k, v) in pares {
            std::env::set_var(k, v);
            chs.push(*k);
        }
        GuardaEnv { chaves: chs }
    }
}
impl Drop for GuardaEnv {
    fn drop(&mut self) {
        for k in &self.chaves {
            std::env::remove_var(k);
        }
    }
}

fn cfg_multi(queries: Vec<String>, formato: FormatoSaida, stream: bool) -> Configuracoes {
    Configuracoes {
        query: queries.first().cloned().unwrap_or_default(),
        queries,
        num_resultados: None,
        formato,
        timeout_segundos: 5,
        idioma: "pt".to_string(),
        pais: "br".to_string(),
        modo_verboso: false,
        modo_silencioso: true,
        user_agent: "Mozilla/5.0 (teste)".to_string(),
        paralelismo: 2,
        paginas: 1,
        retries: 0,
        endpoint: Endpoint::Html,
        filtro_temporal: None,
        safe_search: SafeSearch::Moderate,
        modo_stream: stream,
        arquivo_saida: None,
        buscar_conteudo: false,
        max_tamanho_conteudo: 10_000,
        proxy: None,
        sem_proxy: false,
        timeout_global_segundos: 60,
        corresponde_plataforma_ua: false,
        limite_por_host: 2,
        caminho_chrome: None,
        seletores: std::sync::Arc::new(
            duckduckgo_search_cli::types::ConfiguracaoSeletores::default(),
        ),
    }
}

/// HTML mínimo com 2 resultados — suficiente para Estratégia 1.
fn html_2_resultados(titulo_a: &str, titulo_b: &str) -> String {
    format!(
        r#"<html><body><div id="links">
          <div class="result">
            <a class="result__a" href="//exemplo.com/a">{titulo_a}</a>
            <a class="result__snippet">snippet A</a>
            <span class="result__url">exemplo.com/a</span>
          </div>
          <div class="result">
            <a class="result__a" href="//exemplo.com/b">{titulo_b}</a>
            <a class="result__snippet">snippet B</a>
            <span class="result__url">exemplo.com/b</span>
          </div>
        </div></body></html>"#
    )
}

// ---------------------------------------------------------------------------
// T1: multi-query em modo barrier — exercita `executar_buscas_paralelas`
//     e JoinSet com staggered launch.
// ---------------------------------------------------------------------------
#[tokio::test]
async fn pipeline_multi_query_barrier_agrega_resultados() {
    let _g = env_lock().lock().await;
    let mock = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(html_2_resultados("Primeiro", "Segundo"))
                .insert_header("content-type", "text/html; charset=utf-8"),
        )
        .mount(&mock)
        .await;

    let base = format!("{}/", mock.uri());
    let _env = GuardaEnv::set(&[
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_HTML", base.clone()),
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_LITE", base),
    ]);

    let cfg = cfg_multi(
        vec!["rust".to_string(), "tokio".to_string()],
        FormatoSaida::Json,
        false,
    );
    let token = CancellationToken::new();

    let res = executar_pipeline(cfg, token)
        .await
        .expect("pipeline multi-query barrier deve ter sucesso");

    match res {
        ResultadoPipeline::Multipla(multi) => {
            assert_eq!(multi.quantidade_queries, 2, "2 queries executadas");
            assert_eq!(multi.buscas.len(), 2);
            assert!(multi.buscas.iter().all(|s| s.quantidade_resultados >= 2));
        }
        outro => panic!("esperava Multipla, recebeu: {outro:?}"),
    }
}

// ---------------------------------------------------------------------------
// T2: multi-query em modo streaming — exercita `executar_buscas_paralelas_streaming`
//     + consumer via mpsc + emissão NDJSON.
// ---------------------------------------------------------------------------
#[tokio::test]
async fn pipeline_multi_query_streaming_drena_e_retorna_stats() {
    let _g = env_lock().lock().await;
    let mock = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(html_2_resultados("Alpha", "Beta"))
                .insert_header("content-type", "text/html; charset=utf-8"),
        )
        .mount(&mock)
        .await;

    let base = format!("{}/", mock.uri());
    let _env = GuardaEnv::set(&[
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_HTML", base.clone()),
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_LITE", base),
    ]);

    // Arquivo de saída para não poluir stdout durante teste.
    let tmp = tempfile::NamedTempFile::new().expect("tempfile");
    let mut cfg = cfg_multi(
        vec!["a".to_string(), "b".to_string(), "c".to_string()],
        FormatoSaida::Json,
        true,
    );
    cfg.arquivo_saida = Some(tmp.path().to_path_buf());

    let token = CancellationToken::new();

    let res = tokio::time::timeout(Duration::from_secs(30), executar_pipeline(cfg, token))
        .await
        .expect("pipeline não deve pendurar")
        .expect("pipeline streaming deve ter sucesso");

    match res {
        ResultadoPipeline::Stream(stats) => {
            assert_eq!(stats.total, 3, "3 queries processadas no stream");
            assert!(stats.sucessos + stats.erros == stats.total);
        }
        outro => panic!("esperava Stream, recebeu: {outro:?}"),
    }

    // Valida que NDJSON foi escrito: 3 linhas JSON válidas.
    let conteudo = std::fs::read_to_string(tmp.path()).expect("ler arquivo de saída");
    let linhas: Vec<&str> = conteudo.lines().filter(|l| !l.trim().is_empty()).collect();
    assert_eq!(linhas.len(), 3, "3 linhas NDJSON (uma por query)");
    for linha in &linhas {
        let _: serde_json::Value = serde_json::from_str(linha).expect("linha NDJSON válida");
    }
}

// ---------------------------------------------------------------------------
// T3: single-query com modo_stream=true — branch que emite warn + fallback agregado.
// ---------------------------------------------------------------------------
#[tokio::test]
async fn pipeline_single_query_com_stream_emite_warn_e_fallback_agregado() {
    let _g = env_lock().lock().await;
    let mock = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(html_2_resultados("Único", "Segundo"))
                .insert_header("content-type", "text/html; charset=utf-8"),
        )
        .mount(&mock)
        .await;

    let base = format!("{}/", mock.uri());
    let _env = GuardaEnv::set(&[
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_HTML", base.clone()),
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_LITE", base),
    ]);

    let cfg = cfg_multi(vec!["solo".to_string()], FormatoSaida::Json, true);
    let token = CancellationToken::new();

    let res = executar_pipeline(cfg, token)
        .await
        .expect("single + stream deve cair em Unica com warn");

    match res {
        ResultadoPipeline::Unica(saida) => {
            assert_eq!(saida.query, "solo");
            assert!(saida.quantidade_resultados >= 2);
        }
        outro => panic!("esperava Unica, recebeu: {outro:?}"),
    }
}

// ---------------------------------------------------------------------------
// T4: queries vazias — deve retornar erro, não panic.
// ---------------------------------------------------------------------------
#[tokio::test]
async fn pipeline_com_queries_vazias_retorna_erro() {
    let cfg = cfg_multi(vec![], FormatoSaida::Json, false);
    let token = CancellationToken::new();
    let res = executar_pipeline(cfg, token).await;
    assert!(res.is_err(), "lista vazia deve produzir erro");
    let msg = format!("{}", res.unwrap_err());
    assert!(
        msg.contains("nenhuma query"),
        "mensagem deve citar 'nenhuma query', foi: {msg}"
    );
}

// ---------------------------------------------------------------------------
// T5: combinar_e_deduplicar_queries — dedup preservando ordem e filtrando vazios.
// ---------------------------------------------------------------------------
#[test]
fn combinar_queries_preserva_ordem_dedup_e_filtra_vazios() {
    let r = combinar_e_deduplicar_queries(
        vec!["rust".into(), "  ".into(), "tokio".into()],
        vec!["rust".into(), "serde".into()],
        vec!["".into(), "serde".into(), "axum".into()],
    );
    assert_eq!(r, vec!["rust", "tokio", "serde", "axum"]);
}

#[test]
fn combinar_queries_lista_totalmente_vazia_retorna_vec_vazio() {
    let r = combinar_e_deduplicar_queries(vec![], vec![], vec!["   ".into(), "\n".into()]);
    assert!(r.is_empty());
}

#[test]
fn combinar_queries_trim_em_cada_entrada() {
    let r = combinar_e_deduplicar_queries(
        vec!["  rust  ".into()],
        vec!["\ttokio\n".into()],
        vec![" rust ".into()],
    );
    // "  rust  " e " rust " após trim são iguais → dedup.
    assert_eq!(r, vec!["rust", "tokio"]);
}

// ---------------------------------------------------------------------------
// T6: ler_queries_de_arquivo — LF, CRLF e linhas em branco.
// ---------------------------------------------------------------------------
#[test]
fn ler_queries_de_arquivo_trata_crlf_e_linhas_vazias() {
    let tmp = tempfile::NamedTempFile::new().expect("tempfile");
    // Mistura LF e CRLF + linhas vazias.
    std::fs::write(tmp.path(), "rust\r\n\r\n  tokio  \nserde\n\n").expect("escrever");
    let qs = ler_queries_de_arquivo(tmp.path()).expect("ler ok");
    assert_eq!(qs, vec!["rust", "tokio", "serde"]);
}

#[test]
fn ler_queries_de_arquivo_inexistente_retorna_erro() {
    let inexistente = PathBuf::from("/tmp/duckduckgo-search-cli-file-nao-existe-xyz-123.txt");
    let r = ler_queries_de_arquivo(&inexistente);
    assert!(r.is_err(), "arquivo inexistente deve falhar");
}

#[test]
fn ler_queries_de_arquivo_vazio_retorna_vec_vazio() {
    let tmp = tempfile::NamedTempFile::new().expect("tempfile");
    std::fs::write(tmp.path(), "").expect("escrever");
    let qs = ler_queries_de_arquivo(tmp.path()).expect("ok");
    assert!(qs.is_empty());
}
