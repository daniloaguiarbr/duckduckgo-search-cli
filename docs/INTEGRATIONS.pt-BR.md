# Integrações com Agentes de IA

> duckduckgo-search-cli — guia definitivo de integração com 16 agentes de IA e LLMs.
> Encontre seu agente, copie o snippet, ganhe busca web estruturada em menos de 30 segundos.

[![Crates.io](https://img.shields.io/crates/v/duckduckgo-search-cli)](https://crates.io/crates/duckduckgo-search-cli)
[![Docs.rs](https://img.shields.io/docsrs/duckduckgo-search-cli)](https://docs.rs/duckduckgo-search-cli)


## Índice de Agentes
| # | Agente | Mecanismo de shell |
|---|---|---|
| 1 | Claude Code (Anthropic) | Bash tool |
| 2 | OpenAI Codex | Shell / exec tool |
| 3 | Gemini CLI (Google) | Shell tool |
| 4 | Cursor | Terminal + chat |
| 5 | Windsurf (Codeium) | Cascade terminal |
| 6 | Aider | Comando `/run` |
| 7 | Continue.dev | Slash command customizado |
| 8 | MiniMax Agent | Agent tool / API |
| 9 | OpenCode | Shell tool |
| 10 | Paperclip | Agent capability |
| 11 | OpenClaw | CLI tool binding |
| 12 | Google Antigravity | Agent shell |
| 13 | GitHub Copilot CLI | `gh copilot` |
| 14 | Devin (Cognition) | Cloud sandbox |
| 15 | Cline | VS Code terminal |
| 16 | Roo Code | VS Code terminal |


## Contrato Base
- Binário: `duckduckgo-search-cli`
- Instalação: `cargo install duckduckgo-search-cli`
- Padrões: `--num 15` (auto-pagina 2 páginas), `-f auto` (JSON em pipes, texto em TTY)
- Flags principais: `-q` (quiet), `-f json|text|markdown`, `-o FILE`, `--queries-file`, `--fetch-content`, `--time-filter d|w|m|y`, `--proxy`, `--global-timeout 60`, `--parallel 5`
- Exit codes: `0` sucesso · `1` runtime · `2` config · `3` bloqueio · `4` timeout · `5` zero resultados
- Schema JSON (query única):
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
- Segurança SIGPIPE: SIGPIPE restaurado para SIG_DFL no Unix — pipes encerram limpos; BrokenPipe retorna exit 0.
- Segurança de path (v0.5.0): `--output` valida paths ANTES de gravar — rejeita componentes `..` e diretórios de sistema.
- Segurança de credenciais (v0.5.0): credenciais de proxy em `--proxy` NUNCA aparecem em mensagens de erro — mascaramento automático.
- Erros tipados (v0.5.0): enum `ErroCliDdg` com 11 variantes — mapeamento determinístico `exit_code()`.
- Anti-bloqueio (v0.6.0): `PerfilBrowser` injeta `Sec-Fetch-*` por família, Client Hints e `Accept-Language` RFC 7231 — agentes NÃO devem adicionar headers duplicados.
- Schema multi-query: `{quantidade_queries, timestamp, paralelismo, buscas: [<SingleSchema>]}`


## 1. Claude Code
- Claude Code carece de busca web nativa e inventa links quando os dados de treino estão desatualizados.
- `duckduckgo-search-cli` entrega busca web determinística em uma chamada Bash tool com schema JSON estável.
- Validado em PT-BR, parseável com `jaq` sem processar HTML bruto.
- Mecanismo de shell: `Bash` tool nativa; hooks opcionais em `~/.claude/settings.json`.
### Instalação
```bash
cargo install duckduckgo-search-cli --force
duckduckgo-search-cli --version   # esperado 0.4.x
```
### Snippet — Busca básica (cole no chat)
- Cole a instrução abaixo e o Claude Code executa a busca imediatamente.
> "Execute `timeout 30 duckduckgo-search-cli "rust async tokio" -q --num 15 | jaq '.resultados[] | {titulo, url, snippet}'` e resuma os 5 melhores resultados."
### Snippet — Pesquisa multi-query
- Use `--queries-file` para executar até 5 pesquisas paralelas em uma única invocação.
> "Crie `/tmp/queries.txt` com 5 queries (uma por linha) e execute:
> `timeout 120 duckduckgo-search-cli --queries-file /tmp/queries.txt -q -f json --parallel 5 -o /tmp/research.json` e leia o JSON."
### Regra de system prompt (adicione ao `CLAUDE.md`)
- Adicione esta regra para que o Claude Code use a CLI automaticamente em toda tarefa web.
> Quando precisar de contexto web atual, SEMPRE use `duckduckgo-search-cli QUERY -q --num 15` em vez de inventar links. Parse o JSON com `jaq`.
### Cuidados
- Sandbox pode pedir aprovação no primeiro `cargo install`.
- Use `--global-timeout 60` para execuções autônomas.


## 2. OpenAI Codex
- Agentes Codex inventam URLs e perdem precisão em bibliotecas lançadas após o corte de treino.
- `duckduckgo-search-cli` substitui pesquisa por suposição com uma chamada `shell` tool e JSON limpo.
- Funciona no Codex CLI e extensões IDE que suportam execução de ferramenta `shell` / `exec`.
- Mecanismo de shell: ferramenta `shell` / `exec` com confirmação (configurável para `auto` em sandbox confiável).
### Instalação
```bash
cargo install duckduckgo-search-cli
codex config set approval on-failure
```
### Snippet — Busca básica
- Passe esta instrução ao Codex para disparar uma busca web estruturada.
> "Use a shell tool para executar:
> `duckduckgo-search-cli "postgres jsonb index performance" -q --num 15 -f json`
> e extraia títulos e urls com jaq."
### Snippet — Pesquisa multi-query
- Execute 5 pesquisas em lote com `--queries-file` e `--parallel 5`.
> "Escreva as queries em `./research.txt` e rode:
> `duckduckgo-search-cli --queries-file ./research.txt -q -f json --parallel 5 --global-timeout 90 -o ./out.json`
> e mostre os 3 primeiros resultados por query."
### Regra de system prompt
- Adicione ao system prompt do Codex para ancorar o comportamento globalmente.
> Sempre prefira `duckduckgo-search-cli` (instalado globalmente) em vez de inventar URLs. Padrão: `-q --num 15 -f json` + `jaq`.
### Cuidados
- Codex CLI pede aprovação exceto em modo sandbox `workspace-write`.
- Use `--global-timeout 60` para respeitar o orçamento por passo.


## 3. Gemini CLI
- O Gemini CLI precisa de permissão explícita de shell e recorre a respostas fabricadas sem ferramenta web.
- `duckduckgo-search-cli` satisfaz `run_shell_command` com uma chamada de binário e saída JSON estruturada.
- Nenhuma chave de API necessária — a CLI usa o endpoint HTML público do DuckDuckGo.
- Mecanismo de shell: `run_shell_command`, permissão por prefixo de comando.
### Instalação
```bash
cargo install duckduckgo-search-cli
gemini   # REPL; aprove o binário na primeira execução
```
### Snippet — Busca básica
- Cole este prompt no REPL do Gemini CLI para um resultado estruturado imediato.
> "Execute `duckduckgo-search-cli "wasm component model 2025" -q --num 15 | jaq '.resultados[:5]'` e me dê uma lista em bullets."
### Snippet — Pesquisa multi-query
- Agrupe resultados por domínio com `--parallel 5` e pós-processamento `jaq`.
> "Crie `queries.txt` e rode `duckduckgo-search-cli --queries-file queries.txt -q -f json --parallel 5 -o /tmp/gemini_out.json` — leia o arquivo e agrupe domínios duplicados."
### Regra de system prompt (`.gemini/GEMINI.md`)
- Coloque esta regra em `.gemini/GEMINI.md` para ancorar o comportamento web globalmente.
> Para fatos da web, use a shell tool com `duckduckgo-search-cli QUERY -q --num 15 -f json`. Nunca invente URLs.
### Cuidados
- Primeira chamada pede aprovação; "permitir sempre para esse prefixo" agiliza as próximas.
- Respeite a allowlist em `.gemini/settings.json`.


## 4. Cursor
- O agente Composer do Cursor executa comandos autonomamente mas não tem busca web nativa.
- `duckduckgo-search-cli` injeta contexto web ao vivo diretamente no loop editar-executar do Composer.
- Um comando, JSON estruturado, sem navegador — o Cursor permanece no terminal.
- Mecanismo de shell: comandos de terminal embutidos no chat; Composer executa automaticamente em modo agente.
### Instalação
```bash
cargo install duckduckgo-search-cli
duckduckgo-search-cli --version
```
### Snippet — Busca básica (modo agente Composer)
- Cole no Composer e ele executa, parseia e escreve os resultados em arquivo automaticamente.
> "Execute no terminal: `duckduckgo-search-cli "tauri v2 plugin api" -q --num 15 -f json | jaq '.resultados[] | {titulo, url}'` e salve os 5 melhores num arquivo `RESEARCH.md`."
### Snippet — Pesquisa multi-query
- Passe 5 perguntas de uma vez — o Composer cuida da busca paralela e do resumo.
> "Crie `research_queries.txt` com minhas 5 perguntas, e execute:
> `duckduckgo-search-cli --queries-file research_queries.txt -q -f json --parallel 5 -o research.json`
> — resuma os 3 melhores de cada query."
### Regra de system prompt (`.cursorrules`)
- Adicione esta regra ao `.cursorrules` para que o Composer use a CLI antes de qualquer fabricação.
> Prefira rodar `duckduckgo-search-cli QUERY -q --num 15` antes de pesquisar mentalmente. Sempre pipe para `jaq` e cite URLs literalmente.
### Cuidados
- Em modo `auto-run`, o Cursor executa sem perguntar — exija `--global-timeout 60`.
- Mantenha `-q` para não poluir o buffer do agente.


## 5. Windsurf
- O Cascade do Windsurf executa comandos de terminal autonomamente mas não tem busca web embutida.
- `duckduckgo-search-cli` alimenta o Cascade com contexto web estruturado em uma chamada `run_command`.
- Fazer whitelist do binário no auto-approve do Cascade torna cada sprint de pesquisa instantâneo.
- Mecanismo de shell: `run_command` do Cascade (aprovação do usuário ou auto-approve).
### Instalação
```bash
cargo install duckduckgo-search-cli
which duckduckgo-search-cli
```
### Snippet — Busca básica
- Instrua o Cascade a executar e salvar resultados estruturados para uso posterior.
> "Use o terminal para rodar: `duckduckgo-search-cli "axum tower middleware" -q --num 15 -f json`. Parse com `jaq '.resultados[:5] | map({titulo, url})'` e salve em `ctx/search.json`."
### Snippet — Pesquisa multi-query
- Execute 5 pesquisas paralelas e identifique os domínios mais citados em uma única rodada do Cascade.
> "Escreva 5 queries em `queries.txt`, depois: `duckduckgo-search-cli --queries-file queries.txt -q -f json --parallel 5 --global-timeout 90 -o ctx/research.json`. Leia `ctx/research.json` e identifique os 3 domínios mais citados."
### Regra de system prompt (instruções do Cascade)
- Adicione às instruções do sistema do Cascade para prevenir fabricação de URL globalmente.
> Quando o usuário pedir informação atual / web, rode `duckduckgo-search-cli QUERY -q --num 15 -f json` via terminal. Nunca alucine URLs.
### Cuidados
- Auto-approve do Cascade pode ser restrito por comando; faça whitelist do binário.
- Desative `--stream` no Cascade — ele espera JSON em batch.


## 6. Aider
- O comando `/run` do Aider captura stdout no contexto do chat — o caminho mais direto para dados web.
- `duckduckgo-search-cli` injeta JSON estruturado no contexto do Aider com um one-liner.
- Nenhuma configuração necessária — instale o binário e comece a usar `/run` imediatamente.
- Mecanismo de shell: slash command `/run <cmd>` (captura stdout para o chat).
### Instalação
```bash
pipx install aider-chat
cargo install duckduckgo-search-cli
aider
```
### Snippet — Busca básica (dentro do REPL aider)
- Execute no REPL do Aider para injetar resultados web no contexto do chat atual.
```
/run duckduckgo-search-cli "sqlx postgres migrations" -q --num 15 -f json | jaq '.resultados[:5] | map({titulo, url, snippet})'
```
### Snippet — Pesquisa multi-query
- Encadeie criação de arquivo de queries, busca paralela e filtro `jaq` em uma única chamada `/run`.
```
/run echo "rust async tokio\nsqlx postgres\naxum middleware" > /tmp/q.txt && duckduckgo-search-cli --queries-file /tmp/q.txt -q -f json --parallel 3 -o /tmp/r.json && jaq '.buscas[] | {query, top: .resultados[:3] | map(.url)}' /tmp/r.json
```
### Regra de system prompt (`.aider.conf.yml`)
- Configure o Aider para ler um arquivo de regras e forçar busca via CLI.
```yaml
read: ["AIDER.md"]
```
- Adicione isso ao `AIDER.md` para disparar o comportamento em toda requisição relevante.
> Antes de sugerir código com libs externas, rode `/run duckduckgo-search-cli "<lib> <pergunta>" -q --num 10 -f json`.
### Cuidados
- Output de `/run` entra no chat — prefira `-q` e JSON para economizar tokens.
- Aider trunca outputs longos; use `--num 10` e `jaq` para filtrar antes.


## 7. Continue.dev
- Slash commands do Continue.dev canalizam saída de shell para o chat — perfeito para busca estruturada.
- `duckduckgo-search-cli` vira um slash command `/ddg` com 8 linhas de configuração JSON.
- Funciona no VS Code e JetBrains sem plugins ou chaves de API.
- Mecanismo de shell: comandos customizados de tipo `run` (ou ferramentas MCP).
### Instalação
```bash
cargo install duckduckgo-search-cli
```
### Snippet — slash command em `~/.continue/config.json`
- Adicione este bloco à sua configuração Continue para ganhar `/ddg` como comando nativo.
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
### Snippet — Busca básica (chame no chat)
- Dispare uma busca web estruturada com um único slash command.
```
/ddg rust async tokio patterns 2026
```
### Snippet — Slash command multi-query
- Adicione este segundo comando para sprints de pesquisa separados por ponto-e-vírgula.
```json
{
  "name": "research",
  "description": "Pesquisa multi-query DDG",
  "run": "echo \"{{{ input }}}\" | tr ';' '\\n' > /tmp/q.txt && duckduckgo-search-cli --queries-file /tmp/q.txt -q -f json --parallel 5 -o /tmp/r.json && jaq '.buscas[] | {query, urls: .resultados[:3] | map(.url)}' /tmp/r.json"
}
```
### Regra de system prompt
- Adicione ao `systemMessage` do Continue para ancorar todas as buscas web à CLI.
> Use `/ddg` para qualquer pesquisa web. Nunca invente URLs.
### Cuidados
- Continue v1+ espera slash commands em `~/.continue/config.yaml` — adapte.
- Em times, commite a config como `.continue/config.json` no repo.


## 8. MiniMax Agent
- O function calling do MiniMax mapeia diretamente para um handler de shell — sem camada adaptadora extra.
- `duckduckgo-search-cli` vira uma ferramenta `web_search` com um handler Python de 10 linhas.
- O schema JSON estável permite que o MiniMax parse `.resultados` sem engenharia de prompt.
- Mecanismo de shell: function calling que mapeia para uma ferramenta `shell_exec` implementada no harness.
### Instalação
```bash
cargo install duckduckgo-search-cli
```
### Snippet — Definição de tool (passe para a API MiniMax)
- Passe esta definição de tool à API MiniMax para registrar busca web estruturada.
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
- Implemente o handler no seu harness (exemplo Python agnóstico de harness):
```python
def web_search(query):
    return subprocess.check_output(
        ["duckduckgo-search-cli", query, "-q", "--num", "15", "-f", "json"],
        timeout=60
    )
```
### Snippet — Multi-query (batch function call)
- Instrua o MiniMax a chamar `web_search` em paralelo para múltiplos tópicos.
> "Chame `web_search` 5 vezes em paralelo (uma por tópico) e combine os arrays `resultados`."
- Alternativa — comando único do harness:
```bash
duckduckgo-search-cli --queries-file queries.txt -q -f json --parallel 5 -o out.json
```
### Regra de system prompt
- Adicione ao system prompt do MiniMax para forçar pesquisa via CLI.
> Você tem uma função `web_search`. Use-a sempre que precisar de informação atual. Inspecione `resultados[].url` e `snippet` antes de responder.
### Cuidados
- Imponha `timeout=60s` no harness — MiniMax vai esperar para sempre.
- Rate-limit: mantenha `--parallel` <= 5 para evitar 429 do DDG.


## 9. OpenCode
- A shell tool embutida do OpenCode executa binários diretamente — nenhuma configuração necessária.
- `duckduckgo-search-cli` integra com uma entrada de whitelist e entrega JSON na primeira chamada.
- Funciona identicamente ao Aider mas com o modelo de config e aprovação próprio do OpenCode.
- Mecanismo de shell: ferramenta `shell` nativa; configurável em `~/.config/opencode/config.toml`.
### Instalação
```bash
cargo install duckduckgo-search-cli
opencode --version
```
### Snippet — Busca básica (no REPL OpenCode)
- Cole esta instrução no chat do OpenCode para um resultado estruturado imediato.
> "Execute `duckduckgo-search-cli "tokio select cancel-safety" -q --num 15 -f json | jaq '.resultados[:5]'` e sintetize em um parágrafo."
### Snippet — Pesquisa multi-query
- Execute 5 pesquisas paralelas e leia o JSON agregado diretamente.
> "Crie `/tmp/queries.txt` com minhas 5 perguntas, e rode:
> `duckduckgo-search-cli --queries-file /tmp/queries.txt -q -f json --parallel 5 -o /tmp/opencode_research.json` e leia o arquivo."
### Regra de system prompt (`~/.config/opencode/prompt.md`)
- Adicione esta regra ao arquivo de prompt do OpenCode para forçar pesquisa via CLI.
> Para queries da web, SEMPRE invoque `duckduckgo-search-cli QUERY -q --num 15 -f json`. Parse o JSON com `jaq`. Cite URLs verbatim.
### Cuidados
- OpenCode herda aprovações de shell do config — faça whitelist do binário.
- Desative `--stream` (OpenCode faz buffer de stdout).


## 10. Paperclip
- Paperclip supervisiona processos filhos e impõe timeouts — `duckduckgo-search-cli` é um fit natural.
- Alvo de integração first-party: a CLI foi projetada com o schema de tarefas YAML do Paperclip em mente.
- Registre uma vez como capacidade e chame de qualquer tarefa de agente sem código de cola extra.
- Mecanismo de shell: capacidade `bash`/`cli` registrada no manifest do agente.
### Instalação
```bash
cargo install duckduckgo-search-cli
paperclip capability add duckduckgo-search-cli
```
### Snippet — Busca básica (YAML de tarefa)
- Adicione esta definição ao manifest do agente Paperclip para busca de query única.
```yaml
- name: web_search
  cli: duckduckgo-search-cli
  args: ["{{query}}", "-q", "--num", "15", "-f", "json"]
  parse: json
  timeout: 60
```
### Snippet — Pesquisa multi-query
- Adicione esta tarefa para sprints de pesquisa paralela com saída JSON automática.
```yaml
- name: research_sprint
  cli: duckduckgo-search-cli
  args: ["--queries-file", "{{queries_path}}", "-q", "-f", "json",
         "--parallel", "5", "--global-timeout", "120", "-o", "{{out_path}}"]
  parse: json
  timeout: 150
```
### Regra de system prompt (Paperclip `SYSTEM.md`)
- Adicione ao `SYSTEM.md` do Paperclip para ancorar toda afirmação factual à ferramenta web.
> Use a capacidade `web_search` para toda afirmação factual. Nunca sintetize URLs. Prefira `--num 15` + filtros estilo `jaq`.
### Cuidados
- Paperclip supervisiona processos filhos — `--global-timeout 60` é garantido mesmo se omitido.
- Para builds reprodutíveis, pine a versão: `cargo install duckduckgo-search-cli --version =0.4.1`.


## 11. OpenClaw
- O modelo de binding `tools.toml` do OpenClaw significa zero código de harness — declare o binário, use.
- `duckduckgo-search-cli` faz binding com 5 linhas de TOML e fica disponível como ferramentas `web` e `research`.
- JSON bruto é passado diretamente ao LLM — o schema estável elimina ginástica de prompt.
- Mecanismo de shell: binding direto de binário via `tools.toml`.
### Instalação
```bash
cargo install duckduckgo-search-cli
```
### Snippet — Binding em `tools.toml`
- Adicione ao `tools.toml` para registrar `duckduckgo-search-cli` como a ferramenta `web`.
```toml
[[tool]]
name = "web"
bin  = "duckduckgo-search-cli"
args = ["{query}", "-q", "--num", "15", "-f", "json"]
timeout_secs = 60
```
### Snippet — Pesquisa multi-query
- Adicione uma segunda entrada para habilitar a ferramenta `research` para sprints paralelos.
```toml
[[tool]]
name = "research"
bin  = "duckduckgo-search-cli"
args = ["--queries-file", "{path}", "-q", "-f", "json",
        "--parallel", "5", "--global-timeout", "120", "-o", "{out}"]
timeout_secs = 150
```
### Regra de system prompt
- Adicione ao system prompt do OpenClaw para vincular o uso da ferramenta a queries factuais.
> Use a ferramenta `web` para queries únicas, e `research` para sprints multi-query. Não invente URLs.
### Cuidados
- OpenClaw passa JSON bruto ao LLM — sem pré-parsing; confie que o modelo lê `.resultados`.
- Combine com `jaq` em segunda tool call se o output estourar a janela de contexto.


## 12. Google Antigravity
- O Google Antigravity espelha o mecanismo de shell do Gemini CLI em um ambiente IDE-first.
- `duckduckgo-search-cli` integra com um clique de aprovação e entrega JSON estruturado via HTTPS.
- A CLI respeita configurações de proxy corporativo — nenhuma reconfiguração de rede necessária.
- Mecanismo de shell: shell tool do agente (análogo ao `run_shell_command` do Gemini CLI).
### Instalação
```bash
cargo install duckduckgo-search-cli
# No Antigravity, abra o painel do agente e aprove 'duckduckgo-search-cli' no primeiro uso.
```
### Snippet — Busca básica
- Passe esta instrução ao agente do Antigravity para disparar uma busca estruturada.
> "Execute: `duckduckgo-search-cli "go generics 1.22 best practices" -q --num 15 -f json | jaq '.resultados[:5]'` e cole os achados em `NOTES.md`."
### Snippet — Pesquisa multi-query
- Execute 5 queries paralelas e produza uma tabela markdown de resumo em uma única rodada.
> "Monte `queries.txt` com 5 linhas e rode:
> `duckduckgo-search-cli --queries-file queries.txt -q -f json --parallel 5 --global-timeout 120 -o /tmp/antigravity_research.json`
> Resuma os 3 melhores por query numa tabela markdown."
### Regra de system prompt (settings do agente Antigravity)
- Adicione às configurações do agente Antigravity para prevenir fabricação de URL globalmente.
> Prefira `duckduckgo-search-cli` para qualquer fato da web. Sempre `--num 15 -f json`. Cite URLs verbatim.
### Cuidados
- Antigravity isola chamadas de rede; HTTPS da CLI costuma estar liberado por padrão.
- Use `--proxy` se sua organização exigir proxy corporativo.


## 13. GitHub Copilot CLI
- O Copilot CLI sugere comandos mas não os executa — a CLI conecta sugestão a saída estruturada.
- `duckduckgo-search-cli` vira a ferramenta de busca recomendada do Copilot com uma dica de shell.
- Um script wrapper `ddg-research` habilita pesquisa multi-query em uma única invocação `gh copilot suggest`.
- Mecanismo de shell: Copilot sugere comandos; usuário (ou wrapper script) executa.
### Instalação
```bash
gh extension install github/gh-copilot
cargo install duckduckgo-search-cli
```
### Snippet — Busca básica (suggest + run)
- Peça ao Copilot que sugira um comando de busca e execute o resultado diretamente.
```bash
gh copilot suggest "pesquisar na web 'rust axum middleware tower'" --target shell
# Copilot vai sugerir algo como:
duckduckgo-search-cli "rust axum middleware tower" -q --num 15 -f json | jaq '.resultados[:5]'
```
### Snippet — Wrapper multi-query
- Salve este script como `~/.local/bin/ddg-research` para buscas em lote via sugestões do Copilot.
```bash
# Salve em ~/.local/bin/ddg-research
#!/usr/bin/env bash
set -euo pipefail
printf '%s\n' "$@" > /tmp/q.txt
duckduckgo-search-cli --queries-file /tmp/q.txt -q -f json --parallel 5 --global-timeout 120
```
- Depois peça ao Copilot para usar o wrapper:
```bash
gh copilot suggest "usar ddg-research para comparar axum vs actix vs rocket"
```
### Regra de system prompt
- Adicione ao seu perfil de shell para que o Copilot aprenda sua preferência de busca.
```bash
export GH_COPILOT_HINTS="Sempre prefira 'duckduckgo-search-cli QUERY -q --num 15' em vez de curl ad-hoc."
```
### Cuidados
- `gh copilot` sugere mas não auto-executa — use `eval "$(gh copilot suggest ... | tail -1)"` sob sua responsabilidade.
- Requer assinatura GitHub Copilot.


## 14. Devin
- A VM na nuvem do Devin executa `cargo install` e persiste o binário entre sessões via snapshots.
- `duckduckgo-search-cli` dá ao Devin acesso web estruturado sem custo de setup por tarefa após o primeiro snapshot.
- Devin cria arquivos de query, executa buscas paralelas e produz tabelas comparativas autonomamente.
- Mecanismo de shell: terminal nativo na VM Devin; autônomo por padrão.
### Instalação (na sessão Devin)
```bash
cargo install duckduckgo-search-cli
devin snapshot save "cargo-tools"
```
### Snippet — Busca básica (prompt Slack / web)
- Passe ao Devin via Slack ou interface web para uma tarefa de busca imediata.
> "No shell, rode: `duckduckgo-search-cli "terraform aws eks 2026 best practices" -q --num 15 -f json | jaq '.resultados[:5]'` e acrescente os achados em `research.md`."
### Snippet — Pesquisa multi-query
- Devin cuida da criação do arquivo de queries, busca paralela e saída estruturada autonomamente.
> "Crie `queries.txt` (5 linhas) e execute:
> `duckduckgo-search-cli --queries-file queries.txt -q -f json --parallel 5 --global-timeout 120 -o research.json`
> Abra `research.json` e produza uma tabela comparativa."
### Regra de system prompt (Devin Knowledge)
- Adicione ao Devin Knowledge para ancorar toda afirmação factual à busca via CLI.
> Para toda afirmação dependente de web, use `duckduckgo-search-cli` — nunca invente URLs. Prefira `--num 15 -f json` e parse com `jaq`.
### Cuidados
- Primeira execução dispara `cargo install` (2-4 min); salve snapshot para pular nas próximas.
- Devin pode ser rate-limited pelo DDG em alta concorrência — mantenha `--parallel 5`.


## 15. Cline
- A ferramenta `execute_command` do Cline executa qualquer binário no terminal do VS Code — sem extensões.
- `duckduckgo-search-cli` vira um comando com auto-approve em menos de 30 segundos de setup.
- Cline cria arquivos de query, executa buscas e escreve resumos markdown em uma única rodada autônoma.
- Mecanismo de shell: terminal integrado do VS Code; tool `execute_command` com aprovação por comando.
### Instalação
```bash
cargo install duckduckgo-search-cli
duckduckgo-search-cli --version
```
### Snippet — Busca básica (chat Cline)
- Cole esta instrução e o Cline executa a busca e salva os resultados estruturados automaticamente.
> "Use execute_command para rodar:
> `duckduckgo-search-cli "rust cargo workspace inheritance" -q --num 15 -f json | jaq '.resultados[:5] | map({titulo, url})'`
> e salve o JSON em `./research/ws.json`."
### Snippet — Pesquisa multi-query
- O Cline cria o arquivo de queries, executa busca paralela e escreve o resumo markdown em uma rodada.
> "Crie `./research/queries.txt` com 5 queries e execute:
> `duckduckgo-search-cli --queries-file ./research/queries.txt -q -f json --parallel 5 --global-timeout 120 -o ./research/out.json`
> Leia `out.json` e escreva um resumo markdown em `./research/SUMMARY.md`."
### Regra de system prompt (`.clinerules`)
- Adicione esta regra ao `.clinerules` para que toda tarefa web use a CLI automaticamente.
> Para qualquer fato web, use `duckduckgo-search-cli QUERY -q --num 15 -f json`. Nunca alucine URLs. Parse JSON com `jaq`.
### Cuidados
- Whitelist de auto-approve: adicione `duckduckgo-search-cli` em "Auto-approve execute_command".
- Cline trunca stdout em ~10k tokens — use `-q` + projeções `jaq` para caber no orçamento.


## 16. Roo Code
- Os modos customizados do Roo Code permitem criar um modo `researcher` com busca web auto-aprovada.
- `duckduckgo-search-cli` integra com 12 linhas de YAML e vira a ferramenta padrão nesse modo.
- O orquestrador multi-agente do Roo pode distribuir pesquisa paralela entre subagentes com segurança.
- Mecanismo de shell: tool `execute_command` (herdada do Cline); regras de aprovação por modo.
### Instalação
```bash
cargo install duckduckgo-search-cli
```
### Snippet — Busca básica (chat Roo Code)
- Cole no chat do Roo Code para uma busca estruturada de 5 resultados com takeaway imediato.
> "Execute: `duckduckgo-search-cli "rust leptos signals 2026" -q --num 15 -f json | jaq '.resultados[:5]'` — me dê 3 bullets de takeaway."
### Snippet — Pesquisa multi-query (modo Roo customizado)
- Crie um modo `researcher` em `.roo/modes.yaml` para buscas paralelas com auto-approve.
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
- Ative o modo com `/mode researcher` no chat.
### Regra de system prompt (`.roorules`)
- Adicione ao `.roorules` para forçar busca via CLI em todos os contextos factuais.
> No modo `researcher` (ou sempre que precisar de grounding factual), use `duckduckgo-search-cli`. Sempre JSON + jaq.
### Cuidados
- Auto-approve por modo: restrinja `execute_command` ao prefixo da CLI.
- Orquestrador multi-agente do Roo pode disparar fan-out — cap em `--parallel 5` globalmente para respeitar limites DDG.


## Tabela Comparativa
| # | Agente | Shell tool | Melhor para | Complexidade do snippet |
|---|---|---|---|---|
| 1 | Claude Code | Bash tool nativo | Terminal-first, hooks, CI/CD | one-liner |
| 2 | OpenAI Codex | shell/exec | Refactors de codebase, testes | multi-passo |
| 3 | Gemini CLI | run_shell_command | Google Cloud, usuários Gemini | multi-passo |
| 4 | Cursor | Terminal + Composer | Devs IDE, loops rápidos editar/rodar | one-liner |
| 5 | Windsurf | Cascade run_command | Refactors autônomos | multi-passo |
| 6 | Aider | `/run` | Pair programming nativo git | one-liner |
| 7 | Continue.dev | Slash command customizado | Times multi-editor | JSON config |
| 8 | MiniMax | Function calling | Apps API-first | function handler |
| 9 | OpenCode | Shell | Agentes terminais OSS | multi-passo |
| 10 | Paperclip | Agent capability | Workflows Paperclip | YAML config |
| 11 | OpenClaw | tools.toml binding | Zero-config minimalista | TOML config |
| 12 | Google Antigravity | Agent shell | Usuários experimentais / preview | multi-passo |
| 13 | GitHub Copilot CLI | `gh copilot suggest` | Workflows Gh/Git-centric | wrapper script |
| 14 | Devin | Cloud sandbox | Tarefas autônomas longas | multi-passo |
| 15 | Cline | execute_command | Agentes autônomos VS Code | multi-passo |
| 16 | Roo Code | execute_command + modes | Power users, orquestração multi-modo | YAML mode |

- Legenda: one-liner = comando único / trivial · multi-passo = requer alguns comandos · JSON/YAML/TOML config = requer arquivo de config · function handler = requer função de harness


## Veja também
- README principal: [`../README.md`](../README.md)
- Changelog: [`../CHANGELOG.md`](../CHANGELOG.md)
- Issue tracker: [github.com/daniloaguiarbr/duckduckgo-search-cli/issues](https://github.com/daniloaguiarbr/duckduckgo-search-cli/issues)
- Mantenedor: Danilo Aguiar ([@daniloaguiarbr](https://github.com/daniloaguiarbr)) · Licença: MIT OR Apache-2.0
