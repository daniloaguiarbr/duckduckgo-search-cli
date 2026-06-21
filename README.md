[![crates.io](https://img.shields.io/crates/v/duckduckgo-search-cli)](https://crates.io/crates/duckduckgo-search-cli)
[![docs.rs](https://img.shields.io/docsrs/duckduckgo-search-cli)](https://docs.rs/duckduckgo-search-cli)
[![CI](https://github.com/daniloaguiarbr/duckduckgo-search-cli/actions/workflows/ci.yml/badge.svg)](https://github.com/daniloaguiarbr/duckduckgo-search-cli/actions)
[![License](https://img.shields.io/crates/l/duckduckgo-search-cli)](https://crates.io/crates/duckduckgo-search-cli)

> Web search at terminal speed — give your AI agent superhuman context.

<!--
SEO keywords: duckduckgo cli, search cli rust, llm web search tool, ai agent search,
claude code search, gemini cli search, codex search tool, headless web search,
json search results cli, parallel search rust, rust web search, cli web grounding,
aider search, cursor search tool, continue dev search, devin search, cline search,
retrieval augmented generation cli, rag cli, no api key search, ddg cli, tokio search cli,
rustls search cli, ndjson search stream, agent shell tool, mcp adjacent search cli
-->

## English
- Read this document in [Português](README.pt-BR.md).
### Quick Install
- Instale com um comando via cargo:

```bash
cargo install duckduckgo-search-cli
```
### Why this exists

Every modern LLM carries a knowledge cutoff, and every autonomous agent eventually needs something its weights never saw: the latest library version, a 2026 incident post-mortem, a vendor's current pricing page. Bolting on a hosted search API costs money, leaks queries, and breaks when the vendor rate-limits you in the middle of a multi-step plan.

`duckduckgo-search-cli` is a single Rust binary that turns any shell into a first-class search tool. No API key. No tracking. Chrome-powered search that runs invisibly. A stable JSON schema, bounded concurrency, and predictable exit codes — exactly what an agent needs to ground itself in real-time web data without becoming a liability.

### Superpowers for every AI agent

Drop this binary into any agent that can run a shell command. That is nearly every serious agent shipping today.

| Agent                  | How it benefits                                                                                   |
| ---------------------- | ------------------------------------------------------------------------------------------------- |
| Claude Code            | `Bash` tool invokes `duckduckgo-search-cli "query" --num 15 -q \| jaq '.resultados'` for grounded research before edits. |
| OpenAI Codex           | Shell access feeds structured JSON into the context window for up-to-date library docs.           |
| Gemini CLI             | Pipe results into Gemini's JSON mode for synthesis of long-tail web facts.                        |
| Cursor                 | Declare the binary in `.cursorrules` and let the AI call it whenever it lacks context.            |
| Windsurf               | Cascade agents call the CLI as a deterministic tool for fresh web data.                           |
| Aider                  | Use `/run` to inject search results straight into the edit conversation.                          |
| Continue.dev           | Register as a custom slash-command to ground in-IDE completions.                                  |
| MiniMax                | Shell-tool integration gives MiniMax agents DuckDuckGo-backed global coverage.                    |
| OpenCode               | Model-agnostic shell integration — works with any provider OpenCode is pointed at.                |
| Paperclip              | Drop-in primitive for autonomous research pipelines.                                              |
| OpenClaw               | Open-source agent runtime — use it as the default search backend.                                 |
| Google Antigravity     | Headless CLI fallback for the surface-level browser agent when direct scraping is needed.         |
| GitHub Copilot CLI     | `gh copilot` workflows that require verified URLs and fresh references.                           |
| Devin                  | Autonomous engineer reads the JSON output to plan before touching code.                           |
| Cline                  | VS Code agent calls the binary as a terminal command for grounded answers.                        |
| Roo Code               | Roo Cline fork — same zero-config shell integration.                                              |

### Why it's perfect for AI agents

- **JSON-first by default.** Stable schema with `resultados[]` and `metadados`, field order frozen across releases, ready for `jaq` and direct parsing into tool calls.
- **Zero API key, zero tracking.** Talks directly to DuckDuckGo's HTML endpoint over HTTPS. No authentication to rotate, no dashboard to babysit, no data leak surface.
- **Parallel by design.** `--parallel 1..=20` fans out multiple queries through a `tokio::JoinSet`, and `--per-host-limit` prevents burst abuse when `--fetch-content` is on.
- **15 results by default.** Generous context for LLMs without forcing you to spell out `--num`. Override per call when you need to.
- **Auto-pagination that just works.** When `--num` exceeds a single DuckDuckGo page, the CLI automatically crawls up to 2 pages so you always get the count you asked for.
- **Optional readable body extraction.** `--fetch-content` downloads each URL and embeds cleaned text straight into the JSON, capped by `--max-content-length`.
- **Cross-platform single binary.** Linux (glibc, musl/Alpine), macOS Intel + Apple Silicon Universal, Windows MSVC — all from one `cargo install`.
- **Real browser TLS fingerprint via BoringSSL (v0.7.3+).** BoringSSL is statically linked by `wreq`, producing a JA4_o fingerprint identical to Chrome/Safari. Eliminates the Cloudflare CAPTCHA that affected macOS in v0.7.2. Build requires `cmake`, `perl`, `pkg-config`, and `libclang-dev` on Linux. musl/Alpine static builds still work but require the same toolchain. See `docs/decisions/0001-tls-boring-via-wreq.md` and `docs/CROSS_PLATFORM.md`.
- **NDJSON streaming.** `--stream` emits one line per result the moment it arrives, feeding reactive pipelines without buffering the whole response.
- **Hardened exit codes.** Distinct codes for runtime errors, bad config, soft rate-limit, global timeout, and zero-results — so agents can branch deterministically.
- **v0.5.0 security hardening.** Path traversal validation on `--output` rejects `..` and system directories; proxy credentials masked in error messages; typed errors via `ErroCliDdg` with 11 deterministic variants.
- **v0.6.0 anti-blocking.** Per-browser `Sec-Fetch-*` headers and Client Hints for Chrome/Edge; `Accept-Language` with RFC 7231 q-values; HTTP 202 anomaly detection; silent block detection with 5 KB threshold.

### Agent Skill — bundled, bilingual, auto-activating

Stop writing system prompts that remind your agent to search. This repo already ships a pre-built Claude Agent Skill, and Claude picks it up automatically the moment a user mentions research, verification, fresh docs or URL grounding — in less than a second, with zero prompt engineering.

- **Two production-grade skills live in this repo.** `skill/duckduckgo-search-cli-en/SKILL.md` and `skill/duckduckgo-search-cli-pt/SKILL.md` — English and Brazilian Portuguese, each with a unique `name` field so both can coexist in the same Claude install.
- **Auto-activation, straight out of the box.** The `description` field is front-loaded with the triggers users actually type ("search the web", "ground this", "verify this URL", "pesquise online", "traga resultados atualizados"). Claude matches on semantics — no slash command, no tool registration.
- **14 canonical MUST/NEVER sections per skill.** Mandatory `-q -f json` contract, `jaq` parsing, deterministic exit codes, batch mode, content extraction, endpoint fallback, retries, post-validation — the agent reads this once and stops inventing flags forever.
- **Token-efficient by design.** One ~1,000-word skill replaces a sprawling system prompt. Loaded once per session, referenced every time — trims hundreds of tokens off every future search turn.
- **Anti-hallucination guarantee.** Every flag the agent might invoke is documented inside the skill with a frozen JSON contract. No made-up arguments, no retry loops, no wasted tool calls.
- **Installs in one command.** Copy the folder into your Claude config and you are done — the skill lives on GitHub, not in the crates.io tarball, so always pull the freshest version from `main`.

```bash
# One-shot install (clone and copy whichever language you prefer).
git clone https://github.com/daniloaguiarbr/duckduckgo-search-cli
cp -r duckduckgo-search-cli/skill/duckduckgo-search-cli-en ~/.claude/skills/
cp -r duckduckgo-search-cli/skill/duckduckgo-search-cli-pt ~/.claude/skills/

# Restart Claude Code (or reload the Agent SDK). That is the whole setup.
```

### 📚 Documentation

Three deep-dive guides ship with the crate. Read them once — they pay back forever.

| Guide | Why it matters |
|-------|---------------|
| [`docs/AGENT_RULES.md`](docs/AGENT_RULES.md) | 30+ MUST/NEVER rules for any LLM/agent invoking this CLI in production. Bilingual EN+PT. |
| [`docs/COOKBOOK.md`](docs/COOKBOOK.md) | 15 copy-paste recipes for research, ETL, monitoring, content extraction. Bilingual EN+PT. |
| [`docs/INTEGRATIONS.md`](docs/INTEGRATIONS.md) | Drop-in snippets for 16 agents: Claude Code, Codex, Gemini CLI, Cursor, Windsurf, Aider, Continue.dev, MiniMax, OpenCode, Paperclip, OpenClaw, Antigravity, Copilot CLI, Devin, Cline, Roo Code. |

### Prerequisites (v0.8.5+)
- Google Chrome or Chromium (auto-detected via `detect_chrome()`)
- Linux: `sudo dnf install xorg-x11-server-Xvfb` (Fedora) or `sudo apt install xvfb` (Debian/Ubuntu)
- Chrome is the PRIMARY search transport since v0.8.0
- wreq HTTP client is used ONLY for `--fetch-content` and `--probe`
- To build without Chrome: `cargo build --no-default-features`
- v0.8.5: Chrome runs HEADED inside a private Xvfb virtual display — ZERO visible windows
- The CLI auto-spawns and auto-kills Xvfb — no manual setup needed on desktops
- Fallback: headless mode if Xvfb is not available (with anti-bot risk)
- Env vars: `DUCKDUCKGO_CHROME_VISIBLE=1` (debug), `DUCKDUCKGO_CHROME_HEADLESS=1` (force headless), `DUCKDUCKGO_CHROME_XVFB=1` (xvfb on servers)

### Quick Start

```bash
cargo install duckduckgo-search-cli
duckduckgo-search-cli "rust async runtime"
# 15 fresh JSON results on your desk.

# For LLMs and agents:
duckduckgo-search-cli "tokio JoinSet examples" --num 15 -q | jaq '.resultados'
```

### Deep Research (v0.7.0)

For multi-hop research questions — "compare the four major Rust HTTP clients in 2026", "what changed in Tokio 1.40", "summarise the history of DuckDuckGo's HTML endpoint" — `duckduckgo-search-cli` ships a query fan-out pipeline that decomposes the original question into 1..=12 sub-queries, fans them out in parallel, aggregates the results, and optionally synthesises a numbered-reference report.

```bash
# Default heuristic decomposition (5 sub-queries, RRF aggregation, no synthesis).
duckduckgo-search-cli deep-research "best rust http client 2026" -f json -q \
  | jaq '.resultados[] | {titulo, url, score}'

# Markdown report with explicit token budget and full content extraction.
duckduckgo-search-cli deep-research "tokio vs async-std production 2026" \
  --synthesize --budget-tokens 1500 --synth-format markdown \
  --fetch-content --max-content-length 6000 -f json -q

# Manual sub-queries from a file (comments `#` and blanks ignored).
cat > /tmp/qs.txt <<EOF
# Overview
what is tokio runtime 2026
# Comparison
tokio vs async-std vs smol
# Adoption
tokio production users 2026
EOF
duckduckgo-search-cli deep-research "tokio runtime 2026" \
  --sub-queries-file /tmp/qs.txt --aggregate dedupe-by-url -f json -q
```

#### Deep Research flags

| Flag                       | Default        | Description                                                                 |
| -------------------------- | -------------- | --------------------------------------------------------------------------- |
| `--max-sub-queries N`      | `5`            | Maximum sub-queries produced (1..=12).                                      |
| `--sub-query-strategy`     | `heuristic`    | `heuristic` (five canonical templates) or `manual` (from `--sub-queries-file`). |
| `--sub-queries-file PATH`  | (none)         | Path to explicit sub-queries (one per line; `#` comments skipped).          |
| `--aggregate`              | `rrf`          | `rrf` (Reciprocal Rank Fusion, K=60) or `dedupe-by-url` (canonical URL).    |
| `--depth`                  | `0`            | Reflection rounds planned but not executed in v0.7.0.                      |
| `--fetch-content`          | off            | Extract page body for the aggregated top-K.                                 |
| `--synthesize`             | off            | Produce a final Markdown / PlainText / JSON report.                          |
| `--budget-tokens N`        | `1200`         | Token budget for the synthesised report (1 token ≈ 4 chars).               |
| `--synth-format`           | `markdown`     | Output format for synthesis: `markdown`, `plain`, `json`.                   |

#### Deep Research output schema

```jsonc
{
  "metadados": {
    "query_original": "best rust http client 2026",
    "sub_queries": [
      { "texto": "...", "estrategia": "heuristic", "status": "ok", "elapsed_ms": 420 }
    ],
    "total_resultados_unicos": 27,
    "tempo_total_ms": 1850,
    "nivel_cascata": 0
  },
  "resultados": [
    { "titulo": "...", "url": "...", "score": 0.041, "fontes": ["..."] }
  ],
  "sintese": "# Research Report\n\n...\n\n[1] Title — url"
}
```

The subcommand inherits the global flags (`--num`, `--lang`, `--country`, `--parallel`, `--endpoint`, `--proxy`, `--retries`, `--global-timeout`) and adds the deep-research-specific knobs above. All cancellation, retry, anti-bot, and circuit-breaker behaviour from the search path applies unchanged.

### Real-world recipes

```bash
# 1. Extract only URLs for a downstream fetcher.
duckduckgo-search-cli "site:example.com changelog 2025" --num 15 -f json \
  | jaq -r '.resultados[].url'

# 2. Feed cleaned page bodies into a summarizer.
duckduckgo-search-cli "tokio runtime internals" --num 15 \
  --fetch-content --max-content-length 4000 -f json \
  | jaq -r '.resultados[] | "# \(.titulo)\n\(.conteudo)\n"' > corpus.md

# 3. Fan-out multiple queries in one shot.
duckduckgo-search-cli "rust rayon" "rust tokio" "rust crossbeam" \
  --num 15 --parallel 3 -f json

# 4. NDJSON streaming for reactive pipelines.
duckduckgo-search-cli "wasm runtimes" --num 15 --stream \
  | jaq -r 'select(.url) | .url' \
  | xargs -I{} my-downloader {}

# 5. Route through a corporate proxy (env var also respected).
duckduckgo-search-cli "vendor status page 2026" --num 15 \
  --proxy http://user:pass@proxy.internal:8080 -f json

# 6. Offline smoke test (no real network).
cargo test --test integration_wiremock
```

### Configuration

```bash
# Write default selectors.toml and user-agents.toml to the XDG dir.
duckduckgo-search-cli init-config

# Dry-run first to see what would be written.
duckduckgo-search-cli init-config --dry-run

# Overwrite existing files explicitly.
duckduckgo-search-cli init-config --force
```

### Commands

| Command                                          | Purpose                                                       |
| ------------------------------------------------ | ------------------------------------------------------------- |
| `duckduckgo-search-cli <QUERY>...`               | Default search (equivalent to `buscar`).                      |
| `duckduckgo-search-cli buscar <QUERY>...`        | Explicit search subcommand.                                   |
| `duckduckgo-search-cli deep-research <QUERY>`    | Query fan-out, aggregation, and optional synthesis (v0.7.0).  |
| `duckduckgo-search-cli init-config`              | Write `selectors.toml` and `user-agents.toml` to XDG.         |

### Flags

| Flag                       | Default    | Description                                                        |
| -------------------------- | ---------- | ------------------------------------------------------------------ |
| `-n`, `--num`              | `15`       | Max results per query (auto-paginates when > 10).                  |
| `-f`, `--format`           | `auto`     | `json`, `text`, `markdown`, or `auto` (TTY-aware).                 |
| `-o`, `--output`           | stdout     | Write to file (v0.5.0: path validation, parent dirs, Unix 0o644). |
| `-t`, `--timeout`          | `15`       | Per-request timeout (seconds).                                     |
| `--global-timeout`         | `60`       | Whole-pipeline timeout (1..=3600 seconds).                         |
| `-l`, `--lang`             | `pt`       | DuckDuckGo `kl` language code.                                     |
| `-c`, `--country`          | `br`       | DuckDuckGo `kl` country code.                                      |
| `-p`, `--parallel`         | `5`        | Concurrent requests (1..=20).                                      |
| `--pages`                  | `1`        | Pages to crawl per query (1..=5, auto-raised by `--num`).          |
| `--retries`                | `2`        | Extra retries on 429/403/timeout (0..=10).                         |
| `--endpoint`               | `html`     | `html` or `lite`.                                                  |
| `--time-filter`            | (none)     | `d`, `w`, `m`, or `y`.                                             |
| `--safe-search`            | `moderate` | `off`, `moderate`, or `on`.                                        |
| `--stream`                 | off        | Emit one NDJSON line per result as they arrive.                    |
| `--fetch-content`          | off        | Fetch each URL and embed cleaned body text.                        |
| `--max-content-length`     | `10000`    | Character cap per extracted body (1..=100_000).                    |
| `--per-host-limit`         | `2`        | Concurrent fetches per host (1..=10).                              |
| `--proxy URL`              | (none)     | HTTP/HTTPS/SOCKS5 proxy (takes precedence over env vars).          |
| `--no-proxy`               | off        | Disable every proxy source.                                        |
| `--queries-file PATH`      | (none)     | Read additional queries (one per line).                            |
| `--match-platform-ua`      | off        | Filter UA pool to the current OS.                                  |
| `--chrome-path PATH`       | (auto)     | Manual Chrome executable (feature `chrome`).                       |
| `-v`, `--verbose`          | off        | Debug logs on stderr.                                              |
| `-q`, `--quiet`            | off        | Error-only logs on stderr.                                         |
| `--probe`                  | off        | Pre-flight health check (1 minimal request, JSON report).          |
| `--probe-deep`             | off        | Real-query CAPTCHA interstitial detector (v0.7.3+).               |
| `--identity-profile`       | `auto`     | Pin a 12-identity pool profile (`chrome-win`, `safari-mac`, ...). |
| `--seed N`                 | (random)   | Deterministic seed for UA + identity selection.                   |
| `--no-warmup`              | off        | Skip the `GET https://duckduckgo.com/` warm-up (v0.7.3+).         |
| `--no-cookie-persistence`  | off        | Keep cookies in memory only; never write to disk (v0.7.3+).       |
| `--cookies-path PATH`      | XDG config | Override the default cookie jar path (v0.7.3+).                  |
| `--allow-lite-fallback`    | off        | Auto-fallback to `--endpoint lite` when CAPTCHA detected (v0.7.3+). |
| `--pre-flight`             | off        | Auto-route to Lite when ghost-block detected (sub-4KB body, no result-page signal, v0.7.9+). |

## Schema JSON (v0.7.10)

### Consumer migration guide

When parsing the JSON envelope, consumers MUST handle these schema
changes:

| Version | Field path | Type | Default | BC |
|---|---|---|---|---|
| v0.7.10 | `metadados.pre_flight_disparado` | bool | `false` | Additive |

No fields removed. No fields deprecated. The `--require-results`
flag in `deep-research` is local to that subcommand and emits exit
code `70` (EX_SOFTWARE) on zero aggregated results instead of `0`
(silent zero-result).

When the probe-deep endpoint detects a CAPTCHA, the JSON envelope
now includes the specific marker matched:

```json
{
  "type": "probe_deep",
  "cascata_motivo": "cloudflare",
  "sugestao_mitigacao": "Cloudflare challenge detected (marker: cf-turnstile). Re-run with --pre-flight..."
}
```

Consumers should treat sentinels starting with `<` (e.g.
`<ghost-block-no-marker>`, `<empty-body>`, `<no-marker>`) as
non-literal markers and omit them from user-facing lists.

### Migration notes (v0.7.9 → v0.7.10)

- New optional field `metadados.pre_flight_disparado: bool` (default
  `false`). Ignore if not present in v0.7.9 envelopes.
- New flag `deep-research --require-results` (default `false`). When
  set, exit code 70 is emitted on zero aggregated results. CI pipelines
  that need strict failure detection should opt in.
- `sugestao_mitigacao` in `probe_deep` envelope now includes the
  matched marker when available (e.g. `cf-challenge`,
  `robot-detected`). For ghost-block heuristic, the message omits
  the marker name and cites the heuristic instead.

### Environment variables

| Variable       | Description                                                 | Example                            |
| -------------- | ----------------------------------------------------------- | ---------------------------------- |
| `RUST_LOG`     | Overrides the `tracing-subscriber` filter.                  | `RUST_LOG=duckduckgo=debug`        |
| `HTTP_PROXY`   | Default HTTP proxy (lower priority than `--proxy`).         | `http://user:pass@proxy:8080`      |
| `HTTPS_PROXY`  | Default HTTPS proxy.                                        | `http://proxy:8443`                |
| `ALL_PROXY`    | Fallback proxy for any scheme.                              | `socks5://127.0.0.1:9050`          |
| `CHROME_PATH`  | Fallback Chrome path (feature `chrome`).                    | `/opt/google/chrome/chrome`        |
| `DUCKDUCKGO_CHROME_VISIBLE` | Force headed Chrome with visible window (debug). | `DUCKDUCKGO_CHROME_VISIBLE=1` |
| `DUCKDUCKGO_CHROME_HEADLESS` | Force headless Chrome (anti-bot risk). | `DUCKDUCKGO_CHROME_HEADLESS=1` |
| `DUCKDUCKGO_CHROME_XVFB` | Opt-in headed via xvfb-run on servers. | `DUCKDUCKGO_CHROME_XVFB=1` |
| `DUCKDUCKGO_SEARCH_CLI_NO_CHROME` | Disable Chrome at runtime. | `DUCKDUCKGO_SEARCH_CLI_NO_CHROME=1` |
| `DUCKDUCKGO_ZERO_CAUSE_STRICT` | BC opt-out: map exit 6 back to exit 5 (v0.8.0+). | `DUCKDUCKGO_ZERO_CAUSE_STRICT=false` |

### Output formats

- `json` (default for pipes): canonical schema with `resultados[]` and `metadados`, stable field order. Each result may include the optional `titulo_original` field when the "Official site" heuristic replaces the title with `url_exibicao`. The `html.duckduckgo.com/html/` endpoint does not expose related searches in the DOM, so v0.3.0 removed the `buscas_relacionadas` field.
- `text`: human-readable block `NN. Title\n   URL\n   snippet`.
- `markdown`: `- [Title](URL)\n  > snippet`.
- Stream (`--stream`): NDJSON where each line is one result; metadata emitted as the final line.

### Exit codes

| Code | Meaning                                                        |
| ---- | -------------------------------------------------------------- |
| 0    | Success.                                                       |
| 1    | Runtime error (network, parse, I/O).                           |
| 2    | Invalid configuration (CLI flag out of range, bad proxy URL).  |
| 3    | DuckDuckGo 202 block anomaly (soft-rate-limit).                |
| 4    | Global timeout exceeded.                                       |
| 5    | Zero results across all queries.                               |
| 6    | Suspected block (zero results with non-legitimate cause, v0.8.0+). |

### Troubleshooting

1. **HTTP 202 / block anomaly (exit 3)** — back off, raise `--retries`, rotate UA via `init-config` and tweak `user-agents.toml`.
2. **Rate limited (HTTP 429)** — lower `--per-host-limit`, enable `--match-platform-ua`, or add `--proxy`.
3. **Zero results (exit 5)** — check `--lang` and `--country`, try `--endpoint lite`, and verify `--time-filter`.
4. **Chrome not found** — install Chromium via your package manager, or pass `--chrome-path /path/to/chrome`; the feature must be compiled with `cargo install duckduckgo-search-cli --features chrome`.
5. **UTF-8 issues on Windows** — the binary auto-switches cmd.exe to code page 65001; if you still see mojibake, run `chcp 65001` before the command.
6. **How do I integrate with Claude Code, Cursor, Aider, or another agent?** — expose the binary as a shell tool. Most agents accept a command template such as `duckduckgo-search-cli "{query}" --num 15 -q -f json`. The stable schema keeps the tool contract stable across releases.
7. **Pipe to jaq/jq returns empty** — check `echo ${PIPESTATUS[*]}` after the pipe. If the first number is non-zero, the CLI errored before producing output. Common causes: DuckDuckGo rate-limiting (exit 5), global timeout (exit 4), or missing query. Always pass `-q -f json` when piping.
8. **`--output` rejects my path (exit 2)** — v0.5.0 validates output paths before writing. Paths containing `..` are rejected to prevent directory traversal. Paths targeting system directories (`/etc`, `/usr`, `/bin`, `C:\Windows`) are blocked. Use paths under your home directory, `/tmp`, or the current working directory.
9. **Getting exit 5 (zero results) frequently** — this is usually temporary rate-limiting from DuckDuckGo, not a permanent block. Wait 60 seconds and retry. If the problem persists, add `--proxy socks5://127.0.0.1:9050` to rotate your outbound IP, or try `--endpoint lite` as a fallback. v0.6.0 browser fingerprint profiles reduce this significantly by mimicking real browser sessions.
10. **CAPTCHA interstitial suspected (v0.7.3+)** — run `duckduckgo-search-cli --probe-deep -q -f json` to classify the response body. If `status` is `captcha`, the response is blocked. The probe also reports `sugestao_mitigacao` with concrete next steps (rotate proxy, switch endpoint, back off). Treat the cookie jar as credential: the file `cookies.json` is written with 0o600 permissions and contains session cookies from DuckDuckGo.

### Migration notes (v0.6.x → v0.7.0)

- **New subcommand `deep-research`** is the only public addition. The existing `buscar` / default-search path keeps its flags, JSON schema, and exit codes byte-for-byte identical.
- **Four new public modules** are exposed in `lib.rs` — `deep_research`, `decomposition`, `aggregation`, `synthesis` — for downstream crates that want to compose their own research pipeline around the same primitives.
- **New direct dependencies** in `Cargo.toml`: `url = "2"`, `regex = "1"`, and `proptest = "1"` (dev-only).
- **Zero breaking changes** to `SearchOutput`, `MultiSearchOutput`, the default-config JSON schema, or any exit code.


## Migration notes (v0.7.7 → v0.7.8)

- **Zero breaking changes.** All CLI flags, JSON output schemas, and exit codes from v0.7.7 remain unchanged.
- **Anti-bot detector overhaul (GAP-WS-50, WS-51, WS-52)**: the `detectar_interstitial` function now recognizes the new DDG anomaly-modal interstitial (CSS classes `anomaly-modal__mask` and `anomaly-modal__title`, marker text `Unfortunately, bots use DuckDuckGo too.`, challenge URL `anomaly.js?cc=botnet`). The `--probe-deep` subcommand now uses a long calibration query (`the quick brown fox jumps over the lazy dog`) instead of the short `rust` to actually trigger upstream bot scoring. The `--allow-lite-fallback` flag now consults the detector before falling back, so a real anti-bot block returns `exit 3` with `cascata_motivo` populated instead of a silent `exit 5` with zero results.
- **Verbose `-vv` and `-vvv` are now supported (GAP-WS-53)**: `--verbose` switched from `ArgAction::SetTrue` to `ArgAction::Count`. Use `-v` for `info`, `-vv` for `debug`, `-vvv` for `trace`. The `RUST_LOG` env var continues to override. Examples:
  - `duckduckgo-search-cli -v "rust async"` — info-level logs
  - `duckduckgo-search-cli -vv "rust async"` — debug-level logs (request/response bodies)
  - `duckduckgo-search-cli -vvv "rust async" 2>debug.log` — trace-level logs for deep forensics
- **`--retries N` is now honored (GAP-WS-57)**: previously the value was hard-coded to 1, so `--retries 5` silently behaved like `--retries 1`. The flag is now read from `Config.retries` with a clamp of `[1, 10]` to prevent abuse (`--retries 999` triggers anti-bot). Example: `duckduckgo-search-cli --retries 5 --allow-lite-fallback "rust async runtime"` now retries 5 times and falls back to lite on interstitial detection.
- **`--allow-lite-fallback` now actually works (GAP-WS-52)**: example pipeline that previously returned zero results silently now returns a captcha-detected fallback response:
  - `duckduckgo-search-cli --probe-deep --allow-lite-fallback -q -f json` — pre-flight check with auto-fallback opt-in
  - `duckduckgo-search-cli --allow-lite-fallback --retries 3 "long tail query" 2>cascata.log` — auto-fallback enabled, 3 retries per request, logs cascade reason to stderr
- **Subcommand `buscar` is now hidden (GAP-WS-56)**: the canonical form is still top-level invocation (`duckduckgo-search-cli "query"`). The `buscar` subcommand remains functional but no longer appears in `--help`. The help for `buscar --help` no longer duplicates the global help.
- **Supply chain (GAP-WS-54)**: `scraper` bumped from 0.20 to 0.27, which transitively removes the unmaintained `fxhash 0.2.1` (RUSTSEC-2025-0057). `cargo audit --deny warnings` is now a hard CI gate in both `ci.yml` and `release.yml`. `async-std` (RUSTSEC-2025-0052) remains only in the optional `chrome` feature.
- **Doc drift fix (GAP-WS-55)**: the `wreq` comment in `Cargo.toml` was rewritten to reflect the actual decision (pin on `wreq 6.0.0-rc.29` plus the three direct pins for `wreq-util`, `brotli-decompressor`, `alloc-no-stdlib`), not the never-happened regression mentioned in the stale comment.
- **Test count: 305 (292 lib + 13 integration)**, 0 clippy warnings, 0 fmt diff, 0 cargo-deny warnings, `cargo doc --offline --no-deps` clean.

## Migration notes (v0.7.0 → v0.7.1)

- **Zero breaking changes.** All CLI flags, JSON output schemas, and exit codes from v0.7.0 remain unchanged.
- **Dependency migration (internal)**: `rand` bumped from `0.8` to `0.9` to align with `proptest 1.11+` (dev-dep). All internal call sites updated:
  - `Rng::gen_range` → `Rng::random_range` (7 sites)
  - `Rng::gen_bool` → `Rng::random_bool` (2 sites)
  - `Rng::gen::<T>()` → `Rng::random::<T>()` (1 site)
  - `rand::thread_rng()` → `rand::rng()` (4 sites)
  - `rand::seq::SliceRandom::choose` → `rand::seq::IndexedRandom::choose` for slice `.choose()` calls; `IteratorRandom::choose` kept for iterator `.choose()` calls
- **MSRV bump**: `rust-version` raised from `1.75` to `1.85` to satisfy `rand 0.9` MSRV and the wave of edition-2024 transitive deps (`assert_cmd 2.2+`, `blake3 1.8+`, `clap 4.6+`, `proptest 1.11+`, `chrono 0.4.41+`, `idna 1.1+`, `icu_* 2.0+`, `home 0.5.11+`, `async-lock 3.4+`, etc.).
- **reqwest builder cleanup**: removed `ClientBuilder::gzip(true)` and `.brotli(true)` calls (these methods were removed in `reqwest 0.12+`; decompression is now automatic via the `Accept-Encoding` header).
- **CI hygiene**: two `actionlint` shellcheck warnings fixed:
  - `.github/workflows/ci.yml:520` — quoted command substitution `$(date ...)` to `"\$(date ...)"` (SC2046)
  - `.github/workflows/release.yml:505` — added `--` prefix to glob `sha256sum -- *` (SC2035)
- **Security advisory ignore**: `RUSTSEC-2026-0009` (time 0.3.40 DoS via RFC 2822 stack exhaustion) added to `deny.toml` ignore list. The fix in `time 0.3.47` requires `rust-version 1.88+` which we cannot satisfy at the current MSRV. Impact: a CLI that only parses `Date` headers from HTTP responses under the user's explicit `--lang`/`--country` flags; the response body size cap already limits input length.
- **392 tests passing** (279 lib + 12 doc + 101 integration). 0 clippy warnings, 0 doc warnings, 0 fmt diff, 4 cargo-deny gates green, `cargo publish --dry-run` clean.

## Migration notes (v0.7.4 → v0.7.5)

- **No runtime changes.** v0.7.5 is a build-experience and documentation release: same flags, same JSON schema, same dependencies.
- **GAP-WS-29/30/31/32/33/34/35/36/37 closed in this repository.** The v0.7.4 NASM preflight was extended to also detect **CMake** (the `cmake` crate 0.1.58 needs `cmake.exe` in PATH BEFORE `enable_language(ASM_NASM)` is evaluated), **MSVC compiler and linker** (`cl.exe`/`link.exe` — need `Launch-VsDevShell.ps1` to set PATH, INCLUDE, LIB), and **Perl** (`perl.exe` for BoringSSL's perlasm generator). New preflight in `build.rs` aborts in seconds with the exact fix for each of the four tools. Escape hatches: `DDG_SKIP_NASM_CHECK=1`, `DDG_SKIP_CMAKE_CHECK=1`, `DDG_SKIP_MSVC_CHECK=1`, `DDG_SKIP_PERL_CHECK=1`. Root cause: the C++ CMake tools for Windows sub-component of the Visual Studio Installer is deselected by default — installing only the C++ workload does NOT provide CMake.
- **Helper extended `scripts/install-windows.ps1`** — now also detects and auto-installs CMake (`winget install -e --id Kitware.CMake` or choco) and Perl (`winget install -e --id StrawberryPerl.StrawberryPerl`), and reports the exact MSVC install / `Launch-VsDevShell.ps1` instruction (MSVC is too large to auto-install). New `--check-only` mode produces a tabular report suitable for CI gates and human support.
- **New `scripts/check-windows-toolchain.ps1`** — standalone diagnostic (no installs) that checks all 7 tools (cargo, rustc, cmake, nasm, cl.exe, link.exe, perl) and emits text or JSON output. Exit code 0 if all present, 1 otherwise. Use for support tickets and CI gates.
- **New `docs/INSTALL-WINDOWS.md`** — step-by-step guide covering 5 installation methods (Visual Studio Installer + standalone tools; all-standalone via winget; Chocolatey only; helper script; standalone diagnostic). Includes troubleshooting for each of the 4 GAPs and the `DDG_SKIP_*_CHECK` escape hatches.
- **Documentation corrected** — the false claim that "VS Build Tools with C++ workload provides CMake" was replaced in `docs/CROSS_PLATFORM.md`, `skill/duckduckgo-search-cli-en/SKILL.md`, `llms.txt` and `llms-full.txt`. The C++ workload does NOT include the C++ CMake tools sub-component — it must be selected manually in the Visual Studio Installer.

## Migration notes (v0.7.3 → v0.7.4)

- **No runtime changes.** v0.7.4 is a build-experience and documentation release: same flags, same JSON schema, same dependencies.
- **GAP-WS-28 closed in this repository.** `cargo install` on native Windows MSVC without NASM previously failed minutes into the build with the cryptic `CMake Error: No CMAKE_ASM_NASM_COMPILER could be found`. A new `build.rs` preflight now fails in SECONDS with the exact fix (`winget install -e --id NASM.NASM`, PATH adjustment, or `scripts/install-windows.ps1`). Root cause: BoringSSL requires NASM-format crypto assembly unless `OPENSSL_NO_ASM` is set, and the `btls-sys` v0.5.6 branch that sets it for Windows is unreachable in native builds (early return when host == target in its build script). Set `DDG_SKIP_NASM_CHECK=1` to bypass the preflight (e.g., custom toolchain files).
- **New helper `scripts/install-windows.ps1`** — detects NASM, installs it via winget (choco fallback), fixes the session PATH, and runs `cargo install duckduckgo-search-cli --locked` with any extra arguments forwarded.
- **CI hardening**: Windows jobs in `ci.yml` and `release.yml` now verify/install NASM explicitly instead of relying on the runner image to ship it.

## Migration notes (v0.7.2 → v0.7.3)

- **BREAKING BUILD-ENV: TLS stack changed from rustls to BoringSSL via `wreq`.** The build now requires `cmake`, `perl`, `pkg-config`, and `libclang-dev` on Linux, and the NASM assembler (`winget install -e --id NASM.NASM`), the C++ CMake tools for Windows sub-component (manually selected in the Visual Studio Installer — NOT included in the C++ workload by default; see `docs/INSTALL-WINDOWS.md` for step-by-step), Strawberry Perl (`winget install -e --id StrawberryPerl.StrawberryPerl`), and the MSVC toolchain (cl.exe, link.exe, configured via `Launch-VsDevShell.ps1`) on Windows MSVC. Note that `cargo install` always compiles from source — crates.io does not ship pre-built binaries — so these prerequisites apply to every `cargo install` user, not only to CI. Windows users can run `scripts/install-windows.ps1`, which installs NASM, CMake, and Perl automatically when missing (MSVC is not auto-installed — too intrusive). Without the C++ CMake tools the build fails with `failed to execute command: program not found / is cmake not installed?`; without NASM with `No CMAKE_ASM_NASM_COMPILER could be found` (see `gaps.md` GAP-WS-28/29/30/31/36).
- **GAP-WS-27 closed.** The macOS CAPTCHA interstitial (HTTP 200 with empty body, exit 5, `quantidade_resultados: 0`) caused by Cloudflare's bot scoring of the `rustls` TLS fingerprint is fixed. Same query that returned 0 results in v0.7.2 returns 5 results in v0.7.3 on the same machine. See `gaps.md` and `docs/decisions/0001-tls-boring-via-wreq.md`.
- **New CLI flags (additive)**:
  - `--no-warmup` — skip the warm-up `GET https://duckduckgo.com/` before the first real query
  - `--no-cookie-persistence` — keep cookies in memory only; never write `cookies.json` to disk
  - `--cookies-path <PATH>` — override the default XDG cookie jar path
  - `--probe-deep` — run a real search query and classify the body as `ok` or `captcha` based on Cloudflare and DuckDuckGo markers
  - `--allow-lite-fallback` — opt-in to automatic fallback from `html` to `lite` endpoint when `--probe-deep` (or zero-result retries) detect CAPTCHA
- **New persistent state: cookie jar.** A `cookies.json` file is now written to `~/.config/duckduckgo-search-cli/cookies.json` (Linux), `%APPDATA%\duckduckgo-search-cli\cookies.json` (Windows), or `~/Library/Application Support/duckduckgo-search-cli/cookies.json` (macOS). Unix permissions are `0o600` (owner read+write only). Treat this file as you would treat a credential — see `SECURITY.md`. Use `--no-cookie-persistence` to opt out.
- **Zero changes to JSON output schema.** All fields from v0.7.2 remain present. No new `Option<T>` fields added at the top level (the session/cookie state is internal to the pipeline, not exposed to the agent).
- **New dependencies**: `wreq 6.0.0-rc.29`, `wreq-util 3.0.0-rc.12`, plus transitive `boring2 4.15.11`, `webpki-root-certs 1.0.7`, and the BoringSSL C toolchain.
- **Removed dependencies**: `reqwest 0.12.28`, `time 0.3.47` (no longer a direct dep — purely transitive now).
- **Test count: 292 lib** (was 279 in v0.7.2). +13 new tests across `session_warmup` (5), `wreq_cookie_adapter` (3), and `probe_deep` (5). 0 clippy warnings, 0 fmt diff, 2 cargo-deny warnings (RUSTSEC-2025-0057 + RUSTSEC-2025-0052, both already in ignore list).
- **Binary size**: +20 MB (BoringSSL is statically linked). Release build time: ~40s longer than v0.7.2 (BoringSSL compiles in).

## Migration notes (v0.7.1 → v0.7.2)

- **Zero breaking changes.** All CLI flags, JSON output schemas, and exit codes from v0.7.1 remain unchanged.
- **Security advisory fix (RUSTSEC-2026-0009)**: `time 0.3.40` denial-of-service via RFC 2822 stack exhaustion was being pulled in transitively via `cookie_store 0.22.0` → `reqwest 0.12.28`. v0.7.2 pins `time = "0.3.47"` as a direct dep to override the transitive constraint.
- **`rand` 0.10 migration**: dev-deps (proptest 1.11+, getrandom 0.4+) unified on rand 0.10 and the convenience methods moved from `Rng` to `RngExt`. All internal call sites updated: `random_range`, `random_bool`, `random`, and `IndexedRandom::choose`.
- **MSRV bump**: `rust-version` raised from 1.75 to 1.88 (required by `time 0.3.47+` and `rand 0.10`).
- **CI hygiene fix**: 6 latent clippy errors that were silently breaking the CI matrix in v0.7.1 are caught now by `cargo clippy --all-targets --all-features -- -D warnings`.

## Migration notes (v0.7.0 → v0.7.1)

- **Zero breaking changes.** All CLI flags, JSON output schemas, and exit codes from v0.7.0 remain unchanged.
- **Dependency migration (internal)**: `rand` bumped from `0.8` to `0.9` to align with `proptest 1.11+` (dev-dep). All internal call sites updated.
- **MSRV bump**: `rust-version` raised from `1.75` to `1.85` to satisfy `rand 0.9` MSRV and the wave of edition-2024 transitive deps.
- **reqwest builder cleanup**: removed `ClientBuilder::gzip(true)` and `.brotli(true)` calls.
- **CI hygiene**: two `actionlint` shellcheck warnings fixed.
- **Security advisory ignore**: `RUSTSEC-2026-0009` (time 0.3.40 DoS) added to `deny.toml` ignore list.
- **392 tests passing** (279 lib + 12 doc + 101 integration). 0 clippy warnings, 0 doc warnings, 0 fmt diff, 4 cargo-deny gates green, `cargo publish --dry-run` clean.

## Migration notes (v0.3.x → v0.4.0)

- `--num` now defaults to `15` (previously the full single-page payload, roughly 11). Scripts that processed "all results" continue to work — you just get a consistent count.
- When `--num > 10` and `--pages` is left at the default `1`, the CLI automatically raises `--pages` to `ceil(num / 10)` (capped at 5). Pass `--pages 1` explicitly to force a single page.
- JSON schema unchanged: `resultados[]`, `metadados`, `titulo_original` remain exactly as in v0.3.x.

See the [CHANGELOG](CHANGELOG.md) for release history.


## Migration notes (v0.6.4 → v0.6.5)

- **Zero breaking changes.** All CLI flags, JSON output schemas, and exit codes from v0.6.4 remain unchanged.
- **Windows build fixed (MP-26)**: `cargo install duckduckgo-search-cli` now succeeds on Windows. The v0.6.4 build broke on Windows because `windows-sys 0.59+` changed `HANDLE` from `isize` to `*mut c_void` and the existing code did `handle as isize` casts. v0.6.5 uses `!handle.is_null() && handle != INVALID_HANDLE_VALUE` instead.
- **CI matrix green again (CI-01)**: v0.6.4 was published with a failing CI on all 3 SOs due to 6 latent clippy errors. v0.6.5 fixes all of them and re-runs `cargo clippy --all-targets --all-features -- -D warnings` in CI.
- **No new CLI flags or JSON fields.** All v0.6.5 changes are internal or build/quality improvements.
- **One new transitive dependency**: `indicatif 0.18` (ProgressBar in long crawls; auto-hides in pipes).
- **WS-12 circuit breaker**: when `--fetch-content --parallel` is used, the new per-host circuit breaker opens after 3 consecutive failures and blocks requests to that host for 30 seconds before allowing a probe. This protects long crawls from cascading failures on a single dead domain.
- **333 tests passing** (243 unit + 90 integration + 6 doc). 6 clippy errors fixed, 5 new property tests, 4 new circuit breaker tests, 1 new wiremock Retry-After test.


## Migration notes (v0.6.3 → v0.6.4)

- **Zero breaking changes.** All CLI flags, JSON output schemas, and exit codes from v0.6.3 remain unchanged.
- **New CLI flags (additive)**:
  - `--probe` — sends one minimal pre-flight request and reports health as JSON
  - `--identity-profile` — pins the session to a specific identity from the 12-identity pool (`auto` by default for adaptive rotation)
  - `--seed` — now also controls identity pool rotation (was UA-only in v0.6.3)
- **New JSON metadata fields (additive, `skip_serializing_if = "Option::is_none"`)**:
  - `metadados.identidade_usada` — identity tag (`<family>-<platform>-<16hex>`) used for the response
  - `metadados.nivel_cascata` — cascade level (0..=4) reached during the request
- **Version note**: v0.7.0 was in development but rolled back to v0.6.4 to preserve the feature set under a stable patch number. The released binary is functionally identical to what would have been v0.7.0.


## v0.6.5 highlights (Windows HANDLE fix + CI green + circuit breaker)

v0.6.5 is a quality release focused on Windows portability and CI hygiene.
The biggest practical improvement is that **`cargo install duckduckgo-search-cli`
now works on Windows** for the first time since v0.6.4. The 6 latent clippy
errors that broke CI on all 3 SOs in v0.6.4 are also fixed.

- **MP-26 (CRITICAL)**: `src/platform.rs:51-69` rewritten to handle the
  `windows-sys 0.59+` ABI change (`HANDLE = *mut c_void`). Uses
  `INVALID_HANDLE_VALUE` from `windows_sys::Win32::Foundation` for the Win32
  sentinel and `is_null()` for the null-check.
- **CI-01**: 6 clippy errors fixed — `doc_markdown` on 3 strings
  (`PowerShell`, `rules_rust.md`, `TempDir`), `needless_return`,
  `missing_debug_implementations` on `ChromeBrowser` and `CircuitBreakerMap`.
  `cargo clippy --all-targets --all-features -- -D warnings` passes.
- **WS-12 circuit breaker**: per-host breaker in `src/content_fetch.rs`
  (3 failures → 30s cooldown). Protects `--fetch-content --parallel`
  crawls from cascading failures on dead domains.
- **WS-11 property tests**: 5 invariants in `src/extraction.rs` (empty
  inputs, dense positions, absolute URLs, idempotence, no panic on
  malformed HTML). Zero new dependencies.
- **WS-23 wiremock Retry-After**: integration test validates the 429
  backoff honors the `Retry-After: 2` header.
- **WS-25 indicatif ProgressBar**: `--fetch-content` shows a progress
  bar on stderr. Auto-hides in pipes (no contamination of stdout JSON).
- **Preventive FFI lints**: `improper_ctypes` and
  `improper_ctypes_definitions` are now `deny` in `Cargo.toml`, blocking
  future FFI type drift.
- **CI additions**: `--version --help` smoke test on all 3 SOs;
  `cargo build --no-default-features` job to validate the minimal build.


## v0.6.4 highlights (WS-26 anti-bot)

v0.6.4 introduces an adaptive anti-bot identity pool that addresses the root cause of HTTP 202/403/429 blocks from DuckDuckGo. The previous version selected a single User-Agent at startup and reused it for the entire session, producing a single fingerprint that anti-bot systems could classify after the first request. The new pool:

- Maintains 12 identities (4 browser families × 3 platforms: Windows, macOS, Linux)
- On detected block (HTTP 202/403/429), rotates through a 5-level cascade: same identity → same family/different platform → different family/same platform → different family+platform → random
- Produces seed-deterministic header order via `IdentityProfile::shuffled_headers()` (Accept-Language variants, Sec-CH-UA-Arch variations, randomized header order)
- Reports `identidade_usada` and `nivel_cascata` in NDJSON for diagnostic visibility

Usage:

```bash
# Default — adaptive rotation from 12 identities
duckduckgo-search-cli -q -n 10 -f json "query"

# Pin a specific identity for reproducible testing
duckduckgo-search-cli -q -n 10 -f json --identity-profile chrome-linux "query"

# Pre-flight health check before launching a real query
duckduckgo-search-cli --probe

# Deterministic seed for debugging anti-bot rotation
duckduckgo-search-cli -q -n 10 -f json --seed 42 "query"
```

License: MIT OR Apache-2.0.


## Português

### Por que isto existe

Toda LLM moderna carrega um corte de conhecimento, e todo agente autônomo eventualmente precisa de algo que seus pesos nunca viram: a versão recente de uma biblioteca, o post-mortem de um incidente de 2026, a página de preços atual de um fornecedor. Plugar uma API de busca hospedada custa dinheiro, vaza consultas e quebra no momento em que o vendor aplica rate-limit no meio de um plano de múltiplas etapas.

O `duckduckgo-search-cli` é um único binário Rust que transforma qualquer shell em ferramenta de busca de primeira classe. Sem API key. Sem tracking. Sem Chrome no caminho quente. Só um schema JSON estável, concorrência limitada e exit codes previsíveis — exatamente o que um agente precisa para se ancorar em dados reais da web sem virar ponto de falha.

### Superpoderes para cada agente de IA

Basta que o agente possa executar um comando de shell. Quase todo agente sério em produção hoje consegue.

| Agente                 | Como se beneficia                                                                                         |
| ---------------------- | --------------------------------------------------------------------------------------------------------- |
| Claude Code            | Ferramenta `Bash` chama `duckduckgo-search-cli "query" --num 15 -q \| jaq '.resultados'` antes de editar. |
| OpenAI Codex           | Acesso ao shell injeta o JSON estruturado no contexto para docs de biblioteca atualizadas.                |
| Gemini CLI             | Pipe do resultado para o modo JSON do Gemini para sintetizar fatos da cauda longa da web.                 |
| Cursor                 | Declarar o binário em `.cursorrules` e deixar a IA chamar quando faltar contexto.                         |
| Windsurf               | Agentes Cascade chamam o CLI como ferramenta determinística para dados frescos.                           |
| Aider                  | Use `/run` para injetar os resultados direto na conversa de edição.                                       |
| Continue.dev           | Registrar como slash-command customizado para ancorar completions na IDE.                                 |
| MiniMax                | Integração de shell dá aos agentes MiniMax cobertura global via DuckDuckGo.                               |
| OpenCode               | Integração de shell agnóstica de modelo — funciona com qualquer provider apontado no OpenCode.            |
| Paperclip              | Primitivo plug-and-play para pipelines autônomos de pesquisa.                                             |
| OpenClaw               | Runtime de agente open-source — usar como backend de busca padrão.                                        |
| Google Antigravity     | CLI headless como fallback para o agente de browser de superfície quando scraping direto é necessário.    |
| GitHub Copilot CLI     | Workflows `gh copilot` que exigem URLs verificadas e referências frescas.                                 |
| Devin                  | Engenheiro autônomo lê o JSON para planejar antes de tocar no código.                                     |
| Cline                  | Agente do VS Code chama o binário como comando de terminal para respostas ancoradas.                      |
| Roo Code               | Fork do Roo Cline — mesma integração de shell sem configuração.                                           |

### Por que é perfeito para agentes de IA

- **JSON-first por padrão.** Schema estável com `resultados[]` e `metadados`, ordem de campos congelada entre releases, pronto para `jaq` e parsing direto em chamadas de tool.
- **Zero API key, zero tracking.** Fala direto com o endpoint HTML do DuckDuckGo sobre HTTPS. Sem autenticação para rotacionar, sem dashboard para babá, sem superfície de vazamento.
- **Paralelismo nativo.** `--parallel 1..=20` distribui múltiplas queries via `tokio::JoinSet`, e `--per-host-limit` evita abuso em bursts quando `--fetch-content` está ligado.
- **15 resultados por padrão.** Contexto generoso para LLMs sem te obrigar a digitar `--num`. Sobrescreva por chamada quando precisar.
- **Auto-paginação que simplesmente funciona.** Quando `--num` supera uma página única do DuckDuckGo, o CLI crawla até 2 páginas automaticamente para entregar a contagem pedida.
- **Extração opcional de body legível.** `--fetch-content` baixa cada URL e embute texto limpo direto no JSON, limitado por `--max-content-length`.
- **Binário único cross-platform.** Linux (glibc, musl/Alpine), macOS Intel + Apple Silicon Universal, Windows MSVC — tudo a partir de um `cargo install`.
- **v0.7.3+: Fingerprint TLS real de navegador via BoringSSL (wreq).** BoringSSL é estaticamente vinculado e produz fingerprint JA4_o idêntico ao Chrome/Safari, eliminando o CAPTCHA do Cloudflare que afetava o macOS na v0.7.2. Build requer `cmake`, `perl`, `pkg-config` e `libclang-dev` no Linux. Ver `docs/decisions/0001-tls-boring-via-wreq.md` e `docs/CROSS_PLATFORM.md`.
- **Streaming NDJSON.** `--stream` emite uma linha por resultado no momento em que chega, alimentando pipelines reativos sem buffer da resposta completa.
- **Exit codes endurecidos.** Códigos distintos para erro de runtime, config inválida, soft rate-limit, timeout global e zero resultados — para o agente ramificar deterministicamente.
- **Anti-bloqueio v0.6.0.** Headers `Sec-Fetch-*` por família de browser, Client Hints para Chrome/Edge, detecção de HTTP 202 anomaly e detecção de bloqueio silencioso com limiar de 5 KB.

### Skill de agente — empacotada, bilíngue, auto-ativada

Pare de escrever system prompts lembrando seu agente de pesquisar. Este repo já entrega uma Claude Agent Skill pronta, e o Claude a dispara sozinho no instante em que o usuário fala em pesquisa, verificação, docs atualizadas ou grounding de URL — em menos de um segundo, sem engenharia de prompt.

- **Duas skills production-grade no repo.** `skill/duckduckgo-search-cli-en/SKILL.md` e `skill/duckduckgo-search-cli-pt/SKILL.md` — inglês e português brasileiro, cada uma com `name` único para coexistir na mesma instalação do Claude.
- **Auto-ativação de fábrica.** O campo `description` vem carregado com os triggers que o usuário realmente digita ("pesquise online", "verifique essa URL", "traga resultados atualizados", "search the web", "ground this"). O Claude casa por semântica — sem slash command, sem registro manual de tool.
- **14 seções canônicas MUST/NEVER por skill.** Contrato obrigatório `-q -f json`, parsing com `jaq`, exit codes determinísticos, batch, extração de conteúdo, fallback de endpoint, retries, validação pós-invocação — o agente lê uma vez e para de inventar flags para sempre.
- **Econômica em tokens.** Uma skill de ~1.000 palavras substitui um system prompt inchado. Carregada uma vez por sessão, referenciada sempre — economiza centenas de tokens em cada turno de busca.
- **Garantia anti-alucinação.** Cada flag que o agente pode invocar está documentada dentro da skill com contrato JSON congelado. Sem argumentos inventados, sem loops de retry, sem tool call desperdiçado.
- **Instalação em um comando.** Copie a pasta para o config do Claude e acabou — a skill mora no GitHub, fora do tarball do crates.io, então sempre puxe a versão mais fresca do `main`.

```bash
# Instalação direta (clone e copie a linguagem que preferir).
git clone https://github.com/daniloaguiarbr/duckduckgo-search-cli
cp -r duckduckgo-search-cli/skill/duckduckgo-search-cli-pt ~/.claude/skills/
cp -r duckduckgo-search-cli/skill/duckduckgo-search-cli-en ~/.claude/skills/

# Reinicie o Claude Code (ou recarregue o Agent SDK). É todo o setup.
```

### 📚 Documentação

Três guias técnicos profundos acompanham a crate. Leia uma vez — o retorno é vitalício.

| Guia | Por que importa |
|------|-----------------|
| [`docs/AGENT_RULES.md`](docs/AGENT_RULES.md) | 30+ regras DEVE/JAMAIS para qualquer LLM/agente invocar a CLI em produção. Bilíngue EN+PT. |
| [`docs/COOKBOOK.md`](docs/COOKBOOK.md) | 15 receitas copy-paste para pesquisa, ETL, monitoramento, extração de conteúdo. Bilíngue EN+PT. |
| [`docs/INTEGRATIONS.md`](docs/INTEGRATIONS.md) | Snippets prontos para 16 agentes: Claude Code, Codex, Gemini CLI, Cursor, Windsurf, Aider, Continue.dev, MiniMax, OpenCode, Paperclip, OpenClaw, Antigravity, Copilot CLI, Devin, Cline, Roo Code. |

### Início rápido

```bash
cargo install duckduckgo-search-cli
duckduckgo-search-cli "rust async runtime"
# 15 resultados JSON frescos na sua mesa.

# Para LLMs e agentes:
duckduckgo-search-cli "tokio JoinSet exemplos" --num 15 -q | jaq '.resultados'
```

### Receitas práticas

```bash
# 1. Extrair apenas URLs para um fetcher downstream.
duckduckgo-search-cli "site:example.com changelog 2025" --num 15 -f json \
  | jaq -r '.resultados[].url'

# 2. Enviar bodies limpos para um summarizer.
duckduckgo-search-cli "tokio runtime internals" --num 15 \
  --fetch-content --max-content-length 4000 -f json \
  | jaq -r '.resultados[] | "# \(.titulo)\n\(.conteudo)\n"' > corpus.md

# 3. Fan-out de múltiplas queries em uma única invocação.
duckduckgo-search-cli "rust rayon" "rust tokio" "rust crossbeam" \
  --num 15 --parallel 3 -f json

# 4. Streaming NDJSON para pipelines reativos.
duckduckgo-search-cli "wasm runtimes" --num 15 --stream \
  | jaq -r 'select(.url) | .url' \
  | xargs -I{} my-downloader {}

# 5. Passar por proxy corporativo (env vars também são respeitadas).
duckduckgo-search-cli "vendor status page 2026" --num 15 \
  --proxy http://user:pass@proxy.internal:8080 -f json

# 6. Teste offline sem rede real.
cargo test --test integration_wiremock
```

### Configuração

```bash
# Grava selectors.toml e user-agents.toml padrão no diretório XDG.
duckduckgo-search-cli init-config

# Dry-run primeiro para ver o que seria escrito.
duckduckgo-search-cli init-config --dry-run

# Sobrescrever arquivos existentes explicitamente.
duckduckgo-search-cli init-config --force
```

### Comandos

| Comando                                    | Propósito                                               |
| ------------------------------------------ | ------------------------------------------------------- |
| `duckduckgo-search-cli <QUERY>...`         | Busca padrão (equivalente a `buscar`).                  |
| `duckduckgo-search-cli buscar <QUERY>...`  | Subcommand explícito de busca.                          |
| `duckduckgo-search-cli init-config`        | Grava `selectors.toml` e `user-agents.toml` no XDG.     |

### Flags

| Flag                       | Padrão     | Descrição                                                          |
| -------------------------- | ---------- | ------------------------------------------------------------------ |
| `-n`, `--num`              | `15`       | Máximo de resultados por query (auto-pagina quando > 10).          |
| `-f`, `--format`           | `auto`     | `json`, `text`, `markdown` ou `auto` (detecta TTY).                |
| `-o`, `--output`           | stdout     | Grava no arquivo (diretórios criados, permissão Unix 0o644).       |
| `-t`, `--timeout`          | `15`       | Timeout por request (segundos).                                    |
| `--global-timeout`         | `60`       | Timeout global do pipeline (1..=3600 segundos).                    |
| `-l`, `--lang`             | `pt`       | Código de idioma `kl` do DuckDuckGo.                               |
| `-c`, `--country`          | `br`       | Código de país `kl` do DuckDuckGo.                                 |
| `-p`, `--parallel`         | `5`        | Requests concorrentes (1..=20).                                    |
| `--pages`                  | `1`        | Páginas por query (1..=5, auto-elevado por `--num`).               |
| `--retries`                | `2`        | Retries extras em 429/403/timeout (0..=10).                        |
| `--endpoint`               | `html`     | `html` ou `lite`.                                                  |
| `--time-filter`            | (nenhum)   | `d`, `w`, `m` ou `y`.                                              |
| `--safe-search`            | `moderate` | `off`, `moderate` ou `on`.                                         |
| `--stream`                 | off        | Emite uma linha NDJSON por resultado conforme chegam.              |
| `--fetch-content`          | off        | Baixa cada URL e adiciona texto limpo.                             |
| `--max-content-length`     | `10000`    | Limite de caracteres por body (1..=100_000).                       |
| `--per-host-limit`         | `2`        | Fetches concorrentes por host (1..=10).                            |
| `--proxy URL`              | (nenhum)   | Proxy HTTP/HTTPS/SOCKS5 (prevalece sobre env vars).                |
| `--no-proxy`               | off        | Desativa todas as fontes de proxy.                                 |
| `--queries-file PATH`      | (nenhum)   | Lê queries adicionais (uma por linha).                             |
| `--match-platform-ua`      | off        | Filtra UAs para o SO atual.                                        |
| `--chrome-path PATH`       | (auto)     | Caminho manual do Chrome (feature `chrome`).                       |
| `-v`, `--verbose`          | off        | Logs DEBUG em stderr.                                              |
| `-q`, `--quiet`            | off        | Apenas logs ERROR em stderr.                                       |

### Variáveis de ambiente

| Variável       | Descrição                                                       | Exemplo                            |
| -------------- | --------------------------------------------------------------- | ---------------------------------- |
| `RUST_LOG`     | Sobrescreve o filtro do `tracing-subscriber`.                   | `RUST_LOG=duckduckgo=debug`        |
| `HTTP_PROXY`   | Proxy HTTP padrão (prioridade menor que `--proxy`).             | `http://user:pass@proxy:8080`      |
| `HTTPS_PROXY`  | Proxy HTTPS padrão.                                             | `http://proxy:8443`                |
| `ALL_PROXY`    | Proxy fallback para qualquer scheme.                            | `socks5://127.0.0.1:9050`          |
| `CHROME_PATH`  | Caminho fallback para Chrome (feature `chrome`).                | `/opt/google/chrome/chrome`        |

### Formatos de saída

- `json` (default em pipes): schema canônico com `resultados[]` e `metadados`, ordem de campos estável. Cada resultado pode incluir o campo opcional `titulo_original` quando a heurística "Official site" substitui o título pelo `url_exibicao`. O endpoint `html.duckduckgo.com/html/` não expõe buscas relacionadas no DOM; a v0.3.0 removeu o campo `buscas_relacionadas`.
- `text`: bloco legível `NN. Título\n   URL\n   snippet`.
- `markdown`: `- [Título](URL)\n  > snippet`.
- Stream (`--stream`): NDJSON, cada linha é um resultado; metadados na linha final.

### Códigos de saída

| Código | Significado                                                    |
| ------ | -------------------------------------------------------------- |
| 0      | Sucesso.                                                       |
| 1      | Erro de runtime (rede, parse, I/O).                            |
| 2      | Configuração inválida (flag fora de faixa, proxy malformado).  |
| 3      | Bloqueio DuckDuckGo (anomalia HTTP 202).                       |
| 4      | Timeout global excedido.                                       |
| 5      | Zero resultados em todas as queries.                           |

### Troubleshooting

1. **HTTP 202 / bloqueio (exit 3)** — aumente `--retries`, rotacione UA via `init-config` editando `user-agents.toml`.
2. **Rate limit (HTTP 429)** — reduza `--per-host-limit`, ative `--match-platform-ua` ou use `--proxy`.
3. **Zero resultados (exit 5)** — confira `--lang` e `--country`, tente `--endpoint lite`, revise `--time-filter`.
4. **Chrome não encontrado** — instale Chromium pelo gerenciador de pacotes ou passe `--chrome-path /caminho/chrome`; a feature precisa ser compilada com `cargo install duckduckgo-search-cli --features chrome`.
5. **Problemas UTF-8 no Windows** — o binário muda cmd.exe para code page 65001 automaticamente; se ver mojibake, execute `chcp 65001` antes.
6. **Como integro com Claude Code, Cursor, Aider ou outro agente?** — exponha o binário como shell tool. A maioria dos agentes aceita um template de comando como `duckduckgo-search-cli "{query}" --num 15 -q -f json`. O schema estável mantém o contrato da tool estável entre releases.
7. **Pipe para jaq/jq retorna vazio** — verifique `echo ${PIPESTATUS[*]}` após o pipe. Se o primeiro número for diferente de zero, o CLI errou antes de produzir output. Causas comuns: rate-limiting do DuckDuckGo (exit 5), timeout global (exit 4) ou query ausente. Sempre passe `-q -f json` ao usar pipe.
8. **`--output` rejeita meu path (exit 2)** — v0.5.0 valida paths de saída antes de escrever. Paths contendo `..` são rejeitados para prevenir travessia de diretório. Paths apontando para diretórios de sistema (`/etc`, `/usr`, `/bin`, `C:\Windows`) são bloqueados. Use paths no seu diretório home, `/tmp` ou no diretório de trabalho atual.
9. **Exit 5 (zero resultados) com frequência** — normalmente é rate-limiting temporário do DuckDuckGo, não um bloqueio permanente. Aguarde 60 segundos e repita. Se persistir, adicione `--proxy socks5://127.0.0.1:9050` para rotacionar o IP, ou tente `--endpoint lite` como fallback. Os perfis de browser da v0.6.0 reduzem significativamente esse problema ao imitar sessões reais de navegador.
10. **`timeout 60 duckduckgo-search-cli -vv` retorna "the argument '--verbose' cannot be used multiple times"** — o binário `~/.cargo/bin/timeout` (crate Rust `timeout-cli` v0.1.0) sombreia o GNU coreutils e re-parseia os args do subprocesso, consumindo `-v` antes do clap. **Workaround**: use `/usr/bin/timeout` GNU explicitamente. Para diagnosticar qual `timeout` está no seu PATH: `command -v timeout` e `file $(command -v timeout)`. Script auxiliar em `scripts/detect-timeout-wrapper.sh`.

### Notas de migração (v0.3.x → v0.4.0)

- `--num` agora é `15` por padrão (antes era o payload completo de uma página, ~11). Scripts que processavam "todos os resultados" continuam funcionando — você só ganha uma contagem consistente.
- Quando `--num > 10` e `--pages` permanece no default `1`, o CLI eleva automaticamente `--pages` para `ceil(num / 10)` (limitado a 5). Passe `--pages 1` explicitamente para forçar uma única página.
- Schema JSON inalterado: `resultados[]`, `metadados` e `titulo_original` permanecem idênticos à v0.3.x.

Veja o [CHANGELOG](CHANGELOG.md) para o histórico completo.

Licença: MIT OR Apache-2.0.
