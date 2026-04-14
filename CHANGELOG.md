# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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

- **Schema JSON**: campo `buscas_relacionadas` REMOVIDO de `SaidaBusca` e
  `SaidaBuscaMultipla.buscas[i]`. O endpoint `html.duckduckgo.com/html/` não
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

- Campo `titulo_original: Option<String>` em `ResultadoBusca`. Presente
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
- Doctests in public API: `pipeline::combinar_e_deduplicar_queries`, `fetch_conteudo::extrair_host`, and `search::formatar_kl` — compilable examples on docs.rs that double as regression tests.
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
- 22 new tests raising coverage from 77.4% to 86.4% (lines): `tests/integracao_pipeline.rs` (10), `tests/integracao_fetch_conteudo.rs` (3), and 9 inline tests for `output.rs` covering `emitir_ndjson`, `emitir_stream_text`, `emitir_stream_markdown`, and the `ResultadoPipeline` variants via `tempfile`.

### Changed

- `parallel.rs` coverage 50% → 81%; `pipeline.rs` 55% → 82%; `fetch_conteudo.rs` 68% → 85%; `output.rs` 70% → 87%.

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
