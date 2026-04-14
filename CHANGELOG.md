# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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
