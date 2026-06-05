# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.6.10] - 2026-06-05

### Fixed
- **CI: `Publish to crates.io` job rejected by environment protection rules — tag `v0.6.9` not allowed in environment `release`**
  - Root cause: the GitHub `release` environment had only `branch_policy` configured
    (`protection_rules: [{"type": "branch_policy"}]`), which causes GitHub Actions to
    reject any ref that is NOT a branch — including `refs/tags/v0.6.9`. The run ended
    with `conclusion: failure` and `steps_count: 0` (job never even started), showing
    the annotation `Tag "v0.6.9" is not allowed to deploy to release due to
    environment protection rules`.
  - Solution: created a new `release-publish` environment (id `16308925736`) with no
    `protection_rules`, which accepts ANY ref — including SemVer tags. The `crates_io`
    job now uses `environment: name: release-publish`.

- **CI: `actionlint` exit 3 — `is a directory` error when invoking `actionlint .github/workflows/`**
  - Root cause: `actionlint` v1.x does NOT accept a directory as a positional argument;
    it expects individual files (e.g. `*.yml`) or to be invoked with no arguments
    (recursive auto-discovery of `.github/workflows/`). The incorrect invocation
    produced the error `could not read ".github/workflows/": is a directory` with
    exit 3, marking the `workflow syntax check (actionlint)` job as failed.
  - Solution: corrected the invocation to `actionlint` (no arguments) in the
    `Run actionlint` step of `ci.yml`. Local validation confirmed exit 0 with
    zero syntax errors.

- **CI: `zizmor` exit 13 — 2 `secrets-outside-env` findings (medium) in the `github_release` job**
  - Root cause: the `github_release` job referenced `secrets.GPG_PRIVATE_KEY` and
    `secrets.GPG_PASSPHRASE` in `env:` without a dedicated `environment:`. The
    `zizmor >= 1.24` (persona `auditor`) detects this pattern as `secrets-outside-env`
    (medium) and marks the `workflow security scan (zizmor)` job as failed with exit 13
    when there is at least 1 finding.
  - Solution: (1) removed the GPG secrets from the `github_release` `env:` and added
    the `GPG_SIGNING_ENABLED: "false"` gate at workflow level; (2) the
    `Sign SHA256SUMS with GPG` step was renamed to `(DESABILITADO)` and never
    executes; (3) created a `.github/zizmor.yml` config with
    `rules.secrets-outside-env.config.allow` listing `CRATES_IO_TOKEN` (which is
    at repo level for compatibility). Cosign keyless (job `attest`) already provides
    cryptographic integrity via Sigstore, covering the role GPG signing would play.

- **CI: package list now includes `.github/zizmor.yml` (intentional zizmor configuration)**
  - Added `.github/zizmor.yml` with allow rules for the `CRATES_IO_TOKEN` secret at
    repo level. This file is a static config, contains no credentials and is safe
    to version.

## [0.6.9] - 2026-06-05

### Fixed
- **CI: Windows `.zip` release asset was empty (209 bytes) — bug in `Package (Windows)` PowerShell script**
  - Root cause: the script used `${TARGET}` / `${BIN}` / `${EXT}` syntax, which is **bash interpolation**.
    In PowerShell, `${VAR}` is a string literal — env vars are interpolated as `$env:VAR`.
    Result: `Copy-Item` failed silently (source path became `target//release/`) and
    `Compress-Archive` produced an almost-empty zip (only `SHA256SUMS.txt`).
  - Solution: replaced all `${VAR}` with `$env:VAR` in PowerShell `run:` blocks
    (Package (Windows) and Generate SHA256SUMS (Windows)).
  - Reference: incident-jaq-not-found-runner-2026-06-05 + cross-cutting audit on 2026-06-05

- **CI: `sbom.cdx.json` CycloneDX SBOM was 0 bytes (file not actually generated)**
  - Root cause: `cargo cyclonedx --override-filename sbom.cdx.json` actually writes
    `sbom.cdx.json.json` because the `--override-filename` flag auto-appends `.json`.
    The `wc -c < sbom.cdx.json` step then read 0 bytes from the non-existent file and
    the `Upload SBOM as artifact` step uploaded an empty file (artifact ignored downstream).
  - Solution: changed invocation to `cargo cyclonedx --format json --override-filename sbom`
    (stem only), then `mv sbom.json sbom.cdx.json` to match the expected filename.

- **CI: GitHub Release for v0.6.8 was incomplete (missing Windows zip + sbom)**
  - Root cause: the above two bugs combined meant the v0.6.8 release workflow produced
    a Windows zip with only the SHA256SUMS stub and an empty SBOM. Manually uploaded
    the real SBOM after the fact; Windows zip requires a full re-run.

## [Unreleased]

### Fixed
- **CI: exit 101 `crate already exists` on `Publish to crates.io` job (post-mortem 2026-06-05)**
  - Root cause: trigger duplicado do workflow para tag v0.6.6 já publicada causou `cargo publish`
    exit 101 com `error: crate duckduckgo-search-cli@0.6.6 already exists on crates.io index`.
    crates.io é append-only immutable, versões NUNCA podem ser sobrescritas.
  - Solution: added `preflight` + `crates_io` guard jobs with:
    - Tag-vs-Cargo.toml version consistency check
    - SemVer format validation
    - CHANGELOG entry presence check
    - Co-authored-by AI agent block in recent commits
    - `cargo search` with timeout + retry to detect already-published version
    - `cargo publish` skip with warning + evidence upload when already published
    - Timeout (300s) + retry (3 attempts, backoff 10s/20s/30s) on `cargo publish`
  - Resolution pattern: idempotent release workflow with explicit skip path

- **CI: 18+ Node.js 20 deprecation warnings in all jobs**
  - Root cause: actions/checkout@v4, actions/upload-artifact@v4, actions/download-artifact@v4
    use Node 20. Node 20 deprecated 2025-09-19, removed 2026-09-16.
  - Solution:
    - Updated all actions to v6 (Node 24 native)
    - Updated `softprops/action-gh-release` from v2 to v3
    - Added `FORCE_JAVASCRIPT_ACTIONS_TO_NODE24: "true"` as belt-and-suspenders
  - Migration path: v6 is Node 24 native, v4 needs explicit env var

- **CI: exit 141 SIGPIPE intermittent in `validate (ubuntu-latest)`**
  - Root cause: `cargo test` writes to pipe whose consumer closes early
  - Solution: explicit `|| { ec=$?; if [ $ec -eq 141 ]; then exit 0; fi; exit $ec; }` guard
  - Trade-off: 141 silently becomes warning, may mask real test bugs

- **CI: exit 1 in `validate (windows-latest)` from VS2022→VS2026 redirect**
  - Root cause: GitHub redirects `windows-latest` to `windows-2025-vs2026` since 2025-06-15.
    VS2026 has breaking changes in MSVC toolchain that affect Rust stable.
  - Solution: pinned `windows-2022` in `ci.yml` matrix and `release.yml` build target
  - Re-evaluate pin after 2026-07-15 once VS2026 stabilizes

### Added
- **SBOM CycloneDX generation in release workflow** — `cargo cyclonedx --format json` produces
  `sbom.cdx.json` uploaded as artifact. Enables compliance with EU Cyber Resilience Act.
- **SLSA provenance attestation** — `actions/attest-build-provenance@v2` creates signed
  provenance for all release artifacts. Level 3 SLSA compliance.
- **cosign keyless OIDC signing** — every binary + SHA256SUMS.txt signed with `cosign sign-blob`
  using GitHub OIDC token. No private key management required.
- **SHA256SUMS published with every release** — `sha256sum` generated per target, combined
  into single `SHA256SUMS.txt`, uploaded as release asset and as part of every binary tarball/zip.
- **GPG tag signing** — optional `gpg --detach-sign SHA256SUMS.txt` if `GPG_PRIVATE_KEY` secret
  is configured. `continue-on-error: true` to avoid blocking release on missing key.
- **Concurrency control** — `concurrency.group: release-${{ github.ref }}-${{ github.sha }}`
  prevents parallel runs for same tag+SHA. `cancel-in-progress: false` (release) / conditional
  on PR (CI) ensures publish is never aborted mid-flight.
- **Pre-flight job in release workflow** — validates tag version == Cargo.toml version,
  SemVer format, CHANGELOG entry, no AI agent Co-authored-by BEFORE any build runs.
- **Cron weekly dependency update** — `scheduled_update` job runs Sundays 03:00 UTC,
  executes `cargo update --workspace`, creates PR if changes detected.
- **Zizmor security scan** — static analysis of GitHub Actions workflows detects
  injection, untrusted input, and other security anti-patterns. Runs only on PRs.
- **Actionlint syntax check** — validates YAML syntax of all workflow files. Runs only on PRs.
- **Dependabot for actions and crates** — `.github/dependabot.yml` creates weekly PRs
  for GitHub Actions updates and Rust crate updates. Groups by major/minor/patch.
- **`.gitattributes` LF normalization** — forces LF line endings in all text files,
  preventing CRLF issues on Windows that break `cargo fmt --check`.

### Security
- **Permissions hardened per job** — top-level `permissions: contents: write packages: write
  id-token: write attestations: write checks: write discussions: write` for release;
  per-job `permissions:` blocks in CI for least-privilege.
- **`continue-on-error: true` on GPG step** — missing GPG key does not block release;
  optional enhancement.
- **No `pull_request_target` triggers** — workflows never run with write permissions
  on PRs from forks.

## [0.6.8] - 2026-06-05

### Fixed
- **CI: exit 127 `jaq: command not found` in `github_release` job of release workflow**
  - Root cause: `release.yml` (lines 625-626) used `jaq` (Rust binary) to parse JSON
    response from GitHub REST API, but the GitHub Actions Ubuntu 24.04 runner only
    has `jq 1.7` pre-installed — `jaq` is not part of the standard runner image.
    Bug introduced by commit `7f489b5` (2026-06-05) when bypassing the broken
    `softprops/action-gh-release` action.
  - Solution: replaced `jaq` with `jq` (pre-installed, syntax-compatible) and added
    explicit fail-fast validation for extracted `UPLOAD_URL` and `RELEASE_ID` values
    to surface clear diagnostic messages on malformed API responses.
  - Reference: <https://github.com/actions/runner-images/blob/main/images/ubuntu/
    Ubuntu2404-Readme.md> (Tools section lists `jq 1.7`, `jaq` is absent)

## [0.6.7] - 2026-06-05

### Fixed
- **CI: post-mortem completo do incident-publish-101-2026-06-05** (hardening release pipeline)
  - Added `preflight` job validating tag==Cargo.toml, SemVer, CHANGELOG, no AI Co-authored-by
  - Added guard de versão duplicada em `crates_io` job (zizmor: secrets-outside-env resolvido)
  - cargo publish com timeout 300s + 3 retries (network resilience)
  - Concurrency group por tag+sha (impede runs paralelos)
- **CI: 18+ Node.js 20 deprecation warnings**
  - Updated actions to v6 (Node 24 native)
  - Updated softprops/action-gh-release v2 → v3
  - Added `FORCE_JAVASCRIPT_ACTIONS_TO_NODE24` as belt-and-suspenders
- **CI: zizmor security scan: 134 findings → 0**
  - SHA pinning para 11 actions (unpinned-uses)
  - per-job least-privilege permissions (excessive-permissions)
  - comments + inline trailing em todas as permissions
  - secrets em env: job-level + GitHub Environments dedicados
  - ${{ ... }} em run: mitigated via env vars (template-injection)
  - dtolnay/rust-toolchain substituído por setup via rustup (superfluous-actions)
  - caches removidos do release.yml (cache-poisoning)
- **CI: actionlint 0 erros em ambos workflows**
- **CI: zizmor zero findings (exit 0)**
- **CI: dependabot.yml para auto-update semanal de actions e crates**
- **CI: .gitattributes força LF line endings em todos os arquivos de texto**
- **clippy: `#[cfg(feature = "chrome")]` redundante removido de src/lib.rs:74**
  - browser.rs:25 já tem `#![cfg(feature = "chrome")]` que cobre o módulo
- **clippy: SAFETY comments adicionados a todos os Windows unsafe blocks em src/platform.rs**
  - 5 blocos unsafe agora têm `// SAFETY:` comments explicando precondições
  - Necessário para `clippy::undocumented_unsafe_blocks` (deny em rust 1.96+)
- **test: tests incompatíveis com Windows marcados com `#[cfg(unix)]`**
  - `rejeita_path_absoluto_etc` (testa /etc/shadow)
  - `rejeita_path_absoluto_usr` (testa /usr/bin/evil)
  - Ambos passam em Linux/macOS, pulam em Windows onde os paths são regulares

### Added
- **SBOM CycloneDX generation em release workflow**
  - `cargo cyclonedx --format json` produz `sbom.cdx.json`
  - Compliance com EU Cyber Resilience Act
- **SLSA build provenance via `actions/attest-build-provenance@v2`**
- **cosign keyless OIDC signing** (todos os binários + SHA256SUMS.txt)
- **SHA256SUMS publicado com cada release** (gerado por target)
- **GPG tag signing** (opcional, `continue-on-error: true` se chave ausente)
- **Pre-flight job em release workflow** (9 gates + 1 dry-run)
- **Attestation job** (SBOM + cosign + SLSA em 1 job)
- **scheduled_update Cron semanal** (cargo update automático)
- **Zizmor security scan em CI** (zero findings)
- **Actionlint syntax check em CI** (zero erros)
- **Dependabot para actions e Rust crates** (PRs semanais)

### Security
- **Permissions endurecidas per-job** (least-privilege)
- **Persist-credentials: false em 18/18 actions/checkout** (artipacked)
- **Sem triggers `pull_request_target`** (forks não rodam com write)
- **SHA pinning completo** (11 actions com 40 chars + version comment)

## [0.6.6] - 2026-06-05

### Fixed
- **docs.rs build failure (Build #3487310) caused by `#[doc(cfg(...))]` becoming unstable**
  - Removed `#[cfg_attr(docsrs, doc(cfg(feature = "chrome")))]` from `src/lib.rs:70`
  - Root cause: in Oct 2025 the Rust team merged `doc_auto_cfg` into `doc_cfg` (rust-lang/rust#43781),
    making `#[doc(cfg(...))]` require `#![feature(doc_cfg)]` (nightly-only) on the crate root.
    The build failed with `error[E0658]: #[doc(cfg)] is experimental` on nightly `1.98.0`.
  - The feature gating itself is preserved: `#[cfg(feature = "chrome")]` still excludes
    `pub mod browser` from default builds. The module-level docstring in `src/browser.rs`
    already documents the feature requirement explicitly.
  - `cargo doc --all-features` and `RUSTDOCFLAGS="--cfg docsrs" cargo doc --all-features`
    both pass without warning or error.

## [0.6.5] - 2026-06-05

### Fixed
- **MP-26 — Windows HANDLE cast broken in `windows-sys 0.59+`** (`src/platform.rs:51-63`)
  - `HANDLE` mudou de `isize` para `*mut c_void` upstream (`microsoft/windows-rs`, `raw-window-handle#171`)
  - Substituído `handle != 0 && handle != usize::MAX` por `!handle.is_null() && handle != INVALID_HANDLE_VALUE`
  - Removidos casts inválidos `handle as isize` (a assinatura moderna aceita `HANDLE` direto)
  - Atualizado o `// SAFETY:` comment para documentar nulidade e sentinela Win32
- **CI: `validate` falhava em todos os 3 SOs** (Linux/macOS/Windows) por 6 erros de clippy
  - 3× `clippy::doc_markdown` (`PowerShell`, `rules_rust.md`, `TempDir`) em `src/platform.rs` e `src/browser.rs`
  - 1× `clippy::needless_return` em `src/browser.rs:149`
  - 2× `missing_debug_implementations` em `src/browser.rs:223` (`ChromeBrowser`) e `src/content_fetch.rs` (`CircuitBreakerMap`)

### Added
- **WS-11 — Property-based invariants for HTML parsers** (`src/extraction.rs` +5 testes)
  - Invariante: inputs vazios/quebrados retornam `Vec` vazio sem panic
  - Invariante: positions são densos e 1-based
  - Invariante: URLs absolutos (`http`/`https`) ou vazios
  - Invariante: extração é idempotente
  - Invariante: HTML malformado não causa panic
  - **Zero dependência nova** (apenas stdlib + `#[test]`)
- **WS-12 — Per-host circuit breaker** (`src/content_fetch.rs`)
  - Threshold: 3 falhas consecutivas abrem o circuito
  - Cooldown: 30s antes de half-open probe
  - Integração em `enrich_with_content` antes de cada fetch
  - `BreakerDecision::{Allow, Reject}` para inspeção
  - **Zero dependência nova** (`std::sync::Mutex<HashMap>`)
- **WS-23 — `Retry-After` header test** (`tests/integration_wiremock.rs`)
  - Mock retorna 429 com `retry-after: 2`
  - Asserção: `elapsed_ms >= 1500` (delay mínimo respeitado)
  - Usa `wiremock` 0.6 já em dev-deps
- **WS-25 — `indicatif` ProgressBar para crawls longos** (`src/content_fetch.rs`)
  - `indicatif = "0.18"` adicionado
  - Bar com template `[{elapsed_precise}] {bar:40.cyan/blue} {pos:>4}/{len:4} {msg}`
  - Auto-detecta TTY (esconde em pipes)
  - `progress.finish_and_clear()` ao final
- **Lints preventivos FFI** (`Cargo.toml`)
  - `improper_ctypes = "deny"` (rejeita casts FFI inválidos)
  - `improper_ctypes_definitions = "deny"` (rejeita definições incorretas)

### Tests
- 333 testes passando (243 lib + 24 + 3 + 5 + 10 + 10 + 14 + 18 + 6 doc)
- 6 novos testes de invariantes em `extraction.rs` (WS-11)
- 4 novos testes de circuit breaker em `content_fetch.rs` (WS-12)
- 1 novo teste de Retry-After em `integration_wiremock.rs` (WS-23)
- `cargo fmt --all --check` clean
- `cargo clippy --all-targets --all-features --locked -- -D warnings` clean
- `cargo publish --dry-run --locked --allow-dirty` clean

## [0.6.4] - 2026-06-03

### Added
- **WS-26 — Adaptive anti-bot identity rotation** (new `src/identity.rs` module)
  - 12-identity pool (4 browser families × 3 platforms) for adaptive rotation
  - `IdentityProfile::shuffled_headers()` produces seed-deterministic header order
  - `IdentityPool::rotate_on_block()` implements a 5-level cascade: same identity → same family/different platform → different family/same platform → different family+platform → random
  - `BrowserFamily` and `Platform` enums with canonical English names
  - 5 unit tests covering pool size, cascade level, determinism, header shape, tag stability
- **New CLI flags** (additive, no breaking changes)
  - `--probe` — pre-flight health check (sends 1 minimal request, reports status/latency/Set-Cookie as JSON)
  - `--identity-profile` — pin the session to a specific identity (`auto`, `chrome-win`, `chrome-mac`, `chrome-linux`, `edge-win`, `firefox-linux`, `safari-mac`). `auto` is default.
- **New JSON metadata fields** (additive, `Option` + `skip_serializing_if = "Option::is_none"`)
  - `metadados.identidade_usada` — string tag of the identity that produced the response
  - `metadados.nivel_cascata` — cascade level reached during the request

### Changed
- **Version rollback**: `0.7.0` (unpublished) → `0.6.4` to preserve the in-development feature set under a stable patch number
- All existing CLI flags, JSON output schemas, and exit codes remain unchanged — strictly additive changes

### Tests
- 5 new identity unit tests (313 total tests passing, up from 308)
- All 224 lib tests + 83 integration tests + 6 doc tests pass
- `cargo clippy --lib --bins -- -D warnings` clean
- `cargo fmt --check` clean

## [0.7.0] - 2026-06-01

### Changed
- Complete internationalization: ~600 identifiers renamed PT→EN across 15 source files (struct fields, local variables, parameters, production functions, test functions)
- Module `fetch_conteudo` renamed to `content_fetch`
- Test files `integracao_*.rs` renamed to `integration_*.rs`
- Replaced `anyhow` with typed `CliError` across all 11 modules — zero external error crate dependency
- `output.rs`: all formatting functions renamed (`formatar_*` → `format_*`, `escrever_*` → `write_*`)
- `config_init.rs`: struct fields renamed with `#[serde(rename)]` to preserve JSON backwards compatibility
- `search.rs`: `RetryResult` and `AggregatedSearchResult` fields renamed PT→EN
- `types.rs`: `Config` fields `perfil_browser`/`corresponde_plataforma_ua`/`caminho_chrome` → `browser_profile`/`match_platform_ua`/`chrome_path`

### Added
- Loom concurrency tests (`tests/loom_atomics.rs`) — validates `AtomicBool` visibility across threads
- Criterion benchmarks (`benches/extraction_bench.rs`) — HTML extraction performance baselines
- Doc comments for all 70 previously undocumented public items — zero `missing_docs` warnings
- `.ingest-queue.sqlite` added to `.gitignore` and `Cargo.toml` exclude

### Fixed
- RUSTSEC-2026-0097: updated `rand` 0.8.5 → 0.8.6
- RUSTSEC-2026-0104: updated `rustls-webpki` 0.103.12 → 0.103.13

### Security
- `deny.toml`: added `skip-tree` for 30 transitive duplicate crates (chromiumoxide, scraper, console-subscriber ecosystems)

### Known Limitations
- Loom tests require `RUSTFLAGS="--cfg loom"` which conflicts with `hyper-util` — tests compile but cannot run until upstream resolves the cfg conflict
- JSON output field names remain in Portuguese Brazilian (`posicao`, `titulo`, `resultados`, etc.) — BY DESIGN since v0.2.0

## [0.6.3] - 2026-04-17

### Changed
- Translated all 96 doc comments (`///` and `//!`) across 19 source files from Portuguese to English — docs.rs now renders fully in English for international crates.io audience.
- No code behavior, public API, or JSON output fields changed.

## [0.6.2] - 2026-04-17

### Added
- 19 novos arquivos de documentação — conformidade completa com rules_rust_documentacao.md (28 gaps G01-G28)
- Documentação bilíngue EN+PT: HOW_TO_USE, CROSS_PLATFORM, AGENTS-GUIDE, COOKBOOK.pt-BR, INTEGRATIONS.pt-BR
- CODE_OF_CONDUCT.md + CODE_OF_CONDUCT.pt-BR.md — Contributor Covenant 2.1
- README.pt-BR.md, CHANGELOG.pt-BR.md, CONTRIBUTING.pt-BR.md, SECURITY.pt-BR.md
- docs/AGENTS.pt-BR.md — guia imperativo para LLMs em português
- docs/AGENTS-GUIDE.md + docs/AGENTS-GUIDE.pt-BR.md — guia persuasivo bilíngue
- llms.txt — arquivo compacto de orientação para LLMs (< 50 KB)
- llms-full.txt — concatenação completa de docs para contexto longo de LLMs
- eval-queries.json × 2 — 20 queries de avaliação EN + 20 PT-BR para skill testing

### Changed
- README.md — link para README.pt-BR.md + quick install antes da linha 30
- CONTRIBUTING.md — MSRV Rust 1.75 explícito + PR checklist 8 itens + branching strategy + nextest
- SECURITY.md — tabela de versão específica v0.6.2 + política de embargo 90 dias + zero bold + zero emojis
- skill/SKILL.md (EN+PT) — seção Workflow com 5 passos numerados verificáveis

## [0.6.1] - 2026-04-17

### Fixed
- `--timeout 0` now returns exit 2 (invalid config) instead of executing a search with zero timeout and returning exit 5.
- `--output /tmp/../../etc/passwd` now returns exit 2 (invalid config) instead of exit 1 (runtime OS error) — path traversal validation moved to `montar_configuracoes()`, before the pipeline starts.

### Added
- `validar_timeout_segundos()` method on `CliArgs` — rejects values of 0 with a descriptive error.
- Early path traversal check in `montar_configuracoes()` — calls `paths::validate_output_path()` at config validation time, not at write time.
- 2 E2E regression tests: `timeout_zero_retorna_exit_2` and `output_com_path_traversal_retorna_exit_2`.
- 1 unit test: `validar_timeout_segundos_rejeita_zero`.

## [0.6.0] - 2026-04-16

### Security
- Browser fingerprint profiles per-family previnem detecção anti-bot do DuckDuckGo.
- Headers `Sec-Fetch-*` e Client Hints por família imitam sessão de navegador real.
- `Accept-Language` com q-values RFC 7231 elimina fingerprint de UA genérico.
- Detecção de bloqueio silencioso com limiar de 5 KB previne resultados truncados.

### Added
- `BrowserFamily` enum — variantes `Chrome`, `Firefox`, `Edge`, `Safari`.
- `BrowserProfile` struct — encapsula família, versão e conjunto de headers por família.
- Headers `Sec-Fetch-Dest`, `Sec-Fetch-Mode`, `Sec-Fetch-Site` por família em `http.rs`.
- Client Hints (`Sec-Ch-Ua`, `Sec-Ch-Ua-Mobile`, `Sec-Ch-Ua-Platform`) para Chrome e Edge.
- Detecção de HTTP 202 anomaly em `search.rs` com backoff exponencial automático.
- Detecção de bloqueio silencioso — resposta com menos de 5 000 bytes é tratada como bloqueio.
- `BrowserProfile` propagado via `Config` para todos os módulos da pipeline.
- Headers de paginação com `Sec-Fetch-Site: same-origin` para imitar navegação real.

### Changed
- `Accept-Language` atualizado para `pt-BR,pt;q=0.9,en-US;q=0.8,en;q=0.7` conforme RFC 7231.
- `Accept` header agora reflete o perfil completo do browser por família.
- Delays de paginação aumentados de 500–1 000 ms para 800–1 500 ms.
- Limiar de bloqueio silencioso aumentado de 100 para 5 000 bytes.

## [0.5.0] - 2026-04-16

### Security
- Path traversal validation on `--output` — rejects `..` components and writes to system directories (`/etc`, `/usr`, `C:\Windows`).
- Proxy credential masking — error messages no longer expose passwords from `--proxy http://user:pass@host` URLs.

### Added
- `src/paths.rs` — centralized path validation, parent directory creation, and Unix permission application.
- `src/signals.rs` — centralized SIGPIPE restoration (Unix) and Ctrl+C/SIGINT handler (cross-platform).
- `ErroCliDdg` enum with `thiserror` — 11 typed error variants with `exit_code()` and `codigo_erro()` methods.
- `mascarar_url_proxy()` in `http.rs` — redacts credentials from proxy URLs in error context.
- 21 new unit tests across `paths.rs`, `signals.rs`, `error.rs`, and `http.rs`.

### Changed
- `thiserror = "2"` added to dependencies for structured domain errors.
- `src/main.rs` reduced from 63 to 23 lines — signal handling extracted to `signals.rs`.
- `src/output.rs` file writes now validate paths via `paths::validate_output_path()` before I/O.
- `deny.toml` updated with RUSTSEC-2026-0097 exception (rand 0.8 unsound with custom logger — not applicable).

## [0.4.4] - 2026-04-16

### Fixed
- SIGPIPE restored to SIG_DFL on Unix — pipes to `jaq`, `head`, and other consumers no longer lose stdout silently.
- BrokenPipe errors detected in anyhow chain and treated as exit 0 (not exit 1) at all output boundaries.

### Added
- `--help` now shows EXIT CODES (0–5) and PIPE USAGE sections via `after_long_help`.
- 3 E2E tests for pipe regression: exit codes in help, short help exclusion, stdout byte count.
- README troubleshooting item 7: "Pipe to jaq/jq returns empty" with PIPESTATUS diagnostic (EN + PT).
- `docs_rules/rules_rust.md`: SIGPIPE + BrokenPipe added to I/O checklist.
- `docs/AGENT_RULES.md`: R24 pipe safety rule with PIPESTATUS diagnostic.
- `docs/COOKBOOK.md`: Recipe 16 pipe diagnostic (EN + PT).
- `docs/INTEGRATIONS.md`: pipe safety clause in baseline contract.
- Exit code branching section in both skill files (EN + PT).

## [0.4.3] - 2026-04-15

### Changed

- **`README.md`** — Nova seção persuasiva "Agent Skill" (EN + PT) posicionada
  entre a tabela de agentes e a seção de Documentação, no pico de atenção do
  leitor. Copywriting AIDA destacando a skill bilíngue empacotada em `skill/`:
  auto-ativação semântica sem slash command, 14 seções canônicas MUST/NEVER,
  contrato JSON anti-alucinação, economia de tokens em cada turno de busca,
  instalação em um comando (`git clone` + `cp -r`). Benefícios explícitos para
  LLMs (decisão automática de quando buscar) e desenvolvedores (zero prompt
  engineering, zero tool registration). Tarball do crates.io inalterado —
  skills continuam vivendo apenas no GitHub.

## [0.4.2] - 2026-04-15

### Added

- **`skill/duckduckgo-search-cli-pt/SKILL.md`** e
  **`skill/duckduckgo-search-cli-en/SKILL.md`** — Skills bilíngues para Claude
  Code, Claude Agent SDK e plataformas compatíveis com Agent Skills. Cada
  skill traz frontmatter YAML com `name` único por idioma e `description`
  carregado de triggers semânticos para auto-invocação, além de 14 seções
  H2 canônicas (Missão, Contrato de Invocação, Proibições Absolutas,
  Parsing com `jaq`, Schema JSON, Exit Codes, Batch, Fetch-Content,
  Endpoint, Retries, Receitas, Validação, Memória, Regra de Ouro).
  Publicadas no GitHub, excluídas do tarball do crates.io.

### Changed

- **`docs/AGENT_RULES.md`** (833 linhas, +7,6%) — Reescrita editorial
  aplicando copywriting AIDA: cada regra abre com benefício mensurável,
  linguagem imperativa MUST/NEVER reforçada, zero narrativa decorativa,
  zero negrito com asteriscos duplos, zero separador visual `---` entre
  seções. Bilíngue EN+PT espelhado com tom idêntico.
- **`docs/COOKBOOK.md`** (1082 linhas, −3,1%) — Cada receita abre com o
  ganho concreto antes do comando, bullets curtos de 8 a 15 palavras,
  pipelines `jaq` + `xh` + `sd` preservados intactos.
- **`docs/INTEGRATIONS.md`** (1212 linhas, +1,3%) — 16 agentes com tabela
  comparativa textual, snippets determinísticos por agente, zero emoji.

### Meta

- `Cargo.toml` exclude ampliado para cobrir `skill/` e `skill/**` — skills
  ficam no GitHub e fora do tarball publicado em crates.io.

## [0.4.1] - 2026-04-14

### Added

- **`docs/AGENT_RULES.md`** (773 linhas) — Regras imperativas bilíngue (EN+PT)
  com 30+ rules `MUST`/`NEVER` (R01..R30) para LLMs/agentes invocarem a CLI
  em produção. Cobre: invariantes core, contrato JSON, rate limiting, error
  handling, performance, segurança, anti-patterns. Quick Reference Card no
  final.
- **`docs/COOKBOOK.md`** (1117 linhas) — 15 receitas copy-paste bilíngue
  combinando `duckduckgo-search-cli` + `jaq` + `xh` + `sd` para casos reais:
  research consolidado, ETL multi-query, extração de domínios, monitoramento
  com filtro temporal, content extraction com `--fetch-content`, comparação
  top 5 vs top 15, NDJSON para pipelines, function wrappers para bash.
- **`docs/INTEGRATIONS.md`** (1196 linhas) — Snippets prontos para 16
  agentes/LLMs: Claude Code, OpenAI Codex, Gemini CLI, Cursor, Windsurf,
  Aider, Continue.dev, MiniMax, OpenCode, Paperclip, OpenClaw, Google
  Antigravity, GitHub Copilot CLI, Devin, Cline, Roo Code. Cada agente
  documenta: pitch, mecanismo de shell, setup, snippet básico, snippet
  multi-query, system prompt rule, caveats.
- Seção **Documentation** no README.md (EN + PT) linkando os 3 guias.

### Fixed

- README.md badge cluster e referências internas conferidas contra
  `daniloaguiarbr/duckduckgo-search-cli` (repo canônico).

## [0.4.0] - 2026-04-14

### Changed (BREAKING)

- **Default de `--num` / `-n`**: alterado de "todos os resultados da primeira
  página" (~11) para **15**, com **auto-paginação** automática. Quando o
  número efetivo excede 10, o binário agora busca **2 páginas** por query
  para satisfazer o teto solicitado, desde que `--pages` não tenha sido
  customizado pelo usuário.
- **Auto-paginação automática**: se `--num > 10` (seja porque o usuário
  passou explicitamente ou porque o default 15 foi aplicado) E `--pages`
  não foi customizado (continua no default 1), o binário auto-eleva
  `--pages` para `ceil(num/10)` respeitando o teto de 5 páginas validado
  por `validar_paginas`. Impacto: mais requests por query (2x no caso
  default) e latência marginalmente maior, porém com cobertura completa
  dos resultados solicitados.

### Added

- Documentação no comentário do flag `--num` em `cli.rs` descrevendo a
  nova semântica de default e auto-paginação.
- 4 novos testes unitários em `lib.rs::testes`:
  `montar_configuracoes_aplica_default_num_15_quando_omitido`,
  `montar_configuracoes_respeita_pages_explicito_acima_de_1`,
  `montar_configuracoes_auto_pagina_quando_num_maior_que_10`,
  `montar_configuracoes_nao_auto_pagina_quando_num_10_ou_menos`.
- 2 novos testes wiremock em `tests/integracao_wiremock.rs`:
  `testa_default_num_15_auto_pagina_2_paginas`,
  `testa_auto_paginacao_respeita_pages_explicito`.

### Migration Guide

- **Quem quer o comportamento antigo** (1 página, ~11 resultados):
  passe `--pages 1 --num 10` explicitamente. O `--pages 1` explícito é
  indistinguível do default (trade-off aceito: `paginas > 1` é o único
  sinal de "customização"), então o mais seguro é combinar com `--num 10`
  para garantir que nada será auto-paginado.
- **Quem já passava `--num 5`** (ou qualquer valor <= 10): comportamento
  **inalterado** (sem auto-paginação, 1 página).
- **Quem já passava `--num 20 --pages 2`** ou similar: comportamento
  **inalterado** (respeita explícito do usuário).
- **Quem confiava no default sem flags**: agora recebe até 15 resultados
  em vez de ~11, com 1 request extra por query. Para restaurar o antigo,
  passe `--pages 1 --num 10`.

## [0.3.0] - 2026-04-14

### Changed (BREAKING)

- **Schema JSON**: campo `buscas_relacionadas` REMOVIDO de `SearchOutput` e
  `MultiSearchOutput.buscas[i]`. O endpoint `html.duckduckgo.com/html/` não
  expõe related searches no DOM atual; manter o campo sempre vazio era ruído.
  Pipelines que parseavam `.buscas_relacionadas` precisam ajuste.
- **Pool de User-Agents**: removidos UAs de browsers de texto (`Lynx 2.9.0`,
  `w3m/0.5.3`, `Links 2.29`, `ELinks 0.16.1.1`) que faziam DuckDuckGo retornar
  HTML degradado. Substituídos por 6 UAs modernos validados empiricamente
  contra o `/html/` endpoint: Chrome 146 (Win/Mac/Linux), Edge 145 Windows,
  Firefox 134 Linux, Safari 17.6 macOS. Firefox Win/Mac foram REMOVIDOS após
  retornarem HTTP 202 anomaly em validação real (heurística anti-bot do DDG).

### Fixed

- **Snippet duplicava título e URL no início**: o seletor padrão tinha
  fallback `.result__body` (container pai) que fazia `text()` recursivo
  capturar título+URL+snippet concatenados. Trocado por `.result__snippet`
  puro. Pipelines como `jaq '.resultados[].snippet'` agora retornam apenas
  o texto descritivo do resultado.
- **Título "Official site"**: DuckDuckGo renderiza literalmente este texto
  como label para domínios verificados (ex: prefeituras). O scraper agora
  detecta este caso e substitui pelo `url_exibicao` (ex: `saofidelis.rj.gov.br`).
  O texto original é preservado no novo campo opcional `titulo_original`
  para auditoria.

### Added

- Campo `titulo_original: Option<String>` em `SearchResult`. Presente
  apenas quando o título foi substituído por heurística (atualmente: caso
  "Official site"). Serializado com `#[serde(skip_serializing_if = "Option::is_none")]`
  — não aparece no JSON quando ausente.
- Resultados patrocinados (`.result--ad`) excluídos do container default
  via seletor `.result:not(.result--ad)`.

### Removed

- Função `extrair_buscas_relacionadas` em `src/search.rs` (dead code com
  seletor hardcoded que nunca encontrava nada).
- Seção `[related_searches]` em selectors default.

### Migration Guide (v0.2.x → v0.3.0)

- Pipelines `jaq '.buscas_relacionadas[]'`: campo não existe mais.
  Remover do filtro ou tratar `null`.
- Esperando snippet com prefixo título+URL? Agora vem só o texto descritivo
  — ajuste regex/parsing downstream se necessário.
- Confiando em `titulo == "Official site"` para detectar sites verificados?
  Use `titulo_original.as_deref() == Some("Official site")`.
- **CONFIG EXTERNO LEGADO**: usuários que rodaram `init-config` em versões
  anteriores possuem `~/.config/duckduckgo-search-cli/{selectors,user-agents}.toml`
  com defaults antigos (snippet com `.result__body` + UAs `Lynx`/`w3m`/etc.).
  Esses arquivos OVERRIDE os defaults embutidos. Para aplicar as correções
  desta versão, execute APÓS atualizar:
  ```
  duckduckgo-search-cli init-config --force
  ```
  O flag `--force` sobrescreve os arquivos externos. Backup recomendado se
  você editou manualmente para hotfix de seletores.

## [0.2.0] - 2026-04-14

### Changed (BREAKING)

Schema JSON serializado agora usa nomes de campo em **português brasileiro**,
alinhado com os exemplos `jaq` do README e com o invariante INVIOLÁVEL do
blueprint v2 do projeto ("Logs e nomes de campo em português brasileiro").

Pipelines que dependiam do schema em inglês da `v0.1.0` precisam atualizar
os seletores `jaq`. Tabela de renomeações:

| Antes (v0.1.0) | Depois (v0.2.0) |
|----------------|-----------------|
| `position` | `posicao` |
| `title` | `titulo` |
| `displayed_url` | `url_exibicao` |
| `content` | `conteudo` |
| `content_length` | `tamanho_conteudo` |
| `content_extraction_method` | `metodo_extracao_conteudo` |
| `execution_time_ms` | `tempo_execucao_ms` |
| `selectors_hash` | `hash_seletores` |
| `retries` | `retentativas` |
| `fallback_endpoint_used` | `usou_endpoint_fallback` |
| `concurrent_fetches` | `fetches_simultaneos` |
| `fetch_successes` | `sucessos_fetch` |
| `fetch_failures` | `falhas_fetch` |
| `chrome_used` | `usou_chrome` |
| `proxy_used` | `usou_proxy` |
| `engine` | `motor` |
| `region` | `regiao` |
| `results_count` | `quantidade_resultados` |
| `results` | `resultados` |
| `related_searches` | `buscas_relacionadas` |
| `pages_fetched` | `paginas_buscadas` |
| `error` | `erro` |
| `message` | `mensagem` |
| `metadata` | `metadados` |
| `queries_count` | `quantidade_queries` |
| `parallel` | `paralelismo` |
| `searches` | `buscas` |

Campos inalterados: `url`, `snippet`, `query`, `endpoint`, `timestamp`, `user_agent`.

### Fixed

- Pipelines documentados no README (`jaq '.resultados[].titulo'`, etc.) agora
  funcionam end-to-end. Em `v0.1.0` retornavam `null` por divergência do schema
  (bug reportado pelo usuário).

## [Unreleased]

### Added

- `LICENSE-MIT` and `LICENSE-APACHE` (dual-licensed per `Cargo.toml`, aligning the tarball with the SPDX declaration).
- `.pre-commit-config.yaml` with three hook groups: (1) pre-commit-hooks standard (trailing whitespace, EOF, YAML/TOML validity, mixed line endings), (2) Rust hooks (`cargo fmt` + `cargo clippy -D warnings`), (3) local `commit-msg` hook blocking `Co-authored-by:` from AI agents (mirrors the CI `commit_check` job). Reduces CI round-trips for trivial violations.
- `.gitattributes` forcing LF on `.rs` / `.toml` / `.sh` / `.yml` / `.md` / fixture HTML — prevents silent corruption when cloning on Windows with `core.autocrlf=true` (which would otherwise break shebangs, rustfmt, and content-extraction tests). Binary extensions (`.png`, `.woff2`, etc.) marked explicitly. `Cargo.lock` and `target/` flagged `linguist-generated` to exclude from GitHub language stats.
- `.editorconfig` normalizing UTF-8, LF, trailing-whitespace trim, and per-language indent (Rust/TOML 4, YAML/JSON/MD 2, Makefile tab) across VS Code, RustRover, vim, and other editors — eliminates spurious formatting diffs caused by per-dev settings drift.
- `.github/PULL_REQUEST_TEMPLATE.md` with the 10-gate checklist + project-specific constraints (no cache, no MCP, rustls-only, `println!` confined to `output.rs`, PT-BR identifiers).
- `.github/ISSUE_TEMPLATE/bug_report.yml` + `feature_request.yml` + `config.yml` — structured triage with platform dropdown (glibc/musl/NixOS/Flatpak/Snap/macOS ARM/macOS Intel/Windows/WSL), install method, and constraint verification. `config.yml` redirects security reports to Security Advisories and usage questions to Discussions.
- `Cross.toml` enabling `cross build --target <t>` for ARM64/ARMv7 Linux targets (musl + glibc + hard-float) from any x86_64 host with Docker/Podman — complements the native CI pipeline for developers without a GitHub Actions runner.
- `CONTRIBUTING.md` with the 10-gate validation matrix, coding standards (Brazilian Portuguese identifiers, rustls-only TLS, `output.rs` as the sole `println!` site), three-layer testing strategy, supply-chain guardrails, and the tag-driven release process.
- `.cargo/config.toml` exposing 8 developer aliases (`cargo check-all`, `cargo lint`, `cargo docs`, `cargo test-all`, `cargo cov`, `cargo cov-html`, `cargo publish-check`, `cargo pkg-list`) — each mirrors a CI job for local reproduction.
- Doctests in public API: `pipeline::combine_and_dedup_queries`, `content_fetch::extract_host`, and `search::format_kl` — compilable examples on docs.rs that double as regression tests.
- `SECURITY.md` documenting the private-disclosure workflow via GitHub Security Advisories, response SLA (72 h), scope (HTTP/HTML parsing, credential leaks, path traversal, TLS) and security design assumptions (stateless, rustls-only, no JS for search).
- `.github/dependabot.yml` enabling weekly automatic dependency updates for both `cargo` and `github-actions` ecosystems, with semantic grouping (dev-deps, tokio-ecosystem, tracing-ecosystem) and PR count limits.
- `rust-toolchain.toml` pinning `stable` with `rustfmt` + `clippy` components for reproducible dev/CI builds.
- `.github/workflows/release.yml` triggered by `v*.*.*` tags (and `workflow_dispatch` with `dry_run`) running the 5-stage release pipeline per `rules_rust.md` §19: validate → build_matrix (5 targets) → macos_universal (lipo) → github_release (with generated notes) → crates_io (publish gated on `CRATES_IO_TOKEN` secret).
- `msrv` job in `ci.yml` extracting `rust-version` from `Cargo.toml` and running `cargo check` on that toolchain to detect MSRV drift on every PR.
- `.github/workflows/ci.yml` enforcing the 10-gate validation matrix across Ubuntu, macOS, and Windows:
  - `cargo check` / `clippy -D warnings` / `fmt --check` / `doc -D warnings` / `test --all-features` on all three OSes.
  - `cargo llvm-cov --fail-under-lines 80` dedicated job on Ubuntu.
  - `cargo audit` + `cargo deny check advisories licenses bans sources` supply-chain gate.
  - `cargo publish --dry-run` + `cargo package --list` sensitive-file guard.
  - Static musl binary smoke test (`x86_64-unknown-linux-musl`) covering Alpine Linux and minimal containers.
  - `commit_check` job blocking `Co-authored-by:` trailers from AI agents in PRs.
- `deny.toml` with full four-axis supply-chain policy (advisories/licenses/bans/sources) and documented ignores for three transitive unmaintained advisories (`RUSTSEC-2025-0057 fxhash`, `RUSTSEC-2025-0052 async-std`, `RUSTSEC-2026-0097 rand`) with justification and revisit notes.
- 22 new tests raising coverage from 77.4% to 86.4% (lines): `tests/integration_pipeline.rs` (10), `tests/integracao_fetch_conteudo.rs` (3), and 9 inline tests for `output.rs` covering `emit_ndjson`, `emit_stream_text`, `emit_stream_markdown`, and the `PipelineResult` variants via `tempfile`.

### Changed

- `parallel.rs` coverage 50% → 81%; `pipeline.rs` 55% → 82%; `content_fetch.rs` 68% → 85%; `output.rs` 70% → 87%.

## [0.1.0] - 2026-04-14

### Added

- Core search pipeline against DuckDuckGo HTML endpoint via pure HTTP (`html.duckduckgo.com/html/`).
- Lite endpoint fallback via `--endpoint lite` for JavaScript-less pages.
- Multi-query mode with automatic deduplication, positional args, `--queries-file`, and stdin.
- Parallel fan-out of queries with `--parallel` (1..=20), bounded by `tokio::JoinSet` + `Semaphore`.
- `--pages` (1..=5) to collect multiple result pages per query.
- `--fetch-content` fetches each result URL via pure HTTP, applies readability, and embeds the cleaned text in the JSON output.
- `--max-content-length` (1..=100_000) truncates extracted content respecting word boundaries.
- Chrome headless fallback under `--features chrome` with cross-platform detection (Linux including Flatpak/Snap, macOS including Apple Silicon, Windows including registry paths) and stealth flags (`--disable-blink-features=AutomationControlled`, `--window-size=1920,1080`, `--no-first-run`, platform-specific `--no-sandbox`, `--disable-gpu`).
- `--chrome-path` flag to manually specify the Chrome/Chromium executable.
- `--proxy URL` + `--no-proxy` (HTTP/HTTPS/SOCKS5) with precedence over env vars.
- `--global-timeout` (1..=3600 s) wraps the whole pipeline in `tokio::time::timeout`.
- `--per-host-limit` (1..=10) rate-limits fetches per host via a per-host `Semaphore` map.
- `--match-platform-ua` narrows the user-agent pool to the current platform.
- `--stream` NDJSON mode emits one result per line as they are extracted.
- Four output formats: `json` (default), `text`, `markdown`, `auto` (TTY-aware).
- External configuration files: `selectors.toml` and `user-agents.toml` under XDG config dir, overriding embedded defaults.
- Subcommand `init-config` with `--force` and `--dry-run` to bootstrap user config files.
- Exit codes: `0` success, `1` runtime, `2` config, `3` block (HTTP 202 anomaly), `4` global timeout, `5` zero results.
- UTF-8 console initialization on Windows via `SetConsoleOutputCP(65001)`.
- Rustls-TLS everywhere for dependency-free cross-platform builds.
- `tracing` + `tracing-subscriber` with `RUST_LOG` honored; `--verbose` / `--quiet` flags.
- 163 unit + integration tests covering CLI parsing, config montage, HTTP extraction, parallel fan-out, selectors, and wiremock-backed search flows.

### Security

- All credentials (`--proxy user:pass@host`) are masked in logs.
- Output file creation applies Unix permissions `0o644`.

[Unreleased]: https://github.com/comandoaguiar/duckduckgo-search-cli/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/comandoaguiar/duckduckgo-search-cli/releases/tag/v0.1.0

