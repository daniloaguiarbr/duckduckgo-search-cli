//! Testes de integração com `wiremock` — ZERO chamadas HTTP reais.
//!
//! Cada teste sobe um `MockServer` em porta aleatória e define URL base via
//! variável de ambiente lida por `search::url_base_html`/`url_base_lite`. A variável
//! é definida e limpa dentro do próprio teste. Todos os testes que manipulam env
//! vars executam serializados (por constraint implícito do `std::env::set_var`),
//! e cada teste usa paths distintos (ou o mesmo path `/`) para evitar interferência.

use duckduckgo_search_cli::search::{
    buscar_com_paginacao, executar_com_retry, extrair_tokens_paginacao, MotivoFalhaRetry,
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
/// `std::env::set_var` não é thread-safe; cada teste adquire o lock async-friendly.
fn env_lock() -> &'static TokioMutex<()> {
    static LOCK: OnceLock<TokioMutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| TokioMutex::new(()))
}

fn cliente_teste() -> Client {
    Client::builder()
        .timeout(Duration::from_secs(10))
        .user_agent("Mozilla/5.0 (teste)")
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

fn html_com_3_resultados_classe() -> String {
    r#"<html><body>
    <div id="links">
      <div class="result">
        <a class="result__a" href="//exemplo.com/um">Resultado Um</a>
        <a class="result__snippet">Descrição do primeiro resultado.</a>
        <span class="result__url">exemplo.com/um</span>
      </div>
      <div class="result">
        <a class="result__a" href="//exemplo.com/dois">Resultado Dois</a>
        <a class="result__snippet">Descrição do segundo resultado.</a>
      </div>
      <div class="result">
        <a class="result__a" href="//exemplo.com/tres">Resultado Três</a>
        <a class="result__snippet">Descrição do terceiro resultado.</a>
      </div>
      <div class="result result--ad">
        <a class="result__a" href="//anuncio.com">Anúncio Patrocinado</a>
      </div>
    </div>
    </body></html>"#
        .to_string()
}

fn html_com_tokens_vqd_e_resultados(vqd: &str, s: &str, dc: &str, titulos: &[&str]) -> String {
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

fn html_vazio_sem_result() -> String {
    // HTML sem `.result` mas com tamanho > 100 bytes para passar a sanidade anti-bloqueio.
    r#"<html><head><title>DuckDuckGo sem resultados</title></head><body><div id="links"><p>Nenhum resultado para esta busca específica foi encontrado aqui.</p></div></body></html>"#.to_string()
}

fn html_lite_tabela() -> String {
    r#"<html><body>
    <table>
      <tr><td valign="top">1.&nbsp;</td><td><a class="result-link" href="//exemplo.com/lite1">Lite Um</a></td></tr>
      <tr><td>&nbsp;</td><td class="result-snippet">Descrição detalhada do primeiro resultado lite.</td></tr>
      <tr><td valign="top">2.&nbsp;</td><td><a class="result-link" href="//exemplo.com/lite2">Lite Dois</a></td></tr>
      <tr><td>&nbsp;</td><td class="result-snippet">Descrição detalhada do segundo resultado lite.</td></tr>
    </table>
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

// ---------------------------------------------------------------------------
// Teste 1: Estratégia 1 com HTML completo → 3 resultados extraídos, 0 anúncios.
// ---------------------------------------------------------------------------
#[tokio::test]
async fn testa_estrategia_1_sucesso() {
    let _g = env_lock().lock().await;
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(html_com_3_resultados_classe())
                .insert_header("content-type", "text/html; charset=utf-8"),
        )
        .mount(&mock_server)
        .await;

    let base = format!("{}/", mock_server.uri());
    let _env = GuardaEnv::set(&[
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_HTML", base.clone()),
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_LITE", base),
    ]);

    let cliente = cliente_teste();
    let cfg = configuracoes_base(Endpoint::Html, 1, 0);
    let flag = Arc::new(AtomicBool::new(false));
    let token = CancellationToken::new();

    let agregado = buscar_com_paginacao(&cliente, &cfg, "rust", &flag, &token)
        .await
        .expect("busca deve ter sucesso");

    assert_eq!(agregado.resultados.len(), 3, "3 resultados orgânicos");
    assert_eq!(agregado.resultados[0].titulo, "Resultado Um");
    assert_eq!(agregado.resultados[0].url, "https://exemplo.com/um");
    assert_eq!(agregado.paginas_buscadas, 1);
    assert!(!agregado.usou_fallback_lite);
    assert_eq!(agregado.endpoint_efetivo, Endpoint::Html);
}

// ---------------------------------------------------------------------------
// Teste 2: HTML vazio → fallback para Lite → resultados extraídos via Estratégia 3.
// ---------------------------------------------------------------------------
#[tokio::test]
async fn testa_fallback_lite_quando_html_vazio() {
    let _g = env_lock().lock().await;
    let mock_server_html = MockServer::start().await;
    let mock_server_lite = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(html_vazio_sem_result())
                .insert_header("content-type", "text/html; charset=utf-8"),
        )
        .mount(&mock_server_html)
        .await;

    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(html_lite_tabela())
                .insert_header("content-type", "text/html; charset=utf-8"),
        )
        .mount(&mock_server_lite)
        .await;

    let base_html = format!("{}/", mock_server_html.uri());
    let base_lite = format!("{}/", mock_server_lite.uri());
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
        .expect("fallback Lite deve ter sucesso");

    assert_eq!(agregado.resultados.len(), 2, "2 resultados do Lite");
    assert_eq!(agregado.resultados[0].titulo, "Lite Um");
    assert!(agregado.usou_fallback_lite, "flag fallback deve estar true");
    assert_eq!(agregado.endpoint_efetivo, Endpoint::Lite);
}

// ---------------------------------------------------------------------------
// Teste 3: Retry em 429 — 2 primeiras respostas 429, 3a 200.
// ---------------------------------------------------------------------------
#[tokio::test]
async fn testa_retry_em_429() {
    let _g = env_lock().lock().await;
    let mock_server = MockServer::start().await;

    // Limitações: cada Mock responde com o mesmo template. Para sequenciar respostas,
    // usamos `up_to_n_times` nos mocks anteriores e deixamos o mock final como catch-all.
    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(ResponseTemplate::new(429))
        .up_to_n_times(2)
        .with_priority(1)
        .mount(&mock_server)
        .await;

    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(html_com_3_resultados_classe())
                .insert_header("content-type", "text/html; charset=utf-8"),
        )
        .with_priority(2)
        .mount(&mock_server)
        .await;

    let base = format!("{}/", mock_server.uri());
    let _env = GuardaEnv::set(&[
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_HTML", base.clone()),
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_LITE", base),
    ]);

    let cliente = cliente_teste();
    // 2 retries → até 3 tentativas no total.
    let cfg = configuracoes_base(Endpoint::Html, 1, 2);
    let flag = Arc::new(AtomicBool::new(false));
    let token = CancellationToken::new();

    let agregado = buscar_com_paginacao(&cliente, &cfg, "rust", &flag, &token)
        .await
        .expect("retry deve eventualmente ter sucesso");

    assert_eq!(agregado.resultados.len(), 3);
    assert_eq!(
        agregado.tentativas, 3,
        "DEVE ter executado exatamente 3 tentativas (2 falhas 429 + 1 sucesso)"
    );
    assert!(
        flag.load(std::sync::atomic::Ordering::Relaxed),
        "flag_rate_limit global deve ter sido ativada"
    );
}

// ---------------------------------------------------------------------------
// Teste 4: 403 persistente → erro `blocked` após esgotar retries.
// ---------------------------------------------------------------------------
#[tokio::test]
async fn testa_blocked_apos_retries_esgotados() {
    let _g = env_lock().lock().await;
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(ResponseTemplate::new(403))
        .mount(&mock_server)
        .await;

    let base = format!("{}/", mock_server.uri());
    let _env = GuardaEnv::set(&[
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_HTML", base.clone()),
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_LITE", base),
    ]);

    let cliente = cliente_teste();
    // 1 retry → até 2 tentativas.
    let cfg = configuracoes_base(Endpoint::Html, 1, 1);
    let flag = Arc::new(AtomicBool::new(false));
    let token = CancellationToken::new();

    let resultado = executar_com_retry(
        &cliente,
        &format!("{}/", mock_server.uri()),
        cfg.retries,
        &flag,
        &token,
    )
    .await;

    match resultado {
        Err(MotivoFalhaRetry::Blocked) => {}
        other => panic!("esperava MotivoFalhaRetry::Blocked, recebi {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// Teste 5: Paginação vqd — 3 páginas, combinação de resultados.
// ---------------------------------------------------------------------------
#[tokio::test]
async fn testa_paginacao_vqd() {
    let _g = env_lock().lock().await;
    let mock_server = MockServer::start().await;

    // Página 1 (GET) — HTML com tokens vqd/s/dc.
    let html_pg1 =
        html_com_tokens_vqd_e_resultados("vqd-pg1", "0", "30", &["Res Um", "Res Dois", "Res Três"]);
    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(html_pg1)
                .insert_header("content-type", "text/html; charset=utf-8"),
        )
        .mount(&mock_server)
        .await;

    // Página 2 (POST com vqd-pg1) — retorna HTML com vqd-pg2.
    let html_pg2 =
        html_com_tokens_vqd_e_resultados("vqd-pg2", "30", "60", &["Res Quatro", "Res Cinco"]);
    Mock::given(method("POST"))
        .and(path("/"))
        .and(body_string_contains("vqd=vqd-pg1"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(html_pg2)
                .insert_header("content-type", "text/html; charset=utf-8"),
        )
        .with_priority(1)
        .mount(&mock_server)
        .await;

    // Página 3 (POST com vqd-pg2).
    let html_pg3 = html_com_tokens_vqd_e_resultados("vqd-pg3", "60", "90", &["Res Seis"]);
    Mock::given(method("POST"))
        .and(path("/"))
        .and(body_string_contains("vqd=vqd-pg2"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(html_pg3)
                .insert_header("content-type", "text/html; charset=utf-8"),
        )
        .with_priority(2)
        .mount(&mock_server)
        .await;

    let base = format!("{}/", mock_server.uri());
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
        .expect("paginação deve funcionar");

    assert_eq!(
        agregado.resultados.len(),
        6,
        "deve combinar resultados das 3 páginas"
    );
    assert_eq!(agregado.paginas_buscadas, 3);
    // Posições devem ser 1..=6, preservando ordem por página.
    for (i, r) in agregado.resultados.iter().enumerate() {
        assert_eq!(r.posicao, (i + 1) as u32);
    }
    assert_eq!(agregado.resultados[0].titulo, "Res Um");
    assert_eq!(agregado.resultados[3].titulo, "Res Quatro");
    assert_eq!(agregado.resultados[5].titulo, "Res Seis");
}

// ---------------------------------------------------------------------------
// Teste 6: Filtro de anúncios — HTML com anúncios mistos, apenas orgânicos retornados.
// ---------------------------------------------------------------------------
#[tokio::test]
async fn testa_filtro_anuncios() {
    let _g = env_lock().lock().await;
    let mock_server = MockServer::start().await;

    // HTML misto: 2 orgânicos + 2 anúncios (um por classe, outro por data-nrn).
    let html = r#"<html><body>
    <div id="links">
      <div class="result result--ad">
        <a class="result__a" href="//anuncio1.com">Anúncio Um</a>
      </div>
      <div class="result">
        <a class="result__a" href="//organico.com/a">Orgânico A</a>
        <a class="result__snippet">Snippet A</a>
      </div>
      <div class="result" data-nrn="ad">
        <a class="result__a" href="//anuncio2.com">Anúncio Dois</a>
      </div>
      <div class="result">
        <a class="result__a" href="//organico.com/b">Orgânico B</a>
        <a class="result__snippet">Snippet B</a>
      </div>
      <div class="result">
        <a class="result__a" href="//duckduckgo.com/y.js?ad=x">Tracker</a>
      </div>
    </div>
    </body></html>"#;

    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(html)
                .insert_header("content-type", "text/html; charset=utf-8"),
        )
        .mount(&mock_server)
        .await;

    let base = format!("{}/", mock_server.uri());
    let _env = GuardaEnv::set(&[
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_HTML", base.clone()),
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_LITE", base),
    ]);

    let cliente = cliente_teste();
    let cfg = configuracoes_base(Endpoint::Html, 1, 0);
    let flag = Arc::new(AtomicBool::new(false));
    let token = CancellationToken::new();

    let agregado = buscar_com_paginacao(&cliente, &cfg, "rust", &flag, &token)
        .await
        .expect("sucesso");

    assert_eq!(
        agregado.resultados.len(),
        2,
        "apenas orgânicos devem sobreviver ao filtro"
    );
    assert_eq!(agregado.resultados[0].titulo, "Orgânico A");
    assert_eq!(agregado.resultados[1].titulo, "Orgânico B");
    for r in &agregado.resultados {
        assert!(!r.url.contains("anuncio"));
        assert!(!r.url.contains("y.js"));
    }
}

// ---------------------------------------------------------------------------
// Teste 7 (v0.3.0): heurística "Official site" — DDG renderiza literalmente
// "Official site" como título para domínios verificados. O scraper substitui
// pelo `url_exibicao` e preserva o texto literal em `titulo_original`.
// ---------------------------------------------------------------------------
#[tokio::test]
async fn testa_heuristica_official_site() {
    let _g = env_lock().lock().await;
    let mock_server = MockServer::start().await;

    // HTML com resultado que tem título literal "Official site" + .result__url.
    let html = r#"<html><body>
    <div id="links">
      <div class="result">
        <a class="result__a" href="//saofidelis.rj.gov.br/">Official site</a>
        <span class="result__url">saofidelis.rj.gov.br</span>
        <a class="result__snippet">Prefeitura Municipal de São Fidélis.</a>
      </div>
      <div class="result">
        <a class="result__a" href="//exemplo.com/outro">Título Normal</a>
        <span class="result__url">exemplo.com</span>
        <a class="result__snippet">Snippet qualquer.</a>
      </div>
    </div>
    </body></html>"#;

    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(html)
                .insert_header("content-type", "text/html; charset=utf-8"),
        )
        .mount(&mock_server)
        .await;

    let base = format!("{}/", mock_server.uri());
    let _env = GuardaEnv::set(&[
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_HTML", base.clone()),
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_LITE", base),
    ]);

    let cliente = cliente_teste();
    let cfg = configuracoes_base(Endpoint::Html, 1, 0);
    let flag = Arc::new(AtomicBool::new(false));
    let token = CancellationToken::new();

    let agregado = buscar_com_paginacao(&cliente, &cfg, "saofidelis", &flag, &token)
        .await
        .expect("sucesso");

    assert_eq!(agregado.resultados.len(), 2, "2 orgânicos esperados");

    // Resultado 1: título substituído por url_exibicao, original preservado.
    let r1 = &agregado.resultados[0];
    assert_eq!(
        r1.titulo, "saofidelis.rj.gov.br",
        "titulo deve ser o url_exibicao"
    );
    assert_eq!(
        r1.titulo_original.as_deref(),
        Some("Official site"),
        "titulo_original deve preservar o literal"
    );

    // Resultado 2: título normal → sem substituição, titulo_original = None.
    let r2 = &agregado.resultados[1];
    assert_eq!(r2.titulo, "Título Normal");
    assert!(
        r2.titulo_original.is_none(),
        "titulo_original deve ser None quando não há substituição"
    );
}

// ---------------------------------------------------------------------------
// Teste 8 (v0.3.0): schema JSON NÃO contém mais `buscas_relacionadas`.
// ---------------------------------------------------------------------------
#[tokio::test]
async fn testa_schema_v03_sem_buscas_relacionadas() {
    let _g = env_lock().lock().await;
    let mock_server = MockServer::start().await;

    let html = html_com_tokens_vqd_e_resultados("v", "0", "0", &["T1", "T2"]);
    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(html)
                .insert_header("content-type", "text/html; charset=utf-8"),
        )
        .mount(&mock_server)
        .await;

    let base = format!("{}/", mock_server.uri());
    let _env = GuardaEnv::set(&[
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_HTML", base.clone()),
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_LITE", base),
    ]);

    let cliente = cliente_teste();
    let cfg = configuracoes_base(Endpoint::Html, 1, 0);
    let flag = Arc::new(AtomicBool::new(false));
    let token = CancellationToken::new();

    let agregado = buscar_com_paginacao(&cliente, &cfg, "teste", &flag, &token)
        .await
        .expect("sucesso");

    // Serializar como JSON e confirmar que o campo NÃO aparece.
    use duckduckgo_search_cli::types::{MetadadosBusca, SaidaBusca};
    let saida = SaidaBusca {
        query: "teste".into(),
        motor: "duckduckgo".into(),
        endpoint: "html".into(),
        timestamp: "2026-04-14T00:00:00Z".into(),
        regiao: "br-pt".into(),
        quantidade_resultados: agregado.resultados.len() as u32,
        resultados: agregado.resultados,
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

    let json = serde_json::to_string_pretty(&saida).expect("serializa");
    assert!(
        !json.contains("buscas_relacionadas"),
        "v0.3.0: schema JSON NÃO deve expor buscas_relacionadas"
    );
    assert!(
        !json.contains("related_searches"),
        "v0.3.0: schema JSON NÃO deve expor related_searches"
    );
}

// ===================================================================
// Testes de --fetch-content HTTP puro (iteração 5).
// ===================================================================

/// Página HTML real com conteúdo suficiente para passar o limiar de 200 chars.
fn html_artigo_real() -> String {
    r#"<!DOCTYPE html><html><head><title>Artigo de Teste</title></head>
    <body>
      <nav><a href="/">Home</a> <a href="/about">About</a></nav>
      <article>
        <h1>Título Principal do Artigo</h1>
        <p>Este é o primeiro parágrafo do artigo com conteúdo substantivo suficiente para passar o limiar de linha mínima. Contém várias frases para simular um texto real de notícia ou documentação técnica.</p>
        <p>Segundo parágrafo relevante com mais informações sobre o tema tratado no artigo. Incluímos conteúdo em português brasileiro com acentuação correta para validar a decodificação UTF-8 adequadamente.</p>
        <p>Terceiro parágrafo conclui o artigo com uma síntese dos pontos principais abordados ao longo do texto. Este conteúdo deve ser preservado integralmente pela extração readability.</p>
      </article>
      <footer>Copyright 2026 Rodapé do site</footer>
    </body></html>"#.to_string()
}

#[tokio::test]
async fn fetch_content_http_extrai_artigo_real_via_wiremock() {
    use duckduckgo_search_cli::content::extrair_conteudo_http;

    let _guarda = env_lock().lock().await;
    let servidor = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/artigo"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_raw(html_artigo_real().into_bytes(), "text/html; charset=utf-8"),
        )
        .mount(&servidor)
        .await;

    let cliente = cliente_teste();
    let token = CancellationToken::new();
    let url = format!("{}/artigo", servidor.uri());

    let resultado = extrair_conteudo_http(&cliente, &url, 2000, &token)
        .await
        .expect("fetch deve ter sucesso");
    let (texto, tamanho_orig) = resultado.expect("conteúdo presente");
    assert!(
        texto.contains("primeiro parágrafo"),
        "deve conter primeiro parágrafo: {texto:?}"
    );
    assert!(texto.contains("Segundo parágrafo"));
    assert!(texto.contains("Terceiro parágrafo"));
    // Nav e footer devem ter sido removidos.
    assert!(!texto.contains("About"));
    assert!(!texto.contains("Copyright"));
    // Tamanho original reportado > 0.
    assert!(tamanho_orig > 0);
}

#[tokio::test]
async fn fetch_content_http_rejeita_content_type_nao_html() {
    use duckduckgo_search_cli::content::extrair_conteudo_http;

    let _guarda = env_lock().lock().await;
    let servidor = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/pdf"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(vec![0u8; 100], "application/pdf"))
        .mount(&servidor)
        .await;

    let cliente = cliente_teste();
    let token = CancellationToken::new();
    let url = format!("{}/pdf", servidor.uri());

    let resultado = extrair_conteudo_http(&cliente, &url, 1000, &token)
        .await
        .expect("request OK");
    assert!(
        resultado.is_none(),
        "Content-Type não HTML deve retornar None"
    );
}

#[tokio::test]
async fn fetch_content_http_decodifica_latin1_corretamente() {
    use duckduckgo_search_cli::content::extrair_conteudo_http;

    let _guarda = env_lock().lock().await;
    let servidor = MockServer::start().await;

    // HTML em Latin-1 (ISO-8859-1) contendo 'ç' (0xE7) + 'á' (0xE1).
    let mut html: Vec<u8> = b"<html><body><article>".to_vec();
    // Parágrafo longo o bastante para passar o limiar (20+ chars).
    html.extend_from_slice(
        b"<p>Uma a\xe7\xe3o interessante foi realizada pelos desenvolvedores do sistema de busca que n\xe3o pode ser ignorada.</p>"
    );
    html.extend_from_slice(
        b"<p>Outro par\xe1grafo relevante com mais texto para chegar ao limiar m\xednimo de conte\xfado necess\xe1rio.</p>"
    );
    html.extend_from_slice(b"</article></body></html>");

    Mock::given(method("GET"))
        .and(path("/latin1"))
        .respond_with(
            ResponseTemplate::new(200).set_body_raw(html, "text/html; charset=iso-8859-1"),
        )
        .mount(&servidor)
        .await;

    let cliente = cliente_teste();
    let token = CancellationToken::new();
    let url = format!("{}/latin1", servidor.uri());

    let resultado = extrair_conteudo_http(&cliente, &url, 2000, &token)
        .await
        .expect("fetch deve ter sucesso");
    let (texto, _) = resultado.expect("conteúdo presente");
    // 'ação' com acento deve estar presente (decodificado corretamente de Latin-1).
    assert!(
        texto.contains("ação") || texto.contains("parágrafo"),
        "texto deve ter acentuação UTF-8 correta: {texto:?}"
    );
}

// Sanity check do teste auxiliar extrair_tokens_paginacao (cobertura extra no
// nível de integração para garantir que o helper está exposto e funciona).
#[test]
fn sanity_extrair_tokens_paginacao_via_lib_publica() {
    let html = html_com_tokens_vqd_e_resultados("v1", "0", "10", &["a", "b"]);
    let (vqd, s, dc) = extrair_tokens_paginacao(&html).expect("tokens presentes");
    assert_eq!(vqd, "v1");
    assert_eq!(s, "0");
    assert_eq!(dc, "10");
}

#[test]
fn ndjson_serializa_saida_busca_em_uma_linha_valida() {
    use duckduckgo_search_cli::types::{MetadadosBusca, ResultadoBusca, SaidaBusca};
    let saida = SaidaBusca {
        query: "rust".into(),
        motor: "duckduckgo".into(),
        endpoint: "html".into(),
        timestamp: "2026-04-14T00:00:00Z".into(),
        regiao: "br-pt".into(),
        quantidade_resultados: 1,
        resultados: vec![ResultadoBusca {
            posicao: 1,
            titulo: "Exemplo com\nnova linha".to_string(),
            url: "https://exemplo.com".to_string(),
            url_exibicao: None,
            snippet: None,
            titulo_original: None,
            conteudo: None,
            tamanho_conteudo: None,
            metodo_extracao_conteudo: None,
        }],
        paginas_buscadas: 1,
        erro: None,
        mensagem: None,
        metadados: MetadadosBusca {
            tempo_execucao_ms: 100,
            hash_seletores: "abc123".into(),
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
    let linha = serde_json::to_string(&saida).expect("serializar NDJSON");
    // Uma linha só (sem \n intermediário — \n dentro do título é escapado como \\n).
    assert!(
        !linha.contains('\n'),
        "NDJSON deve ser UMA linha só — \\n literais no conteúdo DEVEM estar escapados"
    );
    let parsed: serde_json::Value =
        serde_json::from_str(&linha).expect("NDJSON deve ser JSON válido");
    assert_eq!(parsed["query"], "rust");
    assert_eq!(parsed["quantidade_resultados"], 1);
}

// ---------------------------------------------------------------------------
// Teste v0.4.0 #1: default --num 15 + auto-paginação para 2 páginas.
//
// Simula a configuração que `montar_configuracoes` produziria quando o usuário
// NÃO passa `--num` (default=15) e `--pages` está no default (1): ela eleva
// `paginas` para 2 e fixa `num_resultados = Some(15)`. O teste verifica que
// `buscar_com_paginacao` respeita isso: faz GET da página 1, POST da página 2
// (com vqd), agrega 11 + 10 = 21 resultados e trunca em 15.
// ---------------------------------------------------------------------------
#[tokio::test]
async fn testa_default_num_15_auto_pagina_2_paginas() {
    let _g = env_lock().lock().await;
    let mock_server = MockServer::start().await;

    // Página 1 — 11 resultados (quantidade típica da primeira página do DDG).
    let titulos_pg1: Vec<String> = (1..=11).map(|i| format!("Res Pg1 {i}")).collect();
    let refs_pg1: Vec<&str> = titulos_pg1.iter().map(String::as_str).collect();
    let html_pg1 = html_com_tokens_vqd_e_resultados("vqd-auto-pg1", "0", "30", &refs_pg1);
    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(html_pg1)
                .insert_header("content-type", "text/html; charset=utf-8"),
        )
        .mount(&mock_server)
        .await;

    // Página 2 — 10 resultados adicionais. A busca virá via POST com vqd-auto-pg1.
    let titulos_pg2: Vec<String> = (1..=10).map(|i| format!("Res Pg2 {i}")).collect();
    let refs_pg2: Vec<&str> = titulos_pg2.iter().map(String::as_str).collect();
    let html_pg2 = html_com_tokens_vqd_e_resultados("vqd-auto-pg2", "30", "60", &refs_pg2);
    Mock::given(method("POST"))
        .and(path("/"))
        .and(body_string_contains("vqd=vqd-auto-pg1"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(html_pg2)
                .insert_header("content-type", "text/html; charset=utf-8"),
        )
        .with_priority(1)
        .mount(&mock_server)
        .await;

    let base = format!("{}/", mock_server.uri());
    let _env = GuardaEnv::set(&[
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_HTML", base.clone()),
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_LITE", base),
    ]);

    let cliente = cliente_teste();
    // Simula a configuração PÓS-montar_configuracoes com default --num 15
    // e auto-paginação para 2 páginas (comportamento novo em v0.4.0).
    let mut cfg = configuracoes_base(Endpoint::Html, 2, 0);
    cfg.num_resultados = Some(15);
    let flag = Arc::new(AtomicBool::new(false));
    let token = CancellationToken::new();

    let agregado = buscar_com_paginacao(&cliente, &cfg, "rust", &flag, &token)
        .await
        .expect("auto-paginação deve funcionar");

    assert_eq!(
        agregado.resultados.len(),
        15,
        "deve truncar em --num=15 após agregar 21 resultados de 2 páginas"
    );
    assert_eq!(
        agregado.paginas_buscadas, 2,
        "deve ter buscado exatamente 2 páginas"
    );
    assert_eq!(agregado.resultados[0].titulo, "Res Pg1 1");
    // Página 2 começa após os 11 da página 1 → posições 12..=15 vêm da página 2.
    assert_eq!(agregado.resultados[11].titulo, "Res Pg2 1");
    assert_eq!(agregado.resultados[14].titulo, "Res Pg2 4");
}

// ---------------------------------------------------------------------------
// Teste v0.4.0 #2: auto-paginação RESPEITA --pages explícito do usuário.
//
// Quando o usuário passa --pages 3 explicitamente, a lógica de auto-paginação
// NÃO sobrescreve. Este teste simula cfg com paginas=3 + num=15 e verifica
// que buscar_com_paginacao executa 3 páginas (e trunca em 15).
// ---------------------------------------------------------------------------
#[tokio::test]
async fn testa_auto_paginacao_respeita_pages_explicito() {
    let _g = env_lock().lock().await;
    let mock_server = MockServer::start().await;

    // Página 1 — 11 resultados.
    let titulos_pg1: Vec<String> = (1..=11).map(|i| format!("Pg1-{i}")).collect();
    let refs_pg1: Vec<&str> = titulos_pg1.iter().map(String::as_str).collect();
    let html_pg1 = html_com_tokens_vqd_e_resultados("v-expl-1", "0", "30", &refs_pg1);
    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(html_pg1)
                .insert_header("content-type", "text/html; charset=utf-8"),
        )
        .mount(&mock_server)
        .await;

    // Página 2 — 5 resultados.
    let titulos_pg2: Vec<String> = (1..=5).map(|i| format!("Pg2-{i}")).collect();
    let refs_pg2: Vec<&str> = titulos_pg2.iter().map(String::as_str).collect();
    let html_pg2 = html_com_tokens_vqd_e_resultados("v-expl-2", "30", "60", &refs_pg2);
    Mock::given(method("POST"))
        .and(path("/"))
        .and(body_string_contains("vqd=v-expl-1"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(html_pg2)
                .insert_header("content-type", "text/html; charset=utf-8"),
        )
        .with_priority(1)
        .mount(&mock_server)
        .await;

    // Página 3 — 3 resultados.
    let titulos_pg3: Vec<String> = (1..=3).map(|i| format!("Pg3-{i}")).collect();
    let refs_pg3: Vec<&str> = titulos_pg3.iter().map(String::as_str).collect();
    let html_pg3 = html_com_tokens_vqd_e_resultados("v-expl-3", "60", "90", &refs_pg3);
    Mock::given(method("POST"))
        .and(path("/"))
        .and(body_string_contains("vqd=v-expl-2"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(html_pg3)
                .insert_header("content-type", "text/html; charset=utf-8"),
        )
        .with_priority(2)
        .mount(&mock_server)
        .await;

    let base = format!("{}/", mock_server.uri());
    let _env = GuardaEnv::set(&[
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_HTML", base.clone()),
        ("DUCKDUCKGO_SEARCH_CLI_BASE_URL_LITE", base),
    ]);

    let cliente = cliente_teste();
    // Simula --num 15 --pages 3 (explícito) → montar_configuracoes NÃO sobrescreve.
    let mut cfg = configuracoes_base(Endpoint::Html, 3, 0);
    cfg.num_resultados = Some(15);
    let flag = Arc::new(AtomicBool::new(false));
    let token = CancellationToken::new();

    let agregado = buscar_com_paginacao(&cliente, &cfg, "rust", &flag, &token)
        .await
        .expect("deve buscar 3 páginas conforme --pages explícito");

    assert_eq!(
        agregado.paginas_buscadas, 3,
        "deve respeitar --pages=3 explícito"
    );
    // 11 + 5 + 3 = 19 agregados, truncados em 15.
    assert_eq!(
        agregado.resultados.len(),
        15,
        "19 agregados devem ser truncados em num=15"
    );
    assert_eq!(agregado.resultados[0].titulo, "Pg1-1");
    assert_eq!(agregado.resultados[11].titulo, "Pg2-1");
}
