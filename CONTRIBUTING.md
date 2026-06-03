# Contributing to duckduckgo-search-cli

Thanks for your interest in contributing to duckduckgo-search-cli.
Every contribution improves a tool used by developers and AI agents worldwide.
Read this in [Português](CONTRIBUTING.pt-BR.md).


## Quick Start
### Setup em Cinco Comandos
- Clone o repositório: `git clone https://github.com/daniloaguiarbr/duckduckgo-search-cli`
- Acesse o diretório: `cd duckduckgo-search-cli`
- Verifique compilação: `cargo check-all`
- Execute clippy: `cargo lint`
- Rode os testes: `cargo nextest run`

Aliases estão em `.cargo/config.toml`.


## Development Setup
### Prerequisites
- MSRV (Minimum Supported Rust Version): Rust 1.75
- Execute `rustup update stable` para garantir a versão mínima
- Instale nextest com: `cargo install cargo-nextest`
- Instale llvm-cov com: `cargo install cargo-llvm-cov`
- Instale cargo-audit com: `cargo install cargo-audit`
- Instale cargo-deny com: `cargo install cargo-deny`


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
- Propague erros com `?` e contexto via `.context("...")`
- Use `anyhow::Result` em caminhos binários e `thiserror` em structs de biblioteca
### I/O Centralizado
- O módulo `output.rs` é o ÚNICO lugar permitido para chamar `println!` ou `print!`
- Todos os outros módulos registram via `tracing`
### TLS Obrigatório
- Use `rustls` exclusivamente
- Não reative `native-tls` — quebra NixOS, Alpine e builds musl estáticos
### Restrições de Design
- Sem cache, sem MCP, sem API paga — restrições inegociáveis do blueprint v2


## Testing
### Três Camadas de Teste
- Testes unitários inline com `#[cfg(test)] mod testes` para funções puras
- Testes de integração em `tests/` usando `wiremock` — ZERO HTTP real
- Doctests dentro de blocos `///` em APIs públicas — duplos como exemplos no docs.rs
### Execução de Testes
- Execute testes com `cargo nextest run` (runner preferido)
- Instale nextest com: `cargo install cargo-nextest`
- Execute cobertura com `cargo llvm-cov` — mínimo 80% obrigatório
- Qualquer PR que reduza a cobertura abaixo do limite falhará no CI


## 10-Gate Validation Matrix
### Gates Obrigatórios
Every PR must pass all 10 gates (enforced by `.github/workflows/ci.yml`):

| # | Gate | Comando local |
|---|------|---------------|
| 1 | Compilation | `cargo check-all` |
| 2 | Clippy | `cargo lint` |
| 3 | Format | `cargo fmt --check` |
| 4 | Docs | `RUSTDOCFLAGS="-D warnings" cargo docs` |
| 5 | Tests | `cargo test-all` |
| 6 | Coverage >= 80% | `cargo cov` |
| 7 | Vuln audit | `cargo audit` |
| 8 | Supply chain | `cargo deny check advisories licenses bans sources` |
| 9 | Publish dry-run | `cargo publish-check` |
| 10 | Package content | `cargo pkg-list` |


## Pull Request Checklist
### Itens Verificáveis Antes de Abrir PR
- `cargo fmt --all --check` retorna ZERO diferenças
- `cargo clippy --all-targets -- -D warnings` retorna ZERO warnings
- `cargo test --all-features` retorna ZERO falhando
- `cargo nextest run` retorna ZERO falhando
- `cargo doc --no-deps` sem warnings
- `cargo audit` sem vulnerabilidades conhecidas
- CHANGELOG.md atualizado com a mudança
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
- Execute os 10 gates de validação completos
- Crie tag anotada: `git tag -a v0.X.Y -m "descrição"`
- Push: `git push origin main && git push origin v0.X.Y`
- O workflow `.github/workflows/release.yml` executa o restante: matrix de build com 5 targets mais macOS Universal, GitHub Release e publicação no crates.io
- Mantenedores: garanta que o secret `CRATES_IO_TOKEN` está configurado antes de criar a tag
