# COOKBOOK / Livro de Receitas

> **duckduckgo-search-cli** — executable recipes for real-world pipelines.
> **duckduckgo-search-cli** — receitas executáveis para pipelines do mundo real.

Every recipe is a paste-and-run command. No concepts. No theory. Just wiring that works.
Cada receita é um comando pronto para colar. Sem conceitos. Sem teoria. Apenas encanamento que funciona.

---

## Table of Contents / Índice

### English Recipes
- [Recipe 01 — Quick research (top 5 as CSV)](#recipe-01--quick-research-top-5-as-csv)
- [Recipe 02 — Markdown report to file](#recipe-02--markdown-report-to-file)
- [Recipe 03 — Multi-query parallel research with dedup](#recipe-03--multi-query-parallel-research-with-dedup)
- [Recipe 04 — Domain whitelist extraction](#recipe-04--domain-whitelist-extraction)
- [Recipe 05 — Time-filtered news monitoring (last 24h)](#recipe-05--time-filtered-news-monitoring-last-24h)
- [Recipe 06 — Deep research with content extraction for LLM context](#recipe-06--deep-research-with-content-extraction-for-llm-context)
- [Recipe 07 — Rate-limited safe research](#recipe-07--rate-limited-safe-research)
- [Recipe 08 — Proxy-routed research with verification](#recipe-08--proxy-routed-research-with-verification)
- [Recipe 09 — Quiet pipeline for cron / systemd](#recipe-09--quiet-pipeline-for-cron--systemd)
- [Recipe 10 — Detect blocked queries (exit code 3)](#recipe-10--detect-blocked-queries-exit-code-3)
- [Recipe 11 — Compare top 5 vs top 15 URL sets](#recipe-11--compare-top-5-vs-top-15-url-sets)
- [Recipe 12 — Markdown side-by-side comparison of two queries](#recipe-12--markdown-side-by-side-comparison-of-two-queries)
- [Recipe 13 — JSON Lines (NDJSON) for ETL ingestion](#recipe-13--json-lines-ndjson-for-etl-ingestion)
- [Recipe 14 — Search and summarize with a local LLM](#recipe-14--search-and-summarize-with-a-local-llm)
- [Recipe 15 — Bash function wrapper `ddg-deep`](#recipe-15--bash-function-wrapper-ddg-deep)

### Receitas em Português
- [Receita 01 — Pesquisa rápida (top 5 como CSV)](#receita-01--pesquisa-rápida-top-5-como-csv)
- [Receita 02 — Relatório Markdown em arquivo](#receita-02--relatório-markdown-em-arquivo)
- [Receita 03 — Pesquisa paralela multi-query com deduplicação](#receita-03--pesquisa-paralela-multi-query-com-deduplicação)
- [Receita 04 — Extração de whitelist de domínios](#receita-04--extração-de-whitelist-de-domínios)
- [Receita 05 — Monitoramento de notícias filtrado por tempo (últimas 24h)](#receita-05--monitoramento-de-notícias-filtrado-por-tempo-últimas-24h)
- [Receita 06 — Pesquisa profunda com extração de conteúdo para contexto de LLM](#receita-06--pesquisa-profunda-com-extração-de-conteúdo-para-contexto-de-llm)
- [Receita 07 — Pesquisa segura com rate-limit](#receita-07--pesquisa-segura-com-rate-limit)
- [Receita 08 — Pesquisa via proxy com verificação](#receita-08--pesquisa-via-proxy-com-verificação)
- [Receita 09 — Pipeline silencioso para cron / systemd](#receita-09--pipeline-silencioso-para-cron--systemd)
- [Receita 10 — Detectar queries bloqueadas (exit code 3)](#receita-10--detectar-queries-bloqueadas-exit-code-3)
- [Receita 11 — Comparar conjuntos top 5 vs top 15](#receita-11--comparar-conjuntos-top-5-vs-top-15)
- [Receita 12 — Comparação lado-a-lado em Markdown de duas queries](#receita-12--comparação-lado-a-lado-em-markdown-de-duas-queries)
- [Receita 13 — JSON Lines (NDJSON) para ingestão ETL](#receita-13--json-lines-ndjson-para-ingestão-etl)
- [Receita 14 — Busca e sumarização com LLM local](#receita-14--busca-e-sumarização-com-llm-local)
- [Receita 15 — Função bash `ddg-deep`](#receita-15--função-bash-ddg-deep)

- [Recipe-to-Use-Case Table / Tabela Receita → Caso de Uso](#recipe-to-use-case-table--tabela-receita--caso-de-uso)

---

## ENGLISH RECIPES

### Recipe 01 — Quick research (top 5 as CSV)

**Goal:** Grab the top 5 titles + URLs for a single query and pipe as CSV.

**Command:**
```bash
timeout 30 duckduckgo-search-cli -q -n 5 -f json "rust async runtimes 2026" \
  | jaq -r '.resultados[] | [.posicao, .titulo, .url] | @csv'
```

**Expected output:**
```
1,"Tokio — asynchronous Rust runtime","https://tokio.rs/"
2,"async-std: Async version of the Rust standard library","https://async.rs/"
3,"smol — A small and fast async runtime for Rust","https://github.com/smol-rs/smol"
4,"Choosing an Async Runtime in Rust (2026 edition)","https://blog.rust-lang.org/..."
5,"Comparing Rust async runtimes","https://example.com/..."
```

**Why this works:** `-q` silences tracing so stdout is pure JSON; `jaq -r` emits raw CSV rows without outer quoting. `timeout 30` guards against hung requests.

---

### Recipe 02 — Markdown report to file

**Goal:** Produce a clean Markdown report for a single query, written to disk.

**Command:**
```bash
timeout 45 duckduckgo-search-cli -q \
  -n 15 \
  -f markdown \
  -o reports/rust-webassembly.md \
  "rust webassembly edge computing"
```

**Expected output:**
```
(no stdout; file written)
$ bat -p reports/rust-webassembly.md | head -6
# Search results — rust webassembly edge computing
_Fetched: 2026-04-14T12:34:56Z — 15 results_

1. **WASM on the edge with Rust** — https://example.com/...
   > Short snippet describing the page...
```

**Why this works:** `-o` creates parent directories and writes with Unix `0o644`. The `markdown` formatter produces a human-reviewable artifact suitable for PR descriptions or status reports.

---

### Recipe 03 — Multi-query parallel research with dedup

**Goal:** Run five queries in parallel, deduplicate URLs across all results, count occurrences per URL.

**Command:**
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

**Expected output:**
```
      4 https://tokio.rs/
      3 https://github.com/async-rs/async-std
      2 https://docs.rs/tokio/latest/tokio/
      1 https://blog.rust-lang.org/async-book
      1 https://github.com/smol-rs/smol
```

**Why this works:** `--queries-file` plus `--parallel 5` fans out with per-host politeness preserved. The consolidated JSON has the `buscas[]` array; `jaq` flattens results and `uniq -c` gives URL frequency — great for spotting canonical sources.

---

### Recipe 04 — Domain whitelist extraction

**Goal:** Build a whitelist of trustworthy domains from N queries on a topic.

**Command:**
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

**Expected output:**
```
https://pgdash.io
https://postgresqlco.nf
https://wiki.postgresql.org
https://www.crunchydata.com
https://www.enterprisedb.com
https://www.postgresql.org
```

**Why this works:** `rg -oP` extracts origin (scheme + host) only; `sort -u` yields a stable unique list — the raw material for policy files, allow-lists, or RAG source filters.

---

### Recipe 05 — Time-filtered news monitoring (last 24h)

**Goal:** Every morning, fetch last-24h results on a topic and save timestamped JSON.

**Command:**
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

**Expected output:**
```
1. EU AI Act enforcement begins — https://...
2. New AI safety benchmark released — https://...
3. Anthropic publishes interpretability update — https://...
4. OpenAI governance reshuffle — https://...
5. Senate hearing on frontier models — https://...
```

**Why this works:** `--time-filter d` restricts to last 24 hours (DuckDuckGo's `df=d`). The timestamp-named file enables trivial cron/systemd rotation with no overwrites.

---

### Recipe 06 — Deep research with content extraction for LLM context

**Goal:** Fetch top 10 results AND extract up to 5k chars of content per page, ready to feed an LLM.

**Command:**
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

**Expected output:**
```
1243 /tmp/llm-context.md
## A Primer on Differential Privacy
URL: https://example.org/dp-primer

Differential privacy is a mathematical framework...
(up to 5000 chars)
---
```

**Why this works:** `--fetch-content` populates the `conteudo` field per result with HTML-stripped text capped by `--max-content-length`. Piping into a `.md` produces LLM-ready long-context payload — no intermediate scraper needed.

---

### Recipe 07 — Rate-limited safe research

**Goal:** Execute sensitive queries politely — minimal parallelism, single request per host, conservative retries.

**Command:**
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

**Expected output:**
```
5
1823
2104
1987
2231
1902
```

**Why this works:** `--parallel 2 --per-host-limit 1` keeps you below anti-abuse thresholds; `--retries 3` smooths transient failures; `--global-timeout 280` guarantees the whole job fits inside `timeout 300` safely.

---

### Recipe 08 — Proxy-routed research with verification

**Goal:** Route all traffic via a SOCKS5 proxy and verify the run really used it.

**Command:**
```bash
timeout 60 duckduckgo-search-cli -q \
  --proxy socks5://127.0.0.1:1080 \
  -n 10 \
  -f json \
  "geoip restricted content test" \
  | jaq '.metadados | {usou_proxy, user_agent, tempo_execucao_ms}'
```

**Expected output:**
```json
{
  "usou_proxy": true,
  "user_agent": "Mozilla/5.0 (...)",
  "tempo_execucao_ms": 2134
}
```

**Why this works:** `metadados.usou_proxy` is set to `true` only when the proxy was actually wired into the HTTP client — it is the authoritative signal. If you see `false`, the proxy never attached and you are leaking your real IP.

---

### Recipe 09 — Quiet pipeline for cron / systemd

**Goal:** Unsupervised execution — no tracing, hard time cap, durable output.

**Command:**
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

**Expected output:**
```
(no stdout; hourly JSON snapshots land in /var/log/ddg/; errors, if any, append to errors.log)
```

**Why this works:** `-q` removes tracing noise that pollutes cron logs; `--global-timeout` is smaller than the outer `timeout` so the CLI exits cleanly with a meaningful exit code instead of being SIGKILL'd.

---

### Recipe 10 — Detect blocked queries (exit code 3)

**Goal:** Distinguish real failures from HTTP-202 anti-bot blocks and log them separately.

**Command:**
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

**Expected output:**
```
OK: legitimate query
BLOCKED: probably blocked bot-like query
```

**Why this works:** Exit code 3 is reserved specifically for the HTTP-202 anti-bot signature. Branching on it lets retries target only blocks (e.g., rotate proxy) instead of spraying every error path.

---

### Recipe 11 — Compare top 5 vs top 15 URL sets

**Goal:** Quantify which URLs appear in the top 15 but would have been missed at top 5.

**Command:**
```bash
Q="llm inference benchmarking"

timeout 30 duckduckgo-search-cli -q -n 5  -f json "$Q" > /tmp/top5.json
timeout 30 duckduckgo-search-cli -q -n 15 -f json "$Q" > /tmp/top15.json

jaq -r '.resultados[].url' /tmp/top5.json  | sort -u > /tmp/urls5.txt
jaq -r '.resultados[].url' /tmp/top15.json | sort -u > /tmp/urls15.txt

echo "=== Only in top 15 (missed at 5) ==="
comm -13 /tmp/urls5.txt /tmp/urls15.txt
```

**Expected output:**
```
=== Only in top 15 (missed at 5) ===
https://arxiv.org/abs/2404.12345
https://github.com/some-lab/llm-bench
https://huggingface.co/blog/...
...
```

**Why this works:** `sort -u` normalizes for `comm -13` which prints lines unique to the second file — a clean set-difference telling you exactly what breadth you gain by widening `--num`.

---

### Recipe 12 — Markdown side-by-side comparison of two queries

**Goal:** Build a comparison `.md` with two queries rendered as columns.

**Command:**
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

**Expected output:**
```
| # | rust web framework axum | rust web framework actix |
|---|-----|-----|
| 1 | Axum — ergonomic web framework | Actix Web — powerful, pragmatic |
| 2 | Getting started with Axum | Actix Web quickstart |
| 3 | Axum + Tower middleware | Actix-web middleware guide |
...
```

**Why this works:** Two independent JSON payloads plus shell loop with `jaq` indexing yields a Markdown table without any table library — universally renderable in GitHub, VS Code, `glow`, etc.

---

### Recipe 13 — JSON Lines (NDJSON) for ETL ingestion

**Goal:** Flatten a multi-query run into one result per line (NDJSON), ready for ClickHouse / BigQuery / DuckDB COPY.

**Command:**
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

**Expected output:**
```
150 /tmp/results.ndjson
{"query":"q1","ts":"2026-04-14T12:00:00Z","posicao":1,"titulo":"...","url":"...","snippet":"..."}
{"query":"q1","ts":"2026-04-14T12:00:00Z","posicao":2,"titulo":"...","url":"...","snippet":"..."}
{"query":"q1","ts":"2026-04-14T12:00:00Z","posicao":3,"titulo":"...","url":"...","snippet":"..."}
```

**Why this works:** `jaq -c` emits compact one-object-per-line NDJSON — native format for `COPY FROM` in most columnar stores. The shape is flat with the query attached for grouping.

---

### Recipe 14 — Search and summarize with a local LLM

**Goal:** Pipe a consolidated JSON to a local OpenAI-compatible LLM (e.g., `llama.cpp server`, Ollama) for summarization.

**Command:**
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

**Expected output:**
```
- RAG combines retrieval + generation to ground LLMs with fresh context (https://...).
- Embeddings + vector DB are the canonical retrieval layer (https://...).
- Chunking strategy materially affects answer quality (https://...).
- Re-ranking improves precision@k before the LLM call (https://...).
- Evaluation typically uses answer faithfulness + context recall (https://...).
```

**Why this works:** The CLI produces structured JSON with optional fetched content; `jaq` shapes it into the single string the OpenAI-style API wants. `xh` handles JSON encoding automatically.

---

### Recipe 15 — Bash function wrapper `ddg-deep`

**Goal:** Reusable function that applies safe defaults (timeout, retries, sane `--num`, JSON output, auto-timestamped file).

**Command:**
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

**Expected output:**
```
Saved: ./ddg-rust-async-runtime-comparison-2026-20260414T153000Z.json
1. Tokio — asynchronous Rust runtime
2. async-std: Async version of the Rust standard library
3. smol — A small and fast async runtime
4. Comparing async runtimes in Rust — 2026 edition
5. Glommio — thread-per-core runtime
```

**Why this works:** A function encodes your opinionated defaults in one place. Every invocation inherits the safe timeout / retry / `--global-timeout` combo without the operator having to remember them.

---

## RECEITAS EM PORTUGUÊS

### Receita 01 — Pesquisa rápida (top 5 como CSV)

**Objetivo:** Pegar os 5 primeiros títulos + URLs de uma query e cuspir como CSV.

**Comando:**
```bash
timeout 30 duckduckgo-search-cli -q -n 5 -f json "rust async runtimes 2026" \
  | jaq -r '.resultados[] | [.posicao, .titulo, .url] | @csv'
```

**Saída esperada:**
```
1,"Tokio — runtime assíncrono para Rust","https://tokio.rs/"
2,"async-std: Versão assíncrona da std","https://async.rs/"
3,"smol — runtime async pequeno e rápido","https://github.com/smol-rs/smol"
4,"Escolhendo um runtime async em Rust (2026)","https://blog.rust-lang.org/..."
5,"Comparando runtimes async em Rust","https://example.com/..."
```

**Por que funciona:** `-q` silencia o tracing e deixa o stdout como JSON puro; `jaq -r` emite CSV bruto sem aspas externas. `timeout 30` protege contra requisições travadas.

---

### Receita 02 — Relatório Markdown em arquivo

**Objetivo:** Gerar um relatório Markdown limpo para uma query, gravado em disco.

**Comando:**
```bash
timeout 45 duckduckgo-search-cli -q \
  -n 15 \
  -f markdown \
  -o reports/rust-webassembly.md \
  "rust webassembly edge computing"
```

**Saída esperada:**
```
(sem stdout; arquivo gravado)
$ bat -p reports/rust-webassembly.md | head -6
# Search results — rust webassembly edge computing
_Fetched: 2026-04-14T12:34:56Z — 15 results_

1. **WASM na borda com Rust** — https://example.com/...
   > Snippet curto descrevendo a página...
```

**Por que funciona:** `-o` cria diretórios pai e grava com `0o644` no Unix. O formatter `markdown` gera um artefato revisável por humanos — bom para descrições de PR e relatórios de status.

---

### Receita 03 — Pesquisa paralela multi-query com deduplicação

**Objetivo:** Rodar cinco queries em paralelo, deduplicar URLs entre todos os resultados e contar ocorrências.

**Comando:**
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

**Saída esperada:**
```
      4 https://tokio.rs/
      3 https://github.com/async-rs/async-std
      2 https://docs.rs/tokio/latest/tokio/
      1 https://blog.rust-lang.org/async-book
      1 https://github.com/smol-rs/smol
```

**Por que funciona:** `--queries-file` com `--parallel 5` faz fan-out preservando a polidez por host. O JSON consolidado tem o array `buscas[]`; `jaq` achata os resultados e `uniq -c` dá a frequência por URL — ótimo para identificar fontes canônicas.

---

### Receita 04 — Extração de whitelist de domínios

**Objetivo:** Construir uma whitelist de domínios confiáveis a partir de N queries sobre um tema.

**Comando:**
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

**Saída esperada:**
```
https://pgdash.io
https://postgresqlco.nf
https://wiki.postgresql.org
https://www.crunchydata.com
https://www.enterprisedb.com
https://www.postgresql.org
```

**Por que funciona:** `rg -oP` extrai apenas a origem (esquema + host); `sort -u` gera uma lista única e estável — matéria-prima para arquivos de política, allow-lists ou filtros de fontes RAG.

---

### Receita 05 — Monitoramento de notícias filtrado por tempo (últimas 24h)

**Objetivo:** Toda manhã, buscar resultados das últimas 24 horas sobre um tema e salvar JSON com timestamp.

**Comando:**
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

**Saída esperada:**
```
1. Início da aplicação do AI Act na UE — https://...
2. Novo benchmark de segurança em IA divulgado — https://...
3. Anthropic publica atualização sobre interpretabilidade — https://...
4. Reestruturação na governança da OpenAI — https://...
5. Audiência no Senado sobre modelos de fronteira — https://...
```

**Por que funciona:** `--time-filter d` restringe às últimas 24 horas (`df=d` no DuckDuckGo). O arquivo com timestamp no nome simplifica rotação em cron/systemd sem sobrescrita.

---

### Receita 06 — Pesquisa profunda com extração de conteúdo para contexto de LLM

**Objetivo:** Buscar os 10 primeiros resultados E extrair até 5k caracteres de conteúdo por página, prontos para alimentar um LLM.

**Comando:**
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

**Saída esperada:**
```
1243 /tmp/llm-context.md
## Uma introdução à privacidade diferencial
URL: https://example.org/dp-primer

Privacidade diferencial é um framework matemático...
(até 5000 caracteres)
---
```

**Por que funciona:** `--fetch-content` popula o campo `conteudo` por resultado com texto HTML-stripped limitado por `--max-content-length`. Jogando em um `.md`, você obtém payload de contexto longo pronto para LLM — sem scraper intermediário.

---

### Receita 07 — Pesquisa segura com rate-limit

**Objetivo:** Executar queries sensíveis com polidez — paralelismo mínimo, 1 requisição por host, retries conservadores.

**Comando:**
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

**Saída esperada:**
```
5
1823
2104
1987
2231
1902
```

**Por que funciona:** `--parallel 2 --per-host-limit 1` mantém o volume abaixo de thresholds anti-abuso; `--retries 3` absorve falhas transitórias; `--global-timeout 280` garante que o job inteiro caiba dentro do `timeout 300` externo com folga.

---

### Receita 08 — Pesquisa via proxy com verificação

**Objetivo:** Rotear todo o tráfego via proxy SOCKS5 e verificar que o run realmente usou.

**Comando:**
```bash
timeout 60 duckduckgo-search-cli -q \
  --proxy socks5://127.0.0.1:1080 \
  -n 10 \
  -f json \
  "teste de conteudo restrito por geoip" \
  | jaq '.metadados | {usou_proxy, user_agent, tempo_execucao_ms}'
```

**Saída esperada:**
```json
{
  "usou_proxy": true,
  "user_agent": "Mozilla/5.0 (...)",
  "tempo_execucao_ms": 2134
}
```

**Por que funciona:** `metadados.usou_proxy` só vai para `true` quando o proxy foi de fato plugado no cliente HTTP — é o sinal autoritativo. Se vier `false`, o proxy não engatou e o IP real vazou.

---

### Receita 09 — Pipeline silencioso para cron / systemd

**Objetivo:** Execução não-supervisionada — sem tracing, corte duro de tempo, output durável.

**Comando:**
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

**Saída esperada:**
```
(sem stdout; snapshots JSON horários aterrissam em /var/log/ddg/; erros, se houver, acumulam em errors.log)
```

**Por que funciona:** `-q` elimina o ruído de tracing que polui logs de cron; `--global-timeout` é menor que o `timeout` externo, então a CLI encerra limpa com exit code significativo em vez de levar SIGKILL.

---

### Receita 10 — Detectar queries bloqueadas (exit code 3)

**Objetivo:** Distinguir falhas reais de bloqueios anti-bot (HTTP 202) e logar separadamente.

**Comando:**
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

**Saída esperada:**
```
OK: query legítima
BLOQUEADO: query provavelmente bloqueada que parece bot
```

**Por que funciona:** O exit code 3 é reservado exclusivamente para a assinatura HTTP-202 anti-bot. Ramificar nele permite direcionar retries só para bloqueios (ex: rotacionar proxy) em vez de disparar fallback para todo tipo de erro.

---

### Receita 11 — Comparar conjuntos top 5 vs top 15

**Objetivo:** Quantificar quais URLs aparecem no top 15 e seriam perdidas no top 5.

**Comando:**
```bash
Q="llm inference benchmarking"

timeout 30 duckduckgo-search-cli -q -n 5  -f json "$Q" > /tmp/top5.json
timeout 30 duckduckgo-search-cli -q -n 15 -f json "$Q" > /tmp/top15.json

jaq -r '.resultados[].url' /tmp/top5.json  | sort -u > /tmp/urls5.txt
jaq -r '.resultados[].url' /tmp/top15.json | sort -u > /tmp/urls15.txt

echo "=== Apenas no top 15 (perdidos no top 5) ==="
comm -13 /tmp/urls5.txt /tmp/urls15.txt
```

**Saída esperada:**
```
=== Apenas no top 15 (perdidos no top 5) ===
https://arxiv.org/abs/2404.12345
https://github.com/some-lab/llm-bench
https://huggingface.co/blog/...
...
```

**Por que funciona:** `sort -u` normaliza a entrada de `comm -13`, que imprime apenas as linhas exclusivas do segundo arquivo — uma diferença de conjuntos limpa que mostra exatamente quanto alcance adicional se ganha ampliando `--num`.

---

### Receita 12 — Comparação lado-a-lado em Markdown de duas queries

**Objetivo:** Construir um `.md` de comparação com duas queries renderizadas em colunas.

**Comando:**
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

**Saída esperada:**
```
| # | rust web framework axum | rust web framework actix |
|---|-----|-----|
| 1 | Axum — framework web ergonômico | Actix Web — poderoso e pragmático |
| 2 | Começando com Axum | Quickstart do Actix Web |
| 3 | Axum + middleware Tower | Guia de middleware do Actix-web |
...
```

**Por que funciona:** Dois payloads JSON independentes + loop shell com indexação via `jaq` geram uma tabela Markdown sem nenhuma biblioteca de tabela — renderizável universalmente em GitHub, VS Code, `glow`, etc.

---

### Receita 13 — JSON Lines (NDJSON) para ingestão ETL

**Objetivo:** Achatamento de uma execução multi-query em um resultado por linha (NDJSON), pronto para `COPY` em ClickHouse / BigQuery / DuckDB.

**Comando:**
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

**Saída esperada:**
```
150 /tmp/results.ndjson
{"query":"q1","ts":"2026-04-14T12:00:00Z","posicao":1,"titulo":"...","url":"...","snippet":"..."}
{"query":"q1","ts":"2026-04-14T12:00:00Z","posicao":2,"titulo":"...","url":"...","snippet":"..."}
{"query":"q1","ts":"2026-04-14T12:00:00Z","posicao":3,"titulo":"...","url":"...","snippet":"..."}
```

**Por que funciona:** `jaq -c` emite NDJSON compacto com um objeto por linha — formato nativo para `COPY FROM` na maioria dos bancos colunares. O formato é plano e carrega a query para agrupamento.

---

### Receita 14 — Busca e sumarização com LLM local

**Objetivo:** Enviar JSON consolidado a um LLM local compatível com OpenAI (ex: `llama.cpp server`, Ollama) para sumarização.

**Comando:**
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

**Saída esperada:**
```
- RAG combina retrieval + geração para ancorar LLMs com contexto fresco (https://...).
- Embeddings + banco vetorial são a camada canônica de retrieval (https://...).
- Estratégia de chunking afeta materialmente a qualidade da resposta (https://...).
- Re-ranking aumenta a precisão@k antes da chamada ao LLM (https://...).
- Avaliação tipicamente usa faithfulness + context recall (https://...).
```

**Por que funciona:** A CLI produz JSON estruturado com conteúdo opcional; `jaq` molda no formato de string única que a API estilo OpenAI espera. `xh` cuida da codificação JSON automaticamente.

---

### Receita 15 — Função bash `ddg-deep`

**Objetivo:** Função reutilizável que aplica defaults seguros (timeout, retries, `--num` razoável, saída JSON, arquivo com timestamp automático).

**Comando:**
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

**Saída esperada:**
```
Salvo: ./ddg-comparacao-de-runtimes-async-em-rust-2026-20260414T153000Z.json
1. Tokio — runtime assíncrono para Rust
2. async-std: Versão assíncrona da std
3. smol — runtime async pequeno e rápido
4. Comparando runtimes async em Rust — edição 2026
5. Glommio — runtime thread-per-core
```

**Por que funciona:** Uma função codifica seus defaults opinativos em um único lugar. Cada invocação herda a combinação segura de timeout / retries / `--global-timeout` sem o operador precisar lembrar.

---

## Recipe-to-Use-Case Table / Tabela Receita → Caso de Uso

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
| 14 | Pipeline busca→sumarização com LLM / Search→summarize LLM pipeline | `duckduckgo-search-cli --fetch-content`, `jaq`, `xh`, `timeout` |
| 15 | Defaults opinativos reutilizáveis / Reusable opinionated defaults | `duckduckgo-search-cli`, função bash, `jaq`, `date`, `timeout` |

---

_End of COOKBOOK / Fim do Livro de Receitas._
