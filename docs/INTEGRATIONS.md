# `duckduckgo-search-cli` — Integration Guide for 16 AI Agents / LLMs

> The definitive copy-paste playbook for plugging **`duckduckgo-search-cli`** into every major AI coding agent and LLM harness on the market. Find your agent, copy the snippet, ship.

[![Crates.io](https://img.shields.io/crates/v/duckduckgo-search-cli)](https://crates.io/crates/duckduckgo-search-cli)
[![Docs.rs](https://img.shields.io/docsrs/duckduckgo-search-cli)](https://docs.rs/duckduckgo-search-cli)

---

## 📋 Agent Index / Índice de Agentes

| # | Agent | Shell mechanism | Jump |
|---|---|---|---|
| 1 | Claude Code (Anthropic) | Bash tool | [EN](#1-claude-code) · [PT](#1-claude-code-1) |
| 2 | OpenAI Codex | Shell / exec tool | [EN](#2-openai-codex) · [PT](#2-openai-codex-1) |
| 3 | Gemini CLI (Google) | Shell tool | [EN](#3-gemini-cli) · [PT](#3-gemini-cli-1) |
| 4 | Cursor | Terminal + chat | [EN](#4-cursor) · [PT](#4-cursor-1) |
| 5 | Windsurf (Codeium) | Cascade terminal | [EN](#5-windsurf) · [PT](#5-windsurf-1) |
| 6 | Aider | `/run` command | [EN](#6-aider) · [PT](#6-aider-1) |
| 7 | Continue.dev | Custom slash command | [EN](#7-continuedev) · [PT](#7-continuedev-1) |
| 8 | MiniMax Agent | Agent tool / API | [EN](#8-minimax-agent) · [PT](#8-minimax-agent-1) |
| 9 | OpenCode | Shell tool | [EN](#9-opencode) · [PT](#9-opencode-1) |
| 10 | Paperclip | Agent capability | [EN](#10-paperclip) · [PT](#10-paperclip-1) |
| 11 | OpenClaw | CLI tool binding | [EN](#11-openclaw) · [PT](#11-openclaw-1) |
| 12 | Google Antigravity | Agent shell | [EN](#12-google-antigravity) · [PT](#12-google-antigravity-1) |
| 13 | GitHub Copilot CLI | `gh copilot` | [EN](#13-github-copilot-cli) · [PT](#13-github-copilot-cli-1) |
| 14 | Devin (Cognition) | Cloud sandbox | [EN](#14-devin) · [PT](#14-devin-1) |
| 15 | Cline | VS Code terminal | [EN](#15-cline) · [PT](#15-cline-1) |
| 16 | Roo Code | VS Code terminal | [EN](#16-roo-code) · [PT](#16-roo-code-1) |

---

## ⚙️ Baseline Contract / Contrato Base

- **Binary:** `duckduckgo-search-cli`
- **Install:** `cargo install duckduckgo-search-cli`
- **Defaults:** `--num 15` (auto-paginates 2 pages), `-f auto` (JSON in pipes, text in TTY)
- **Key flags:** `-q` (quiet), `-f json|text|markdown`, `-o FILE`, `--queries-file`, `--fetch-content`, `--time-filter d|w|m|y`, `--proxy`, `--global-timeout 60`, `--parallel 5`
- **Exit codes:** `0` success · `1` runtime · `2` config · `3` block · `4` timeout · `5` zero results
- **JSON schema (single query):**
  ```json
  {
    "query": "...", "motor": "duckduckgo", "endpoint": "html",
    "timestamp": "2026-04-14T10:00:00Z", "regiao": "br-pt",
    "quantidade_resultados": 15, "paginas_buscadas": 2,
    "resultados": [
      {"posicao": 1, "titulo": "...", "url": "...", "snippet": "...", "url_exibicao": "...", "titulo_original": "..."}
    ],
    "metadados": {"tempo_execucao_ms": 1234, "user_agent": "..."}
  }
  ```
- **Multi-query schema:** `{quantidade_queries, timestamp, paralelismo, buscas: [<SingleSchema>]}`

---

# 🇬🇧 ENGLISH SECTION

## 1. Claude Code

> Anthropic's official terminal companion — gives Claude first-class `Bash` tool access with XDG-aware hooks and slash commands.

**How it invokes shell:** Native `Bash` tool; optional hooks via `~/.claude/settings.json`.

**Setup:**
```bash
cargo install duckduckgo-search-cli --force
duckduckgo-search-cli --version   # expect 0.4.x
```

**Snippet — Basic search (paste in chat):**
> "Run `timeout 30 duckduckgo-search-cli "rust async tokio" -q --num 15 | jaq '.resultados[] | {titulo, url, snippet}'` and summarize the top 5 results for me."

**Snippet — Multi-query research:**
> "Create `/tmp/queries.txt` with these 5 search queries (one per line), then run:
> `timeout 120 duckduckgo-search-cli --queries-file /tmp/queries.txt -q -f json --parallel 5 -o /tmp/research.json` and read the JSON file."

**System prompt rule (add to `CLAUDE.md`):**
> When you need fresh web context, ALWAYS use `duckduckgo-search-cli QUERY -q --num 15` instead of fabricating links or relying on training data. Parse JSON output with `jaq`.

**Caveats:**
- Sandbox may require `cargo install` approval on first run.
- Use `--global-timeout 60` for autonomous / unattended runs.

---

## 2. OpenAI Codex

> OpenAI's coding agent family (Codex CLI + IDE extensions) — supports shell/exec tool for deterministic command runs.

**How it invokes shell:** `shell` / `exec` tool with approval prompt (configurable to `auto` in trusted sandboxes).

**Setup:**
```bash
cargo install duckduckgo-search-cli
# Optionally set approval to 'on-failure' for faster iteration
codex config set approval on-failure
```

**Snippet — Basic search:**
> "Use the shell tool to execute:
> `duckduckgo-search-cli "postgres jsonb index performance" -q --num 15 -f json`
> then extract titles + urls with jaq."

**Snippet — Multi-query research:**
> "Write queries to `./research.txt`, then run:
> `duckduckgo-search-cli --queries-file ./research.txt -q -f json --parallel 5 --global-timeout 90 -o ./out.json`
> and show me the first 3 results per query."

**System prompt rule:**
> Always prefer `duckduckgo-search-cli` (installed globally) over inventing URLs. Default to `-q --num 15 -f json` and pipe through `jaq`.

**Caveats:**
- Codex CLI will prompt for command approval unless sandbox mode is `workspace-write`.
- Set `--global-timeout 60` to avoid hitting the agent's per-step budget.

---

## 3. Gemini CLI

> Google's official Gemini terminal agent — ships with a `run_shell_command` tool that the user explicitly grants.

**How it invokes shell:** `run_shell_command` tool, permission-gated per command prefix.

**Setup:**
```bash
cargo install duckduckgo-search-cli
gemini   # launches REPL; allow `duckduckgo-search-cli` on first prompt
```

**Snippet — Basic search:**
> "Run `duckduckgo-search-cli "wasm component model 2025" -q --num 15 | jaq '.resultados[:5]'` and give me a bullet list of the findings."

**Snippet — Multi-query research:**
> "Create `queries.txt`, then run `duckduckgo-search-cli --queries-file queries.txt -q -f json --parallel 5 -o /tmp/gemini_out.json` — read the file and cluster duplicate domains."

**System prompt rule (`.gemini/GEMINI.md`):**
> For web facts, use the shell tool to call `duckduckgo-search-cli QUERY -q --num 15 -f json`. Never fabricate URLs.

**Caveats:**
- First call requires per-session approval; "allow always for this prefix" speeds subsequent runs.
- Respect your project `.gemini/settings.json` `tool_permissions` allowlist.

---

## 4. Cursor

> AI-first fork of VS Code (cursor.com) with Composer agent mode and terminal-aware chat.

**How it invokes shell:** Terminal commands embedded in chat, with agent mode auto-running in Composer.

**Setup:**
```bash
cargo install duckduckgo-search-cli
# Verify from Cursor's integrated terminal:
duckduckgo-search-cli --version
```

**Snippet — Basic search (Composer agent mode):**
> "Run in terminal: `duckduckgo-search-cli "tauri v2 plugin api" -q --num 15 -f json | jaq '.resultados[] | {titulo, url}'` and paste the top 5 into a `RESEARCH.md` file."

**Snippet — Multi-query research:**
> "Create `research_queries.txt` with my 5 questions, then execute:
> `duckduckgo-search-cli --queries-file research_queries.txt -q -f json --parallel 5 -o research.json`
> — summarize each query's top-3 results."

**System prompt rule (`.cursorrules`):**
> Prefer running `duckduckgo-search-cli QUERY -q --num 15` before searching the web mentally. Always pipe to `jaq` and cite URLs verbatim.

**Caveats:**
- In `auto-run` mode, Cursor executes without asking — enforce `--global-timeout 60`.
- Keep `-q` (quiet) to avoid cluttering the agent chat buffer.

---

## 5. Windsurf

> Codeium's agentic IDE with Cascade — has built-in terminal and autonomous command execution.

**How it invokes shell:** Cascade's `run_command` / terminal proposer (user approves or auto-approves).

**Setup:**
```bash
cargo install duckduckgo-search-cli
# Confirm from Windsurf terminal:
which duckduckgo-search-cli
```

**Snippet — Basic search:**
> "Use the terminal to run: `duckduckgo-search-cli "axum tower middleware" -q --num 15 -f json`. Parse with `jaq '.resultados[:5] | map({titulo, url})'` and save to `ctx/search.json`."

**Snippet — Multi-query research:**
> "Write 5 search queries to `queries.txt`, then: `duckduckgo-search-cli --queries-file queries.txt -q -f json --parallel 5 --global-timeout 90 -o ctx/research.json`. Read `ctx/research.json` and identify the 3 most-cited domains."

**System prompt rule (Cascade system instructions):**
> When the user asks for current / web-based information, run `duckduckgo-search-cli QUERY -q --num 15 -f json` via the terminal. Never hallucinate URLs.

**Caveats:**
- Cascade auto-approval can be scoped per-command; whitelist `duckduckgo-search-cli`.
- Disable `--stream` in Cascade — it expects batched JSON.

---

## 6. Aider

> aider.chat — the canonical open-source pair-programmer CLI. Has `/run` for shell invocation piped into context.

**How it invokes shell:** `/run <cmd>` slash command (captures stdout into chat context).

**Setup:**
```bash
pipx install aider-chat
cargo install duckduckgo-search-cli
aider
```

**Snippet — Basic search (inside aider REPL):**
```
/run duckduckgo-search-cli "sqlx postgres migrations" -q --num 15 -f json | jaq '.resultados[:5] | map({titulo, url, snippet})'
```

**Snippet — Multi-query research:**
```
/run echo "rust async tokio\nsqlx postgres\naxum middleware" > /tmp/q.txt && duckduckgo-search-cli --queries-file /tmp/q.txt -q -f json --parallel 3 -o /tmp/r.json && jaq '.buscas[] | {query, top: .resultados[:3] | map(.url)}' /tmp/r.json
```

**System prompt rule (`.aider.conf.yml`):**
```yaml
read: ["AIDER.md"]
```
And in `AIDER.md`:
> Before suggesting code that depends on external libs, run `/run duckduckgo-search-cli "<lib> <question>" -q --num 10 -f json`.

**Caveats:**
- `/run` output is injected into the chat — prefer `-q` and JSON to minimize tokens.
- Aider truncates long outputs; use `--num 10` and `jaq` to pre-filter.

---

## 7. Continue.dev

> Open-source IDE autopilot for VS Code / JetBrains — supports custom slash commands configured in `config.json`.

**How it invokes shell:** Custom commands of type `run` / custom tools (via MCP or `commands` array).

**Setup:**
```bash
cargo install duckduckgo-search-cli
```

**Snippet — `~/.continue/config.json` slash command:**
```json
{
  "slashCommands": [
    {
      "name": "ddg",
      "description": "Search the web via DuckDuckGo",
      "run": "duckduckgo-search-cli \"{{{ input }}}\" -q --num 15 -f json | jaq '.resultados[:5] | map({titulo, url, snippet})'"
    }
  ]
}
```

**Snippet — Basic search (invoke in chat):**
```
/ddg rust async tokio patterns 2026
```

**Snippet — Multi-query research slash command:**
```json
{
  "name": "research",
  "description": "Multi-query DDG research",
  "run": "echo \"{{{ input }}}\" | tr ';' '\\n' > /tmp/q.txt && duckduckgo-search-cli --queries-file /tmp/q.txt -q -f json --parallel 5 -o /tmp/r.json && jaq '.buscas[] | {query, urls: .resultados[:3] | map(.url)}' /tmp/r.json"
}
```

**System prompt rule:** add to Continue's `systemMessage`:
> Use `/ddg` for any web search. Never hallucinate URLs.

**Caveats:**
- Continue v1+ expects slash commands in `~/.continue/config.yaml` — adapt accordingly.
- For team setups, commit the config to the repo as `.continue/config.json`.

---

## 8. MiniMax Agent

> MiniMax's agentic LLM family (M1, Abab) — supports tool/function calling and shell agents.

**How it invokes shell:** Function calling that maps to a `shell_exec` tool you implement in the harness.

**Setup:**
```bash
cargo install duckduckgo-search-cli
```

**Snippet — Tool definition (pass to MiniMax API):**
```json
{
  "name": "web_search",
  "description": "Search the web via duckduckgo-search-cli and return JSON",
  "parameters": {
    "type": "object",
    "properties": { "query": { "type": "string" } },
    "required": ["query"]
  }
}
```
Handler (pseudo-Python, harness-agnostic):
```python
def web_search(query):
    return subprocess.check_output(
        ["duckduckgo-search-cli", query, "-q", "--num", "15", "-f", "json"],
        timeout=60
    )
```

**Snippet — Multi-query (batched function call):**
> "Call `web_search` 5 times in parallel (one per topic), then merge the `resultados` arrays."

Alternative — single command:
```bash
duckduckgo-search-cli --queries-file queries.txt -q -f json --parallel 5 -o out.json
```

**System prompt rule:**
> You have a `web_search` function. Use it whenever you need current information. Always inspect `resultados[].url` and `snippet` before answering.

**Caveats:**
- Enforce harness-side `timeout=60s` — MiniMax will happily wait forever.
- Rate-limit: keep `--parallel` ≤ 5 to avoid DDG 429s.

---

## 9. OpenCode

> Open-source terminal coding agent (competitor to Aider) — supports shell tool out of the box.

**How it invokes shell:** Built-in `shell` tool; configurable via `~/.config/opencode/config.toml`.

**Setup:**
```bash
cargo install duckduckgo-search-cli
opencode --version
```

**Snippet — Basic search (in OpenCode REPL):**
> "Run `duckduckgo-search-cli "tokio select cancel-safety" -q --num 15 -f json | jaq '.resultados[:5]'` and synthesize a one-paragraph answer."

**Snippet — Multi-query research:**
> "Create `/tmp/queries.txt` with my 5 questions, then run:
> `duckduckgo-search-cli --queries-file /tmp/queries.txt -q -f json --parallel 5 -o /tmp/opencode_research.json` and read the file."

**System prompt rule (`~/.config/opencode/prompt.md`):**
> For web queries, ALWAYS invoke `duckduckgo-search-cli QUERY -q --num 15 -f json`. Parse JSON with `jaq`. Cite URLs verbatim.

**Caveats:**
- OpenCode inherits shell approvals from config — whitelist the binary.
- Disable `--stream` (OpenCode buffers stdout).

---

## 10. Paperclip

> AIPaperclip's in-house agent framework (author: @daniloaguiarbr) — first-party integration target for this CLI.

**How it invokes shell:** `bash`/`cli` capability registered in the agent manifest.

**Setup:**
```bash
cargo install duckduckgo-search-cli
# In Paperclip workspace:
paperclip capability add duckduckgo-search-cli
```

**Snippet — Basic search (agent task YAML):**
```yaml
- name: web_search
  cli: duckduckgo-search-cli
  args: ["{{query}}", "-q", "--num", "15", "-f", "json"]
  parse: json
  timeout: 60
```

**Snippet — Multi-query research:**
```yaml
- name: research_sprint
  cli: duckduckgo-search-cli
  args: ["--queries-file", "{{queries_path}}", "-q", "-f", "json",
         "--parallel", "5", "--global-timeout", "120", "-o", "{{out_path}}"]
  parse: json
  timeout: 150
```

**System prompt rule (Paperclip `SYSTEM.md`):**
> Use the `web_search` capability for every factual claim. Never synthesize URLs. Prefer `--num 15` + `jaq`-style filtering.

**Caveats:**
- Paperclip supervises child processes — `--global-timeout 60` is enforced even if you omit it.
- For reproducible runs, pin the CLI version: `cargo install duckduckgo-search-cli --version =0.4.1`.

---

## 11. OpenClaw

> Paperclip-family minimal CLI agent (author: @daniloaguiarbr) — zero-config shell tool binding.

**How it invokes shell:** Direct binary binding via `tools.toml`.

**Setup:**
```bash
cargo install duckduckgo-search-cli
```

**Snippet — `tools.toml` binding:**
```toml
[[tool]]
name = "web"
bin  = "duckduckgo-search-cli"
args = ["{query}", "-q", "--num", "15", "-f", "json"]
timeout_secs = 60
```

**Snippet — Multi-query research:**
```toml
[[tool]]
name = "research"
bin  = "duckduckgo-search-cli"
args = ["--queries-file", "{path}", "-q", "-f", "json",
        "--parallel", "5", "--global-timeout", "120", "-o", "{out}"]
timeout_secs = 150
```

**System prompt rule:**
> Use tool `web` for single queries, tool `research` for multi-query sprints. Do not invent URLs.

**Caveats:**
- OpenClaw passes raw JSON to the LLM — no pre-parsing; rely on the model to read `.resultados`.
- Pair with `jaq` in a second tool call if output exceeds the context window.

---

## 12. Google Antigravity

> Google's experimental agent-first IDE — deep Gemini integration with autonomous shell execution.

**How it invokes shell:** Agent shell tool (mirrors Gemini CLI's `run_shell_command`).

**Setup:**
```bash
cargo install duckduckgo-search-cli
# In Antigravity, open the agent panel and allow 'duckduckgo-search-cli' on first use.
```

**Snippet — Basic search:**
> "Execute: `duckduckgo-search-cli "go generics 1.22 best practices" -q --num 15 -f json | jaq '.resultados[:5]'` and paste findings into `NOTES.md`."

**Snippet — Multi-query research:**
> "Build `queries.txt` with 5 lines, then run:
> `duckduckgo-search-cli --queries-file queries.txt -q -f json --parallel 5 --global-timeout 120 -o /tmp/antigravity_research.json`
> Summarize each query's top-3 in a markdown table."

**System prompt rule (Antigravity agent settings):**
> Prefer `duckduckgo-search-cli` for any web fact. Always `--num 15 -f json`. Cite URLs verbatim.

**Caveats:**
- Antigravity sandboxes network calls; the CLI itself uses HTTPS and is usually whitelisted by default.
- Use `--proxy` if your org mandates egress through a corporate proxy.

---

## 13. GitHub Copilot CLI

> `gh copilot` — the official GitHub Copilot command-line companion (`gh copilot suggest`, `gh copilot explain`).

**How it invokes shell:** Copilot *suggests* commands; the user (or a wrapper script) executes them.

**Setup:**
```bash
gh extension install github/gh-copilot
cargo install duckduckgo-search-cli
```

**Snippet — Basic search (suggest + run):**
```bash
gh copilot suggest "search the web for 'rust axum middleware tower'" --target shell
# Copilot will propose:
duckduckgo-search-cli "rust axum middleware tower" -q --num 15 -f json | jaq '.resultados[:5]'
```

**Snippet — Multi-query wrapper:**
```bash
# Save as ~/.local/bin/ddg-research
#!/usr/bin/env bash
set -euo pipefail
printf '%s\n' "$@" > /tmp/q.txt
duckduckgo-search-cli --queries-file /tmp/q.txt -q -f json --parallel 5 --global-timeout 120
```
Then:
```bash
gh copilot suggest "use ddg-research to compare axum vs actix vs rocket"
```

**System prompt rule:** add to your `~/.bashrc` / `~/.zshrc`:
```bash
export GH_COPILOT_HINTS="Always prefer 'duckduckgo-search-cli QUERY -q --num 15' over ad-hoc curl."
```

**Caveats:**
- `gh copilot` suggests but does **not** auto-execute — wrap with `eval "$(gh copilot suggest ... | tail -1)"` at your own risk.
- Requires a GitHub Copilot subscription.

---

## 14. Devin

> Cognition Labs' autonomous software engineer — cloud sandbox with full shell access.

**How it invokes shell:** Native terminal in the Devin VM; autonomous by default.

**Setup (in Devin session):**
```bash
cargo install duckduckgo-search-cli
# Persist to Devin's machine snapshot so future sessions reuse it:
devin snapshot save "cargo-tools"
```

**Snippet — Basic search (Devin Slack / web prompt):**
> "In the shell, run: `duckduckgo-search-cli "terraform aws eks 2026 best practices" -q --num 15 -f json | jaq '.resultados[:5]'` and append results to `research.md`."

**Snippet — Multi-query research:**
> "Create `queries.txt` (5 lines), then execute:
> `duckduckgo-search-cli --queries-file queries.txt -q -f json --parallel 5 --global-timeout 120 -o research.json`
> Open `research.json` and produce a comparison table."

**System prompt rule (Devin Knowledge):**
> For every web-dependent claim, use `duckduckgo-search-cli` — never fabricate URLs. Prefer `--num 15 -f json` and parse with `jaq`.

**Caveats:**
- First run triggers `cargo install` (2–4 min); save a snapshot to skip that in future sessions.
- Devin may hit DDG rate-limits under high parallelism — keep `--parallel 5`.

---

## 15. Cline

> VS Code extension (formerly Claude Dev) — autonomous coding agent with approval workflows and auto-run mode.

**How it invokes shell:** VS Code integrated terminal; `execute_command` tool with per-command approval.

**Setup:**
```bash
cargo install duckduckgo-search-cli
# From a VS Code terminal that Cline can see:
duckduckgo-search-cli --version
```

**Snippet — Basic search (Cline chat):**
> "Use execute_command to run:
> `duckduckgo-search-cli "rust cargo workspace inheritance" -q --num 15 -f json | jaq '.resultados[:5] | map({titulo, url})'`
> and save the JSON to `./research/ws.json`."

**Snippet — Multi-query research:**
> "Create `./research/queries.txt` with 5 queries, then execute:
> `duckduckgo-search-cli --queries-file ./research/queries.txt -q -f json --parallel 5 --global-timeout 120 -o ./research/out.json`
> Read `out.json` and write a markdown summary to `./research/SUMMARY.md`."

**System prompt rule (`.clinerules`):**
> For any web fact, use `duckduckgo-search-cli QUERY -q --num 15 -f json`. Never hallucinate URLs. Parse JSON with `jaq`.

**Caveats:**
- Auto-approval whitelists: add `duckduckgo-search-cli` to "Auto-approve execute_command" in Cline settings.
- Cline truncates stdout at ~10k tokens — use `-q` + `jaq` projections to stay under budget.

---

## 16. Roo Code

> Community fork of Cline with expanded customization, custom modes, and multi-agent orchestration.

**How it invokes shell:** `execute_command` tool (inherited from Cline); mode-specific approval rules.

**Setup:**
```bash
cargo install duckduckgo-search-cli
```

**Snippet — Basic search (Roo Code chat):**
> "Execute: `duckduckgo-search-cli "rust leptos signals 2026" -q --num 15 -f json | jaq '.resultados[:5]'` — give me a 3-bullet takeaway."

**Snippet — Multi-query research (with custom Roo mode):**
Create a custom mode `researcher` in `.roo/modes.yaml`:
```yaml
- slug: researcher
  name: Web Researcher
  whenToUse: "Invoke for any fact-heavy question"
  customInstructions: |
    Always run:
      duckduckgo-search-cli --queries-file /tmp/q.txt -q -f json --parallel 5 --global-timeout 120 -o /tmp/r.json
    before answering. Cite .resultados[].url verbatim.
  autoApprove: ["execute_command"]
```
Then: `/mode researcher` in chat.

**System prompt rule (`.roorules`):**
> When in `researcher` mode (or whenever factual grounding is needed), use `duckduckgo-search-cli`. Always JSON + jaq.

**Caveats:**
- Per-mode auto-approval: scope `execute_command` tightly to the CLI prefix.
- Roo's multi-agent orchestrator may fan out — cap `--parallel 5` globally to respect DDG limits.

---

# 🇧🇷 SEÇÃO EM PORTUGUÊS

## 1. Claude Code

> Companheiro oficial da Anthropic para terminal — acesso nativo à `Bash` tool, hooks e slash commands.

**Como invoca shell:** `Bash` tool nativa; hooks opcionais em `~/.claude/settings.json`.

**Instalação:**
```bash
cargo install duckduckgo-search-cli --force
duckduckgo-search-cli --version   # esperado 0.4.x
```

**Snippet — Busca básica (cole no chat):**
> "Execute `timeout 30 duckduckgo-search-cli "rust async tokio" -q --num 15 | jaq '.resultados[] | {titulo, url, snippet}'` e resuma os 5 melhores resultados."

**Snippet — Pesquisa multi-query:**
> "Crie `/tmp/queries.txt` com 5 queries (uma por linha) e execute:
> `timeout 120 duckduckgo-search-cli --queries-file /tmp/queries.txt -q -f json --parallel 5 -o /tmp/research.json` e leia o JSON."

**Regra de system prompt (adicione ao `CLAUDE.md`):**
> Quando precisar de contexto web atual, SEMPRE use `duckduckgo-search-cli QUERY -q --num 15` em vez de inventar links. Parse o JSON com `jaq`.

**Cuidados:**
- Sandbox pode pedir aprovação no primeiro `cargo install`.
- Use `--global-timeout 60` para execuções autônomas.

---

## 2. OpenAI Codex

> Família de agentes de código da OpenAI (Codex CLI + extensões IDE) — suporta ferramenta `shell` / `exec`.

**Como invoca shell:** Ferramenta `shell` / `exec` com confirmação (configurável para `auto` em sandbox confiável).

**Instalação:**
```bash
cargo install duckduckgo-search-cli
codex config set approval on-failure
```

**Snippet — Busca básica:**
> "Use a shell tool para executar:
> `duckduckgo-search-cli "postgres jsonb index performance" -q --num 15 -f json`
> e extraia titulos e urls com jaq."

**Snippet — Pesquisa multi-query:**
> "Escreva as queries em `./research.txt` e rode:
> `duckduckgo-search-cli --queries-file ./research.txt -q -f json --parallel 5 --global-timeout 90 -o ./out.json`
> e mostre os 3 primeiros resultados por query."

**Regra de system prompt:**
> Sempre prefira `duckduckgo-search-cli` (instalado globalmente) em vez de inventar URLs. Padrão: `-q --num 15 -f json` + `jaq`.

**Cuidados:**
- Codex CLI pede aprovação exceto em modo sandbox `workspace-write`.
- Use `--global-timeout 60` para respeitar o orçamento por passo.

---

## 3. Gemini CLI

> Agente oficial de terminal do Google Gemini — ferramenta `run_shell_command` com permissão explícita.

**Como invoca shell:** `run_shell_command`, permissão por prefixo de comando.

**Instalação:**
```bash
cargo install duckduckgo-search-cli
gemini   # REPL; aprove o binário na primeira execução
```

**Snippet — Busca básica:**
> "Execute `duckduckgo-search-cli "wasm component model 2025" -q --num 15 | jaq '.resultados[:5]'` e me dê uma lista em bullets."

**Snippet — Pesquisa multi-query:**
> "Crie `queries.txt` e rode `duckduckgo-search-cli --queries-file queries.txt -q -f json --parallel 5 -o /tmp/gemini_out.json` — leia o arquivo e agrupe domínios duplicados."

**Regra de system prompt (`.gemini/GEMINI.md`):**
> Para fatos da web, use a shell tool com `duckduckgo-search-cli QUERY -q --num 15 -f json`. Nunca invente URLs.

**Cuidados:**
- Primeira chamada pede aprovação; "permitir sempre para esse prefixo" agiliza as próximas.
- Respeite a allowlist em `.gemini/settings.json`.

---

## 4. Cursor

> Fork AI-first do VS Code (cursor.com) com agente Composer e chat ciente do terminal.

**Como invoca shell:** Comandos do terminal embutidos no chat; Composer executa automaticamente em modo agente.

**Instalação:**
```bash
cargo install duckduckgo-search-cli
duckduckgo-search-cli --version
```

**Snippet — Busca básica (modo agente Composer):**
> "Execute no terminal: `duckduckgo-search-cli "tauri v2 plugin api" -q --num 15 -f json | jaq '.resultados[] | {titulo, url}'` e salve os 5 melhores num arquivo `RESEARCH.md`."

**Snippet — Pesquisa multi-query:**
> "Crie `research_queries.txt` com minhas 5 perguntas, e execute:
> `duckduckgo-search-cli --queries-file research_queries.txt -q -f json --parallel 5 -o research.json`
> — resuma os 3 melhores de cada query."

**Regra de system prompt (`.cursorrules`):**
> Prefira rodar `duckduckgo-search-cli QUERY -q --num 15` antes de pesquisar mentalmente. Sempre pipe para `jaq` e cite URLs literalmente.

**Cuidados:**
- Em modo `auto-run`, o Cursor executa sem perguntar — exija `--global-timeout 60`.
- Mantenha `-q` para não poluir o buffer do agente.

---

## 5. Windsurf

> IDE agêntico da Codeium com Cascade — terminal integrado e execução autônoma de comandos.

**Como invoca shell:** `run_command` do Cascade (aprovação do usuário ou auto-approve).

**Instalação:**
```bash
cargo install duckduckgo-search-cli
which duckduckgo-search-cli
```

**Snippet — Busca básica:**
> "Use o terminal para rodar: `duckduckgo-search-cli "axum tower middleware" -q --num 15 -f json`. Parse com `jaq '.resultados[:5] | map({titulo, url})'` e salve em `ctx/search.json`."

**Snippet — Pesquisa multi-query:**
> "Escreva 5 queries em `queries.txt`, depois: `duckduckgo-search-cli --queries-file queries.txt -q -f json --parallel 5 --global-timeout 90 -o ctx/research.json`. Leia `ctx/research.json` e identifique os 3 domínios mais citados."

**Regra de system prompt (instruções do Cascade):**
> Quando o usuário pedir informação atual / web, rode `duckduckgo-search-cli QUERY -q --num 15 -f json` via terminal. Nunca alucine URLs.

**Cuidados:**
- Auto-approve do Cascade pode ser restrito por comando; faça whitelist do binário.
- Desative `--stream` no Cascade — ele espera JSON em batch.

---

## 6. Aider

> aider.chat — o pair programmer open-source canônico. Tem `/run` para executar shell com stdout injetado no contexto.

**Como invoca shell:** Slash command `/run <cmd>` (captura stdout para o chat).

**Instalação:**
```bash
pipx install aider-chat
cargo install duckduckgo-search-cli
aider
```

**Snippet — Busca básica (dentro do REPL aider):**
```
/run duckduckgo-search-cli "sqlx postgres migrations" -q --num 15 -f json | jaq '.resultados[:5] | map({titulo, url, snippet})'
```

**Snippet — Pesquisa multi-query:**
```
/run echo "rust async tokio\nsqlx postgres\naxum middleware" > /tmp/q.txt && duckduckgo-search-cli --queries-file /tmp/q.txt -q -f json --parallel 3 -o /tmp/r.json && jaq '.buscas[] | {query, top: .resultados[:3] | map(.url)}' /tmp/r.json
```

**Regra de system prompt (`.aider.conf.yml`):**
```yaml
read: ["AIDER.md"]
```
E no `AIDER.md`:
> Antes de sugerir código com libs externas, rode `/run duckduckgo-search-cli "<lib> <pergunta>" -q --num 10 -f json`.

**Cuidados:**
- Output de `/run` entra no chat — prefira `-q` e JSON para economizar tokens.
- Aider trunca outputs longos; use `--num 10` e `jaq` para filtrar antes.

---

## 7. Continue.dev

> Autopilot IDE open-source para VS Code / JetBrains — suporta slash commands customizados em `config.json`.

**Como invoca shell:** Comandos customizados de tipo `run` (ou ferramentas MCP).

**Instalação:**
```bash
cargo install duckduckgo-search-cli
```

**Snippet — slash command em `~/.continue/config.json`:**
```json
{
  "slashCommands": [
    {
      "name": "ddg",
      "description": "Pesquisa web via DuckDuckGo",
      "run": "duckduckgo-search-cli \"{{{ input }}}\" -q --num 15 -f json | jaq '.resultados[:5] | map({titulo, url, snippet})'"
    }
  ]
}
```

**Snippet — Busca básica (chame no chat):**
```
/ddg rust async tokio patterns 2026
```

**Snippet — Slash command multi-query:**
```json
{
  "name": "research",
  "description": "Pesquisa multi-query DDG",
  "run": "echo \"{{{ input }}}\" | tr ';' '\\n' > /tmp/q.txt && duckduckgo-search-cli --queries-file /tmp/q.txt -q -f json --parallel 5 -o /tmp/r.json && jaq '.buscas[] | {query, urls: .resultados[:3] | map(.url)}' /tmp/r.json"
}
```

**Regra de system prompt:** adicione ao `systemMessage` do Continue:
> Use `/ddg` para qualquer pesquisa web. Nunca invente URLs.

**Cuidados:**
- Continue v1+ espera slash commands em `~/.continue/config.yaml` — adapte.
- Em times, commite a config como `.continue/config.json` no repo.

---

## 8. MiniMax Agent

> Família de LLMs agênticos da MiniMax (M1, Abab) — suporta function calling e agentes com shell.

**Como invoca shell:** Function calling que mapeia para uma ferramenta `shell_exec` implementada no harness.

**Instalação:**
```bash
cargo install duckduckgo-search-cli
```

**Snippet — Definição de tool (passe para a API MiniMax):**
```json
{
  "name": "web_search",
  "description": "Pesquisa web via duckduckgo-search-cli retornando JSON",
  "parameters": {
    "type": "object",
    "properties": { "query": { "type": "string" } },
    "required": ["query"]
  }
}
```
Handler (pseudo-Python, agnóstico de harness):
```python
def web_search(query):
    return subprocess.check_output(
        ["duckduckgo-search-cli", query, "-q", "--num", "15", "-f", "json"],
        timeout=60
    )
```

**Snippet — Multi-query (batch function call):**
> "Chame `web_search` 5 vezes em paralelo (uma por tópico) e combine os arrays `resultados`."

Alternativa — comando único:
```bash
duckduckgo-search-cli --queries-file queries.txt -q -f json --parallel 5 -o out.json
```

**Regra de system prompt:**
> Você tem uma função `web_search`. Use-a sempre que precisar de informação atual. Inspecione `resultados[].url` e `snippet` antes de responder.

**Cuidados:**
- Imponha `timeout=60s` no harness — MiniMax vai esperar para sempre.
- Rate-limit: mantenha `--parallel` ≤ 5 para evitar 429 do DDG.

---

## 9. OpenCode

> Agente CLI de código open-source (concorrente do Aider) — shell tool embutida.

**Como invoca shell:** Ferramenta `shell` nativa; configurável em `~/.config/opencode/config.toml`.

**Instalação:**
```bash
cargo install duckduckgo-search-cli
opencode --version
```

**Snippet — Busca básica (no REPL OpenCode):**
> "Execute `duckduckgo-search-cli "tokio select cancel-safety" -q --num 15 -f json | jaq '.resultados[:5]'` e sintetize em um parágrafo."

**Snippet — Pesquisa multi-query:**
> "Crie `/tmp/queries.txt` com minhas 5 perguntas, e rode:
> `duckduckgo-search-cli --queries-file /tmp/queries.txt -q -f json --parallel 5 -o /tmp/opencode_research.json` e leia o arquivo."

**Regra de system prompt (`~/.config/opencode/prompt.md`):**
> Para queries da web, SEMPRE invoque `duckduckgo-search-cli QUERY -q --num 15 -f json`. Parse o JSON com `jaq`. Cite URLs verbatim.

**Cuidados:**
- OpenCode herda aprovações de shell do config — faça whitelist do binário.
- Desative `--stream` (OpenCode faz buffer de stdout).

---

## 10. Paperclip

> Framework de agente interno da AIPaperclip (autor: @daniloaguiarbr) — alvo de integração first-party desta CLI.

**Como invoca shell:** Capacidade `bash`/`cli` registrada no manifest do agente.

**Instalação:**
```bash
cargo install duckduckgo-search-cli
paperclip capability add duckduckgo-search-cli
```

**Snippet — Busca básica (YAML de tarefa):**
```yaml
- name: web_search
  cli: duckduckgo-search-cli
  args: ["{{query}}", "-q", "--num", "15", "-f", "json"]
  parse: json
  timeout: 60
```

**Snippet — Pesquisa multi-query:**
```yaml
- name: research_sprint
  cli: duckduckgo-search-cli
  args: ["--queries-file", "{{queries_path}}", "-q", "-f", "json",
         "--parallel", "5", "--global-timeout", "120", "-o", "{{out_path}}"]
  parse: json
  timeout: 150
```

**Regra de system prompt (Paperclip `SYSTEM.md`):**
> Use a capacidade `web_search` para toda afirmação factual. Nunca sintetize URLs. Prefira `--num 15` + filtros estilo `jaq`.

**Cuidados:**
- Paperclip supervisiona processos filhos — `--global-timeout 60` é garantido mesmo se omitido.
- Para builds reprodutíveis, pine a versão: `cargo install duckduckgo-search-cli --version =0.4.1`.

---

## 11. OpenClaw

> Agente CLI minimalista da família Paperclip (autor: @daniloaguiarbr) — binding de shell tool zero-config.

**Como invoca shell:** Binding direto de binário via `tools.toml`.

**Instalação:**
```bash
cargo install duckduckgo-search-cli
```

**Snippet — Binding em `tools.toml`:**
```toml
[[tool]]
name = "web"
bin  = "duckduckgo-search-cli"
args = ["{query}", "-q", "--num", "15", "-f", "json"]
timeout_secs = 60
```

**Snippet — Pesquisa multi-query:**
```toml
[[tool]]
name = "research"
bin  = "duckduckgo-search-cli"
args = ["--queries-file", "{path}", "-q", "-f", "json",
        "--parallel", "5", "--global-timeout", "120", "-o", "{out}"]
timeout_secs = 150
```

**Regra de system prompt:**
> Use a ferramenta `web` para queries únicas, e `research` para sprints multi-query. Não invente URLs.

**Cuidados:**
- OpenClaw passa JSON bruto ao LLM — sem pré-parsing; confie que o modelo lê `.resultados`.
- Combine com `jaq` em segunda tool call se o output estourar a janela de contexto.

---

## 12. Google Antigravity

> IDE experimental agent-first do Google — integração profunda com Gemini e execução autônoma de shell.

**Como invoca shell:** Shell tool do agente (análogo ao `run_shell_command` do Gemini CLI).

**Instalação:**
```bash
cargo install duckduckgo-search-cli
```

**Snippet — Busca básica:**
> "Execute: `duckduckgo-search-cli "go generics 1.22 best practices" -q --num 15 -f json | jaq '.resultados[:5]'` e cole os achados em `NOTES.md`."

**Snippet — Pesquisa multi-query:**
> "Monte `queries.txt` com 5 linhas e rode:
> `duckduckgo-search-cli --queries-file queries.txt -q -f json --parallel 5 --global-timeout 120 -o /tmp/antigravity_research.json`
> Resuma os 3 melhores por query numa tabela markdown."

**Regra de system prompt (settings do agente Antigravity):**
> Prefira `duckduckgo-search-cli` para qualquer fato da web. Sempre `--num 15 -f json`. Cite URLs verbatim.

**Cuidados:**
- Antigravity isola chamadas de rede; HTTPS da CLI costuma estar liberado por padrão.
- Use `--proxy` se sua organização exigir proxy corporativo.

---

## 13. GitHub Copilot CLI

> `gh copilot` — companheiro oficial de linha de comando do GitHub Copilot (`gh copilot suggest`, `gh copilot explain`).

**Como invoca shell:** Copilot *sugere* comandos; usuário (ou wrapper script) executa.

**Instalação:**
```bash
gh extension install github/gh-copilot
cargo install duckduckgo-search-cli
```

**Snippet — Busca básica (suggest + run):**
```bash
gh copilot suggest "pesquisar na web 'rust axum middleware tower'" --target shell
# Copilot vai sugerir algo como:
duckduckgo-search-cli "rust axum middleware tower" -q --num 15 -f json | jaq '.resultados[:5]'
```

**Snippet — Wrapper multi-query:**
```bash
# Salve em ~/.local/bin/ddg-research
#!/usr/bin/env bash
set -euo pipefail
printf '%s\n' "$@" > /tmp/q.txt
duckduckgo-search-cli --queries-file /tmp/q.txt -q -f json --parallel 5 --global-timeout 120
```
Depois:
```bash
gh copilot suggest "usar ddg-research para comparar axum vs actix vs rocket"
```

**Regra de system prompt:** no seu `~/.bashrc` / `~/.zshrc`:
```bash
export GH_COPILOT_HINTS="Sempre prefira 'duckduckgo-search-cli QUERY -q --num 15' em vez de curl ad-hoc."
```

**Cuidados:**
- `gh copilot` sugere mas **não** auto-executa — `eval "$(gh copilot suggest ... | tail -1)"` sob sua responsabilidade.
- Requer assinatura GitHub Copilot.

---

## 14. Devin

> Engenheiro de software autônomo da Cognition Labs — sandbox na nuvem com shell completa.

**Como invoca shell:** Terminal nativo na VM Devin; autônomo por padrão.

**Instalação (na sessão Devin):**
```bash
cargo install duckduckgo-search-cli
devin snapshot save "cargo-tools"
```

**Snippet — Busca básica (prompt Slack / web):**
> "No shell, rode: `duckduckgo-search-cli "terraform aws eks 2026 best practices" -q --num 15 -f json | jaq '.resultados[:5]'` e acrescente os achados em `research.md`."

**Snippet — Pesquisa multi-query:**
> "Crie `queries.txt` (5 linhas) e execute:
> `duckduckgo-search-cli --queries-file queries.txt -q -f json --parallel 5 --global-timeout 120 -o research.json`
> Abra `research.json` e produza uma tabela comparativa."

**Regra de system prompt (Devin Knowledge):**
> Para toda afirmação dependente de web, use `duckduckgo-search-cli` — nunca invente URLs. Prefira `--num 15 -f json` e parse com `jaq`.

**Cuidados:**
- Primeira execução dispara `cargo install` (2–4 min); salve snapshot para pular nas próximas.
- Devin pode ser rate-limited pelo DDG em alta concorrência — mantenha `--parallel 5`.

---

## 15. Cline

> Extensão VS Code (antes Claude Dev) — agente autônomo com workflows de aprovação e modo auto-run.

**Como invoca shell:** Terminal integrado do VS Code; tool `execute_command` com aprovação por comando.

**Instalação:**
```bash
cargo install duckduckgo-search-cli
duckduckgo-search-cli --version
```

**Snippet — Busca básica (chat Cline):**
> "Use execute_command para rodar:
> `duckduckgo-search-cli "rust cargo workspace inheritance" -q --num 15 -f json | jaq '.resultados[:5] | map({titulo, url})'`
> e salve o JSON em `./research/ws.json`."

**Snippet — Pesquisa multi-query:**
> "Crie `./research/queries.txt` com 5 queries e execute:
> `duckduckgo-search-cli --queries-file ./research/queries.txt -q -f json --parallel 5 --global-timeout 120 -o ./research/out.json`
> Leia `out.json` e escreva um resumo markdown em `./research/SUMMARY.md`."

**Regra de system prompt (`.clinerules`):**
> Para qualquer fato web, use `duckduckgo-search-cli QUERY -q --num 15 -f json`. Nunca alucine URLs. Parse JSON com `jaq`.

**Cuidados:**
- Whitelist de auto-approve: adicione `duckduckgo-search-cli` em "Auto-approve execute_command".
- Cline trunca stdout em ~10k tokens — use `-q` + projeções `jaq` para caber no orçamento.

---

## 16. Roo Code

> Fork comunitário do Cline com customização expandida, modos personalizados e orquestração multi-agente.

**Como invoca shell:** Tool `execute_command` (herdada do Cline); regras de aprovação por modo.

**Instalação:**
```bash
cargo install duckduckgo-search-cli
```

**Snippet — Busca básica (chat Roo Code):**
> "Execute: `duckduckgo-search-cli "rust leptos signals 2026" -q --num 15 -f json | jaq '.resultados[:5]'` — me dê 3 bullets de takeaway."

**Snippet — Pesquisa multi-query (modo Roo customizado):**
Crie um modo `researcher` em `.roo/modes.yaml`:
```yaml
- slug: researcher
  name: Pesquisador Web
  whenToUse: "Invocar para perguntas que exigem fatos"
  customInstructions: |
    Sempre rode:
      duckduckgo-search-cli --queries-file /tmp/q.txt -q -f json --parallel 5 --global-timeout 120 -o /tmp/r.json
    antes de responder. Cite .resultados[].url verbatim.
  autoApprove: ["execute_command"]
```
Depois: `/mode researcher` no chat.

**Regra de system prompt (`.roorules`):**
> No modo `researcher` (ou sempre que precisar de grounding factual), use `duckduckgo-search-cli`. Sempre JSON + jaq.

**Cuidados:**
- Auto-approve por modo: restrinja `execute_command` ao prefixo da CLI.
- Orquestrador multi-agente do Roo pode disparar fan-out — cap em `--parallel 5` globalmente para respeitar limites DDG.

---

## 📊 Comparative Table / Tabela Comparativa

| # | Agent | Shell tool | Best for | Snippet complexity |
|---|---|---|---|---|
| 1 | Claude Code | Bash tool nativo | Terminal-first, hooks, CI/CD | ⭐ |
| 2 | OpenAI Codex | shell/exec | Codebase refactors, tests | ⭐⭐ |
| 3 | Gemini CLI | run_shell_command | Google Cloud, Gemini power users | ⭐⭐ |
| 4 | Cursor | Terminal + Composer | IDE devs, fast edit/run loops | ⭐ |
| 5 | Windsurf | Cascade run_command | Autonomous refactors | ⭐⭐ |
| 6 | Aider | `/run` | Git-native pair programming | ⭐ |
| 7 | Continue.dev | Custom slash | Multi-editor teams | ⭐⭐⭐ |
| 8 | MiniMax | Function calling | API-first apps | ⭐⭐⭐ |
| 9 | OpenCode | Shell | OSS terminal agents | ⭐⭐ |
| 10 | Paperclip | Agent capability | Paperclip workflows | ⭐⭐ |
| 11 | OpenClaw | tools.toml binding | Minimalist zero-config | ⭐ |
| 12 | Google Antigravity | Agent shell | Experimental / preview users | ⭐⭐ |
| 13 | GitHub Copilot CLI | `gh copilot suggest` | Gh/Git-centric workflows | ⭐⭐ |
| 14 | Devin | Cloud sandbox | Long-running autonomous tasks | ⭐⭐ |
| 15 | Cline | execute_command | VS Code autonomous agents | ⭐⭐ |
| 16 | Roo Code | execute_command + modes | Power users, multi-mode orchestration | ⭐⭐⭐ |

> Legend: ⭐ one-liner / trivial · ⭐⭐ multi-step / small config · ⭐⭐⭐ requires YAML/JSON setup or function-calling glue.

---

## 🔗 See also / Veja também

- Main README: [`../README.md`](../README.md)
- Changelog: [`../CHANGELOG.md`](../CHANGELOG.md)
- Issue tracker: [github.com/daniloaguiarbr/duckduckgo-search-cli/issues](https://github.com/daniloaguiarbr/duckduckgo-search-cli/issues)

> Maintainer: Danilo Aguiar ([@daniloaguiarbr](https://github.com/daniloaguiarbr)) · License: MIT OR Apache-2.0
