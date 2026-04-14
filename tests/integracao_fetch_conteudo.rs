//! Testes de integração para `fetch_conteudo::enriquecer_com_conteudo` via `wiremock`.
//!
//! Cobrem o CAMINHO FELIZ (HTTP retorna HTML válido → snippet preenchido) que não
//! é exercitado pelos testes unitários inline do módulo (que cobrem no-op e cancelado).

use duckduckgo_search_cli::fetch_conteudo::enriquecer_com_conteudo;
use duckduckgo_search_cli::types::{
    Configuracoes, Endpoint, FormatoSaida, MetadadosBusca, ResultadoBusca, SafeSearch, SaidaBusca,
};
use reqwest::Client;
use std::time::Duration;
use tokio_util::sync::CancellationToken;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn cfg(paralelismo: u32) -> Configuracoes {
    Configuracoes {
        query: "q".into(),
        queries: vec!["q".into()],
        num_resultados: None,
        formato: FormatoSaida::Json,
        timeout_segundos: 5,
        idioma: "pt".into(),
        pais: "br".into(),
        modo_verboso: false,
        modo_silencioso: true,
        user_agent: "Mozilla/5.0 (teste)".into(),
        paralelismo,
        paginas: 1,
        retries: 0,
        endpoint: Endpoint::Html,
        filtro_temporal: None,
        safe_search: SafeSearch::Moderate,
        modo_stream: false,
        arquivo_saida: None,
        buscar_conteudo: true,
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

fn saida_com_urls(urls: &[&str]) -> SaidaBusca {
    let resultados: Vec<ResultadoBusca> = urls
        .iter()
        .enumerate()
        .map(|(i, u)| ResultadoBusca {
            posicao: (i + 1) as u32,
            titulo: format!("Titulo {i}"),
            url: (*u).to_string(),
            url_exibicao: None,
            snippet: Some(format!("snippet {i}")),
            conteudo: None,
            tamanho_conteudo: None,
            metodo_extracao_conteudo: None,
        })
        .collect();
    SaidaBusca {
        query: "q".into(),
        motor: "duckduckgo".into(),
        endpoint: "html".into(),
        timestamp: "2026-04-14T00:00:00Z".into(),
        regiao: "br-pt".into(),
        quantidade_resultados: resultados.len() as u32,
        resultados,
        buscas_relacionadas: vec![],
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
    }
}

fn artigo_html(titulo: &str) -> String {
    // HTML realista para readability: <article> + vários parágrafos longos.
    let paragrafos: Vec<String> = (0..5)
        .map(|i| {
            format!(
                "<p>Este é o parágrafo número {i} do artigo sobre {titulo}, \
                 com texto suficiente para ultrapassar o threshold de 200 caracteres \
                 e convencer o extrator de que há conteúdo relevante a preservar.</p>"
            )
        })
        .collect();
    format!(
        "<html><head><title>{titulo}</title></head><body>\
         <nav>menu</nav>\
         <article>{}</article>\
         <footer>rodapé</footer>\
         </body></html>",
        paragrafos.join("")
    )
}

// ---------------------------------------------------------------------------
// T1: caminho feliz — 2 URLs distintas, HTTP retorna HTML com artigo → ambos enriquecidos.
// ---------------------------------------------------------------------------
#[tokio::test]
async fn enriquece_duas_urls_via_http_puro_e_marca_metodo_http() {
    let mock = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/a"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_raw(artigo_html("Rust").into_bytes(), "text/html; charset=utf-8"),
        )
        .mount(&mock)
        .await;

    Mock::given(method("GET"))
        .and(path("/b"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            artigo_html("Tokio").into_bytes(),
            "text/html; charset=utf-8",
        ))
        .mount(&mock)
        .await;

    let url_a = format!("{}/a", mock.uri());
    let url_b = format!("{}/b", mock.uri());
    let mut saida = saida_com_urls(&[&url_a, &url_b]);

    let cliente = Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .unwrap();
    let cfg = cfg(2);
    let token = CancellationToken::new();

    enriquecer_com_conteudo(&mut saida, &cliente, &cfg, &token).await;

    assert_eq!(saida.metadados.fetches_simultaneos, 2);
    assert_eq!(saida.metadados.sucessos_fetch, 2);
    assert_eq!(saida.metadados.falhas_fetch, 0);
    for r in &saida.resultados {
        let conteudo = r.conteudo.as_ref().expect("conteudo presente");
        assert!(conteudo.len() > 100, "conteúdo não-trivial");
        assert_eq!(
            r.metodo_extracao_conteudo.as_deref(),
            Some("http"),
            "método deve ser http (sem feature chrome ativa ou sem fallback acionado)"
        );
    }
    // Sem feature `chrome` ativa, este campo permanece false.
    assert!(!saida.metadados.usou_chrome);
}

// ---------------------------------------------------------------------------
// T2: endpoint retorna content-type não-HTML → deve registrar falha, não crashar.
// ---------------------------------------------------------------------------
#[tokio::test]
async fn enriquece_com_content_type_nao_html_registra_falha() {
    let mock = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/img"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(b"PNGDATA".to_vec(), "image/png"))
        .mount(&mock)
        .await;

    let url = format!("{}/img", mock.uri());
    let mut saida = saida_com_urls(&[&url]);

    let cliente = Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .unwrap();
    let cfg = cfg(1);
    let token = CancellationToken::new();

    enriquecer_com_conteudo(&mut saida, &cliente, &cfg, &token).await;

    assert_eq!(saida.metadados.fetches_simultaneos, 1);
    assert_eq!(saida.metadados.sucessos_fetch, 0, "não-HTML = 0 sucessos");
    assert_eq!(saida.metadados.falhas_fetch, 1, "não-HTML conta como falha");
    assert!(saida.resultados[0].conteudo.is_none());
    assert!(saida.resultados[0].metodo_extracao_conteudo.is_none());
}

// ---------------------------------------------------------------------------
// T3: dois resultados no MESMO host — exercita semáforo por host sem serializar tudo.
// ---------------------------------------------------------------------------
#[tokio::test]
async fn enriquece_mesmo_host_respeitando_limite_por_host() {
    let mock = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/p1"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_raw(artigo_html("A").into_bytes(), "text/html; charset=utf-8"),
        )
        .mount(&mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/p2"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_raw(artigo_html("B").into_bytes(), "text/html; charset=utf-8"),
        )
        .mount(&mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/p3"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_raw(artigo_html("C").into_bytes(), "text/html; charset=utf-8"),
        )
        .mount(&mock)
        .await;

    let u1 = format!("{}/p1", mock.uri());
    let u2 = format!("{}/p2", mock.uri());
    let u3 = format!("{}/p3", mock.uri());
    let mut saida = saida_com_urls(&[&u1, &u2, &u3]);

    let cliente = Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .unwrap();
    let mut cfg = cfg(3);
    cfg.limite_por_host = 2;
    let token = CancellationToken::new();

    enriquecer_com_conteudo(&mut saida, &cliente, &cfg, &token).await;

    assert_eq!(saida.metadados.sucessos_fetch, 3);
    assert_eq!(saida.metadados.falhas_fetch, 0);
}
