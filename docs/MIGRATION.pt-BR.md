# Migration Guide

This guide covers version-to-version migration paths for `duckduckgo-search-cli`.
Each section documents breaking changes, additive changes, and rollback
instructions.

## Migration v0.6.4 → v0.6.5

### What Changes
- **No breaking changes** — v0.6.5 is fully backward-compatible with v0.6.4
- Windows build was broken in v0.6.4 and is fixed in v0.6.5
- CI now passes on all 3 SOs (Linux/macOS/Windows) — v0.6.4 had failing CI
- New `--fetch-content` long crawls now show a ProgressBar on stderr (auto-hidden in pipes)
- 5 new property tests in `extraction.rs`, 4 new circuit breaker tests, 1 new wiremock test

### Step-by-Step Migration

```bash
# Update to v0.6.5
cargo install duckduckgo-search-cli --version 0.6.5 --force

# Verify the new version
duckduckgo-search-cli --version
# duckduckgo-search-cli 0.6.5
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
