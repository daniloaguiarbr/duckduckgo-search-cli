// SPDX-License-Identifier: MIT OR Apache-2.0
//! Testes de integração baseados em fixtures HTML REAIS capturadas do `DuckDuckGo`.
//!
//! As fixtures em `tests/fixtures/` foram obtidas em 2026-04-14 via:
//!   xh "<https://html.duckduckgo.com/html/?q=rust+programming>"
//!   xh "<https://lite.duckduckgo.com/lite/?q=rust+programming>"
//!
//! IMPORTANTE: o User-Agent enviado por padrão pelo `xh` (`xh/0.25.3`) NÃO é
//! identificado como bot pelo `DuckDuckGo`, ao contrário de UAs Chrome/Firefox
//! "completos" que retornam HTTP 202 com challenge anomaly. Esta diferença foi
//! a causa raiz de "0 resultados" reportada na iteração 3 e está documentada
//! na FASE A do diagnóstico desta iteração.
//!
//! Estes testes garantem regressão das mudanças nos seletores: se DDG mudar
//! o DOM, estes testes falham (e a fixture deve ser re-capturada).

use duckduckgo_search_cli::extraction::{
    extract_results, extract_results_lite, extract_results_with_strategies,
};
use std::fs;
use std::path::PathBuf;

fn load_fixture(name: &str) -> String {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("tests");
    path.push("fixtures");
    path.push(name);
    fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("falha ao ler fixture {}: {e}", path.display()))
}

#[test]
fn extracao_html_real_pagina_1_recupera_pelo_menos_dez_resultados() {
    let html = load_fixture("ddg_html_pagina_1.html");
    let results = extract_results(&html);
    assert!(
        results.len() >= 10,
        "esperado >= 10 resultados, obtido {}",
        results.len()
    );

    // All results must have non-empty title and URL.
    for r in &results {
        assert!(
            !r.title.is_empty(),
            "título vazio na posição {}",
            r.position
        );
        assert!(!r.url.is_empty(), "URL vazia na posição {}", r.position);
        assert!(
            r.url.starts_with("https://") || r.url.starts_with("http://"),
            "URL não é absoluta na posição {}: {}",
            r.position,
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
    let with_snippet = results
        .iter()
        .filter(|r| r.snippet.as_ref().map(|s| !s.is_empty()).unwrap_or(false))
        .count();
    assert!(
        with_snippet >= 8,
        "esperado pelo menos 8 resultados com snippet, obtido {with_snippet}"
    );

    // Positions must be sequential starting at 1.
    for (i, r) in results.iter().enumerate() {
        assert_eq!(
            r.position,
            (i + 1) as u32,
            "posições devem ser sequenciais 1-indexed"
        );
    }
}

#[test]
fn real_html_page_1_extracts_display_url_when_present() {
    let html = load_fixture("ddg_html_pagina_1.html");
    let results = extract_results(&html);
    let with_display_url = results
        .iter()
        .filter(|r| {
            r.display_url
                .as_ref()
                .map(|u| !u.is_empty())
                .unwrap_or(false)
        })
        .count();
    assert!(
        with_display_url >= 8,
        "esperado >= 8 resultados com display_url, obtido {with_display_url}"
    );
}

#[test]
fn extracao_lite_real_pagina_1_recupera_pelo_menos_dez_resultados() {
    let html = load_fixture("ddg_lite_pagina_1.html");
    let results = extract_results_lite(&html);
    assert!(
        results.len() >= 10,
        "esperado >= 10 resultados Lite, obtido {}",
        results.len()
    );

    for r in &results {
        assert!(
            !r.title.is_empty(),
            "título Lite vazio na posição {}",
            r.position
        );
        assert!(
            !r.url.is_empty(),
            "URL Lite vazia na posição {}",
            r.position
        );
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
    let with_snippet = results
        .iter()
        .filter(|r| r.snippet.as_ref().map(|s| !s.is_empty()).unwrap_or(false))
        .count();
    assert!(
        with_snippet >= 8,
        "esperado >= 8 resultados Lite com snippet, obtido {with_snippet}"
    );
}

#[test]
fn extracao_html_real_pagina_1_filtra_links_internos_do_duckduckgo() {
    let html = load_fixture("ddg_html_pagina_1.html");
    let results = extract_results(&html);
    for r in &results {
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
fn strategies_extraction_combines_and_works_on_real_html() {
    let html = load_fixture("ddg_html_pagina_1.html");
    let simple_results = extract_results(&html);
    let strategy_results = extract_results_with_strategies(&html);

    // In valid HTML, Strategy 1 wins — the fallback function must return
    // the same count.
    assert_eq!(
        simple_results.len(),
        strategy_results.len(),
        "Estratégia 1 devolveu resultados — Estratégia 2 não deve sobrescrever"
    );
}
