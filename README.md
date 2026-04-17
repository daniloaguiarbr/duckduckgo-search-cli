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

`duckduckgo-search-cli` is a single Rust binary that turns any shell into a first-class search tool. No API key. No tracking. No Chrome in the hot path. Just a stable JSON schema, bounded concurrency, and predictable exit codes — exactly what an agent needs to ground itself in real-time web data without becoming a liability.

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
- **Pure `rustls-tls`.** No OpenSSL, no SChannel surprises, static musl builds work on the first try inside any Alpine container.
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

### Quick Start

```bash
cargo install duckduckgo-search-cli
duckduckgo-search-cli "rust async runtime"
# 15 fresh JSON results on your desk.

# For LLMs and agents:
duckduckgo-search-cli "tokio JoinSet examples" --num 15 -q | jaq '.resultados'
```

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
cargo test --test integracao_wiremock
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

| Command                                    | Purpose                                                |
| ------------------------------------------ | ------------------------------------------------------ |
| `duckduckgo-search-cli <QUERY>...`         | Default search (equivalent to `buscar`).               |
| `duckduckgo-search-cli buscar <QUERY>...`  | Explicit search subcommand.                            |
| `duckduckgo-search-cli init-config`        | Write `selectors.toml` and `user-agents.toml` to XDG.  |

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

### Environment variables

| Variable       | Description                                                 | Example                            |
| -------------- | ----------------------------------------------------------- | ---------------------------------- |
| `RUST_LOG`     | Overrides the `tracing-subscriber` filter.                  | `RUST_LOG=duckduckgo=debug`        |
| `HTTP_PROXY`   | Default HTTP proxy (lower priority than `--proxy`).         | `http://user:pass@proxy:8080`      |
| `HTTPS_PROXY`  | Default HTTPS proxy.                                        | `http://proxy:8443`                |
| `ALL_PROXY`    | Fallback proxy for any scheme.                              | `socks5://127.0.0.1:9050`          |
| `CHROME_PATH`  | Fallback Chrome path (feature `chrome`).                    | `/opt/google/chrome/chrome`        |

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

### Migration notes (v0.3.x → v0.4.0)

- `--num` now defaults to `15` (previously the full single-page payload, roughly 11). Scripts that processed "all results" continue to work — you just get a consistent count.
- When `--num > 10` and `--pages` is left at the default `1`, the CLI automatically raises `--pages` to `ceil(num / 10)` (capped at 5). Pass `--pages 1` explicitly to force a single page.
- JSON schema unchanged: `resultados[]`, `metadados`, `titulo_original` remain exactly as in v0.3.x.

See the [CHANGELOG](CHANGELOG.md) for release history.

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
- **`rustls-tls` puro.** Sem OpenSSL, sem surpresas no SChannel, builds musl estáticas funcionam de primeira em qualquer container Alpine.
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
cargo test --test integracao_wiremock
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
| `duckduckgo-search-cli buscar <QUERY>...`  | Subcomando explícito de busca.                          |
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

### Notas de migração (v0.3.x → v0.4.0)

- `--num` agora é `15` por padrão (antes era o payload completo de uma página, ~11). Scripts que processavam "todos os resultados" continuam funcionando — você só ganha uma contagem consistente.
- Quando `--num > 10` e `--pages` permanece no default `1`, o CLI eleva automaticamente `--pages` para `ceil(num / 10)` (limitado a 5). Passe `--pages 1` explicitamente para forçar uma única página.
- Schema JSON inalterado: `resultados[]`, `metadados` e `titulo_original` permanecem idênticos à v0.3.x.

Veja o [CHANGELOG](CHANGELOG.md) para o histórico completo.

Licença: MIT OR Apache-2.0.
