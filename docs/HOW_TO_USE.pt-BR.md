# Como Usar o duckduckgo-search-cli

Busca web em tempo real no seu terminal — 15 resultados frescos em menos de 3 segundos.


## Por Que Este Guia
- Siga este guia e execute sua primeira busca web em menos de 60 segundos
- Aprenda os comandos principais, padrões avançados e integrações com pipelines shell
- Entenda cada exit code e saiba exatamente como se recuperar de cada erro


## Pré-requisitos
### Obrigatórios
- Acesso à rede para duckduckgo.com
- Rust 1.88+ ao instalar via `cargo install` (MSRV desde v0.7.2)
- Binários pré-compilados não exigem instalação do Rust
- **v0.7.3+ ao compilar do source no Linux**: `cmake`, `perl`, `pkg-config` e `libclang-dev` (deps de build do BoringSSL via `wreq 6.0.0-rc`)
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


## v0.7.3 — Sessão + Probe-Deep + BoringSSL (correção do GAP-WS-27)

v0.7.3 fecha atomicamente o GAP-WS-27 (CAPTCHA no macOS) substituindo a stack TLS `rustls` por BoringSSL embarcado via `wreq 6.0.0-rc.29`, mais persistência de cookies de sessão e detecção profunda de CAPTCHA.

### Mudança da Stack TLS (wreq + BoringSSL)

A CLI agora usa `wreq 6.0.0-rc.29` em vez de `reqwest 0.12` + `rustls-tls`. O `wreq` traz o BoringSSL embarcado (via `boring2 v4.15.11`) e produz um fingerprint `JA4_o` idêntico ao Chrome/Safari real, fechando a porta de entrada do Cloudflare Bot Management que gerava o CAPTCHA.

- Dependências adicionadas: `wreq = "6.0.0-rc"` com features `tokio-rt, webpki-roots, cookies, gzip, brotli, deflate, zstd, socks, form, query`; `wreq-util = "3.0.0-rc.12"`.
- Dependências removidas: `reqwest`, `rustls`, `cookie_store`, `cookie` (em deps diretas).
- ADR formal: `docs/decisions/0001-tls-boring-via-wreq.md`.

### Pré-requisitos de Build Mudaram (v0.7.3+)

Compilar do source no Linux agora requer `cmake`, `perl`, `pkg-config` e `libclang-dev` (BoringSSL). Binários pré-compilados do crates.io não são afetados.

```bash
# Debian/Ubuntu
sudo apt-get install cmake perl pkg-config libclang-dev
# Fedora/RHEL
sudo dnf install cmake perl pkg-config clang-devel
# Alpine
apk add cmake perl pkgconf clang-dev
```

### Persistência de Cookies de Sessão

A feature `session` persiste cookies do DuckDuckGo em `cookies.json` para que requisições subsequentes reutilizem a sessão, e faz um `GET https://duckduckgo.com/` de warm-up antes da primeira query real para popular os cookies de sessão.

- Localização do cookie jar:
  - macOS: `~/Library/Application Support/duckduckgo-search-cli/cookies.json`
  - Linux: `~/.config/duckduckgo-search-cli/cookies.json`
  - Windows: `%APPDATA%\duckduckgo-search-cli\cookies.json`
- Permissões Unix: `0o600` (owner read+write only).
- O cookie jar contém cookies de sessão do DuckDuckGo. Trate como credencial.

#### Flags de Sessão

```bash
# Desabilitar warm-up (pular GET /warm-up)
duckduckgo-search-cli --no-warmup "query"

# Manter cookies só em memória (não gravar cookies.json)
duckduckgo-search-cli --no-cookie-persistence "query"

# Apontar para um cookie jar em volume criptografado
duckduckgo-search-cli --cookies-path /Volumes/encrypted/cookies.json "query"
```

### Detecção Profunda de CAPTCHA (probe-deep)

`--probe-deep` faz uma query de teste real e classifica o body retornado como `ok` ou `captcha`, expondo o JSON:

```bash
duckduckgo-search-cli --probe-deep -q -f json
# {"status": "ok", "endpoint": "html", "http_status": 202,
#  "latency_ms": 97, "cascata_motivo": "none",
#  "sugestao_mitigacao": "no interstitial detected"}
```

Use `--probe-deep` em CI antes de lançar queries caras, especialmente em runners macOS onde o GAP-WS-27 se manifestava.

#### Fallback Automático html→lite

Por padrão, o probe-deep apenas detecta e reporta. Para acionar fallback automático de `html` para `lite` quando CAPTCHA é detectado, passe `--allow-lite-fallback`:

```bash
duckduckgo-search-cli --probe-deep --allow-lite-fallback -q -f json "query"
```

### Validação Empírica (v0.7.3)

```bash
# Antes (v0.7.2): quantidade_resultados: 0, ms: 1695
# Depois (v0.7.3): quantidade_resultados: 5, ms: 735
duckduckgo-search-cli "rust wreq emulation browser fingerprint 2026" -q -f json --num 5
```


## v0.7.2 — rand 0.10 RngExt + time 0.3.47 RUSTSEC-2026-0009 + MSRV 1.88

v0.7.2 é uma release de manutenção que endereça duas dependências upstream:

- `time = "0.3.47"` pinado como dependência direta para sobrescrever `time 0.3.40` que vinha transitivamente via `cookie_store 0.22.0` e `reqwest 0.12.28`. Resolve `RUSTSEC-2026-0009` (stack exhaustion DoS em time 0.3.40).
- `rand 0.10.1` reorganizou os métodos `random_range`, `random_bool` e `random` do trait `Rng` para o trait extension `RngExt`. Substituído `use rand::Rng;` por `use rand::RngExt;` em `src/identity.rs`, `src/parallel.rs` e `src/search.rs`.
- MSRV subiu de 1.85 para 1.88 (exigido por `time 0.3.47` e `rand 0.10`).


## v0.7.1 — Patch de Manutenção

v0.7.1 é uma release puramente de manutenção sem novas flags CLI e sem novos campos JSON. Sincroniza `Cargo.lock` self-version 0.7.0 → 0.7.1 e conserta warnings de clippy latentes.


## v0.7.0 — Subcomando `deep-research`

v0.7.0 introduz o subcomando `deep-research` para pesquisa multi-hop com fan-out de sub-queries.

```bash
duckduckgo-search-cli -q -f json deep-research "tokio vs async-std 2026" \
  --synthesize --synth-format markdown | jaq -r '.sintese'
```

Campos novos: `.metadados.sub_queries[]`, `.metadados.total_resultados_unicos`, `.metadados.tempo_total_ms`, `.resultados[].score`, `.resultados[].fontes[]`, `.sintese` (opt-in via `--synthesize`).


## v0.6.4 — Pool Adaptativo de Identidades Anti-Bot (WS-26)

### Problema
As heurísticas anti-bot do DuckDuckGo classificam uma única combinação de User-Agent + IP + ordem de headers após a primeira requisição. Reutilizar a mesma identidade em todas as chamadas de paginação e em múltiplas queries produz uma única fingerprint que é bloqueada com HTTP 202 (anomalia), HTTP 403 ou HTTP 429.

### Solução
v0.6.4 introduz um pool de 12 identidades com rotação em cascata de 5 níveis:

| Nível | Estratégia |
|-------|------------|
| 0     | Identidade atual (sem rotação) |
| 1     | Mesma família, plataforma diferente |
| 2     | Família diferente, mesma plataforma |
| 3     | Família e plataforma diferentes + endpoint rebaixado para lite |
| 4     | Identidade aleatória + sleep recomendado de 30-60s antes de retentar |

### Uso

#### Probe antes de lançar uma query real

```bash
duckduckgo-search-cli --probe
```

O probe envia uma requisição mínima e reporta status, latência e presença de Set-Cookie como JSON. Exit 0 significa que o endpoint está acessível da sua combinação IP/UA; exit 1 significa que a requisição falhou.

#### Fixa uma identidade específica (determinístico para testes)

```bash
duckduckgo-search-cli -q -n 10 -f json --identity-profile chrome-linux "query"
```

Perfis válidos: `auto` (padrão), `chrome-win`, `chrome-mac`, `chrome-linux`, `edge-win`, `firefox-linux`, `safari-mac`.

#### Rotação de identidade reproduzível (debug de anti-bot)

```bash
duckduckgo-search-cli -q -n 10 -f json --seed 42 "query"
```

A mesma seed produz a mesma sequência de identidades entre execuções. Use para reproduzir bloqueios anti-bot durante debug.

#### Inspecionar qual identidade produziu uma resposta

```bash
duckduckgo-search-cli -q -n 5 -f json "query" | jaq '.metadados.identidade_usada'
# Output: "chrome-linux-11111111aaaa0001"
```


## v0.6.5 — Instalação no Windows corrigida, CI verde, circuit breaker, ProgressBar

v0.6.5 é uma release de qualidade sem novas flags CLI e sem novos campos JSON.
Ela foca em tornar a ferramenta confiável nos três alvos de plataforma e em
crawls longos.

### Windows agora funciona out of the box (MP-26)

`cargo install duckduckgo-search-cli` no Windows falhava em v0.6.4 porque
o upstream `windows-sys 0.59+` mudou o tipo `HANDLE` de `isize` para
`*mut c_void`. v0.6.5 corrige isto com:

```rust
// src/platform.rs:51-69 — verificação type-safe de HANDLE
let handle = unsafe { GetStdHandle(STD_OUTPUT_HANDLE) };
if !handle.is_null() && handle != INVALID_HANDLE_VALUE {
    if unsafe { GetConsoleMode(handle, &mut mode) } != 0 { ... }
}
```

O cast `handle as isize` (que seria UB) foi removido completamente.

### Circuit breaker protege crawls longos (WS-12)

Quando `--fetch-content --parallel` raspa muitas páginas do mesmo domínio,
3 falhas consecutivas nesse host agora abrem o circuito por 30 segundos.
Todas as requisições para esse host são curto-circuitadas durante o cooldown,
prevenindo falhas em cascata que bloqueariam o crawl inteiro.

Você não precisa fazer nada — o breaker é automático. Mas pode observá-lo
no stderr se `--verbose` estiver ativo.

### ProgressBar no stderr, não no stdout (WS-25)

`--fetch-content` agora mostra uma barra de progresso no stderr. A saída JSON
no stdout permanece limpa para pipes. A barra se esconde em contextos não-TTY
(CI, logs).

### Matrix CI verde em todos os 3 SOs (CI-01)

v0.6.4 foi publicada com CI quebrado em Linux, macOS e Windows. v0.6.5
restaura a matrix verde corrigindo 6 erros de clippy latentes e adicionando
smoke tests por plataforma (`--version --help`) ao pipeline CI.

### Novos lints bloqueiam drift FFI futuro

`improper_ctypes = "deny"` e `improper_ctypes_definitions = "deny"` estão
agora ativos. Eles teriam pego o problema de HANDLE da v0.6.4 em tempo de
compilação se estivessem ativos então.

O campo `identidade_usada` reporta a identidade que produziu a resposta bem-sucedida. O campo `nivel_cascata` reporta o nível de cascata atingido (0-4).
