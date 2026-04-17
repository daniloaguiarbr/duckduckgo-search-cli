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

Upstream: https://github.com/daniloaguiarbr/duckduckgo-search-cli
Schema contract valid for `duckduckgo-search-cli` v0.6.x.
