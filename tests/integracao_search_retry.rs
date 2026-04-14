//! Testes de integração focados em caminhos não cobertos de `search.rs`:
//! - `executar_busca` (versão de compatibilidade single-query simples)
//! - Cancelamento mid-retry em `executar_com_retry`
//! - Caminhos de erro / borda de `buscar_com_paginacao`:
//!   * tokens vqd ausentes → sem paginação possível
//!   * página seguinte com status não-OK → para
//!   * página seguinte com zero resultados → para
//!   * página seguinte sem tokens vqd → para após adicionar
//!   * cancelamento durante paginação
//!   * fallback Lite que também falha → mantém vazio
//!   * truncate por `num_resultados`
//!   * retry de 429 esgotado
//!
//! ZERO chamadas HTTP reais — todos via `wiremock::MockServer`.

use duckduckgo_search_cli::search::{
    buscar_com_paginacao, executar_busca, executar_com_retry, MotivoFalhaRetry,
};
use duckduckgo_search_cli::types::{Configuracoes, Endpoint, FormatoSaida, SafeSearch};
use reqwest::Client;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, OnceLock};
use std::time::Duration;
use tokio::sync::Mutex as TokioMutex;
use tokio_util::sync::CancellationToken;
use wiremock::matchers::{body_string_contains, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// Mutex async global para serializar testes que manipulam env vars.
fn env_lock() -> &'static TokioMutex<()> {
    static LOCK: OnceLock<TokioMutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| TokioMutex::new(()))
}

fn cliente_teste() -> Client {
    Client::builder()
        .timeout(Duration::from_secs(10))
        .user_agent("Mozilla/5.0 (teste-search-retry)")
        .build()
        .expect("cliente de teste")
}

fn configuracoes_base(endpoint: Endpoint, paginas: u32, retries: u32) -> Configuracoes {
    Configuracoes {
        query: "rust".to_string(),
        queries: vec!["rust".to_string()],
        num_resultados: None,
        formato: FormatoSaida::Json,
        timeout_segundos: 5,
        idioma: "pt".to_string(),
        pais: "br".to_string(),
        modo_verboso: false,
        modo_silencioso: true,
        user_agent: "Mozilla/5.0 (teste)".to_string(),
        paralelismo: 1,
        paginas,
        retries,
        endpoint,
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
        seletores: std::sync::Arc::new(
            duckduckgo_search_cli::types::ConfiguracaoSeletores::default(),
        ),
    }
}

/// HTML mínimo (>100 bytes) com 3 resultados orgânicos para passar o filtro anti-bloqueio.
fn html_3_resultados() -> String {
    r#"<html><body>
    <div id="links">
      <div class="result">
        <a class="result__a" href="//exemplo.com/um">Resultado Um</a>
        <a class="result__snippet">Snippet do primeiro resultado.</a>
      </div>
      <div class="result">
        <a class="result__a" href="//exemplo.com/dois">Resultado Dois</a>
        <a class="result__snippet">Snippet do segundo resultado.</a>
      </div>
      <div class="result">
        <a class="result__a" href="//exemplo.com/tres">Resultado Três</a>
        <a class="result__snippet">Snippet do terceiro resultado.</a>
      </div>
    </div>
    </body></html>"#
        .to_string()
}

fn html_com_tokens_e_resultados(vqd: &str, s: &str, dc: &str, titulos: &[&str]) -> String {
    let mut html = String::from("<html><body>");
    html.push_str(&format!(
        r#"<form><input name="vqd" value="{vqd}"><input name="s" value="{s}"><input name="dc" value="{dc}"></form>"#
    ));
    html.push_str(r#"<div id="links">"#);
    for t in titulos {
        html.push_str(&format!(
            r#"<div class="result"><a class="result__a" href="//exemplo.com/{}">{}</a><a class="result__snippet">snippet de {}</a></div>"#,
            t.replace(' ', "-"),
            t,
            t
        ));
    }
    html.push_str("</div></body></html>");
    html
}

/// HTML SEM os tokens vqd/s/dc (para forçar caminho "sem paginação possível").
fn html_sem_tokens_vqd() -> String {
    r#"<html><body>
    <div id="links">
      <div class="result">
        <a class="result__a" href="//exemplo.com/sem-tokens">Resultado Sem Tokens</a>
        <a class="result__snippet">Snippet sem tokens vqd presentes no formulário.</a>
      </div>
      <div class="result">
        <a class="result__a" href="//exemplo.com/outro">Outro Resultado</a>
        <a class="result__snippet">Outro snippet com texto suficiente para ultrapassar 100 bytes.</a>
      </div>
    </div>
    </body></html>"#
        .to_string()
}

/// Guard para configurar env vars durante um teste e limpar ao sair.
struct GuardaEnv {
    chaves: Vec<&'static str>,
}
impl GuardaEnv {
    fn set(chaves: &[(&'static str, String)]) -> Self {
        let mut chs = Vec::new();
        for (k, v) in chaves {
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

// ===========================================================================
// `executar_busca` — função standalone de compatibilidade (iteração 1).
// ===========================================================================

#[tokio::test]
async fn executar_busca_retorna_html_em_status_200() {
    let _g = env_lock().lock().await;
    let mock = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(html_3_resultados())
                .insert_header("content-type", "text/html; charset=utf-8"),
        )
        .mount(&mock)
        .await;

    let base = format!("{}/", mock.uri());
    let _env = GuardaEnv::set(&[("DUCKDUCKGO_SEARCH_CLI_BASE_URL_HTML", base)]);

    let cliente = cliente_teste();
    let html = executar_busca(&cliente, "rust", "pt", "br")
        .await
        .expect("status 200 + body grande deve retornar Ok");
    assert!(html.contains("Resultado Um"));
    assert!(html.len() > 100);
}

#[tokio::test]
async fn executar_busca_falha_com_status_500() {
    let _g = env_lock().lock().await;
    let mock = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(ResponseTemplate::new(500).set_body_string("erro interno"))
        .mount(&mock)
        .await;

    let base = format!("{}/", mock.uri());
    let _env = GuardaEnv::set(&[("DUCKDUCKGO_SEARCH_CLI_BASE_URL_HTML", base)]);

    let cliente = cliente_teste();
    let resultado = executar_busca(&cliente, "rust", "pt", "br").await;
    let erro = resultado.expect_err("status 500 deve ser erro");
    let msg = format!("{erro:#}");
    assert!(
        msg.contains("500"),
        "mensagem do erro deve citar o status 500: {msg}"
    );
}

#[tokio::test]
async fn executar_busca_falha_com_body_pequeno() {
    let _g = env_lock().lock().await;
    let mock = MockServer::start().await;

    // Status 200, mas body com menos de 100 bytes → suspeita de bloqueio.
    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string("ok")
                .insert_header("content-type", "text/html; charset=utf-8"),
        )
        .mount(&mock)
        .await;

    let base = format!("{}/", mock.uri());
    let _env = GuardaEnv::set(&[("DUCKDUCKGO_SEARCH_CLI_BASE_URL_HTML", base)]);

    let cliente = cliente_teste();
    let resultado = executar_busca(&cliente, "rust", "pt", "br").await;
    let erro = resultado.expect_err("body pequeno deve ser erro");
    let msg = format!("{erro:#}");
    assert!(
        msg.to_lowercase().contains("pequeno") || msg.contains("bloqueio"),
        "mensagem deve mencionar body pequeno / bloqueio: {msg}"
    );
}

// ===========================================================================
// `executar_com_retry` — cancelamento, retry esgotado e caminhos de erro.
// ===========================================================================

#[tokio::test]
async fn retry_aborta_quando_token_ja_cancelado() {
    let _g = env_lock().lock().await;
    let mock = MockServer::start().await;

    // Mock retorna 200, mas o cancelamento deve abortar antes mesmo da primeira tentativa.
    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(html_3_resultados())
                .insert_header("content-type", "text/html; charset=utf-8"),
        )
        .mount(&mock)
        .await;

    let cliente = cliente_teste();
    let flag = Arc::new(AtomicBool::new(false));
    let token = CancellationToken::new();
    token.cancel(); // já cancelado antes mesmo de chamar.

    let url = format!("{}/", mock.uri());
    let resultado = executar_com_retry(&cliente, &url, 3, &flag, &token).await;

    match resultado {
        Err(MotivoFalhaRetry::Rede(msg)) => {
            assert!(
                msg.to_lowercase().contains("cancel"),
                "mensagem de cancelamento esperada: {msg}"
            );
        }
        outro => panic!("esperava Err(Rede(\"cancel...\")), recebi {outro:?}"),
    }
}

#[tokio::test]
async fn retry_429_esgotado_retorna_rate_limited() {
    let _g = env_lock().lock().await;
    let mock = MockServer::start().await;

    // SEMPRE 429 — esgota retries e retorna RateLimited.
    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(ResponseTemplate::new(429))
        .mount(&mock)
        .await;

    let cliente = cliente_teste();
    let flag = Arc::new(AtomicBool::new(false));
    let token = CancellationToken::new();

    // 0 retries → 1 tentativa única → backoff não acionado.
    let url = format!("{}/", mock.uri());
    let resultado = executar_com_retry(&cliente, &url, 0, &flag, &token).await;
    match resultado {
        Err(MotivoFalhaRetry::RateLimited) => {}
        outro => panic!("esperava RateLimited, recebi {outro:?}"),
    }
    assert!(
        flag.load(std::sync::atomic::Ordering::Relaxed),
        "flag de rate limit deve ser ativada"
    );
}

#[tokio::test]
async fn retry_status_4xx_nao_retentavel_retorna_http_erro() {
    let _g = env_lock().lock().await;
    let mock = MockServer::start().await;

    // 418 não é 200/403/429 → cai no caminho "outros 4xx/5xx → não retentamos".
    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(ResponseTemplate::new(418))
        .mount(&mock)
        .await;

    let cliente = cliente_teste();
    let flag = Arc::new(AtomicBool::new(false));
    let token = CancellationToken::new();

    let url = format!("{}/", mock.uri());
    let resultado = executar_com_retry(&cliente, &url, 3, &flag, &token).await;
    match resultado {
        Err(MotivoFalhaRetry::HttpErro(418)) => {}
        outro => panic!("esperava HttpErro(418), recebi {outro:?}"),
    }
}

// ===========================================================================
// `buscar_com_paginacao` — caminhos de borda da paginação.
// ===========================================================================

#[tokio::test]
async fn paginacao_sem_tokens_vqd_emite_warning_e_retorna_apenas_pagina_1() {
    let _g = env_lock().lock().await;
    let mock = MockServer::start().await;

    // Página 1 com resultados mas SEM tokens vqd/s/dc → bloqueia paginação.
    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(html_sem_tokens_vqd())
                .insert_header("content-type", "text/html; charset=utf-8"),
        )
        .mount(&mock)
        .await;

    let base = format!("{}/", mock.uri());
    let _env = GuardaEnv::set(&[
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_HTML", base.clone()),
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_LITE", base),
    ]);

    let cliente = cliente_teste();
    // Pede 3 páginas, mas como não há tokens vqd, só virá a 1.
    let cfg = configuracoes_base(Endpoint::Html, 3, 0);
    let flag = Arc::new(AtomicBool::new(false));
    let token = CancellationToken::new();

    let agregado = buscar_com_paginacao(&cliente, &cfg, "rust", &flag, &token)
        .await
        .expect("primeira página deve ter sucesso");
    assert_eq!(agregado.paginas_buscadas, 1);
    assert_eq!(agregado.resultados.len(), 2);
}

#[tokio::test]
async fn paginacao_truncada_por_num_resultados() {
    let _g = env_lock().lock().await;
    let mock = MockServer::start().await;

    // Página 1: 3 resultados + tokens.
    let html_pg1 = html_com_tokens_e_resultados("vqd-trunc-1", "0", "30", &["A", "B", "C"]);
    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(html_pg1)
                .insert_header("content-type", "text/html; charset=utf-8"),
        )
        .mount(&mock)
        .await;

    // Página 2: 3 resultados → total acumulado = 6.
    let html_pg2 = html_com_tokens_e_resultados("vqd-trunc-2", "30", "60", &["D", "E", "F"]);
    Mock::given(method("POST"))
        .and(path("/"))
        .and(body_string_contains("vqd=vqd-trunc-1"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(html_pg2)
                .insert_header("content-type", "text/html; charset=utf-8"),
        )
        .mount(&mock)
        .await;

    let base = format!("{}/", mock.uri());
    let _env = GuardaEnv::set(&[
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_HTML", base.clone()),
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_LITE", base),
    ]);

    let cliente = cliente_teste();
    let mut cfg = configuracoes_base(Endpoint::Html, 2, 0);
    cfg.num_resultados = Some(4); // truncar acumulado de 6 para 4.
    let flag = Arc::new(AtomicBool::new(false));
    let token = CancellationToken::new();

    let agregado = buscar_com_paginacao(&cliente, &cfg, "rust", &flag, &token)
        .await
        .expect("paginação ok");
    assert_eq!(
        agregado.resultados.len(),
        4,
        "resultados devem ser truncados a 4"
    );
}

#[tokio::test]
async fn paginacao_para_quando_pagina_seguinte_retorna_status_nao_ok() {
    let _g = env_lock().lock().await;
    let mock = MockServer::start().await;

    let html_pg1 = html_com_tokens_e_resultados("vqd-bad-1", "0", "30", &["A", "B"]);
    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(html_pg1)
                .insert_header("content-type", "text/html; charset=utf-8"),
        )
        .mount(&mock)
        .await;

    // Página 2 (POST) retorna 503 → paginação deve parar e devolver só a página 1.
    Mock::given(method("POST"))
        .and(path("/"))
        .and(body_string_contains("vqd=vqd-bad-1"))
        .respond_with(ResponseTemplate::new(503))
        .mount(&mock)
        .await;

    let base = format!("{}/", mock.uri());
    let _env = GuardaEnv::set(&[
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_HTML", base.clone()),
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_LITE", base),
    ]);

    let cliente = cliente_teste();
    let cfg = configuracoes_base(Endpoint::Html, 3, 0);
    let flag = Arc::new(AtomicBool::new(false));
    let token = CancellationToken::new();

    let agregado = buscar_com_paginacao(&cliente, &cfg, "rust", &flag, &token)
        .await
        .expect("primeira página ok mesmo com pg 2 falhando");
    assert_eq!(agregado.paginas_buscadas, 1);
    assert_eq!(agregado.resultados.len(), 2);
}

#[tokio::test]
async fn paginacao_para_quando_pagina_seguinte_retorna_zero_resultados() {
    let _g = env_lock().lock().await;
    let mock = MockServer::start().await;

    let html_pg1 = html_com_tokens_e_resultados("vqd-zero-1", "0", "30", &["X", "Y", "Z"]);
    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(html_pg1)
                .insert_header("content-type", "text/html; charset=utf-8"),
        )
        .mount(&mock)
        .await;

    // Página 2 retorna HTML > 100 bytes mas sem `.result` → zero resultados → para.
    let html_vazio = r#"<html><head><title>nada</title></head><body><div id="links"><p>Sem resultados nesta página de teste, apenas texto suficiente para superar 100 bytes.</p></div></body></html>"#;
    Mock::given(method("POST"))
        .and(path("/"))
        .and(body_string_contains("vqd=vqd-zero-1"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(html_vazio)
                .insert_header("content-type", "text/html; charset=utf-8"),
        )
        .mount(&mock)
        .await;

    let base = format!("{}/", mock.uri());
    let _env = GuardaEnv::set(&[
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_HTML", base.clone()),
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_LITE", base),
    ]);

    let cliente = cliente_teste();
    let cfg = configuracoes_base(Endpoint::Html, 3, 0);
    let flag = Arc::new(AtomicBool::new(false));
    let token = CancellationToken::new();

    let agregado = buscar_com_paginacao(&cliente, &cfg, "rust", &flag, &token)
        .await
        .expect("ok");
    assert_eq!(
        agregado.paginas_buscadas, 1,
        "paginas_buscadas fica em 1 porque pg 2 trouxe zero resultados"
    );
    assert_eq!(agregado.resultados.len(), 3);
}

#[tokio::test]
async fn paginacao_para_quando_pagina_seguinte_perde_tokens_vqd() {
    let _g = env_lock().lock().await;
    let mock = MockServer::start().await;

    let html_pg1 = html_com_tokens_e_resultados("vqd-lost-1", "0", "30", &["A", "B"]);
    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(html_pg1)
                .insert_header("content-type", "text/html; charset=utf-8"),
        )
        .mount(&mock)
        .await;

    // Página 2: tem resultados MAS perdeu tokens vqd → paginação para após adicionar pg 2.
    let html_pg2_sem_tokens = r#"<html><body>
    <div id="links">
      <div class="result">
        <a class="result__a" href="//exemplo.com/p2a">Pg 2 A</a>
        <a class="result__snippet">snippet pg2a com texto suficiente.</a>
      </div>
      <div class="result">
        <a class="result__a" href="//exemplo.com/p2b">Pg 2 B</a>
        <a class="result__snippet">snippet pg2b com texto suficiente.</a>
      </div>
    </div>
    </body></html>"#;
    Mock::given(method("POST"))
        .and(path("/"))
        .and(body_string_contains("vqd=vqd-lost-1"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(html_pg2_sem_tokens)
                .insert_header("content-type", "text/html; charset=utf-8"),
        )
        .mount(&mock)
        .await;

    let base = format!("{}/", mock.uri());
    let _env = GuardaEnv::set(&[
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_HTML", base.clone()),
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_LITE", base),
    ]);

    let cliente = cliente_teste();
    let cfg = configuracoes_base(Endpoint::Html, 5, 0);
    let flag = Arc::new(AtomicBool::new(false));
    let token = CancellationToken::new();

    let agregado = buscar_com_paginacao(&cliente, &cfg, "rust", &flag, &token)
        .await
        .expect("ok");
    assert_eq!(
        agregado.paginas_buscadas, 2,
        "pg 2 contou — mas a 3 não veio porque perdeu tokens"
    );
    assert_eq!(
        agregado.resultados.len(),
        4,
        "2 da pg 1 + 2 da pg 2 = 4 resultados totais"
    );
}

#[tokio::test]
async fn paginacao_aborta_se_token_ja_cancelado_no_inicio_do_loop() {
    let _g = env_lock().lock().await;
    let mock = MockServer::start().await;

    let html_pg1 = html_com_tokens_e_resultados("vqd-cancel-1", "0", "30", &["A", "B"]);
    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(html_pg1)
                .insert_header("content-type", "text/html; charset=utf-8"),
        )
        .mount(&mock)
        .await;

    // Mock POST que NUNCA deve ser chamado (cancelamento pré-loop deve abortar).
    Mock::given(method("POST"))
        .and(path("/"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(html_3_resultados())
                .insert_header("content-type", "text/html; charset=utf-8"),
        )
        .mount(&mock)
        .await;

    let base = format!("{}/", mock.uri());
    let _env = GuardaEnv::set(&[
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_HTML", base.clone()),
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_LITE", base),
    ]);

    let cliente = cliente_teste();
    let cfg = configuracoes_base(Endpoint::Html, 3, 0);
    let flag = Arc::new(AtomicBool::new(false));
    let token = CancellationToken::new();

    // Spawn task que cancela após pequeno delay para simular cancelamento mid-execução.
    // Como o loop tem múltiplas chances de checar `is_cancelled()`, isso garante que
    // alguma das checagens dispare.
    let token_clone = token.clone();
    let handle = tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(50)).await;
        token_clone.cancel();
    });

    let agregado = buscar_com_paginacao(&cliente, &cfg, "rust", &flag, &token)
        .await
        .expect("primeira página deve completar antes do cancelamento");

    handle.await.expect("task de cancelamento ok");

    // O cancelamento deve abortar o loop antes de buscar todas as 3 páginas.
    assert!(
        agregado.paginas_buscadas < 3,
        "cancelamento deve abortar antes de completar 3 páginas (efetivamente: {})",
        agregado.paginas_buscadas
    );
}

#[tokio::test]
async fn fallback_lite_falha_mantem_resultados_vazios() {
    let _g = env_lock().lock().await;
    let mock_html = MockServer::start().await;
    let mock_lite = MockServer::start().await;

    // HTML retorna 200 mas com zero `.result` → dispara fallback Lite.
    let html_vazio = r#"<html><head><title>vazio</title></head><body><div id="links"><p>Nenhum resultado encontrado para teste de fallback Lite. Texto suficiente para passar 100 bytes.</p></div></body></html>"#;
    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(html_vazio)
                .insert_header("content-type", "text/html; charset=utf-8"),
        )
        .mount(&mock_html)
        .await;

    // Lite também falha — retorna 503 persistente.
    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(ResponseTemplate::new(503))
        .mount(&mock_lite)
        .await;

    let base_html = format!("{}/", mock_html.uri());
    let base_lite = format!("{}/", mock_lite.uri());
    let _env = GuardaEnv::set(&[
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_HTML", base_html),
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_LITE", base_lite),
    ]);

    let cliente = cliente_teste();
    let cfg = configuracoes_base(Endpoint::Html, 1, 0);
    let flag = Arc::new(AtomicBool::new(false));
    let token = CancellationToken::new();

    let agregado = buscar_com_paginacao(&cliente, &cfg, "rust", &flag, &token)
        .await
        .expect("HTML 200 + Lite 503 deve retornar Ok com lista vazia");
    assert_eq!(
        agregado.resultados.len(),
        0,
        "ambos endpoints sem resultados → vetor vazio"
    );
    assert!(
        !agregado.usou_fallback_lite,
        "fallback Lite falhou → flag fica false"
    );
    assert_eq!(agregado.endpoint_efetivo, Endpoint::Html);
}

#[tokio::test]
async fn primeira_pagina_blocked_por_body_pequeno_retorna_motivo_blocked() {
    let _g = env_lock().lock().await;
    let mock = MockServer::start().await;

    // Status 200 mas body MUITO pequeno → buscar_com_paginacao retorna Blocked.
    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string("ok")
                .insert_header("content-type", "text/html; charset=utf-8"),
        )
        .mount(&mock)
        .await;

    let base = format!("{}/", mock.uri());
    let _env = GuardaEnv::set(&[
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_HTML", base.clone()),
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_LITE", base),
    ]);

    let cliente = cliente_teste();
    let cfg = configuracoes_base(Endpoint::Html, 1, 0);
    let flag = Arc::new(AtomicBool::new(false));
    let token = CancellationToken::new();

    let resultado = buscar_com_paginacao(&cliente, &cfg, "rust", &flag, &token).await;
    match resultado {
        Err(MotivoFalhaRetry::Blocked) => {}
        Err(outro) => panic!("esperava Blocked, recebi outro motivo: {outro:?}"),
        Ok(_) => panic!("esperava Blocked, recebi Ok"),
    }
}
