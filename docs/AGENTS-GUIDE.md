# Integrating duckduckgo-search-cli with AI Agents

Give your agent real-time web context with zero API keys, deterministic exit codes, and a frozen JSON schema built for machine parsing.


## Why AI Agents Use This Tool
### Measurable Gains
- Eliminates 3-5 HTTP round trips per search query
- Returns structured JSON — no HTML parsing required
- Exit codes enable deterministic flow control in any shell
- Zero side effects — read-only, stateless, idempotent per call
- Works in any shell-capable agent framework without dependencies
- Saves ~40 tokens per result compared to HTML scraping pipelines
- Reduces search latency 60-80% versus headless browser approaches
- Enables parallel multi-query without rate limit risk using `--per-host-limit`
- Processes 20 queries in under 30 seconds with `--parallel 3`
- No API key to rotate, no dashboard to monitor, no vendor lock-in


## Compatible Agents
### Compatibility Matrix
| Agent | Integration Method | Notes |
|-------|--------------------|-------|
| Claude Code | `Bash tool` subprocess | Native JSON piping with `jaq` |
| OpenAI Codex | `code_interpreter` subprocess | Works with `jaq` for JSON parsing |
| GPT-4o | `function_calling` wrapper | JSON as structured tool return value |
| Google Gemini | `code execution` subprocess | Grounding via `--fetch-content` flag |
| GitHub Copilot | `@workspace` terminal | Pipe results to context window |
| Cursor | Terminal execution | Declare in `.cursorrules` for auto-use |
| Codeium | CLI integration | Standard subprocess call pattern |
| Aider | `!shell` command via `/run` | Feed results directly into edit session |
| Devin | Shell agent subprocess | Direct subprocess, reads JSON output |
| SWE-agent | Shell tool | Standard stdin/stdout contract |
| AutoGPT | Shell command tool | JSON parsing supported natively |
| CrewAI | Custom tool wrapper | Define JSON schema as tool definition |
| LangChain | `BashTool` integration | Standard subprocess with exit code check |
| LlamaIndex | `SubprocessTool` | Structured JSON output ready to parse |
| Phind | Terminal direct execution | No wrapper needed |
| Continue.dev | Terminal integration | Register as custom slash-command |


## stdin/stdout Contract
### Input
- Input: command-line arguments only — no stdin required
- All configuration via flags; no interactive prompts in any path
- Stateless: each invocation is fully independent
- No files modified unless `-o`/`--output` is explicitly specified
- No network calls except to DuckDuckGo's search endpoint
### Output Format
- Stdout: structured payload (JSON/Markdown/text) — clean, no tracing noise
- Stderr: tracing logs only — use `-q` to silence completely in pipelines
- `-f json` is MANDATORY in all machine-readable pipelines
- `-q` is MANDATORY in all piped invocations
### Error Handling
- Exit code is ALWAYS meaningful — check BEFORE processing stdout
- Non-zero exit means stdout may be empty or partial — NEVER parse blindly
- Use `${PIPESTATUS[0]}` to detect upstream failure in `cmd | jaq` pipes


## JSON Output Structure
### Single Query
```json
{
  "query": "rust async runtime",
  "resultados": [
    {
      "posicao": 1,
      "titulo": "Result Title",
      "url": "https://example.com/page",
      "snippet": "Brief description from the search result...",
      "url_exibicao": "example.com › page",
      "titulo_original": "Result Title"
    }
  ],
  "quantidade_resultados": 10,
  "metadados": {
    "usou_proxy": false,
    "usou_endpoint_fallback": false,
    "user_agent": "Mozilla/5.0...",
    "tempo_execucao_ms": 1234
  }
}
```

- `.resultados[].titulo` — ALWAYS present when `resultados` is non-empty
- `.resultados[].url` — ALWAYS present when `resultados` is non-empty
- `.resultados[].posicao` — ALWAYS present when `resultados` is non-empty
- `.resultados[].snippet` — optional (`Option<String>`) — ALWAYS use `// ""` fallback
- `.resultados[].url_exibicao` — optional — ALWAYS use `// .url` fallback
- `.metadados.usou_endpoint_fallback` — `true` signals IP reputation degradation
- Content fields `.conteudo` and `.tamanho_conteudo` — absent without `--fetch-content`
### Multi-Query Mode
```json
{
  "quantidade_queries": 3,
  "buscas": [
    {
      "query": "first query",
      "resultados": [...],
      "metadados": {...}
    },
    {
      "query": "second query",
      "resultados": [...],
      "metadados": {...}
    }
  ]
}
```

- Multi-query root is `{ quantidade_queries, buscas: [...] }` — NEVER `.resultados` at root
- Single query root is `{ query, resultados, metadados }` — no `.buscas` key present
- ALWAYS distinguish root structure before accessing results

```bash
# single query
duckduckgo-search-cli -q -f json "one" | jaq '.resultados | length'
# multi-query
duckduckgo-search-cli -q -f json "one" "two" | jaq '.buscas[0].resultados | length'
```


## Exit Code Protocol
### Exit Code Reference
| Code | Meaning | Recommended Agent Action |
|------|---------|--------------------------|
| 0 | Success | Parse stdout JSON immediately |
| 1 | Runtime error | Read stderr; retry once with `-v` for diagnostics |
| 2 | Config error | Run `init-config --force`; check config path |
| 3 | Anti-bot block | Wait 300+ seconds; switch `--endpoint lite`; rotate proxy |
| 4 | Global timeout | Raise `--global-timeout`; reduce `--parallel` value |
| 5 | Zero results | Broaden query; try different `--lang` or `--country` |

```bash
run_ddg() {
  local query="$1"
  local outfile="$2"
  timeout 60 duckduckgo-search-cli -q -f json --num 15 "$query" > "$outfile"
  local ec=$?
  case $ec in
    0) return 0 ;;
    3) echo "BLOCKED: wait 300s and rotate proxy." >&2; return 3 ;;
    4) echo "TIMEOUT: raise --global-timeout." >&2; return 4 ;;
    5) echo "ZERO_RESULTS: rephrase query." >&2; return 5 ;;
    *) echo "ERROR($ec): check stderr." >&2; return "$ec" ;;
  esac
}
```


## Idempotency and Side Effects
- Every call is read-only — no files, no state, no network side effects beyond search
- Same query returns semantically equivalent results (web content changes over time)
- Safe to retry on transient failures: exit 1, exit 4 are safe to retry once
- NOT safe to retry exit 3 immediately — repeated retries deepen the block window
- Exit 3 requires 300+ second backoff before any retry attempt
- Exit 5 is not a failure — broaden or rephrase the query instead of retrying


## Payload Limits and Timeouts
### Recommended Limits
- `-n 10` for context windows under 8k tokens
- `-n 20` for context windows between 16k-32k tokens
- `--max-content-length 3000` per page for standard RAG pipelines
- `--max-content-length 8000` for deep context extraction use cases
- Reduce `--num` to 5 when `--fetch-content` is enabled (N× latency per result)
### Global Timeout Pattern
- `--global-timeout` MUST always be set 10 seconds below the outer `timeout` value
- Outer `timeout` terminates the process; `--global-timeout` lets the CLI exit cleanly
- NEVER set both values equal — CLI will not terminate gracefully

```bash
# outer 60s, global-timeout 50s — always leave 10s gap
timeout 60 duckduckgo-search-cli -q -f json --num 15 --global-timeout 50 "$QUERY"
```


## Language Control
### Per-Request Language
- `--lang pt-br` for Brazilian Portuguese results
- `--lang es` for Spanish results
- `--lang en-us` for United States English results
- Default: results ranked in the query's detected language
- Language affects result ranking and regional sources — not translation
- Combine `--lang` with `--country` for precise regional targeting


## Anti-bot Handling
### Detection and Recovery
- v0.6.0 ships per-browser `Sec-Fetch-*` headers and Client Hints automatically
- NEVER inject custom `Sec-Fetch-*` or `Accept-Language` headers manually
- Exit code 3 means the IP is soft-blocked — STOP retrying immediately
- Wait at least 300 seconds before any retry after exit 3
- Switch to `--endpoint lite` after the first exit 3 occurrence
- Use `HTTPS_PROXY` env var (NEVER `--proxy` in argv) for credential safety

```bash
case $exit_code in
  3) sleep 300 && HTTPS_PROXY="$PROXY_URL" retry_search ;;
  4) increase_global_timeout && retry_search ;;
  5) rephrase_query && retry_search ;;
esac
```

### v0.6.4 Adaptive Identity Pool (WS-26)
- 12 identities (4 browser families × 3 platforms) rotate through a 5-level cascade on HTTP 202/403/429
- Inspect `metadados.identidade_usada` (e.g. `chrome-linux-11111111aaaa0001`) to know which identity succeeded
- Inspect `metadados.nivel_cascata` (0..=4) to know how exhausted the pool is
- Use `--probe` for pre-flight health checks in CI before launching real queries

```bash
# v0.6.4 adaptive anti-bot pre-flight
timeout 15 duckduckgo-search-cli --probe && \
  timeout 30 duckduckgo-search-cli -q -n 10 -f json "query" | \
  jaq -r '.resultados[] | "[\(.metadados.identidade_usada // "n/a")] \(.titulo) — \(.url)"'

# Pin a specific identity for reproducible testing
timeout 30 duckduckgo-search-cli -q -n 10 -f json \
  --identity-profile chrome-linux "query"

# Reproducible identity rotation
timeout 30 duckduckgo-search-cli -q -n 10 -f json --seed 42 "query"
```


## Integration Examples
### Claude Code (Anthropic)
```bash
# Canonical Claude Code pattern — ground before editing
RESULTS=$(timeout 60 duckduckgo-search-cli -q -f json --num 15 "$QUERY")
EXIT=$?
if [ $EXIT -eq 0 ]; then
  echo "$RESULTS" | jaq -r '.resultados[] | "## \(.titulo)\n\(.snippet // "")\nURL: \(.url)\n"'
fi
```

### OpenAI Codex / GPT-4o
```bash
# Feed structured JSON into function_calling tool return
timeout 30 duckduckgo-search-cli -q -n 10 -f json "$QUERY" \
  | jaq '[.resultados[] | {title: .titulo, url, snippet: (.snippet // "")}]'
```

### Google Gemini
```bash
# Grounding via full page content for synthesis
timeout 120 duckduckgo-search-cli -q -n 5 \
  --fetch-content --max-content-length 3000 -f json "$QUERY" \
  | jaq -r '.resultados[] | "# \(.titulo)\n\(.conteudo // .snippet // "")\n"'
```

### GitHub Copilot
```bash
# Terminal pipeline — output piped directly to Copilot context
timeout 30 duckduckgo-search-cli -q -n 5 -f json "$QUERY" \
  | jaq -r '.resultados[] | "\(.posicao). \(.titulo) — \(.url)"'
```

### Cursor
```bash
# Declare in .cursorrules — Cursor invokes automatically when context is needed
timeout 30 duckduckgo-search-cli -q -n 10 -f json "$QUERY" \
  | jaq '.resultados[] | {titulo, url, snippet: (.snippet // "")}'
```

### Codeium
```bash
# Standard subprocess — same JSON contract as all other agents
timeout 30 duckduckgo-search-cli -q -n 10 -f json "$QUERY" > /tmp/search.json
[ $? -eq 0 ] && jaq -r '.resultados[].url' /tmp/search.json
```


## Performance Patterns
### Parallel Multi-Query
```bash
# queries.txt: one query per line, no blank lines
printf 'rust async channels\ntokio JoinSet examples\nrayon parallel iterators\n' > /tmp/queries.txt
timeout 300 duckduckgo-search-cli -q \
  --queries-file /tmp/queries.txt \
  --parallel 3 --per-host-limit 1 --retries 3 \
  --global-timeout 280 -n 10 -f json -o /tmp/results.json

# Extract all unique URLs across all queries
jaq -r '.buscas[].resultados[].url' /tmp/results.json | sort -u
```

- NEVER use `--parallel` above 5 — triggers anti-bot HTTP 202 response
- NEVER use `--per-host-limit` above 2 — increases block probability significantly
- NEVER loop over queries in shell — use `--queries-file` to reuse connection pools
- ALWAYS calculate `--global-timeout` as `(queries / parallel) * avg_secs * 1.5`
### Content Fetching for RAG
```bash
# Deep context extraction — reduce --num when using --fetch-content
timeout 120 duckduckgo-search-cli -q -n 5 \
  --fetch-content --max-content-length 3000 \
  -f json "$QUERY" \
  | jaq -r '.resultados[] | "\(.titulo)\n\(.conteudo // .snippet // "")\n---"'
```

- `--fetch-content` multiplies latency by N (one fetch per result)
- ALWAYS cap with `--max-content-length` to control memory and token usage
- Reduce `-n` to 5 or fewer when `--fetch-content` is active


## Checklist for Agent Developers
- Use `-q` in EVERY invocation that pipes stdout to a parser
- Use `-f json` in EVERY script — never rely on default format
- Check exit code BEFORE parsing stdout — non-zero means partial or empty output
- Set `--global-timeout` to `outer_timeout - 10` in every production call
- Use `--per-host-limit 1` together with `--parallel` to prevent blocking
- Use `--retries 3` for resilience in long-running research workflows
- Use `--fetch-content` only when snippet text is insufficient for the task
- Use `${PIPESTATUS[0]}` to detect upstream CLI failure in piped commands
- Use `HTTPS_PROXY` env var — NEVER pass proxy credentials in argv
- Use `--queries-file` for batch work — NEVER shell loops over queries
- Use `--output` for large result sets of 50 or more results
- NEVER use `--stream` — it is a placeholder and is not implemented
- NEVER inject custom `Sec-Fetch-*` headers — v0.6.0 handles them automatically
- NEVER raise `--parallel` above 5 or `--per-host-limit` above 2
- Use `duckduckgo-search-cli --probe` in CI before launching real queries (v0.6.4+, v0.6.5+)
- Treat `.metadados.identidade_usada` as `Option<String>` — use `// "n/a"` fallback in `jaq` (v0.6.4+, v0.6.5+)
- Treat `.metadados.nivel_cascata` as `Option<u32>` — use `// 0` fallback in `jaq` (v0.6.4+, v0.6.5+)
- For reproducible testing use `--identity-profile <name>` not `--seed` alone (v0.6.4+, v0.6.5+)

Upstream: https://github.com/daniloaguiarbr/duckduckgo-search-cli
Schema contract valid for `duckduckgo-search-cli` v0.7.7/v0.7.8 (extended across the v0.7.x line — see CHANGELOG).


## v0.7.3 — New Flags + JSON Behaviour

### New CLI Flags
- `--probe-deep` — runs a real search query and reports `status: "ok"` or `status: "captcha"`. Use this in CI gates for macOS runners to detect Cloudflare Bot Management interstitials before launching expensive pipelines.
- `--no-warmup` — skip the `GET https://duckduckgo.com/` warm-up that populates session cookies.
- `--no-cookie-persistence` — keep cookies in memory only; never write `cookies.json` to disk.
- `--cookies-path <PATH>` — override the default XDG cookie jar path.
- `--allow-lite-fallback` — opt-in to automatic fallback from `html` to `lite` endpoint when CAPTCHA is detected.

### Probe-Deep JSON Output Schema

The `--probe-deep` flag emits the following JSON contract:

```jsonc
{
  "type": "probe_deep",
  "endpoint": "html",
  "status": "ok",                        // "ok" | "captcha"
  "http_status": 200,                    // HTTP status of the probe request
  "latency_ms": 235,                     // wall clock latency of the probe
  "cascade_level": 0,                    // 0..=4
  "cascata_motivo": "none",              // "none" | "captcha" | "zero_results_after_retries"
  "sugestao_mitigacao": "no interstitial detected",
  "url": "https://html.duckduckgo.com/html/?q=rust"
}
```

When `status` is `"captcha"`, the operator should follow `sugestao_mitigacao` for next steps (rotate proxy, switch endpoint, back off).

### Cookie Jar Location
- Linux: `~/.config/duckduckgo-search-cli/cookies.json`
- Windows: `%APPDATA%\duckduckgo-search-cli\cookies.json`
- macOS: `~/Library/Application Support/duckduckgo-search-cli/cookies.json`

Unix permissions are `0o600` (owner read+write only). The file is internal state and is NOT exposed in the JSON output schema.


## v0.7.4 — Windows NASM preflight (build-time only)

v0.7.4 adds a build.rs preflight that detects nasm.exe on PATH for Windows MSVC native builds. Without NASM, the build fails in seconds with the exact fix rather than minutes into cryptic CMake errors. The preflight is build-time only; no new CLI flags, no new JSON fields.

- New env var: DDG_SKIP_NASM_CHECK=1 to bypass the preflight in custom build environments.
- New behavior: cargo build on Windows panics with actionable message when NASM is missing.
- CI hardening: windows-2022 jobs verify/install NASM explicitly.
- No runtime impact — same flags, same JSON output schema, same dependencies as v0.7.3.

## v0.7.5 — 4 tools preflight + helper scripts (build-time only)

v0.7.5 extends the v0.7.4 preflight to all four tools the BoringSSL build needs on Windows MSVC: NASM, CMake 3.20+ (with the C++ CMake tools for Windows sub-component), MSVC C/C++ toolchain (cl.exe/link.exe), and Strawberry Perl. Each missing tool triggers a panic in seconds with the exact fix.

- New env vars: DDG_SKIP_CMAKE_CHECK=1, DDG_SKIP_MSVC_CHECK=1, DDG_SKIP_PERL_CHECK=1 (plus DDG_SKIP_NASM_CHECK=1 from v0.7.4). Use to bypass preflight in custom build environments.
- New helper scripts in scripts/:
  - install-windows.ps1 — auto-installs NASM, CMake, Perl; reports MSVC with Launch-VsDevShell.ps1 instruction.
  - check-windows-toolchain.ps1 — standalone diagnostic; exit 0 = all 7 tools present, 1 = gap.
- New docs: docs/INSTALL-WINDOWS.md (5 installation methods, troubleshooting for each GAP, all 4 escape hatches).
- No runtime impact — same flags, same JSON output schema, same dependencies as v0.7.4. crates.io ships NO pre-built binaries for any platform.
- Test count: 405 lib tests (was 392 at v0.7.0 project total; 333 at v0.6.5 historical).


## v0.7.6 — Cargo install lockfile fix (GAP-WS-48, build-time only)

v0.7.6 fixes the GAP-WS-48 `cargo install` collision between `alloc-no-stdlib 2.0.4` and `3.0.0` by removing the `wreq-util` dep and the `brotli` feature from `wreq`. Three pins in `Cargo.toml` keep the supply chain deterministic: `brotli-decompressor = "=5.0.1"`, `alloc-no-stdlib = "=2.0.4"` (added in v0.7.7), and the `wreq 6.0.0-rc.29` choice.

- No new CLI flags, no new JSON fields, no new schema changes.
- `cargo install duckduckgo-search-cli --locked` is the supported path on a fresh system.
- `cargo tree | rg 'brotli|alloc-no-stdlib|alloc-stdlib|wreq-util'` should return zero matches after install.
- Build time dropped from ~37s to ~24s after brotli removal.


## v0.7.7 — TLS fingerprint restoration (GAP-WS-49, runtime fix)

v0.7.7 restores the JA4_o fingerprint that bypasses the DDG anti-bot interstitial. The fix re-adds `wreq-util 3.0.0-rc.12` with `default-features = false` and `features = ["emulation"]`, plus the three direct pins documented in v0.7.6. The v0.7.6 gap was that `--probe-deep` returned `status: "ok"` while real queries returned zero results.

- No new CLI flags, no new JSON fields.
- `cargo install duckduckgo-search-cli --version 0.7.7 --locked` is the recommended install path.
- `cargo tree` must show `wreq-util 3.0.0-rc.12`, `brotli 8.0.3`, `brotli-decompressor 5.0.1`, `alloc-no-stdlib 2.0.4`.
- Real-query smoke test: `duckduckgo-search-cli "rust async runtime" -q -f json` must return `quantidade_resultados >= 5`.


## v0.7.8 — Anti-bot detector overhaul + UX hardening (GAP-WS-50..57)

v0.7.8 closes 8 functional gaps in a single release. The schema contract is unchanged (zero breaking changes) but several CLI flags and internal behaviors are tightened.

### Detector overhaul (GAP-WS-50, GAP-WS-51, GAP-WS-52)
- `detectar_interstitial` in `src/probe_deep.rs` now recognizes 8 new Cloudflare markers (`anomaly-modal`, `anomaly-modal__mask`, `anomaly-modal__title`, `anomaly.js?cc=botnet`, `cf-turnstile`, `cf-spinner`, `Just a moment`, `cf-mitigated`) and 1 new DDG marker (`Unfortunately, bots use DuckDuckGo too.`).
- 8 new unit tests in `src/probe_deep.rs::tests` validate each marker with HTML fixtures.
- The probe-deep calibration query is now the 9-word pangram `the quick brown fox jumps over the lazy dog` (constant `PROBE_CALIBRATION_QUERY` in `src/lib.rs`). The 1-word query `rust` returned the DDG home page without triggering the bot detector, producing false-negative probe results.
- `--allow-lite-fallback` now consults the detector before falling back to `lite`. The fallback only triggers when the detector classifies an interstitial, not on any zero-result page.

### Verbose accumulation (GAP-WS-53)
- `-v` is now `ArgAction::Count` in `src/cli.rs`.
- Mapping: `-v` = info, `-vv` = debug, `-vvv` = trace.
- `RUST_LOG` env var still overrides.

### Supply chain (GAP-WS-54, GAP-WS-55)
- `scraper` bumped from `0.20.0` to `0.27.0` to resolve transitive `fxhash 0.2.1` (RUSTSEC-2025-0057, unmaintained).
- `cargo audit --deny warnings` is now a CI gate in `ci.yml` and `release.yml`.
- The `Cargo.toml` comment block on `wreq` was rewritten to document the intentional pin in `wreq 6.0.0-rc.29` plus the three direct pins.

### UX (GAP-WS-56, GAP-WS-57)
- The `buscar` subcommand is now `#[command(hide = true)]`. It is still invokable but does not appear in `--help`.
- `--retries N` is now honored end-to-end in `src/parallel.rs::execute_with_retry`. The pre-v0.7.8 bug hard-coded the value to 1, ignoring the flag. The new clamp is `[1, 10]` to prevent `--retries 999` from triggering anti-bot.
- 1 regression test in `tests/integration_search_retry.rs` validates that `--retries 5` produces `metadados.retentativas == 5` in the JSON output.

### Impact
- 305 tests (292 lib + 13 integration) passing; 0 advisories from `cargo audit --deny warnings`.
- Zero breaking changes in the JSON schema or exit codes.
- 4 new detector markers (resilience to anti-bot template changes).
- 1 newly honored flag (`--retries`).
- 1 hidden subcommand (`buscar`).
