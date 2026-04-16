//! Testes de integração para `parallel.rs` — multi-query com `wiremock`.
//!
//! ZERO chamadas HTTP reais. Cada teste sobe `MockServer` em porta aleatória e
//! aponta `DUCKDUCKGO_SEARCH_CLI_BASE_URL_HTML`/`_LITE` para ele. A serialização
//! contra outros testes que mexem em env vars é feita via `env_lock()` async.
//!
//! Cobre:
//! - Happy path multi-query (`executar_buscas_paralelas` com N queries em sucesso).
//! - Happy path streaming (`executar_buscas_paralelas_streaming` consumindo via mpsc).
//! - Streaming com consumer fechado → tasks remanescentes são abortadas via `abort_all`.
//! - `paginas > 1` força construção de Client isolado por task (paths 138-146 e 342-350).

use duckduckgo_search_cli::parallel::{
    executar_buscas_paralelas, executar_buscas_paralelas_streaming,
};
use duckduckgo_search_cli::types::{
    ConfiguracaoSeletores, Configuracoes, Endpoint, FormatoSaida, SafeSearch,
};
use std::sync::{Arc, OnceLock};
use std::time::Duration;
use tokio::sync::{mpsc, Mutex as TokioMutex};
use tokio_util::sync::CancellationToken;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// Mutex async global para serializar testes que manipulam env vars.
/// `std::env::set_var` não é thread-safe; cada teste adquire o lock antes de
/// configurar `DUCKDUCKGO_SEARCH_CLI_BASE_URL_*`.
fn env_lock() -> &'static TokioMutex<()> {
    static LOCK: OnceLock<TokioMutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| TokioMutex::new(()))
}

/// Guard que define env vars no `set` e remove no `Drop`. Evita vazamento entre
/// testes serializados pelo `env_lock()`.
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

/// Helper que monta uma `Configuracoes` enxuta para os testes de paralelismo.
/// `paginas` controla decisão Client compartilhado vs isolado (seção 4.3).
fn configuracoes_teste_wm(
    endpoint: Endpoint,
    paginas: u32,
    queries: Vec<String>,
    paralelismo: u32,
) -> Configuracoes {
    let primeira = queries.first().cloned().unwrap_or_default();
    Configuracoes {
        query: primeira,
        queries,
        num_resultados: None,
        formato: FormatoSaida::Json,
        timeout_segundos: 5,
        idioma: "pt".to_string(),
        pais: "br".to_string(),
        modo_verboso: false,
        modo_silencioso: true,
        user_agent: "Mozilla/5.0 (teste-parallel)".to_string(),
        perfil_browser: duckduckgo_search_cli::http::criar_perfil_browser("Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/146.0.0.0 Safari/537.36"),
        paralelismo,
        paginas,
        retries: 0,
        endpoint,
        filtro_temporal: None,
        safe_search: SafeSearch::Moderate,
        modo_stream: false,
        arquivo_saida: None,
        buscar_conteudo: false,
        max_tamanho_conteudo: 10_000,
        proxy: None,
        sem_proxy: true, // evita herdar proxy do ambiente em CI
        timeout_global_segundos: 60,
        corresponde_plataforma_ua: false,
        limite_por_host: 2,
        caminho_chrome: None,
        seletores: Arc::new(ConfiguracaoSeletores::default()),
    }
}

/// HTML com 2 resultados orgânicos com corpo acima de 5 000 bytes (limiar anti-bloqueio).
fn html_dois_resultados() -> String {
    // Padding garante que o corpo fique acima de LIMIAR_BLOQUEIO_SILENCIOSO (5 000 bytes).
    let padding = "<!-- padding para superar o limiar de detecção de bloqueio silencioso do DuckDuckGo. Este comentário é apenas preenchimento e não afeta a extração de resultados. -->".repeat(30);
    format!(
        r#"<html><body>
    {padding}
    <div id="links">
      <div class="result">
        <a class="result__a" href="//exemplo.com/a">Resultado A</a>
        <a class="result__snippet">Snippet A com texto suficiente para passar nos filtros padrão.</a>
        <span class="result__url">exemplo.com/a</span>
      </div>
      <div class="result">
        <a class="result__a" href="//exemplo.com/b">Resultado B</a>
        <a class="result__snippet">Snippet B com texto suficiente para passar nos filtros padrão.</a>
        <span class="result__url">exemplo.com/b</span>
      </div>
    </div>
    </body></html>"#
    )
}

/// HTML com tokens vqd/s/dc para paginação — corpo acima de 5 000 bytes (limiar anti-bloqueio).
fn html_pagina_com_tokens(vqd: &str, s: &str, dc: &str, prefixo: &str) -> String {
    // Padding garante que o corpo fique acima de LIMIAR_BLOQUEIO_SILENCIOSO (5 000 bytes).
    let padding = "<!-- padding para superar o limiar de detecção de bloqueio silencioso do DuckDuckGo. Este comentário é apenas preenchimento e não afeta a extração de resultados. -->".repeat(30);
    format!(
        r#"<html><body>
        {padding}
        <form><input name="vqd" value="{vqd}"><input name="s" value="{s}"><input name="dc" value="{dc}"></form>
        <div id="links">
          <div class="result">
            <a class="result__a" href="//exemplo.com/{prefixo}-1">{prefixo} Um</a>
            <a class="result__snippet">snippet de {prefixo} um com tamanho suficiente.</a>
          </div>
          <div class="result">
            <a class="result__a" href="//exemplo.com/{prefixo}-2">{prefixo} Dois</a>
            <a class="result__snippet">snippet de {prefixo} dois com tamanho suficiente.</a>
          </div>
        </div>
        </body></html>"#
    )
}

// ---------------------------------------------------------------------------
// Teste 1: Happy path multi-query — 3 queries, paralelismo 2, todas com sucesso.
// Cobre: spawn loop, semaphore acquire, client compartilhado (paginas=1),
// executar_query_com_cancelamento happy path, drop(permit), coleta ordenada.
// ---------------------------------------------------------------------------
#[tokio::test]
async fn multi_query_happy_path_3_queries_paralelismo_2() {
    let _g = env_lock().lock().await;
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(html_dois_resultados())
                .insert_header("content-type", "text/html; charset=utf-8"),
        )
        .mount(&mock_server)
        .await;

    let base = format!("{}/", mock_server.uri());
    let _env = GuardaEnv::set(&[
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_HTML", base.clone()),
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_LITE", base),
    ]);

    let queries = vec!["alpha".to_string(), "beta".to_string(), "gamma".to_string()];
    let cfg = configuracoes_teste_wm(Endpoint::Html, 1, queries.clone(), 2);
    let token = CancellationToken::new();

    let saida = executar_buscas_paralelas(queries, cfg, token)
        .await
        .expect("multi-query deve retornar Ok");

    assert_eq!(saida.quantidade_queries, 3);
    assert_eq!(saida.paralelismo, 2);
    assert_eq!(saida.buscas.len(), 3);

    // Ordem original DEVE ser preservada apesar do staggered launch.
    assert_eq!(saida.buscas[0].query, "alpha");
    assert_eq!(saida.buscas[1].query, "beta");
    assert_eq!(saida.buscas[2].query, "gamma");

    // Todas as queries devem ter sucesso (sem campo erro) e com 2 resultados cada.
    for busca in &saida.buscas {
        assert!(
            busca.erro.is_none(),
            "query {:?} deveria ter sucesso, mas falhou: {:?}",
            busca.query,
            busca.mensagem
        );
        assert_eq!(busca.quantidade_resultados, 2);
        assert_eq!(busca.resultados.len(), 2);
        assert_eq!(busca.paginas_buscadas, 1);
    }
}

// ---------------------------------------------------------------------------
// Teste 2: paginas > 1 → força construção de Client isolado por task.
// Cobre linhas 138-146 (branch `None => http::construir_cliente_com_proxy`).
// Apenas 1 query para manter o teste rápido; 2 páginas via tokens vqd.
// ---------------------------------------------------------------------------
#[tokio::test]
async fn multi_query_com_paginas_maior_que_1_usa_client_isolado() {
    let _g = env_lock().lock().await;
    let mock_server = MockServer::start().await;

    // Página 1 GET — retorna tokens para permitir POST de página 2.
    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(html_pagina_com_tokens("vqd-pg1", "0", "30", "P1"))
                .insert_header("content-type", "text/html; charset=utf-8"),
        )
        .mount(&mock_server)
        .await;

    // Página 2 POST — qualquer body POST com vqd serve.
    Mock::given(method("POST"))
        .and(path("/"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(html_pagina_com_tokens("vqd-pg2", "30", "60", "P2"))
                .insert_header("content-type", "text/html; charset=utf-8"),
        )
        .mount(&mock_server)
        .await;

    let base = format!("{}/", mock_server.uri());
    let _env = GuardaEnv::set(&[
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_HTML", base.clone()),
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_LITE", base),
    ]);

    let queries = vec!["query-multipagina".to_string()];
    // paginas = 2 ATIVA o branch de Client isolado (cliente_compartilhado = None).
    let cfg = configuracoes_teste_wm(Endpoint::Html, 2, queries.clone(), 1);
    let token = CancellationToken::new();

    let saida = executar_buscas_paralelas(queries, cfg, token)
        .await
        .expect("multi-query com paginas>1 deve retornar Ok");

    assert_eq!(saida.buscas.len(), 1);
    let busca = &saida.buscas[0];
    assert!(
        busca.erro.is_none(),
        "query deveria ter sucesso: {:?}",
        busca.mensagem
    );
    // 2 resultados por página x 2 páginas = 4.
    assert_eq!(busca.quantidade_resultados, 4);
    assert_eq!(busca.paginas_buscadas, 2);
}

// ---------------------------------------------------------------------------
// Teste 3: Streaming happy path — consumer recebe todos os resultados via mpsc
// e estatísticas refletem total/sucessos corretamente.
// Cobre: executar_buscas_paralelas_streaming completo até retorno
// `EstatisticasStream`, branch de envio bem-sucedido por canal.
// ---------------------------------------------------------------------------
#[tokio::test]
async fn streaming_happy_path_consumer_recebe_todos_resultados() {
    let _g = env_lock().lock().await;
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(html_dois_resultados())
                .insert_header("content-type", "text/html; charset=utf-8"),
        )
        .mount(&mock_server)
        .await;

    let base = format!("{}/", mock_server.uri());
    let _env = GuardaEnv::set(&[
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_HTML", base.clone()),
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_LITE", base),
    ]);

    let queries = vec!["s-um".to_string(), "s-dois".to_string()];
    let cfg = configuracoes_teste_wm(Endpoint::Html, 1, queries.clone(), 2);
    let token = CancellationToken::new();
    let (tx, mut rx) = mpsc::channel(8);

    // Consumer: drena o canal em paralelo ao produtor.
    let consumer = tokio::spawn(async move {
        let mut recebidos = Vec::new();
        while let Some((indice, saida)) = rx.recv().await {
            recebidos.push((indice, saida));
        }
        recebidos
    });

    let estatisticas = executar_buscas_paralelas_streaming(queries, cfg, token, tx)
        .await
        .expect("streaming deve retornar Ok");

    let recebidos = consumer.await.expect("consumer task deve concluir");

    assert_eq!(estatisticas.total, 2);
    assert_eq!(estatisticas.sucessos, 2);
    assert_eq!(estatisticas.erros, 0);
    assert_eq!(estatisticas.paralelismo, 2);
    assert!(!estatisticas.timestamp_inicio.is_empty());

    assert_eq!(recebidos.len(), 2, "consumer deve receber as 2 saídas");
    for (_indice, saida) in &recebidos {
        assert!(saida.erro.is_none(), "saída streaming deveria estar limpa");
        assert_eq!(saida.quantidade_resultados, 2);
    }
}

// ---------------------------------------------------------------------------
// Teste 4: Streaming com cancelamento ANTES do start — todas as queries
// retornam saída de erro e estatísticas marcam tudo como erro.
// Cobre branch de cancelamento dentro da task antes do `acquire_owned`.
// ---------------------------------------------------------------------------
#[tokio::test]
async fn streaming_cancelado_antes_do_start_marca_tudo_como_erro() {
    // Não toca em env — sem mocks porque tasks abortam antes de qualquer HTTP.
    let token = CancellationToken::new();
    token.cancel();

    let queries = vec!["a".to_string(), "b".to_string(), "c".to_string()];
    let cfg = configuracoes_teste_wm(Endpoint::Html, 1, queries.clone(), 3);
    let (tx, mut rx) = mpsc::channel(8);

    let consumer = tokio::spawn(async move {
        let mut recebidos = Vec::new();
        while let Some(item) = rx.recv().await {
            recebidos.push(item);
        }
        recebidos
    });

    let estatisticas = executar_buscas_paralelas_streaming(queries, cfg, token, tx)
        .await
        .expect("streaming cancelado deve retornar Ok com estatísticas");

    let recebidos = consumer.await.expect("consumer task deve concluir");

    assert_eq!(estatisticas.total, 3);
    assert_eq!(estatisticas.sucessos, 0);
    assert_eq!(estatisticas.erros, 3);
    assert_eq!(recebidos.len(), 3);
    for (_, saida) in &recebidos {
        assert!(saida.erro.is_some(), "saída cancelada deve ter erro");
    }
}

// ---------------------------------------------------------------------------
// Teste 5: Streaming com consumer fechando canal cedo → produtor detecta
// `send` falhando, chama `abort_all` e termina a função sem panicar.
// Cobre linhas 385-393 (branch `Err(erro_send)` + `abort_all` + `break`).
// ---------------------------------------------------------------------------
#[tokio::test]
async fn streaming_consumer_fechado_aborta_tasks_remanescentes() {
    let _g = env_lock().lock().await;
    let mock_server = MockServer::start().await;

    // Resposta lenta para garantir que algumas tasks ainda estejam em voo
    // quando o consumer fechar o canal. 200ms é suficiente porque o staggered
    // launch já espalha o início das tasks (DELAY_BASE_STAGGERED_MS = 200ms).
    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(html_dois_resultados())
                .insert_header("content-type", "text/html; charset=utf-8")
                .set_delay(Duration::from_millis(200)),
        )
        .mount(&mock_server)
        .await;

    let base = format!("{}/", mock_server.uri());
    let _env = GuardaEnv::set(&[
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_HTML", base.clone()),
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_LITE", base),
    ]);

    // Várias queries, paralelismo baixo → muitas pendentes quando rx é dropado.
    let queries: Vec<String> = (0..6).map(|i| format!("q-{i}")).collect();
    let cfg = configuracoes_teste_wm(Endpoint::Html, 1, queries.clone(), 2);
    let token = CancellationToken::new();
    let (tx, rx) = mpsc::channel(1);

    // Drop imediato do receiver: força `tx.send().await` a falhar assim que
    // o produtor tentar emitir o primeiro resultado, disparando `abort_all`.
    drop(rx);

    let estatisticas = executar_buscas_paralelas_streaming(queries, cfg, token, tx)
        .await
        .expect("streaming deve retornar Ok mesmo com consumer fechado");

    assert_eq!(estatisticas.total, 6);
    // Pelo menos 1 task pode ter contado como sucesso/erro antes do abort,
    // mas o total processado DEVE ser <= total enviado. O importante é que
    // a função NÃO panicou e retornou estatísticas consistentes.
    assert!(
        estatisticas.sucessos + estatisticas.erros <= 6,
        "soma sucessos+erros não pode ultrapassar total"
    );
}
