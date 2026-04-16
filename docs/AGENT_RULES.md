# AGENT RULES — `duckduckgo-search-cli`
- Regras imperativas para agentes de IA que invocam `duckduckgo-search-cli` em pipelines de produção.
- Imperative rules for AI agents invoking `duckduckgo-search-cli` in production pipelines.
- Version: v0.4.4 · Schema: stable · Audience: Claude Code · Cursor · Codex · Aider · any autonomous agent.

## TL;DR — 5 regras que eliminam 90% das falhas de agente / 5 rules that eliminate 90% of agent failures
- ALWAYS pipe with `-q -f json` and parse with `jaq`. NEVER parse text output.
- ALWAYS wrap rate-limited calls with `timeout 60` and a sane `--parallel` (max 5 unless you own the outbound IP).
- NEVER assume optional JSON fields (`snippet`, `url_exibicao`, `titulo_original`) exist — use `jaq ' // "" '` fallbacks.
- ALWAYS check the process exit code: `0` success, `3` block, `4` global timeout, `5` zero results — each demands a different strategy.
- NEVER hardcode API keys, proxies, or User-Agents into arguments — they belong in `$XDG_CONFIG_HOME/duckduckgo-search-cli/` or environment variables.

## ENGLISH
### A. Core Invariants — Rules That Never Change
#### R01 — Pass `-q` to guarantee your parser never sees log noise
- Omitting `-q` sends `tracing` logs to stderr and pollutes downstream parsers unpredictably.
- `-q` (`--quiet`) is the explicit contract: stdout carries only the payload, stderr is silenced.
- Every pipeline that parses stdout MUST include `-q` without exception.

```bash
timeout 30 duckduckgo-search-cli "rust async runtime" -q --num 15 | jaq '.resultados[].url'
```

#### R02 — Declare `-f json` explicitly to survive CI redirection quirks
- When stdout is a TTY, `auto` defaults to human-readable text.
- When stdout is not a TTY, `auto` resolves to `json` — but CI pipelines with `tee` or log capture break this assumption.
- ALWAYS specify `-f json` explicitly in scripts to guarantee deterministic output format.

```bash
duckduckgo-search-cli "query" -q -f json --num 15 | jaq '.quantidade_resultados'
```

#### R03 — NEVER use `text` or `markdown` formats for machine parsing
- `text` and `markdown` are human-facing presentation layers with no stable schema.
- Only `json` has a versioned contract guaranteed not to break between patch releases.
- Any pipeline reading fields by position or pattern from text output WILL break on upgrade.

```bash
# NEVER
duckduckgo-search-cli "q" -f text | rg 'http'
# ALWAYS
duckduckgo-search-cli "q" -q -f json | jaq -r '.resultados[].url'
```

#### R04 — Pin `--num` explicitly to prevent silent behavior changes on defaults update
- The current default is `15` results with auto-pagination across 2 pages.
- Relying on defaults means your pipeline changes behavior when defaults change between versions.
- Pin the exact count your pipeline was tested with to guarantee reproducible result sets.

```bash
duckduckgo-search-cli "q" -q --num 15
duckduckgo-search-cli "q" -q --num 30 --pages 3   # override pages when needing >15
```

#### R05 — Cap `--parallel` at 5 to stay below DuckDuckGo's anti-bot threshold
- The hard maximum is `20`, but values above `5` reliably trigger HTTP 202 anti-bot responses.
- Anti-bot responses produce exit code `3`, which requires 300+ second backoff to recover from.
- Default is `5`. Accept the default unless you control the outbound IP reputation.

```bash
duckduckgo-search-cli --queries-file q.txt -q --parallel 5
```

#### R06 — Use `--output` for large result sets to get atomic writes and correct permissions
- `--output` creates parent directories automatically, sets Unix permissions `0o644`, and forces `json` even in TTY mode.
- Shell redirection (`>`) does none of these and silently truncates on partial failures.
- Use `--output` for any result set larger than a single-query response.

```bash
duckduckgo-search-cli "q" --num 50 --pages 4 --output /tmp/out/results.json -q
```

#### R07 — NEVER invoke without `timeout` — network I/O without a fence hangs pipelines indefinitely
- A stalled TCP connection or rate-limit retry loop can hold your agent blocked for minutes.
- `timeout 60` covers single-query calls. `timeout 300` covers large batch files.
- Every agent invocation MUST be wrapped with `timeout` — no exceptions.

```bash
timeout 60 duckduckgo-search-cli "q" -q
timeout 300 duckduckgo-search-cli --queries-file big.txt -q --parallel 5
```

#### R08 — Use `--queries-file` for batch work to reuse connection pools and UA rotation
- Shell loops spawn one process per query, paying DNS, TLS, and startup costs each time.
- One `--queries-file` invocation reuses HTTP connection pooling, UA rotation, and per-host rate limiting across all queries.
- Process startup cost is approximately 30–80 ms. Batching amortizes this across all queries.

```bash
printf 'query one\nquery two\nquery three\n' > /tmp/q.txt
duckduckgo-search-cli --queries-file /tmp/q.txt -q --parallel 3 -f json
```

#### R09 — NEVER use `--stream` — it is a placeholder with no implementation
- The flag exists in the CLI interface as reserved for future use.
- It is NOT implemented in v0.4.x.
- Any pipeline depending on `--stream` will produce undefined behavior.

#### R10 — Prefer `--endpoint html` and fall back to `lite` only after confirmed anti-bot block
- `html` endpoint delivers richer metadata: snippet, display URL, canonical title.
- `lite` is a degradation strategy, not a starting point.
- Switch to `--endpoint lite` only after receiving exit code `3` from `html`.

```bash
duckduckgo-search-cli "q" -q --endpoint html --num 15
duckduckgo-search-cli "q" -q --endpoint lite --num 15   # only after exit code 3
```

### B. JSON Output Contract — Fields You Can Trust and Fields You Cannot
#### R11 — Distinguish single-query vs multi-query JSON root to avoid silent null access
- Single query: root is a `SaidaBusca` object with `.query`, `.resultados`, `.metadados`.
- Multi-query or `--queries-file`: root is `{ "quantidade_queries", "buscas": [SaidaBusca, ...] }`.
- Accessing `.resultados` on a multi-query response returns null — your pipeline silently produces empty output.

```bash
duckduckgo-search-cli "one" -q | jaq '.resultados | length'
duckduckgo-search-cli "one" "two" -q | jaq '.buscas[0].resultados | length'
```

#### R12 — Access `.resultados[].titulo` and `.resultados[].url` as guaranteed non-null fields
- These two fields are always present when `resultados` is non-empty.
- They are typed as `String`, not `Option<String>` — null-checking them is unnecessary noise.
- Build your extraction pipelines on these two fields as the reliable foundation.

```bash
duckduckgo-search-cli "q" -q | jaq -r '.resultados[] | "\(.posicao): \(.titulo) — \(.url)"'
```

#### R13 — NEVER assume `snippet`, `url_exibicao`, or `titulo_original` are present
- These three are `Option<String>` — DuckDuckGo does not always return them.
- Absent means the field was not extractable from the HTML, not that the result is invalid.
- Use `// ""` or `// .url` fallbacks on every access to these fields.

```bash
duckduckgo-search-cli "q" -q | jaq '.resultados[] | {
  titulo,
  url,
  snippet: (.snippet // ""),
  url_exibicao: (.url_exibicao // .url),
  titulo_original: (.titulo_original // .titulo)
}'
```

#### R14 — Read `.metadados.tempo_execucao_ms` for accurate latency observability
- This is the canonical latency signal measured inside the binary, accounting for retries and pagination.
- Do not measure wall-clock time in your wrapper — it includes process startup and shell overhead.
- Feed this value into your observability stack to track DuckDuckGo response degradation over time.

```bash
duckduckgo-search-cli "q" -q | jaq '.metadados.tempo_execucao_ms'
```

#### R15 — Check `.metadados.usou_endpoint_fallback` to detect silent endpoint degradation
- If `true`, DuckDuckGo forced a degradation from `html` to `lite` mid-request.
- This predicts future rate limiting and means your result metadata may be incomplete.
- Log every `true` occurrence — a pattern of fallbacks signals IP reputation degradation.

```bash
duckduckgo-search-cli "q" -q | jaq '.metadados.usou_endpoint_fallback'
```

#### R16 — Use `.quantidade_resultados` over `(.resultados | length)` to read the declared contract
- Both return the same number today.
- `.quantidade_resultados` is the declared schema field — stable across refactors.
- `(.resultados | length)` is a derived value that breaks if pagination or deduplication changes the array structure.

```bash
duckduckgo-search-cli "q" -q --num 15 | jaq '.quantidade_resultados'
```

#### R17 — Gate `.conteudo` and content fields on `--fetch-content` to avoid silent null access
- Without `--fetch-content`, the fields `.conteudo`, `.tamanho_conteudo`, and `.metodo_extracao_conteudo` are absent from the JSON.
- Never probe for them blindly — absent fields return null in `jaq`, silently breaking downstream logic.
- Always pass `--max-content-length` to cap memory when enabling `--fetch-content`.

```bash
duckduckgo-search-cli "q" -q --fetch-content --max-content-length 10000 \
  | jaq '.resultados[] | {url, size: (.tamanho_conteudo // 0)}'
```

### C. Rate Limiting and Etiquette — Stay Below the Anti-Bot Threshold
#### R18 — Respect `--per-host-limit` at 2 to avoid anti-bot detection
- Default is `2` concurrent requests per host. Hard maximum is `10`.
- Raising this value directly increases the probability of HTTP `202` anti-bot responses (exit code `3`).
- Anti-bot blocks require 300+ second recovery windows — the cost of raising this limit is never worth it.

```bash
duckduckgo-search-cli --queries-file q.txt -q --per-host-limit 2 --parallel 5
```

#### R19 — Use built-in `--retries` instead of shell retry loops to get exponential backoff
- The built-in retry uses exponential backoff tuned specifically to DuckDuckGo's rate limits.
- Shell retry loops apply fixed delays, saturate the IP faster, and produce cascading blocks.
- Default is `2` retries. Maximum is `10`. Raise to `3` for unstable network environments.

```bash
duckduckgo-search-cli "q" -q --retries 3 --timeout 20
```

#### R20 — Raise `--global-timeout` for batch jobs to prevent premature termination
- Default `--global-timeout` is `60` seconds per CLI invocation across ALL queries.
- A `--queries-file` with 50 queries at `--parallel 5` requires approximately 600 seconds.
- Calculate: `(num_queries / parallel) * avg_query_seconds * 1.5` and set accordingly.

```bash
duckduckgo-search-cli --queries-file big.txt -q --parallel 5 --global-timeout 600
```

### D. Error Handling — Branch on Exit Code Before Parsing
#### R21 — Branch on exit code before parsing stdout to route failures correctly
- Parsing stdout on a non-zero exit code produces garbage or silent null results.
- Each exit code demands a distinct response strategy — see table below.
- MUST check `$?` immediately after every invocation before any `jaq` pipeline.

| Code | Meaning         | Agent action                                              |
|------|-----------------|-----------------------------------------------------------|
| `0`  | success         | parse `.resultados`                                       |
| `1`  | runtime error   | read stderr; retry once with `-v` for diagnostics         |
| `2`  | config error    | run `duckduckgo-search-cli init-config --force`, re-try   |
| `3`  | anti-bot block  | back off 300+ s; switch `--endpoint lite`; rotate proxy   |
| `4`  | global timeout  | raise `--global-timeout`; reduce `--parallel`             |
| `5`  | zero results    | refine query; try different `--lang` / `--country`        |

```bash
timeout 60 duckduckgo-search-cli "q" -q -f json > /tmp/out.json
case $? in
  0) jaq '.resultados[].url' /tmp/out.json ;;
  3) echo "blocked, backing off"; sleep 300 ;;
  4) echo "timeout, lowering parallelism" ;;
  5) echo "no results for query" ;;
  *) echo "unexpected error"; cat /tmp/out.json ;;
esac
```

#### R22 — Parse `erro` and `mensagem` fields when exit code is non-zero with JSON on stdout
- On some failures the CLI emits structured JSON to stdout even with a non-zero exit code.
- Ignoring stdout on failure means losing actionable diagnostic information.
- Check `has("erro")` before attempting to read `.resultados`.

```bash
duckduckgo-search-cli "q" -q -f json \
  | jaq 'if has("erro") then {erro, mensagem} else .resultados end'
```

#### R23 — NEVER silently swallow non-zero exit codes — agents that hide failures cannot self-correct
- An agent ignoring exit codes cannot distinguish success from total failure.
- Silent swallowing propagates corrupt state through downstream pipeline stages.
- Fail loud unless you have a specific, documented retry strategy.

```bash
# NEVER
duckduckgo-search-cli "q" -q 2>/dev/null || true
# ALWAYS
duckduckgo-search-cli "q" -q || { echo "failed: $?" >&2; exit 1; }
```

#### R24 — Verify pipe integrity with PIPESTATUS when consuming stdout via pipe
- In `cmd | jaq`, the shell reports only `jaq`'s exit code — the CLI's exit code is hidden.
- A failed CLI (exit 1–5) produces empty or partial stdout that `jaq` silently ignores.
- MUST check `${PIPESTATUS[0]}` after every piped invocation to detect upstream failure.
- The CLI restores SIGPIPE to SIG_DFL on Unix — broken pipes terminate cleanly (no EPIPE errors).

```bash
# NEVER — exit code of duckduckgo-search-cli is silently lost
timeout 60 duckduckgo-search-cli "q" -q -f json | jaq '.resultados[].url'

# ALWAYS — capture PIPESTATUS to detect upstream failure
timeout 60 duckduckgo-search-cli "q" -q -f json | jaq '.resultados[].url'
ddg_exit=${PIPESTATUS[0]}
if [ "$ddg_exit" -ne 0 ]; then echo "CLI failed: exit $ddg_exit" >&2; fi
```

### E. Performance — Eliminate Redundant Process Starts and Wasted Connections
#### R25 — Rely on auto-pagination to avoid doubling DNS, TLS, and UA rotation overhead
- The binary pools connections and reuses UA selection across pages within a single invocation.
- Invoking it twice for pages 1 and 2 doubles DNS resolution, TLS handshake, and UA rotation cost.
- Default is 2 pages automatically. Use `--pages` to extend, not separate invocations.

```bash
# one call, 2 pages fetched automatically
duckduckgo-search-cli "q" -q --num 15
# NEVER — two calls, wasted overhead
duckduckgo-search-cli "q" -q --num 7 --pages 1
duckduckgo-search-cli "q" -q --num 8 --pages 2
```

#### R26 — Treat `--fetch-content` as an N-times latency multiplier — use only when consuming the body
- Enabling `--fetch-content` adds one HTTP fetch per result in the response.
- With 15 results, latency multiplies by up to 15x.
- Use `--max-content-length` to cap memory consumption. Use `--num 5` when content fetching is required.

```bash
duckduckgo-search-cli "q" -q --num 5 --fetch-content --max-content-length 5000
```

#### R27 — Prefer one batched invocation to amortize 30–80 ms process startup cost per query
- Each process invocation pays DNS, TLS, UA initialization, and Tokio runtime startup.
- One `--queries-file` invocation amortizes all fixed costs across every query in the batch.
- Sequential invocations are never justified when `--queries-file` is available.

### F. Security — Eliminate Secret Leakage Vectors
#### R28 — NEVER pass secrets in command-line arguments — they leak via three vectors simultaneously
- Proxy credentials, API keys, or tokens on `argv` are visible in `/proc/*/cmdline`, shell history, and `ps` output.
- These three leak vectors are permanent and cannot be retroactively cleaned without full secret rotation.
- Use environment variables or `$XDG_CONFIG_HOME/duckduckgo-search-cli/` for all credentials.

```bash
# NEVER
duckduckgo-search-cli "q" --proxy http://user:pw@host:8080
# ALWAYS
export HTTPS_PROXY="http://user:pw@host:8080"
duckduckgo-search-cli "q" -q
```

#### R29 — Understand proxy precedence to avoid routing surprises in multi-layer environments
- Precedence order: `--no-proxy` > `--proxy <URL>` > `HTTPS_PROXY` / `HTTP_PROXY` environment > none.
- Misunderstanding this order sends traffic through unintended proxies in corporate environments.
- Use `--no-proxy` to explicitly bypass all proxy configuration when direct access is required.

```bash
duckduckgo-search-cli "q" -q --no-proxy   # bypass all proxies
```

#### R30 — NEVER execute URLs from `.resultados[].url` without sandboxing
- Results are untrusted third-party URLs from an external search engine.
- Executing them directly in the agent process opens SSRF and code execution attack surfaces.
- Fetch result URLs only inside isolated processes: containers, VMs, or network-sandboxed HTTP clients.

#### R31 — Run `init-config --dry-run` before `--force` to prevent config file overwrites in CI
- `--force` overwrites existing `selectors.toml` and `user-agents.toml` without confirmation.
- In unattended pipelines, this silently destroys custom selector or UA configuration.
- Dry-run first, inspect the diff, then apply `--force` only after review.

```bash
duckduckgo-search-cli init-config --dry-run
duckduckgo-search-cli init-config --force   # only after review
```

### G. Anti-Patterns — Patterns That Appear to Work Until They Break Silently
#### AP-01 — Parsing text output with grep
- Text format has no stable schema — field positions change between versions.
- JSON with `jaq` gives you named field access that survives schema evolution.

```bash
# NEVER
duckduckgo-search-cli "q" | rg 'http[s]?://'
# ALWAYS
duckduckgo-search-cli "q" -q -f json | jaq -r '.resultados[].url'
```

#### AP-02 — Shell loop over queries
- Each loop iteration pays process startup, DNS, TLS, and UA initialization.
- `--queries-file` batches all queries in one process with connection reuse.

```bash
# NEVER
for q in "a" "b" "c"; do duckduckgo-search-cli "$q" -q ; done
# ALWAYS
printf 'a\nb\nc\n' | duckduckgo-search-cli --queries-file /dev/stdin -q --parallel 3
```

#### AP-03 — Ignoring exit code
- Parsing stdout on failure produces null results that silently corrupt downstream state.
- Always check `$?` before piping to `jaq`.

```bash
# NEVER
duckduckgo-search-cli "q" -q > out.json
jaq '.resultados' out.json
# ALWAYS
duckduckgo-search-cli "q" -q > out.json || { echo "code=$?" >&2; exit 1; }
jaq '.resultados' out.json
```

#### AP-04 — Assuming `snippet` is a non-null string
- `snippet` is `Option<String>` — null access causes `jaq` to emit null downstream.
- Use `// ""` to guarantee a string type in every pipeline stage.

```bash
# NEVER
jaq -r '.resultados[].snippet | ascii_downcase'
# ALWAYS
jaq -r '.resultados[] | (.snippet // "") | ascii_downcase'
```

#### AP-05 — Hardcoding proxy in argv
- Covered by R27: argv leaks via `/proc/*/cmdline`, shell history, and `ps`.
- Agents repeatedly commit this anti-pattern — it is listed twice deliberately.

#### AP-06 — Raising `--parallel` to 20 to increase throughput
- Values above `5` reliably trigger DuckDuckGo anti-bot detection.
- Exit code `3` requires 300+ second recovery — the throughput gain is erased by the backoff.

```bash
# NEVER
duckduckgo-search-cli --queries-file big.txt -q --parallel 20
# ALWAYS
duckduckgo-search-cli --queries-file big.txt -q --parallel 5 --global-timeout 600
```

#### AP-07 — Using `--stream`
- `--stream` is a placeholder flag. It is NOT implemented in v0.4.x.
- Any invocation using it produces undefined behavior.

```bash
# NEVER — placeholder, not implemented
duckduckgo-search-cli "q" --stream
# ALWAYS — omit the flag
duckduckgo-search-cli "q" -q -f json
```

#### AP-08 — Invoking without a `timeout` wrapper
- Network I/O without a fence hangs your agent pipeline indefinitely on stalled connections.
- `timeout 60` is the minimum for any invocation touching the network.

```bash
# NEVER
duckduckgo-search-cli "q" -q
# ALWAYS
timeout 60 duckduckgo-search-cli "q" -q
```

## PORTUGUÊS
### A. Invariantes Centrais — Regras Que Nunca Mudam
#### R01 — Passe `-q` para garantir que seu parser nunca veja ruído de log
- Omitir `-q` envia logs `tracing` para stderr e polui parsers downstream de forma imprevisível.
- `-q` (`--quiet`) é o contrato explícito: stdout carrega apenas o payload, stderr é silenciado.
- Todo pipeline que parseia stdout DEVE incluir `-q` sem exceção.

```bash
timeout 30 duckduckgo-search-cli "rust async runtime" -q --num 15 | jaq '.resultados[].url'
```

#### R02 — Declare `-f json` explicitamente para sobreviver a redirecionamentos de CI
- Quando stdout é TTY, `auto` resolve para texto legível por humanos.
- Quando stdout não é TTY, `auto` resolve para `json` — mas pipelines de CI com `tee` ou captura de log quebram essa suposição.
- SEMPRE especifique `-f json` explicitamente em scripts para garantir formato de saída determinístico.

```bash
duckduckgo-search-cli "consulta" -q -f json --num 15 | jaq '.quantidade_resultados'
```

#### R03 — JAMAIS use os formatos `text` ou `markdown` para parsing de máquina
- `text` e `markdown` são camadas de apresentação para humanos, sem schema estável.
- Apenas `json` tem contrato versionado com garantia de não quebrar entre releases patch.
- Qualquer pipeline que leia campos por posição ou padrão em saída textual VAI quebrar no upgrade.

```bash
# JAMAIS
duckduckgo-search-cli "consulta" -f text | rg 'http'
# SEMPRE
duckduckgo-search-cli "consulta" -q -f json | jaq -r '.resultados[].url'
```

#### R04 — Fixe `--num` explicitamente para evitar mudanças silenciosas de comportamento
- O padrão atual é `15` resultados com auto-paginação em 2 páginas.
- Depender de padrões significa que seu pipeline muda de comportamento quando os padrões mudam entre versões.
- Fixe o número exato que seu pipeline testou para garantir conjuntos de resultados reproduzíveis.

```bash
duckduckgo-search-cli "consulta" -q --num 15
duckduckgo-search-cli "consulta" -q --num 30 --pages 3
```

#### R05 — Limite `--parallel` em 5 para ficar abaixo do limiar anti-bot do DuckDuckGo
- O máximo rígido é `20`, mas valores acima de `5` acionam respostas HTTP 202 anti-bot de forma confiável.
- Respostas anti-bot produzem exit code `3`, exigindo backoff de 300+ segundos para recuperação.
- O padrão é `5`. Aceite o padrão a menos que você controle a reputação do IP de saída.

```bash
duckduckgo-search-cli --queries-file q.txt -q --parallel 5
```

#### R06 — Use `--output` para conjuntos grandes de resultados e obtenha escritas atômicas com permissões corretas
- `--output` cria diretórios-pai automaticamente, aplica permissões Unix `0o644` e força `json` mesmo em modo TTY.
- Redirecionamento de shell (`>`) não faz nenhum desses e trunca silenciosamente em falhas parciais.
- Use `--output` para qualquer conjunto de resultados maior que uma resposta de query única.

```bash
duckduckgo-search-cli "consulta" --num 50 --pages 4 --output /tmp/saida/resultados.json -q
```

#### R07 — JAMAIS invoque sem `timeout` — I/O de rede sem cerca trava pipelines indefinidamente
- Uma conexão TCP travada ou loop de retry por rate-limit pode manter seu agente bloqueado por minutos.
- `timeout 60` cobre chamadas de query única. `timeout 300` cobre arquivos de lote grandes.
- Toda invocação de agente DEVE ser envolvida com `timeout` — sem exceções.

```bash
timeout 60 duckduckgo-search-cli "consulta" -q
timeout 300 duckduckgo-search-cli --queries-file lote.txt -q --parallel 5
```

#### R08 — Use `--queries-file` para trabalho em lote e reaproveite pools de conexão e rotação de UA
- Loops de shell geram um processo por query, pagando DNS, TLS e custo de startup a cada vez.
- Uma invocação com `--queries-file` reaproveita pool HTTP, rotação de UA e rate-limit por host em todas as queries.
- O custo de startup do processo é aproximadamente 30–80 ms. Lotes amortizam isso em todas as queries.

```bash
printf 'consulta um\nconsulta dois\nconsulta tres\n' > /tmp/q.txt
duckduckgo-search-cli --queries-file /tmp/q.txt -q --parallel 3 -f json
```

#### R09 — JAMAIS use `--stream` — é um placeholder sem implementação
- A flag existe na interface da CLI como reservada para uso futuro.
- NÃO está implementada na v0.4.x.
- Qualquer pipeline que dependa de `--stream` produzirá comportamento indefinido.

#### R10 — Prefira `--endpoint html` e recorra a `lite` apenas após bloqueio anti-bot confirmado
- O endpoint `html` entrega metadados mais ricos: snippet, URL de exibição, título canônico.
- `lite` é uma estratégia de degradação, não um ponto de partida.
- Troque para `--endpoint lite` apenas após receber exit code `3` do `html`.

```bash
duckduckgo-search-cli "consulta" -q --endpoint html --num 15
duckduckgo-search-cli "consulta" -q --endpoint lite --num 15   # só após exit code 3
```

### B. Contrato da Saída JSON — Campos Confiáveis e Campos Opcionais
#### R11 — Distinga raiz JSON de query única vs múltiplas para evitar acesso nulo silencioso
- Query única: raiz é objeto `SaidaBusca` com `.query`, `.resultados`, `.metadados`.
- Múltiplas queries ou `--queries-file`: raiz é `{ "quantidade_queries", "buscas": [SaidaBusca, ...] }`.
- Acessar `.resultados` em resposta de múltiplas queries retorna null — seu pipeline produz saída vazia silenciosamente.

```bash
duckduckgo-search-cli "uma" -q | jaq '.resultados | length'
duckduckgo-search-cli "uma" "duas" -q | jaq '.buscas[0].resultados | length'
```

#### R12 — Acesse `.resultados[].titulo` e `.resultados[].url` como campos garantidos não-nulos
- Esses dois campos estão sempre presentes quando `resultados` é não-vazio.
- São tipados como `String`, não `Option<String>` — verificar nulos neles é ruído desnecessário.
- Construa seus pipelines de extração sobre esses dois campos como fundação confiável.

```bash
duckduckgo-search-cli "consulta" -q | jaq -r '.resultados[] | "\(.posicao): \(.titulo) — \(.url)"'
```

#### R13 — JAMAIS assuma que `snippet`, `url_exibicao` ou `titulo_original` estão presentes
- Esses três são `Option<String>` — o DuckDuckGo nem sempre os retorna.
- Ausente significa que o campo não foi extraível do HTML, não que o resultado é inválido.
- Use `// ""` ou `// .url` como fallback em todo acesso a esses campos.

```bash
duckduckgo-search-cli "consulta" -q | jaq '.resultados[] | {
  titulo,
  url,
  snippet: (.snippet // ""),
  url_exibicao: (.url_exibicao // .url),
  titulo_original: (.titulo_original // .titulo)
}'
```

#### R14 — Leia `.metadados.tempo_execucao_ms` para observabilidade precisa de latência
- Este é o sinal canônico de latência medido dentro do binário, contabilizando retries e paginação.
- Não meça wall-clock no seu wrapper — inclui startup de processo e overhead de shell.
- Alimente esse valor no seu stack de observabilidade para rastrear degradação de resposta ao longo do tempo.

```bash
duckduckgo-search-cli "consulta" -q | jaq '.metadados.tempo_execucao_ms'
```

#### R15 — Verifique `.metadados.usou_endpoint_fallback` para detectar degradação silenciosa de endpoint
- Se `true`, o DuckDuckGo forçou degradação de `html` para `lite` durante a requisição.
- Isso prediz rate-limiting futuro e significa que seus metadados de resultado podem estar incompletos.
- Registre toda ocorrência `true` — um padrão de fallbacks sinaliza degradação da reputação do IP.

```bash
duckduckgo-search-cli "consulta" -q | jaq '.metadados.usou_endpoint_fallback'
```

#### R16 — Use `.quantidade_resultados` em vez de `(.resultados | length)` para ler o contrato declarado
- Os dois retornam o mesmo número hoje.
- `.quantidade_resultados` é o campo declarado no schema — estável entre refatorações.
- `(.resultados | length)` é valor derivado que quebra se paginação ou deduplicação mudar a estrutura do array.

```bash
duckduckgo-search-cli "consulta" -q --num 15 | jaq '.quantidade_resultados'
```

#### R17 — Condicione `.conteudo` e campos de conteúdo a `--fetch-content` para evitar acesso nulo silencioso
- Sem `--fetch-content`, os campos `.conteudo`, `.tamanho_conteudo` e `.metodo_extracao_conteudo` estão ausentes do JSON.
- Nunca sonde por eles às cegas — campos ausentes retornam null no `jaq`, quebrando silenciosamente a lógica downstream.
- Sempre passe `--max-content-length` para limitar consumo de memória ao ativar `--fetch-content`.

```bash
duckduckgo-search-cli "consulta" -q --fetch-content --max-content-length 10000 \
  | jaq '.resultados[] | {url, tamanho: (.tamanho_conteudo // 0)}'
```

### C. Rate Limiting e Etiqueta — Fique Abaixo do Limiar Anti-Bot
#### R18 — Respeite `--per-host-limit` em 2 para evitar detecção anti-bot
- Padrão é `2` requisições concorrentes por host. Máximo rígido é `10`.
- Elevar esse valor aumenta diretamente a probabilidade de respostas HTTP `202` anti-bot (exit code `3`).
- Bloqueios anti-bot exigem janelas de recuperação de 300+ segundos — o custo de elevar esse limite nunca vale.

```bash
duckduckgo-search-cli --queries-file q.txt -q --per-host-limit 2 --parallel 5
```

#### R19 — Use `--retries` interno em vez de loops de retry no shell para obter backoff exponencial
- O retry interno usa backoff exponencial calibrado especificamente para os limites do DuckDuckGo.
- Loops de retry no shell aplicam delays fixos, saturam o IP mais rápido e produzem bloqueios em cascata.
- Padrão é `2` retries. Máximo é `10`. Eleve para `3` em ambientes de rede instável.

```bash
duckduckgo-search-cli "consulta" -q --retries 3 --timeout 20
```

#### R20 — Eleve `--global-timeout` para lotes para evitar terminação prematura
- O padrão `--global-timeout` é `60` segundos por invocação da CLI para TODAS as queries.
- Um `--queries-file` com 50 queries em `--parallel 5` requer aproximadamente 600 segundos.
- Calcule: `(num_queries / parallel) * avg_query_seconds * 1.5` e configure adequadamente.

```bash
duckduckgo-search-cli --queries-file lote.txt -q --parallel 5 --global-timeout 600
```

### D. Tratamento de Erros — Bifurque no Exit Code Antes de Parsear
#### R21 — Bifurque no exit code antes de parsear stdout para rotear falhas corretamente
- Parsear stdout em exit code não-zero produz lixo ou resultados nulos silenciosos.
- Cada exit code exige uma estratégia de resposta distinta — veja a tabela abaixo.
- DEVE verificar `$?` imediatamente após cada invocação antes de qualquer pipeline `jaq`.

| Código | Significado      | Ação do agente                                                  |
|--------|------------------|-----------------------------------------------------------------|
| `0`    | sucesso          | parsear `.resultados`                                           |
| `1`    | erro runtime     | ler stderr; retry único com `-v` para diagnóstico               |
| `2`    | erro config      | rodar `duckduckgo-search-cli init-config --force`, re-tentar    |
| `3`    | bloqueio anti-bot| recuar 300+ s; trocar `--endpoint lite`; rotacionar proxy       |
| `4`    | timeout global   | elevar `--global-timeout`; reduzir `--parallel`                 |
| `5`    | zero resultados  | refinar query; trocar `--lang` / `--country`                    |

```bash
timeout 60 duckduckgo-search-cli "consulta" -q -f json > /tmp/out.json
case $? in
  0) jaq '.resultados[].url' /tmp/out.json ;;
  3) echo "bloqueado, recuando"; sleep 300 ;;
  4) echo "timeout, reduzir paralelismo" ;;
  5) echo "sem resultados para a query" ;;
  *) echo "erro inesperado"; cat /tmp/out.json ;;
esac
```

#### R22 — Parsear `erro` e `mensagem` quando exit code não-zero E stdout for JSON
- Em certas falhas a CLI ainda emite JSON estruturado em stdout mesmo com exit code não-zero.
- Ignorar stdout em falhas significa perder informação diagnóstica acionável.
- Verifique `has("erro")` antes de tentar ler `.resultados`.

```bash
duckduckgo-search-cli "consulta" -q -f json \
  | jaq 'if has("erro") then {erro, mensagem} else .resultados end'
```

#### R23 — JAMAIS engula silenciosamente exit codes não-zero — agentes que escondem falhas não conseguem se autocorrigir
- Um agente que ignora exit codes não consegue distinguir sucesso de falha total.
- Engolir silenciosamente propaga estado corrompido pelos estágios downstream do pipeline.
- Falhe alto a menos que você tenha uma estratégia de retry específica e documentada.

```bash
# JAMAIS
duckduckgo-search-cli "consulta" -q 2>/dev/null || true
# SEMPRE
duckduckgo-search-cli "consulta" -q || { echo "falhou: $?" >&2; exit 1; }
```

#### R24 — Verifique integridade do pipe com PIPESTATUS ao consumir stdout via pipe
- Em `cmd | jaq`, o shell reporta apenas o exit code do `jaq` — o exit code do CLI fica oculto.
- Um CLI com falha (exit 1–5) produz stdout vazio ou parcial que o `jaq` ignora silenciosamente.
- DEVE verificar `${PIPESTATUS[0]}` após toda invocação em pipe para detectar falha upstream.
- O CLI restaura SIGPIPE para SIG_DFL no Unix — pipes quebrados terminam limpos (sem erros EPIPE).

```bash
# JAMAIS — exit code do duckduckgo-search-cli é silenciosamente perdido
timeout 60 duckduckgo-search-cli "consulta" -q -f json | jaq '.resultados[].url'

# SEMPRE — capture PIPESTATUS para detectar falha upstream
timeout 60 duckduckgo-search-cli "consulta" -q -f json | jaq '.resultados[].url'
ddg_exit=${PIPESTATUS[0]}
if [ "$ddg_exit" -ne 0 ]; then echo "CLI falhou: exit $ddg_exit" >&2; fi
```

### E. Performance — Elimine Starts de Processo Redundantes e Conexões Desperdiçadas
#### R25 — Confie na auto-paginação para evitar duplicar DNS, TLS e overhead de rotação de UA
- O binário pooliza conexões e reaproveita seleção de UA entre páginas em uma única invocação.
- Invocá-lo duas vezes para páginas 1 e 2 duplica resolução DNS, handshake TLS e custo de rotação de UA.
- O padrão é 2 páginas automaticamente. Use `--pages` para estender, não invocações separadas.

```bash
# uma chamada, 2 páginas buscadas automaticamente
duckduckgo-search-cli "consulta" -q --num 15
# JAMAIS — duas chamadas, overhead desperdiçado
duckduckgo-search-cli "consulta" -q --num 7 --pages 1
duckduckgo-search-cli "consulta" -q --num 8 --pages 2
```

#### R26 — Trate `--fetch-content` como multiplicador de latência N-vezes — use somente quando for consumir o corpo
- Ativar `--fetch-content` adiciona um fetch HTTP por resultado na resposta.
- Com 15 resultados, a latência se multiplica em até 15x.
- Use `--max-content-length` para limitar consumo de memória. Use `--num 5` quando fetch de conteúdo for necessário.

```bash
duckduckgo-search-cli "consulta" -q --num 5 --fetch-content --max-content-length 5000
```

#### R27 — Prefira uma invocação em lote para amortizar o custo de startup de 30–80 ms por query
- Cada invocação de processo paga DNS, TLS, inicialização de UA e startup do runtime Tokio.
- Uma invocação com `--queries-file` amortiza todos os custos fixos em cada query do lote.
- Invocações sequenciais nunca se justificam quando `--queries-file` está disponível.

### F. Segurança — Elimine Vetores de Vazamento de Segredos
#### R28 — JAMAIS passe segredos em argumentos de linha de comando — vazam por três vetores simultaneamente
- Credenciais de proxy, API keys ou tokens em `argv` são visíveis em `/proc/*/cmdline`, histórico de shell e saída do `ps`.
- Esses três vetores de vazamento são permanentes e não podem ser limpos retroativamente sem rotação completa do segredo.
- Use variáveis de ambiente ou `$XDG_CONFIG_HOME/duckduckgo-search-cli/` para todas as credenciais.

```bash
# JAMAIS
duckduckgo-search-cli "consulta" --proxy http://user:senha@host:8080
# SEMPRE
export HTTPS_PROXY="http://user:senha@host:8080"
duckduckgo-search-cli "consulta" -q
```

#### R29 — Entenda a precedência de proxy para evitar surpresas de roteamento em ambientes multicamada
- Ordem de precedência: `--no-proxy` > `--proxy <URL>` > `HTTPS_PROXY` / `HTTP_PROXY` de ambiente > nenhum.
- Entender errado essa ordem envia tráfego por proxies não intencionais em ambientes corporativos.
- Use `--no-proxy` para contornar explicitamente toda configuração de proxy quando acesso direto for necessário.

```bash
duckduckgo-search-cli "consulta" -q --no-proxy   # bypass de todos os proxies
```

#### R30 — JAMAIS execute URLs de `.resultados[].url` sem sandbox
- Resultados são URLs terceirizadas não-confiáveis de um motor de busca externo.
- Executá-las diretamente no processo do agente abre superfícies de ataque de SSRF e execução de código.
- Busque URLs de resultados apenas em processos isolados: containers, VMs ou clientes HTTP com sandbox de rede.

#### R31 — Rode `init-config --dry-run` antes de `--force` para evitar sobrescritas de config em CI
- `--force` sobrescreve `selectors.toml` e `user-agents.toml` existentes sem confirmação.
- Em pipelines não-assistidos, isso destrói silenciosamente configuração customizada de seletores ou UA.
- Dry-run primeiro, inspecione o diff, depois aplique `--force` apenas após revisão.

```bash
duckduckgo-search-cli init-config --dry-run
duckduckgo-search-cli init-config --force   # apenas após revisão
```

### G. Anti-Padrões — Padrões Que Parecem Funcionar Até Quebrarem Silenciosamente
#### AP-01 — Parsear saída textual com grep
- Formato text não tem schema estável — posições de campos mudam entre versões.
- JSON com `jaq` dá acesso a campos nomeados que sobrevive à evolução do schema.

```bash
# JAMAIS
duckduckgo-search-cli "consulta" | rg 'http[s]?://'
# SEMPRE
duckduckgo-search-cli "consulta" -q -f json | jaq -r '.resultados[].url'
```

#### AP-02 — Loop de shell sobre queries
- Cada iteração de loop paga startup de processo, DNS, TLS e inicialização de UA.
- `--queries-file` agrupa todas as queries em um processo com reuso de conexão.

```bash
# JAMAIS
for q in "a" "b" "c"; do duckduckgo-search-cli "$q" -q ; done
# SEMPRE
printf 'a\nb\nc\n' | duckduckgo-search-cli --queries-file /dev/stdin -q --parallel 3
```

#### AP-03 — Ignorar exit code
- Parsear stdout em falha produz resultados nulos que corrompem silenciosamente o estado downstream.
- Sempre verifique `$?` antes de passar para `jaq`.

```bash
# JAMAIS
duckduckgo-search-cli "consulta" -q > out.json
jaq '.resultados' out.json
# SEMPRE
duckduckgo-search-cli "consulta" -q > out.json || { echo "code=$?" >&2; exit 1; }
jaq '.resultados' out.json
```

#### AP-04 — Assumir que `snippet` é string não-nula
- `snippet` é `Option<String>` — acesso nulo faz `jaq` emitir null downstream.
- Use `// ""` para garantir tipo string em todo estágio do pipeline.

```bash
# JAMAIS
jaq -r '.resultados[].snippet | ascii_downcase'
# SEMPRE
jaq -r '.resultados[] | (.snippet // "") | ascii_downcase'
```

#### AP-05 — Hardcodar proxy em argv
- Coberto pela R27: argv vaza via `/proc/*/cmdline`, histórico de shell e `ps`.
- Agentes repetem esse anti-padrão com frequência — está listado duas vezes deliberadamente.

#### AP-06 — Subir `--parallel` para 20 para aumentar throughput
- Valores acima de `5` acionam detecção anti-bot do DuckDuckGo de forma confiável.
- Exit code `3` exige recuperação de 300+ segundos — o ganho de throughput é anulado pelo backoff.

```bash
# JAMAIS
duckduckgo-search-cli --queries-file lote.txt -q --parallel 20
# SEMPRE
duckduckgo-search-cli --queries-file lote.txt -q --parallel 5 --global-timeout 600
```

#### AP-07 — Usar `--stream`
- `--stream` é uma flag placeholder. NÃO está implementada na v0.4.x.
- Qualquer invocação que a use produz comportamento indefinido.

```bash
# JAMAIS — placeholder, não implementado
duckduckgo-search-cli "consulta" --stream
# SEMPRE — omita a flag
duckduckgo-search-cli "consulta" -q -f json
```

#### AP-08 — Invocar sem envoltório `timeout`
- I/O de rede sem cerca trava seu pipeline de agente indefinidamente em conexões travadas.
- `timeout 60` é o mínimo para qualquer invocação que toque a rede.

```bash
# JAMAIS
duckduckgo-search-cli "consulta" -q
# SEMPRE
timeout 60 duckduckgo-search-cli "consulta" -q
```

## Quick Reference Card
| ID  | English                                                            | Português                                                            |
|-----|--------------------------------------------------------------------|----------------------------------------------------------------------|
| R01 | MUST pass `-q` when piping to parser                               | DEVE passar `-q` ao canalizar para parser                            |
| R02 | MUST specify `-f json` explicitly in scripts                       | DEVE especificar `-f json` explicitamente em scripts                 |
| R03 | NEVER parse `text` / `markdown` for machines                       | JAMAIS parsear `text` / `markdown` para máquinas                     |
| R04 | MUST pass `--num` explicitly (default `15`, auto-paginates 2)      | DEVE passar `--num` explicitamente (padrão `15`, auto-pagina 2)      |
| R05 | MUST cap `--parallel` at 5 by default                              | DEVE limitar `--parallel` em 5 por padrão                            |
| R06 | MUST use `--output` for large sets                                 | DEVE usar `--output` para conjuntos grandes                          |
| R07 | NEVER invoke without `timeout`                                     | JAMAIS invocar sem `timeout`                                         |
| R08 | MUST use `--queries-file` for batch                                | DEVE usar `--queries-file` para lotes                                |
| R09 | NEVER use `--stream` (placeholder)                                 | JAMAIS usar `--stream` (placeholder)                                 |
| R10 | MUST prefer `--endpoint html`                                      | DEVE preferir `--endpoint html`                                      |
| R11 | MUST distinguish single vs multi-query JSON root                   | DEVE distinguir raiz JSON única vs múltipla                          |
| R12 | MUST treat `titulo`/`url` as guaranteed                            | DEVE tratar `titulo`/`url` como garantidos                           |
| R13 | NEVER assume optional fields present                               | JAMAIS assumir campos opcionais presentes                            |
| R14 | MUST read `.metadados.tempo_execucao_ms`                           | DEVE ler `.metadados.tempo_execucao_ms`                              |
| R15 | MUST check `.metadados.usou_endpoint_fallback`                     | DEVE verificar `.metadados.usou_endpoint_fallback`                   |
| R16 | MUST use `.quantidade_resultados` over length                      | DEVE usar `.quantidade_resultados` em vez de length                  |
| R17 | MUST gate content fields on `--fetch-content`                      | DEVE condicionar campos de conteúdo a `--fetch-content`              |
| R18 | MUST respect `--per-host-limit` 2                                  | DEVE respeitar `--per-host-limit` 2                                  |
| R19 | MUST use built-in `--retries`                                      | DEVE usar `--retries` interno                                        |
| R20 | MUST raise `--global-timeout` for batches                          | DEVE elevar `--global-timeout` para lotes                            |
| R21 | MUST branch on exit code 0/1/2/3/4/5                               | DEVE bifurcar em exit code 0/1/2/3/4/5                               |
| R22 | MUST parse `erro`/`mensagem` on failure                            | DEVE parsear `erro`/`mensagem` em falhas                             |
| R23 | NEVER swallow non-zero exit codes                                  | JAMAIS engolir exit codes não-zero                                   |
| R24 | MUST rely on auto-pagination                                       | DEVE confiar em auto-paginação                                       |
| R25 | MUST treat `--fetch-content` as expensive                          | DEVE tratar `--fetch-content` como caro                              |
| R26 | MUST prefer one batched invocation                                 | DEVE preferir invocação única em lote                                |
| R27 | NEVER pass secrets in argv                                         | JAMAIS passar segredos em argv                                       |
| R28 | MUST understand proxy precedence                                   | DEVE entender precedência de proxy                                   |
| R29 | NEVER execute result URLs without sandbox                          | JAMAIS executar URLs de resultados sem sandbox                       |
| R30 | MUST dry-run `init-config` before `--force`                        | DEVE dry-run `init-config` antes de `--force`                        |

End of AGENT_RULES.md · Upstream: https://github.com/daniloaguiarbr/duckduckgo-search-cli · Schema contract valid for `duckduckgo-search-cli` v0.4.x.
