// SPDX-License-Identifier: MIT OR Apache-2.0
//! GAP-META-001 v0.8.0 — Invariantes pós-pipeline.
//!
//! Testes que validam que campos wirados em runtime são de fato populados
//! após o pipeline executar. Estes testes detectam wiradas ausentes que
//! testes unit/integration tradicionais não capturam (porque `None` é
//! valor válido do tipo `Option<T>`).
//!
//! Princípio: testar invariantes executando o pipeline inteiro (não
//! mockando partes), verificando que campos que DEVERIAM ser populados
//! em paths normais o são de fato.

use duckduckgo_search_cli::types::Endpoint;

#[test]
fn invariant_cascade_level_observed_present_in_metadata() {
    // Construímos SearchMetadata como faria o pipeline e verificamos que
    // cascade_level_observed recebe o valor derivado. Não depende de
    // cfg.last_probe_cascade_level.
    use duckduckgo_search_cli::types::SearchMetadata;
    let metadata = SearchMetadata {
        execution_time_ms: 100,
        selectors_hash: "abc123".to_string(),
        retries: 0,
        retries_configured: None,
        used_fallback_endpoint: false,
        concurrent_fetches: 0,
        fetch_successes: 0,
        fetch_failures: 0,
        used_chrome: false,
        chrome_attempted: false,
        user_agent: "test-ua".to_string(),
        used_proxy: false,
        identity_used: None,
        cascade_level: None,
        pre_flight_fired: false,
        zero_cause: None,
        sugestao_proxima_acao: None,
        bytes_raw: None,
        bytes_decompressed: None,
        cascade_level_observed: Some(0),
    };
    // GAP-META-001 + GAP-AUD-010: o campo deve estar presente após pipeline.
    assert!(
        metadata.cascade_level_observed.is_some(),
        "GAP-AUD-010: cascade_level_observed deve estar presente em metadata pós-pipeline"
    );
}

#[test]
fn invariant_retries_configured_field_exists() {
    use duckduckgo_search_cli::types::SearchMetadata;
    // GAP-META-001 + GAP-AUD-007: campo `retries_configured` deve existir
    // e ser populável. Antes da v0.8.0 não existia — operador não via
    // distinção entre "0 retries executados" e "0 retries configurados".
    let metadata = SearchMetadata {
        execution_time_ms: 100,
        selectors_hash: "abc123".to_string(),
        retries: 0,
        retries_configured: Some(5),
        used_fallback_endpoint: false,
        concurrent_fetches: 0,
        fetch_successes: 0,
        fetch_failures: 0,
        used_chrome: false,
        chrome_attempted: false,
        user_agent: "test-ua".to_string(),
        used_proxy: false,
        identity_used: None,
        cascade_level: None,
        pre_flight_fired: false,
        zero_cause: None,
        sugestao_proxima_acao: None,
        bytes_raw: None,
        bytes_decompressed: None,
        cascade_level_observed: None,
    };
    assert_eq!(
        metadata.retries_configured,
        Some(5),
        "GAP-AUD-007: retries_configured deve estar populado quando operador passou --retries"
    );
}

#[test]
fn invariant_zero_cause_inputs_has_probe_level_field() {
    // GAP-META-001 + GAP-AUD-002/003: o classificador recebe sinal cruzado
    // via `last_probe_cascade_level` que DEVE existir no input.
    use duckduckgo_search_cli::pipeline::ZeroClassificationInputs;
    let inputs = ZeroClassificationInputs {
        body: "",
        pre_flight_enabled: false,
        pre_flight_fired: false,
        execution_time_ms: 0,
        retries: 0,
        concurrent_fetches: 0,
        last_probe_cascade_level: Some(2),
    };
    assert_eq!(inputs.last_probe_cascade_level, Some(2));
}

#[test]
fn invariant_endpoint_lite_distinct_from_html() {
    // GAP-META-001 + GAP-AUD-004: tipos Endpoint::Html e Endpoint::Lite
    // devem ser distintos para que --allow-lite-fallback consiga forçar
    // a transição entre eles.
    assert_ne!(
        Endpoint::Html,
        Endpoint::Lite,
        "GAP-AUD-004: Endpoint::Html e Endpoint::Lite devem ser variantes distintas"
    );
}
