# Guia de Migração

Este guia cobre caminhos de migração entre versões do `duckduckgo-search-cli`.
Cada seção documenta mudanças que quebram compatibilidade, mudanças aditivas
e instruções de rollback.

## Migração v0.7.2 → v0.7.3

### O que muda
- **QUEBRA DE AMBIENTE DE BUILD (apenas builds do código-fonte)**: A stack TLS mudou de `rustls` para BoringSSL via `wreq 6.0.0-rc.29`. Compilar do código-fonte no Linux agora requer `cmake`, `perl`, `pkg-config` e `libclang-dev`. Binários pré-compilados do crates.io não são afetados. A matrix `.github/workflows/release.yml` instala esses pacotes automaticamente.
- **GAP-WS-27 fechado**: O interstitial de CAPTCHA no macOS está corrigido. A mesma query que retornava `quantidade_resultados: 0` na v0.7.2 retorna 5 resultados na v0.7.3 na mesma máquina. Ver `gaps.md` e `docs/decisions/0001-tls-boring-via-wreq.md`.
- **Novas flags CLI (aditivas)**:
  - `--no-warmup` — pula o warm-up `GET https://duckduckgo.com/` antes da primeira query real
  - `--no-cookie-persistence` — mantém cookies em memória apenas; nunca grava `cookies.json` em disco
  - `--cookies-path <PATH>` — sobrescreve o path XDG padrão do cookie jar
  - `--probe-deep` — executa uma query real e classifica o body como `ok` ou `captcha` baseado em marcadores Cloudflare e DuckDuckGo
  - `--allow-lite-fallback` — opt-in para fallback automático do endpoint `html` para `lite` quando `--probe-deep` (ou retentativas de zero resultados) detectam CAPTCHA
- **Novo estado persistente: cookie jar**: Um arquivo `cookies.json` agora é gravado em `~/.config/duckduckgo-search-cli/cookies.json` (Linux), `%APPDATA%\duckduckgo-search-cli\cookies.json` (Windows), ou `~/Library/Application Support/duckduckgo-search-cli/cookies.json` (macOS). Permissões Unix são `0o600` (owner read+write only). Trate este arquivo como trataria uma credencial — ver `SECURITY.pt-BR.md`. Use `--no-cookie-persistence` para desabilitar.
- **Zero mudanças no schema JSON de saída**. Todos os campos da v0.7.2 permanecem presentes. Nenhum campo `Option<T>` novo adicionado no nível superior.
- **Novas dependências**: `wreq 6.0.0-rc.29`, `wreq-util 3.0.0-rc.12`, mais as transitivas `boring2 4.15.11`, `webpki-root-certs 1.0.7`, e a toolchain C do BoringSSL.
- **Dependências removidas**: `reqwest 0.12.28`. `time 0.3.47` não é mais dep direta — puramente transitiva agora.
- **Contagem de testes: 292 lib** (era 279 na v0.7.2). +13 novos testes em `session_warmup` (5), `wreq_cookie_adapter` (3), e `probe_deep` (5). 0 warnings de clippy, 0 diff de fmt, 2 warnings de cargo-deny (RUSTSEC-2025-0057 + RUSTSEC-2025-0052, ambos já na lista de ignore).
- **Tamanho do binário**: +20 MB (BoringSSL estaticamente vinculado). Tempo de build de release: ~40s mais longo que v0.7.2 (BoringSSL compila).

### Migração passo-a-passo

```bash
# Atualize para v0.7.3 (binário pré-compilado — sem deps de source build)
cargo install duckduckgo-search-cli --version 0.7.3 --force

# Verifique a nova versão
duckduckgo-search-cli --version
# duckduckgo-search-cli 0.7.3

# Verifique a correção do GAP-WS-27 no macOS
duckduckgo-search-cli "rust wreq emulation browser fingerprint" -q -f json --num 5
# Esperado: 5 resultados em menos de 2 segundos, sem CAPTCHA

# Teste o novo probe-deep (detecção de CAPTCHA)
duckduckgo-search-cli --probe-deep -q -f json
# Esperado: {"status": "ok", "cascata_motivo": "none", "sugestao_mitigacao": "..."}

# Build do código-fonte (apenas se for compilar — não é necessário para `cargo install`)
sudo apt install cmake perl pkg-config libclang-dev
git checkout v0.7.3
cargo build --release
```

### Mudanças no schema JSON

Nenhuma mudança de schema. v0.7.3 preserva todos os campos da v0.7.2:

| Campo                          | Status    | Notas                                       |
|--------------------------------|-----------|---------------------------------------------|
| `.resultados[].titulo`         | inalterado | Sempre presente quando não vazio           |
| `.resultados[].url`            | inalterado | Sempre presente quando não vazio           |
| `.metadados.identidade_usada`  | inalterado | `Option<String>` — v0.6.4+                |
| `.metadados.nivel_cascata`     | inalterado | `Option<u32>` (0..=4) — v0.6.4+           |
| `.metadados.usou_endpoint_fallback` | inalterado | `bool` — v0.6.0+                        |

O arquivo `cookies.json` é estado interno e não é exposto no schema JSON de saída.

### Notas de compatibilidade
- O binário v0.7.3 é API-compatível com v0.7.2 (sem remoções de flag CLI, sem remoções de campo JSON)
- Os alvos de build de v0.7.3 permanecem inalterados: `x86_64-unknown-linux-gnu`, `x86_64-unknown-linux-musl`, `aarch64-apple-darwin`, `x86_64-apple-darwin`, `x86_64-pc-windows-msvc`
- Binários v0.7.2 que funcionavam em Linux/macOS continuam funcionando — sem upgrade urgente necessário **a menos que** você tenha sido afetado pelo CAPTCHA do macOS (GAP-WS-27)
- Usuários de macOS que tiveram queries com zero resultados na v0.7.2 devem atualizar para v0.7.3 para corrigir o CAPTCHA. A correção é estrutural (fingerprint TLS), não uma solução alternativa.
- A nova dependência `wreq 6.0.0-rc.29` é instalada automaticamente por `cargo install`.

### Rollback

Se precisar voltar para v0.7.2 (ex.: algum problema inesperado de build do BoringSSL):

```bash
# Instalar uma versão específica mais antiga
cargo install duckduckgo-search-cli --version 0.7.2 --force
```

> **Nota**: v0.7.2 foi a versão afetada pelo GAP-WS-27. Voltar reintroduz o bug do CAPTCHA do macOS. Faça isso apenas se v0.7.3 tiver um problema crítico na sua plataforma.

### Veja também

- `CHANGELOG.pt-BR.md` — changelog completo
- `gaps.md` — entrada do GAP-WS-27 com reprodução empírica
- `docs/decisions/0001-tls-boring-via-wreq.md` — decisão arquitetural
- `docs/CROSS_PLATFORM.pt-BR.md` — pré-requisitos de build do BoringSSL
- `SECURITY.pt-BR.md` — tratamento do cookie jar
- `README.pt-BR.md` — overview e início rápido


## Migração v0.7.1 → v0.7.2

### O que muda
- **Zero quebras.** Todas as flags CLI, schemas JSON de saída e exit codes da v0.7.1 permanecem inalterados.
- **Correção de advisory de segurança (RUSTSEC-2026-0009)**: `time 0.3.40` denial-of-service via RFC 2822 stack exhaustion estava sendo puxado transitivamente via `cookie_store 0.22.0` → `reqwest 0.12.28`. v0.7.2 fixa `time = "0.3.47"` como dep direta para sobrescrever a constraint transitiva.
- **Migração `rand` 0.10**: dev-deps (proptest 1.11+, getrandom 0.4+) unificaram em rand 0.10 e os métodos de conveniência migraram de `Rng` para `RngExt`. Todos os call sites internos atualizados: `random_range`, `random_bool`, `random`, e `IndexedRandom::choose`.
- **Bump de MSRV**: `rust-version` saltou de 1.75 para 1.88 (requerido por `time 0.3.47+` e `rand 0.10`).
- **Correção de higiene de CI**: 6 erros latentes de clippy que estavam quebrando silenciosamente a matrix CI na v0.7.1 são capturados agora por `cargo clippy --all-targets --all-features -- -D warnings`.

### Migração passo-a-passo

```bash
# Atualize para v0.7.2
cargo install duckduckgo-search-cli --version 0.7.2 --force

# Verifique a nova versão
duckduckgo-search-cli --version
# duckduckgo-search-cli 0.7.2

# Verifique que a migração do rand 0.10 funciona
duckduckgo-search-cli "rust async" -q -f json | jaq '.resultados[].titulo'
```

### Mudanças no schema JSON

Nenhuma mudança de schema. v0.7.2 preserva todos os campos da v0.7.1.

### Rollback

```bash
cargo install duckduckgo-search-cli --version 0.7.1 --force
```


## Migração v0.7.0 → v0.7.1

### O que muda
- **Zero quebras.** Todas as flags CLI, schemas JSON de saída e exit codes da v0.7.0 permanecem inalterados.
- **Migração de dependência (interna)**: `rand` saltou de `0.8` para `0.9` para alinhar com `proptest 1.11+` (dev-dep). Todos os call sites internos atualizados.
- **Bump de MSRV**: `rust-version` saltou de `1.75` para `1.85` para satisfazer MSRV de `rand 0.9` e a onda de deps transitivas edition-2024.
- **Limpeza do builder reqwest**: chamadas `ClientBuilder::gzip(true)` e `.brotli(true)` removidas.
- **Higiene de CI**: dois warnings de `actionlint` shellcheck corrigidos.
- **Ignore de advisory de segurança**: `RUSTSEC-2026-0009` (time 0.3.40 DoS) adicionado à lista de ignore do `deny.toml`.

### Migração passo-a-passo

```bash
# Atualize para v0.7.1
cargo install duckduckgo-search-cli --version 0.7.1 --force

# Verifique
duckduckgo-search-cli --version
# duckduckgo-search-cli 0.7.1
```

### Mudanças no schema JSON

Nenhuma mudança de schema. v0.7.1 preserva todos os campos da v0.7.0.

### Rollback

```bash
cargo install duckduckgo-search-cli --version 0.7.0 --force
```


## Migração v0.6.x → v0.7.0

### O que muda
- **Apenas aditivo** — v0.7.0 é totalmente retrocompatível com v0.6.x. O subcomando `buscar`, o schema JSON de configuração padrão, cada flag existente e cada exit code permanecem byte-for-byte idênticos.
- **Novo subcomando público** `deep-research` para pesquisa multi-hop por LLM. Operadores que não invocam `deep-research` não veem mudança observável.
- **Quatro novos módulos públicos** em `lib.rs` — `deep_research`, `decomposition`, `aggregation`, `synthesis` — composíveis a partir de crates downstream.
- **Novas dependências diretas** em `Cargo.toml`: `url = "2"`, `regex = "1"`, e `proptest = "1"` (dev-only). As três são adições puras; nenhuma dependência foi atualizada ou removida.

### O que atualizar no seu pipeline
- Se você roteiriza contra o enum `Subcommand`, adicione um braço de match para `Subcommand::DeepResearch(DeepResearchArgs)`.
- Se você consome `lib::run` diretamente, roteie `args.subcommand` para `lib::execute_deep_research` (o helper que constrói um `Config` padrão e chama o pipeline).
- Se você fixa uma versão mínima suportada em `Cargo.toml` de uma crate downstream, atualize para `duckduckgo-search-cli = "0.7"`.
- Nenhuma migração de schema JSON é necessária: os schemas `SearchOutput` e `MultiSearchOutput` permanecem inalterados.

### Rollback
- Fixe em `duckduckgo-search-cli = "0.6.5"` em crates downstream; o binário no crates.io é totalmente retrocompatível.

## Migration v0.6.4 → v0.6.5

### What Changes
- **No breaking changes** — v0.6.5 is fully backward-compatible with v0.6.4
- Windows build was broken in v0.6.4 and is fixed in v0.6.5
- CI now passes on all 3 SOs (Linux/macOS/Windows) — v0.6.4 had failing CI
- New `--fetch-content` long crawls now show a ProgressBar on stderr (auto-hidden in pipes)
- 5 new property tests in `extraction.rs`, 4 new circuit breaker tests, 1 new wiremock test

### Step-by-Step Migration

```bash
# Update to v0.6.5
cargo install duckduckgo-search-cli --version 0.6.5 --force

# Verify the new version
duckduckgo-search-cli --version
# duckduckgo-search-cli 0.6.5
```

### JSON Schema Changes

No schema changes. v0.6.5 preserves all v0.6.4 fields:

| Field                          | Status    | Notes                                      |
|--------------------------------|-----------|--------------------------------------------|
| `.resultados[].titulo`         | unchanged | Always present when non-empty              |
| `.resultados[].url`            | unchanged | Always present when non-empty              |
| `.metadados.identidade_usada`  | unchanged | `Option<String>` — v0.6.4+                |
| `.metadados.nivel_cascata`     | unchanged | `Option<u32>` (0..=4) — v0.6.4+           |

### Compatibility Notes

- v0.6.5 binary is API-compatible with v0.6.4 (no CLI flag removals, no JSON field removals)
- v0.6.5 build targets are unchanged: `x86_64-unknown-linux-gnu`,
  `x86_64-unknown-linux-musl`, `aarch64-apple-darwin`,
  `x86_64-apple-darwin`, `x86_64-pc-windows-msvc`
- v0.6.4 binaries that worked on Linux/macOS continue to work — no urgent upgrade required
- v0.6.4 binaries that failed on Windows will succeed after upgrading to v0.6.5

### Rollback

If you need to roll back to v0.6.4 (e.g., for Windows users until you can deploy v0.6.5):

```bash
# Install a specific older version
cargo install duckduckgo-search-cli --version 0.6.4 --force
```

> **Note**: v0.6.4 was published with a broken Windows build. It is recommended
> to upgrade to v0.6.5 as soon as possible on Windows. On Linux/macOS, v0.6.4
> is functional and can be retained if needed.

### See Also

- `CHANGELOG.md` — full changelog
- `docs/CROSS_PLATFORM.md` — platform-specific notes
- `SECURITY.md` — vulnerability disclosure
- `README.md` — overview and quick start


## Migration v0.6.3 → v0.6.4

### What Changes
- New `--probe` flag for pre-flight health checks
- New `--identity-profile <auto|chrome-win|...>` flag to pin identity
- New `--seed` semantics (now also controls identity pool rotation)
- New optional JSON fields `.metadados.identidade_usada` and `.metadados.nivel_cascata`
- New 12-identity adaptive anti-bot pool (WS-26)

### Step-by-Step Migration

```bash
# Update to v0.6.4
cargo install duckduckgo-search-cli --version 0.6.4 --force

# Verify
duckduckgo-search-cli --version
```

### JSON Schema Changes

All new fields are `Option<T>` (additive, non-breaking):

| Field                          | Type           | Added in   | Notes                            |
|--------------------------------|----------------|------------|----------------------------------|
| `.metadados.identidade_usada`  | `Option<String>` | v0.6.4     | Format `<family>-<platform>-<16hex>` |
| `.metadados.nivel_cascata`     | `Option<u32>`    | v0.6.4     | Cascade level 0..=4              |

### Compatibility Notes
- v0.6.4 is API-compatible with v0.6.3 (no breaking changes)
- All 313 tests in v0.6.4 pass identically against v0.6.3 schemas

### Rollback

```bash
cargo install duckduckgo-search-cli --version 0.6.3 --force
```


## Migration v0.6.2 → v0.6.3

### What Changes
- All `///` doc comments translated from Portuguese to English
- Zero code behavior changes

### Step-by-Step Migration

```bash
cargo install duckduckgo-search-cli --version 0.6.3 --force
```

### JSON Schema Changes
None. v0.6.3 is binary-compatible with v0.6.2.

### Rollback

```bash
cargo install duckduckgo-search-cli --version 0.6.2 --force
```


## Migration v0.6.1 → v0.6.2

### What Changes
- Documentation-only release: 19 new bilingual files (EN + PT-BR)
- `llms.txt`, `llms-full.txt` added for LLM discovery
- `CODE_OF_CONDUCT.md` (Contributor Covenant 2.1)
- `eval-queries.json` × 2 (20 EN + 20 PT-BR)

### Step-by-Step Migration
None required — documentation-only release.

### JSON Schema Changes
None.


## Migration v0.6.0 → v0.6.1

### What Changes
- `--timeout 0` now returns exit code 2 (invalid config) instead of executing a search with zero timeout
- `--output /tmp/../../etc/passwd` now returns exit code 2 (invalid config) instead of exit 1
- New `validar_timeout_segundos()` method on `CliArgs`
- Early path traversal check in `montar_configuracacoes()`

### Step-by-Step Migration
None required for valid usage. Pipelines that previously relied on `--timeout 0`
or path-traversal commands will now exit with code 2 instead of 5/1.

### JSON Schema Changes
None.

### Rollback

```bash
cargo install duckduckgo-search-cli --version 0.6.0 --force
```


## Migration v0.5.x → v0.6.0

### What Changes
- Browser fingerprint profiles added (4 profiles)
- Anti-bot evasion layer (per-profile User-Agent + Sec-CH-UA + Sec-Fetch headers)
- New `--browser-profile` flag
- New `--no-browser-fingerprint` flag to disable
- New `.metadados.user_agent` field in JSON

### Step-by-Step Migration

```bash
# Update to v0.6.0
cargo install duckduckgo-search-cli --version 0.6.0 --force
```

### JSON Schema Changes

New field: `.metadados.user_agent` (string). Always present from v0.6.0 onwards.

### Rollback

```bash
cargo install duckduckgo-search-cli --version 0.5.0 --force
```
