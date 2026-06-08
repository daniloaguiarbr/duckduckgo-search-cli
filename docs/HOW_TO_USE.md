# How to Use duckduckgo-search-cli

Real-time web search in your terminal — 15 fresh results in under 3 seconds.


## Why This Guide
- Follow this guide and run your first web search in under 60 seconds
- Learn core commands, advanced patterns, and shell pipeline integrations
- Understand every exit code and know exactly how to recover from each error


## Prerequisites
### Required
- Network access to duckduckgo.com
- Rust 1.88+ when installing via `cargo install` (MSRV since v0.7.2)
- Pre-built binaries do not require Rust installation
- **v0.7.3+ when compiling from source on Linux**: `cmake`, `perl`, `pkg-config`, and `libclang-dev` (BoringSSL build prerequisites via `wreq 6.0.0-rc`)
### Optional
- `jaq` (Rust replacement for `jq`) to process JSON in pipelines
- A SOCKS5 proxy for IP rotation when rate-limited


## Installation
### Cargo (Recommended)
- Run: `cargo install duckduckgo-search-cli`
- Binary location: `~/.cargo/bin/duckduckgo-search-cli`
- Verify: `duckduckgo-search-cli --version`
### Pre-built Binaries
- Download from [GitHub Releases](https://github.com/daniloaguiarbr/duckduckgo-search-cli/releases)
- Available for Linux (glibc + musl), macOS Universal, and Windows MSVC
- No Rust installation required — single static binary


## First Command
### Basic Search
```bash
duckduckgo-search-cli "rust async programming"
```
- Default: 15 results, auto-detects TTY for format
- Add `-f json` for machine-readable output
- Add `-q` to suppress tracing logs when piping
### Expected Output
```
 1. Title of first result
    https://example.com/page
    Snippet text describing the page content...

 2. Title of second result
    ...
```
- Use `-f json` to get structured output for scripts and agents
- Use `-f markdown` to get a linkable list for reports


## Core Commands
### Text Search
```bash
# Human-readable output (default on TTY)
duckduckgo-search-cli -n 5 "query"
```
- Default format on TTY is `text`
- Default format in pipes is `json`
- Use `-n N` to control result count (default: 15)
### JSON Output
```bash
# Machine-readable output for scripts and LLMs
duckduckgo-search-cli -q -n 10 -f json "query"
```
- Always pass `-q` when piping to suppress tracing logs
- Schema: `resultados[]` array with `titulo`, `url`, `snippet`
- Field order is frozen across releases — safe for scripted parsing
### Markdown Report
```bash
# Linkable list for reports and documents
duckduckgo-search-cli -n 15 -f markdown -o report.md "query"
```
- Format: `- [Title](URL)\n  > snippet`
- Use `-o` to save directly to file
### Save to File
```bash
# Atomic write — safe for concurrent scripts
duckduckgo-search-cli -q -n 10 -f json -o results.json "query"
```
- Creates parent directories automatically
- Unix permissions set to `0o644`
- Paths with `..` are rejected (path traversal protection)


## Advanced Patterns
### Fetch Page Content
```bash
# Download and embed cleaned page text into JSON
duckduckgo-search-cli -q -n 5 --fetch-content --max-content-length 8000 -f json "query"
```
- Field `conteudo` appears in each result object when enabled
- Use `--max-content-length` to cap characters per page (default: 10000)
- Use `--per-host-limit 1` to avoid hammering a single domain
### Multi-Query Parallel Search
```bash
# One query per line in queries.txt
duckduckgo-search-cli -q \
  --queries-file queries.txt \
  --parallel 3 \
  --per-host-limit 1 \
  --retries 3 \
  -n 10 -f json \
  -o results.json
```
- `--parallel` controls concurrent requests (1..=20)
- `--per-host-limit` caps fetches per domain (1..=10)
- Results grouped per query under `.buscas[]` in multi-query mode
### Time-Filtered Search
```bash
# Results from the last 24 hours only
duckduckgo-search-cli -q -n 10 --time-filter d -f json "breaking news query"
```
- Values: `d` (day), `w` (week), `m` (month), `y` (year)
- Combine with `--endpoint lite` for higher freshness on low-traffic queries
### Proxy Routing
```bash
# Route through a SOCKS5 proxy
duckduckgo-search-cli -q -n 10 --proxy socks5://127.0.0.1:9050 -f json "query"

# Route through an HTTP corporate proxy
duckduckgo-search-cli -q -n 10 --proxy http://user:pass@proxy.internal:8080 -f json "query"
```
- `--proxy` takes precedence over `HTTP_PROXY` and `ALL_PROXY` env vars
- Use `--no-proxy` to disable all proxy sources explicitly
### Language Control
```bash
# Portuguese results
duckduckgo-search-cli -q -n 10 --lang pt -f json "query"

# English results from the US
duckduckgo-search-cli -q -n 10 --lang en --country us -f json "query"
```
- Default lang: `pt`, default country: `br`
- Uses DuckDuckGo `kl` region codes


## Integration with Shell Scripts
### Extract URLs from Results
```bash
duckduckgo-search-cli -q -n 10 -f json "query" \
  | jaq -r '.resultados[].url'
```
- Outputs one URL per line, ready for `xargs` or downstream fetchers
### Filter by Snippet Keywords
```bash
duckduckgo-search-cli -q -n 20 -f json "query" \
  | jaq -r '.resultados[] | select(.snippet | test("rust")) | .titulo'
```
- `test()` in `jaq` is a regex match against the snippet text
### Count Results
```bash
duckduckgo-search-cli -q -n 10 -f json "query" \
  | jaq '.resultados | length'
```
- Verify actual count returned versus requested `-n`
### Handle Exit Codes in Scripts
```bash
duckduckgo-search-cli -q -n 10 -f json "query" > /tmp/out.json
case $? in
  0) echo "OK" ;;
  3) echo "Anti-bot block — wait 60s or rotate proxy" >&2 ;;
  4) echo "Global timeout exceeded" >&2 ;;
  5) echo "Zero results — try broader query" >&2 ;;
  *) echo "Error: exit $?" >&2 ;;
esac
```
- Always check `$?` before consuming the output file
- Exit code 3 is temporary — retry after a short pause


## Integration with AI Agents
### Claude Code
```bash
# In a Claude Code Bash tool call:
RESULTS=$(duckduckgo-search-cli -q -n 10 -f json "$QUERY" \
  | jaq -r '.resultados[] | "## \(.titulo)\n\(.snippet)\nURL: \(.url)\n"')
```
- Install the bundled skill for auto-activation without prompt engineering
- Skill path: `skill/duckduckgo-search-cli-en/SKILL.md`
### OpenAI Codex / GPT
```bash
# Feed structured JSON as context into messages[].content
duckduckgo-search-cli -q -n 10 -f json "$QUERY" | jaq '.resultados'
```
- The stable `resultados[]` schema maps cleanly to tool call response fields
- Use `--fetch-content` to embed full page bodies for deeper grounding
### Gemini
```bash
# Full page text as grounding data
duckduckgo-search-cli -q -n 5 \
  --fetch-content --max-content-length 5000 \
  -f json "$QUERY" \
  | jaq -r '.resultados[].conteudo // empty'
```
- Pipe content into Gemini's JSON mode for synthesis of long-tail facts
### Any LLM via Pipe
```bash
duckduckgo-search-cli -q -n 10 -f json "$QUERY" \
  | jaq -r '.resultados[] | "## \(.titulo)\n\(.snippet)\n"'
```
- Output is plain Markdown — paste directly into any context window
- See `docs/INTEGRATIONS.md` for 16 agent-specific drop-in snippets


## Common Errors
### HTTP 202 Anti-bot Block (exit 3)
- DuckDuckGo returned a soft challenge page, not real results
- Wait 60 seconds before retrying
- Rotate outbound IP with `--proxy socks5://127.0.0.1:9050`
- Increase retries: `--retries 5`
- Run `duckduckgo-search-cli init-config` to refresh browser profiles
### Global Timeout (exit 4)
- Pipeline exceeded `--global-timeout` (default: 60 seconds)
- Increase value: `--global-timeout 120`
- Reduce result count: `-n 5`
- Add `--endpoint lite` for faster responses on slow connections
### Zero Results (exit 5)
- Often temporary rate-limiting, not a permanent block
- Wait 60 seconds and retry the same query
- Broaden the query by removing specific terms
- Remove `--time-filter` if set — it narrows the result pool
- Try `--endpoint lite` as a fallback endpoint
### Invalid Config (exit 2)
- A flag is out of range or a path is invalid
- `--timeout 0` is rejected — minimum is 1 second
- `--output ../../../etc/passwd` is rejected — path traversal blocked
- `--global-timeout 0` is rejected — minimum is 1 second
- `--parallel 0` is rejected — minimum is 1


## Exit Codes Reference

| Code | Meaning | Recommended Action |
|------|---------|-------------------|
| 0 | Success | Process results normally |
| 1 | Runtime error (network, parse, I/O) | Check stderr for details |
| 2 | Invalid config (flag out of range, bad path) | Fix the argument |
| 3 | DuckDuckGo anti-bot block (HTTP 202) | Wait 60s or rotate proxy |
| 4 | Global timeout exceeded | Increase `--global-timeout` |
| 5 | Zero results across all queries | Broaden query or remove filters |


## Next Steps
- See `docs/COOKBOOK.md` for 15 copy-paste recipes for research, ETL, and monitoring
- See `docs/INTEGRATIONS.md` for 16 LLM agent integration guides
- See `docs/AGENTS-GUIDE.md` for the full stdin/stdout contract and schema reference
- See `docs/CROSS_PLATFORM.md` for Linux, macOS, Windows, and Docker setup guides
- See `docs/AGENT_RULES.md` for 30+ MUST/NEVER rules for production agent use


## v0.7.3 — Session + Probe-Deep + BoringSSL (GAP-WS-27 fix)

v0.7.3 atomically closes GAP-WS-27 (CAPTCHA on macOS) by replacing the `rustls` TLS stack with embedded BoringSSL via `wreq 6.0.0-rc.29`, plus session cookie persistence and deep CAPTCHA detection.

### TLS Stack Switch (wreq + BoringSSL)

The CLI now uses `wreq 6.0.0-rc.29` instead of `reqwest 0.12` + `rustls-tls`. `wreq` bundles BoringSSL (via `boring2 v4.15.11`) and produces a `JA4_o` fingerprint identical to real Chrome/Safari, closing the Cloudflare Bot Management entry point that produced the CAPTCHA.

- Added dependencies: `wreq = "6.0.0-rc"` with features `tokio-rt, webpki-roots, cookies, gzip, brotli, deflate, zstd, socks, form, query`; `wreq-util = "3.0.0-rc.12"`.
- Removed dependencies: `reqwest`, `rustls`, `cookie_store`, `cookie` (in direct deps).
- Formal ADR: `docs/decisions/0001-tls-boring-via-wreq.md`.

### Build Prerequisites Changed (v0.7.3+)

Compiling from source on Linux now requires `cmake`, `perl`, `pkg-config`, and `libclang-dev` (BoringSSL). Pre-built binaries from crates.io are unaffected.

```bash
# Debian/Ubuntu
sudo apt-get install cmake perl pkg-config libclang-dev
# Fedora/RHEL
sudo dnf install cmake perl pkg-config clang-devel
# Alpine
apk add cmake perl pkgconf clang-dev
```

### Session Cookie Persistence

The `session` feature persists DuckDuckGo cookies to `cookies.json` so subsequent requests reuse the session, and performs a `GET https://duckduckgo.com/` warm-up before the first real query to populate session cookies.

- Cookie jar location:
  - macOS: `~/Library/Application Support/duckduckgo-search-cli/cookies.json`
  - Linux: `~/.config/duckduckgo-search-cli/cookies.json`
  - Windows: `%APPDATA%\duckduckgo-search-cli\cookies.json`
- Unix permissions: `0o600` (owner read+write only).
- The cookie jar contains DuckDuckGo session cookies. Treat as a credential.

#### Session Flags

```bash
# Disable warm-up (skip the GET /warm-up request)
duckduckgo-search-cli --no-warmup "query"

# Keep cookies in memory only (don't write cookies.json to disk)
duckduckgo-search-cli --no-cookie-persistence "query"

# Point the cookie jar at an encrypted volume
duckduckgo-search-cli --cookies-path /Volumes/encrypted/cookies.json "query"
```

### Deep CAPTCHA Detection (probe-deep)

`--probe-deep` runs a real test query and classifies the returned body as `ok` or `captcha`:

```bash
duckduckgo-search-cli --probe-deep -q -f json
# {"status": "ok", "endpoint": "html", "http_status": 202,
#  "latency_ms": 97, "cascata_motivo": "none",
#  "sugestao_mitigacao": "no interstitial detected"}
```

Use `--probe-deep` in CI before launching expensive queries, especially on macOS runners where GAP-WS-27 manifested.

#### Automatic html→lite Fallback

By default, probe-deep only detects and reports. To trigger automatic fallback from `html` to `lite` when CAPTCHA is detected, pass `--allow-lite-fallback`:

```bash
duckduckgo-search-cli --probe-deep --allow-lite-fallback -q -f json "query"
```

### Empirical Validation (v0.7.3)

```bash
# Before (v0.7.2): quantidade_resultados: 0, ms: 1695
# After (v0.7.3): quantidade_resultados: 5, ms: 735
duckduckgo-search-cli "rust wreq emulation browser fingerprint 2026" -q -f json --num 5
```


## v0.7.2 — rand 0.10 RngExt + time 0.3.47 RUSTSEC-2026-0009 + MSRV 1.88

v0.7.2 is a maintenance release that addresses two upstream dependencies:

- `time = "0.3.47"` pinned as a direct dependency to override `time 0.3.40` which arrived transitively via `cookie_store 0.22.0` and `reqwest 0.12.28`. Resolves `RUSTSEC-2026-0009` (stack exhaustion DoS in time 0.3.40).
- `rand 0.10.1` reorganized `random_range`, `random_bool`, and `random` methods from trait `Rng` to extension trait `RngExt`. Replaced `use rand::Rng;` with `use rand::RngExt;` in `src/identity.rs`, `src/parallel.rs`, and `src/search.rs`.
- MSRV raised from 1.85 to 1.88 (required by `time 0.3.47` and `rand 0.10`).


## v0.7.1 — Maintenance Patch

v0.7.1 is a purely maintenance release with no new CLI flags and no new JSON fields. Syncs `Cargo.lock` self-version 0.7.0 → 0.7.1 and fixes latent clippy warnings.


## v0.7.0 — `deep-research` Subcommand

v0.7.0 introduces the `deep-research` subcommand for multi-hop research with sub-query fan-out.

```bash
duckduckgo-search-cli -q -f json deep-research "tokio vs async-std 2026" \
  --synthesize --synth-format markdown | jaq -r '.sintese'
```

New fields: `.metadados.sub_queries[]`, `.metadados.total_resultados_unicos`, `.metadados.tempo_total_ms`, `.resultados[].score`, `.resultados[].fontes[]`, `.sintese` (opt-in via `--synthesize`).


## v0.6.4/v0.6.5 — Adaptive Anti-Bot Identity Pool (WS-26)

### Problem
DuckDuckGo's anti-bot heuristics classify a single User-Agent + IP + header-order combination after the first request. Reusing the same identity across all pagination calls and across multiple queries produces a single fingerprint that gets blocked with HTTP 202 (anomaly), HTTP 403, or HTTP 429.

### Solution
v0.6.4 introduces a 12-identity pool (preserved in v0.6.5) with 5-level cascade rotation:

| Level | Strategy |
|-------|----------|
| 0     | Current identity (no rotation) |
| 1     | Same family, different platform |
| 2     | Different family, same platform |
| 3     | Different family and platform + endpoint downgraded to lite |
| 4     | Random identity + recommended 30-60s sleep before retry |

### Usage

#### Probe before launching a real query

```bash
duckduckgo-search-cli --probe
```

The probe sends one minimal request and reports status, latency, and Set-Cookie presence as JSON. Exit 0 means the endpoint is reachable from your IP/UA combination; exit 1 means the request failed.

#### Pin a specific identity (deterministic for tests)

```bash
duckduckgo-search-cli -q -n 10 -f json --identity-profile chrome-linux "query"
```

Valid profiles: `auto` (default), `chrome-win`, `chrome-mac`, `chrome-linux`, `edge-win`, `firefox-linux`, `safari-mac`.

#### Reproducible identity rotation (debugging anti-bot)

```bash
duckduckgo-search-cli -q -n 10 -f json --seed 42 "query"
```

Same seed produces the same sequence of identities across runs. Use this to reproduce anti-bot blocks for debugging.

#### Inspect which identity produced a response

```bash
duckduckgo-search-cli -q -n 5 -f json "query" | jaq '.metadados.identidade_usada'
# Output: "chrome-linux-11111111aaaa0001"
```


## v0.6.5 — Windows install fixed, CI green, circuit breaker, ProgressBar

v0.6.5 is a quality release with no new CLI flags and no new JSON fields.
It focuses on making the tool reliable across all three target platforms
and on long-running crawls.

### Windows now works out of the box (MP-26)

`cargo install duckduckgo-search-cli` on Windows failed in v0.6.4 because
the upstream `windows-sys 0.59+` changed the `HANDLE` type from `isize` to
`*mut c_void`. v0.6.5 fixes this with:

```rust
// src/platform.rs:51-69 — type-safe HANDLE check
let handle = unsafe { GetStdHandle(STD_OUTPUT_HANDLE) };
if !handle.is_null() && handle != INVALID_HANDLE_VALUE {
    if unsafe { GetConsoleMode(handle, &mut mode) } != 0 { ... }
}
```

The cast `handle as isize` (which would have been UB) is removed entirely.

### Circuit breaker protects long crawls (WS-12)

When `--fetch-content --parallel` scrapes many pages from the same domain,
3 consecutive failures on that host now open the circuit for 30 seconds.
All requests to that host are short-circuited during the cooldown,
preventing cascading failures that would block the entire crawl.

You don't need to do anything — the breaker is automatic. But you can
observe it in stderr if `--verbose` is set.

### ProgressBar on stderr, not stdout (WS-25)

`--fetch-content` now shows a progress bar on stderr. JSON output on stdout
stays clean for pipes. The bar auto-hides in non-TTY contexts (CI, logs).

### CI matrix green on all 3 SOs (CI-01)

v0.6.4 was published with a broken CI on Linux, macOS, and Windows. v0.6.5
restores the green matrix by fixing 6 latent clippy errors and adding
per-platform smoke tests (`--version --help`) to the CI pipeline.

### New lints block future FFI drift

`improper_ctypes = "deny"` and `improper_ctypes_definitions = "deny"` are
now active. These would have caught the v0.6.4 HANDLE issue at compile time
if they had been active then.

The `identidade_usada` field reports the identity that produced the successful response. The `nivel_cascata` field reports the cascade level reached (0-4).


## v0.7.0 — Deep Research Pipeline

For multi-hop research questions, use the `deep-research` subcommand. It decomposes one query into up to 12 sub-queries, fans them out in parallel, aggregates via RRF or canonical-URL dedup, and optionally produces a Markdown report.

## v0.7.3 — Session + Probe-Deep + BoringSSL

The TLS stack changed from `rustls` to BoringSSL via `wreq 6.0.0-rc.29`. This closes the GAP-WS-27 macOS CAPTCHA (Cloudflare Bot Management detected `rustls` as a non-browser fingerprint via JA4_o). BoringSSL produces a JA4_o identical to Chrome/Safari. See `docs/decisions/0001-tls-boring-via-wreq.md` for the architectural decision.

### Cookie persistence + warm-up

Each invocation now starts with a warm-up `GET https://duckduckgo.com/` (skippable with `--no-warmup`) that populates session cookies. The cookies are persisted to `~/.config/duckduckgo-search-cli/cookies.json` (Linux), `%APPDATA%\duckduckgo-search-cli\cookies.json` (Windows), or `~/Library/Application Support/duckduckgo-search-cli/cookies.json` (macOS) with Unix permissions `0o600`. The path is overridable via `--cookies-path <PATH>`. Treat this file as a credential. Use `--no-cookie-persistence` to keep cookies in memory only.

### CAPTCHA detection via probe-deep

`--probe-deep` runs a real search query and classifies the body as `ok` or `captcha` based on Cloudflare and DuckDuckGo markers (`cf-chl-bypass`, `cf-challenge`, `challenge-platform`, `Attention Required`, `__cf_chl_jschl_tk__`, `robot-detected`, `bots, we have detected`). The probe report includes `status`, `cascata_motivo`, `sugestao_mitigacao`, `http_status`, and `latency_ms`. Use this in CI gates for macOS runners to detect CAPTCHA early.

### Auto-fallback to lite (opt-in)

`--allow-lite-fallback` automatically switches from the `html` endpoint to the `lite` endpoint when `--probe-deep` (or zero-result retries) detect CAPTCHA. Off by default to avoid silently changing the content type of the response.

```bash
# 1. Quick fan-out (no synthesis, 5 sub-queries by default).
timeout 60 duckduckgo-search-cli -q -f json deep-research "best rust http client 2026" \
  | jaq '.resultados | length'

# 2. Synthesised Markdown report with a token budget.
timeout 120 duckduckgo-search-cli -q -f json deep-research "tokio vs async-std 2026" \
  --synthesize --synth-format markdown --budget-tokens 1500 \
  | jaq -r '.sintese'

# 3. Manual sub-queries (the file's `# comments` and blank lines are ignored).
cat > /tmp/qs.txt <<EOF
# Visão geral
what is tokio runtime 2026
# Comparação
tokio vs async-std
EOF
timeout 60 duckduckgo-search-cli -q -f json deep-research "tokio 2026" \
  --sub-queries-file /tmp/qs.txt --aggregate dedupe-by-url \
  | jaq '.metadados.sub_queries | length'
```

The `deep-research` subcommand inherits every global flag (`-q -f json`, `--num`, `--lang`, `--country`, `--parallel`, `--endpoint`, `--proxy`, `--retries`, `--global-timeout`, `--fetch-content`, `--max-content-length`) and adds:

- `--max-sub-queries N` — cap the fan-out (1..=12, default 5)
- `--sub-query-strategy` — `heuristic` (default) or `manual`
- `--sub-queries-file PATH` — required for `manual`; comments and blanks are ignored
- `--aggregate` — `rrf` (default, K=60) or `dedupe-by-url`
- `--synthesize` — produce a final report
- `--budget-tokens N` — cap the synthesis length (1 token ≈ 4 chars)
- `--synth-format` — `markdown` (default), `plain`, or `json`


## v0.7.3 — Session + Probe-Deep + BoringSSL — Operacional

A stack TLS mudou de `rustls` para BoringSSL via `wreq 6.0.0-rc.29`. Isso fecha o GAP-WS-27 do CAPTCHA do macOS (Cloudflare Bot Management detectou `rustls` como fingerprint de não-navegador via JA4_o). BoringSSL produz JA4_o idêntico ao Chrome/Safari. Ver `docs/decisions/0001-tls-boring-via-wreq.md` para a decisão arquitetural.

### Pré-requisitos de build

Compilar do código-fonte no Linux agora requer:

```bash
# Debian / Ubuntu
sudo apt install cmake perl pkg-config libclang-dev

# Fedora / RHEL
sudo dnf install cmake perl pkg-config clang-devel

# Alpine
sudo apk add cmake perl pkg-config clang-dev
```

Usuários que instalam o binário pré-compilado do crates.io não precisam dessas deps.

### Sessão + cookie jar

Cada invocação agora começa com um warm-up `GET https://duckduckgo.com/` (pode ser pulado com `--no-warmup`) que popula os cookies de sessão. Os cookies são persistidos em `~/.config/duckduckgo-search-cli/cookies.json` (Linux), `%APPDATA%\duckduckgo-search-cli\cookies.json` (Windows), ou `~/Library/Application Support/duckduckgo-search-cli/cookies.json` (macOS) com permissões Unix `0o600`. O path é sobrescrevível via `--cookies-path <PATH>`. Trate este arquivo como credencial. Use `--no-cookie-persistence` para manter cookies em memória apenas.

### Detecção de CAPTCHA via probe-deep

`--probe-deep` executa uma query real e classifica o body como `ok` ou `captcha` baseado em marcadores Cloudflare e DuckDuckGo (`cf-chl-bypass`, `cf-challenge`, `challenge-platform`, `Attention Required`, `__cf_chl_jschl_tk__`, `robot-detected`, `bots, we have detected`). O relatório inclui `status`, `cascata_motivo`, `sugestao_mitigacao`, `http_status` e `latency_ms`. Use isto em portões de CI para runners macOS para detectar CAPTCHA cedo.

```bash
# Em CI antes de queries reais em macOS
timeout 30 duckduckgo-search-cli --probe-deep -q -f json | jaq -e '.status == "ok"'
```

### Fallback automático para lite (opt-in)

`--allow-lite-fallback` muda automaticamente do endpoint `html` para o endpoint `lite` quando `--probe-deep` (ou retentativas de zero resultados) detectam CAPTCHA. Desligado por padrão para evitar mudar silenciosamente o tipo de conteúdo da resposta.
