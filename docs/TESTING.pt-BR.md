# Guia de Testes

Este guia cobre execução, categorização e integração CI para os testes
de `duckduckgo-search-cli`.

## Adições de Testes em v0.7.3

A release v0.7.3 adicionou 13 testes, todos endereçando o GAP-WS-27 (CAPTCHA no macOS) e seus três fatores de causa raiz:

- **`session_warmup` (5 testes unitários)** — resolução de path XDG no Linux, macOS e Windows; criação de diretório ausente; override de path via `DUCKDUCKGO_SEARCH_CLI_HOME`; estabilidade da constante `DEFAULT_COOKIES_FILENAME`.
- **`wreq_cookie_adapter` (3 testes unitários)** — `PersistentJar::empty()` produz um `Arc<dyn CookieStore>` válido; roundtrip `parse_json` preserva cookies através da fronteira `wreq::cookie::Jar`; roundtrip `save`/`load` com permissões Unix `0o600` e semântica de escrita atômica.
- **`probe_deep` (5 testes unitários)** — `detectar_interstitial` identifica corretamente os marcadores do Cloudflare (`cf-chl-bypass`, `cf-challenge`, `challenge-platform`, `Attention Required`, `__cf_chl_jschl_tk__`); `detectar_interstitial` identifica corretamente os marcadores `robot-detected` e `bots, we have detected` do DuckDuckGo; `sugestao_mitigacao` retorna passos concretos para cada tipo de interstitial; `InterstitialKind::None` é o default para uma resposta HTML normal; `execute_probe_deep` produz um JSON report válido.
- **Total: 292 testes lib passando** (era 279 em v0.7.2 = +13 novos). As mudanças v0.7.3 são puramente aditivas. Nenhum teste removido, nenhuma assinatura de teste alterada, nenhuma fixture renomeada.

### Gaps v0.7.3 fechados por estes testes

- **`probe_deep::detectar_interstitial`** — valida que os marcadores são detectados (o custo de um falso negativo é um CAPTCHA não diagnosticado). Cinco marcadores do Cloudflare + dois do DuckDuckGo são testados em isolamento.
- **`wreq_cookie_adapter::PersistentJar`** — valida que a ponte JSON ↔ `wreq::cookie::Jar` não perde cookies durante roundtrip. Uma regressão aqui silenciosamente descartaria cookies de sessão, reintroduzindo o GAP-WS-27.
- **`session_warmup::default_cookies_path`** — valida que a resolução XDG está correta por plataforma. Uma regressão aqui colocaria o cookie jar no diretório errado ou falharia em setar permissões `0o600` no Unix.

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

| Category       | Speed      | Isolation   | Real I/O  | Count (v0.7.3) |
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
