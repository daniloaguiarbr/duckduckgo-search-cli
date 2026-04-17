# Como Usar o duckduckgo-search-cli

Busca web em tempo real no seu terminal — 15 resultados frescos em menos de 3 segundos.


## Por Que Este Guia
- Siga este guia e execute sua primeira busca web em menos de 60 segundos
- Aprenda os comandos principais, padrões avançados e integrações com pipelines shell
- Entenda cada exit code e saiba exatamente como se recuperar de cada erro


## Pré-requisitos
### Obrigatórios
- Acesso à rede para duckduckgo.com
- Rust 1.75+ ao instalar via `cargo install`
- Binários pré-compilados não exigem instalação do Rust
### Opcionais
- `jaq` (substituto Rust do jq) para processar JSON em pipelines
- Um proxy SOCKS5 para rotação de IP quando houver rate-limiting


## Instalação
### Cargo (Recomendado)
- Execute: `cargo install duckduckgo-search-cli`
- Localização do binário: `~/.cargo/bin/duckduckgo-search-cli`
- Verifique: `duckduckgo-search-cli --version`
### Binários Pré-compilados
- Baixe em [GitHub Releases](https://github.com/daniloaguiarbr/duckduckgo-search-cli/releases)
- Disponível para Linux (glibc + musl), macOS Universal e Windows MSVC
- Nenhuma instalação do Rust necessária — binário estático único


## Primeiro Comando
### Busca Básica
```bash
duckduckgo-search-cli "programação async em rust"
```
- Padrão: 15 resultados, formato detectado automaticamente pelo TTY
- Adicione `-f json` para saída legível por máquina
- Adicione `-q` para suprimir logs de tracing ao usar pipe
### Saída Esperada
```
 1. Título do primeiro resultado
    https://exemplo.com/pagina
    Texto do snippet descrevendo o conteúdo da página...

 2. Título do segundo resultado
    ...
```
- Use `-f json` para obter saída estruturada para scripts e agentes
- Use `-f markdown` para obter uma lista linkável para relatórios


## Comandos Principais
### Busca em Texto
```bash
# Saída legível por humanos (padrão no TTY)
duckduckgo-search-cli -n 5 "query"
```
- Formato padrão no TTY é `text`
- Formato padrão em pipes é `json`
- Use `-n N` para controlar a quantidade de resultados (padrão: 15)
### Saída JSON
```bash
# Saída legível por máquina para scripts e LLMs
duckduckgo-search-cli -q -n 10 -f json "query"
```
- Sempre passe `-q` ao usar pipe para suprimir logs de tracing
- Schema: array `resultados[]` com `titulo`, `url`, `snippet`
- Ordem dos campos congelada entre versões — segura para parsing automatizado
### Relatório Markdown
```bash
# Lista linkável para relatórios e documentos
duckduckgo-search-cli -n 15 -f markdown -o relatorio.md "query"
```
- Formato: `- [Título](URL)\n  > snippet`
- Use `-o` para salvar diretamente em arquivo
### Salvar em Arquivo
```bash
# Escrita atômica — segura para scripts concorrentes
duckduckgo-search-cli -q -n 10 -f json -o resultados.json "query"
```
- Cria diretórios pai automaticamente
- Permissões Unix definidas como `0o644`
- Caminhos com `..` são rejeitados (proteção contra path traversal)


## Padrões Avançados
### Buscar Conteúdo das Páginas
```bash
# Baixa e embute o texto limpo de cada página no JSON
duckduckgo-search-cli -q -n 5 --fetch-content --max-content-length 8000 -f json "query"
```
- Campo `conteudo` aparece em cada objeto de resultado quando ativado
- Use `--max-content-length` para limitar caracteres por página (padrão: 10000)
- Use `--per-host-limit 1` para evitar sobrecarregar um único domínio
### Busca Paralela com Múltiplas Queries
```bash
# Uma query por linha no arquivo queries.txt
duckduckgo-search-cli -q \
  --queries-file queries.txt \
  --parallel 3 \
  --per-host-limit 1 \
  --retries 3 \
  -n 10 -f json \
  -o resultados.json
```
- `--parallel` controla requisições simultâneas (1..=20)
- `--per-host-limit` limita fetches por domínio (1..=10)
- Resultados agrupados por query em `.buscas[]` no modo multi-query
### Busca Filtrada por Tempo
```bash
# Apenas resultados das últimas 24 horas
duckduckgo-search-cli -q -n 10 --time-filter d -f json "query de notícias recentes"
```
- Valores: `d` (dia), `w` (semana), `m` (mês), `y` (ano)
- Combine com `--endpoint lite` para maior frescor em queries de baixo volume
### Roteamento via Proxy
```bash
# Rotear via proxy SOCKS5
duckduckgo-search-cli -q -n 10 --proxy socks5://127.0.0.1:9050 -f json "query"

# Rotear via proxy HTTP corporativo
duckduckgo-search-cli -q -n 10 --proxy http://usuario:senha@proxy.interno:8080 -f json "query"
```
- `--proxy` tem precedência sobre variáveis de ambiente `HTTP_PROXY` e `ALL_PROXY`
- Use `--no-proxy` para desativar todas as fontes de proxy explicitamente
### Controle de Idioma
```bash
# Resultados em português
duckduckgo-search-cli -q -n 10 --lang pt -f json "query"

# Resultados em inglês dos EUA
duckduckgo-search-cli -q -n 10 --lang en --country us -f json "query"
```
- Padrão de idioma: `pt`, padrão de país: `br`
- Usa os códigos de região `kl` do DuckDuckGo


## Integração com Scripts Shell
### Extrair URLs dos Resultados
```bash
duckduckgo-search-cli -q -n 10 -f json "query" \
  | jaq -r '.resultados[].url'
```
- Saída com uma URL por linha, pronta para `xargs` ou fetchers downstream
### Filtrar por Palavras-chave no Snippet
```bash
duckduckgo-search-cli -q -n 20 -f json "query" \
  | jaq -r '.resultados[] | select(.snippet | test("rust")) | .titulo'
```
- `test()` no `jaq` aplica regex contra o texto do snippet
### Contar Resultados
```bash
duckduckgo-search-cli -q -n 10 -f json "query" \
  | jaq '.resultados | length'
```
- Verifique a contagem real retornada versus o `-n` solicitado
### Tratar Exit Codes em Scripts
```bash
duckduckgo-search-cli -q -n 10 -f json "query" > /tmp/saida.json
case $? in
  0) echo "OK" ;;
  3) echo "Bloqueio anti-bot — aguarde 60s ou rotacione proxy" >&2 ;;
  4) echo "Timeout global excedido" >&2 ;;
  5) echo "Zero resultados — tente query mais ampla" >&2 ;;
  *) echo "Erro: exit $?" >&2 ;;
esac
```
- Sempre verifique `$?` antes de consumir o arquivo de saída
- Exit code 3 é temporário — faça retry após uma breve pausa


## Integração com Agentes de IA
### Claude Code
```bash
# Em uma chamada de ferramenta Bash do Claude Code:
RESULTADOS=$(duckduckgo-search-cli -q -n 10 -f json "$QUERY" \
  | jaq -r '.resultados[] | "## \(.titulo)\n\(.snippet)\nURL: \(.url)\n"')
```
- Instale a skill incluída para ativação automática sem engenharia de prompt
- Caminho da skill: `skill/duckduckgo-search-cli-pt/SKILL.md`
### OpenAI Codex / GPT
```bash
# Injeta JSON estruturado como contexto em messages[].content
duckduckgo-search-cli -q -n 10 -f json "$QUERY" | jaq '.resultados'
```
- O schema estável `resultados[]` mapeia limpo para campos de tool call response
- Use `--fetch-content` para embedar bodies completos para grounding mais profundo
### Gemini
```bash
# Texto completo das páginas como dados de grounding
duckduckgo-search-cli -q -n 5 \
  --fetch-content --max-content-length 5000 \
  -f json "$QUERY" \
  | jaq -r '.resultados[].conteudo // empty'
```
- Pipe do conteúdo para o modo JSON do Gemini para síntese de fatos de cauda longa
### Qualquer LLM via Pipe
```bash
duckduckgo-search-cli -q -n 10 -f json "$QUERY" \
  | jaq -r '.resultados[] | "## \(.titulo)\n\(.snippet)\n"'
```
- A saída é Markdown puro — cole diretamente em qualquer janela de contexto
- Veja `docs/INTEGRATIONS.md` para 16 snippets prontos por agente


## Erros Comuns
### Bloqueio Anti-bot HTTP 202 (exit 3)
- O DuckDuckGo retornou uma página de desafio, não resultados reais
- Aguarde 60 segundos antes de tentar novamente
- Rotacione o IP de saída com `--proxy socks5://127.0.0.1:9050`
- Aumente as tentativas: `--retries 5`
- Execute `duckduckgo-search-cli init-config` para atualizar perfis de browser
### Timeout Global (exit 4)
- O pipeline excedeu o `--global-timeout` (padrão: 60 segundos)
- Aumente o valor: `--global-timeout 120`
- Reduza a contagem de resultados: `-n 5`
- Adicione `--endpoint lite` para respostas mais rápidas em conexões lentas
### Zero Resultados (exit 5)
- Geralmente é rate-limiting temporário, não um bloqueio permanente
- Aguarde 60 segundos e repita a mesma query
- Amplie a query removendo termos muito específicos
- Remova `--time-filter` se estiver definido — ele restringe o pool de resultados
- Tente `--endpoint lite` como endpoint de fallback
### Configuração Inválida (exit 2)
- Uma flag está fora da faixa permitida ou o caminho é inválido
- `--timeout 0` é rejeitado — mínimo é 1 segundo
- `--output ../../../etc/passwd` é rejeitado — path traversal bloqueado
- `--global-timeout 0` é rejeitado — mínimo é 1 segundo
- `--parallel 0` é rejeitado — mínimo é 1


## Referência de Códigos de Saída

| Código | Significado | Ação Recomendada |
|--------|------------|-----------------|
| 0 | Sucesso | Processar resultados normalmente |
| 1 | Erro de runtime (rede, parse, I/O) | Verificar stderr para detalhes |
| 2 | Configuração inválida (flag fora da faixa, caminho inválido) | Corrigir o argumento |
| 3 | Bloqueio anti-bot DuckDuckGo (HTTP 202) | Aguardar 60s ou rotacionar proxy |
| 4 | Timeout global excedido | Aumentar `--global-timeout` |
| 5 | Zero resultados em todas as queries | Ampliar query ou remover filtros |


## Próximos Passos
- Veja `docs/COOKBOOK.md` para 15 receitas copy-paste de pesquisa, ETL e monitoramento
- Veja `docs/INTEGRATIONS.md` para 16 guias de integração com agentes de IA
- Veja `docs/AGENTS-GUIDE.md` para o contrato completo stdin/stdout e referência de schema
- Veja `docs/CROSS_PLATFORM.md` para guias de configuração em Linux, macOS, Windows e Docker
- Veja `docs/AGENT_RULES.md` para 30+ regras DEVE/JAMAIS para uso em produção com agentes
