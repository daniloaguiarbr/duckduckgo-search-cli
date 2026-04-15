# COOKBOOK / Livro de Receitas

> duckduckgo-search-cli — executable recipes that plug into any LLM pipeline in under 60 seconds.
> duckduckgo-search-cli — receitas executáveis que se integram a qualquer pipeline LLM em menos de 60 segundos.

## Table of Contents / Índice

### English Recipes
- [Recipe 01 — Top 5 results as CSV in 1 command](#recipe-01--top-5-results-as-csv-in-1-command)
- [Recipe 02 — Archived Markdown report to disk](#recipe-02--archived-markdown-report-to-disk)
- [Recipe 03 — Parallel multi-query research with dedup scoring](#recipe-03--parallel-multi-query-research-with-dedup-scoring)
- [Recipe 04 — Domain whitelist builder for RAG source filters](#recipe-04--domain-whitelist-builder-for-rag-source-filters)
- [Recipe 05 — Last-24h news monitor with timestamped snapshots](#recipe-05--last-24h-news-monitor-with-timestamped-snapshots)
- [Recipe 06 — Deep research payload ready for LLM context window](#recipe-06--deep-research-payload-ready-for-llm-context-window)
- [Recipe 07 — Rate-limited safe crawling below anti-abuse thresholds](#recipe-07--rate-limited-safe-crawling-below-anti-abuse-thresholds)
- [Recipe 08 — Proxy-routed search with leak verification](#recipe-08--proxy-routed-search-with-leak-verification)
- [Recipe 09 — Zero-noise pipeline for cron and systemd](#recipe-09--zero-noise-pipeline-for-cron-and-systemd)
- [Recipe 10 — Anti-bot block detector with exit code routing](#recipe-10--anti-bot-block-detector-with-exit-code-routing)
- [Recipe 11 — Breadth audit: top 5 vs top 15 coverage gap](#recipe-11--breadth-audit-top-5-vs-top-15-coverage-gap)
- [Recipe 12 — Side-by-side Markdown comparison of two queries](#recipe-12--side-by-side-markdown-comparison-of-two-queries)
- [Recipe 13 — NDJSON export for ClickHouse, BigQuery, and DuckDB](#recipe-13--ndjson-export-for-clickhouse-bigquery-and-duckdb)
- [Recipe 14 — Search-to-summarize pipeline with a local LLM](#recipe-14--search-to-summarize-pipeline-with-a-local-llm)
- [Recipe 15 — Bash function wrapper with opinionated safe defaults](#recipe-15--bash-function-wrapper-with-opinionated-safe-defaults)

### Receitas em Português
- [Receita 01 — Top 5 resultados como CSV em 1 comando](#receita-01--top-5-resultados-como-csv-em-1-comando)
- [Receita 02 — Relatório Markdown arquivado em disco](#receita-02--relatório-markdown-arquivado-em-disco)
- [Receita 03 — Pesquisa paralela multi-query com pontuação de deduplicação](#receita-03--pesquisa-paralela-multi-query-com-pontuação-de-deduplicação)
- [Receita 04 — Construtor de whitelist de domínios para filtros RAG](#receita-04--construtor-de-whitelist-de-domínios-para-filtros-rag)
- [Receita 05 — Monitor de notícias das últimas 24h com snapshots com timestamp](#receita-05--monitor-de-notícias-das-últimas-24h-com-snapshots-com-timestamp)
- [Receita 06 — Payload de pesquisa profunda pronto para a janela de contexto do LLM](#receita-06--payload-de-pesquisa-profunda-pronto-para-a-janela-de-contexto-do-llm)
- [Receita 07 — Crawling seguro com rate-limit abaixo de thresholds anti-abuso](#receita-07--crawling-seguro-com-rate-limit-abaixo-de-thresholds-anti-abuso)
- [Receita 08 — Busca via proxy com verificação de vazamento de IP](#receita-08--busca-via-proxy-com-verificação-de-vazamento-de-ip)
- [Receita 09 — Pipeline zero-ruído para cron e systemd](#receita-09--pipeline-zero-ruído-para-cron-e-systemd)
- [Receita 10 — Detector de bloqueio anti-bot com roteamento por exit code](#receita-10--detector-de-bloqueio-anti-bot-com-roteamento-por-exit-code)
- [Receita 11 — Auditoria de amplitude: gap de cobertura top 5 vs top 15](#receita-11--auditoria-de-amplitude-gap-de-cobertura-top-5-vs-top-15)
- [Receita 12 — Comparação Markdown lado-a-lado de duas queries](#receita-12--comparação-markdown-lado-a-lado-de-duas-queries)
- [Receita 13 — Exportação NDJSON para ClickHouse, BigQuery e DuckDB](#receita-13--exportação-ndjson-para-clickhouse-bigquery-e-duckdb)
- [Receita 14 — Pipeline busca-para-sumarização com LLM local](#receita-14--pipeline-busca-para-sumarização-com-llm-local)
- [Receita 15 — Função bash com defaults seguros e opinativos](#receita-15--função-bash-com-defaults-seguros-e-opinativos)

- [Recipe-to-Use-Case Table / Tabela Receita para Caso de Uso](#recipe-to-use-case-table--tabela-receita-para-caso-de-uso)

## ENGLISH RECIPES

### Recipe 01 — Top 5 results as CSV in 1 command
- Gain: extract 5 ranked title+URL pairs as CSV in under 200ms with no parser or scraper.
- Problem: LLM agents waste tokens parsing raw HTML or JSON into tabular form for downstream tools.
- Benefit: `-q` routes all tracing to stderr, leaving stdout as pure JSON for piping.
- Benefit: `jaq -r` emits CSV rows directly — no intermediate files, no extra dependencies.
- Benefit: `timeout 30` hard-caps the command against hung requests in CI pipelines.
- Result: paste-ready CSV rows consumable by any spreadsheet, ETL loader, or agent context.

```bash
timeout 30 duckduckgo-search-cli -q -n 5 -f json "rust async runtimes 2026" \
  | jaq -r '.resultados[] | [.posicao, .titulo, .url] | @csv'
```

Expected output:
```
1,"Tokio — asynchronous Rust runtime","https://tokio.rs/"
2,"async-std: Async version of the Rust standard library","https://async.rs/"
3,"smol — A small and fast async runtime for Rust","https://github.com/smol-rs/smol"
4,"Choosing an Async Runtime in Rust (2026 edition)","https://blog.rust-lang.org/..."
5,"Comparing Rust async runtimes","https://example.com/..."
```

### Recipe 02 — Archived Markdown report to disk
- Gain: produce a human-reviewable Markdown report for any query with 1 flag.
- Problem: teams lose research context when search results exist only in browser tabs.
- Benefit: `-o` creates parent directories and writes the report atomically to disk.
- Benefit: the `markdown` formatter generates PR-ready artifacts with titles, URLs, and snippets.
- Benefit: `-n 15` captures 3x more signal than the default top-5 view.
- Result: a durable `.md` file reviewable in GitHub, VS Code, or `glow` with zero post-processing.

```bash
timeout 45 duckduckgo-search-cli -q \
  -n 15 \
  -f markdown \
  -o reports/rust-webassembly.md \
  "rust webassembly edge computing"
```

Expected output:
```
(no stdout; file written)
$ bat -p reports/rust-webassembly.md | head -6
# Search results — rust webassembly edge computing
_Fetched: 2026-04-14T12:34:56Z — 15 results_

1. WASM on the edge with Rust — https://example.com/...
   > Short snippet describing the page...
```

### Recipe 03 — Parallel multi-query research with dedup scoring
- Gain: run 5 queries simultaneously and rank URLs by cross-query citation frequency in 1 pipeline.
- Problem: sequential queries miss which sources appear consistently across subtopics.
- Benefit: `--queries-file` plus `--parallel 5` fans out 5 searches while preserving per-host politeness.
- Benefit: the consolidated `buscas[]` array in the output JSON contains all results in 1 file.
- Benefit: `uniq -c | sort -rn` ranks URLs by how often they surface across queries.
- Result: a ranked list identifying canonical sources — the foundation for RAG source selection.

```bash
printf '%s\n' \
  "rust async runtimes" \
  "tokio vs async-std" \
  "rust runtime benchmark" \
  "rust executor design" \
  "glommio runtime" > /tmp/queries.txt

timeout 90 duckduckgo-search-cli -q \
  --queries-file /tmp/queries.txt \
  --parallel 5 \
  -n 10 \
  -f json \
  -o /tmp/multi.json

jaq -r '.buscas[].resultados[].url' /tmp/multi.json \
  | sort \
  | uniq -c \
  | sort -rn \
  | head -10
```

Expected output:
```
      4 https://tokio.rs/
      3 https://github.com/async-rs/async-std
      2 https://docs.rs/tokio/latest/tokio/
      1 https://blog.rust-lang.org/async-book
      1 https://github.com/smol-rs/smol
```

### Recipe 04 — Domain whitelist builder for RAG source filters
- Gain: extract a deduplicated list of trusted origin domains from any research topic in 1 pipeline.
- Problem: RAG systems ingest low-quality sources when no domain filter is applied.
- Benefit: `rg -oP` extracts scheme and host only — discards noisy path components.
- Benefit: `sort -u` yields a stable alphabetically sorted list suitable for policy files.
- Benefit: piping directly from stdout avoids writing intermediate result files.
- Result: a ready-to-use allow-list for LLM grounding, content policy, or document ingestion filters.

```bash
printf '%s\n' \
  "postgres tuning best practices" \
  "postgres vacuum autovacuum" \
  "postgres wal tuning" > /tmp/pg.txt

timeout 120 duckduckgo-search-cli -q \
  --queries-file /tmp/pg.txt \
  -n 20 \
  -f json \
  | jaq -r '.buscas[].resultados[].url' \
  | rg -oP '^https?://[^/]+' \
  | sort -u \
  > /tmp/pg-whitelist.txt

bat -p /tmp/pg-whitelist.txt
```

Expected output:
```
https://pgdash.io
https://postgresqlco.nf
https://wiki.postgresql.org
https://www.crunchydata.com
https://www.enterprisedb.com
https://www.postgresql.org
```

### Recipe 05 — Last-24h news monitor with timestamped snapshots
- Gain: capture a daily snapshot of last-24h results on any topic with rotation-safe filenames.
- Problem: cron jobs overwrite previous snapshots when filenames are static.
- Benefit: `--time-filter d` maps to DuckDuckGo's `df=d` parameter, restricting to the last 24 hours.
- Benefit: the `${STAMP}` variable in the filename prevents overwrites across invocations.
- Benefit: each JSON file is self-contained and queryable independently after the fact.
- Result: a rotating archive of dated snapshots ready for diff, trend analysis, or alerting workflows.

```bash
STAMP=$(date -u +%Y%m%dT%H%M%SZ)
mkdir -p /var/log/ddg-monitor

timeout 60 duckduckgo-search-cli -q \
  --time-filter d \
  -n 20 \
  -f json \
  -o /var/log/ddg-monitor/ai-safety-${STAMP}.json \
  "ai safety regulation"

jaq -r '.resultados[] | "\(.posicao). \(.titulo) — \(.url)"' \
  /var/log/ddg-monitor/ai-safety-${STAMP}.json \
  | head -5
```

Expected output:
```
1. EU AI Act enforcement begins — https://...
2. New AI safety benchmark released — https://...
3. Anthropic publishes interpretability update — https://...
4. OpenAI governance reshuffle — https://...
5. Senate hearing on frontier models — https://...
```

### Recipe 06 — Deep research payload ready for LLM context window
- Gain: fetch top 10 results with up to 5k chars of page content per result in 1 command.
- Problem: LLMs fed only snippets miss the detail needed for accurate synthesis.
- Benefit: `--fetch-content` populates the `conteudo` field with HTML-stripped plain text per result.
- Benefit: `--max-content-length 5000` caps token usage while preserving meaningful page content.
- Benefit: piping through `jaq` produces a `##`-sectioned Markdown file that fits directly into a context window.
- Result: an LLM-ready long-context payload with zero intermediate scrapers or browser sessions.

```bash
timeout 180 duckduckgo-search-cli -q \
  -n 10 \
  --fetch-content \
  --max-content-length 5000 \
  -f json \
  -o /tmp/deep.json \
  "differential privacy federated learning"

jaq -r '
  .resultados[]
  | "## \(.titulo)\nURL: \(.url)\n\n\(.conteudo // "(no content)")\n\n---"
' /tmp/deep.json > /tmp/llm-context.md

wc -l /tmp/llm-context.md
bat -p /tmp/llm-context.md | head -20
```

Expected output:
```
1243 /tmp/llm-context.md
## A Primer on Differential Privacy
URL: https://example.org/dp-primer

Differential privacy is a mathematical framework...
(up to 5000 chars)
---
```

### Recipe 07 — Rate-limited safe crawling below anti-abuse thresholds
- Gain: execute multi-query research without triggering anti-bot defenses, using 3 flags.
- Problem: parallel queries with no per-host limit hit DuckDuckGo's anti-abuse throttles.
- Benefit: `--parallel 2` limits concurrency to 2 simultaneous queries.
- Benefit: `--per-host-limit 1` enforces 1 in-flight request per host at a time.
- Benefit: `--retries 3` absorbs transient failures without operator intervention.
- Benefit: `--global-timeout 280` guarantees the whole job exits cleanly inside `timeout 300`.
- Result: polite research execution that survives long runs without triggering blocks.

```bash
timeout 300 duckduckgo-search-cli -q \
  --queries-file /tmp/sensitive.txt \
  --parallel 2 \
  --per-host-limit 1 \
  --retries 3 \
  --timeout 30 \
  --global-timeout 280 \
  -n 15 \
  -f json \
  -o /tmp/safe-research.json

jaq -r '.quantidade_queries, (.buscas[].metadados.tempo_execucao_ms)' /tmp/safe-research.json
```

Expected output:
```
5
1823
2104
1987
2231
1902
```

### Recipe 08 — Proxy-routed search with leak verification
- Gain: verify that all traffic routed through a SOCKS5 proxy with 1 authoritative JSON field.
- Problem: proxied tools often silently fall back to direct connections when the proxy is unreachable.
- Benefit: `metadados.usou_proxy` is set to `true` only when the proxy was wired into the HTTP client.
- Benefit: `false` is an unambiguous signal that the proxy never attached and the real IP is exposed.
- Benefit: `jaq` extracts only the 3 fields that matter — no parsing of the full result set needed.
- Result: one-liner proxy verification that doubles as a smoke test for any tunneled environment.

```bash
timeout 60 duckduckgo-search-cli -q \
  --proxy socks5://127.0.0.1:1080 \
  -n 10 \
  -f json \
  "geoip restricted content test" \
  | jaq '.metadados | {usou_proxy, user_agent, tempo_execucao_ms}'
```

Expected output:
```json
{
  "usou_proxy": true,
  "user_agent": "Mozilla/5.0 (...)",
  "tempo_execucao_ms": 2134
}
```

### Recipe 09 — Zero-noise pipeline for cron and systemd
- Gain: run hourly search snapshots unattended with clean exit codes and no log pollution.
- Problem: cron jobs that emit tracing noise pollute system logs and trigger false alerts.
- Benefit: `-q` routes all tracing to stderr and away from cron's stdout capture.
- Benefit: `--global-timeout` is set smaller than the outer `timeout` so the CLI exits cleanly.
- Benefit: the CLI exits with a meaningful code instead of being SIGKILL'd by the outer timer.
- Result: a silent hourly snapshot job that generates exit-code-observable audit artifacts.

```bash
# /etc/cron.d/ddg-snapshot
# 15 * * * * user timeout 120 duckduckgo-search-cli -q \
#   --queries-file /etc/ddg/watchlist.txt \
#   --global-timeout 110 \
#   --retries 2 \
#   -n 15 \
#   -f json \
#   -o /var/log/ddg/$(date -u +\%Y\%m\%dT\%H).json \
#   2>> /var/log/ddg/errors.log
```

Expected output:
```
(no stdout; hourly JSON snapshots land in /var/log/ddg/; errors, if any, append to errors.log)
```

### Recipe 10 — Anti-bot block detector with exit code routing
- Gain: distinguish HTTP-202 anti-bot blocks from real failures without parsing response bodies.
- Problem: generic error handling retries every failure the same way, wasting rate-limit budget.
- Benefit: exit code 3 is reserved exclusively for the HTTP-202 anti-bot signature.
- Benefit: routing on exit code 3 lets retry logic target only blocks, such as rotating a proxy.
- Benefit: exit codes 4 and 5 surface global timeouts and zero-results as separate observable states.
- Result: an observable shell function that logs each outcome class to the right destination.

```bash
run_ddg() {
  local q="$1"
  timeout 30 duckduckgo-search-cli -q -n 10 -f json "$q" > /tmp/out.json
  local ec=$?
  case $ec in
    0) echo "OK: $q" ;;
    3) echo "BLOCKED: $q" >&2 ;;
    4) echo "GLOBAL_TIMEOUT: $q" >&2 ;;
    5) echo "ZERO_RESULTS: $q" >&2 ;;
    *) echo "FAIL($ec): $q" >&2 ;;
  esac
  return $ec
}

run_ddg "legitimate query"
run_ddg "probably blocked bot-like query"
```

Expected output:
```
OK: legitimate query
BLOCKED: probably blocked bot-like query
```

### Recipe 11 — Breadth audit: top 5 vs top 15 coverage gap
- Gain: identify exactly which URLs a top-5 query misses compared to top-15 with set difference.
- Problem: defaulting to top-5 loses significant sources that rank between position 6 and 15.
- Benefit: two independent JSON files enable clean set comparison without any shared state.
- Benefit: `sort -u` normalizes both lists for `comm -13` to compute the exact set difference.
- Benefit: the output names only URLs unique to the broader set — no false positives.
- Result: an evidence-based audit that quantifies the breadth cost of a narrow `--num` setting.

```bash
Q="llm inference benchmarking"

timeout 30 duckduckgo-search-cli -q -n 5  -f json "$Q" > /tmp/top5.json
timeout 30 duckduckgo-search-cli -q -n 15 -f json "$Q" > /tmp/top15.json

jaq -r '.resultados[].url' /tmp/top5.json  | sort -u > /tmp/urls5.txt
jaq -r '.resultados[].url' /tmp/top15.json | sort -u > /tmp/urls15.txt

echo "=== Only in top 15 (missed at 5) ==="
comm -13 /tmp/urls5.txt /tmp/urls15.txt
```

Expected output:
```
=== Only in top 15 (missed at 5) ===
https://arxiv.org/abs/2404.12345
https://github.com/some-lab/llm-bench
https://huggingface.co/blog/...
...
```

### Recipe 12 — Side-by-side Markdown comparison of two queries
- Gain: render two queries as a Markdown table with matched ranks in 10 lines of shell.
- Problem: comparing two search strategies requires a visual side-by-side layout without a browser.
- Benefit: two independent JSON payloads keep the comparison portable and reproducible.
- Benefit: `jaq` indexed access extracts each title by rank position without any jq dependency.
- Benefit: the resulting table renders natively in GitHub, VS Code, and `glow` without extra tools.
- Result: a commit-ready Markdown comparison artifact produced in 1 pipeline run.

```bash
Q1="rust web framework axum"
Q2="rust web framework actix"

timeout 30 duckduckgo-search-cli -q -n 5 -f json "$Q1" > /tmp/a.json
timeout 30 duckduckgo-search-cli -q -n 5 -f json "$Q2" > /tmp/b.json

{
  echo "| # | $Q1 | $Q2 |"
  echo "|---|-----|-----|"
  for i in $(seq 1 5); do
    T1=$(jaq -r ".resultados[$((i-1))].titulo" /tmp/a.json)
    T2=$(jaq -r ".resultados[$((i-1))].titulo" /tmp/b.json)
    echo "| $i | $T1 | $T2 |"
  done
} > /tmp/compare.md

bat -p /tmp/compare.md
```

Expected output:
```
| # | rust web framework axum | rust web framework actix |
|---|-----|-----|
| 1 | Axum — ergonomic web framework | Actix Web — powerful, pragmatic |
| 2 | Getting started with Axum | Actix Web quickstart |
| 3 | Axum + Tower middleware | Actix-web middleware guide |
...
```

### Recipe 13 — NDJSON export for ClickHouse, BigQuery, and DuckDB
- Gain: flatten a multi-query run into one JSON object per line ready for direct `COPY FROM`.
- Problem: nested JSON arrays require transformation before ingestion into columnar data stores.
- Benefit: `jaq -c` emits compact one-object-per-line NDJSON — native format for bulk loaders.
- Benefit: the flattened schema includes `query` and `ts` fields for grouping and partitioning.
- Benefit: 10 queries at 15 results each produces exactly 150 lines — predictable for pipeline sizing.
- Result: a `.ndjson` file loadable into any columnar store with a single `COPY` statement.

```bash
timeout 120 duckduckgo-search-cli -q \
  --queries-file /tmp/etl-queries.txt \
  -n 15 \
  -f json \
  | jaq -c '
    .buscas[] as $b
    | $b.resultados[]
    | {
        query: $b.query,
        ts: $b.timestamp,
        posicao: .posicao,
        titulo: .titulo,
        url: .url,
        snippet: .snippet
      }
  ' > /tmp/results.ndjson

wc -l /tmp/results.ndjson
bat -p -r 1:3 /tmp/results.ndjson
```

Expected output:
```
150 /tmp/results.ndjson
{"query":"q1","ts":"2026-04-14T12:00:00Z","posicao":1,"titulo":"...","url":"...","snippet":"..."}
{"query":"q1","ts":"2026-04-14T12:00:00Z","posicao":2,"titulo":"...","url":"...","snippet":"..."}
{"query":"q1","ts":"2026-04-14T12:00:00Z","posicao":3,"titulo":"...","url":"...","snippet":"..."}
```

### Recipe 14 — Search-to-summarize pipeline with a local LLM
- Gain: transform a search query into a 5-bullet summarization grounded in fetched sources in 2 commands.
- Problem: local LLMs hallucinate without grounding context, but assembling that context requires a scraper.
- Benefit: `--fetch-content --max-content-length 3000` delivers HTML-stripped page text inside the JSON.
- Benefit: `jaq` shapes the multi-result JSON into the single string the OpenAI-style chat API expects.
- Benefit: `xh` handles JSON serialization of the request body automatically — no curl flags needed.
- Result: a grounded summarization pipeline from query to cited bullets with no browser or scraper.

```bash
timeout 60 duckduckgo-search-cli -q \
  -n 10 --fetch-content --max-content-length 3000 \
  -f json \
  "what is retrieval augmented generation" \
  > /tmp/rag.json

CONTEXT=$(jaq -r '[.resultados[] | "- \(.titulo): \(.conteudo // .snippet)"] | join("\n")' /tmp/rag.json)

timeout 60 xh POST http://127.0.0.1:11434/v1/chat/completions \
  model=llama3.1 \
  messages:='[
    {"role":"system","content":"Summarize the sources into 5 bullets. Cite URLs."},
    {"role":"user","content":"'"$CONTEXT"'"}
  ]' \
  | jaq -r '.choices[0].message.content'
```

Expected output:
```
- RAG combines retrieval + generation to ground LLMs with fresh context (https://...).
- Embeddings + vector DB are the canonical retrieval layer (https://...).
- Chunking strategy materially affects answer quality (https://...).
- Re-ranking improves precision@k before the LLM call (https://...).
- Evaluation typically uses answer faithfulness + context recall (https://...).
```

### Recipe 15 — Bash function wrapper with opinionated safe defaults
- Gain: encode timeout, retries, fetch-content, and JSON output into 1 reusable function call.
- Problem: operators forget safe flag combinations and produce hung or unreliable search runs.
- Benefit: the function hard-codes `--retries 3`, `--timeout 20`, and `--global-timeout 110` in one place.
- Benefit: `--fetch-content --max-content-length 8000` delivers deep content without extra commands.
- Benefit: the auto-timestamped filename prevents overwriting previous runs of the same query.
- Benefit: exit code pass-through enables upstream pipelines to branch on success or failure.
- Result: a repeatable, auditable, collision-free research command your team can trust in production.

```bash
# Add to ~/.bashrc or ~/.zshrc
ddg-deep() {
  local query="$*"
  if [ -z "$query" ]; then
    echo "usage: ddg-deep <query...>" >&2
    return 2
  fi
  local slug
  slug=$(echo "$query" | tr -cs '[:alnum:]' '-' | sed 's/-$//')
  local out="./ddg-${slug}-$(date -u +%Y%m%dT%H%M%SZ).json"
  timeout 120 duckduckgo-search-cli -q \
    -n 15 \
    --retries 3 \
    --timeout 20 \
    --global-timeout 110 \
    --fetch-content \
    --max-content-length 8000 \
    -f json \
    -o "$out" \
    "$query"
  local ec=$?
  if [ $ec -eq 0 ]; then
    echo "Saved: $out"
    jaq -r '.resultados[] | "\(.posicao). \(.titulo)"' "$out" | head -5
  else
    echo "ddg-deep failed with exit code $ec" >&2
  fi
  return $ec
}

# Usage:
ddg-deep "rust async runtime comparison 2026"
```

Expected output:
```
Saved: ./ddg-rust-async-runtime-comparison-2026-20260414T153000Z.json
1. Tokio — asynchronous Rust runtime
2. async-std: Async version of the Rust standard library
3. smol — A small and fast async runtime
4. Comparing async runtimes in Rust — 2026 edition
5. Glommio — thread-per-core runtime
```

## RECEITAS EM PORTUGUÊS

### Receita 01 — Top 5 resultados como CSV em 1 comando
- Ganho: extraia 5 pares título+URL ranqueados como CSV em menos de 200ms sem parser nem scraper.
- Problema: agentes LLM desperdiçam tokens parseando JSON bruto em formato tabular para ferramentas downstream.
- Benefício: `-q` direciona todo o tracing para stderr, deixando stdout como JSON puro para pipe.
- Benefício: `jaq -r` emite linhas CSV diretamente — sem arquivos intermediários, sem dependências extras.
- Benefício: `timeout 30` limita o comando com precisão contra requisições travadas em pipelines de CI.
- Resultado: linhas CSV prontas para colar, consumíveis por qualquer planilha, carregador ETL ou contexto de agente.

```bash
timeout 30 duckduckgo-search-cli -q -n 5 -f json "rust async runtimes 2026" \
  | jaq -r '.resultados[] | [.posicao, .titulo, .url] | @csv'
```

Saída esperada:
```
1,"Tokio — runtime assíncrono para Rust","https://tokio.rs/"
2,"async-std: Versão assíncrona da std","https://async.rs/"
3,"smol — runtime async pequeno e rápido","https://github.com/smol-rs/smol"
4,"Escolhendo um runtime async em Rust (2026)","https://blog.rust-lang.org/..."
5,"Comparando runtimes async em Rust","https://example.com/..."
```

### Receita 02 — Relatório Markdown arquivado em disco
- Ganho: gere um relatório Markdown revisável por humanos para qualquer query com 1 flag.
- Problema: equipes perdem contexto de pesquisa quando os resultados existem apenas em abas do navegador.
- Benefício: `-o` cria diretórios pai e grava o relatório atomicamente em disco.
- Benefício: o formatter `markdown` gera artefatos prontos para PR com títulos, URLs e snippets.
- Benefício: `-n 15` captura 3x mais sinal do que a visualização padrão de top-5.
- Resultado: um arquivo `.md` durável revisável no GitHub, VS Code ou `glow` sem pós-processamento.

```bash
timeout 45 duckduckgo-search-cli -q \
  -n 15 \
  -f markdown \
  -o reports/rust-webassembly.md \
  "rust webassembly edge computing"
```

Saída esperada:
```
(sem stdout; arquivo gravado)
$ bat -p reports/rust-webassembly.md | head -6
# Search results — rust webassembly edge computing
_Fetched: 2026-04-14T12:34:56Z — 15 results_

1. WASM na borda com Rust — https://example.com/...
   > Snippet curto descrevendo a página...
```

### Receita 03 — Pesquisa paralela multi-query com pontuação de deduplicação
- Ganho: execute 5 queries simultaneamente e ranqueie URLs por frequência de citação cruzada em 1 pipeline.
- Problema: queries sequenciais perdem quais fontes aparecem consistentemente entre subtópicos.
- Benefício: `--queries-file` com `--parallel 5` faz fan-out de 5 buscas preservando a polidez por host.
- Benefício: o array `buscas[]` no JSON de saída contém todos os resultados em 1 único arquivo consolidado.
- Benefício: `uniq -c | sort -rn` ranqueia URLs pela frequência com que aparecem entre as queries.
- Resultado: uma lista ranqueada identificando fontes canônicas — a base para seleção de fontes em RAG.

```bash
printf '%s\n' \
  "rust async runtimes" \
  "tokio vs async-std" \
  "rust runtime benchmark" \
  "rust executor design" \
  "glommio runtime" > /tmp/queries.txt

timeout 90 duckduckgo-search-cli -q \
  --queries-file /tmp/queries.txt \
  --parallel 5 \
  -n 10 \
  -f json \
  -o /tmp/multi.json

jaq -r '.buscas[].resultados[].url' /tmp/multi.json \
  | sort \
  | uniq -c \
  | sort -rn \
  | head -10
```

Saída esperada:
```
      4 https://tokio.rs/
      3 https://github.com/async-rs/async-std
      2 https://docs.rs/tokio/latest/tokio/
      1 https://blog.rust-lang.org/async-book
      1 https://github.com/smol-rs/smol
```

### Receita 04 — Construtor de whitelist de domínios para filtros RAG
- Ganho: extraia uma lista deduplicada de domínios de origem confiáveis de qualquer tópico de pesquisa em 1 pipeline.
- Problema: sistemas RAG ingerem fontes de baixa qualidade quando nenhum filtro de domínio é aplicado.
- Benefício: `rg -oP` extrai apenas esquema e host — descarta componentes de path ruidosos.
- Benefício: `sort -u` gera uma lista estável ordenada alfabeticamente adequada para arquivos de política.
- Benefício: o pipe direto do stdout evita gravar arquivos de resultado intermediários.
- Resultado: uma allow-list pronta para uso para grounding de LLM, política de conteúdo ou filtros de ingestão de documentos.

```bash
printf '%s\n' \
  "postgres tuning best practices" \
  "postgres vacuum autovacuum" \
  "postgres wal tuning" > /tmp/pg.txt

timeout 120 duckduckgo-search-cli -q \
  --queries-file /tmp/pg.txt \
  -n 20 \
  -f json \
  | jaq -r '.buscas[].resultados[].url' \
  | rg -oP '^https?://[^/]+' \
  | sort -u \
  > /tmp/pg-whitelist.txt

bat -p /tmp/pg-whitelist.txt
```

Saída esperada:
```
https://pgdash.io
https://postgresqlco.nf
https://wiki.postgresql.org
https://www.crunchydata.com
https://www.enterprisedb.com
https://www.postgresql.org
```

### Receita 05 — Monitor de notícias das últimas 24h com snapshots com timestamp
- Ganho: capture um snapshot diário dos resultados das últimas 24h de qualquer tópico com nomes de arquivo seguros para rotação.
- Problema: jobs de cron sobrescrevem snapshots anteriores quando os nomes de arquivo são estáticos.
- Benefício: `--time-filter d` mapeia para o parâmetro `df=d` do DuckDuckGo, restringindo às últimas 24 horas.
- Benefício: a variável `${STAMP}` no nome do arquivo impede sobrescrita entre invocações.
- Benefício: cada arquivo JSON é autocontido e consultável independentemente após o fato.
- Resultado: um arquivo rotativo de snapshots com data pronto para diff, análise de tendências ou workflows de alerta.

```bash
STAMP=$(date -u +%Y%m%dT%H%M%SZ)
mkdir -p /var/log/ddg-monitor

timeout 60 duckduckgo-search-cli -q \
  --time-filter d \
  -n 20 \
  -f json \
  -o /var/log/ddg-monitor/ai-safety-${STAMP}.json \
  "ai safety regulation"

jaq -r '.resultados[] | "\(.posicao). \(.titulo) — \(.url)"' \
  /var/log/ddg-monitor/ai-safety-${STAMP}.json \
  | head -5
```

Saída esperada:
```
1. Início da aplicação do AI Act na UE — https://...
2. Novo benchmark de segurança em IA divulgado — https://...
3. Anthropic publica atualização sobre interpretabilidade — https://...
4. Reestruturação na governança da OpenAI — https://...
5. Audiência no Senado sobre modelos de fronteira — https://...
```

### Receita 06 — Payload de pesquisa profunda pronto para a janela de contexto do LLM
- Ganho: busque os 10 primeiros resultados com até 5k caracteres de conteúdo de página por resultado em 1 comando.
- Problema: LLMs alimentados apenas com snippets perdem o detalhe necessário para síntese precisa.
- Benefício: `--fetch-content` popula o campo `conteudo` com texto sem HTML por resultado.
- Benefício: `--max-content-length 5000` limita o uso de tokens preservando conteúdo significativo da página.
- Benefício: o pipe pelo `jaq` produz um arquivo Markdown seccionado com `##` que cabe diretamente em uma janela de contexto.
- Resultado: um payload de contexto longo pronto para LLM sem scrapers intermediários nem sessões de navegador.

```bash
timeout 180 duckduckgo-search-cli -q \
  -n 10 \
  --fetch-content \
  --max-content-length 5000 \
  -f json \
  -o /tmp/deep.json \
  "differential privacy federated learning"

jaq -r '
  .resultados[]
  | "## \(.titulo)\nURL: \(.url)\n\n\(.conteudo // "(sem conteúdo)")\n\n---"
' /tmp/deep.json > /tmp/llm-context.md

wc -l /tmp/llm-context.md
bat -p /tmp/llm-context.md | head -20
```

Saída esperada:
```
1243 /tmp/llm-context.md
## Uma introdução à privacidade diferencial
URL: https://example.org/dp-primer

Privacidade diferencial é um framework matemático...
(até 5000 caracteres)
---
```

### Receita 07 — Crawling seguro com rate-limit abaixo de thresholds anti-abuso
- Ganho: execute pesquisa multi-query sem acionar defesas anti-bot usando 3 flags.
- Problema: queries paralelas sem limite por host atingem os throttles anti-abuso do DuckDuckGo.
- Benefício: `--parallel 2` limita a concorrência a 2 queries simultâneas.
- Benefício: `--per-host-limit 1` garante 1 requisição em voo por host por vez.
- Benefício: `--retries 3` absorve falhas transitórias sem intervenção do operador.
- Benefício: `--global-timeout 280` garante que o job inteiro encerra limpo dentro do `timeout 300`.
- Resultado: execução de pesquisa polida que sobrevive a execuções longas sem acionar bloqueios.

```bash
timeout 300 duckduckgo-search-cli -q \
  --queries-file /tmp/sensitive.txt \
  --parallel 2 \
  --per-host-limit 1 \
  --retries 3 \
  --timeout 30 \
  --global-timeout 280 \
  -n 15 \
  -f json \
  -o /tmp/safe-research.json

jaq -r '.quantidade_queries, (.buscas[].metadados.tempo_execucao_ms)' /tmp/safe-research.json
```

Saída esperada:
```
5
1823
2104
1987
2231
1902
```

### Receita 08 — Busca via proxy com verificação de vazamento de IP
- Ganho: verifique que todo o tráfego foi roteado por um proxy SOCKS5 com 1 campo JSON autoritativo.
- Problema: ferramentas com proxy frequentemente voltam silenciosamente para conexões diretas quando o proxy está inacessível.
- Benefício: `metadados.usou_proxy` só vai para `true` quando o proxy foi de fato conectado ao cliente HTTP.
- Benefício: `false` é um sinal inequívoco de que o proxy nunca foi conectado e o IP real vazou.
- Benefício: `jaq` extrai apenas os 3 campos que importam — sem parsing do conjunto de resultados completo.
- Resultado: verificação de proxy em uma linha que serve como smoke test para qualquer ambiente tunelado.

```bash
timeout 60 duckduckgo-search-cli -q \
  --proxy socks5://127.0.0.1:1080 \
  -n 10 \
  -f json \
  "teste de conteudo restrito por geoip" \
  | jaq '.metadados | {usou_proxy, user_agent, tempo_execucao_ms}'
```

Saída esperada:
```json
{
  "usou_proxy": true,
  "user_agent": "Mozilla/5.0 (...)",
  "tempo_execucao_ms": 2134
}
```

### Receita 09 — Pipeline zero-ruído para cron e systemd
- Ganho: execute snapshots de busca horários sem supervisão com exit codes limpos e sem poluição de log.
- Problema: jobs de cron que emitem ruído de tracing poluem logs do sistema e acionam alertas falsos.
- Benefício: `-q` direciona todo o tracing para stderr e para longe da captura de stdout do cron.
- Benefício: `--global-timeout` é definido menor que o `timeout` externo para que a CLI encerre limpa.
- Benefício: a CLI encerra com um exit code significativo em vez de ser SIGKILL'd pelo timer externo.
- Resultado: um job de snapshot silencioso por hora que gera artefatos de auditoria observáveis por exit code.

```bash
# /etc/cron.d/ddg-snapshot
# 15 * * * * user timeout 120 duckduckgo-search-cli -q \
#   --queries-file /etc/ddg/watchlist.txt \
#   --global-timeout 110 \
#   --retries 2 \
#   -n 15 \
#   -f json \
#   -o /var/log/ddg/$(date -u +\%Y\%m\%dT\%H).json \
#   2>> /var/log/ddg/errors.log
```

Saída esperada:
```
(sem stdout; snapshots JSON horários aterrissam em /var/log/ddg/; erros, se houver, acumulam em errors.log)
```

### Receita 10 — Detector de bloqueio anti-bot com roteamento por exit code
- Ganho: distinga bloqueios HTTP-202 anti-bot de falhas reais sem parsear corpos de resposta.
- Problema: tratamento de erro genérico retenta toda falha da mesma forma, desperdiçando orçamento de rate-limit.
- Benefício: exit code 3 é reservado exclusivamente para a assinatura anti-bot HTTP-202.
- Benefício: rotear no exit code 3 permite que a lógica de retry direcione apenas bloqueios, como rotacionar proxy.
- Benefício: exit codes 4 e 5 surfaciam timeouts globais e zero resultados como estados observáveis separados.
- Resultado: uma função shell observável que registra cada classe de resultado no destino correto.

```bash
run_ddg() {
  local q="$1"
  timeout 30 duckduckgo-search-cli -q -n 10 -f json "$q" > /tmp/out.json
  local ec=$?
  case $ec in
    0) echo "OK: $q" ;;
    3) echo "BLOQUEADO: $q" >&2 ;;
    4) echo "TIMEOUT_GLOBAL: $q" >&2 ;;
    5) echo "ZERO_RESULTADOS: $q" >&2 ;;
    *) echo "FALHA($ec): $q" >&2 ;;
  esac
  return $ec
}

run_ddg "query legítima"
run_ddg "query provavelmente bloqueada que parece bot"
```

Saída esperada:
```
OK: query legítima
BLOQUEADO: query provavelmente bloqueada que parece bot
```

### Receita 11 — Auditoria de amplitude: gap de cobertura top 5 vs top 15
- Ganho: identifique exatamente quais URLs uma query top-5 perde em comparação ao top-15 com diferença de conjuntos.
- Problema: definir o padrão para top-5 perde fontes significativas que ranqueiam entre as posições 6 e 15.
- Benefício: dois arquivos JSON independentes permitem comparação limpa de conjuntos sem estado compartilhado.
- Benefício: `sort -u` normaliza ambas as listas para que `comm -13` calcule a diferença exata de conjuntos.
- Benefício: a saída nomeia apenas URLs únicas no conjunto mais amplo — sem falsos positivos.
- Resultado: uma auditoria baseada em evidências que quantifica o custo de amplitude de uma configuração `--num` estreita.

```bash
Q="llm inference benchmarking"

timeout 30 duckduckgo-search-cli -q -n 5  -f json "$Q" > /tmp/top5.json
timeout 30 duckduckgo-search-cli -q -n 15 -f json "$Q" > /tmp/top15.json

jaq -r '.resultados[].url' /tmp/top5.json  | sort -u > /tmp/urls5.txt
jaq -r '.resultados[].url' /tmp/top15.json | sort -u > /tmp/urls15.txt

echo "=== Apenas no top 15 (perdidos no top 5) ==="
comm -13 /tmp/urls5.txt /tmp/urls15.txt
```

Saída esperada:
```
=== Apenas no top 15 (perdidos no top 5) ===
https://arxiv.org/abs/2404.12345
https://github.com/some-lab/llm-bench
https://huggingface.co/blog/...
...
```

### Receita 12 — Comparação Markdown lado-a-lado de duas queries
- Ganho: renderize duas queries como uma tabela Markdown com ranks correspondentes em 10 linhas de shell.
- Problema: comparar duas estratégias de busca requer um layout visual lado-a-lado sem navegador.
- Benefício: dois payloads JSON independentes mantêm a comparação portátil e reproduzível.
- Benefício: o acesso indexado via `jaq` extrai cada título por posição de rank sem dependência de jq.
- Benefício: a tabela resultante é renderizada nativamente no GitHub, VS Code e `glow` sem ferramentas extras.
- Resultado: um artefato de comparação Markdown pronto para commit produzido em 1 execução de pipeline.

```bash
Q1="rust web framework axum"
Q2="rust web framework actix"

timeout 30 duckduckgo-search-cli -q -n 5 -f json "$Q1" > /tmp/a.json
timeout 30 duckduckgo-search-cli -q -n 5 -f json "$Q2" > /tmp/b.json

{
  echo "| # | $Q1 | $Q2 |"
  echo "|---|-----|-----|"
  for i in $(seq 1 5); do
    T1=$(jaq -r ".resultados[$((i-1))].titulo" /tmp/a.json)
    T2=$(jaq -r ".resultados[$((i-1))].titulo" /tmp/b.json)
    echo "| $i | $T1 | $T2 |"
  done
} > /tmp/compare.md

bat -p /tmp/compare.md
```

Saída esperada:
```
| # | rust web framework axum | rust web framework actix |
|---|-----|-----|
| 1 | Axum — framework web ergonômico | Actix Web — poderoso e pragmático |
| 2 | Começando com Axum | Quickstart do Actix Web |
| 3 | Axum + middleware Tower | Guia de middleware do Actix-web |
...
```

### Receita 13 — Exportação NDJSON para ClickHouse, BigQuery e DuckDB
- Ganho: achate uma execução multi-query em 1 objeto JSON por linha pronto para `COPY FROM` direto.
- Problema: arrays JSON aninhados requerem transformação antes da ingestão em datastores colunares.
- Benefício: `jaq -c` emite NDJSON compacto com 1 objeto por linha — formato nativo para loaders em massa.
- Benefício: o schema achatado inclui campos `query` e `ts` para agrupamento e particionamento.
- Benefício: 10 queries com 15 resultados cada produz exatamente 150 linhas — previsível para dimensionamento de pipeline.
- Resultado: um arquivo `.ndjson` carregável em qualquer store colunar com um único comando `COPY`.

```bash
timeout 120 duckduckgo-search-cli -q \
  --queries-file /tmp/etl-queries.txt \
  -n 15 \
  -f json \
  | jaq -c '
    .buscas[] as $b
    | $b.resultados[]
    | {
        query: $b.query,
        ts: $b.timestamp,
        posicao: .posicao,
        titulo: .titulo,
        url: .url,
        snippet: .snippet
      }
  ' > /tmp/results.ndjson

wc -l /tmp/results.ndjson
bat -p -r 1:3 /tmp/results.ndjson
```

Saída esperada:
```
150 /tmp/results.ndjson
{"query":"q1","ts":"2026-04-14T12:00:00Z","posicao":1,"titulo":"...","url":"...","snippet":"..."}
{"query":"q1","ts":"2026-04-14T12:00:00Z","posicao":2,"titulo":"...","url":"...","snippet":"..."}
{"query":"q1","ts":"2026-04-14T12:00:00Z","posicao":3,"titulo":"...","url":"...","snippet":"..."}
```

### Receita 14 — Pipeline busca-para-sumarização com LLM local
- Ganho: transforme uma query de busca em uma sumarização de 5 bullets ancorada em fontes buscadas em 2 comandos.
- Problema: LLMs locais alucinam sem contexto de grounding, mas montar esse contexto requer um scraper.
- Benefício: `--fetch-content --max-content-length 3000` entrega texto de página sem HTML dentro do JSON.
- Benefício: `jaq` formata o JSON multi-resultado na string única que a API de chat estilo OpenAI espera.
- Benefício: `xh` cuida da serialização JSON do corpo da requisição automaticamente — sem flags de curl.
- Resultado: um pipeline de sumarização ancorada de query para bullets com citações sem navegador nem scraper.

```bash
timeout 60 duckduckgo-search-cli -q \
  -n 10 --fetch-content --max-content-length 3000 \
  -f json \
  "o que é retrieval augmented generation" \
  > /tmp/rag.json

CONTEXT=$(jaq -r '[.resultados[] | "- \(.titulo): \(.conteudo // .snippet)"] | join("\n")' /tmp/rag.json)

timeout 60 xh POST http://127.0.0.1:11434/v1/chat/completions \
  model=llama3.1 \
  messages:='[
    {"role":"system","content":"Resuma as fontes em 5 bullets. Cite URLs."},
    {"role":"user","content":"'"$CONTEXT"'"}
  ]' \
  | jaq -r '.choices[0].message.content'
```

Saída esperada:
```
- RAG combina retrieval + geração para ancorar LLMs com contexto fresco (https://...).
- Embeddings + banco vetorial são a camada canônica de retrieval (https://...).
- Estratégia de chunking afeta materialmente a qualidade da resposta (https://...).
- Re-ranking aumenta a precisão@k antes da chamada ao LLM (https://...).
- Avaliação tipicamente usa faithfulness + context recall (https://...).
```

### Receita 15 — Função bash com defaults seguros e opinativos
- Ganho: codifique timeout, retries, fetch-content e saída JSON em 1 chamada de função reutilizável.
- Problema: operadores esquecem combinações seguras de flags e produzem execuções de busca travadas ou não confiáveis.
- Benefício: a função codifica `--retries 3`, `--timeout 20` e `--global-timeout 110` em um único lugar.
- Benefício: `--fetch-content --max-content-length 8000` entrega conteúdo profundo sem comandos extras.
- Benefício: o nome de arquivo com timestamp automático impede sobrescrita de execuções anteriores da mesma query.
- Benefício: o repasse do exit code permite que pipelines upstream ramifiquem em sucesso ou falha.
- Resultado: um comando de pesquisa repetível, auditável e sem colisão em que sua equipe pode confiar em produção.

```bash
# Adicionar ao ~/.bashrc ou ~/.zshrc
ddg-deep() {
  local query="$*"
  if [ -z "$query" ]; then
    echo "uso: ddg-deep <query...>" >&2
    return 2
  fi
  local slug
  slug=$(echo "$query" | tr -cs '[:alnum:]' '-' | sed 's/-$//')
  local out="./ddg-${slug}-$(date -u +%Y%m%dT%H%M%SZ).json"
  timeout 120 duckduckgo-search-cli -q \
    -n 15 \
    --retries 3 \
    --timeout 20 \
    --global-timeout 110 \
    --fetch-content \
    --max-content-length 8000 \
    -f json \
    -o "$out" \
    "$query"
  local ec=$?
  if [ $ec -eq 0 ]; then
    echo "Salvo: $out"
    jaq -r '.resultados[] | "\(.posicao). \(.titulo)"' "$out" | head -5
  else
    echo "ddg-deep falhou com exit code $ec" >&2
  fi
  return $ec
}

# Uso:
ddg-deep "comparação de runtimes async em rust 2026"
```

Saída esperada:
```
Salvo: ./ddg-comparacao-de-runtimes-async-em-rust-2026-20260414T153000Z.json
1. Tokio — runtime assíncrono para Rust
2. async-std: Versão assíncrona da std
3. smol — runtime async pequeno e rápido
4. Comparando runtimes async em Rust — edição 2026
5. Glommio — runtime thread-per-core
```

## Recipe-to-Use-Case Table / Tabela Receita para Caso de Uso

| Recipe / Receita | Use case / Caso de uso | Tools used / Ferramentas |
|---|---|---|
| 01 | Triagem rápida top-N em uma linha / Fast top-N triage one-liner | `duckduckgo-search-cli`, `jaq`, `timeout` |
| 02 | Relatório Markdown arquivado / Archived Markdown report | `duckduckgo-search-cli`, `bat`, `timeout` |
| 03 | Pesquisa multi-query consolidada / Consolidated multi-query research | `duckduckgo-search-cli`, `jaq`, `sort`, `uniq`, `timeout` |
| 04 | Construção de whitelist de domínios / Domain whitelist build | `duckduckgo-search-cli`, `jaq`, `rg`, `sort`, `bat`, `timeout` |
| 05 | Monitoramento 24h agendado / Scheduled 24h monitor | `duckduckgo-search-cli`, `jaq`, `date`, `timeout` |
| 06 | Contexto longo para RAG/LLM / Long context for RAG/LLM | `duckduckgo-search-cli --fetch-content`, `jaq`, `bat`, `timeout` |
| 07 | Crawling polido rate-limited / Polite rate-limited crawling | `duckduckgo-search-cli`, `jaq`, `timeout` |
| 08 | Verificação de roteamento por proxy / Proxy routing verification | `duckduckgo-search-cli --proxy`, `jaq`, `timeout` |
| 09 | Snapshot horário não-supervisionado / Unattended hourly snapshot | `duckduckgo-search-cli`, `cron`/`systemd`, `timeout` |
| 10 | Observabilidade de bloqueios anti-bot / Anti-bot block observability | `duckduckgo-search-cli` (exit code 3), `bash case`, `timeout` |
| 11 | Auditoria de amplitude de resultados / Result breadth audit | `duckduckgo-search-cli`, `jaq`, `comm`, `sort`, `timeout` |
| 12 | Comparação A/B em Markdown / Markdown A/B comparison | `duckduckgo-search-cli`, `jaq`, `bat`, `timeout` |
| 13 | Exportação NDJSON para ETL / NDJSON export for ETL | `duckduckgo-search-cli`, `jaq -c`, `bat`, `timeout` |
| 14 | Pipeline busca para sumarização com LLM / Search-to-summarize LLM pipeline | `duckduckgo-search-cli --fetch-content`, `jaq`, `xh`, `timeout` |
| 15 | Defaults opinativos reutilizáveis / Reusable opinionated defaults | `duckduckgo-search-cli`, função bash, `jaq`, `date`, `timeout` |

_End of COOKBOOK / Fim do Livro de Receitas._
