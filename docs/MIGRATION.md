# Migration Guide

This guide covers version-to-version migration paths for `duckduckgo-search-cli`.
Each section documents breaking changes, additive changes, and rollback
instructions.

## Migration v0.7.2 → v0.7.3

### What Changes
- **BREAKING BUILD-ENV (source builds only)**: TLS stack changed from `rustls` to BoringSSL via `wreq 6.0.0-rc.29`. Building from source on Linux now requires `cmake`, `perl`, `pkg-config`, and `libclang-dev`. Pre-built binaries from crates.io are unaffected. The `.github/workflows/release.yml` matrix installs these packages automatically.
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
# Update to v0.7.3 (pre-built binary — no source build deps required)
cargo install duckduckgo-search-cli --version 0.7.3 --force

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
