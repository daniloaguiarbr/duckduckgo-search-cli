# Migration Guide

This guide covers version-to-version migration paths for `duckduckgo-search-cli`.
Each section documents breaking changes, additive changes, and rollback
instructions.

## Migration v0.7.2 → v0.7.3

### What Changes
- **BREAKING BUILD-ENV (source builds only)**: TLS stack changed from `rustls` to BoringSSL via `wreq 6.0.0-rc.29`. Building from source on Linux now requires `cmake`, `perl`, `pkg-config`, and `libclang-dev`. **Building from source on Windows MSVC requires FOUR tools** (NASM, CMake 3.20+, MSVC C/C++ toolchain, Strawberry Perl — closed as GAP-WS-28/29/30/31 progressively in v0.7.4 and v0.7.5). **`cargo install` always compiles from source** — crates.io does not distribute pre-built binaries for any platform, so these prerequisites apply to every Windows user, not only to CI. See `docs/INSTALL-WINDOWS.md` for step-by-step setup. The `.github/workflows/release.yml` matrix installs these packages automatically.
- **GAP-WS-27 closed**: The macOS CAPTCHA interstitial is fixed. Same query that returned `quantidade_resultados: 0` in v0.7.2 returns 5 results in v0.7.3 on the same machine. See `gaps.md` and `docs/decisions/0001-tls-boring-via-wreq.md`.
- **New CLI flags (additive)**:
  - `--no-warmup` — skip the warm-up `GET https://duckduckgo.com/` before the first real query
  - `--no-cookie-persistence` — keep cookies in memory only; never write `cookies.json` to disk
  - `--cookies-path <PATH>` — override the default XDG cookie jar path
  - `--probe-deep` — run a real search query and classify the body as `ok` or `captcha` based on Cloudflare and DuckDuckGo markers
  - `--allow-lite-fallback` — opt-in to automatic fallback from `html` to `lite` endpoint when `--probe-deep` (or zero-result retries) detect CAPTCHA
- **New persistent state: cookie jar**: A `cookies.json` file is now written to `~/.config/duckduckgo-search-cli/cookies.json` (Linux), `%APPDATA%\duckduckgo-search-cli\cookies.json` (Windows), or `~/Library/Application Support/duckduckgo-search-cli/cookies.json` (macOS). Unix permissions are `0o600` (owner read+write only). Treat this file as you would treat a credential — see `SECURITY.md`. Use `--no-cookie-persistence` to opt out.
- **Zero changes to JSON output schema**. All fields from v0.7.2 remain present. No new `Option<T>` fields added at the top level.
- **New dependencies**: `wreq 6.0.0-rc.29`, `wreq-util 3.0.0-rc.12`, plus transitive `boring2 4.15.11`, `webpki-root-certs 1.0.7`, and the BoringSSL C toolchain.
- **Removed dependencies**: `reqwest 0.12.28`. `time 0.3.47` is no longer a direct dep — purely transitive now.
- **Test count: 292 lib** (was 279 in v0.7.2). +13 new tests across `session_warmup` (5), `wreq_cookie_adapter` (3), and `probe_deep` (5). 0 clippy warnings, 0 fmt diff, 2 cargo-deny warnings (RUSTSEC-2025-0057 + RUSTSEC-2025-0052, both already in ignore list).
- **Binary size**: +20 MB (BoringSSL is statically linked). Release build time: ~40s longer than v0.7.2 (BoringSSL compiles in).

### Step-by-Step Migration

```bash
# Update to v0.7.3 (build prereqs required — see below)
cargo install duckduckgo-search-cli --version 0.7.3 --force
# (Linux) sudo apt install cmake perl pkg-config libclang-dev
# (Windows MSVC) see docs/INSTALL-WINDOWS.md for the 4-tool setup

# Verify the new version
duckduckgo-search-cli --version
# duckduckgo-search-cli 0.7.3

# Verify the GAP-WS-27 fix on macOS
duckduckgo-search-cli "rust wreq emulation browser fingerprint" -q -f json --num 5
# Expect 5 results in under 2 seconds, no CAPTCHA

# Try the new probe-deep (CAPTCHA detection)
duckduckgo-search-cli --probe-deep -q -f json
# Expect: {"status": "ok", "cascata_motivo": "none", "sugestao_mitigacao": "..."}

# Build from source (only if compiling — not needed for `cargo install`)
sudo apt install cmake perl pkg-config libclang-dev
git checkout v0.7.3
cargo build --release
```

### JSON Schema Changes

No schema changes. v0.7.3 preserves all v0.7.2 fields:

| Field                          | Status    | Notes                                      |
|--------------------------------|-----------|--------------------------------------------|
| `.resultados[].titulo`         | unchanged | Always present when non-empty              |
| `.resultados[].url`            | unchanged | Always present when non-empty              |
| `.metadados.identidade_usada`  | unchanged | `Option<String>` — v0.6.4+                |
| `.metadados.nivel_cascata`     | unchanged | `Option<u32>` (0..=4) — v0.6.4+           |
| `.metadados.usou_endpoint_fallback` | unchanged | `bool` — v0.6.0+                        |

The `cookies.json` file is internal state and not exposed in the JSON output schema.

### Compatibility Notes
- v0.7.3 binary is API-compatible with v0.7.2 (no CLI flag removals, no JSON field removals)
- v0.7.3 build targets are unchanged: `x86_64-unknown-linux-gnu`, `x86_64-unknown-linux-musl`, `aarch64-apple-darwin`, `x86_64-apple-darwin`, `x86_64-pc-windows-msvc`
- v0.7.2 binaries that worked on Linux/macOS continue to work — no urgent upgrade required **unless** you were affected by the macOS CAPTCHA (GAP-WS-27)
- macOS users that experienced zero-result queries in v0.7.2 must upgrade to v0.7.3 to fix the CAPTCHA. The fix is structural (TLS fingerprint), not a workaround.
- The new `wreq 6.0.0-rc.29` dependency is automatically installed by `cargo install`.

### Rollback

If you need to roll back to v0.7.2 (e.g., for some unexpected BoringSSL build issue):

```bash
# Install a specific older version
cargo install duckduckgo-search-cli --version 0.7.2 --force
```

> **Note**: v0.7.2 was the version affected by GAP-WS-27. Rolling back re-introduces the macOS CAPTCHA bug. Only do this if v0.7.3 has a critical issue on your platform.

### See Also

- `CHANGELOG.md` — full changelog
- `gaps.md` — GAP-WS-27 entry with empirical reproduction
- `docs/decisions/0001-tls-boring-via-wreq.md` — architectural decision
- `docs/CROSS_PLATFORM.md` — BoringSSL build prerequisites
- `SECURITY.md` — cookie jar handling
- `README.md` — overview and quick start


## Migration v0.7.1 → v0.7.2


## Migration v0.7.3 → v0.7.4

### What Changes
- **Build-experience release** — same flags, same JSON output schema, no breaking changes
- **GAP-WS-28 fixed** — `build.rs` preflight detects the NASM assembler on PATH before invoking the BoringSSL CMake build
- Without NASM, the build fails in seconds with the exact fix instead of after minutes of cryptic CMake errors
- New env var `DDG_SKIP_NASM_CHECK=1` as an escape hatch for custom build environments
- CI matrix in `.github/workflows/release.yml` now installs NASM via Chocolatey on the Windows-2022 image

### Step-by-Step Migration

```bash
# Update to v0.7.4
cargo install duckduckgo-search-cli --version 0.7.4 --force

# Verify
duckduckgo-search-cli --version
# duckduckgo-search-cli 0.7.4
```

### JSON Schema Changes

None. v0.7.4 preserves all v0.7.3 fields, all v0.7.2 fields, and all v0.6.x fields. The preflight runs at build time only and does not affect the runtime JSON contract.

### Compatibility Notes
- v0.7.4 binary is API-compatible with v0.7.3, v0.7.2, and v0.6.x
- v0.7.4 build targets are unchanged from v0.7.3
- v0.7.3 binaries continue to work — upgrade is optional, recommended only for Windows MSVC users who hit the NASM build error

### Rollback

```bash
cargo install duckduckgo-search-cli --version 0.7.3 --force
```

### See Also
- `gaps.md` — GAP-WS-28 (Windows NASM build failure)
- `CHANGELOG.md` — v0.7.4 release notes


## Migration v0.7.4 → v0.7.5

### What Changes
- **Documentation and build-experience release** — same flags, same JSON output schema, no breaking changes
- **Pre-flight build tooling detection (4 detectors)**: v0.7.5 adds `build.rs` preflight that detects whether the local toolchain has the four BoringSSL build prerequisites on Windows MSVC: NASM, CMake 3.20+, MSVC C/C++ toolchain (cl.exe, link.exe), Strawberry Perl
- **4 escape hatches** for Windows build failures: clear actionable error messages with the exact `cargo install` retry that pulls in the missing tool
- **`cargo install` ALWAYS compiles from source** — crates.io does NOT distribute pre-built binaries for any platform; the 4-toolchain prerequisite applies to every Windows user, not only CI
- **CI matrix (`windows-2022`)** in `.github/workflows/ci.yml` and `.github/workflows/release.yml` now checks for AND installs CMake 3.20+, Strawberry Perl, MSVC C/C++ Build Tools, in addition to NASM (already present since v0.7.4)
- See `gaps.md` entries WS-29, WS-30, WS-31, WS-32, WS-33, WS-34, WS-35, WS-36, WS-37 for the full build-experience gap analysis

### Step-by-Step Migration

```bash
# Update to v0.7.5 (build prereqs required when compiling from source)
cargo install duckduckgo-search-cli --version 0.7.5 --force

# Verify the new version
duckduckgo-search-cli --version
# duckduckgo-search-cli 0.7.5

# (Windows MSVC) follow docs/INSTALL-WINDOWS.md for the 4-tool setup
# (Linux) sudo apt install cmake perl pkg-config libclang-dev
```

### JSON Schema Changes

No schema changes. v0.7.5 preserves all v0.7.4 fields, all v0.7.3 fields, and all v0.6.x fields. The `build.rs` preflight runs at build time only and does not affect the runtime JSON contract.

### Compatibility Notes
- v0.7.5 binary is API-compatible with v0.7.4, v0.7.3, and v0.6.x (no CLI flag removals, no JSON field removals)
- v0.7.5 build targets are unchanged: `x86_64-unknown-linux-gnu`, `x86_64-unknown-linux-musl`, `aarch64-apple-darwin`, `x86_64-apple-darwin`, `x86_64-pc-windows-msvc`
- v0.7.4 binaries continue to work — upgrade is optional, recommended only if you want the preflight detectors and the improved CI matrix
- The new `build.rs` preflight adds zero runtime cost — it only runs at `cargo build` time

### Rollback

```bash
cargo install duckduckgo-search-cli --version 0.7.4 --force
```

### See Also

- `gaps.md` — WS-29 through WS-37 (build experience gap chain)
- `docs/INSTALL-WINDOWS.md` — Windows MSVC 4-tool step-by-step
- `CHANGELOG.md` — v0.7.5 release notes

### What Changes
- **Zero breaking changes.** All CLI flags, JSON output schemas, and exit codes from v0.7.1 remain unchanged.
- **Security advisory fix (RUSTSEC-2026-0009)**: `time 0.3.40` denial-of-service via RFC 2822 stack exhaustion was being pulled in transitively via `cookie_store 0.22.0` → `reqwest 0.12.28`. v0.7.2 pins `time = "0.3.47"` as a direct dep to override the transitive constraint.
- **`rand` 0.10 migration**: dev-deps (proptest 1.11+, getrandom 0.4+) unified on rand 0.10 and the convenience methods moved from `Rng` to `RngExt`. All internal call sites updated: `random_range`, `random_bool`, `random`, and `IndexedRandom::choose`.
- **MSRV bump**: `rust-version` raised from 1.75 to 1.88 (required by `time 0.3.47+` and `rand 0.10`).
- **CI hygiene fix**: 6 latent clippy errors that were silently breaking the CI matrix in v0.7.1 are caught now by `cargo clippy --all-targets --all-features -- -D warnings`.

### Step-by-Step Migration

```bash
# Update to v0.7.2
cargo install duckduckgo-search-cli --version 0.7.2 --force

# Verify the new version
duckduckgo-search-cli --version
# duckduckgo-search-cli 0.7.2

# Verify the rand 0.10 migration works
duckduckgo-search-cli "rust async" -q -f json | jaq '.resultados[].titulo'
```

### JSON Schema Changes

No schema changes. v0.7.2 preserves all v0.7.1 fields.

### Rollback

```bash
cargo install duckduckgo-search-cli --version 0.7.1 --force
```


## Migration v0.7.0 → v0.7.1

### What Changes
- **Zero breaking changes.** All CLI flags, JSON output schemas, and exit codes from v0.7.0 remain unchanged.
- **Dependency migration (internal)**: `rand` bumped from `0.8` to `0.9` to align with `proptest 1.11+` (dev-dep). All internal call sites updated.
- **MSRV bump**: `rust-version` raised from `1.75` to `1.85` to satisfy `rand 0.9` MSRV and the wave of edition-2024 transitive deps.
- **reqwest builder cleanup**: removed `ClientBuilder::gzip(true)` and `.brotli(true)` calls.
- **CI hygiene**: two `actionlint` shellcheck warnings fixed.
- **Security advisory ignore**: `RUSTSEC-2026-0009` (time 0.3.40 DoS) added to `deny.toml` ignore list.

### Step-by-Step Migration

```bash
# Update to v0.7.1
cargo install duckduckgo-search-cli --version 0.7.1 --force

# Verify
duckduckgo-search-cli --version
# duckduckgo-search-cli 0.7.1
```

### JSON Schema Changes

No schema changes. v0.7.1 preserves all v0.7.0 fields.

### Rollback

```bash
cargo install duckduckgo-search-cli --version 0.7.0 --force
```


## Migration v0.6.x → v0.7.0

### What Changes
- **Additive only** — v0.7.0 is fully backward-compatible with v0.6.x. The
  `buscar` subcommand, default-config JSON schema, every existing flag,
  and every exit code remain byte-for-byte identical.
- **New public subcommand** `deep-research` for multi-hop LLM research.
  Operators that do not invoke `deep-research` see no observable change.
- **Four new public modules** in `lib.rs` — `deep_research`,
  `decomposition`, `aggregation`, `synthesis` — composable from
  downstream crates.
- **New direct dependencies** in `Cargo.toml`: `url = "2"`, `regex = "1"`,
  and `proptest = "1"` (dev-only). All three are pure additions; no
  dependency was upgraded or removed.

### What to Update in Your Pipeline
- If you script against the `Subcommand` enum, add a match arm for
  `Subcommand::DeepResearch(DeepResearchArgs)`.
- If you consume `lib::run` directly, route `args.subcommand` to
  `lib::execute_deep_research` (the helper that builds a default `Config`
  and calls the pipeline).
- If you pin a minimum-supported version in `Cargo.toml` of a downstream
  crate, bump to `duckduckgo-search-cli = "0.7"`.
- No JSON-schema migration is required: the `SearchOutput` and
  `MultiSearchOutput` schemas are unchanged.

### Rollback
- Pin to `duckduckgo-search-cli = "0.6.5"` in downstream crates; the
  binary on crates.io is fully backward-compatible.

## Migration v0.6.4 → v0.6.5

### What Changes
- **No breaking changes** — v0.6.5 is fully backward-compatible with v0.6.4
- **Windows build fixed (MP-26)** — `cargo install duckduckgo-search-cli`
  on Windows now succeeds. v0.6.4 was unbuildable due to
  `windows-sys 0.59+` changing `HANDLE` from `isize` to `*mut c_void`.
  v0.6.5 uses `!handle.is_null() && handle != INVALID_HANDLE_VALUE` and
  passes the `HANDLE` directly to Win32 APIs.
- **CI matrix restored (CI-01)** — v0.6.4 was published with `validate`
  failing on Linux, macOS, and Windows due to 6 latent clippy errors
  (3× `doc_markdown`, 1× `needless_return`, 2× `missing_debug_implementations`).
  v0.6.5 fixes them all. CI now runs `cargo clippy --all-targets --all-features -- -D warnings`
  on every push.
- **New lints active** — `improper_ctypes`, `improper_ctypes_definitions`,
  `missing_safety_doc`, and `unsafe_op_in_unsafe_fn` are now `deny` to
  prevent future regressions of the v0.6.4 HANDLE issue.
- **Per-host circuit breaker (WS-12)** — `--fetch-content --parallel` now
  opens a 30s breaker on a host after 3 consecutive failures. No CLI flag.
- **ProgressBar (WS-25)** — `--fetch-content` shows a progress bar on
  stderr. Auto-hides in pipes. New transitive dep: `indicatif 0.18`.
- **Property-based tests (WS-11)** — 5 invariants in `extraction.rs`
  validate empty inputs, dense positions, absolute URLs, idempotence,
  malformed HTML tolerance. Zero new dependencies.
- **Retry-After header test (WS-23)** — wiremock test validates 429
  responses respect `Retry-After: N` delay. Uses existing `wiremock 0.6`
  dev-dependency.
- **CI smoke tests** — every platform runs `--version` and `--help` on
  the built binary before declaring green. New `cargo build --no-default-features`
  job validates the minimal build.
- **Test count** — 333 tests in v0.6.5 (was 322 in v0.6.4). 11 new tests
  added (5 WS-11 + 4 WS-12 + 1 WS-23 + 1 fix).

### Step-by-Step Migration

```bash
# Update to v0.6.5
cargo install duckduckgo-search-cli --version 0.6.5 --force

# Verify the new version
duckduckgo-search-cli --version
# duckduckgo-search-cli 0.6.5

# Verify Windows console UTF-8 still works
duckduckgo-search-cli "olá mundo" --num 5 -q -f json | jaq '.resultados[].titulo'

# Try the new circuit breaker on a long crawl
timeout 120 duckduckgo-search-cli \
  --queries-file /tmp/long-queries.txt \
  -q -f json --parallel 5 --per-host-limit 1 \
  --fetch-content --max-content-length 5000 \
  --global-timeout 100
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
- The new `indicatif 0.18` transitive dependency is automatically installed

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


## Migration v0.7.5 → v0.7.6

### What Changes
- **GAP-WS-48 (CRITICAL, install) — same-day `cargo install` fix** for an `alloc-no-stdlib 2.0.4` vs `3.0.0` version conflict that broke fresh installs.
- No breaking changes to CLI flags, JSON output schemas, or exit codes.
- No changes to runtime behavior from v0.7.5; the only diff is in dependency resolution at `cargo install` time.
- See `gaps.md` entry GAP-WS-48 for the conflict trace.

### Step-by-Step Migration

```bash
# Update to v0.7.6
cargo install duckduckgo-search-cli --version 0.7.6 --force

# Verify the new version
duckduckgo-search-cli --version
# duckduckgo-search-cli 0.7.6
```

### JSON Schema Changes

No schema changes. v0.7.6 preserves all v0.7.5 fields, all v0.7.4 fields, and all v0.6.x fields.

### Compatibility Notes
- v0.7.6 binary is API-compatible with v0.7.5, v0.7.4, v0.7.3, and v0.6.x
- v0.7.6 build targets are unchanged from v0.7.5
- v0.7.5 binaries continue to work — upgrade is optional, recommended for users who hit the install conflict

### Validation
- `cargo install --version 0.7.6 --force` succeeds on a clean toolchain
- `duckduckgo-search-cli --version` reports 0.7.6
- `duckduckgo-search-cli "rust" -q -f json` returns the expected JSON envelope

### Rollback

```bash
cargo install duckduckgo-search-cli --version 0.7.5 --force
```

### See Also
- `gaps.md` — GAP-WS-48 (same-day install fix)
- `CHANGELOG.md` — v0.7.6 release notes


## Migration v0.7.6 → v0.7.7

### What Changes
- **GAP-WS-49 (CRITICAL, query) — TLS fingerprint emulation restored** via `wreq 6.0.0-rc.29` + `wreq-util 3.0.0-rc.12` (feature `emulation`).
- v0.7.6 resolved `cargo install` but the published binary produced zero-result queries because BoringSSL without emulation gives a JA3/JA4 fingerprint that Cloudflare Bot Management flags.
- v0.7.7 re-adds `wreq-util = { version = "3.0.0-rc", default-features = false, features = ["emulation"] }` plus the `brotli` feature on `wreq` and 2 direct pins to make `cargo install` reproducible.
- See `gaps.md` entry GAP-WS-49 for the full root cause and reproduction steps.

### Step-by-Step Migration

```bash
# Update to v0.7.7
cargo install duckduckgo-search-cli --version 0.7.7 --force

# Verify the new version
duckduckgo-search-cli --version
# duckduckgo-search-cli 0.7.7

# Verify a real query returns non-zero results
duckduckgo-search-cli "rust async runtime" -q -f json | jaq '.quantidade_resultados'
# Expect: >0
```

### JSON Schema Changes

No schema changes. v0.7.7 preserves all v0.7.6 fields, all v0.7.5 fields, and all v0.6.x fields.

### Compatibility Notes
- v0.7.7 binary is API-compatible with v0.7.6, v0.7.5, v0.7.4, v0.7.3, and v0.6.x
- v0.7.7 build targets are unchanged from v0.7.6
- v0.7.6 binaries continue to work but produce empty result sets due to GAP-WS-49

### Validation
- `cargo install --version 0.7.7 --force` succeeds on a clean toolchain
- `duckduckgo-search-cli --probe-deep -q -f json` reports `status: "ok"`
- 5/5 sample queries return `quantidade_resultados > 0`
- `duckduckgo-search-cli --version` reports 0.7.7

### Rollback

```bash
cargo install duckduckgo-search-cli --version 0.7.6 --force
```

### See Also
- `gaps.md` — GAP-WS-49 (TLS fingerprint regression)
- `CHANGELOG.md` — v0.7.7 release notes
- `docs/decisions/0002-anti-bot-detector-overhaul-v0-7-8.md` — context for the v0.7.8 follow-up


## Migration v0.7.7 → v0.7.8

### What Changes
- **Anti-bot detector overhaul (GAP-WS-50, CRITICAL)** — `CLOUDFLARE_MARKERS` and `DDG_MARKERS` in `src/probe_deep.rs` expanded to recognize the new `anomaly-modal` interstitial that DDG rolled out in 2026-06-14.
- **Probe-deep calibration (GAP-WS-51, HIGH)** — query `q=rust` replaced with the 9-word pangram `the quick brown fox jumps over the lazy dog` via constant `PROBE_CALIBRATION_QUERY` in `src/lib.rs`.
- **Lite fallback opt-in (GAP-WS-52, HIGH)** — `--allow-lite-fallback` now consults `detectar_interstitial` before triggering; no more silent Lite fallback when the user did not opt in.
- **Verbose levels (GAP-WS-53, LOW)** — `-v` is now `ArgAction::Count`; `-vv` and `-vvv` work per Unix convention.
- **Supply chain (GAP-WS-54, MEDIUM)** — `scraper` bumped 0.20.0 → 0.27.0 to clear RUSTSEC-2025-0057 transitively via `fxhash 0.2.1`.
- **Docs drift (GAP-WS-55, LOW)** — `Cargo.toml` wreq comment rewritten to reflect the actual pin on `wreq 6.0.0-rc.29`.
- **Hidden subcommand (GAP-WS-56, LOW)** — `buscar` gets `#[command(hide = true)]`; no more duplicate `--help`.
- **Retries honored (GAP-WS-57, MEDIUM)** — `--retries N` propagates to `execute_with_retry` with `[1, 10]` clamp; `--retries 999` no longer triggers anti-bot.
- See `gaps.md` entries WS-50 through WS-57 for the full chain.

### Step-by-Step Migration

```bash
# Update to v0.7.8
cargo install duckduckgo-search-cli --version 0.7.8 --force

# Verify the new version
duckduckgo-search-cli --version
# duckduckgo-search-cli 0.7.8

# Verify the new probe-deep behavior
duckduckgo-search-cli --probe-deep -q -f json | jaq '.status'
# Expect: "ok" or "captcha" (honest classification)

# Verify the verbose levels
duckduckgo-search-cli -vvv --version
# Expect: prints version AND trace-level logs to stderr
```

### JSON Schema Changes

No schema changes. v0.7.8 preserves all v0.7.7 fields, all v0.7.6 fields, and all v0.6.x fields. The `metadados.retentativas` field is now populated correctly when `--retries N` is used.

### Compatibility Notes
- v0.7.8 binary is API-compatible with v0.7.7, v0.7.6, v0.7.5, v0.7.4, v0.7.3, and v0.6.x
- v0.7.8 build targets are unchanged from v0.7.7
- `buscar` subcommand still works when invoked directly; only hidden from `--help`
- `--retries` values above 10 are now clamped with a warning instead of triggering anti-bot
- v0.7.7 binaries continue to work but miss the `anomaly-modal` interstitial detection

### Validation
- `cargo install --version 0.7.8 --force` succeeds on a clean toolchain
- `cargo audit --deny warnings` reports 0 advisories
- `duckduckgo-search-cli --probe-deep -q -f json` returns `status: "ok"` in clean environments
- 5/5 sample queries return `quantidade_resultados > 0`
- `duckduckgo-search-cli -vv "rust" -q -f json` emits DEBUG-level logs to stderr
- `duckduckgo-search-cli "rust" -q -f json --retries 5` populates `metadados.retentativas = 5`
- 305 lib + 18 integration tests pass; 0 advisories unignored

### Rollback

```bash
cargo install duckduckgo-search-cli --version 0.7.7 --force
```

### See Also
- `gaps.md` — GAP-WS-50 through GAP-WS-57 (anti-bot detector overhaul chain)
- `docs/decisions/0002-anti-bot-detector-overhaul-v0-7-8.md` — ADR for the 8 gaps
- `CHANGELOG.md` — v0.7.8 release notes
- `docs/COOKBOOK.md` — Recipe 25 (detector), Recipe 26 (verbose), Recipe 27 (retries)
