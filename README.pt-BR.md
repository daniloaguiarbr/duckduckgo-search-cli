# duckduckgo-search-cli

[![crates.io](https://img.shields.io/crates/v/duckduckgo-search-cli)](https://crates.io/crates/duckduckgo-search-cli)
[![docs.rs](https://img.shields.io/docsrs/duckduckgo-search-cli)](https://docs.rs/duckduckgo-search-cli)
[![CI](https://github.com/daniloaguiarbr/duckduckgo-search-cli/actions/workflows/ci.yml/badge.svg)](https://github.com/daniloaguiarbr/duckduckgo-search-cli/actions)
[![License](https://img.shields.io/crates/l/duckduckgo-search-cli)](https://crates.io/crates/duckduckgo-search-cli)

> Busca web na velocidade do terminal — dê ao seu agente de IA contexto sobre-humano.

[Read in English](README.md)


## O que é?
- Binário Rust único que transforma qualquer shell em ferramenta de busca de primeira classe
- Sem API key, sem tracking, sem Chrome no caminho quente
- Schema JSON estável com `resultados[]` e `metadados`, ordem de campos congelada entre releases
- Exit codes determinísticos para agentes ramificarem sem ambiguidade
- Paralelismo nativo via `tokio::JoinSet` com controle de concorrência por host
- Funciona em Linux (glibc, musl/Alpine), macOS Intel + Apple Silicon Universal e Windows MSVC


## Por que usar?
- Sem API key para rotacionar e sem dashboard para monitorar
- Perfis de browser v0.6.0 imitam sessões reais para evitar bloqueios anti-bot
- `--fetch-content` baixa e limpa o body de cada URL direto no JSON para o agente
- Schema estável entre releases: nenhuma quebra de contrato para pipelines existentes


## Instalação
- Instale via Cargo com um único comando:

```bash
cargo install duckduckgo-search-cli
```


## Uso Rápido
- Busca básica com 15 resultados (padrão):

```bash
duckduckgo-search-cli "rust async programming"
```

- Busca com saída JSON e 10 resultados:

```bash
duckduckgo-search-cli -f json -n 10 "tokio tutorial"
```

- Busca para LLMs e agentes com parsing via jaq:

```bash
duckduckgo-search-cli "tokio JoinSet exemplos" --num 15 -q | jaq '.resultados'
```

- Busca com conteúdo de páginas embutido no JSON:

```bash
duckduckgo-search-cli --fetch-content -n 5 "melhores frameworks web rust"
```


## Receitas Práticas
- Extrair apenas URLs para um fetcher downstream:

```bash
duckduckgo-search-cli "site:example.com changelog 2025" --num 15 -f json \
  | jaq -r '.resultados[].url'
```

- Enviar bodies limpos para um summarizer:

```bash
duckduckgo-search-cli "tokio runtime internals" --num 15 \
  --fetch-content --max-content-length 4000 -f json \
  | jaq -r '.resultados[] | "# \(.titulo)\n\(.conteudo)\n"' > corpus.md
```

- Fan-out de múltiplas queries em uma única invocação:

```bash
duckduckgo-search-cli "rust rayon" "rust tokio" "rust crossbeam" \
  --num 15 --parallel 3 -f json
```

- Streaming NDJSON para pipelines reativos:

```bash
duckduckgo-search-cli "wasm runtimes" --num 15 --stream \
  | jaq -r 'select(.url) | .url' \
  | xargs -I{} my-downloader {}
```

- Roteamento via proxy corporativo:

```bash
duckduckgo-search-cli "vendor status page 2026" --num 15 \
  --proxy http://user:pass@proxy.internal:8080 -f json
```


## Configuração
- Grava os arquivos padrão no diretório XDG:

```bash
duckduckgo-search-cli init-config
```

- Dry-run para ver o que seria escrito:

```bash
duckduckgo-search-cli init-config --dry-run
```

- Sobrescrever arquivos existentes explicitamente:

```bash
duckduckgo-search-cli init-config --force
```


## Comandos

| Comando | Propósito |
|---|---|
| `duckduckgo-search-cli <QUERY>...` | Busca padrão (equivalente a `buscar`) |
| `duckduckgo-search-cli buscar <QUERY>...` | Subcomando explícito de busca |
| `duckduckgo-search-cli init-config` | Grava `selectors.toml` e `user-agents.toml` no XDG |


## Flags Disponíveis

| Flag | Padrão | Descrição |
|---|---|---|
| `-n`, `--num` | `15` | Máximo de resultados por query (auto-pagina quando > 10) |
| `-f`, `--format` | `auto` | `json`, `text`, `markdown` ou `auto` (detecta TTY) |
| `-o`, `--output` | stdout | Grava no arquivo (valida path, cria diretórios, Unix 0o644) |
| `-t`, `--timeout` | `15` | Timeout por request em segundos |
| `--global-timeout` | `60` | Timeout global do pipeline (1..=3600 segundos) |
| `-l`, `--lang` | `pt` | Código de idioma `kl` do DuckDuckGo |
| `-c`, `--country` | `br` | Código de país `kl` do DuckDuckGo |
| `-p`, `--parallel` | `5` | Requests concorrentes (1..=20) |
| `--pages` | `1` | Páginas por query (1..=5, auto-elevado por `--num`) |
| `--retries` | `2` | Retries extras em 429/403/timeout (0..=10) |
| `--endpoint` | `html` | `html` ou `lite` |
| `--time-filter` | (nenhum) | `d`, `w`, `m` ou `y` |
| `--safe-search` | `moderate` | `off`, `moderate` ou `on` |
| `--stream` | off | Emite uma linha NDJSON por resultado conforme chegam |
| `--fetch-content` | off | Baixa cada URL e adiciona texto limpo ao JSON |
| `--max-content-length` | `10000` | Limite de caracteres por body (1..=100_000) |
| `--per-host-limit` | `2` | Fetches concorrentes por host (1..=10) |
| `--proxy URL` | (nenhum) | Proxy HTTP/HTTPS/SOCKS5 (prevalece sobre env vars) |
| `--no-proxy` | off | Desativa todas as fontes de proxy |
| `--queries-file PATH` | (nenhum) | Lê queries adicionais de arquivo (uma por linha) |
| `--match-platform-ua` | off | Filtra pool de user-agents para o SO atual |
| `--chrome-path PATH` | (auto) | Caminho manual do Chrome (feature `chrome`) |
| `-v`, `--verbose` | off | Logs DEBUG em stderr |
| `-q`, `--quiet` | off | Apenas logs ERROR em stderr |


## Variáveis de Ambiente

| Variável | Descrição | Exemplo |
|---|---|---|
| `RUST_LOG` | Sobrescreve o filtro do `tracing-subscriber` | `RUST_LOG=duckduckgo=debug` |
| `HTTP_PROXY` | Proxy HTTP padrão (prioridade menor que `--proxy`) | `http://user:pass@proxy:8080` |
| `HTTPS_PROXY` | Proxy HTTPS padrão | `http://proxy:8443` |
| `ALL_PROXY` | Proxy fallback para qualquer scheme | `socks5://127.0.0.1:9050` |
| `CHROME_PATH` | Caminho fallback para Chrome (feature `chrome`) | `/opt/google/chrome/chrome` |


## Formatos de Saída
- `json` (padrão em pipes): schema canônico com `resultados[]` e `metadados`, ordem de campos estável
- `text`: bloco legível `NN. Título\n   URL\n   snippet`
- `markdown`: `- [Título](URL)\n  > snippet`
- `--stream`: NDJSON, cada linha é um resultado, metadados emitidos na linha final


## Exit Codes

| Código | Significado |
|---|---|
| 0 | Sucesso |
| 1 | Erro de runtime (rede, parse, I/O) |
| 2 | Configuração inválida (flag fora de faixa, proxy malformado) |
| 3 | Bloqueio DuckDuckGo (anomalia HTTP 202) |
| 4 | Timeout global excedido |
| 5 | Zero resultados em todas as queries |


## Troubleshooting
### Bloqueio anti-bot (exit 3)
- Aumente `--retries` para dar mais tentativas ao cliente
- Rotacione user-agents via `init-config` editando `user-agents.toml`
- Adicione `--proxy socks5://127.0.0.1:9050` para rotacionar o IP de saída
- Os perfis de browser da v0.6.0 reduzem este problema ao imitar sessões reais

### Rate limit HTTP 429
- Reduza `--per-host-limit` para diminuir concorrência por host
- Ative `--match-platform-ua` para filtrar UAs ao SO atual
- Use `--proxy` para rotacionar o IP de saída

### Timeout (exit 4)
- Aumente `--global-timeout` para pipelines lentos
- Aumente `-t` para requests individuais em redes instáveis
- Verifique conectividade antes de re-executar

### Zero resultados (exit 5)
- Aguarde 60 segundos, pois normalmente é rate-limiting temporário
- Confira `--lang` e `--country` para garantir localização correta
- Tente `--endpoint lite` como fallback alternativo
- Revise `--time-filter` se estiver restringindo o período

### Path rejeitado em --output (exit 2)
- Caminhos com `..` são rejeitados para prevenir travessia de diretório
- Caminhos para diretórios de sistema (`/etc`, `/usr`, `/bin`) são bloqueados
- Use caminhos sob o diretório home, `/tmp` ou diretório de trabalho atual

### Pipe para jaq retorna vazio
- Verifique `echo ${PIPESTATUS[*]}` após o pipe
- Se o primeiro número for diferente de zero, o CLI errou antes de produzir output
- Sempre passe `-q -f json` ao usar pipe para manter stdout limpo


## Skill de Agente
- Este repositório entrega uma Claude Agent Skill pronta para uso imediato
- Instalação em dois comandos:

```bash
git clone https://github.com/daniloaguiarbr/duckduckgo-search-cli
cp -r duckduckgo-search-cli/skill/duckduckgo-search-cli-pt ~/.claude/skills/
cp -r duckduckgo-search-cli/skill/duckduckgo-search-cli-en ~/.claude/skills/
```

- Reinicie o Claude Code ou recarregue o Agent SDK para ativar
- Auto-ativação: o Claude dispara a skill quando o usuário menciona pesquisa ou verificação


## Documentação

| Guia | Por que importa |
|---|---|
| [`docs/AGENT_RULES.md`](docs/AGENT_RULES.md) | 30+ regras DEVE/JAMAIS para qualquer LLM invocar a CLI em produção |
| [`docs/COOKBOOK.md`](docs/COOKBOOK.md) | 15 receitas copy-paste para pesquisa, ETL, monitoramento e extração de conteúdo |
| [`docs/INTEGRATIONS.md`](docs/INTEGRATIONS.md) | Snippets para 16 agentes: Claude Code, Codex, Gemini CLI, Cursor, Windsurf, Aider e mais |


## Notas de Migração
### v0.3.x para v0.4.0
- `--num` agora é `15` por padrão (antes era o payload completo de uma página, ~11)
- Quando `--num > 10` e `--pages` permanece no default `1`, o CLI eleva automaticamente `--pages` para `ceil(num / 10)` limitado a 5
- Schema JSON inalterado: `resultados[]`, `metadados` e `titulo_original` permanecem idênticos à v0.3.x

Veja o [CHANGELOG](CHANGELOG.md) para o histórico completo de versões.


## Contribuindo
- Abra uma issue antes de criar um Pull Request para discutir a mudança proposta
- Leia os guias em `docs/` para entender a arquitetura antes de contribuir


## Licença
- Licenciado sob MIT OR Apache-2.0
- Escolha a licença que melhor atende às suas necessidades
