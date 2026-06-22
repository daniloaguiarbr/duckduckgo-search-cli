# Contributing to duckduckgo-search-cli

Thanks for your interest in contributing to duckduckgo-search-cli.
Every contribution improves a tool used by developers and AI agents worldwide.
Read this in [Português](CONTRIBUTING.pt-BR.md).


## Quick Start
### Setup em Cinco Comandos
- Clone o repositório: `git clone https://github.com/daniloaguiarbr/duckduckgo-search-cli`
- Acesse o diretório: `cd duckduckgo-search-cli`
- Verifique compilação: `cargo check --all-targets`
- Execute clippy: `cargo clippy --all-targets --all-features -- -D warnings`
- Rode os testes: `cargo test --all-features`

Aliases de atalho NÃO existem — use os comandos canônicos acima.


## Development Setup
### Prerequisites
- MSRV (Minimum Supported Rust Version): Rust 1.88 — declarado em `Cargo.toml` (`rust-version`) e travado em `rust-toolchain.toml`
- Execute `rustup update stable` para garantir a versão correta
- Instale llvm-cov com: `cargo install cargo-llvm-cov`
- Instale cargo-audit com: `cargo install cargo-audit`
- Instale cargo-deny com: `cargo install cargo-deny`
- O projeto NÃO usa `cargo-nextest` — a suíte roda via `cargo test` padrão


## Chrome Development Prerequisites (v0.8.0)
- Install Google Chrome or Chromium for E2E tests
- Install `xvfb` on headless Linux: `sudo apt install xvfb`
- Run E2E tests with: `xvfb-run --auto-servernum --server-args="-screen 0 1920x1080x24" cargo test`
- Run tests without Chrome: `cargo test --no-default-features`
- The `chrome` feature is enabled by default in `Cargo.toml`
- Chrome stealth tests are in `tests/integration_chrome_stealth.rs`
- Deep-research Chrome tests are in `tests/integration_deep_research.rs`


## Code of Conduct
### Contrato Social
- Este projeto adota o [Contributor Covenant](CODE_OF_CONDUCT.md)
- Leia integralmente antes de abrir qualquer issue ou pull request
- Reporte violações seguindo o canal descrito em `CODE_OF_CONDUCT.md`


## Branching Strategy
### Fluxo de Branches
- Ramificação principal: `main`
- Branches de feature: `feature/nome-descritivo` a partir de main
- Branches de fix: `fix/nome-do-bug` a partir de main
- Abra PR de volta para main
- Squash and Merge é o método padrão de merge


## Coding Standards
### Convenções Obrigatórias
- Comentários de código, mensagens de log e nomes de campos de structs em português brasileiro conforme `CLAUDE.md`
- Identificadores de API pública podem ser em inglês quando seguem estilo Rust convencional como `from` e `into`
- Nunca use `.unwrap()` ou `.expect()` em código de produção
- Propague erros com `?` e a variante tipada definida em `src/error.rs` (enum `CliError` via `thiserror`)
- O projeto usa `thiserror 2` puro — `anyhow` NÃO está nas dependências
### I/O Centralizado
- O módulo `output.rs` é o ÚNICO lugar permitido para chamar `println!` ou `print!`
- Todos os outros módulos registram via `tracing`
### TLS Obrigatorio
- A stack TLS e `reqwest` + `rustls-tls` desde v0.8.6 (Rust puro, zero deps nativas C)
- v0.7.3-v0.8.5 usava `wreq 6.0.0-rc.29` com BoringSSL — substituido na v0.8.6 (ADR-0008)
- Chrome headed (v0.8.0+) fornece fingerprint TLS real de navegador
- Nao reative `native-tls` — quebra NixOS, Alpine e builds musl estaticos
- `cmake`, `perl`, NASM NAO sao mais necessarios desde v0.8.6
### Restrições de Design
- Sem cache, sem MCP, sem API paga — restrições inegociáveis do blueprint v2


## Testing
### Três Camadas de Teste
- Testes unitários inline com `#[cfg(test)] mod testes` para funções puras
- Testes de integração em `tests/` usando `wiremock` — ZERO HTTP real
- Doctests dentro de blocos `///` em APIs públicas — duplos como exemplos no docs.rs
### Execução de Testes
- Execute testes com `cargo test --all-features` (runner padrão)
- Execute cobertura com `cargo llvm-cov` — mínimo 80% obrigatório
- Qualquer PR que reduza a cobertura abaixo do limite falhará no CI


## 10-Gate Validation Matrix
### Gates Obrigatórios
Every PR must pass all 10 gates (enforced by `.github/workflows/ci.yml`):

| # | Gate | Comando local |
|---|------|---------------|
| 1 | Compilation | `cargo check --all-targets --all-features` |
| 2 | Clippy | `cargo clippy --all-targets --all-features -- -D warnings` |
| 3 | Format | `cargo fmt --all -- --check` |
| 4 | Docs | `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps` |
| 5 | Tests | `cargo test --all-features` |
| 6 | Coverage >= 80% | `cargo llvm-cov --workspace --all-features` |
| 7 | Vuln audit | `cargo audit --deny warnings` |
| 8 | Supply chain | `cargo deny check advisories licenses bans sources` |
| 9 | Publish dry-run | `cargo publish --dry-run --allow-dirty` |
| 10 | Package content | `cargo package --list-files` |


## Pull Request Checklist
### Itens Verificáveis Antes de Abrir PR
- `cargo fmt --all -- --check` retorna ZERO diferenças
- `cargo clippy --all-targets --all-features -- -D warnings` retorna ZERO warnings
- `cargo test --all-features` retorna ZERO falhando
- `cargo doc --no-deps` sem warnings
- `cargo audit --deny warnings` sem vulnerabilidades conhecidas
- CHANGELOG.md e CHANGELOG.pt-BR.md atualizados com a mudança
- Título do PR descreve o problema resolvido em termos do usuário


## Commit Convention
### Prefixos Convencionais
- Use prefixos: `feat:`, `fix:`, `deps:`, `ci:`, `docs:`, `test:`, `refactor:`
- Nunca adicione trailers `Co-authored-by:` de agentes de IA como dependabot, renovate, Claude, GPT, Copilot, Cursor ou Gemini
- Use squash and merge para PRs com múltiplos commits
- CI bloqueia commits com Co-authored-by de agentes


## Supply Chain
### Gestão de Dependências
- Toda nova dependência deve passar em `cargo deny check`
- Se o candidato traz nova licença fora da allowlist ou advisory transitivo, encontre alternativa ou documente o ignore em `deny.toml`
- Documente com linhas `# Why:` e `# How to apply:` no `deny.toml`
- Prefira crates com `trustScore >= 7` no `context7` (veja `CLAUDE.md`)


## Documentação Relacionada
### Links Úteis
- [CHANGELOG.md](CHANGELOG.md) e [CHANGELOG.pt-BR.md](CHANGELOG.pt-BR.md) — histórico bilíngue sincronizado
- [SECURITY.md](SECURITY.md) — política de reporte responsável e versões suportadas
- [INSTALL-WINDOWS.md](INSTALL-WINDOWS.md) — pré-requisitos BoringSSL no Windows (NASM, CMake, MSVC, Perl)
- [INTEGRATIONS.md](INTEGRATIONS.md) — catálogo de integrações com 16+ agentes de IA
- [docs/INTEGRATIONS.md](docs/INTEGRATIONS.md) — guia completo de integração
- [docs/INSTALL-WINDOWS.pt-BR.md](docs/INSTALL-WINDOWS.pt-BR.md) — versão em português
- [docs/decisions/](docs/decisions/) — Architecture Decision Records (ADRs)
- [docs/CROSS_PLATFORM.md](docs/CROSS_PLATFORM.md) — comportamento por plataforma


## Pre-Publish Gate
### Bloqueio Pré-Publicação
- O job `pre-publish` em `.github/workflows/release.yml` exige `CARGO_INSTALL_FLAGS=--locked`
- Compilação do `cargo install` é parte do gate — sem network ímpar no build
- Mantenedores: nunca publique sem essa flag ativa
- Falha do `pre-publish` cancela a release no crates.io


## Workflow com Agent Teams
### Orquestração de Releases
- Releases da v0.7.8+ usaram o fluxo de 8 fases via Agent Teams
- Cada teammate recebe prompt autocontido com Regra Zero, identidade, contexto, ferramentas
- Líder coordena, delega, verifica — não implementa diretamente
- Ver `CLAUDE.md` na raiz para o protocolo completo
- ADRs em `docs/decisions/` documentam decisões tomadas por cada release


## How to Report Bugs
### Template de Bug Report
- Abra uma issue com título descritivo no formato: `[bug] descrição concisa do problema`
- Inclua versão da CLI: `duckduckgo-search-cli --version`
- Inclua sistema operacional e versão do Rust: `rustc --version`
- Inclua comando exato que reproduz o problema
- Inclua saída completa incluindo stderr


## How to Request Features
### Template de Feature Request
- Abra uma issue com título descritivo no formato: `[feature] descrição concisa`
- Descreva o problema que a feature resolveria
- Descreva o comportamento esperado
- Inclua exemplos de uso ou casos reais


## Reporting Security Issues
### Reporte Responsável
- Veja [SECURITY.md](SECURITY.md) para o processo completo
- Não abra issues públicas para vulnerabilidades
- Use GitHub Security Advisories para divulgação responsável


## Release Process
### Fluxo de Release para Mantenedores
- Bump do campo `version` em `Cargo.toml`
- Atualize `CHANGELOG.md` movendo conteúdo de `[Unreleased]` para novo header de versão com data
- Sincronize `CHANGELOG.pt-BR.md` com a mesma entrada bilíngue
- Execute os 10 gates de validação completos
- Crie tag anotada: `git tag -a v0.X.Y -m "descrição"`
- Push: `git push origin main && git push origin v0.X.Y`
- O workflow `.github/workflows/release.yml` executa o restante: matrix de build com 5 targets mais macOS Universal, GitHub Release e publicação no crates.io
- Mantenedores: garanta que o secret `CRATES_IO_TOKEN` está configurado antes de criar a tag
- O gate `pre-publish` aborta a publicação se `CARGO_INSTALL_FLAGS=--locked` falhar


## Notas da Release v0.7.8
### Oito Gaps Fechados (Anti-Bot Detector Overhaul)
- GAP-WS-50 — listas expandidas em `src/probe_deep.rs` (8 marcadores Cloudflare + 1 DDG)
- GAP-WS-51 — constante `PROBE_CALIBRATION_QUERY` em `src/lib.rs` para query canônica do probe
- GAP-WS-52 — predicado de fallback condicional em `src/search.rs` honra o detector real
- GAP-WS-53 — níveis `-vv` e `-vvv` adicionados em `src/cli.rs` com `ArgAction::Count`
- GAP-WS-54 — `scraper` bumpado para 0.27 resolve RUSTSEC-2025-0057 transitivo
- GAP-WS-55 — bloco wreq reescrito em `Cargo.toml` com pin exato em 6.0.0-rc.29
- GAP-WS-56 — subcomando `Buscar` marcado como `#[command(hide = true)]`
- GAP-WS-57 — `retries` agora honrado em `src/parallel.rs` no laço de error_output
- ADR completa em `docs/decisions/0002-anti-bot-detector-overhaul-v0-7-8.md`


## Notas da Release v0.7.9

### Ghost-Block + Markers 2026 (Oito Gaps Fechados)
- GAP-WS-58 (CRITICAL) — `detectar_interstitial` classifica body sub-4KB sem `result-page-signal` como `InterstitialKind::Cloudflare`
- GAP-WS-59 (HIGH) — 5 marcadores Cloudflare novos + 1 marker DDG novo
- GAP-WS-59 (HIGH) — `--allow-lite-fallback` e `--pre-flight` viraram `global = true`
- v0.7.9 P1 — `detectar_interstitial_com_match` retorna `(&'static str, InterstitialKind)` com marker literal
- v0.7.9 P3 — `SearchMetadata.pre_flight_fired: bool` adicionado ao envelope
- v0.7.9 P4b — `sugestao_mitigacao_com_marker` injeta marker real (ex.: `cf-challenge`)
- `Config.pre_flight` adicionado com default `false`


## Notas da Release v0.7.10

### Pino de Identidade + Bench Wiring + Pre-Publish Gate (Sete Gaps Fechados)
- GAP-WS-60 (CRITICAL) — `--identity-profile` propaga para `failure_output` e `error_output` via `identity_tag_for_cli_identity` em `src/identity.rs`
- GAP-AUD-001 (auditoria local) — pino `identidade_usada` agora presente em failure paths (era `null`)
- GAP-AUD-002 (auditoria local) — `[[bench]] harness = false` em `Cargo.toml` corrige `cargo bench` que rodava test harness
- B1 (CRITICAL) — `--pre-flight` não emite mais dois JSON concatenados no stdout
- B2 (CRITICAL) — `pre_flight_blocked` agora retorna exit 3 (era 0)
- B3 (MÉDIO) — `--global-timeout` virou global, aceito em subcomandos
- B4 (CRITICAL) — `--probe-deep` standalone retorna exit 3 quando detecta captcha
- v0.7.10 P4 — `--require-results` em `deep-research`, exit 4 quando fan-out zero
- v0.7.10 P5 — probe-deep scheduler integrado em `execute_single_search`
- v0.7.10 P6 — snapshot test `cloudflare_markers_snapshot_v0_7_10` via `insta = "1"`
- v0.7.10 P7 — `src/proxy_detection.rs` novo módulo (Vivo Fiber, Gigaweb, Cloudflare)
- v0.7.10 P16 — `src/ddg_class_watch.rs` watchdog runtime
- v0.7.10 P19 — `scripts/pre-publish-gate.sh` 7 gates antes de `cargo publish`
- v0.7.10 P19 — `skill/duckduckgo-search-cli-{en,pt}/eval-queries.json` +4 queries (q47-q50)

### Mudança de Workflow (regra 1244)
- A partir de v0.7.10, releases usam `atomwrite` direto + `TaskCreate` em vez de Agent Teams, devido a bug conhecido de estado `Team does not exist` documentado em `mem 1244` do graphrag
- Lead continua orquestrando, mas edições atômicas vão via `atomwrite --workspace . write --expect-checksum <CS>`
- Cada patch: `atomwrite read` (checksum) → `atomwrite write` → `cargo check --offline` → `cargo test --lib --offline`
