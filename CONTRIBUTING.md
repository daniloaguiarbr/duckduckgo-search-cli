# Contributing to duckduckgo-search-cli

Thanks for your interest! This guide covers the minimum you need to land a
change successfully.

## Quick Start

```bash
git clone https://github.com/comandoaguiar/duckduckgo-search-cli
cd duckduckgo-search-cli
cargo check-all    # gate 1 — compila
cargo lint         # gate 2 — clippy -D warnings
cargo fmt --check  # gate 3 — format
cargo test-all     # gate 5 — todos os testes (unit + integration + doctest)
```

Aliases estão em `.cargo/config.toml`.

## 10-Gate Validation Matrix

Every PR must pass all 10 gates (enforced by `.github/workflows/ci.yml`):

| # | Gate | Comando local |
|---|------|---------------|
| 1 | Compilation | `cargo check-all` |
| 2 | Clippy | `cargo lint` |
| 3 | Format | `cargo fmt --check` |
| 4 | Docs | `RUSTDOCFLAGS="-D warnings" cargo docs` |
| 5 | Tests | `cargo test-all` |
| 6 | Coverage ≥ 80% | `cargo cov` |
| 7 | Vuln audit | `cargo audit` |
| 8 | Supply chain | `cargo deny check advisories licenses bans sources` |
| 9 | Publish dry-run | `cargo publish-check` |
| 10 | Package content | `cargo pkg-list` |

## Coding Standards

- **Language**: code comments, log messages, and struct field names are in
  Brazilian Portuguese per `CLAUDE.md`. Public API identifiers may be in
  English when that matches conventional Rust style (e.g., `from`, `into`).
- **Error handling**: use `anyhow::Result` in binary paths, `thiserror` in
  library structs. Never `.unwrap()` or `.expect()` in production code —
  propagate with `?` and context via `.context("...")`.
- **I/O**: the `output.rs` module is the **only** place allowed to call
  `println!` / `print!`. All other modules log via `tracing`.
- **TLS**: `rustls` only. Do not re-enable `native-tls` — it breaks NixOS,
  Alpine, and static musl builds.
- **No cache, no MCP, no paid API** — these are non-negotiable design
  constraints per the v2 blueprint.

## Testing

Three layers:

1. **Unit tests** inline (`#[cfg(test)] mod testes`) for pure functions.
2. **Integration tests** in `tests/` using `wiremock` — ZERO real HTTP.
3. **Doctests** inside `///` blocks in public API — they double as
   examples on docs.rs.

`cargo llvm-cov` must stay ≥ 80% overall. Any PR that drops coverage
below the threshold will fail CI.

## Supply Chain

- Every new dependency must pass `cargo deny check`. If the candidate
  brings a new license not in the allowlist or a transitive advisory,
  you must either find an alternative or document the ignore in
  `deny.toml` with `# Why:` and `# How to apply:` lines.
- Prefer crates with `trustScore ≥ 7` on `context7` (see `CLAUDE.md`).

## Commit Hygiene

- **Never** add `Co-authored-by:` trailers from AI agents (dependabot,
  renovate, Claude, GPT, Copilot, Cursor, Gemini, etc.) — CI blocks these.
- Use `squash and merge` for PRs with multiple commits.
- Commit messages follow conventional prefixes: `feat:`, `fix:`, `deps:`,
  `ci:`, `docs:`, `test:`, `refactor:`.

## Reporting Security Issues

See [SECURITY.md](SECURITY.md). **Do not open public issues for
vulnerabilities.** Use GitHub Security Advisories instead.

## Release Process

Releases are tag-driven:

1. Bump `version` in `Cargo.toml`.
2. Update `CHANGELOG.md` (move `[Unreleased]` content under a new
   version header, with date).
3. `git tag v0.X.Y && git push origin v0.X.Y`.
4. `.github/workflows/release.yml` handles the rest: build matrix
   (5 targets + macOS Universal), GitHub Release, crates.io publish.

Maintainers: ensure `CRATES_IO_TOKEN` secret is set before tagging.
