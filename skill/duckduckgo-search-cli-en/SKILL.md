---
name: duckduckgo-search-cli-en
description: Use this skill WHENEVER the user asks for web search, internet research, up-to-date documentation lookup, factual grounding, URL verification, page content extraction, external evidence gathering, RAG enrichment, fact-checking, library version lookup, incident post-mortem, current vendor pricing, or any data outside the knowledge cutoff. Triggers include "search the web", "ground this", "web search", "fetch URL content", "look this up online", "verify this URL", "get current results". Invokes the `duckduckgo-search-cli` v0.5.0 CLI via Bash with a stable JSON contract, zero API key, path traversal validation on --output, and automatic credential masking in error messages. English version.
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
- NEVER invoke without `timeout` — pipelines hang indefinitely.

```bash
timeout 60 duckduckgo-search-cli "<query>" -q -f json --num 15 | jaq '.resultados'
```

## Absolute Prohibitions
- FORBIDDEN to use `-f text` or `-f markdown` for programmatic parsing.
- FORBIDDEN to omit `-q` in any pipeline that reads stdout.
- FORBIDDEN to use `--stream` — flag reserved, NOT implemented in v0.5.0.
- FORBIDDEN to raise `--parallel` above 5 without outbound IP control.
- FORBIDDEN to raise `--per-host-limit` above 2 — triggers HTTP 202 anti-bot.
- FORBIDDEN to retry in shell loops — use native `--retries` with exponential backoff.
- FORBIDDEN to hardcode API keys, proxies, or User-Agents in arguments.
- FORBIDDEN to assume `snippet`, `url_exibicao`, `titulo_original` are always present.
- FORBIDDEN to pass `--output` with `..` in the path — v0.5.0 rejects path traversal
- FORBIDDEN to pass `--output` targeting `/etc`, `/usr`, or `C:\Windows` — system dirs blocked

## Mandatory JSON Parsing with jaq
- ALWAYS use `jaq` (NEVER `jq`) to process JSON output.
- ALWAYS apply `// ""` fallback on optional fields.
- ALWAYS distinguish single-query root (`.resultados`) from multi-query root (`.buscas[]`).
- MUST extract latency via `.metadados.tempo_execucao_ms` for observability.
- MUST monitor `.metadados.usou_endpoint_fallback` to detect IP degradation.

```bash
timeout 60 duckduckgo-search-cli "rust async runtime" -q -f json --num 15 \
  | jaq '.resultados[] | {
      posicao,
      titulo,
      url,
      snippet: (.snippet // ""),
      url_exibicao: (.url_exibicao // .url)
    }'
```

## Guaranteed vs Optional JSON Fields
- GUARANTEED non-null: `.query`, `.resultados[].posicao`, `.resultados[].titulo`, `.resultados[].url`.
- OPTIONAL `Option<String>`: `.resultados[].snippet`, `.resultados[].url_exibicao`, `.resultados[].titulo_original`.
- METADATA always present: `.metadados.tempo_execucao_ms`, `.metadados.quantidade_resultados`, `.metadados.usou_endpoint_fallback`.
- CONDITIONAL on `--fetch-content`: `.resultados[].conteudo`, `.tamanho_conteudo`, `.metodo_extracao_conteudo`.

## Deterministic Exit Codes
- Exit 0: success — parse stdout with `jaq`.
- Exit 1: runtime error — read stderr and report to the user.
- Exit 2: CLI argument error — fix flags before retrying.
- Exit 3: anti-bot block HTTP 202 — wait 300s and switch to `--endpoint lite`.
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

## Golden Rule
- When in doubt between hallucinating and invoking the CLI, ALWAYS invoke the CLI.
- Cost of one invocation is 60-300ms. Cost of hallucination is rework and loss of trust.
- ALWAYS prefer verified data with URL over plausible assumption without source.


## Security Guarantees (v0.5.0)
- `--output` validates paths BEFORE writing — `..` and system directories rejected automatically
- Proxy credentials in `--proxy` URLs NEVER appear in error messages or stderr
- Credential masking transforms `http://user:pass@host` into `http://us***@host` in all error output
- Agents generate dynamic filenames without manual path validation — the CLI rejects unsafe paths
- SIGPIPE restored on Unix — pipes to `jaq`, `head`, `wc` terminate cleanly without EPIPE errors
- BrokenPipe detected in error chain — returns exit 0 instead of propagating as exit 1
- Typed errors via `ErroCliDdg` enum — 11 variants with deterministic `exit_code()` mapping
