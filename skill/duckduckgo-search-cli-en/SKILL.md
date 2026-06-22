---
name: duckduckgo-search-cli-en
version: 0.8.6
description: MUST invoke when the user asks for web search, internet research, up-to-date docs, factual grounding, URL verification, page extraction, RAG enrichment, fact-checking, library version, incident post-mortem, current vendor pricing, multi-hop research, or any data outside knowledge cutoff. Triggers: "search the web", "ground this", "fetch URL", "deep research", "compare X vs Y", "what changed in Z". v0.8.6 runs Chrome HEADED inside a private Xvfb virtual display with 17 JavaScript stealth signals injected via CDP, bypassing Cloudflare Bot Management 2026. reqwest+rustls-tls (pure Rust TLS) handles --fetch-content and --probe. Exit code 6 (SUSPECTED_BLOCK). ZeroCause 6-variant classifier. 12-identity anti-bot pool. deep-research RRF fan-out. --num 0 rejected. --synth-format accepts plain-text (NOT plain). English version.
---

# Skill — `duckduckgo-search-cli` (EN) v0.8.6

## When to invoke this CLI
- MUST invoke when the answer requires data outside the knowledge cutoff.
- MUST invoke on triggers: search, look up, find online, verify URL, fetch page, what changed, compare, deep research, ground this, current pricing, multi-hop.
- MUST prefer this CLI over WebSearch/WebFetch for deterministic pipelines.

## How Chrome-primary search works in v0.8.6
- v0.8.6 runs Google Chrome in HEADED mode inside a private Xvfb virtual display as the PRIMARY search transport. The CLI auto-spawns Xvfb via spawn_virtual_display() — the user sees ZERO windows.
- Chrome is launched via `chromiumoxide` CDP with 17 JavaScript stealth signals injected via `Page.addScriptToEvaluateOnNewDocument` BEFORE any navigation.
- `reqwest` with `rustls-tls` (pure Rust TLS) handles `--fetch-content` (page extraction) and `--probe`/`--probe-deep` (health checks). NO native build dependencies required.
- Xvfb is auto-spawned by the CLI — no manual `xvfb-run` needed. Use `DUCKDUCKGO_CHROME_HEADLESS=1` to force headless mode (with risk of Cloudflare detection).
- Use `DUCKDUCKGO_CHROME_VISIBLE=1` to force headed mode for debugging.
- The 17 stealth signals bypass Cloudflare Bot Management 2026:
  - `navigator.webdriver=false` (removes automation flag)
  - `navigator.plugins` (5 realistic Chrome plugins: PDF Plugin, PDF Viewer, Native Client)
  - `navigator.languages` (`['en-US','en']`)
  - `window.chrome` object (full runtime/app/loadTimes/csi emulation)
  - `navigator.connection` (rtt:50, downlink:10, effectiveType:'4g')
  - `navigator.maxTouchPoints` (0 for desktop)
  - `window.outerHeight/outerWidth` (realistic offset from innerHeight/innerWidth)
  - `navigator.hardwareConcurrency` (8)
  - `navigator.deviceMemory` (8)
  - `screen.colorDepth` (24)
  - `Notification.permission` (prompt-aware)
  - `navigator.permissions.query` (notifications handler)
  - `WebGLRenderingContext.getParameter` (ANGLE NVIDIA GeForce GTX 1650 spoofing)
  - `HTMLCanvasElement.toDataURL` (per-session canvas noise injection)
  - `AudioBuffer.getChannelData` (micro-noise audio fingerprint)
  - iframe `contentWindow.chrome` propagation
  - `navigator.vendor` ("Google Inc.")

```bash
# Chrome-primary search (default in v0.8.6 — headed inside Xvfb)
timeout 60 duckduckgo-search-cli "rust async runtime" -q -f json --num 15 | \
  jaq '{usou_chrome: .metadados.usou_chrome, tentou_chrome: .metadados.tentou_chrome}'
```

- OUTPUT FORMULA: `{"usou_chrome":true,"tentou_chrome":true}` when Chrome succeeded.
- ANTI-PATTERN: assuming reqwest is the transport — check `usou_chrome` to confirm.

## How to run a single query
- ALWAYS use this exact pattern for single queries:

```bash
timeout 60 duckduckgo-search-cli "<query>" -q -f json --num 15 | jaq '.resultados'
```

- OUTPUT FORMULA: `array<{posicao:int, titulo:string, url:string, snippet:string?, metadados:{tempo_execucao_ms:int, quantidade_resultados:int, identidade_usada:string?, nivel_cascata:u8?, usou_endpoint_fallback:bool, endpoint_usado:"html"|"lite", pre_flight_disparado:bool, usou_chrome:bool, tentou_chrome:bool}}`
- ANTI-PATTERN: invoking without `timeout` — pipeline hangs indefinitely.
- ANTI-PATTERN: `--num 0` — rejected by clap with exit 2 since v0.8.6 (GAP-WS-067). Minimum is 1.

## How to detect CAPTCHA before paying for a request
- MUST run before any non-trivial query when on shared IPs, corporate proxies, or after observed exit 3:

```bash
timeout 15 duckduckgo-search-cli --probe-deep -q -f json | jaq -e '.status == "ok"'
```

- OUTPUT FORMULA: `{"status":"ok"|"captcha","cascata_motivo":"none"|"cloudflare_anomaly_modal"|..., "sugestao_mitigacao":"..."}`
- IF exit non-zero, MUST sleep 300s before retry (Cloudflare rate limit).
- IF `status == "captcha"`, MUST add `--allow-lite-fallback` to next invocation.

## How to prevent silent zero-result failures on shared IPs (GAP-WS-58)
- MUST add `--pre-flight` to any query when exit 3 risk is non-zero (shared IP, corporate net, sequential batch, post-retry):

```bash
timeout 60 duckduckgo-search-cli --pre-flight "consulting firms" -q -f json | \
  jaq -r '.metadados.endpoint_usado + " fired=" + (.metadados.pre_flight_disparado|tostring)'
```

- OUTPUT FORMULA: `"lite fired=true"` when ghost-block detected, otherwise `"html fired=true"` (probe always runs first when `--pre-flight` active).
- WITHOUT `--pre-flight`, ghost-block returns HTTP 200 with empty body → `quantidade_resultados:0` with exit 0 (silent failure).
- ANTI-PATTERN: ignoring `quantidade_resultados:0` as success — always inspect `.metadados.pre_flight_disparado` and `.metadados.endpoint_usado` before declaring success.

## How to opt into automatic HTML to Lite downgrade on CAPTCHA (GAP-WS-59)
- MUST add `--allow-lite-fallback` when CAPTCHA detected but Lite endpoint not yet attempted:

```bash
timeout 60 duckduckgo-search-cli --allow-lite-fallback "consulting firms" -q -f json | \
  jaq -r '.metadados.endpoint_usado'
```

- OUTPUT FORMULA: `"lite"` when fallback triggered, `"html"` otherwise.
- The flag must come BEFORE the subcommand when used with `deep-research`:

```bash
timeout 120 duckduckgo-search-cli --allow-lite-fallback -q -f json deep-research "test" --max-sub-queries 3
```

- ANTI-PATTERN: passing `--allow-lite-fallback` AFTER `deep-research` subcommand — Clap rejects with exit 2.

## How to correlate a failure to a specific browser identity (GAP-WS-60 + GAP-AUD-001)
- ALWAYS log `identidade_usada` when investigating failures or audit trails:

```bash
timeout 30 duckduckgo-search-cli --identity-profile chrome-linux --global-timeout 1 "x" -q -f json 2>/dev/null | \
  jaq -r '.metadados | "ua=\(.user_agent[0:50]) id=\(.identidade_usada // "n/a") cascade=\(.nivel_cascata // 0)"'
```

- EXPECTED OUTPUT: `"ua=Mozilla/5.0 (X11; Linux x86_64) ... id=chrome-linux-33333333cccc0003 cascade=0"`
- FORMAT FORMULA: `<family>-<platform>-<16hex>` where `16hex` is first 16 chars of seed-derived hash.
- ANTI-PATTERN: assuming `identidade_usada` is guaranteed non-null — it is `Option<String>` (always apply `// "n/a"`).

## How to diagnose zero-result causes with causa_zero (v0.8.0)
- v0.8.0 classifies every zero-result response into one of 6 causal variants via `pipeline::classify_zero_result`.
- The `causa_zero` field appears in `.metadados.causa_zero` when `quantidade_resultados == 0`.
- Variants (kebab-case in JSON):
  - `legitimo` — query genuinely has no results in DDG index.
  - `filtro-silencioso` — DDG dropped the query silently without detectable interstitial.
  - `ghost-block` — Cloudflare served HTTP 200 with sub-4KB body without literal markers.
  - `anti-bot` — explicit anti-bot (HTTP 202, persistent 403, CF/DDG interstitial).
  - `resposta-invalida` — invalid or truncated response (empty body, malformed JSON, proxy intercept).
  - `zero-resultados-suspeito` — decompressed body in suspect range (5-15KB) without result-page signals and without interstitial markers; indicates probable upstream stealth block.
- When `causa_zero` is non-`legitimo` AND `DUCKDUCKGO_ZERO_CAUSE_STRICT` is not `false`, the CLI emits exit code 6 (`SUSPECTED_BLOCK`) instead of exit code 5.
- `sugestao_proxima_acao` field provides an actionable PT-BR suggestion per variant.

```bash
# Diagnose why zero results occurred
timeout 60 duckduckgo-search-cli "blocked query" -q -f json --num 15 > /tmp/r.json
EXIT=$?
if [ "$EXIT" -eq 6 ]; then
  jaq -r '.metadados | "causa=\(.causa_zero // "n/a") sugestao=\(.sugestao_proxima_acao // "n/a")"' /tmp/r.json
fi
```

- For multi-query, inspect `.causa_zero_histogram` for aggregate counts across sub-queries:

```bash
timeout 300 duckduckgo-search-cli --queries-file /tmp/q.txt -q -f json --output /tmp/r.json
jaq '.causa_zero_histogram' /tmp/r.json
```

- OUTPUT FORMULA: `{"anti-bot":2,"legitimo":3}` — BTreeMap sorted lexicographically.

## How to parse JSON output safely with jaq
- ALWAYS use `jaq` (NEVER `jq`) to process JSON output.
- ALWAYS apply `// ""` fallback on optional fields.
- ALWAYS distinguish roots: `.resultados[]` (single-query), `.buscas[]` (multi-query), `.metadados.sub_queries[]` (deep-research).

```bash
timeout 60 duckduckgo-search-cli "rust async runtime" -q -f json --num 15 \
  | jaq -r '.resultados[] | [.posicao, .titulo, .url, (.snippet // "")] | @tsv'
```

- OUTPUT FORMULA: TSV with `posicao<TAB>titulo<TAB>url<TAB>snippet` per line.

## Which JSON fields are guaranteed versus optional
- GUARANTEED non-null: `.query`, `.resultados[].posicao`, `.resultados[].titulo`, `.resultados[].url`, `.metadados.tempo_execucao_ms`, `.metadados.quantidade_resultados`, `.metadados.usou_endpoint_fallback`.
- OPTIONAL `Option<String>`: `.resultados[].snippet`, `.resultados[].url_exibicao`, `.resultados[].titulo_original`, `.metadados.identidade_usada`.
- OPTIONAL `Option<u32>`: `.metadados.nivel_cascata` (0..=4).
- CONDITIONAL on `--fetch-content`: `.resultados[].conteudo`, `.resultados[].tamanho_conteudo`, `.resultados[].metado_extracao_conteudo`.
- v0.7.10: `.metadados.pre_flight_disparado` (bool) and `.metadados.endpoint_usado` (`html` | `lite`).
- v0.8.0: `.metadados.causa_zero` — kebab-case enum: `legitimo` | `filtro-silencioso` | `ghost-block` | `anti-bot` | `resposta-invalida` | `zero-resultados-suspeito`.
- v0.8.0: `.metadados.sugestao_proxima_acao` — PT-BR actionable suggestion string when `causa_zero` is non-`legitimo`.
- v0.8.0: `.causa_zero_histogram` — `BTreeMap<String, u32>` aggregated across multi-query sub-queries.
- v0.8.0: `.metadados.usou_chrome` — `bool` — `true` when Chrome-primary search succeeded.
- v0.8.0: `.metadados.tentou_chrome` — `bool` — `true` when Chrome search was attempted.
- v0.8.0: `.metadados.bytes_brutos` — `Option<u64>` — raw HTTP response size in bytes before decompression.
- v0.8.0: `.metadados.bytes_descomprimidos` — `Option<u64>` — decompressed HTTP response size in bytes.
- v0.8.0: `.metadados.cascata_nivel_observado` — `Option<u32>` — cascade level observed from probe-deep.
- ANTI-PATTERN: omitting `//` fallback on `snippet` and `identidade_usada` — `jaq` exits non-zero on null.

## How to route by exit code in pipelines
- MUST capture exit code BEFORE parsing stdout.
- MUST use `${PIPESTATUS[0]}` when piped through `jaq`.

```bash
timeout 60 duckduckgo-search-cli "query" -q -f json --num 15 > /tmp/r.json
EXIT=$?
case $EXIT in
  0) jaq '.resultados' /tmp/r.json ;;
  3) echo "anti-bot: wait 300s, then --endpoint lite" && sleep 300 ;;
  4) echo "timeout: raise --global-timeout or reduce --num" ;;
  5) echo "zero results (legitimate): reformulate or change --lang" ;;
  6) echo "suspected block: inspect causa_zero" && jaq '.metadados.causa_zero' /tmp/r.json ;;
  *) echo "error $EXIT" >&2; exit $EXIT ;;
esac
```

- EXIT MAP: `0=success`, `1=runtime`, `2=arg-error`, `3=anti-bot`, `4=timeout`, `5=zero-results-legitimate`, `6=suspected-block (causa_zero != legitimo)`.

## How to batch 3 or more queries without paying per-call startup
- MUST use `--queries-file` for 3+ queries — reuses HTTP pool, UA rotation, rate limit:

```bash
printf '%s\n' "tokio runtime" "rayon parallel" "axum middleware" > /tmp/q.txt
timeout 300 duckduckgo-search-cli --queries-file /tmp/q.txt \
  -q -f json --parallel 5 --num 15 --global-timeout 280 \
  --output /tmp/results.json
```

- ANTI-PATTERN: looping CLI query-by-query in shell — pays 30-80ms startup each call.
- ANTI-PATTERN: `--parallel > 5` — saturates outbound IP and triggers anti-bot.
- ANTI-PATTERN: `--per-host-limit > 2` — triggers HTTP 202 anti-bot.

## How to extract page content for LLM context
- MUST pass `--max-content-length` when enabling `--fetch-content`:

```bash
timeout 120 duckduckgo-search-cli "rust async book" -q -f json \
  --num 10 --fetch-content --max-content-length 5000 \
  | jaq -r '.resultados[] | "# \(.titulo)\nURL: \(.url)\n\(.conteudo // "")\n---\n"'
```

- RECOMMENDED 4000-10000 bytes per page for LLM corpora.
- ANTI-PATTERN: using `--fetch-content` without `--max-content-length` — unbounded memory growth.

## How HTTP decompression works in v0.8.6
- `reqwest` with `rustls-tls` sends `accept-encoding: gzip, deflate` and auto-decompresses most responses via its built-in `gzip` and `deflate` features.
- Chrome responses bypass reqwest entirely (body arrives via CDP); `src/decompress.rs` handles edge cases where Chrome returns compressed bodies.
- `src/decompress.rs` dispatches gzip (`flate2::MultiGzDecoder`) and deflate (`flate2::ZlibDecoder`).
- Brotli decompression REMOVED in v0.8.6 — DuckDuckGo never serves brotli for HTML endpoints. Requesting `br` encoding returns `CliError::UnsupportedEncoding`.
- `DECOMPRESSION_MAX_OUTPUT = 32 MiB` protects against gzip bombs.
- Raw and decompressed byte counts are reported in `.metadados.bytes_brutos` and `.metadados.bytes_descomprimidos`.
- Without this decompression layer, interstitial detection (`body.contains("anomaly-modal")`) fails silently on compressed bytes — root cause of GAP-AUD-003.

## How to interpret the 5-level anti-bot cascade
```
Level 0 — Same identity, no rotation
Level 1 — Same family, different platform
Level 2 — Different family, same platform
Level 3 — Different family + platform + endpoint downgraded to lite
Level 4 — Random identity (caller sleeps 30-60s before retry)
FAILURE — Report with cause + retry_after_seconds
```
- IF `nivel_cascata == 4` observed, MUST rotate proxy or wait 300s before next invocation.

## How to run multi-hop research (ALWAYS use manual sub-queries)
- MUST generate 3-5 specific sub-queries yourself instead of relying on heuristic templates
- The default `--sub-query-strategy heuristic` appends generic suffixes ("main aspects components", "vs alternatives comparison") that produce low-quality results
- ALWAYS use `--sub-query-strategy manual --sub-queries-file` with LLM-generated questions
- `--synth-format` accepts `plain-text` (NOT `plain`) — the clap ValueEnum derives kebab-case from `PlainText` (GAP-WS-068)

```bash
# Step 1: generate specific sub-queries (the LLM writes these)
printf '%s\n' \
  "tokio async runtime architecture work stealing scheduler" \
  "async-std vs tokio benchmark performance comparison 2026" \
  "tokio spawn vs spawn_blocking when to use each" \
  "tokio runtime shutdown graceful timeout best practices" \
  "tokio channels mpsc watch broadcast differences" \
  > /tmp/sub-queries.txt

# Step 2: run deep-research with manual sub-queries
timeout 120 duckduckgo-search-cli -q -f json deep-research "tokio vs async-std 2026" \
  --sub-query-strategy manual --sub-queries-file /tmp/sub-queries.txt \
  --aggregate rrf \
  | jaq '.resultados[] | {titulo, url, score}'
```

- ANTI-PATTERN: using default heuristic strategy — produces generic, low-quality sub-queries
- ANTI-PATTERN: copying the user query verbatim as sub-queries — add specific angles
- ANTI-PATTERN: `--synth-format plain` — INVALID. Use `--synth-format plain-text` (GAP-WS-068)
- Each sub-query MUST target a distinct aspect: architecture, benchmarks, pricing, limitations, comparisons
- OUTPUT FORMULA: `.sintese` (Markdown), `.metadados.sub_queries[]` (per-subquery status), `.resultados[]` (RRF-aggregated)
- EXIT MAP: `0=success`, `1=any-sub-query-failed`, `2=arg-error`, `3=anti-bot-during-fanout`, `4=timeout`, `5=zero-aggregated`
- COMBINE with `--pre-flight` for blocked environments:

```bash
timeout 120 duckduckgo-search-cli --pre-flight -q -f json deep-research "rust async 2026" \
  --sub-query-strategy manual --sub-queries-file /tmp/sub-queries.txt --max-sub-queries 5
```

- Synthesis with plain-text format:

```bash
timeout 120 duckduckgo-search-cli -q -f json deep-research "rust async 2026" \
  --synthesize --synth-format plain-text --budget-tokens 800 \
  | jaq -r '.sintese'
```

## How to configure retries and timeouts without triggering anti-bot
- MUST use `--retries 2` (clamped `[1, 10]`, GAP-WS-57 v0.7.8 — flag is now honored).
- MUST use `--timeout 20` per individual HTTP request.
- MUST use `--global-timeout 60` (single) or `300` (batch).
- ANTI-PATTERN: `--retries > 10` — guaranteed anti-bot trigger.
- ANTI-PATTERN: shell retry loops — use native `--retries` with exponential backoff.

## How to discover and use every flag
- `--probe` — minimal health check (v0.6.4+).
- `--probe-deep` — real query CAPTCHA detector (v0.7.3+).
- `--pre-flight` — auto-route via probe-deep first (v0.7.10+, GAP-WS-58).
- `--allow-lite-fallback` — opt-in HTML→Lite downgrade (v0.7.3+, GAP-WS-59).
- `--identity-profile <name>` — pin identity (auto/chrome-win/chrome-mac/chrome-linux/edge-win/firefox-linux/safari-mac).
- `--seed <u64>` — deterministic UA + pool rotation.
- `--no-warmup` — skip cookie warm-up (v0.7.3+).
- `--no-cookie-persistence` — in-memory cookies only (v0.7.3+).
- `--cookies-path <PATH>` — redirect jar to encrypted volume.
- `-v` info / `-vv` debug / `-vvv` trace (additive, v0.7.8 GAP-WS-53).
- `--output <PATH>` — atomic write of full payload (rejected if `..` or `/etc`/`/usr`/`C:\Windows`).
- `--chrome-path <PATH>` — manual Chrome/Chromium binary path (bypasses auto-detection).
- `--num N` — number of results (minimum 1, `--num 0` rejected since v0.8.6 GAP-WS-067).
- `--synth-format` — `markdown` (default), `plain-text` (NOT `plain`), `json` (GAP-WS-068).

## How to format search results as LLM-ready context
- MUST pipe to `jaq` to extract only relevant fields:

```bash
# Top 5 titles + URLs as markdown list
timeout 60 duckduckgo-search-cli "query" -q -f json --num 5 \
  | jaq -r '.resultados[:5] | to_entries[] | "\(.value.posicao). [\(.value.titulo)](\(.value.url))"'
```

```bash
# Source citation block for downstream LLM
timeout 60 duckduckgo-search-cli "incident 2026-06" -q -f json --num 10 \
  | jaq -r '"Sources:\n" + (.resultados[] | "- \(.titulo) — \(.url)\n")'
```

## What you must never do
- FORBIDDEN `-f text` or `-f markdown` for programmatic parsing — use `-f json`.
- FORBIDDEN omit `-q` in pipelines — stderr tracing pollutes stdout.
- FORBIDDEN `--stream` — flag reserved, NOT implemented.
- FORBIDDEN `--parallel > 5` without outbound IP control.
- FORBIDDEN `--per-host-limit > 2` — triggers HTTP 202 anti-bot.
- FORBIDDEN shell retry loops — use native `--retries`.
- FORBIDDEN hardcode API keys, proxies, or User-Agents in arguments.
- FORBIDDEN hardcode `--identity-profile` in CI — let the 12-identity pool adapt.
- FORBIDDEN `--output` with `..` or system dirs (`/etc`, `/usr`, `C:\Windows`).
- FORBIDDEN treat `identidade_usada` or `nivel_cascata` as guaranteed — both are `Option<T>`.
- FORBIDDEN commit `cookies.json` — credential-adjacent file.
- FORBIDDEN ignore `quantidade_resultados:0` — may be ghost-block (use `--pre-flight` or inspect `causa_zero`).
- FORBIDDEN `--num 0` — rejected by clap since v0.8.6 (GAP-WS-067).
- FORBIDDEN `--synth-format plain` — use `plain-text` (GAP-WS-068).

## How to handle the cookie jar as a credential
- Cookie jar path (Linux/macOS/Windows): `~/.config/duckduckgo-search-cli/cookies.json` (Unix mode `0o600`).
- MUST NOT log or echo cookie contents.
- MUST NOT pass `--cookies-path` to unencrypted volumes in production.
- `--no-cookie-persistence` flag for ephemeral sessions.

## How to satisfy build prerequisites on Linux and Windows
- v0.8.6 uses `reqwest` with `rustls-tls` (pure Rust TLS) — ALL native build dependencies (cmake, nasm, perl, MSVC cl.exe) have been REMOVED.
- Linux build deps: Rust toolchain only. No cmake, no perl, no pkg-config, no libclang-dev.
- Linux Chrome runtime deps: Google Chrome or Chromium installed (auto-detected via `detect_chrome`).
- Linux runtime dep: Xvfb package (auto-spawned by CLI). Install: `sudo apt install xvfb` (Debian/Ubuntu), `sudo dnf install xorg-x11-server-Xvfb` (Fedora).
- macOS: Rust toolchain + Chrome. No Xvfb needed (native display used directly).
- Windows: Rust toolchain + Chrome. No Xvfb needed (native display used directly). NO cmake, NO nasm, NO MSVC cl.exe, NO Strawberry Perl required.
- `DDG_SKIP_NASM_CHECK`, `DDG_SKIP_CMAKE_CHECK`, `DDG_SKIP_MSVC_CHECK`, `DDG_SKIP_PERL_CHECK` env vars REMOVED — no build.rs preflights exist.
- `cargo install` ALWAYS compiles from source — crates.io ships NO pre-built binaries.
- Chrome feature is enabled via `cargo build --features chrome` (default in `cargo install`).

## How to install or upgrade to v0.8.6

```bash
cargo install duckduckgo-search-cli --version 0.8.6 --locked --force
```

## How to opt out of exit code 6 for BC with v0.7.x pipelines
- `DUCKDUCKGO_ZERO_CAUSE_STRICT=false` restores v0.7.x behavior: exit 5 for ALL zero-result cases.
- Accepted falsy values: `false`, `0`, `no`, `off`, empty string.
- When unset or any other value: strict mode is ON (default in v0.8.0) — exit 6 emitted for non-`legitimo` causes.
- The `causa_zero` field is STILL published in the JSON envelope even with opt-out active — only the exit code changes.

```bash
# Restore v0.7.x exit code behavior
export DUCKDUCKGO_ZERO_CAUSE_STRICT=false
timeout 60 duckduckgo-search-cli "blocked query" -q -f json --num 15
# Exit 5 even if causa_zero is "anti-bot"
```

## APPENDIX — Migration Notes (v0.8.5 → v0.8.6)
- TLS stack: `wreq` (BoringSSL) REPLACED by `reqwest` + `rustls-tls` (pure Rust TLS). wreq is NO LONGER in the dependency tree.
- ALL native build dependencies REMOVED: cmake, nasm, perl, MSVC cl.exe, Strawberry Perl are NO LONGER needed on any platform.
- `build.rs` preflights for NASM/CMake/MSVC/Perl REMOVED entirely.
- `DDG_SKIP_NASM_CHECK`, `DDG_SKIP_CMAKE_CHECK`, `DDG_SKIP_MSVC_CHECK`, `DDG_SKIP_PERL_CHECK` env vars REMOVED — no longer recognized.
- `cargo install duckduckgo-search-cli` now works on Windows WITHOUT any extra tools beyond Rust toolchain + Chrome.
- Brotli decompression REMOVED — DuckDuckGo never serves brotli for HTML endpoints. `src/decompress.rs` now returns `CliError::UnsupportedEncoding` for `br` encoding.
- HTTP decompression still handled via `flate2` (gzip, deflate) in `src/decompress.rs` for Chrome response edge cases.
- `--num 0` now REJECTED by clap with exit 2 (GAP-WS-067). Minimum value is 1.
- `--synth-format` accepts `plain-text` (NOT `plain`) — clap ValueEnum derives kebab-case from `PlainText` variant (GAP-WS-068).
- Chrome headed + Xvfb anti-bot evasion UNCHANGED from v0.8.5.
- 17 JavaScript stealth signals UNCHANGED from v0.8.5.
- Exit code 6 (SUSPECTED_BLOCK) UNCHANGED from v0.8.0.
- ZeroCause 6-variant classifier UNCHANGED from v0.8.0.
- 12-identity anti-bot pool UNCHANGED from v0.6.4.
- See CHANGELOG.md and README.md for full history.
