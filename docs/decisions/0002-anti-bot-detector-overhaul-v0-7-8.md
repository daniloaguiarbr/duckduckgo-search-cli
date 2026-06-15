# ADR-0002 — Renovação do Detector Anti-Bot + Verbose Acumulado (v0.7.8)

## Contexto e problema

A versão v0.7.7 (commit `118738a`) restaurou o fingerprint TLS via `wreq 6.0.0-rc.29` + `wreq-util 3.0.0-rc.12` (fechando GAP-WS-49), mas três deficiências funcionais persistiam em 2026-06-14:

1. **DDG rolled out `anomaly-modal` interstitial** (classes CSS `anomaly-modal__mask`, `anomaly-modal__title`; challenge URL `anomaly.js?cc=botnet`; copy "Unfortunately, bots use DuckDuckGo too.") que escapava do detector de intersticial herdado, causando exit 5 silencioso com zero resultados. O detector só reconhecia os markers legados `cf-chl-bypass`, `cf-challenge`, `robot-detected`, `bots, we have detected`.
2. **Probe-deep usava query curta `q=rust` (4 chars)** que DDG trata diferente de queries longas — o probe reportava `status: ok` falsamente em ambiente onde queries reais de 3+ palavras acionavam bot scoring. Operadores acreditavam em saúde do ambiente e pipelines disparam queries que retornavam zero.
3. **Fallback `html → lite` era incondicional** quando `accumulated_results.is_empty()` — violava o contrato opt-in de `--allow-lite-fallback` documentado em `cli.rs`. Usuários sem a flag recebiam fallback Lite silencioso.

Adicionalmente, dois achados de supply chain (`scraper 0.20` → transitiva `fxhash 0.2.1` RUSTSEC-2025-0057) e duas melhorias de UX (verbose `-vv` não acumulava, help do subcomando `buscar` inflava `--help`) compunham o conjunto de 8 gaps a fechar na v0.7.8.

## Opções consideradas

### Opção 1 — Migrar para o crate `captcha-detect` (não estável)

Avaliada e rejeitada. Crate externo sem releases estáveis, sem cobertura para o interstitial específico do DDG, e adiciona uma dependência nova para resolver um problema que uma lista de strings resolve nativamente.

### Opção 2 — Manter `detectar_interstitial` como heurística textual mas expandir a lista de markers (escolhida)

Estender as constantes `CLOUDFLARE_MARKERS` e `DDG_MARKERS` em `src/probe_deep.rs` com 8 marcadores novos do Cloudflare (post-2026: `anomaly-modal`, `anomaly-modal__mask`, `anomaly-modal__title`, `anomaly.js?cc=botnet`, `cf-turnstile`, `cf-spinner`, `Just a moment`, `cf-mitigated`) e 1 marcador novo do DDG (`Unfortunately, bots use DuckDuckGo too.`). Markers legados foram preservados para compatibilidade com templates pré-2026.

Para a query de calibração do probe, substituir o literal `"rust"` por uma constante `PROBE_CALIBRATION_QUERY = "the quick brown fox jumps over the lazy dog"` (9 palavras, 43 chars) no topo de `src/lib.rs`. Query longa aciona o bot scoring upstream de forma reprodutível.

Para o fallback, alterar o predicado em `src/search.rs:559` para `accumulated_results.is_empty() && initial_endpoint == Endpoint::Html && cfg.allow_lite_fallback && matches!(detectar_interstitial(&first_html), InterstitialKind::Cloudflare | InterstitialKind::DuckDuckGo)`. Quando o detector classifica interstitial mas a flag está desabilitada, logar `tracing::warn!` estruturado com `kind = interstitial_kind.as_str()` para indicar ao operador que existe mitigação.

### Opção 3 — Carregar o detector de um arquivo de configuração externo

Rejeitada. Aumenta superfície de ataque (arquivo de config pode ser manipulado), viola o princípio YAGNI, e a lista de markers é estável o suficiente para ser hard-coded com cobertura de testes.

## Decisão

Adotada a Opção 2.

**Mudanças resultantes (8 gaps):**

- **GAP-WS-50**: `src/probe_deep.rs:53-66` listas expandidas (8 CF + 1 DDG); 8 testes unitários novos em `src/probe_deep.rs::tests` (não em `tests/integration_probe_deep.rs` como o CHANGELOG cita incorretamente — correção cosmética em revisão posterior).
- **GAP-WS-51**: `src/lib.rs:91, 509` constante `PROBE_CALIBRATION_QUERY`.
- **GAP-WS-52**: `src/search.rs:567-572` predicado de fallback condicional; `tracing::warn!` estruturado quando detector flagra mas flag está off.
- **GAP-WS-53**: `src/cli.rs:418-419` `pub verbose: u8` com `action = ArgAction::Count, conflicts_with = "quiet"`.
- **GAP-WS-54**: `Cargo.toml:130` `scraper = "0.27"`; `deny.toml` sem `RUSTSEC-2025-0057`; `.github/workflows/{ci,release}.yml` com `cargo audit --deny warnings --ignore RUSTSEC-2025-0052`.
- **GAP-WS-55**: `Cargo.toml:62-97` bloco wreq reescrito (sem "regressed to wreq 5.3.0"); reflete decisão real (pin em 6.0.0-rc.29 + 3 pins diretos: `wreq-util`, `brotli-decompressor =5.0.1`, `alloc-no-stdlib =2.0.4`).
- **GAP-WS-56**: `src/cli.rs:164` `#[command(hide = true)]` em `Buscar`.
- **GAP-WS-57**: `src/parallel.rs:644` `retries: config.retries` (dentro de `error_output`).

## Consequências

### Positivas

- Detector de interstitial agora reflete os templates pós-2026 do DDG e do Cloudflare Bot Management.
- Probe-deep dá sinal honesto do ambiente em vez de falso negativo.
- Fallback Lite respeita o contrato opt-in documentado.
- `-vv` e `-vvv` seguem convenção Unix; `RUST_LOG` continua sobrescrevendo.
- Subcomando `buscar` some do `--help` global; invocação direta continua funcional.
- `cargo audit --deny warnings` passa em CI; gate adicionado em `ci.yml` e `release.yml`.
- 305 testes lib + 18 integration passando; 0 advisories não-ignorados.

### Negativas

- Lista de markers é frágil por natureza: novos rollouts do DDG/Cloudflare exigem atualização manual. Mitigação: testes em `src/probe_deep.rs::tests` documentam cada marker; futura migração para `captcha-detect` (Opção 1) fica disponível se a lista crescer além de 20 entries.
- `scraper 0.27` pode introduzir breaking changes sutis em serialização de `Selector`. Mitigação: API `Selector::parse` é compatível; nenhum call site precisou de refactor.
- ADR-0002 retroativo: a decisão foi tomada durante a execução das fases 1-4 do plano v0.7.8 e este ADR é a formalização tardia. Mitigação: este ADR é a fonte canônica para decisões futuras sobre o detector.

### Neutras

- Comentário em `Cargo.toml:62-97` sobre wreq passa de "regressão inexistente" para "decisão intencional de pin" — texto mais longo mas rastreável.
- `tests/integration_probe_deep.rs` foi citado no CHANGELOG mas os testes vivem em `src/probe_deep.rs::tests` — imprecisão cosmética a corrigir.

## Validação

- `cargo check --offline`: exit 0
- `cargo clippy --all-targets --offline -- -D warnings`: exit 0
- `cargo build --release --offline`: exit 0
- `cargo test --offline --lib`: 305 passed, 0 failed
- `cargo test --offline --tests`: 18 passed, 0 failed
- `cargo audit --deny warnings --ignore RUSTSEC-2025-0052`: exit 0
- `cargo install --path . --offline`: exit 0 (pre-publish gate)

## Status

Aceita. Implementada na v0.7.8 (working tree em 2026-06-15; T7.1 tag/publish aguardando autorização do operador).
