# Integrando duckduckgo-search-cli com Agentes de IA

Dê ao seu agente contexto web em tempo real sem chaves de API, com códigos de saída determinísticos e um esquema JSON imutável construído para parsing por máquina.


## Por Que Agentes de IA Usam Esta Ferramenta
### Ganhos Mensuráveis
- Elimina 3-5 round trips HTTP por consulta de busca
- Retorna JSON estruturado — sem necessidade de parsing de HTML
- Códigos de saída habilitam fluxo de controle determinístico em qualquer shell
- Sem efeitos colaterais — somente leitura, sem estado, idempotente por chamada
- Funciona em qualquer framework de agente capaz de executar comandos shell
- Economiza ~40 tokens por resultado comparado a pipelines de scraping HTML
- Reduz latência de busca em 60-80% versus abordagens com navegador headless
- Habilita multi-query paralela sem risco de rate limit usando `--per-host-limit`
- Processa 20 consultas em menos de 30 segundos com `--parallel 3`
- Sem chave de API para rotacionar, sem dashboard para monitorar, sem lock-in


## Agentes Compatíveis
### Matriz de Compatibilidade
| Agente | Método de Integração | Observações |
|--------|----------------------|-------------|
| Claude Code | Subprocess via ferramenta `Bash` | Piping nativo de JSON com `jaq` |
| OpenAI Codex | Subprocess via `code_interpreter` | Funciona com `jaq` para parsing JSON |
| GPT-4o | Wrapper `function_calling` | JSON como valor de retorno estruturado |
| Google Gemini | Subprocess via `code execution` | Grounding via flag `--fetch-content` |
| GitHub Copilot | Terminal `@workspace` | Pipe de resultados para janela de contexto |
| Cursor | Execução no terminal | Declare em `.cursorrules` para uso automático |
| Codeium | Integração CLI | Padrão de chamada subprocess padrão |
| Aider | Comando `!shell` via `/run` | Injeta resultados direto na sessão de edição |
| Devin | Subprocess de agente shell | Subprocess direto, lê saída JSON |
| SWE-agent | Ferramenta shell | Contrato padrão stdin/stdout |
| AutoGPT | Ferramenta de comando shell | Parsing JSON suportado nativamente |
| CrewAI | Wrapper de ferramenta customizada | Defina esquema JSON como definição de tool |
| LangChain | Integração `BashTool` | Subprocess padrão com verificação de exit code |
| LlamaIndex | `SubprocessTool` | Saída JSON estruturada pronta para parsing |
| Phind | Execução direta no terminal | Nenhum wrapper necessário |
| Continue.dev | Integração no terminal | Registre como slash-command customizado |


## Contrato stdin/stdout
### Entrada
- Entrada: somente argumentos de linha de comando — stdin não é necessário
- Toda configuração via flags; sem prompts interativos em nenhum caminho
- Sem estado: cada invocação é completamente independente
- Nenhum arquivo é modificado a menos que `-o`/`--output` seja especificado explicitamente
- Nenhuma chamada de rede além do endpoint de busca do DuckDuckGo
### Formato de Saída
- Stdout: payload estruturado (JSON/Markdown/texto) — limpo, sem ruído de tracing
- Stderr: logs de tracing apenas — use `-q` para silenciar completamente em pipelines
- `-f json` é OBRIGATÓRIO em todos os pipelines legíveis por máquina
- `-q` é OBRIGATÓRIO em todas as invocações com pipe
### Tratamento de Erros
- O código de saída é SEMPRE significativo — verifique ANTES de processar stdout
- Exit code diferente de zero significa que stdout pode estar vazio ou parcial
- Use `${PIPESTATUS[0]}` para detectar falha upstream em pipes `cmd | jaq`


## Estrutura do JSON de Saída
### Consulta Única
```json
{
  "query": "rust async runtime",
  "resultados": [
    {
      "posicao": 1,
      "titulo": "Título do Resultado",
      "url": "https://exemplo.com/pagina",
      "snippet": "Breve descrição do resultado de busca...",
      "url_exibicao": "exemplo.com › pagina",
      "titulo_original": "Título do Resultado"
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

- `.resultados[].titulo` — SEMPRE presente quando `resultados` não está vazio
- `.resultados[].url` — SEMPRE presente quando `resultados` não está vazio
- `.resultados[].posicao` — SEMPRE presente quando `resultados` não está vazio
- `.resultados[].snippet` — opcional (`Option<String>`) — SEMPRE use fallback `// ""`
- `.resultados[].url_exibicao` — opcional — SEMPRE use fallback `// .url`
- `.metadados.usou_endpoint_fallback` — `true` indica degradação de reputação do IP
- Campos de conteúdo `.conteudo` e `.tamanho_conteudo` — ausentes sem `--fetch-content`
### Modo Multi-Query
```json
{
  "quantidade_queries": 3,
  "buscas": [
    {
      "query": "primeira consulta",
      "resultados": [...],
      "metadados": {...}
    },
    {
      "query": "segunda consulta",
      "resultados": [...],
      "metadados": {...}
    }
  ]
}
```

- Raiz de multi-query é `{ quantidade_queries, buscas: [...] }` — NUNCA `.resultados` na raiz
- Raiz de consulta única é `{ query, resultados, metadados }` — sem chave `.buscas`
- SEMPRE distinga a estrutura raiz antes de acessar os resultados

```bash
# consulta única
duckduckgo-search-cli -q -f json "uma" | jaq '.resultados | length'
# multi-query
duckduckgo-search-cli -q -f json "uma" "duas" | jaq '.buscas[0].resultados | length'
```


## Protocolo de Códigos de Saída
### Referência de Códigos de Saída
| Código | Significado | Ação Recomendada para o Agente |
|--------|-------------|--------------------------------|
| 0 | Sucesso | Faça parsing do JSON do stdout imediatamente |
| 1 | Erro de execução | Leia stderr; tente novamente uma vez com `-v` |
| 2 | Erro de configuração | Execute `init-config --force`; verifique o caminho de config |
| 3 | Bloqueio anti-bot | Aguarde 300+ segundos; mude para `--endpoint lite`; rotacione proxy |
| 4 | Timeout global | Aumente `--global-timeout`; reduza o valor de `--parallel` |
| 5 | Zero resultados | Amplie a consulta; tente `--lang` ou `--country` diferentes |

```bash
run_ddg() {
  local consulta="$1"
  local arquivo_saida="$2"
  timeout 60 duckduckgo-search-cli -q -f json --num 15 "$consulta" > "$arquivo_saida"
  local ec=$?
  case $ec in
    0) return 0 ;;
    3) echo "BLOQUEADO: aguarde 300s e rotacione o proxy." >&2; return 3 ;;
    4) echo "TIMEOUT: aumente --global-timeout." >&2; return 4 ;;
    5) echo "ZERO_RESULTADOS: reformule a consulta." >&2; return 5 ;;
    *) echo "ERRO($ec): verifique o stderr." >&2; return "$ec" ;;
  esac
}
```


## Idempotência e Efeitos Colaterais
- Cada chamada é somente leitura — sem arquivos, sem estado, sem efeitos de rede além da busca
- A mesma consulta retorna resultados semanticamente equivalentes (o conteúdo web muda com o tempo)
- Seguro para tentar novamente em falhas transitórias: exit 1 e exit 4 são seguros para uma nova tentativa
- NÃO é seguro tentar novamente o exit 3 imediatamente — tentativas repetidas aprofundam a janela de bloqueio
- Exit 3 requer backoff de 300+ segundos antes de qualquer nova tentativa
- Exit 5 não é falha — amplie ou reformule a consulta em vez de tentar novamente


## Limites de Payload e Timeouts
### Limites Recomendados
- `-n 10` para janelas de contexto menores que 8k tokens
- `-n 20` para janelas de contexto entre 16k-32k tokens
- `--max-content-length 3000` por página para pipelines RAG padrão
- `--max-content-length 8000` para casos de extração de contexto profundo
- Reduza `--num` para 5 ao usar `--fetch-content` (latência N× por resultado)
### Padrão de Timeout Global
- `--global-timeout` DEVE sempre ser definido 10 segundos abaixo do valor do `timeout` externo
- O `timeout` externo encerra o processo; `--global-timeout` permite que a CLI encerre de forma limpa
- NUNCA defina ambos os valores iguais — a CLI não encerrará de forma limpa

```bash
# timeout externo 60s, global-timeout 50s — sempre deixe margem de 10s
timeout 60 duckduckgo-search-cli -q -f json --num 15 --global-timeout 50 "$CONSULTA"
```


## Controle de Idioma
### Idioma por Requisição
- `--lang pt-br` para resultados em português brasileiro
- `--lang es` para resultados em espanhol
- `--lang en-us` para resultados em inglês dos Estados Unidos
- Padrão: resultados classificados no idioma detectado da consulta
- O idioma afeta a classificação dos resultados e as fontes regionais — não a tradução
- Combine `--lang` com `--country` para segmentação regional precisa


## Tratamento de Bloqueio Anti-bot
### Detecção e Recuperação
- v0.6.0 envia automaticamente os headers `Sec-Fetch-*` e Client Hints por perfil de navegador
- NUNCA injete headers `Sec-Fetch-*` ou `Accept-Language` customizados manualmente
- Exit code 3 significa que o IP está com soft-block — PARE de tentar novamente imediatamente
- Aguarde pelo menos 300 segundos antes de qualquer nova tentativa após exit 3
- Mude para `--endpoint lite` após a primeira ocorrência de exit 3
- Use a variável de ambiente `HTTPS_PROXY` — NUNCA passe credenciais de proxy em argv

```bash
case $codigo_saida in
  3) sleep 300 && HTTPS_PROXY="$URL_PROXY" nova_busca ;;
  4) aumentar_timeout_global && nova_busca ;;
  5) reformular_consulta && nova_busca ;;
esac
```

### Pool Adaptativo de Identidades v0.6.4 (WS-26)
- 12 identidades (4 famílias de browser × 3 plataformas) rotacionam em cascata de 5 níveis em HTTP 202/403/429
- Inspecione `metadados.identidade_usada` (ex. `chrome-linux-11111111aaaa0001`) para saber qual identidade teve sucesso
- Inspecione `metadados.nivel_cascata` (0..=4) para saber o quão esgotado o pool está
- Use `--probe` para verificações de saúde pré-voo em CI antes de lançar queries reais

```bash
# v0.6.4 anti-bot adaptativo pré-voo
timeout 15 duckduckgo-search-cli --probe && \
  timeout 30 duckduckgo-search-cli -q -n 10 -f json "query" | \
  jaq -r '.resultados[] | "[\(.metadados.identidade_usada // "n/a")] \(.titulo) — \(.url)"'

# Fixa uma identidade específica para testes reproduzíveis
timeout 30 duckduckgo-search-cli -q -n 10 -f json \
  --identity-profile chrome-linux "query"

# Rotação de identidade reproduzível
timeout 30 duckduckgo-search-cli -q -n 10 -f json --seed 42 "query"
```


## Exemplos de Integração
### Claude Code (Anthropic)
```bash
# Padrão canônico do Claude Code — grounding antes de editar
RESULTADOS=$(timeout 60 duckduckgo-search-cli -q -f json --num 15 "$CONSULTA")
SAIDA=$?
if [ $SAIDA -eq 0 ]; then
  echo "$RESULTADOS" | jaq -r '.resultados[] | "## \(.titulo)\n\(.snippet // "")\nURL: \(.url)\n"'
fi
```

### OpenAI Codex / GPT-4o
```bash
# Alimenta JSON estruturado como retorno de ferramenta function_calling
timeout 30 duckduckgo-search-cli -q -n 10 -f json "$CONSULTA" \
  | jaq '[.resultados[] | {titulo, url, snippet: (.snippet // "")}]'
```

### Google Gemini
```bash
# Grounding via conteúdo completo da página para síntese
timeout 120 duckduckgo-search-cli -q -n 5 \
  --fetch-content --max-content-length 3000 -f json "$CONSULTA" \
  | jaq -r '.resultados[] | "# \(.titulo)\n\(.conteudo // .snippet // "")\n"'
```

### GitHub Copilot
```bash
# Pipeline de terminal — saída canalizada diretamente para o contexto do Copilot
timeout 30 duckduckgo-search-cli -q -n 5 -f json "$CONSULTA" \
  | jaq -r '.resultados[] | "\(.posicao). \(.titulo) — \(.url)"'
```

### Cursor
```bash
# Declare em .cursorrules — Cursor invoca automaticamente quando precisar de contexto
timeout 30 duckduckgo-search-cli -q -n 10 -f json "$CONSULTA" \
  | jaq '.resultados[] | {titulo, url, snippet: (.snippet // "")}'
```

### Codeium
```bash
# Subprocess padrão — mesmo contrato JSON de todos os outros agentes
timeout 30 duckduckgo-search-cli -q -n 10 -f json "$CONSULTA" > /tmp/busca.json
[ $? -eq 0 ] && jaq -r '.resultados[].url' /tmp/busca.json
```


## Padrões de Performance
### Multi-Query Paralela
```bash
# queries.txt: uma consulta por linha, sem linhas em branco
printf 'rust async channels\ntokio JoinSet examples\nrayon parallel iterators\n' > /tmp/consultas.txt
timeout 300 duckduckgo-search-cli -q \
  --queries-file /tmp/consultas.txt \
  --parallel 3 --per-host-limit 1 --retries 3 \
  --global-timeout 280 -n 10 -f json -o /tmp/resultados.json

# Extrai todas as URLs únicas de todas as consultas
jaq -r '.buscas[].resultados[].url' /tmp/resultados.json | sort -u
```

- NUNCA use `--parallel` acima de 5 — dispara resposta anti-bot HTTP 202
- NUNCA use `--per-host-limit` acima de 2 — aumenta significativamente a probabilidade de bloqueio
- NUNCA itere sobre consultas em loop de shell — use `--queries-file` para reutilizar pools de conexão
- SEMPRE calcule `--global-timeout` como `(consultas / paralelo) * media_segs * 1.5`
### Extração de Conteúdo para RAG
```bash
# Extração de contexto profundo — reduza --num ao usar --fetch-content
timeout 120 duckduckgo-search-cli -q -n 5 \
  --fetch-content --max-content-length 3000 \
  -f json "$CONSULTA" \
  | jaq -r '.resultados[] | "\(.titulo)\n\(.conteudo // .snippet // "")\n---"'
```

- `--fetch-content` multiplica a latência por N (uma busca por resultado)
- SEMPRE limite com `--max-content-length` para controlar uso de memória e tokens
- Reduza `-n` para 5 ou menos quando `--fetch-content` estiver ativo


## Checklist para Desenvolvedores de Agentes
- Use `-q` em TODA invocação que faz pipe de stdout para um parser
- Use `-f json` em TODO script — nunca confie no formato padrão
- Verifique o exit code ANTES de processar stdout — não-zero significa saída parcial ou vazia
- Defina `--global-timeout` como `outer_timeout - 10` em cada chamada de produção
- Use `--per-host-limit 1` junto com `--parallel` para evitar bloqueios
- Use `--retries 3` para resiliência em workflows de pesquisa longos
- Use `--fetch-content` apenas quando o snippet for insuficiente para a tarefa
- Use `${PIPESTATUS[0]}` para detectar falha upstream em pipes
- Use `HTTPS_PROXY` env var — NUNCA passe credenciais de proxy em argv
- Use `--queries-file` para trabalho em lote — NUNCA loops de shell
- Use `--output` para conjuntos grandes de resultados (50 ou mais)
- NUNCA use `--stream` — é placeholder e não está implementado
- NUNCA injete headers `Sec-Fetch-*` customizados — v0.6.0 os gerencia automaticamente
- NUNCA eleve `--parallel` acima de 5 ou `--per-host-limit` acima de 2
- Use `duckduckgo-search-cli --probe` em CI antes de lançar queries reais (v0.6.5+)
- Trate `.metadados.identidade_usada` como `Option<String>` — use `// "n/a"` como fallback no `jaq` (v0.6.5+)
- Trate `.metadados.nivel_cascata` como `Option<u32>` — use `// 0` como fallback no `jaq` (v0.6.5+)
- Para testes reproduzíveis use `--identity-profile <nome>` em vez de apenas `--seed` (v0.6.5+)

Upstream: https://github.com/daniloaguiarbr/duckduckgo-search-cli
Contrato de esquema válido para `duckduckgo-search-cli` v0.7.7/v0.7.8.


## v0.7.3 — Novas Flags + Comportamento JSON

### Novas Flags CLI
- `--probe-deep` — executa uma query real e reporta `status: "ok"` ou `status: "captcha"`. Use isto em portões de CI para runners macOS para detectar interstitials do Cloudflare Bot Management antes de lançar pipelines custosas.
- `--no-warmup` — pula o warm-up `GET https://duckduckgo.com/` que popula os cookies de sessão.
- `--no-cookie-persistence` — mantém cookies em memória apenas; nunca grava `cookies.json` em disco.
- `--cookies-path <PATH>` — sobrescreve o path XDG padrão do cookie jar.
- `--allow-lite-fallback` — opt-in para fallback automático do endpoint `html` para o endpoint `lite` quando CAPTCHA é detectado.

### Schema JSON de Saída do Probe-Deep

A flag `--probe-deep` emite o seguinte contrato JSON:

```jsonc
{
  "type": "probe_deep",
  "endpoint": "html",
  "status": "ok",                        // "ok" | "captcha"
  "http_status": 200,                    // status HTTP da requisição de probe
  "latency_ms": 235,                     // latência de wall clock do probe
  "cascade_level": 0,                    // 0..=4
  "cascata_motivo": "none",              // "none" | "captcha" | "zero_results_after_retries"
  "sugestao_mitigacao": "no interstitial detected",
  "url": "https://html.duckduckgo.com/html/?q=rust"
}
```

Quando `status` é `"captcha"`, o operador deve seguir `sugestao_mitigacao` para os próximos passos (rotacionar proxy, trocar endpoint, back off).

### Localização do Cookie Jar
- Linux: `~/.config/duckduckgo-search-cli/cookies.json`
- Windows: `%APPDATA%\duckduckgo-search-cli\cookies.json`
- macOS: `~/Library/Application Support/duckduckgo-search-cli/cookies.json`

Permissões Unix são `0o600` (owner read+write only). O arquivo é estado interno e NÃO é exposto no schema JSON de saída.


## v0.7.4 — Preflight NASM no Windows (apenas build-time)

v0.7.4 adiciona um preflight no build.rs que detecta nasm.exe no PATH em builds nativos Windows MSVC. Sem o NASM, o build falha em segundos com a correção exata em vez de minutos adentro de erros crípticos do CMake. O preflight é apenas build-time; sem novas flags CLI, sem novos campos JSON.

- Nova env var: DDG_SKIP_NASM_CHECK=1 para pular o preflight em ambientes de build customizados.
- Novo comportamento: cargo build no Windows entra em panic com mensagem acionável quando NASM está ausente.
- Endurecimento de CI: jobs windows-2022 verificam/instalam NASM explicitamente.
- Zero impacto em runtime — mesmas flags, mesmo schema JSON de saída, mesmas dependências da v0.7.3.

## v0.7.5 — Preflight 4 ferramentas + scripts auxiliares (apenas build-time)

v0.7.5 estende o preflight da v0.7.4 para todas as quatro ferramentas que o build do BoringSSL precisa no Windows MSVC: NASM, CMake 3.20+ (com o sub-componente C++ CMake tools for Windows), MSVC C/C++ toolchain (cl.exe/link.exe) e Strawberry Perl. Cada ferramenta ausente dispara panic em segundos com a correção exata.

- Novas env vars: DDG_SKIP_CMAKE_CHECK=1, DDG_SKIP_MSVC_CHECK=1, DDG_SKIP_PERL_CHECK=1 (mais DDG_SKIP_NASM_CHECK=1 da v0.7.4). Use para pular preflight em ambientes de build customizados.
- Novos scripts auxiliares em scripts/:
  - install-windows.ps1 — auto-instala NASM, CMake, Perl; reporta MSVC com instrução Launch-VsDevShell.ps1.
  - check-windows-toolchain.ps1 — diagnóstico standalone; exit 0 = todas as 7 ferramentas presentes, 1 = gap.
- Novos docs: docs/INSTALL-WINDOWS.pt-BR.md (5 métodos de instalação, troubleshooting para cada GAP, todos os 4 escape hatches).
- Zero impacto em runtime — mesmas flags, mesmo schema JSON de saída, mesmas dependências da v0.7.4. O crates.io NÃO distribui binários pré-compilados para nenhuma plataforma.
- Contagem de testes: 405 testes lib (eram 392 no total v0.7.0; 333 na v0.6.5 histórica).


## v0.7.6 — Correção do lockfile do cargo install (GAP-WS-48, apenas build-time)

v0.7.6 fecha a colisão GAP-WS-48 entre `alloc-no-stdlib 2.0.4` e `3.0.0` no `cargo install` removendo a dep `wreq-util` e a feature `brotli` do `wreq`. Três pins no `Cargo.toml` mantêm a supply chain determinística: `brotli-decompressor = "=5.0.1"`, `alloc-no-stdlib = "=2.0.4"` (adicionado na v0.7.7) e a escolha de `wreq 6.0.0-rc.29`.

- Zero novas flags CLI, zero novos campos JSON, zero mudanças de schema.
- `cargo install duckduckgo-search-cli --locked` é o caminho suportado em sistema novo.
- `cargo tree | rg 'brotli|alloc-no-stdlib|alloc-stdlib|wreq-util'` deve retornar zero matches após install.
- Tempo de build caiu de ~37s para ~24s após remoção do brotli.


## v0.7.7 — Restauração do fingerprint TLS (GAP-WS-49, correção runtime)

v0.7.7 restaura o fingerprint JA4_o que vence o interstitial anti-bot do DDG. A correção re-adiciona `wreq-util 3.0.0-rc.12` com `default-features = false` e `features = ["emulation"]`, mais os três pins diretos documentados na v0.7.6. O gap da v0.7.6 era que `--probe-deep` retornava `status: "ok"` enquanto queries reais voltavam zero resultados.

- Zero novas flags CLI, zero novos campos JSON.
- `cargo install duckduckgo-search-cli --version 0.7.7 --locked` é o caminho recomendado.
- `cargo tree` deve mostrar `wreq-util 3.0.0-rc.12`, `brotli 8.0.3`, `brotli-decompressor 5.0.1`, `alloc-no-stdlib 2.0.4`.
- Smoke test de query real: `duckduckgo-search-cli "rust async runtime" -q -f json` deve retornar `quantidade_resultados >= 5`.


## v0.7.8 — Renovação do detector anti-bot + endurecimento de UX (GAP-WS-50..57)

v0.7.8 fecha 8 gaps funcionais em um único release. O contrato de schema fica inalterado (zero breaking changes), mas várias flags CLI e comportamentos internos foram endurecidos.

### Renovação do detector (GAP-WS-50, GAP-WS-51, GAP-WS-52)
- `detectar_interstitial` em `src/probe_deep.rs` agora reconhece 8 markers novos do Cloudflare (`anomaly-modal`, `anomaly-modal__mask`, `anomaly-modal__title`, `anomaly.js?cc=botnet`, `cf-turnstile`, `cf-spinner`, `Just a moment`, `cf-mitigated`) e 1 marker novo do DDG (`Unfortunately, bots use DuckDuckGo too.`).
- 8 testes unitários novos em `src/probe_deep.rs::tests` validam cada marker com fixtures HTML.
- A query de calibração do probe-deep agora é o pangrama de 9 palavras `the quick brown fox jumps over the lazy dog` (constante `PROBE_CALIBRATION_QUERY` em `src/lib.rs`). A query de 1 palavra `rust` retornava a home page do DDG sem acionar o detector, gerando falso negativo.
- `--allow-lite-fallback` agora consulta o detector antes de cair para `lite`. O fallback só dispara quando o detector classifica um interstitial, não em qualquer página de zero resultados.

### Acúmulo de verbose (GAP-WS-53)
- `-v` agora é `ArgAction::Count` em `src/cli.rs`.
- Mapeamento: `-v` = info, `-vv` = debug, `-vvv` = trace.
- `RUST_LOG` env var continua sobrescrevendo.

### Supply chain (GAP-WS-54, GAP-WS-55)
- `scraper` subiu de `0.20.0` para `0.27.0` para resolver `fxhash 0.2.1` transitivo (RUSTSEC-2025-0057, não mantido).
- `cargo audit --deny warnings` virou gate de CI em `ci.yml` e `release.yml`.
- O bloco de comentário do `wreq` no `Cargo.toml` foi reescrito para documentar o pin intencional em `wreq 6.0.0-rc.29` mais os três pins diretos.

### UX (GAP-WS-56, GAP-WS-57)
- O subcomando `buscar` agora é `#[command(hide = true)]`. Continua invocável mas não aparece no `--help`.
- `--retries N` agora é honrado de ponta a ponta em `src/parallel.rs::execute_with_retry`. O bug pré-v0.7.8 deixava o valor hard-coded em 1, ignorando a flag. O clamp novo é `[1, 10]` para impedir `--retries 999` de acionar anti-bot.
- 1 teste de regressão em `tests/integration_search_retry.rs` valida que `--retries 5` produz `metadados.retentativas == 5` no JSON.

### Impacto
- 305 testes (292 lib + 13 integration) passando; zero advisories de `cargo audit --deny warnings`.
- Zero breaking changes no schema JSON ou nos exit codes.
- 4 markers novos no detector (resiliência a mudanças de template anti-bot).
- 1 flag CLI recém-honrada (`--retries`).
- 1 subcomando escondido (`buscar`).
