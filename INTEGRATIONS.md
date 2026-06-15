# Integrations

`duckduckgo-search-cli` integrates with 16+ AI agents and automation platforms
via its stable JSON contract, deterministic exit codes, and zero-dependency
binary install. This file is a pointer to the full integration catalog.

## Full Catalog

See [`docs/INTEGRATIONS.md`](docs/INTEGRATIONS.md) for the complete
integration guide, including:

- 16 supported AI agents (Claude, GPT, Gemini, Cursor, OpenCode, etc.)
- Flag aliases introduced in each version
- Summary table consolidating all integrations
- Per-platform installation recipes
- Exit code semantics for agent decision-making
- Per-integration snippets with `timeout`, `jaq`, and `PIPESTATUS`

## Quick Reference

```bash
# Canonical invocation
timeout 60 duckduckgo-search-cli -q -f json --num 15 "query"

# Exit codes
0  success         → parse .resultados
1  runtime error   → read stderr; retry once with -v
2  config error    → re-run init-config --force
3  anti-bot block  → back off 300+ s; switch --endpoint lite
4  global timeout  → raise --global-timeout; reduce --parallel
5  zero results    → refine query or try different --lang

# Current version: v0.7.7 (v0.7.8 em desenvolvimento no branch main)
```

## v0.7.3 Highlights for Integrations
## v0.7.5 Highlights for Integrations

- **GAP-WS-29 fixed (CRITICAL, build experience, Windows)** — `cargo install` on native Windows MSVC without the **C++ CMake tools for Windows** sub-component of the Visual Studio Installer previously failed minutes into the BoringSSL build with the cryptic `program not found / is 'cmake' not installed?`. The `build.rs` preflight now detects this and aborts in SECONDS with the exact fix (`winget install -e --id Kitware.Cmake` OR Visual Studio Installer → Modify → Workloads → Desktop development with C++ → expand → check C++ CMake tools for Windows). New escape hatch: `DDG_SKIP_CMAKE_CHECK=1`.
- **GAP-WS-30 fixed (CRITICAL, build experience, Windows)** — BoringSSL CMake uses the Visual Studio 17 2022 generator which requires `cl.exe` (compiler) and `link.exe` (linker). The `build.rs` preflight now detects both and aborts with the fix (open a Developer PowerShell for VS 2022, or run `Launch-VsDevShell.ps1`). MSVC is NOT auto-installed (5+ GB download, too intrusive). New escape hatch: `DDG_SKIP_MSVC_CHECK=1`.
- **GAP-WS-31 fixed (CRITICAL, build experience, Windows)** — BoringSSL perlasm generator emits crypto assembly in NASM format and requires `perl.exe`. The `build.rs` preflight now detects perl and reports the fix (`winget install -e --id StrawberryPerl.StrawberryPerl`). New escape hatch: `DDG_SKIP_PERL_CHECK=1`.
- **GAP-WS-32/35/36 fixed (MEDIUM, documentation)** — All remaining claims that "pre-built binaries from `cargo install` are unaffected" (or its PT/EN variants) are now qualified across `skill/duckduckgo-search-cli-en/SKILL.md`, `skill/duckduckgo-search-cli-pt/SKILL.md`, `llms-full.txt`, `docs/CROSS_PLATFORM.md`, `README.md`, and `README.pt-BR.md`. **`crates.io` NEVER distributes binaries**; `cargo install` always compiles from source. Users on Windows must satisfy the four BoringSSL build prerequisites (NASM, CMake, MSVC, Perl) themselves before `cargo install` can succeed.
- **`build.rs` preflight coverage expanded** — v0.7.4 only checked for NASM. v0.7.5 checks for all four BoringSSL build prerequisites (nasm, cmake, cl.exe, link.exe, perl) and supports four independent `DDG_SKIP_*_CHECK=1` escape hatches.
- **New `scripts/check-windows-toolchain.ps1`** — standalone diagnostic (no installs) that checks all 7 tools (cargo, rustc, cmake, nasm, cl.exe, link.exe, perl) and emits text or JSON output. Exit code 0 if all present, 1 otherwise. Useful for support tickets and CI gates.
- **New `docs/INSTALL-WINDOWS.md` (EN) + `docs/INSTALL-WINDOWS.pt-BR.md` (PT)** — step-by-step guide covering 5 installation methods (VS Installer + standalone; all-winget standalone; Chocolatey; helper script; standalone diagnostic). Includes troubleshooting for each of the 4 GAPs and the `DDG_SKIP_*_CHECK` escape hatches.
- **CI Windows jobs updated** — `.github/workflows/ci.yml` and `.github/workflows/release.yml` now verify CMake, install Perl, and verify MSVC Build Tools (in addition to the existing NASM step) in every Windows job. This eliminates the implicit dependency on the `windows-2022` image's pre-installed tooling.
- **Zero breaking changes to JSON output schema**. All v0.7.4 fields remain present. All v0.7.3 fields remain present.


- **GAP-WS-27 fixed (CRITICAL)**: The macOS CAPTCHA interstitial that returned HTTP 200 with `quantidade_resultados: 0` while Windows returned full results is closed. TLS stack changed from `rustls` to BoringSSL via `wreq 6.0.0-rc.29`. `cargo install` always compiles from source — crates.io does not distribute pre-built binaries for any platform. The build toolchain change is the trade-off for the BoringSSL TLS fix (GAP-WS-27 closed). Source builds on Linux require `cmake`, `perl`, `pkg-config`, and `libclang-dev`; source builds on Windows require NASM, CMake, MSVC, and Perl (see `gaps.md` GAP-WS-28/29/30/31 and `docs/INSTALL-WINDOWS.md`).
- **`session` feature (cookie persistence + warm-up)**:
  - New flags: `--no-warmup`, `--no-cookie-persistence`, `--cookies-path <PATH>`.
  - Cookie jar persisted to `~/.config/duckduckgo-search-cli/cookies.json` (Linux), `%APPDATA%\duckduckgo-search-cli\cookies.json` (Windows), or `~/Library/Application Support/duckduckgo-search-cli/cookies.json` (macOS) with Unix permissions `0o600`.
  - Warm-up adds one `GET https://duckduckgo.com/` before the first real query to populate session cookies.
- **`probe-deep` feature (CAPTCHA interstitial detection)**:
  - New flags: `--probe-deep` (run a real search query and classify the body as `ok` or `captcha`), `--allow-lite-fallback` (opt-in: only takes effect when the `detectar_interstitial` predicate reports `captcha`; in v0.7.8+ the fallback condition is honored end-to-end via GAP-WS-52).
  - New JSON report fields on the probe response: `status`, `cascata_motivo`, `sugestao_mitigacao`, `http_status`, `latency_ms`.
- **Zero breaking changes to JSON output schema**. All v0.7.2 fields remain present.

## v0.7.0 Highlights for Integrations

- **New subcommand `deep-research`**: agents that need multi-hop answers can
  drop in `duckduckgo-search-cli deep-research "question" --synthesize`
  and get a Markdown report back, with no extra orchestration. Inherits
  every global flag (`-q -f json`, `--num`, `--parallel`, `--proxy`,
  `--fetch-content`) plus deep-research-specific knobs
  (`--max-sub-queries`, `--sub-queries-file`, `--aggregate`,
  `--budget-tokens`, `--synth-format`).
- **Backward-compatible**: zero changes to `buscar`, `init-config`,
  default-config JSON schema, or any exit code. Existing pipelines keep
  working unchanged.

## v0.6.5 Highlights for Integrations

- **MP-26 FIX**: Windows build now compiles. Use `cargo install duckduckgo-search-cli`
  on any platform without manual patches.
- **CI-01 FIX**: CI matrix now green on all 3 SOs (Linux/macOS/Windows).
  Agents running on Windows runners can rely on the binary.
- **WS-12 Circuit breaker**: `--fetch-content --parallel` no longer cascades
  failures across hosts — one slow domain won't block the rest of the crawl.
- **WS-25 ProgressBar**: `indicatif` output to stderr auto-hides in pipes,
  so JSON pipelines on stdout stay clean.

See `CHANGELOG.md` for the complete v0.6.5 changelog and migration notes
from earlier versions.


## v0.7.6 Highlights for Integrations

- **GAP-WS-48 closed (HIGH, build experience)**: `cargo install` was breaking
  on certain platforms due to a `cargo` resolver conflict between
  `alloc-no-stdlib 2.0.4` and `alloc-no-stdlib 3.0.0` brought in transitively
  by the `wreq` stack. v0.7.6 pins `alloc-no-stdlib =2.0.4` directly in
  `Cargo.toml`. Reinstalling from crates.io now works without manual
  dependency cleanup.
- **No CLI contract changes**: All v0.7.5 flags, JSON fields, and exit codes
  remain present. Drop-in replacement.
- **CI change**: The `pre-publish` job now resolves the dependency graph
  during release — if the pin ever drifts again, the release will fail
  before reaching crates.io.


## v0.7.7 Highlights for Integrations

- **GAP-WS-49 closed (CRITICAL, runtime regression)**: A `wreq-util`
  resolution failure in v0.7.6 broke BoringSSL TLS fingerprint emulation
  on certain Linux distributions. The result was silent CAPTCHA
  interception on hosts that previously worked. v0.7.7 pins `wreq-util`
  directly in `Cargo.toml` — no more resolution drift.
- **No CLI contract changes**: All v0.7.6 flags, JSON fields, and exit
  codes remain present. Drop-in replacement.
- **Recommended upgrade path**: v0.7.5 → v0.7.7 is the cleanest jump
  for users who skipped v0.7.6. The v0.7.6 → v0.7.7 delta is
  dependency-only.


## v0.7.8 Highlights for Integrations

- **GAP-WS-50**: `probe-deep` interstitial list expanded — 8 Cloudflare
  markers plus 1 DDG anomaly marker. False-negative rate on CAPTCHA
  detection dropped measurably in benchmark runs.
- **GAP-WS-52**: `--allow-lite-fallback` now reads the real detector
  result. When the detector flags a CAPTCHA but the flag is off, the CLI
  emits a structured `tracing::warn!` and exits with the appropriate
  code instead of silently degrading. Surfaces the trade-off to
  integrators.
- **GAP-WS-53**: `-vv` (debug) and `-vvv` (trace) levels added. Operators
  investigating failed searches can escalate verbosity without
  recompiling. The flag `conflicts_with = "quiet"`.
- **GAP-WS-54**: `scraper` bumped to `0.27.0`. Removes the transitive
  `fxhash 0.2.1` (RUSTSEC-2025-0057, unmaintained). `cargo audit
  --deny warnings` is now a blocking gate in CI and release.
- **GAP-WS-55**: `wreq` block in `Cargo.toml` rewritten to match the
  actual pin in use (`6.0.0-rc.29` plus three direct pins). Eliminates
  documentation-vs-code drift.
- **GAP-WS-56**: Legacy `Buscar` subcommand hidden from `--help` via
  `#[command(hide = true)]`. Remains callable for backward compatibility.
- **GAP-WS-57**: `--retries` flag now honored end-to-end in
  `src/parallel.rs:644`. The previous behavior silently dropped the
  value in the `error_output` path. Integrators relying on retry
  behavior will see it actually take effect.
- **No breaking changes to JSON output schema**. All v0.7.7 fields
  remain present. Drop-in replacement once v0.7.8 is published.

See `CHANGELOG.md` for the complete v0.7.6/v0.7.7/v0.7.8 changelog and
the ADR at `docs/decisions/0002-anti-bot-detector-overhaul-v0-7-8.md`
for the full design rationale of the v0.7.8 overhaul.
