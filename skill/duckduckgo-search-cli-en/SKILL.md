---
name: duckduckgo-search-cli-en
description: Use this skill WHENEVER the user asks for web search, internet research, up-to-date documentation lookup, factual grounding, URL verification, page content extraction, external evidence gathering, RAG enrichment, fact-checking, library version lookup, incident post-mortem, current vendor pricing, multi-hop research questions, or any data outside the knowledge cutoff. Triggers include "search the web", "ground this", "web search", "fetch URL content", "look this up online", "verify this URL", "get current results", "deep research", "compare X vs Y", "what changed in Z". Invokes the `duckduckgo-search-cli` v0.7.3 CLI via Bash with a stable JSON contract, zero API key, 12-identity adaptive anti-bot pool with 5-level cascade rotation (HTTP 202/403/429), per-browser Sec-Fetch-* fingerprint profiles, BoringSSL TLS fingerprint (JA4_o identical to Chrome/Safari) via `wreq 6.0.0-rc.29`, cookie persistence with warm-up to XDG `cookies.json` (Unix permissions 0o600), `--probe-deep` CAPTCHA interstitial detector, path traversal validation on --output, automatic credential masking in error messages, and `identidade_usada` JSON field for diagnostic visibility. The v0.7.0 `deep-research` subcommand fans out one query into 1..=12 sub-queries, aggregates via RRF (K=60) or canonical-URL dedup, and optionally synthesises a Markdown/PlainText/JSON report with a token budget. Windows build fixed in v0.6.5 (MP-26 — `HANDLE` type-safe with `INVALID_HANDLE_VALUE`). Per-host circuit breaker (WS-12) protects against cascading failures in long crawls. indicatif ProgressBar (WS-25) visualizes long crawls. GAP-WS-27 (macOS CAPTCHA) fixed in v0.7.3 by switching from `rustls` to BoringSSL. Released 2026-06-08. See CHANGELOG.md and README.md for full notes. English version.
---

# Skill — `duckduckgo-search-cli` (EN)

## Inviolable Mission
- MUST invoke this CLI whenever the answer requires data outside the knowledge cutoff.
- NEVER invent URLs, library versions, changelogs, pricing, or news.
- ALWAYS prefer this skill over WebSearch/WebFetch for deterministic pipelines.

## Mandatory Invocation Triggers
- MUST invoke on triggers "search", "look up", "find online", "verify URL".
- MUST invoke before quoting any version, API, changelog, or external product price.
- MUST invoke before resolving repository names, authors, or canonical URLs.
- MUST invoke for grounding any factual claim that requires a verifiable source.

## Mandatory Invocation Contract
- ALWAYS pass `-q` to silence tracing logs on stderr.
- ALWAYS pass `-f json` explicitly to guarantee deterministic output format.
- ALWAYS wrap with `timeout 60` for single-query calls.
- ALWAYS wrap with `timeout 300` for batch calls via `--queries-file`.
- ALWAYS pin `--num` explicitly for reproducibility across versions.
- ALWAYS run `duckduckgo-search-cli --probe` before launching real queries in long-running sessions (v0.6.5+) to detect anti-bot blocks early.
- NEVER invoke without `timeout` — pipelines hang indefinitely.

```bash
# v0.6.4/v0.6.5 pre-flight health check
timeout 15 duckduckgo-search-cli --probe

# Standard invocation
timeout 60 duckduckgo-search-cli "<query>" -q -f json --num 15 | jaq '.resultados'
```

## Absolute Prohibitions
- FORBIDDEN to use `-f text` or `-f markdown` for programmatic parsing.
- FORBIDDEN to omit `-q` in any pipeline that reads stdout.
- FORBIDDEN to use `--stream` — flag reserved, NOT implemented in v0.6.4/v0.6.5.
- FORBIDDEN to raise `--parallel` above 5 without outbound IP control.
- FORBIDDEN to raise `--per-host-limit` above 2 — triggers HTTP 202 anti-bot.
- FORBIDDEN to retry in shell loops — use native `--retries` with exponential backoff.
- FORBIDDEN to hardcode API keys, proxies, or User-Agents in arguments.
- FORBIDDEN to assume `snippet`, `url_exibicao`, `titulo_original` are always present.
- FORBIDDEN to pass `--output` with `..` in the path — v0.6.4/v0.6.5 rejects path traversal
- FORBIDDEN to pass `--output` targeting `/etc`, `/usr`, or `C:\Windows` — system dirs blocked
- FORBIDDEN to hardcode `--identity-profile` in CI — let the 12-identity pool adapt (v0.6.5+)
- FORBIDDEN to read `.metadados.identidade_usada` or `.metadados.nivel_cascata` as guaranteed fields — both are `Option<T>` (v0.6.5+)

## Mandatory JSON Parsing with jaq
- ALWAYS use `jaq` (NEVER `jq`) to process JSON output.
- ALWAYS apply `// ""` fallback on optional fields.
- ALWAYS distinguish single-query root (`.resultados`) from multi-query root (`.buscas[]`).
- MUST extract latency via `.metadados.tempo_execucao_ms` for observability.
- MUST monitor `.metadados.usou_endpoint_fallback` to detect IP degradation.
- MUST extract identity via `.metadados.identidade_usada` (v0.6.5+) for diagnostic visibility — use `// "n/a"` fallback.
- MUST inspect `.metadados.nivel_cascata` (v0.6.5+) to detect anti-bot cascade exhaustion — use `// 0` fallback.

```bash
timeout 60 duckduckgo-search-cli "rust async runtime" -q -f json --num 15 \
  | jaq '.resultados[] | {
      posicao,
      titulo,
      url,
      snippet: (.snippet // ""),
      url_exibicao: (.url_exibicao // .url),
      identidade_usada: ((.metadados.identidade_usada // "n/a") | .),
      nivel_cascata: (.metadados.nivel_cascata // 0)
    }'
```

## Guaranteed vs Optional JSON Fields
- GUARANTEED non-null: `.query`, `.resultados[].posicao`, `.resultados[].titulo`, `.resultados[].url`.
- OPTIONAL `Option<String>`: `.resultados[].snippet`, `.resultados[].url_exibicao`, `.resultados[].titulo_original`.
- OPTIONAL `Option<String>` (v0.6.5+): `.metadados.identidade_usada` — identity tag `<family>-<platform>-<16hex>` that produced the response.
- OPTIONAL `Option<u32>` (v0.6.5+): `.metadados.nivel_cascata` — cascade level reached during the request (0..=4).
- METADATA always present: `.metadados.tempo_execucao_ms`, `.metadados.quantidade_resultados`, `.metadados.usou_endpoint_fallback`.
- CONDITIONAL on `--fetch-content`: `.resultados[].conteudo`, `.tamanho_conteudo`, `.metodo_extracao_conteudo`.

## Deterministic Exit Codes
- Exit 0: success — parse stdout with `jaq`.
- Exit 1: runtime error — read stderr and report to the user.
- Exit 2: CLI argument error — fix flags before retrying.
- Exit 3: anti-bot block HTTP 202 — v0.6.4+ cascade has ALREADY rotated up to 5 identities internally. Wait 300s, then switch to `--endpoint lite` and rotate proxy.
- Exit 4: global timeout hit — raise `--global-timeout` or reduce `--num`.
- Exit 5: zero results — reformulate the query before retrying.

```bash
timeout 60 duckduckgo-search-cli "query" -q -f json --num 15 > /tmp/r.json
EXIT=$?
case $EXIT in
  0) jaq '.resultados' /tmp/r.json ;;
  3) echo "anti-bot active, waiting 300s" && sleep 300 ;;
  5) echo "zero results, reformulate the query" ;;
  *) echo "error $EXIT" && exit $EXIT ;;
esac
```

## Mandatory Batching for Volume
- MUST use `--queries-file` for 3+ queries — reuses HTTP pool, UA rotation, rate limit.
- NEVER loop the CLI query-by-query in shell — pays 30-80ms of startup each time.
- MUST keep `--parallel 5` as ceiling to avoid saturating outbound IP.
- MUST write results with `--output` for large files — atomic write and chmod 644.

```bash
printf '%s\n' "tokio runtime" "rayon parallel" "axum middleware" > /tmp/q.txt
timeout 300 duckduckgo-search-cli --queries-file /tmp/q.txt \
  -q -f json --parallel 5 --num 15 \
  --output /tmp/results.json
```

## Content Extraction with --fetch-content
- MUST pass `--max-content-length` to cap memory when enabling `--fetch-content`.
- MUST gate access to `.conteudo` — without `--fetch-content`, the field is null.
- RECOMMENDED 4000-10000 bytes for LLM corpora — balance between context and noise.

```bash
timeout 120 duckduckgo-search-cli "rust async book" -q -f json \
  --num 10 --fetch-content --max-content-length 4000 \
  | jaq -r '.resultados[] | "# \(.titulo)\n\(.conteudo // "")\n---\n"'
```

## Endpoint and Degradation
- MUST use `--endpoint html` as default — rich metadata (snippet, display URL, canonical title).
- ONLY use `--endpoint lite` after confirmed exit code 3.
- NEVER start a pipeline with `lite` — it is a fallback strategy, not a starting point.

## Canonical Retries and Timeouts
- MUST use `--retries 2` as default — 3 only in unstable networks.
- MUST use `--timeout 20` per individual HTTP request.
- MUST use `--global-timeout 60` for single query, 300 for batch.
- NEVER raise `--retries` above 10 — guaranteed anti-bot trigger.

## Quick Reference Recipes
- URLs only: `| jaq -r '.resultados[].url'`.
- Titles only: `| jaq -r '.resultados[].titulo'`.
- Top N results: `| jaq '.resultados[:5]'`.
- Filter by domain: `| jaq '.resultados[] | select(.url | contains("github.com"))'`.
- Count: `| jaq '.quantidade_resultados'`.
- Latency: `| jaq '.metadados.tempo_execucao_ms'`.
- Identity used: `| jaq -r '.metadados.identidade_usada // "n/a"'` (v0.6.5+)
- Cascade level: `| jaq '.metadados.nivel_cascata // 0'` (v0.6.5+)
- Probe health (v0.6.4+): `timeout 15 duckduckgo-search-cli --probe`.
- Long crawl with circuit breaker (v0.6.5+): combine `--queries-file` with `--parallel 5 --retries 2 --global-timeout 580`.
- Cross-platform install (v0.7.3+): `cargo install duckduckgo-search-cli --version 0.7.3 --force` works on Linux, macOS, and Windows.
- Pre-flight CAPTCHA check (v0.7.3+): `timeout 15 duckduckgo-search-cli --probe-deep -q -f json | jaq -e '.status == "ok"'` returns exit 0 only when no Cloudflare interstitial is present.
- Persistent session with cookie jar (v0.7.3+): cookies are auto-persisted to XDG `cookies.json` with Unix mode `0o600`; pass `--cookies-path <PATH>` to redirect to an encrypted volume.
- Bypass warm-up (v0.7.3+): add `--no-warmup` to skip the `GET https://duckduckgo.com/` GET that populates session cookies.
- Disable cookie persistence (v0.7.3+): add `--no-cookie-persistence` to keep cookies in memory only and never write `cookies.json` to disk.
- Allow `html` → `lite` fallback (v0.7.3+): add `--allow-lite-fallback` to opt into automatic endpoint downgrade when CAPTCHA is detected.
- Progress bar to file (v0.6.5+): redirect stderr to a log file with `2> /tmp/progress.log` to keep stdout JSON clean.

## v0.6.4/v0.6.5 — Adaptive Anti-Bot Identity Pool (WS-26)

> **Note**: v0.6.4 was published originally in place of the planned v0.7.0; v0.6.5 (2026-06-05) added MP-26/WS-11/12/23/25/CI-01 to preserve the in-development feature set under a stable patch number. v0.7.0 (released 2026-06-07) supersedes both with the new `deep-research` subcommand and four new public modules. Zero breaking changes from v0.6.5.

### Mandatory Pre-Flight
- MUST run `duckduckgo-search-cli --probe` in CI before launching real queries — sends 1 minimal request, exits 0 if reachable, 1 if blocked.
- MUST inspect `.metadados.nivel_cascata` after exit 3 — the cascade has already rotated up to 5 identities. If `nivel_cascata == 4`, the IP itself is exhausted.


## v0.6.5 — Gaps Resolved (Dedicated Section)

v0.6.5 (released 2026-06-05) closes 7 gaps inherited from v0.6.4. The
sections below are MANDATORY reading for any agent invoking the CLI on
Windows or in long crawls.

### MP-26 — Windows HANDLE Type-Safety Fix

**Problem solved in v0.6.5**: v0.6.4 binary did not compile on Windows.
The `HANDLE` type changed from `isize` (windows-sys 0.52) to
`*mut c_void` (windows-sys 0.59), breaking 4 E0308 mismatched-type errors
in `src/platform.rs`.

**What this means for agents**:
- The same `cargo install duckduckgo-search-cli --version 0.7.3 --force`
  command now works on Linux, macOS AND Windows.
- The Windows binary uses the `INVALID_HANDLE_VALUE` sentinel from
  `windows_sys::Win32::Foundation` (NOT a magic `usize::MAX` comparison).
- The `unsafe` block has full SAFETY documentation describing nullness
  and sentinel checks.
- Lints `improper_ctypes` and `improper_ctypes_definitions` are `deny`
  in `Cargo.toml` — future FFI type drift is blocked at compile time.

**Agent recipe — Verify Windows install**:
```bash
# After cargo install on Windows (PowerShell 5.1+ or 7+)
duckduckgo-search-cli --version
# Expected: duckduckgo-search-cli 0.7.3
duckduckgo-search-cli --help
# Expected: full help text in stderr, exit 0
```

### WS-12 — Per-Host Circuit Breaker

**Problem solved in v0.6.5**: Long crawls (>50 pages) used to hang
retrying failed hosts. After 3 consecutive failures on a single host,
the crawl would loop forever consuming the entire `--global-timeout`.

**What this means for agents**:
- The CLI opens a circuit breaker per host after 3 consecutive failures.
- The breaker stays open for 30 seconds — requests to that host are
  short-circuited without consuming network resources.
- A single success resets the failure counter.
- The half-open state is reachable after the cooldown window.
- Each CLI invocation has a fresh breaker (no shared state between
  invocations).

**Agent recipe — Long crawl with circuit breaker**:
```bash
# 100 pages, 5 in parallel, with automatic circuit breaker
timeout 600 duckduckgo-search-cli \
  --queries-file /tmp/100-queries.txt \
  -q -f json --parallel 5 --per-host-limit 1 \
  --fetch-content --max-content-length 10000 \
  --retries 2 --timeout 30 \
  --global-timeout 580 > /tmp/long-crawl.json

# If a host fails 3x, requests to it are short-circuited for 30s.
# Other hosts continue to be scraped in parallel.
# Wall time reduced from "stuck retrying" to "moves on".
```

**Interaction with --parallel**:
- The circuit breaker is per-host, independent of `--parallel`.
- `--parallel 5` means 5 concurrent requests across distinct hosts.
- If 3 of those 5 fail on the same host, that host enters cooldown.
- Remaining 2 hosts continue normally.
- After 30s, the cooled host is re-evaluated (half-open state).

### WS-25 — indicatif ProgressBar for Long Crawls

**Problem solved in v0.6.5**: Long crawls (>10 URLs with
`--fetch-content`) gave no visual feedback. Users could not tell
whether the crawl was progressing or hung.

**What this means for agents**:
- The CLI displays a progress bar to stderr for any crawl with
  `--fetch-content` and >5 URLs.
- The bar format is
  `[{elapsed_precise}] {bar:40.cyan/blue} {pos:>4}/{len:4} {msg}`.
- The bar advances per task completion.
- The bar is cleared on finish (`finish_and_clear`) so it does not
  pollute downstream stderr consumers.
- The bar is NEVER written to stdout — JSON output stays clean.

**Agent recipe — Long crawl with progress visibility**:
```bash
# stderr shows the progress bar; stdout shows the JSON
timeout 300 duckduckgo-search-cli \
  --queries-file /tmp/50-queries.txt \
  -q -f json --fetch-content --max-content-length 5000 \
  --parallel 3 --global-timeout 280 \
  --output /tmp/results.json 2> /tmp/progress.log
# /tmp/results.json contains the JSON payload
# /tmp/progress.log contains the progress bar events
```

### WS-11 — Property-Based Tests for HTML Parser

**Problem solved in v0.6.5**: v0.6.3 → v0.6.4 migration broke the HTML
parser for empty inputs and malformed HTML. The v0.6.4 release had no
regression test coverage for parser invariants.

**What this means for agents**:
- 5 property tests in `src/extraction.rs` validate the parser never
  panics on malformed HTML, returns empty `Vec` for empty inputs,
  emits dense 1-based positions, normalizes URLs to absolute paths,
  and is deterministic.
- Agents can trust that malformed HTML from the wild does not crash
  the CLI.

### WS-23 — Retry-After Header Respected

**Problem solved in v0.6.5**: HTTP 429 responses with `Retry-After`
header were not honored — the CLI would retry immediately, triggering
anti-bot cascade.

**What this means for agents**:
- The CLI respects the `Retry-After` header in seconds.
- A wiremock test in `tests/integration_wiremock.rs` validates the
  backoff delay is at least `Retry-After` seconds, with 500ms slack
  for CI scheduler overhead.
- Agents do not need to implement their own `Retry-After` handling.

### CI-01 — 6 Latent Clippy Errors Fixed

**Problem solved in v0.6.5**: v0.6.4 was published with 6 clippy errors
that failed CI in all 3 SOs (Linux, macOS, Windows). The errors were:
- `clippy::doc_markdown` on `PowerShell`, `rules_rust.md`, `TempDir`
- `clippy::needless_return` in browser.rs:149
- `missing_debug_implementations` on `ChromeBrowser`
- `missing_debug_implementations` on `CircuitBreakerMap`

**What this means for agents**:
- `cargo clippy --all-targets --all-features -- -D warnings` passes.
- CI matrix returns success in all 3 SOs.
- 333 tests pass (243 lib + 90 integration + 6 doc tests) as of v0.6.5. v0.7.3 ships 391 tests (292 lib + 99 integration + 0 doc).
- Lints `improper_ctypes`, `missing_safety_doc`, and
  `unsafe_op_in_unsafe_fn` are now `deny` to prevent regressions.

### New CLI Flags (v0.6.4+, preserved in v0.6.5)
- `--probe` — pre-flight health check, 1 minimal request, JSON report.
- `--identity-profile <name>` — pin a specific identity from the 12-identity pool. Default `auto` rotates adaptively. Valid names: `auto`, `chrome-win`, `chrome-mac`, `chrome-linux`, `edge-win`, `firefox-linux`, `safari-mac`.
- `--seed <u64>` — deterministic seed for UA selection AND identity pool rotation. Use for reproducible debugging.

### Cascade Strategy (5 Levels)

```
Level 0 — Same identity (no rotation)
  ↓ (HTTP 202/403/429)
Level 1 — Same family, different platform
  ↓ (still blocked)
Level 2 — Different family, same platform
  ↓ (still blocked)
Level 3 — Different family and platform + endpoint downgraded to lite
  ↓ (still blocked)
Level 4 — Random identity (caller should sleep 30-60s before retrying)
  ↓ (still blocked)
FAILURE — Report with specific cause + recommended retry_after_seconds
```

### v0.6.4+ Anti-Bot Recipes (preserved in v0.6.5)
```bash
# Pre-flight health check before real queries
timeout 15 duckduckgo-search-cli --probe && \
  timeout 30 duckduckgo-search-cli "query" -q -f json --num 15

# Pin a specific identity for reproducible tests
timeout 30 duckduckgo-search-cli "query" -q -f json --num 15 --identity-profile chrome-linux

# Diagnose which identity produced a response
timeout 30 duckduckgo-search-cli "query" -q -f json --num 15 | \
  jaq -r '.metadados.identidade_usada // "n/a"'

# Detect cascade exhaustion in CI logs
timeout 30 duckduckgo-search-cli "query" -q -f json --num 15 | \
  jaq '.metadados.nivel_cascata // 0'  # if 4, rotate proxy or wait
```

### Troubleshooting Table by Cascade Level
| `nivel_cascata` | Meaning | Recommended Agent Action |
|---|---|---|
| 0 | First attempt succeeded or no rotation needed | None |
| 1 | First rotation (same family, different platform) succeeded | None |
| 2 | Second rotation (different family, same platform) succeeded | None |
| 3 | Third rotation (different family + platform + lite endpoint) succeeded | Note endpoint was downgraded — investigate why |
| 4 | Fourth rotation (random identity) succeeded or pool exhausted | If succeeded, log identity used. If exhausted, rotate proxy or wait 300s |
| absent | Cascade was not activated (default behavior in v0.6.4/v0.6.5) | None |

## Post-Invocation Validation
- ALWAYS check exit code before parsing stdout.
- ALWAYS inspect `.metadados.usou_endpoint_fallback` and log if `true`.
- ALWAYS confirm `.quantidade_resultados` greater than zero before acting on data.
- NEVER hallucinate missing content — if a field came back null, report absence to the user.

## Memory Integration
- MUST cite the exact URL as source when using a fact from this skill.
- MUST prefer results with low `posicao` (DuckDuckGo ranking) as primary sources.
- NEVER combine facts from multiple results without attributing each to its URL.

## Exit Code Routing
- MUST check exit code BEFORE parsing stdout
- Exit 0: parse `.resultados[]` normally
- Exit 1: runtime error — read stderr, retry with `-v`
- Exit 2: config error — run `init-config --force`
- Exit 3: anti-bot block — back off 300s, switch `--endpoint lite`
- Exit 4: global timeout — raise `--global-timeout`
- Exit 5: zero results — refine query, try different `--lang`
- In pipes: check `${PIPESTATUS[0]}` to capture CLI exit code

## Circuit Breaker Troubleshooting (v0.6.5+, WS-12)

The per-host circuit breaker in v0.6.5 does NOT emit its own exit code
(it shares exit 3 with anti-bot block). Diagnose via execution time
and partial result count:

| Symptom | Diagnosis | Agent Action |
|---|---|---|
| Wall time >> expected for --num count | One or more hosts in cooldown | Reduce `--parallel`, increase `--global-timeout`, or run in 2 invocations |
| Results count < queries count - 1 | At least one host was short-circuited | Inspect the results: missing host pattern means cooldown hit. Re-run after 30s |
| Stderr shows ProgressBar stuck at one position | Circuit breaker open for the current host | Wait 30s, or abort with Ctrl-C and resume with remaining queries |
| Multiple hosts returning HTTP 429 | Per-host cascade not just per-IP | Lower `--parallel` to 2, raise `--retries` to 1 |

## Golden Rule
- When in doubt between hallucinating and invoking the CLI, ALWAYS invoke the CLI.
- Cost of one invocation is 60-300ms. Cost of hallucination is rework and loss of trust.
- ALWAYS prefer verified data with URL over plausible assumption without source.


## Security Guarantees (v0.6.0 + v0.6.4 + v0.6.5)

### Path and Credential Safety (v0.6.0)
- `--output` validates paths BEFORE writing — `..` and system directories rejected automatically
- Proxy credentials in `--proxy` URLs NEVER appear in error messages or stderr
- Credential masking transforms `http://user:pass@host` into `http://us***@host` in all error output
- Agents generate dynamic filenames without manual path validation — the CLI rejects unsafe paths
- SIGPIPE restored on Unix — pipes to `jaq`, `head`, `wc` terminate cleanly without EPIPE errors
- BrokenPipe detected in error chain — returns exit 0 instead of propagating as exit 1
- Typed errors via `ErroCliDdg` enum — 11 variants with deterministic `exit_code()` mapping

### Anti-Blocking (v0.6.0 + v0.6.4)
- v0.6.0: `BrowserProfile` injects per-browser `Sec-Fetch-*` headers and Client Hints — NEVER add duplicate headers
- v0.6.0: HTTP 202 anomaly detection with exponential backoff runs automatically — trust exit code 3, do not retry in shell
- v0.6.0: Silent-block detection — responses under 5 KB are treated as blocks, not successes
- v0.6.4: 12-identity adaptive anti-bot pool (WS-26) — 4 browser families × 3 platforms with 5-level cascade rotation
- v0.6.4: `--probe` for pre-flight health checks in CI before launching real queries
- v0.6.4: `--identity-profile` and `--seed` give deterministic control over the adaptive pool
- v0.6.4: `metadados.identidade_usada` and `metadados.nivel_cascata` provide diagnostic visibility — use `// "n/a"` and `// 0` fallbacks respectively


## Workflow
- Step 1 — invoke the search: `duckduckgo-search-cli -f json -n 10 "query"`
- Step 2 — capture the exit code: check `$?` immediately after the command
- Step 3 — parse JSON results with jaq: `jaq -r '.resultados[] | .titulo + " " + .url'`
- Step 4 — filter relevant fields: `jaq '.resultados[] | {title: .titulo, url: .url, snippet: .snippet}'`
- Step 5 — return structured results to the LLM as context for downstream reasoning


## v0.7.0 — Deep Research Subcommand

For multi-hop research questions, the `deep-research` subcommand fans out one query into up to 12 sub-queries, aggregates the results, and optionally synthesises a Markdown report.

```bash
# 1. Default heuristic fan-out (5 sub-queries, RRF aggregation, no synthesis).
timeout 60 duckduckgo-search-cli -q -f json deep-research "best rust http client 2026" \
  | jaq '.resultados[] | {titulo, url, score}'

# 2. Markdown report with a token budget.
timeout 120 duckduckgo-search-cli -q -f json deep-research "tokio vs async-std 2026" \
  --synthesize --synth-format markdown --budget-tokens 1500 \
  | jaq -r '.sintese'

# 3. Manual sub-queries from a file (`# comments` and blank lines ignored).
cat > /tmp/qs.txt <<EOF
# Overview
what is tokio runtime 2026
# Comparison
tokio vs async-std
EOF
timeout 60 duckduckgo-search-cli -q -f json deep-research "tokio 2026" \
  --sub-queries-file /tmp/qs.txt --aggregate dedupe-by-url
```

### Deep Research output schema (v0.7.0+)
- `.metadados.query_original` — the user's input
- `.metadados.sub_queries[]` — every generated sub-query with `texto`, `estrategia`, `status`, `elapsed_ms`
- `.metadados.total_resultados_unicos` — deduplicated count
- `.metadados.tempo_total_ms` — end-to-end latency
- `.resultados[].score` — normalised `[0.0, 1.0]`, higher is better
- `.resultados[].fontes[]` — sub-queries that produced the result (traceability)
- `.sintese` — present only when `--synthesize` is enabled

The subcommand inherits every global flag (`-q -f json`, `--num`, `--lang`, `--country`, `--parallel`, `--endpoint`, `--proxy`, `--retries`, `--global-timeout`, `--fetch-content`, `--max-content-length`) and adds:

- `--max-sub-queries N` — cap the fan-out (1..=12, default 5)
- `--sub-query-strategy` — `heuristic` (default) or `manual`
- `--sub-queries-file PATH` — required for `manual`; comments and blanks are ignored
- `--aggregate` — `rrf` (default, K=60) or `dedupe-by-url`
- `--synthesize` — produce a final report
- `--budget-tokens N` — cap the synthesis length (1 token ≈ 4 chars)
- `--synth-format` — `markdown` (default), `plain`, or `json`

### Heuristic Templates (5 — built-in fan-out)
The `--sub-query-strategy heuristic` (default) applies 5 canonical templates to the user query:
- `aspect` — explores distinct dimensions of the topic
- `comparison` — surfaces alternatives (skipped when query already contains `vs` or `or`)
- `timeline` — orders results by recency and evolution
- `opinion` — surfaces opinions, reviews, and experiences
- `cause` — surfaces causes, consequences, and roots

When the user query is detected as composite via `is_composite_query` (regex-backed, 6 signal kinds), redundant templates are suppressed. Result: the fan-out produces 1..=12 sub-queries (capped by `--max-sub-queries`).

### Pipeline Defaults
`run_deep_research` builds a default `Config` from global flags: `parallelism=5`, `retries=2`, `endpoint=Html`, `language=en`, `country=us`, `global_timeout=120s`. The pipeline inherits these defaults; the operator does NOT need to pass a full `CliArgs`.

### `--depth` Semantics
`--depth N` controls reflection rounds (0..=3, default 0). When `depth > 0`, the pipeline PLANS follow-up sub-queries based on the first pass but does NOT execute them in v0.7.0. Use `--depth 0` to enforce end-to-end execution.

### Cross-Reference: RRF (K=60)
`--aggregate rrf` uses Reciprocal Rank Fusion with K=60, the same K as `hybrid-search` in the GraphRAG skill. RRF score for a document = sum over sub-queries of `1 / (K + rank)`. Practically scores fall in `(0, 0.05]`. Documents appearing in multiple sub-queries are boosted.

### Exit Codes for `deep-research`
- Exit 0: success — `.metadados.sub_queries[]` has 1+ entries with `status="ok"`.
- Exit 1: runtime error — at least one sub-query failed; inspect `.metadados.sub_queries[].status="error"`.
- Exit 2: argument error — `--max-sub-queries` outside 1..=12, or `--sub-queries-file` missing for `manual` strategy.
- Exit 3: anti-bot block during fan-out (per-host cascade has rotated up to 5 identities).
- Exit 4: global timeout hit before all sub-queries completed.
- Exit 5: zero aggregated results — reformulate the query.

### Cancel Safety
The fan-out loop in `run_deep_research` is cancel-safe. SIGINT or `--global-timeout` triggers `CancellationToken::cancel()`. Each in-flight sub-query gets a `child_token`, the `JoinSet` is aborted, and partial results from completed sub-queries are flushed to stdout. Already-fetched results are not discarded; the JSON contains `metadados.sub_queries[].status="cancelled"` for interrupted ones.

### Plain and JSON Synthesis Examples
```bash
# Plain-text synthesis (no Markdown markup, useful for log files)
timeout 120 duckduckgo-search-cli -q -f json deep-research "rust async 2026" \
  --synthesize --synth-format plain --budget-tokens 800 \
  | jaq -r '.sintese'

# JSON synthesis (structured evidence array, no prose)
timeout 120 duckduckgo-search-cli -q -f json deep-research "rust async 2026" \
  --synthesize --synth-format json --budget-tokens 1200 \
  | jaq '.sintese.evidencias[] | {titulo, url, score}'

# Manual sub-queries with dedupe-by-url (deterministic order)
timeout 60 duckduckgo-search-cli -q -f json deep-research "tokio" \
  --sub-queries-file /tmp/qs.txt --aggregate dedupe-by-url --max-sub-queries 12
```


## v0.7.1 — Maintenance Window (2026-06-06)

Patch release focused on codebase hygiene. Zero breaking changes and zero
agent-facing behavior changes. Agents can safely upgrade from v0.7.0 to
v0.7.1 without modifying pipelines.

What changed (transparent to agents):
- Dependency updates and minor refactors
- CI tooling improvements
- Documentation consistency fixes


## v0.7.2 — RUSTSEC-2026-0009 + rand 0.10 (2026-06-07)

Patch release closing two latent supply-chain and trait drift issues. Zero
breaking changes for agents but the underlying `time 0.3.47` pin and the
`rand 0.10` trait extension shift matter to maintainers. MSRV bumped
from 1.85 to 1.88 in this release.

- `time = "0.3.47"` pinned as a direct dependency to override
  `time 0.3.40` (RUSTSEC-2026-0009 — stack exhaustion DoS) which arrived
  transitively via `cookie_store 0.22.0` and `reqwest 0.12.28`.
- `rand 0.10.1` reorganized `random_range`, `random_bool`, and `random`
  from the `Rng` trait to the `RngExt` extension trait. `use rand::Rng;`
  was replaced with `use rand::RngExt;` in `src/identity.rs`,
  `src/parallel.rs`, and `src/search.rs`.
- MSRV is now 1.88 — agents building from source need a toolchain
  that satisfies this minimum.


## v0.7.3 — Session + Probe-Deep + BoringSSL (GAP-WS-27 fix)

> **Mandatory headline (v0.7.3)**: The TLS stack is `wreq 6.0.0-rc.29` with
> BoringSSL statically linked. `reqwest` and `rustls-tls` are REMOVED from
> the dependency tree. The pre-built binary from `cargo install
> duckduckgo-search-cli --version 0.7.3` requires no extra build deps on
> the operator machine. The build deps `cmake`, `perl`, `pkg-config`, and
> `libclang-dev` only matter when compiling from source on Linux.

### MANDATORY — Recognize the New Flags
- `--probe-deep` — runs a real search query and reports `status: "ok"` or `status: "captcha"`. Use this in CI gates for macOS runners to detect Cloudflare Bot Management interstitials before launching expensive pipelines.
- `--no-warmup` — skip the `GET https://duckduckgo.com/` warm-up that populates session cookies.
- `--no-cookie-persistence` — keep cookies in memory only; never write `cookies.json` to disk.
- `--cookies-path <PATH>` — override the default XDG cookie jar path. Use this to point at an encrypted volume.
- `--allow-lite-fallback` — opt-in to automatic fallback from `html` to `lite` endpoint when CAPTCHA is detected. Off by default.

### MANDATORY — Build Prerequisites for Source Builds (v0.7.3+)
- Compiling from source on Linux now requires `cmake`, `perl`, `pkg-config`, and `libclang-dev`. Pre-built binaries from `cargo install` are unaffected. This requirement is the trade-off for switching the TLS stack from `rustls` to BoringSSL (statically linked by `wreq 6.0.0-rc.29`), which produces a JA4_o fingerprint identical to Chrome/Safari and closes the GAP-WS-27 macOS CAPTCHA.

### MANDATORY — Treat the Cookie Jar as a Credential
- The `session` feature persists DuckDuckGo session cookies to `~/.config/duckduckgo-search-cli/cookies.json` (Linux), `%APPDATA%\duckduckgo-search-cli\cookies.json` (Windows), or `~/Library/Application Support/duckduckgo-search-cli/cookies.json` (macOS) with Unix permissions `0o600`. Read the file with the same care you would read an API key.

### MANDATORY — Probe-Deep in CI Gates
```bash
# Pre-flight CAPTCHA check for macOS runners
timeout 30 duckduckgo-search-cli --probe-deep -q -f json | jaq -e '.status == "ok"'
```

If the probe reports `status: "captcha"`, the operator should:
1. Wait 300+ seconds before retry (Cloudflare rate limit)
2. Switch to `--endpoint lite` manually
3. Add `--allow-lite-fallback` for automatic fallback
4. Rotate proxy via `--proxy socks5://127.0.0.1:9050`

### MANDATORY — Probe-Deep JSON Contract
- `.status` — `ok` (no interstitial) or `captcha` (Cloudflare challenge detected)
- `.endpoint` — the endpoint hit during the probe (`html`)
- `.http_status` — the HTTP response status (202 in v0.7.3 probe)
- `.latency_ms` — wall-clock latency of the probe search
- `.cascade_level` — anti-bot cascade level reached (0..=4)
- `.cascata_motivo` — `none` if clean, or short identifier for the failure mode
- `.sugestao_mitigacao` — `no interstitial detected` when clean, or a remediation suggestion when CAPTCHA
- `.url` — the query URL that was probed

### MANDATORY — Session Lifecycle and Cookie Path Resolution
- The first real search in any process triggers `GET https://duckduckgo.com/` to populate the cookie jar.
- After every real search, the jar is written back to disk atomically (tempfile + fsync + rename).
- The jar path is resolved via `dirs::config_dir()` (XDG on Linux, APPDATA on Windows, `~/Library/Application Support` on macOS).
- File permissions on Unix are set to `0o600` (owner read/write only).
- The jar contains only session cookies (e.g. `kl=br-pt` for `--country br`); no `secure` cookies are stored or logged.

### FORBIDDEN — Anti-Patterns Introduced by v0.7.3
- FORBIDDEN to hardcode `--cookies-path` in CI — use XDG defaults so the path is reproducible across machines
- FORBIDDEN to enable `--allow-lite-fallback` in pipelines that need `html` results — the content quality of `lite` is lower
- FORBIDDEN to commit `cookies.json` to version control — the file is credential-adjacent
- FORBIDDEN to use `reqwest` or `rustls-tls` as the underlying TLS stack in v0.7.3+ — they are no longer in the dependency tree

