//! Testes de integração baseados em fixtures HTML REAIS capturadas do DuckDuckGo.
//!
//! As fixtures em `tests/fixtures/` foram obtidas em 2026-04-14 via:
//!   xh "https://html.duckduckgo.com/html/?q=rust+programming"
//!   xh "https://lite.duckduckgo.com/lite/?q=rust+programming"
//!
//! IMPORTANTE: o User-Agent enviado por padrão pelo `xh` (`xh/0.25.3`) NÃO é
//! identificado como bot pelo DuckDuckGo, ao contrário de UAs Chrome/Firefox
//! "completos" que retornam HTTP 202 com challenge anomaly. Esta diferença foi
//! a causa raiz de "0 resultados" reportada na iteração 3 e está documentada
//! na FASE A do diagnóstico desta iteração.
//!
//! Estes testes garantem regressão das mudanças nos seletores: se DDG mudar
//! o DOM, estes testes falham (e a fixture deve ser re-capturada).

use duckduckgo_search_cli::extraction::{
    extrair_resultados, extrair_resultados_com_estrategias, extrair_resultados_lite,
};
use std::fs;
use std::path::PathBuf;

fn carregar_fixture(nome: &str) -> String {
    let mut caminho = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    caminho.push("tests");
    caminho.push("fixtures");
    caminho.push(nome);
    fs::read_to_string(&caminho)
        .unwrap_or_else(|e| panic!("falha ao ler fixture {}: {e}", caminho.display()))
}

#[test]
fn extracao_html_real_pagina_1_recupera_pelo_menos_dez_resultados() {
    let html = carregar_fixture("ddg_html_pagina_1.html");
    let resultados = extrair_resultados(&html);
    assert!(
        resultados.len() >= 10,
        "esperado >= 10 resultados, obtido {}",
        resultados.len()
    );

    // Todos os resultados devem ter título e URL não-vazios.
    for r in &resultados {
        assert!(
            !r.titulo.is_empty(),
            "título vazio na posição {}",
            r.posicao
        );
        assert!(!r.url.is_empty(), "URL vazia na posição {}", r.posicao);
        assert!(
            r.url.starts_with("https://") || r.url.starts_with("http://"),
            "URL não é absoluta na posição {}: {}",
            r.posicao,
            r.url
        );
        // Nenhuma URL pode permanecer como redirect interno.
        assert!(
            !r.url.contains("duckduckgo.com/l/?uddg="),
            "URL não foi desencapsulada: {}",
            r.url
        );
    }

    // A maioria dos resultados deve ter snippet.
    let com_snippet = resultados
        .iter()
        .filter(|r| r.snippet.as_ref().map(|s| !s.is_empty()).unwrap_or(false))
        .count();
    assert!(
        com_snippet >= 8,
        "esperado pelo menos 8 resultados com snippet, obtido {com_snippet}"
    );

    // Posições devem ser sequenciais começando em 1.
    for (i, r) in resultados.iter().enumerate() {
        assert_eq!(
            r.posicao,
            (i + 1) as u32,
            "posições devem ser sequenciais 1-indexed"
        );
    }
}

#[test]
fn extracao_html_real_pagina_1_extrai_url_de_exibicao_quando_presente() {
    let html = carregar_fixture("ddg_html_pagina_1.html");
    let resultados = extrair_resultados(&html);
    let com_url_exibicao = resultados
        .iter()
        .filter(|r| {
            r.url_exibicao
                .as_ref()
                .map(|u| !u.is_empty())
                .unwrap_or(false)
        })
        .count();
    assert!(
        com_url_exibicao >= 8,
        "esperado >= 8 resultados com url_exibicao, obtido {com_url_exibicao}"
    );
}

#[test]
fn extracao_lite_real_pagina_1_recupera_pelo_menos_dez_resultados() {
    let html = carregar_fixture("ddg_lite_pagina_1.html");
    let resultados = extrair_resultados_lite(&html);
    assert!(
        resultados.len() >= 10,
        "esperado >= 10 resultados Lite, obtido {}",
        resultados.len()
    );

    for r in &resultados {
        assert!(
            !r.titulo.is_empty(),
            "título Lite vazio na posição {}",
            r.posicao
        );
        assert!(!r.url.is_empty(), "URL Lite vazia na posição {}", r.posicao);
        assert!(
            r.url.starts_with("https://") || r.url.starts_with("http://"),
            "URL Lite não é absoluta: {}",
            r.url
        );
        assert!(
            !r.url.contains("duckduckgo.com/l/?uddg="),
            "URL Lite não desencapsulada: {}",
            r.url
        );
    }

    // Maioria dos resultados Lite deve ter snippet (vem em <tr> separado).
    let com_snippet = resultados
        .iter()
        .filter(|r| r.snippet.as_ref().map(|s| !s.is_empty()).unwrap_or(false))
        .count();
    assert!(
        com_snippet >= 8,
        "esperado >= 8 resultados Lite com snippet, obtido {com_snippet}"
    );
}

#[test]
fn extracao_html_real_pagina_1_filtra_links_internos_do_duckduckgo() {
    let html = carregar_fixture("ddg_html_pagina_1.html");
    let resultados = extrair_resultados(&html);
    for r in &resultados {
        assert!(
            !r.url.contains("html.duckduckgo.com")
                && !r.url.contains("lite.duckduckgo.com")
                && !r.url.contains("duckduckgo.com/y.js"),
            "resultado contém URL interna do DDG: {}",
            r.url
        );
    }
}

#[test]
fn extracao_estrategias_combina_e_continua_funcionando_em_html_real() {
    let html = carregar_fixture("ddg_html_pagina_1.html");
    let resultados_simples = extrair_resultados(&html);
    let resultados_com_estrategias = extrair_resultados_com_estrategias(&html);

    // Em HTML válido, Estratégia 1 vence — a função de fallback deve retornar
    // o mesmo número.
    assert_eq!(
        resultados_simples.len(),
        resultados_com_estrategias.len(),
        "Estratégia 1 devolveu resultados — Estratégia 2 não deve sobrescrever"
    );
}
