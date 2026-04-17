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
- Use `-q` em TODA invocação que canaliza stdout para um parser
- Use `-f json` em TODOS os scripts — nunca dependa do formato padrão
- Verifique o código de saída ANTES de processar stdout — exit diferente de zero significa saída parcial ou vazia
- Defina `--global-timeout` como `timeout_externo - 10` em toda chamada de produção
- Use `--per-host-limit 1` junto com `--parallel` para evitar bloqueios
- Use `--retries 3` para resiliência em workflows de pesquisa de longa duração
- Use `--fetch-content` somente quando o texto do snippet for insuficiente para a tarefa
- Use `${PIPESTATUS[0]}` para detectar falha upstream da CLI em comandos com pipe
- Use a variável de ambiente `HTTPS_PROXY` — NUNCA passe credenciais de proxy em argv
- Use `--queries-file` para trabalho em lote — NUNCA loops de shell sobre consultas
- Use `--output` para grandes conjuntos de resultados com 50 ou mais itens
- NUNCA use `--stream` — é um placeholder e não está implementado
- NUNCA injete headers `Sec-Fetch-*` customizados — v0.6.0 os gerencia automaticamente
- NUNCA aumente `--parallel` acima de 5 ou `--per-host-limit` acima de 2

Upstream: https://github.com/daniloaguiarbr/duckduckgo-search-cli
Contrato de esquema válido para `duckduckgo-search-cli` v0.6.x.
