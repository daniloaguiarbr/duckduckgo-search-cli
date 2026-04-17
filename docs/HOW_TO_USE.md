# How to Use duckduckgo-search-cli

Real-time web search in your terminal — 15 fresh results in under 3 seconds.


## Why This Guide
- Follow this guide and run your first web search in under 60 seconds
- Learn core commands, advanced patterns, and shell pipeline integrations
- Understand every exit code and know exactly how to recover from each error


## Prerequisites
### Required
- Network access to duckduckgo.com
- Rust 1.75+ when installing via `cargo install`
- Pre-built binaries require no Rust installation
### Optional
- `jaq` (Rust jq replacement) for JSON processing in pipelines
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
