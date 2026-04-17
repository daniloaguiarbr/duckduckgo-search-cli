# duckduckgo-search-cli — Instruções para Agentes


## Regra Zero
- Leia este documento INTEGRALMENTE antes de invocar `duckduckgo-search-cli`.
- TODAS as suas invocações DEVEM estar em TOTAL conformidade com as regras aqui.
- Violações resultam em erros de execução, pipelines bloqueados e perda de resultados.
- A Regra Zero se aplica a cada chamada, cada script, cada pipeline, sem exceção.


## Visão Geral
- `duckduckgo-search-cli` é uma CLI Rust para busca DuckDuckGo via HTTP puro.
- Projetada para consumo por LLMs e agentes de IA em pipelines automatizados.
- Saída estruturada em JSON, Markdown, texto simples ou TSV.
- Códigos de saída são semanticamente definidos para tratamento preciso de erros.
- Versão: 0.6.1 — MSRV: Rust 1.75.


## Instalação
- Instale via Cargo: `cargo install duckduckgo-search-cli`
- Verifique a instalação: `duckduckgo-search-cli --version`
- Atualize para a versão mais recente: `cargo install duckduckgo-search-cli --force`


## Início Rápido
- OBRIGATÓRIO: SEMPRE envolva com `timeout` e passe `-q -f json`:

```bash
timeout 30 duckduckgo-search-cli -q -f json --num 15 "rust async runtime"
```

- Processe a saída com `jaq` — JAMAIS com `jq` ou ferramentas de texto:

```bash
timeout 30 duckduckgo-search-cli -q -f json --num 10 "consulta" | jaq -r '.resultados[].url'
```

- Verifique o exit code ANTES de parsear:

```bash
timeout 60 duckduckgo-search-cli -q -f json --num 15 "consulta" > /tmp/out.json
case $? in
  0) jaq '.resultados[].url' /tmp/out.json ;;
  3) echo "bloqueado, aguardando 300s"; sleep 300 ;;
  4) echo "timeout global, aumente --global-timeout" ;;
  5) echo "zero resultados, reformule a consulta" ;;
  *) echo "erro inesperado" ;;
esac
```


## Referência de Flags
- `-q, --quiet` — silencia logs de tracing; stdout carrega apenas o payload
- `-f, --format <FORMAT>` — formato de saída: `json` (OBRIGATÓRIO em scripts), `markdown`, `text`, `tsv`
- `-n, --num <N>` — número de resultados por página (padrão 15, máximo 30)
- `--pages <N>` — número de páginas a buscar (padrão 2, auto-paginação)
- `--parallel <N>` — requisições concorrentes em multi-query (DEVE ser ≤ 5)
- `--queries-file <FILE>` — arquivo com uma consulta por linha para modo lote
- `--fetch-content` — busca conteúdo completo da página por resultado (multiplicador de latência N×)
- `--max-content-length <N>` — limite de bytes buscados por página (DEVE usar com `--fetch-content`)
- `--output <FILE>` — grava JSON em arquivo com validação de segurança de caminho
- `--endpoint <html|lite>` — endpoint de busca (padrão `html`; use `lite` apenas após exit code 3)
- `--global-timeout <SEGS>` — timeout total em segundos para todas as consultas (DEVE ser < `timeout` externo)
- `--per-host-limit <N>` — máximo de requisições concorrentes por host (padrão 2, NÃO exceder 2)
- `--retries <N>` — número de tentativas com backoff exponencial (padrão 2)
- `--timeout <SEGS>` — timeout por requisição em segundos
- `--proxy <URL>` — PROIBIDO em argv; use a variável de ambiente `HTTPS_PROXY`
- `--no-proxy` — ignora toda configuração de proxy
- `--lang <LANG>` — filtro de idioma (ex.: `en-us`, `pt-br`)
- `--country <CC>` — filtro de país (ex.: `us`, `br`)
- `--time-filter <d|w|m>` — filtro de tempo: dia, semana, mês
- `--stream` — PROIBIDO: flag placeholder, NÃO implementada
- `-v, --verbose` — saída detalhada para diagnóstico


## Códigos de Saída
| Código | Significado | Ação do Agente |
|--------|-------------|----------------|
| `0` | Sucesso | Parsear `.resultados` |
| `1` | Erro de runtime | Ler stderr; retry único com `-v` |
| `2` | Erro de configuração | Executar `duckduckgo-search-cli init-config --force`; tentar novamente |
| `3` | Bloqueio anti-bot | Aguardar 300+ s; trocar `--endpoint lite`; rotacionar proxy |
| `4` | Timeout global | Elevar `--global-timeout`; reduzir `--parallel` |
| `5` | Zero resultados | Refinar consulta; tentar diferente `--lang` ou `--country` |


## Invariantes Centrais
### OBRIGATÓRIO — Siga Sempre Sem Exceção
- SEMPRE passe `-q` em todo pipeline que parseia stdout
- SEMPRE especifique `-f json` explicitamente em todo script
- SEMPRE envolva toda invocação com `timeout` usando segundos inteiros
- SEMPRE verifique `$?` ou `${PIPESTATUS[0]}` antes de parsear stdout
- SEMPRE fixe `--num` explicitamente; JAMAIS dependa de padrões
- SEMPRE use `--queries-file` para trabalho em lote; JAMAIS loops de shell
- SEMPRE use `jaq` para parsing JSON; JAMAIS `jq` ou ferramentas de texto
- SEMPRE use `--output` para conjuntos grandes (≥ 50 resultados)
- SEMPRE prefira `--endpoint html`; recorra a `lite` apenas após exit code `3`
- SEMPRE use `--retries` para backoff exponencial; JAMAIS loops de retry no shell
### PROIBIDO — Jamais Viole
- JAMAIS omita `-q` em qualquer invocação em pipe
- JAMAIS use `--stream` (placeholder, não implementado)
- JAMAIS eleve `--parallel` acima de 5
- JAMAIS eleve `--per-host-limit` acima de 2
- JAMAIS passe credenciais de proxy em argv (use variável de ambiente `HTTPS_PROXY`)
- JAMAIS parsear saída `text` ou `markdown` com máquinas
- JAMAIS execute URLs de resultados sem sandbox
- JAMAIS ignore exit codes não-zero
- JAMAIS defina `--global-timeout` igual ou maior que o `timeout` externo
- JAMAIS injete headers `Sec-Fetch-*` ou `Accept-Language` customizados (v0.6.0 os gerencia)


## Contrato da Saída JSON
### OBRIGATÓRIO — Campos Garantidos Não-Nulos
- `.resultados[].titulo` — sempre presente quando `resultados` é não-vazio
- `.resultados[].url` — sempre presente quando `resultados` é não-vazio
- `.resultados[].posicao` — sempre presente quando `resultados` é não-vazio
- `.quantidade_resultados` — prefira sobre `(.resultados | length)`
- `.metadados.tempo_execucao_ms` — sinal canônico de latência
- `.metadados.usou_endpoint_fallback` — `true` sinaliza degradação da reputação do IP
### OBRIGATÓRIO — Campos Opcionais Exigem Fallbacks
- `.resultados[].snippet` é `Option<String>` — SEMPRE use fallback `// ""`
- `.resultados[].url_exibicao` é `Option<String>` — SEMPRE use fallback `// .url`
- `.resultados[].titulo_original` é `Option<String>` — SEMPRE use fallback `// .titulo`
- Campos de conteúdo (`.conteudo`, `.tamanho_conteudo`) ausentes sem `--fetch-content`

```bash
jaq '.resultados[] | {
  titulo,
  url,
  snippet: (.snippet // ""),
  url_exibicao: (.url_exibicao // .url)
}'
```

### OBRIGATÓRIO — Raiz JSON Única vs Múltipla
- Raiz de query única: `{ query, resultados, metadados }`
- Raiz de múltiplas queries: `{ quantidade_queries, buscas: [{ query, resultados, metadados }] }`
- JAMAIS acesse `.resultados` diretamente em resposta de múltiplas queries

```bash
# query única
duckduckgo-search-cli -q -f json "uma" | jaq '.resultados | length'
# múltiplas queries
duckduckgo-search-cli -q -f json "uma" "duas" | jaq '.buscas[0].resultados | length'
```


## Rate Limiting e Etiqueta
### OBRIGATÓRIO — Fique Abaixo do Limiar Anti-Bot
- DEVE limitar `--parallel` em 5 (padrão); valores acima de 5 acionam HTTP 202 anti-bot
- DEVE manter `--per-host-limit` em 2 (padrão); valores acima de 2 aumentam probabilidade de bloqueio
- DEVE usar `--retries` interno com backoff exponencial; JAMAIS loops de retry no shell
- DEVE calcular `--global-timeout` como `(consultas / parallel) * média_segs * 1.5`
- Exit code `3` exige janela de backoff de 300+ segundos antes do retry


## Modo Lote
### OBRIGATÓRIO — Use queries-file para Todo Trabalho em Lote
- JAMAIS itere sobre consultas em loops de shell; pague o custo de startup do processo uma vez
- SEMPRE use `--queries-file` para reutilizar pools de conexão e rotação de UA
- SEMPRE defina `--global-timeout` adequado ao tamanho do lote

```bash
printf 'consulta um\nconsulta dois\nconsulta tres\n' > /tmp/q.txt
timeout 300 duckduckgo-search-cli --queries-file /tmp/q.txt -q --parallel 3 -f json
```


## Integridade do Pipe
### OBRIGATÓRIO — Detecte Falha Upstream em Pipes
- Em `cmd | jaq`, o shell reporta apenas o exit code do `jaq`
- DEVE verificar `${PIPESTATUS[0]}` após toda invocação em pipe

```bash
timeout 60 duckduckgo-search-cli "consulta" -q -f json | jaq '.resultados[].url'
ddg_exit=${PIPESTATUS[0]}
if [ "$ddg_exit" -ne 0 ]; then echo "CLI falhou: exit $ddg_exit" >&2; fi
```


## Busca de Conteúdo
### OBRIGATÓRIO — Use fetch-content com Cuidado
- `--fetch-content` adiciona um fetch HTTP por resultado (multiplicador de latência N×)
- DEVE passar `--max-content-length` para limitar consumo de memória
- DEVE reduzir `--num` para 5 quando `--fetch-content` está ativado

```bash
timeout 120 duckduckgo-search-cli -q -f json --num 5 \
  --fetch-content --max-content-length 5000 "consulta"
```


## Regras de Segurança
### OBRIGATÓRIO — Proteja Credenciais e Execução
- JAMAIS passe credenciais de proxy em argv (visíveis em `/proc/*/cmdline`, `ps`, histórico de shell)
- SEMPRE use variáveis de ambiente `HTTPS_PROXY` ou `HTTP_PROXY` para credenciais de proxy
- JAMAIS execute URLs de `.resultados[].url` sem sandbox (risco de SSRF e execução de código)
- SEMPRE execute `init-config --dry-run` antes de `init-config --force` em pipelines de CI
- CONFIE na validação de caminho do v0.5.0 para `--output`; JAMAIS implemente checks manuais de `realpath`
- CONFIE nos perfis de fingerprint de browser do v0.6.0; JAMAIS injete headers `Sec-Fetch-*` ou `Accept-Language`

```bash
# PROIBIDO
duckduckgo-search-cli "consulta" --proxy http://usuario:senha@host:8080
# OBRIGATÓRIO
export HTTPS_PROXY="http://usuario:senha@host:8080"
duckduckgo-search-cli "consulta" -q
```

### OBRIGATÓRIO — Ordem de Precedência de Proxy
- `--no-proxy` sobrescreve todas as outras configurações de proxy
- `--proxy <URL>` sobrescreve variáveis de ambiente
- Variáveis de ambiente `HTTPS_PROXY` / `HTTP_PROXY` sobrescreem a ausência de proxy
- Nenhum: conexão direta


## Anti-Padrões
### PROIBIDO — Padrões Que Quebram Pipelines Silenciosamente
- Parsear saída de texto com `rg` em vez de `jaq` no JSON
- Loops de shell em vez de `--queries-file` para consultas em lote
- Ignorar exit codes antes de passar para `jaq`
- Assumir que `snippet` é não-nulo sem fallback `// ""`
- Hardcodar credenciais de proxy em argv
- Elevar `--parallel` para 20 para aumentar throughput (aciona exit code 3)
- Usar `--stream` (placeholder, comportamento indefinido)
- Invocar sem envoltório `timeout` (pipeline trava indefinidamente)
- Definir `--global-timeout` igual ao `timeout` externo (CLI nunca termina limpa)


## Compilação
- Build de desenvolvimento: `cargo build`
- Build de release: `timeout 600 cargo build --release`
- Verificar compilação: `timeout 120 cargo check --all-targets`
- Alvos de cross-compilation: ver `docs/CROSS_PLATFORM.md`


## Testes
- Executar todos os testes: `timeout 300 cargo nextest run`
- Executar testes de documentação separadamente: `cargo test --doc`
- Executar testes de integração E2E: `timeout 300 cargo test --test integracao_pipeline`
- Executar com todas as features: `timeout 300 cargo test --all-features`
- Cobertura mínima: 80% — JAMAIS faça merge abaixo deste limite


## Linting
- Executar Clippy com warnings como erros: `timeout 180 cargo clippy --all-targets --all-features -- -D warnings`
- ZERO warnings são tolerados em código de produção
- Corrija todas as sugestões do Clippy antes de abrir um pull request


## Formatação
- Verificar formatação: `cargo fmt --all --check`
- Aplicar formatação: `cargo fmt --all`
- ZERO diferenças são toleradas em commits
- Execute verificação de formatação no CI antes de qualquer outro gate


## Cobertura
- Executar com relatório texto: `cargo llvm-cov --text`
- Executar com relatório HTML: `cargo llvm-cov --html`
- Meta mínima: 80% de cobertura de linhas
- Recomendado para código novo: 90% de cobertura de linhas
- Gates de cobertura se aplicam a todo pull request sem exceção


## Auditoria
- Verificar vulnerabilidades: `timeout 120 cargo audit`
- Verificar licenças e supply chain: `timeout 120 cargo deny check advisories licenses bans sources`
- ZERO vulnerabilidades são toleradas em releases
- Execute auditoria no CI a cada push para main


## Sequência de Validação Completa
- Execute os 10 comandos abaixo em ordem antes de qualquer release:
- `cargo fmt --all --check` — ZERO diferenças
- `timeout 180 cargo clippy --all-targets --all-features -- -D warnings` — ZERO warnings
- `timeout 120 cargo check --all-targets` — ZERO erros
- `RUSTDOCFLAGS="-D warnings" timeout 120 cargo doc --no-deps --all-features` — ZERO warnings
- `timeout 300 cargo nextest run` — ZERO falhas
- `cargo llvm-cov --text` — mínimo 80% de cobertura
- `timeout 120 cargo audit` — ZERO vulnerabilidades
- `timeout 120 cargo deny check advisories licenses bans sources` — ZERO violações
- `timeout 120 cargo publish --dry-run --allow-dirty` — ZERO erros
- `cargo package --list` — ZERO arquivos sensíveis


## Padrões de Integração com LLMs
### OBRIGATÓRIO — Padrões Canônicos para Agentes de IA
- Use `-q -f json` como o ÚNICO contrato de saída legível por máquina
- Use `jaq` como o ÚNICO parser JSON em pipelines
- Use `timeout` como o ÚNICO mecanismo para limitar o tempo de execução
- Use `${PIPESTATUS[0]}` como a ÚNICA forma de detectar falha upstream do CLI
- Use `--queries-file` como o ÚNICO mecanismo de invocação em lote
- Use variáveis de ambiente como o ÚNICO armazenamento de credenciais

```bash
# Padrão canônico de invocação por agente
timeout 60 duckduckgo-search-cli -q -f json --num 15 "$CONSULTA" > /tmp/ddg_out.json
ddg_exit=$?
if [ "$ddg_exit" -ne 0 ]; then
  echo "DDG falhou com exit $ddg_exit" >&2
  exit "$ddg_exit"
fi
jaq -r '.resultados[] | "\(.posicao): \(.titulo) — \(.url)"' /tmp/ddg_out.json
```

### OBRIGATÓRIO — Padrão de Carregamento de Contexto para LLMs
- Busque conteúdo para contexto profundo com limites estritos:

```bash
timeout 120 duckduckgo-search-cli -q -f json \
  --num 5 --fetch-content --max-content-length 5000 \
  "$CONSULTA" | jaq '.resultados[] | {titulo, url, conteudo: (.conteudo // "")}'
```

### OBRIGATÓRIO — Padrão de Múltiplas Consultas
- Use `--queries-file` com `--parallel 3` para pesquisa em lote por LLMs:

```bash
printf '%s\n' "${CONSULTAS[@]}" > /tmp/consultas.txt
timeout 300 duckduckgo-search-cli \
  --queries-file /tmp/consultas.txt \
  -q -f json --parallel 3 --per-host-limit 1 --retries 3 \
  --global-timeout 280 > /tmp/multi_out.json
jaq -r '.buscas[].resultados[].url' /tmp/multi_out.json | sort -u
```


## Tratamento de Erros
### OBRIGATÓRIO — Template Completo de Handler

```bash
executar_ddg() {
  local consulta="$1"
  local arquivo_saida="$2"
  timeout 60 duckduckgo-search-cli -q -f json --num 15 "$consulta" > "$arquivo_saida"
  local ec=$?
  case $ec in
    0) return 0 ;;
    3) echo "BLOQUEADO: anti-bot. Aguarde 300s e rotacione proxy." >&2; return 3 ;;
    4) echo "TIMEOUT: aumente --global-timeout." >&2; return 4 ;;
    5) echo "ZERO_RESULTADOS: reformule a consulta." >&2; return 5 ;;
    *) echo "ERRO($ec): verifique stderr." >&2; return "$ec" ;;
  esac
}
```

### OBRIGATÓRIO — Template de Integridade do Pipe

```bash
timeout 60 duckduckgo-search-cli "consulta" -q -f json | jaq '.resultados[].url'
ddg_exit=${PIPESTATUS[0]}
[ "$ddg_exit" -eq 0 ] || { echo "CLI falhou: exit $ddg_exit" >&2; exit "$ddg_exit"; }
```


## Arquivos de Configuração
- Localização padrão: `$XDG_CONFIG_HOME/duckduckgo-search-cli/` (padrão `~/.config/duckduckgo-search-cli/`)
- Override de localização: variável de ambiente `DUCKDUCKGO_SEARCH_CLI_HOME`
- `selectors.toml` — seletores CSS para parsing de HTML
- `user-agents.toml` — pool de rotação de User-Agent
- Inicializar configuração: `duckduckgo-search-cli init-config`
- Atualização segura: `duckduckgo-search-cli init-config --dry-run` depois `--force`
- Componentes `..` em caminhos são rejeitados automaticamente no v0.5.0+


## Cartão de Referência Rápida

| Regra | Instrução |
|-------|-----------|
| R01 | DEVE passar `-q` ao canalizar para qualquer parser |
| R02 | DEVE especificar `-f json` explicitamente em scripts |
| R03 | JAMAIS parsear `text` ou `markdown` com máquinas |
| R04 | DEVE fixar `--num` explicitamente |
| R05 | DEVE limitar `--parallel` em 5 |
| R06 | DEVE usar `--output` para conjuntos grandes |
| R07 | JAMAIS invocar sem `timeout` |
| R08 | DEVE usar `--queries-file` para trabalho em lote |
| R09 | JAMAIS usar `--stream` (não implementado) |
| R10 | DEVE preferir `--endpoint html` |
| R11 | DEVE distinguir raiz JSON única vs múltipla |
| R12 | DEVE tratar `titulo` e `url` como garantidos não-nulos |
| R13 | JAMAIS assumir que campos opcionais estão presentes |
| R14 | DEVE usar `${PIPESTATUS[0]}` para detectar falhas em pipes |
| R15 | JAMAIS passar credenciais de proxy em argv |
| R16 | JAMAIS executar URLs de resultados sem sandbox |
| R17 | JAMAIS injetar headers `Sec-Fetch-*` (v0.6.0 os gerencia) |

Upstream: https://github.com/daniloaguiarbr/duckduckgo-search-cli
Contrato de schema válido para `duckduckgo-search-cli` v0.6.x.
Versão em inglês: `docs/AGENTS.md`.
