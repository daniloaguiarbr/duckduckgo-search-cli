# AGENT RULES — `duckduckgo-search-cli`

> **Imperative rules for AI agents invoking `duckduckgo-search-cli` in production pipelines.**
> **Regras imperativas para agentes de IA que invocam `duckduckgo-search-cli` em pipelines de produção.**

Version: v0.4.1 · Schema: stable · Audience: Claude Code · Cursor · Codex · Aider · any autonomous agent.

---

## TL;DR — If you are an AI agent, do THESE 5 things and skip the rest

1. **ALWAYS** pipe with `-q -f json` and parse with `jaq`. NEVER parse text output.
2. **ALWAYS** wrap rate-limited calls with `timeout 60` and a sane `--parallel` (≤ 5 unless you own the outbound IP).
3. **NEVER** assume optional JSON fields (`snippet`, `url_exibicao`, `titulo_original`) exist — use `jaq ' // "" '` fallbacks.
4. **ALWAYS** check the process exit code: `0` success, `3` block, `4` global timeout, `5` zero results — each demands a different strategy.
5. **NEVER** hardcode API keys, proxies, or User-Agents into arguments — they belong in `$XDG_CONFIG_HOME/duckduckgo-search-cli/` or environment variables.

---

## 🇬🇧 ENGLISH

### A. Core Invariants

#### R01 — MUST pass `-q` (`--quiet`) when piping output to a parser

The default mode emits `tracing` logs to stderr. `-q` is the **explicit contract** that downstream parsers see only stdout payload.

```bash
timeout 30 duckduckgo-search-cli "rust async runtime" -q --num 15 | jaq '.resultados[].url'
```

#### R02 — MUST specify `-f json` (or rely on `auto` in a pipe)

When stdout is not a TTY, `auto` resolves to `json`. Still, **always be explicit** in scripts to survive redirection, `tee`, and CI weirdness.

```bash
duckduckgo-search-cli "query" -q -f json --num 15 | jaq '.quantidade_resultados'
```

#### R03 — MUST NOT rely on `text` or `markdown` formats for machine parsing

`text` and `markdown` are human-facing. They are **not stable contracts**. Only `json` has a versioned schema.

```bash
# ❌ NEVER
duckduckgo-search-cli "q" -f text | rg 'http'
# ✅ ALWAYS
duckduckgo-search-cli "q" -q -f json | jaq -r '.resultados[].url'
```

#### R04 — MUST pass `--num` explicitly; default is `15` with auto-pagination to 2 pages

Relying on defaults silently breaks when defaults change. Pin the number your pipeline was tested with.

```bash
duckduckgo-search-cli "q" -q --num 15
duckduckgo-search-cli "q" -q --num 30 --pages 3   # override pages when needing >15
```

#### R05 — MUST cap `--parallel` at 5 unless you control outbound IP reputation

The default is `5`. The hard maximum is `20`. Going beyond 5 risks DuckDuckGo anti-bot (exit code `3`).

```bash
duckduckgo-search-cli --queries-file q.txt -q --parallel 5
```

#### R06 — MUST use `--output <file>` for large result sets instead of shell redirection

`--output` creates parent directories, sets Unix permissions `0o644`, and forces `json` even in TTY mode.

```bash
duckduckgo-search-cli "q" --num 50 --pages 4 --output /tmp/out/results.json -q
```

#### R07 — NEVER invoke the binary without `timeout` when the network is involved

Agents must fence I/O. Use `timeout 60` (single query) or `timeout 300` (batch of many queries).

```bash
timeout 60 duckduckgo-search-cli "q" -q
timeout 300 duckduckgo-search-cli --queries-file big.txt -q --parallel 5
```

#### R08 — MUST use `--queries-file` for batch work instead of shelling out in a loop

One process reuses HTTP connection pooling, UA rotation, and per-host rate limiting.

```bash
printf 'query one\nquery two\nquery three\n' > /tmp/q.txt
duckduckgo-search-cli --queries-file /tmp/q.txt -q --parallel 3 -f json
```

#### R09 — NEVER use `--stream`

The flag exists as a placeholder. It is **not implemented** in v0.4.x. Treat it as reserved.

#### R10 — MUST prefer `--endpoint html` (default); fall back to `lite` only on repeated anti-bot

`html` gives richer metadata. `lite` is a degradation strategy, not a starting point.

```bash
duckduckgo-search-cli "q" -q --endpoint html --num 15
duckduckgo-search-cli "q" -q --endpoint lite --num 15   # only after exit code 3
```

### B. JSON Output Contract

#### R11 — MUST treat the root object differently for single-query vs multi-query invocations

- **Single query** → root is a `SaidaBusca` object (`.query`, `.resultados`, `.metadados`).
- **Multi-query or `--queries-file`** → root is `{ "quantidade_queries", "buscas": [SaidaBusca, ...] }`.

```bash
duckduckgo-search-cli "one" -q | jaq '.resultados | length'
duckduckgo-search-cli "one" "two" -q | jaq '.buscas[0].resultados | length'
```

#### R12 — MUST access `.resultados[].titulo` and `.resultados[].url` as guaranteed fields

These two are **always present** when `resultados` is non-empty. They are not `Option<String>`.

```bash
duckduckgo-search-cli "q" -q | jaq -r '.resultados[] | "\(.posicao): \(.titulo) — \(.url)"'
```

#### R13 — NEVER assume `snippet`, `url_exibicao`, or `titulo_original` are present

They are `Option<String>`. Absent means the field was not extractable, not that the result is invalid.

```bash
duckduckgo-search-cli "q" -q | jaq '.resultados[] | {
  titulo,
  url,
  snippet: (.snippet // ""),
  url_exibicao: (.url_exibicao // .url),
  titulo_original: (.titulo_original // .titulo)
}'
```

#### R14 — MUST read `.metadados.tempo_execucao_ms` for latency observability

This is the canonical latency signal. Do not measure wall-clock time in your wrapper — trust the binary's internal clock.

```bash
duckduckgo-search-cli "q" -q | jaq '.metadados.tempo_execucao_ms'
```

#### R15 — MUST check `.metadados.usou_endpoint_fallback` before declaring a clean run

If `true`, DuckDuckGo forced a degradation from `html` to `lite`. Log this; it predicts future rate limiting.

```bash
duckduckgo-search-cli "q" -q | jaq '.metadados.usou_endpoint_fallback'
```

#### R16 — MUST use `.quantidade_resultados` instead of `(.resultados | length)` for counts

Both return the same number today, but the field is the **declared contract**. Prefer declared contracts over derived values.

```bash
duckduckgo-search-cli "q" -q --num 15 | jaq '.quantidade_resultados'
```

#### R17 — MUST only trust `.conteudo`, `.tamanho_conteudo`, `.metodo_extracao_conteudo` when `--fetch-content` was passed

Without `--fetch-content`, these fields are absent. Never probe for them blindly.

```bash
duckduckgo-search-cli "q" -q --fetch-content --max-content-length 10000 \
  | jaq '.resultados[] | {url, size: (.tamanho_conteudo // 0)}'
```

### C. Rate Limiting & Etiquette

#### R18 — MUST respect `--per-host-limit` (default `2`, max `10`)

Raising this directly increases the chance of HTTP `202` anti-bot responses (exit code `3`).

```bash
duckduckgo-search-cli --queries-file q.txt -q --per-host-limit 2 --parallel 5
```

#### R19 — MUST use `--retries` (default `2`, max `10`) instead of wrapping in a shell retry loop

The built-in retry uses exponential backoff tuned to DuckDuckGo's rate limits.

```bash
duckduckgo-search-cli "q" -q --retries 3 --timeout 20
```

#### R20 — MUST increase `--global-timeout` for batch jobs (default `60` seconds, max `3600`)

The default is per-CLI-invocation across **all** queries. A `--queries-file` with 50 queries at `--parallel 5` needs ~600 s.

```bash
duckduckgo-search-cli --queries-file big.txt -q --parallel 5 --global-timeout 600
```

### D. Error Handling

#### R21 — MUST branch on exit code before parsing stdout

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

#### R22 — MUST parse `erro` and `mensagem` fields when exit code is non-zero AND stdout contains JSON

On some failures the CLI still emits structured JSON to stdout.

```bash
duckduckgo-search-cli "q" -q -f json \
  | jaq 'if has("erro") then {erro, mensagem} else .resultados end'
```

#### R23 — NEVER silently swallow non-zero exit codes

An agent that ignores exit codes cannot reason about its own failures. Fail loud unless you have a specific retry strategy.

```bash
# ❌ NEVER
duckduckgo-search-cli "q" -q 2>/dev/null || true
# ✅ ALWAYS
duckduckgo-search-cli "q" -q || { echo "failed: $?" >&2; exit 1; }
```

### E. Performance

#### R24 — MUST rely on auto-pagination (default 2 pages) instead of multiple CLI calls

The binary pools connections and reuses UA selection across pages. Invoking it twice for pages 1 and 2 doubles DNS, TLS, and UA rotation overhead.

```bash
# ✅ one call, 2 pages fetched automatically
duckduckgo-search-cli "q" -q --num 15
# ❌ two calls, wasted overhead
duckduckgo-search-cli "q" -q --num 7 --pages 1
duckduckgo-search-cli "q" -q --num 8 --pages 2
```

#### R25 — MUST treat `--fetch-content` as expensive; use only when the agent will consume the body

Enabling `--fetch-content` multiplies latency by N (N = result count). Use `--max-content-length` to cap memory.

```bash
duckduckgo-search-cli "q" -q --num 5 --fetch-content --max-content-length 5000
```

#### R26 — MUST prefer one CLI invocation with `--queries-file` over many sequential invocations

Process startup cost is ~30–80 ms. Batching amortizes this across all queries.

### F. Security

#### R27 — NEVER pass secrets in command-line arguments

Proxy credentials, API keys, or tokens on `argv` leak via `/proc/*/cmdline`, shell history, and `ps`. Use environment variables or `$XDG_CONFIG_HOME/duckduckgo-search-cli/`.

```bash
# ❌ NEVER
duckduckgo-search-cli "q" --proxy http://user:pw@host:8080
# ✅ ALWAYS
export HTTPS_PROXY="http://user:pw@host:8080"
duckduckgo-search-cli "q" -q
```

#### R28 — MUST understand proxy precedence

`--no-proxy` > `--proxy <URL>` > `HTTPS_PROXY` / `HTTP_PROXY` environment > none.

```bash
duckduckgo-search-cli "q" -q --no-proxy   # bypass all proxies
```

#### R29 — NEVER execute URLs from `.resultados[].url` without sandboxing

Results are untrusted third-party URLs. Fetch them only inside isolated processes (containers, VMs, or network-sandboxed HTTP clients).

#### R30 — MUST run `init-config --dry-run` before `--force` in unattended pipelines

`--force` overwrites existing `selectors.toml` and `user-agents.toml`. Dry-run first in CI.

```bash
duckduckgo-search-cli init-config --dry-run
duckduckgo-search-cli init-config --force   # only after review
```

### G. Anti-Patterns

#### AP-01 — Parsing text output with grep

```bash
# ❌ NEVER
duckduckgo-search-cli "q" | rg 'http[s]?://'
# ✅ ALWAYS
duckduckgo-search-cli "q" -q -f json | jaq -r '.resultados[].url'
```

#### AP-02 — Shell loop over queries

```bash
# ❌ NEVER
for q in "a" "b" "c"; do duckduckgo-search-cli "$q" -q ; done
# ✅ ALWAYS
printf 'a\nb\nc\n' | duckduckgo-search-cli --queries-file /dev/stdin -q --parallel 3
```

#### AP-03 — Ignoring exit code

```bash
# ❌ NEVER
duckduckgo-search-cli "q" -q > out.json
jaq '.resultados' out.json
# ✅ ALWAYS
duckduckgo-search-cli "q" -q > out.json || { echo "code=$?" >&2; exit 1; }
jaq '.resultados' out.json
```

#### AP-04 — Assuming `snippet` is a string

```bash
# ❌ NEVER
jaq -r '.resultados[].snippet | ascii_downcase'
# ✅ ALWAYS
jaq -r '.resultados[] | (.snippet // "") | ascii_downcase'
```

#### AP-05 — Hardcoding proxy in argv

Covered by R27; repeated here because agents keep doing it.

#### AP-06 — Raising `--parallel` to 20 "to go faster"

```bash
# ❌ NEVER
duckduckgo-search-cli --queries-file big.txt -q --parallel 20
# ✅ ALWAYS
duckduckgo-search-cli --queries-file big.txt -q --parallel 5 --global-timeout 600
```

#### AP-07 — Using `--stream`

```bash
# ❌ NEVER — placeholder, not implemented
duckduckgo-search-cli "q" --stream
# ✅ ALWAYS — omit the flag
duckduckgo-search-cli "q" -q -f json
```

#### AP-08 — No `timeout` wrapper

```bash
# ❌ NEVER
duckduckgo-search-cli "q" -q
# ✅ ALWAYS
timeout 60 duckduckgo-search-cli "q" -q
```

---

## 🇧🇷 PORTUGUÊS

### A. Invariantes Centrais

#### R01 — DEVE passar `-q` (`--quiet`) ao canalizar saída para parser

O modo padrão emite logs `tracing` em stderr. `-q` é o **contrato explícito** de que o parser a jusante verá apenas payload em stdout.

```bash
timeout 30 duckduckgo-search-cli "rust async runtime" -q --num 15 | jaq '.resultados[].url'
```

#### R02 — DEVE especificar `-f json` (ou confiar em `auto` dentro de pipe)

Quando stdout não é TTY, `auto` resolve para `json`. Ainda assim, **seja sempre explícito** em scripts para sobreviver a redirecionamento, `tee` e esquisitices de CI.

```bash
duckduckgo-search-cli "consulta" -q -f json --num 15 | jaq '.quantidade_resultados'
```

#### R03 — JAMAIS dependa dos formatos `text` ou `markdown` para parsing

`text` e `markdown` são para humanos. **Não têm contrato estável**. Apenas `json` tem schema versionado.

```bash
# ❌ JAMAIS
duckduckgo-search-cli "consulta" -f text | rg 'http'
# ✅ SEMPRE
duckduckgo-search-cli "consulta" -q -f json | jaq -r '.resultados[].url'
```

#### R04 — DEVE passar `--num` explicitamente; padrão é `15` com auto-paginação para 2 páginas

Depender de padrões quebra silenciosamente quando padrões mudam. Fixe o número que seu pipeline testou.

```bash
duckduckgo-search-cli "consulta" -q --num 15
duckduckgo-search-cli "consulta" -q --num 30 --pages 3
```

#### R05 — DEVE limitar `--parallel` em 5 salvo controle próprio da reputação do IP de saída

O padrão é `5`. O máximo rígido é `20`. Ir além de 5 arrisca anti-bot do DuckDuckGo (exit code `3`).

```bash
duckduckgo-search-cli --queries-file q.txt -q --parallel 5
```

#### R06 — DEVE usar `--output <arquivo>` para conjuntos grandes em vez de redirecionamento de shell

`--output` cria diretórios-pai, aplica permissões Unix `0o644` e força `json` mesmo em TTY.

```bash
duckduckgo-search-cli "consulta" --num 50 --pages 4 --output /tmp/saida/resultados.json -q
```

#### R07 — JAMAIS invoque o binário sem `timeout` quando houver rede envolvida

Agentes devem cercar I/O. Use `timeout 60` (query única) ou `timeout 300` (lote grande).

```bash
timeout 60 duckduckgo-search-cli "consulta" -q
timeout 300 duckduckgo-search-cli --queries-file lote.txt -q --parallel 5
```

#### R08 — DEVE usar `--queries-file` para trabalho em lote em vez de loops de shell

Um processo único reaproveita pool HTTP, rotação de UA e rate-limit por host.

```bash
printf 'consulta um\nconsulta dois\nconsulta tres\n' > /tmp/q.txt
duckduckgo-search-cli --queries-file /tmp/q.txt -q --parallel 3 -f json
```

#### R09 — JAMAIS use `--stream`

A flag existe como placeholder. **Não está implementada** na v0.4.x. Trate como reservada.

#### R10 — DEVE preferir `--endpoint html` (padrão); recorra a `lite` só em bloqueio repetido

`html` entrega metadados mais ricos. `lite` é estratégia de degradação, não ponto de partida.

```bash
duckduckgo-search-cli "consulta" -q --endpoint html --num 15
duckduckgo-search-cli "consulta" -q --endpoint lite --num 15   # só após exit code 3
```

### B. Contrato da Saída JSON

#### R11 — DEVE tratar o objeto raiz de forma diferente para query única vs múltiplas

- **Query única** → raiz é objeto `SaidaBusca` (`.query`, `.resultados`, `.metadados`).
- **Múltiplas ou `--queries-file`** → raiz é `{ "quantidade_queries", "buscas": [SaidaBusca, ...] }`.

```bash
duckduckgo-search-cli "uma" -q | jaq '.resultados | length'
duckduckgo-search-cli "uma" "duas" -q | jaq '.buscas[0].resultados | length'
```

#### R12 — DEVE acessar `.resultados[].titulo` e `.resultados[].url` como campos garantidos

Esses dois estão **sempre presentes** quando `resultados` é não-vazio. Não são `Option<String>`.

```bash
duckduckgo-search-cli "consulta" -q | jaq -r '.resultados[] | "\(.posicao): \(.titulo) — \(.url)"'
```

#### R13 — JAMAIS assuma que `snippet`, `url_exibicao` ou `titulo_original` estejam presentes

São `Option<String>`. Ausência significa que o campo não foi extraído, não que o resultado é inválido.

```bash
duckduckgo-search-cli "consulta" -q | jaq '.resultados[] | {
  titulo,
  url,
  snippet: (.snippet // ""),
  url_exibicao: (.url_exibicao // .url),
  titulo_original: (.titulo_original // .titulo)
}'
```

#### R14 — DEVE ler `.metadados.tempo_execucao_ms` para observabilidade de latência

Este é o sinal canônico de latência. Não meça wall-clock no seu wrapper — confie no relógio interno do binário.

```bash
duckduckgo-search-cli "consulta" -q | jaq '.metadados.tempo_execucao_ms'
```

#### R15 — DEVE verificar `.metadados.usou_endpoint_fallback` antes de declarar execução limpa

Se `true`, o DuckDuckGo forçou degradação de `html` para `lite`. Registre isso; prediz rate-limit futuro.

```bash
duckduckgo-search-cli "consulta" -q | jaq '.metadados.usou_endpoint_fallback'
```

#### R16 — DEVE usar `.quantidade_resultados` em vez de `(.resultados | length)` para contagens

Os dois retornam o mesmo número hoje, mas o campo é o **contrato declarado**. Prefira contratos declarados a valores derivados.

```bash
duckduckgo-search-cli "consulta" -q --num 15 | jaq '.quantidade_resultados'
```

#### R17 — DEVE confiar em `.conteudo`, `.tamanho_conteudo`, `.metodo_extracao_conteudo` somente com `--fetch-content`

Sem `--fetch-content` esses campos estão ausentes. Nunca sonde por eles às cegas.

```bash
duckduckgo-search-cli "consulta" -q --fetch-content --max-content-length 10000 \
  | jaq '.resultados[] | {url, tamanho: (.tamanho_conteudo // 0)}'
```

### C. Rate Limiting & Etiqueta

#### R18 — DEVE respeitar `--per-host-limit` (padrão `2`, máx `10`)

Elevar diretamente aumenta probabilidade de respostas HTTP `202` anti-bot (exit code `3`).

```bash
duckduckgo-search-cli --queries-file q.txt -q --per-host-limit 2 --parallel 5
```

#### R19 — DEVE usar `--retries` (padrão `2`, máx `10`) em vez de loop de retry no shell

O retry interno usa backoff exponencial calibrado para os limites do DuckDuckGo.

```bash
duckduckgo-search-cli "consulta" -q --retries 3 --timeout 20
```

#### R20 — DEVE aumentar `--global-timeout` para lotes (padrão `60` s, máx `3600`)

O padrão é por invocação da CLI para **todas** as queries. Um `--queries-file` com 50 queries em `--parallel 5` demanda ~600 s.

```bash
duckduckgo-search-cli --queries-file lote.txt -q --parallel 5 --global-timeout 600
```

### D. Tratamento de Erros

#### R21 — DEVE bifurcar no exit code antes de parsear stdout

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

#### R22 — DEVE parsear `erro` e `mensagem` quando exit code não-zero E stdout for JSON

Em certas falhas a CLI ainda emite JSON estruturado em stdout.

```bash
duckduckgo-search-cli "consulta" -q -f json \
  | jaq 'if has("erro") then {erro, mensagem} else .resultados end'
```

#### R23 — JAMAIS engula silenciosamente exit codes não-zero

Um agente que ignora exit codes não consegue raciocinar sobre suas próprias falhas. Falhe alto salvo estratégia específica de retry.

```bash
# ❌ JAMAIS
duckduckgo-search-cli "consulta" -q 2>/dev/null || true
# ✅ SEMPRE
duckduckgo-search-cli "consulta" -q || { echo "falhou: $?" >&2; exit 1; }
```

### E. Performance

#### R24 — DEVE confiar na auto-paginação (padrão 2 páginas) em vez de múltiplas chamadas

O binário pooliza conexões e reaproveita seleção de UA entre páginas. Invocá-lo duas vezes para páginas 1 e 2 duplica DNS, TLS e rotação de UA.

```bash
# ✅ uma chamada, 2 páginas buscadas automaticamente
duckduckgo-search-cli "consulta" -q --num 15
# ❌ duas chamadas, overhead desperdiçado
duckduckgo-search-cli "consulta" -q --num 7 --pages 1
duckduckgo-search-cli "consulta" -q --num 8 --pages 2
```

#### R25 — DEVE tratar `--fetch-content` como caro; use apenas quando o agente for consumir o corpo

Ativar `--fetch-content` multiplica latência por N (N = número de resultados). Use `--max-content-length` para limitar memória.

```bash
duckduckgo-search-cli "consulta" -q --num 5 --fetch-content --max-content-length 5000
```

#### R26 — DEVE preferir uma invocação com `--queries-file` a múltiplas chamadas sequenciais

O custo de startup do processo é ~30–80 ms. Lotes amortizam isso em todas as queries.

### F. Segurança

#### R27 — JAMAIS passe segredos em argumentos de linha de comando

Credenciais de proxy, API keys ou tokens em `argv` vazam via `/proc/*/cmdline`, histórico de shell e `ps`. Use variáveis de ambiente ou `$XDG_CONFIG_HOME/duckduckgo-search-cli/`.

```bash
# ❌ JAMAIS
duckduckgo-search-cli "consulta" --proxy http://user:senha@host:8080
# ✅ SEMPRE
export HTTPS_PROXY="http://user:senha@host:8080"
duckduckgo-search-cli "consulta" -q
```

#### R28 — DEVE entender a precedência de proxy

`--no-proxy` > `--proxy <URL>` > `HTTPS_PROXY` / `HTTP_PROXY` de ambiente > nenhum.

```bash
duckduckgo-search-cli "consulta" -q --no-proxy   # bypass de todos os proxies
```

#### R29 — JAMAIS execute URLs de `.resultados[].url` sem sandbox

Resultados são URLs terceirizadas não-confiáveis. Busque-as apenas em processos isolados (containers, VMs, ou clientes HTTP com sandbox de rede).

#### R30 — DEVE rodar `init-config --dry-run` antes de `--force` em pipelines não-assistidos

`--force` sobrescreve `selectors.toml` e `user-agents.toml` existentes. Dry-run primeiro em CI.

```bash
duckduckgo-search-cli init-config --dry-run
duckduckgo-search-cli init-config --force   # apenas após revisão
```

### G. Anti-Padrões

#### AP-01 — Parsear saída textual com grep

```bash
# ❌ JAMAIS
duckduckgo-search-cli "consulta" | rg 'http[s]?://'
# ✅ SEMPRE
duckduckgo-search-cli "consulta" -q -f json | jaq -r '.resultados[].url'
```

#### AP-02 — Loop de shell sobre queries

```bash
# ❌ JAMAIS
for q in "a" "b" "c"; do duckduckgo-search-cli "$q" -q ; done
# ✅ SEMPRE
printf 'a\nb\nc\n' | duckduckgo-search-cli --queries-file /dev/stdin -q --parallel 3
```

#### AP-03 — Ignorar exit code

```bash
# ❌ JAMAIS
duckduckgo-search-cli "consulta" -q > out.json
jaq '.resultados' out.json
# ✅ SEMPRE
duckduckgo-search-cli "consulta" -q > out.json || { echo "code=$?" >&2; exit 1; }
jaq '.resultados' out.json
```

#### AP-04 — Assumir que `snippet` é string

```bash
# ❌ JAMAIS
jaq -r '.resultados[].snippet | ascii_downcase'
# ✅ SEMPRE
jaq -r '.resultados[] | (.snippet // "") | ascii_downcase'
```

#### AP-05 — Hardcodar proxy em argv

Coberto pela R27; repetido aqui porque agentes insistem em cometer.

#### AP-06 — Subir `--parallel` para 20 "para acelerar"

```bash
# ❌ JAMAIS
duckduckgo-search-cli --queries-file lote.txt -q --parallel 20
# ✅ SEMPRE
duckduckgo-search-cli --queries-file lote.txt -q --parallel 5 --global-timeout 600
```

#### AP-07 — Usar `--stream`

```bash
# ❌ JAMAIS — placeholder, não implementado
duckduckgo-search-cli "consulta" --stream
# ✅ SEMPRE — omita a flag
duckduckgo-search-cli "consulta" -q -f json
```

#### AP-08 — Ausência de `timeout` envoltório

```bash
# ❌ JAMAIS
duckduckgo-search-cli "consulta" -q
# ✅ SEMPRE
timeout 60 duckduckgo-search-cli "consulta" -q
```

---

## Quick Reference Card

| ID  | 🇬🇧 English                                                        | 🇧🇷 Português                                                        |
|-----|--------------------------------------------------------------------|----------------------------------------------------------------------|
| R01 | MUST pass `-q` when piping to parser                               | DEVE passar `-q` ao canalizar para parser                            |
| R02 | MUST specify `-f json` explicitly in scripts                       | DEVE especificar `-f json` explicitamente em scripts                 |
| R03 | NEVER parse `text` / `markdown` for machines                       | JAMAIS parsear `text` / `markdown` para máquinas                     |
| R04 | MUST pass `--num` explicitly (default `15`, auto-paginates 2)      | DEVE passar `--num` explicitamente (padrão `15`, auto-pagina 2)      |
| R05 | MUST cap `--parallel` ≤ 5 by default                               | DEVE limitar `--parallel` ≤ 5 por padrão                             |
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

---

**End of AGENT_RULES.md** · Upstream: https://github.com/daniloaguiarbr/duckduckgo-search-cli · Schema contract valid for `duckduckgo-search-cli` v0.4.x.
