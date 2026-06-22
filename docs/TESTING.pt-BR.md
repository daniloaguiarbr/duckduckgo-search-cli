# Guia de Testes

Este guia cobre execução, categorização e integração CI para os testes
de `duckduckgo-search-cli`.

## Adições de Testes em v0.7.3

A release v0.7.3 adicionou 13 testes, todos endereçando o GAP-WS-27 (CAPTCHA no macOS) e seus três fatores de causa raiz:

- **`session_warmup` (5 testes unitários)** — resolução de path XDG no Linux, macOS e Windows; criação de diretório ausente; override de path via `DUCKDUCKGO_SEARCH_CLI_HOME`; estabilidade da constante `DEFAULT_COOKIES_FILENAME`.
- **`cookie_adapter` (3 testes unitários, renomeado de `wreq_cookie_adapter` na v0.8.6)** — `PersistentJar::empty()` produz um `Arc<reqwest::cookie::Jar>` válido; roundtrip `parse_json` preserva cookies via extração do header `CookieStore::cookies()`; roundtrip `save`/`load` com permissões Unix `0o600` e semântica de escrita atômica.
- **`probe_deep` (5 testes unitários)** — `detectar_interstitial` identifica corretamente os marcadores do Cloudflare (`cf-chl-bypass`, `cf-challenge`, `challenge-platform`, `Attention Required`, `__cf_chl_jschl_tk__`); `detectar_interstitial` identifica corretamente os marcadores `robot-detected` e `bots, we have detected` do DuckDuckGo; `sugestao_mitigacao` retorna passos concretos para cada tipo de interstitial; `InterstitialKind::None` é o default para uma resposta HTML normal; `execute_probe_deep` produz um JSON report válido.
- **Total: 405 testes lib passando** (era 279 em v0.7.2; total atual do projeto na v0.7.5). As mudanças v0.7.3 são puramente aditivas. Nenhum teste removido, nenhuma assinatura de teste alterada, nenhuma fixture renomeada.

### Gaps v0.7.3 fechados por estes testes

- **`probe_deep::detectar_interstitial`** — valida que os marcadores são detectados (o custo de um falso negativo é um CAPTCHA não diagnosticado). Cinco marcadores do Cloudflare + dois do DuckDuckGo são testados em isolamento.
- **`cookie_adapter::PersistentJar`** — valida que a ponte JSON ↔ `reqwest::cookie::Jar` não perde cookies durante roundtrip (reescrito na v0.8.6 para usar extração de header `CookieStore::cookies()`). Uma regressão aqui silenciosamente descartaria cookies de sessão, reintroduzindo o GAP-WS-27.
- **`session_warmup::default_cookies_path`** — valida que a resolução XDG está correta por plataforma. Uma regressão aqui colocaria o cookie jar no diretório errado ou falharia em setar permissões `0o600` no Unix.


## Adições de Testes em v0.7.4

v0.7.4 adiciona testes em tempo de build que validam o preflight do build.rs para detecção do assembler NASM em builds nativos Windows MSVC.

- **`build::preflight::nasm`** — 4 testes unitários validando:
  - `nasm_in_path` retorna `true` quando nasm.exe está no PATH
  - `nasm_in_path` retorna `false` quando nasm.exe está ausente
  - `known_nasm_dir` retorna `Some` para `C:\Program Files\NASM` e `C:\Program Files (x86)\NASM`
  - `known_nasm_dir` retorna `None` para caminhos desconhecidos
- **GAP-WS-28 fechado por estes testes** — a mensagem de panic, comando de fix e escape hatch DDG_SKIP_NASM_CHECK=1 são todos validados end-to-end no script de build.
- **Contagem de testes**: ~395 testes lib passando (era 292 na v0.7.3 = +3-5 novos testes de preflight de build).

### Gaps v0.7.4 fechados por estes testes

- **`build::preflight::nasm_in_path`** — valida a lógica de scan para nasm.exe no PATH. Uma regressão aqui faria o preflight v0.7.4+ ou falso-positivo (panic quando NASM está instalado) ou falso-negativo (deixa o build prosseguir para o erro críptico do CMake).
- **`build::preflight::known_nasm_dir`** — valida a heurística de detecção de NASM-instalado-mas-PATH-obsoleto. Uma regressão perderia a dica acionável de que o usuário só precisa atualizar o PATH.

## Adições de Testes em v0.7.5

v0.7.5 estende o preflight de build para detectar 4 ferramentas (NASM, CMake 3.20+, MSVC C/C++, Strawberry Perl) e adiciona testes para os scripts auxiliares.

- **`build::preflight::cmake`** — 3 testes unitários validando heurísticas de cmake_in_path e known_cmake_dir.
- **`build::preflight::msvc`** — 2 testes unitários validando detecção de cl_in_path e link_in_path.
- **`build::preflight::perl`** — 3 testes unitários validando heurísticas de perl_in_path e known_perl_dir.
- **`scripts::check_windows_toolchain`** — 4 testes de integração validando schema de saída JSON e booleano all_present para várias combinações de ferramentas.
- **`scripts::install_windows`** — 1 teste de integração smoke-validando que o modo install-windows.ps1 --check-only emite um relatório parseável.
- **GAP-WS-29/30/31 fechados por estes testes** — cada um dos 4 caminhos de panic do preflight é testado em isolamento, e os 4 escape hatches DDG_SKIP_*_CHECK=1 são validados.
- **Contagem de testes**: 405 testes lib passando (era ~395 na v0.7.4 = +8-13 novos testes de preflight de build + script). Este é o total atual do projeto na v0.7.5.
- **CI cross-platform**: o job windows-2022 em .github/workflows/ci.yml roda os novos testes de preflight de build como parte de cargo test --all-targets --all-features.

### Gaps v0.7.5 fechados por estes testes

- **`build::preflight::cmake_in_path`** — valida o scan de cmake.exe no PATH. Uma regressão deixaria o build v0.7.5+ prosseguir para o panic críptico failed to execute command: program not found do crate cmake.
- **`build::preflight::cl_in_path` e `link_in_path`** — valida detecção de compilador/linker MSVC. Ambos devem estar presentes; detecção parcial é tratada como ausente.
- **`build::preflight::perl_in_path`** — valida detecção do interpretador Perl. Strawberry Perl é o Perl Windows de fato; o teste usa o padrão de filename perl.exe.
- **`scripts::check_windows_toolchain::json_output`** — valida que a saída JSON do script de diagnóstico é parseável e contém as 7 entradas de ferramenta esperadas com booleano found e campos string path.
- **`scripts::install_windows::check_only_mode`** — valida que a flag --check-only produz um relatório sem tentar instalar nada, adequado para portões de CI.

## Adições de Testes em v0.6.5

A release v0.6.5 adicionou 11 testes, todos endereçando gaps anteriormente em aberto:

- **WS-11** (5 testes) — invariantes property-based para o parser HTML em
  `extraction.rs`. Valida que inputs vazios retornam `Vec` vazio, positions
  são densos e 1-based, URLs são normalizados para paths absolutos, o parser
  é determinístico, e HTML malformado não causa panic.
- **WS-12** (4 testes) — circuit breaker per-host em `content_fetch.rs`.
  Valida que o estado closed permite requisições, o threshold abre o breaker,
  um único sucesso reseta o contador de falhas, e o estado half-open é
  alcançável após a janela de cooldown.
- **WS-23** (1 teste) — teste de integração wiremock para o header
  `Retry-After` em respostas HTTP 429.

### Gaps v0.6.5 fechados por estes testes
- **MP-26** (Windows HANDLE) — validado por `cargo test --all-features`
  no runner CI `windows-latest`.
- **CI-01** (6 erros de clippy) — `cargo clippy --all-targets --all-features -- -D warnings`
  agora passa.
- **WS-12** (circuit breaker) — coberto por 4 testes unitários em
  `src/content_fetch.rs`.
- **WS-23** (Retry-After) — coberto por 1 teste wiremock em
  `tests/integration_wiremock.rs`.

## Why Categorized Tests

The test suite is split into four categories to balance speed, isolation,
and coverage:

| Category       | Speed      | Isolation   | Real I/O  | Count (v0.7.5) |
|----------------|------------|-------------|-----------|----------------|
| Unit           | < 1 s      | per-fn      | none      | 292            |
| Integration    | < 30 s     | per-test    | localhost | 99             |
| Doc            | < 5 s      | per-doc     | none      | 0              |
| Loom           | n/a        | n/a         | n/a       | 0 (gated)      |

## Test Categories

### Unit Tests
Located in `src/**/tests` modules (mod tests). Fast, in-process, no I/O.
Run with:

```bash
cargo test --lib
```

### Integration Tests
Located in `tests/*.rs` files. Use wiremock (no real HTTP), assert_cmd (no real
subprocess spawn), and tempfile (no real FS writes outside tmpdir).

```bash
# All integration tests
cargo test --tests

# Single integration test file
cargo test --test integration_wiremock
```

### Doc Tests
Located in `///` examples throughout `src/`. Compiled and executed by `cargo test --doc`.

```bash
cargo test --doc
```

### Loom Tests
Located in `tests/loom_atomics.rs`. Gated by `--cfg loom`. NOT compiled by
default — requires explicit opt-in.

```bash
RUSTFLAGS="--cfg loom" cargo test --test loom_atomics --release
```

> **Known limitation**: Loom conflicts with `hyper-util` and currently
> compiles but does not run cleanly. Issue tracked upstream.


## How to Run

### Local Development

```bash
# Quick feedback loop
timeout 300 cargo test --all-features --locked

# Specific category
cargo test --lib --locked
cargo test --tests --locked
cargo test --doc --locked
```

### With Coverage

```bash
# Install cargo-llvm-cov
cargo install cargo-llvm-cov

# Run with HTML report
cargo llvm-cov --all-features --locked --html --open

# Run with text summary only
cargo llvm-cov --all-features --locked --summary-only
```

Minimum line coverage: **80%**. CI fails below this threshold.

### Property-Based Tests (v0.6.5, WS-11)

5 invariants in `src/extraction.rs`:

```bash
cargo test ws11_
# Run all 5 property tests:
# - ws11_invariant_empty_inputs_yield_empty_results
# - ws11_invariant_positions_are_dense_and_one_based
# - ws11_invariant_urls_are_normalized_to_absolute
# - ws11_invariant_extraction_is_idempotent
# - ws11_invariant_malformed_html_does_not_panic
```

### WireMock Retry-After Test (v0.6.5, WS-23)

```bash
cargo test --test integration_wiremock test_retry_after_header_respected
```

### Circuit Breaker Tests (v0.6.5, WS-12)

```bash
cargo test ws12_
# Tests: ws12_breaker_allows_when_closed,
#        ws12_breaker_opens_after_threshold_failures,
#        ws12_breaker_resets_on_success,
#        ws12_breaker_half_opens_after_cooldown
```


## Environment Variables

| Variable                        | Effect                                                |
|---------------------------------|-------------------------------------------------------|
| `RUST_TEST_THREADS`             | Number of parallel test threads (default 1)            |
| `RUST_BACKTRACE`                | Set to `1` or `full` for detailed backtraces           |
| `RUST_LOG`                      | Tracing filter (`debug`, `info`, `warn`, `error`)     |
| `CARGO_TERM_COLOR`              | Force ANSI colors (`always`, `never`, `auto`)         |
| `LOOM_MAX_PREEMPTIONS`          | Max preemption bound for loom tests                    |
| `WIREMOCK_LOG`                  | WireMock request/response logging                      |


## CI Profiles

Three CI jobs run the test suite:

1. **`validate` matrix** — `cargo test --all-features --locked` on Linux, macOS, Windows
2. **`msrv`** — `cargo check --all-targets --all-features --locked` on Rust 1.88 (MSRV desde v0.7.2)
3. **`coverage`** — `cargo llvm-cov --all-features --locked --fail-under-lines 80` on Linux

Plus a manual `cargo nextest` profile available locally:

```toml
# .config/nextest.toml (not in repo, per project convention)
[profile.default]
retries = 2
test-threads = 1
```


## Troubleshooting

### `flaky::lazy_template` failures
Loom tests may be flaky. Re-run with:

```bash
RUSTFLAGS="--cfg loom" cargo test --test loom_atomics --release -- --test-threads=1
```

### `wiremock::MockServer` startup timeout
Increase the wait:

```bash
WIREMOCK_LOG=info cargo test --test integration_wiremock
```

### Coverage drops below 80%
Check the HTML report for uncovered lines:

```bash
cargo llvm-cov --html --open
```

The diff will show which lines are not exercised by the test suite. Add
unit or integration tests to cover the missing branches.

### Tests pass locally but fail in CI
- Check for environment-specific behavior (paths, timeouts, locale)
- Check for `Instant::now()` non-determinism in code under test
- Use `cargo nextest` with retries to detect flaky tests:

```bash
cargo nextest run --retries 3
```


## Adições de Testes em v0.7.6

A v0.7.6 fecha o GAP-WS-48 (fix de mesmo dia do `cargo install`) e adiciona testes de regressão para o conflito de dependência.

- **`build::install::alloc_no_stdlib_pin`** — 2 testes unitários validando que o pin `alloc-no-stdlib = "2.0.4"` é respeitado no `cargo install` e não sofre upgrade silencioso para 3.0.0.
- **`build::install::brotli_decompressor_pin`** — 1 teste unitário validando que o pin `brotli-decompressor = "5.0.1"` sobrevive à resolução em toolchain limpa.
- **`integration::install_clean_toolchain`** — 1 teste de integração que roda `cargo install --path . --offline` em um `target/` novo e asserta exit 0.
- **GAP-WS-48 fechado por estes testes** — cada pin de dependência que o fix da v0.7.6 depende tem um teste dedicado.
- **Contagem de testes**: 408 testes lib passando (era 405 na v0.7.5 = +3 novos testes de pin de install). Este é o total do projeto na v0.7.6.
- **Portão CI**: os novos testes de install rodam no job `install-check` do CI junto com os testes de preflight da v0.7.5.

### Gaps v0.7.6 fechados por estes testes

- **`build::install::alloc_no_stdlib_pin`** — previne o conflito `2.0.4` vs `3.0.0` de reaparecer silenciosamente. Uma regressão re-dispararia o panic original do `cargo install`.
- **`build::install::brotli_decompressor_pin`** — mantém o decoder brotli do BoringSSL fixado em versão conhecida como boa. Uma regressão quebraria o build do source no Linux.
- **`integration::install_clean_toolchain`** — portão de install end-to-end que captura qualquer novo conflito de dependência antes da publicação.


## Adições de Testes em v0.7.7

> **v0.8.6+**: Os testes `tls::emulation` abaixo foram REMOVIDOS quando `wreq` foi substituído por `reqwest` + `rustls-tls`. Ver ADR-0008. Os testes de preflight de build em v0.7.4–v0.7.5 (NASM, CMake, MSVC, Perl) também foram removidos pois os preflights não existem mais no `build.rs`.

A v0.7.7 fecha o GAP-WS-49 (regressão de fingerprint TLS) e adiciona testes de regressão para o stack `wreq` + `wreq-util` emulation. **(Histórico — testes removidos na v0.8.6.)**

- **`tls::emulation::wreq_util_present`** — 2 testes unitários validando que `wreq-util 3.0.0-rc` com `features = ["emulation"]` está na árvore de dependências resolvida. **(Removido na v0.8.6.)**
- **`tls::emulation::brotli_feature_enabled`** — 1 teste unitário validando que a feature `brotli` do `wreq` está habilitada (necessária para o stack de emulation compilar). **(Removido na v0.8.6.)**
- **`tls::probe_deep::captcha_classification`** — 1 teste de integração que roda `--probe-deep` contra endpoint real do DuckDuckGo e asserta que o envelope JSON contém `status`, `cascata_motivo` e `sugestao_mitigacao`.
- **`tls::probe_deep::ok_envelope`** — 1 teste de integração que asserta que o envelope de sucesso bate com o schema documentado em `docs/HOW_TO_USE.pt-BR.md`.
- **GAP-WS-49 fechado por estes testes** — o stack de emulation é trancado no nível de dependência e validado end-to-end.
- **Contagem de testes**: 413 testes lib + integration passando (era 408 na v0.7.6 = +5 novos testes de re-registro TLS). Este é o total do projeto na v0.7.7.
- **Portão CI**: os testes TLS rodavam no job `tls-emulation` do CI em v0.7.7–v0.8.5. **(Removido na v0.8.6 — wreq eliminado.)**

### Gaps v0.7.7 fechados por estes testes (histórico — substituído pela v0.8.6)

- **`tls::emulation::wreq_util_present`** — prevenia outra remoção acidental de `wreq-util`. **(Substituído: wreq-util removido na v0.8.6.)**
- **`tls::emulation::brotli_feature_enabled`** — mantinha a feature `brotli` no grafo de build. **(Substituído: brotli removido na v0.8.6.)**
- **`tls::probe_deep::captcha_classification`** — valida o formato do portão CI para `--probe-deep`. Uma regressão deixaria o portão retornar exit 0 em resposta de captcha.
- **`tls::probe_deep::ok_envelope`** — valida o JSON do caminho de sucesso. Uma regressão quebraria consumidores downstream do CI que parseiam o envelope.


## Adições de Testes em v0.7.8

A v0.7.8 fecha 8 gaps (GAP-WS-50 até GAP-WS-57) e adiciona testes de regressão para cada. A renovação do detector é o maior delta.

- **`probe_deep::markers::cloudflare`** — 4 testes unitários validando os 4 markers novos do Cloudflare (`anomaly-modal`, `anomaly.js`, `botnet`, `Unfortunately, bots`) contra fixtures HTML reais em `tests/fixtures/`.
- **`probe_deep::markers::ddg`** — 1 teste unitário validando o novo marker `anomaly-modal__title` do DDG.
- **`probe_deep::markers::legacy`** — 3 testes unitários validando que markers legados (`cf-chl-bypass`, `cf-challenge`, `robot-detected`) ainda casam.
- **`cli::verbose::count_levels`** — 1 teste unitário validando que `-v` (1), `-vv` (2), `-vvv` (3) parseiam corretamente via `ArgAction::Count`.
- **`cli::verbose::conflicts_with_quiet`** — 1 teste unitário validando que `--verbose` e `--quiet` juntos falham a validação do clap.
- **`search_retry::retries_honored`** — 1 teste de integração em `tests/integration_search_retry.rs` validando que `--retries 5` produz `metadados.retentativas == 5` no JSON.
- **`search_retry::clamp_to_ten`** — 1 teste de integração validando que `--retries 999` é clampado para 10 com aviso.
- **`search::fallback_lite_opt_in`** — 2 testes unitários validando que `--allow-lite-fallback` não aciona quando o usuário não passou o flag.
- **`search::fallback_lite_with_interstitial`** — 2 testes unitários validando que o fallback aciona quando o detector classifica interstitial e o flag está on.
- **Contagem de testes**: 305 lib + 18 testes de integration passando (era 292 lib + 13 integration na v0.7.7 = +10 novos testes v0.7.8). Este é o total do projeto na v0.7.8.
- **Portão CI**: os testes de marker rodam no job `detector-markers` do CI; os testes de retry rodam no job `retry-pipeline` do CI.

### Gaps v0.7.8 fechados por estes testes

- **`probe_deep::markers::cloudflare` e `ddg`** — trancam a lista de markers pós-2026. Uma regressão ao detector só-de-legacy re-abriria o GAP-WS-50.
- **`cli::verbose::count_levels`** — tranca a semântica de `ArgAction::Count`. Uma regressão ao `verbose: bool` único re-abriria o GAP-WS-53.
- **`cli::verbose::conflicts_with_quiet`** — previne a combinação contraditória de flags. Uma regressão deixaria operadores se frustrarem.
- **`search_retry::retries_honored`** — tranca a propagação de `cfg.retries`. Uma regressão ao `1` hard-coded re-abriria o GAP-WS-57.
- **`search_retry::clamp_to_ten`** — tranca o clamp `[1, 10]`. Uma regressão deixaria `--retries 999` acionar detecção anti-bot.
- **`search::fallback_lite_opt_in`** — tranca o contrato de opt-in. Uma regressão a fallback incondicional re-abriria o GAP-WS-52.
- **`search::fallback_lite_with_interstitial`** — tranca o predicado `detectar_interstitial`. Uma regressão a `accumulated_results.is_empty()` deixaria Lite acionar em queries vazias legítimas.


## Testes Chrome Stealth (v0.8.0)
- Testes stealth do Chrome requerem `xvfb-run` em Linux headless
- Execute com: `xvfb-run --auto-servernum --server-args="-screen 0 1920x1080x24" cargo test`
- `tests/integration_chrome_stealth.rs` valida injeção de sinais stealth
- `tests/integration_deep_research.rs` valida pipeline Chrome no deep-research
- Testes unitários em `src/browser.rs` validam argumentos de `flags_stealth()`
- 378 testes passam com a feature Chrome habilitada
- Para pular testes Chrome: `cargo test --no-default-features`
