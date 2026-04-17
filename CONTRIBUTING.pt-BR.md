# Como Contribuir

- Obrigado pelo seu interesse em contribuir!
- Este guia cobre o mínimo necessário para publicar uma mudança com sucesso.


## Início Rápido

```bash
git clone https://github.com/comandoaguiar/duckduckgo-search-cli
cd duckduckgo-search-cli
cargo check-all    # gate 1 — compila
cargo lint         # gate 2 — clippy -D warnings
cargo fmt --check  # gate 3 — format
cargo test-all     # gate 5 — todos os testes (unit + integration + doctest)
```

- Aliases disponíveis em `.cargo/config.toml`.


## Matriz de Validação com 10 Gates

- Todo PR deve passar pelos 10 gates (aplicados por `.github/workflows/ci.yml`):

| # | Gate | Comando local |
|---|------|---------------|
| 1 | Compilação | `cargo check-all` |
| 2 | Clippy | `cargo lint` |
| 3 | Formatação | `cargo fmt --check` |
| 4 | Docs | `RUSTDOCFLAGS="-D warnings" cargo docs` |
| 5 | Testes | `cargo test-all` |
| 6 | Cobertura >= 80% | `cargo cov` |
| 7 | Auditoria de vuln | `cargo audit` |
| 8 | Supply chain | `cargo deny check advisories licenses bans sources` |
| 9 | Dry-run de publish | `cargo publish-check` |
| 10 | Conteúdo do pacote | `cargo pkg-list` |


## Padrões de Código

- Idioma: comentários de código, mensagens de log e nomes de campos de structs devem ser em português brasileiro conforme `CLAUDE.md`
- Identificadores de API pública podem ser em inglês quando isso corresponder ao estilo Rust convencional (ex: `from`, `into`)
- Tratamento de erros: use `anyhow::Result` em caminhos de binário e `thiserror` em structs de biblioteca
- NUNCA use `.unwrap()` ou `.expect()` em código de produção — propague com `?` e contexto via `.context("...")`
- I/O: o módulo `output.rs` é o ÚNICO lugar autorizado a chamar `println!` / `print!`
- Todos os outros módulos registram via `tracing`
- TLS: somente `rustls` — não reative `native-tls` pois quebra NixOS, Alpine e builds musl estáticos
- Sem cache, sem MCP, sem API paga — restrições de design inegociáveis conforme o blueprint v2


## Testes

- Três camadas de teste são obrigatórias:
- Testes unitários inline (`#[cfg(test)] mod testes`) para funções puras
- Testes de integração em `tests/` usando `wiremock` — ZERO HTTP real
- Doctests dentro de blocos `///` na API pública — funcionam também como exemplos no docs.rs
- `cargo llvm-cov` deve manter >= 80% geral
- Qualquer PR que reduza a cobertura abaixo do limite falhará no CI


## Supply Chain

- Toda nova dependência deve passar por `cargo deny check`
- Se o candidato trouxer uma nova licença fora da allowlist ou um advisory transitivo, você deve encontrar uma alternativa ou documentar o ignore em `deny.toml` com linhas `# Why:` e `# How to apply:`
- Prefira crates com `trustScore >= 7` no `context7` (veja `CLAUDE.md`)


## Higiene de Commits

- NUNCA adicione trailers `Co-authored-by:` de agentes de IA (dependabot, renovate, Claude, GPT, Copilot, Cursor, Gemini, etc.) — o CI bloqueia esses trailers
- Use `squash and merge` para PRs com múltiplos commits
- Mensagens de commit seguem prefixos convencionais: `feat:`, `fix:`, `deps:`, `ci:`, `docs:`, `test:`, `refactor:`


## Reportando Problemas de Segurança

- Veja [SECURITY.pt-BR.md](SECURITY.pt-BR.md) para detalhes completos
- NÃO abra issues públicas para vulnerabilidades
- Use GitHub Security Advisories em vez disso


## Processo de Release

- Releases são orientadas por tags:
- Atualize `version` em `Cargo.toml`
- Atualize `CHANGELOG.md` (mova o conteúdo de `[Unreleased]` para um novo cabeçalho de versão com data)
- Execute `git tag v0.X.Y && git push origin v0.X.Y`
- `.github/workflows/release.yml` cuida do resto: matriz de build (5 targets + macOS Universal), GitHub Release, publicação no crates.io
- Mantenedores: certifique-se de que o secret `CRATES_IO_TOKEN` está configurado antes de criar a tag
