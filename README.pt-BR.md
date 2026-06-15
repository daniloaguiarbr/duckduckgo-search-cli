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
- **v0.7.3 — Fingerprint TLS real via BoringSSL (wreq).** BoringSSL é estaticamente vinculado e produz fingerprint JA4_o idêntico ao Chrome/Safari, eliminando o CAPTCHA do Cloudflare que afetava o macOS na v0.7.2. Build requer `cmake`, `perl`, `pkg-config` e `libclang-dev` no Linux. Ver `docs/decisions/0001-tls-boring-via-wreq.md` e `docs/CROSS_PLATFORM.md`.


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
| `duckduckgo-search-cli buscar <QUERY>...` | Subcommand explícito de busca |
| `duckduckgo-search-cli deep-research <QUERY>` | Fan-out de queries, agregação e síntese opcional (v0.7.0) |
| `duckduckgo-search-cli init-config` | Grava `selectors.toml` e `user-agents.toml` no XDG |

## Deep Research (v0.7.0)

Para perguntas de pesquisa multi-hop — "compare os quatro principais clientes HTTP Rust em 2026", "o que mudou no Tokio 1.40", "resuma a história do endpoint HTML do DuckDuckGo" — o `duckduckgo-search-cli` traz um pipeline de fan-out que decompõe a pergunta em 1..=12 sub-queries, dispara em paralelo, agrega e opcionalmente sintetiza um relatório com referências numeradas.

```bash
# Decomposição heurística padrão (5 sub-queries, agregação RRF, sem síntese).
duckduckgo-search-cli deep-research "melhor cliente http rust 2026" -f json -q \
  | jaq '.resultados[] | {titulo, url, score}'

# Relatório em Markdown com orçamento de tokens e extração completa.
duckduckgo-search-cli deep-research "tokio vs async-std produção 2026" \
  --synthesize --budget-tokens 1500 --synth-format markdown \
  --fetch-content --max-content-length 6000 -f json -q

# Sub-queries manuais a partir de arquivo (comentários `#` e linhas vazias ignorados).
cat > /tmp/qs.txt <<EOF
# Visão geral
o que é tokio runtime 2026
# Comparação
tokio vs async-std vs smol
EOF
duckduckgo-search-cli deep-research "tokio runtime 2026" \
  --sub-queries-file /tmp/qs.txt --aggregate dedupe-by-url -f json -q
```

### Flags do Deep Research

- `--max-sub-queries N` máximo de sub-queries geradas (1..=12)
- `--sub-query-strategy` heurística ou manual
- `--sub-queries-file PATH` lista explícita de sub-queries
- `--aggregate` RRF (K=60) ou dedupe por URL canônica
- `--depth` rounds de reflexão planejados mas não executados em v0.7.0
- `--fetch-content` extrai o corpo da página para o top-K
- `--synthesize` produz relatório final em Markdown, PlainText ou JSON
- `--budget-tokens N` limite de tokens do relatório
- `--synth-format` markdown, plain ou json

### Schema JSON de saída

```jsonc
{
  "metadados": {
    "query_original": "melhor cliente http rust 2026",
    "sub_queries": [
      { "texto": "...", "estrategia": "heuristic", "status": "ok", "elapsed_ms": 420 }
    ],
    "total_resultados_unicos": 27,
    "tempo_total_ms": 1850,
    "nivel_cascata": 0
  },
  "resultados": [
    { "titulo": "...", "url": "...", "score": 0.041, "fontes": ["..."] }
  ],
  "sintese": "# Relatório\n\n...\n\n[1] Título — url"
}
```


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
| `--probe` | off | Verificação de saúde pré-voo (1 requisição mínima, relatório JSON) |
| `--identity-profile` | `auto` | Fixa um perfil do pool de 12 identidades (`chrome-win`, `safari-mac`, ...) |
| `--seed N` | (aleatório) | Seed determinístico para seleção de UA e identidade |


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


## Notas de Migração (v0.6.4 → v0.6.5)

- **Zero breaking changes.** Todas as flags CLI, schemas JSON e exit codes de v0.6.4 permanecem inalterados.
- **Build Windows corrigido (MP-26)**: `cargo install duckduckgo-search-cli` agora funciona no Windows. O build da v0.6.4 quebrava no Windows porque `windows-sys 0.59+` mudou `HANDLE` de `isize` para `*mut c_void` e o código fazia casts `handle as isize`. v0.6.5 usa `!handle.is_null() && handle != INVALID_HANDLE_VALUE`.
- **CI matrix verde novamente (CI-01)**: v0.6.4 foi publicada com CI falhando em todos os 3 SOs por 6 erros de clippy latentes. v0.6.5 corrige todos e roda `cargo clippy --all-targets --all-features -- -D warnings` no CI.
- **Sem novas flags CLI ou campos JSON.** Todas as mudanças de v0.6.5 são internas ou melhorias de build/qualidade.
- **Uma nova dependência transitiva**: `indicatif 0.18` (ProgressBar em crawls longos; auto-esconde em pipes).
- **WS-12 circuit breaker**: quando `--fetch-content --parallel` é usado, o novo circuit breaker per-host abre após 3 falhas consecutivas e bloqueia requisições para esse host por 30 segundos antes de permitir uma probe. Isso protege crawls longos de falhas em cascata em um único domínio morto.
- **333 testes passando** (243 unit + 90 integration + 6 doc). 6 erros de clippy corrigidos, 5 novos property tests, 4 novos testes de circuit breaker, 1 novo teste wiremock de Retry-After.


## Notas de Migração (v0.6.x → v0.7.0)

- **Zero breaking changes.** Todas as flags CLI existentes, schemas JSON de `SearchOutput` e `MultiSearchOutput`, e exit codes de v0.6.x permanecem byte-for-byte idênticos em v0.7.0.


## Notas de Migração (v0.7.7 → v0.7.8)

- **Zero mudanças quebrantes.** Todas as flags CLI, schemas JSON de saída e exit codes de v0.7.7 permanecem inalterados.
- **Renovação do detector anti-bot (GAP-WS-50, WS-51, WS-52)**: a função `detectar_interstitial` agora reconhece o novo interstitial DDG anomaly-modal (classes CSS `anomaly-modal__mask` e `anomaly-modal__title`, texto do marker `Unfortunately, bots use DuckDuckGo too.`, URL do challenge `anomaly.js?cc=botnet`). O subcomando `--probe-deep` agora usa uma query de calibração longa (`the quick brown fox jumps over the lazy dog`) em vez de `rust` curto, para realmente acionar o bot scoring upstream. A flag `--allow-lite-fallback` agora consulta o detector antes de cair para lite, então um bloqueio anti-bot real retorna `exit 3` com `cascata_motivo` preenchido em vez de `exit 5` silencioso com zero resultados.
- **Verbose `-vv` e `-vvv` agora suportados (GAP-WS-53)**: `--verbose` trocou de `ArgAction::SetTrue` para `ArgAction::Count". Use `-v` para `info`, `-vv` para `debug`, `-vvv` para `trace`. A env var `RUST_LOG` continua sobrescrevendo. Exemplos:
  - `duckduckgo-search-cli -v "rust async"` — logs nível info
  - `duckduckgo-search-cli -vv "rust async"` — logs nível debug (corpo de request/response)
  - `duckduckgo-search-cli -vvv "rust async" 2>debug.log` — logs nível trace para forense profunda
- **`--retries N` agora é honrado (GAP-WS-57)**: antes o valor estava hard-coded em 1, então `--retries 5` silenciosamente se comportava como `--retries 1`. A flag agora é lida de `Config.retries` com clamp em `[1, 10]` para evitar abuso (`--retries 999` dispara anti-bot). Exemplo: `duckduckgo-search-cli --retries 5 --allow-lite-fallback "rust async runtime"` agora retentativa 5 vezes e cai para lite na detecção de interstitial.
- **`--allow-lite-fallback` agora funciona de verdade (GAP-WS-52)**: pipeline de exemplo que antes retornava zero resultados silenciosos agora retorna resposta de fallback com captcha detectado:
  - `duckduckgo-search-cli --probe-deep --allow-lite-fallback -q -f json` — pre-flight check com opt-in de auto-fallback
  - `duckduckgo-search-cli --allow-lite-fallback --retries 3 "long tail query" 2>cascata.log` — auto-fallback ativado, 3 retentativas por request, log do motivo da cascata em stderr
- **Subcomando `buscar` agora é hidden (GAP-WS-56)**: a forma canônica continua sendo a invocação top-level (`duckduckgo-search-cli "query"`). O subcomando `buscar` continua funcional mas não aparece em `--help`. O help de `buscar --help` deixou de duplicar o help global.
- **Supply chain (GAP-WS-54)**: `scraper` bumped de 0.20 para 0.27, o que remove transitivamente o `fxhash 0.2.1` unmaintained (RUSTSEC-2025-0057). `cargo audit --deny warnings` agora é gate rígido de CI em `ci.yml` e `release.yml`. `async-std` (RUSTSEC-2025-0052) continua apenas na feature opcional `chrome`.
- **Fix de drift de doc (GAP-WS-55)**: o comentário sobre `wreq` no `Cargo.toml` foi reescrito para refletir a decisão real (pin em `wreq 6.0.0-rc.29` mais os três pins diretos para `wreq-util`, `brotli-decompressor`, `alloc-no-stdlib`), não a regressão que nunca aconteceu mencionada no comentário obsoleto.
- **Contagem de testes: 305 (292 lib + 13 integration)**, 0 clippy warnings, 0 fmt diff, 0 cargo-deny warnings, `cargo doc --offline --no-deps` limpo.

## Notas de Migração (v0.7.0 → v0.7.1)

- **Zero breaking changes.** Todas as flags CLI, schemas JSON de saída e exit codes de v0.7.0 permanecem inalterados.
- **Migração de dependência (interna)**: `rand` atualizado de `0.8` para `0.9` para alinhar com `proptest 1.11+` (dev-dep). Todos os call sites internos atualizados:
  - `Rng::gen_range` → `Rng::random_range` (7 sites)
  - `Rng::gen_bool` → `Rng::random_bool` (2 sites)
  - `Rng::gen::<T>()` → `Rng::random::<T>()` (1 site)
  - `rand::thread_rng()` → `rand::rng()` (4 sites)
  - `rand::seq::SliceRandom::choose` → `rand::seq::IndexedRandom::choose` para chamadas `.choose()` em slices; `IteratorRandom::choose` mantido para chamadas `.choose()` em iterators
- **Bump de MSRV**: `rust-version` elevado de `1.75` para `1.85` para satisfazer o MSRV do `rand 0.9` e a onda de deps edition-2024 (`assert_cmd 2.2+`, `blake3 1.8+`, `clap 4.6+`, `proptest 1.11+`, `chrono 0.4.41+`, `idna 1.1+`, `icu_* 2.0+`, `home 0.5.11+`, `async-lock 3.4+`, etc.).
- **Limpeza do builder reqwest**: removidas as chamadas `ClientBuilder::gzip(true)` e `.brotli(true)` (métodos removidos em `reqwest 0.12+`; descompressão agora é automática via header `Accept-Encoding`).
- **Higiene de CI**: dois avisos do `actionlint` shellcheck corrigidos:
  - `.github/workflows/ci.yml:520` — command substitution `$(date ...)` para aspas em `"\$(date ...)"` (SC2046)
  - `.github/workflows/release.yml:505` — adicionado prefixo `--` ao glob `sha256sum -- *` (SC2035)
- **Ignore de advisory de segurança**: `RUSTSEC-2026-0009` (DoS no time 0.3.40 via stack exhaustion em RFC 2822) adicionado à lista ignore do `deny.toml`. A correção em `time 0.3.47` exige `rust-version 1.88+` que não conseguimos satisfazer no MSRV atual. Impacto: a CLI só faz parse de headers `Date` de respostas HTTP sob flags explícitas `--lang`/`--country` do usuário; o cap de tamanho do body da resposta já limita o comprimento da entrada.
- **392 testes passando** (279 lib + 12 doc + 101 integration). 0 avisos clippy, 0 avisos doc, 0 diferenças de fmt, 4 gates do cargo-deny verdes, `cargo publish --dry-run` limpo.
- **Novo subcomando público `deep-research`** para pesquisa multi-hop por LLM. Operadores que não invocam `deep-research` não veem mudança observável.
- **Quatro novos módulos públicos** expostos em `lib.rs` — `deep_research`, `decomposition`, `aggregation`, `synthesis` — composíveis a partir de crates downstream.
- **Novas dependências diretas** no `Cargo.toml`: `url = "2"`, `regex = "1"`, e `proptest = "1"` (somente dev). Todas as três são adições puras; nenhuma dependência foi atualizada ou removida.
- **Sem migração de schema JSON obrigatória**: os schemas `SearchOutput` e `MultiSearchOutput` permanecem inalterados.


## Notas de Migração (v0.6.3 → v0.6.4)

- **Zero breaking changes.** Todas as flags CLI, schemas JSON e exit codes de v0.6.3 permanecem inalterados.
- **Novas flags CLI (aditivas)**:
  - `--probe` — envia uma requisição mínima de pré-voo e reporta saúde em JSON
  - `--identity-profile` — fixa a sessão a uma identidade específica do pool de 12 identidades (`auto` por padrão para rotação adaptativa)
  - `--seed` — agora também controla rotação do pool de identidades (era só UA em v0.6.3)
- **Novos campos JSON de metadados (aditivos, `skip_serializing_if = "Option::is_none"`)**:
  - `metadados.identidade_usada` — tag de identidade (`<família>-<plataforma>-<16hex>`) usada para a resposta
  - `metadados.nivel_cascata` — nível de cascata (0..=4) atingido durante a requisição


## Destaques v0.6.5 (Windows HANDLE fix + CI verde + circuit breaker)

v0.6.5 é uma release de qualidade focada em portabilidade Windows e higiene de CI. A maior melhoria prática é que **`cargo install duckduckgo-search-cli` agora funciona no Windows** pela primeira vez desde v0.6.4. Os 6 erros de clippy latentes que quebraram o CI em todos os 3 SOs em v0.6.4 também são corrigidos.

- **MP-26 (CRÍTICO)**: `src/platform.rs:51-69` reescrito para lidar com a mudança de ABI em `windows-sys 0.59+` (`HANDLE = *mut c_void`). Usa `INVALID_HANDLE_VALUE` de `windows_sys::Win32::Foundation` para a sentinela Win32 e `is_null()` para a verificação de nulidade.
- **CI-01**: 6 erros de clippy corrigidos — `doc_markdown` em 3 strings (`PowerShell`, `rules_rust.md`, `TempDir`), `needless_return`, `missing_debug_implementations` em `ChromeBrowser` e `CircuitBreakerMap`. `cargo clippy --all-targets --all-features -- -D warnings` passa.
- **WS-12 circuit breaker**: breaker per-host em `src/content_fetch.rs` (3 falhas → 30s de cooldown). Protege crawls `--fetch-content --parallel` contra falhas em cascata em domínios mortos.
- **WS-11 property tests**: 5 invariantes em `src/extraction.rs` (inputs vazios, positions densos, URLs absolutos, idempotência, sem panic em HTML malformado). Zero novas dependências.
- **WS-23 wiremock Retry-After**: teste de integração valida que o backoff de 429 respeita o header `Retry-After: 2`.
- **WS-25 indicatif ProgressBar**: `--fetch-content` mostra barra de progresso no stderr. Auto-esconde em pipes (sem contaminação do stdout JSON).
- **Lints FFI preventivos**: `improper_ctypes` e `improper_ctypes_definitions` agora são `deny` no `Cargo.toml`, bloqueando drift futuro de tipo FFI.
- **Adições ao CI**: smoke test `--version --help` em todos os 3 SOs; job `cargo build --no-default-features` para validar o build mínimo.


## Destaques v0.6.4 (WS-26 anti-bot)

v0.6.4 introduz um pool adaptativo de identidades anti-bot que endereça a causa raiz dos bloqueios HTTP 202/403/429 do DuckDuckGo. A versão anterior selecionava um único User-Agent no início e o reutilizava para toda a sessão, produzindo uma única fingerprint que sistemas anti-bot podiam classificar após a primeira requisição. O novo pool:

- Mantém 12 identidades (4 famílias de browser × 3 plataformas: Windows, macOS, Linux)
- Em bloqueio detectado (HTTP 202/403/429), rotaciona através de cascata de 5 níveis: mesma identidade → mesma família/plataforma diferente → família diferente/mesma plataforma → família+plataforma diferentes → aleatória
- Produz ordem de headers determinística via seed em `IdentityProfile::shuffled_headers()` (variantes de Accept-Language, variações de Sec-CH-UA-Arch, ordem aleatorizada)
- Reporta `identidade_usada` e `nivel_cascata` no NDJSON para visibilidade diagnóstica

Uso:

```bash
# Padrão — rotação adaptativa entre 12 identidades
duckduckgo-search-cli -q -n 10 -f json "query"

# Fixa uma identidade específica para testes reproduzíveis
duckduckgo-search-cli -q -n 10 -f json --identity-profile chrome-linux "query"

# Verificação de saúde pré-voo antes de lançar query real
duckduckgo-search-cli --probe

# Seed determinístico para debugar rotação anti-bot
duckduckgo-search-cli -q -n 10 -f json --seed 42 "query"
```


## Contribuindo
- Abra uma issue antes de criar um Pull Request para discutir a mudança proposta
- Leia os guias em `docs/` para entender a arquitetura antes de contribuir


## Licença
- Licenciado sob MIT OR Apache-2.0
- Escolha a licença que melhor atende às suas necessidades


## Notas de migração (v0.7.4 → v0.7.5)

- **Nenhuma mudança de runtime.** v0.7.5 é uma release de experiência de build e documentação: mesmas flags, mesmo schema JSON, mesmas dependências.
- **GAP-WS-29/30/31/32/33/34/35/36/37 fechados neste repositório.** O preflight do `build.rs` da v0.7.4 foi estendido para detectar também **CMake** (o crate `cmake` 0.1.58 precisa de `cmake.exe` no PATH ANTES de `enable_language(ASM_NASM)` ser avaliado), **compilador e linker MSVC** (`cl.exe`/`link.exe` — precisam de `Launch-VsDevShell.ps1` para configurar PATH, INCLUDE, LIB) e **Perl** (`perl.exe` para o gerador perlasm do BoringSSL). Novo preflight no `build.rs` aborta em segundos com a correção exata para cada uma das quatro ferramentas. Escape hatches: `DDG_SKIP_NASM_CHECK=1`, `DDG_SKIP_CMAKE_CHECK=1`, `DDG_SKIP_MSVC_CHECK=1`, `DDG_SKIP_PERL_CHECK=1`. Causa raiz: o sub-componente C++ CMake tools for Windows do Visual Studio Installer vem desmarcado por padrão — instalar apenas o workload C++ NÃO fornece CMake.
- **Helper estendido `scripts/install-windows.ps1`** — agora também detecta e auto-instala CMake (`winget install -e --id Kitware.CMake` ou choco) e Perl (`winget install -e --id StrawberryPerl.StrawberryPerl`), e reporta a instrução exata de instalação MSVC/`Launch-VsDevShell.ps1` (MSVC é grande demais para auto-instalar). Novo modo `--check-only` produz relatório tabular adequado para portões de CI e suporte humano.
- **Novo `scripts/check-windows-toolchain.ps1`** — diagnóstico standalone (sem instalações) que verifica todas as 7 ferramentas (cargo, rustc, cmake, nasm, cl.exe, link.exe, perl) e emite saída texto ou JSON. Exit code 0 se todas presentes, 1 caso contrário. Use para tickets de suporte e portões de CI.
- **Novo `docs/INSTALL-WINDOWS.pt-BR.md`** — guia passo-a-passo cobrindo 5 métodos de instalação (Visual Studio Installer + ferramentas standalone; tudo standalone via winget; apenas Chocolatey; script helper; diagnóstico standalone). Inclui troubleshooting para cada um dos 4 GAPs e os escape hatches `DDG_SKIP_*_CHECK`.
- **Documentação corrigida** — o claim falso de que "VS Build Tools com workload C++ fornece CMake" foi substituído em `docs/CROSS_PLATFORM.pt-BR.md`, `skill/duckduckgo-search-cli-pt/SKILL.md`, `llms.pt-BR.txt` e `llms-full.txt`. O workload C++ NÃO inclui o sub-componente C++ CMake tools — ele deve ser marcado manualmente no Visual Studio Installer.

## Notas de migração (v0.7.3 → v0.7.4)

- **Nenhuma mudança de runtime.** v0.7.4 é uma release de experiência de build e documentação: mesmas flags, mesmo schema JSON, mesmas dependências.
- **GAP-WS-28 fechado neste repositório.** `cargo install` no Windows MSVC nativo sem NASM falhava MINUTOS após o início do build com o erro críptico `CMake Error: No CMAKE_ASM_NASM_COMPILER could be found`. Um novo preflight no `build.rs` agora falha em SEGUNDOS com a correção exata (`winget install -e --id NASM.NASM`, ajuste de PATH, ou `scripts/install-windows.ps1`). Causa raiz: o BoringSSL exige assembly criptográfico em formato NASM a menos que `OPENSSL_NO_ASM` esteja definido, e o ramo do `btls-sys` v0.5.6 que o define para Windows é inalcançável em builds nativos (early return quando host == target no build script dele). Defina `DDG_SKIP_NASM_CHECK=1` para pular o preflight (ex.: toolchain files customizados).
- **Novo helper `scripts/install-windows.ps1`** — detecta NASM, instala via winget (fallback choco), corrige o PATH da sessão e roda `cargo install duckduckgo-search-cli --locked` repassando argumentos extras.
- **Endurecimento do CI**: os jobs Windows de `ci.yml` e `release.yml` agora verificam/instalam NASM explicitamente em vez de depender da imagem do runner.

## Notas de migração (v0.7.2 → v0.7.3)

- **QUEBRA DE AMBIENTE DE BUILD (apenas builds do código-fonte)**: A stack TLS mudou de `rustls` para BoringSSL via `wreq 6.0.0-rc.29`. Compilar do código-fonte no Linux agora requer `cmake`, `perl`, `pkg-config` e `libclang-dev`; no Windows MSVC requer o assembler NASM (`winget install -e --id NASM.NASM`), o **sub-componente C++ CMake tools for Windows** (selecionado manualmente no Visual Studio Installer — NÃO incluído por default no workload C++; ver `docs/INSTALL-WINDOWS.md` para passo a passo), o Strawberry Perl (`winget install -e --id StrawberryPerl.StrawberryPerl`) e a toolchain MSVC (cl.exe, link.exe, configurada via `Launch-VsDevShell.ps1`) no Windows MSVC. Atenção: `cargo install` SEMPRE compila do código-fonte — o crates.io não distribui binários pré-compilados — então esses pré-requisitos valem para todo usuário de `cargo install`, não apenas para o CI. Usuários Windows podem rodar `scripts/install-windows.ps1`, que instala NASM, CMake e Perl automaticamente quando ausentes (MSVC não é auto-instalado — operação intrusiva). Sem o sub-componente C++ CMake tools o build falha com `failed to execute command: program not found / is cmake not installed?`; sem o NASM falha com `No CMAKE_ASM_NASM_COMPILER could be found` (ver `gaps.md` GAP-WS-28/29/30/31/36). A matrix `.github/workflows/release.yml` instala os pacotes Linux automaticamente nos jobs Linux.
- **GAP-WS-27 fechado**: O interstitial de CAPTCHA no macOS está corrigido. Mesma query que retornava `quantidade_resultados: 0` na v0.7.2 retorna 5 resultados na v0.7.3 na mesma máquina. Ver `gaps.md` e `docs/decisions/0001-tls-boring-via-wreq.md`.
- **Novas flags CLI (aditivas)**:
  - `--no-warmup` — pula o warm-up `GET https://duckduckgo.com/` antes da primeira query real
  - `--no-cookie-persistence` — mantém cookies em memória apenas; nunca grava `cookies.json` em disco
  - `--cookies-path <PATH>` — sobrescreve o path XDG padrão do cookie jar
  - `--probe-deep` — executa uma query real e classifica o body como `ok` ou `captcha` baseado em marcadores Cloudflare e DuckDuckGo
  - `--allow-lite-fallback` — opt-in para fallback automático do endpoint `html` para `lite` quando `--probe-deep` (ou retentativas de zero resultados) detectam CAPTCHA
- **Novo estado persistente: cookie jar**: Um arquivo `cookies.json` agora é gravado em `~/.config/duckduckgo-search-cli/cookies.json` (Linux), `%APPDATA%\duckduckgo-search-cli\cookies.json` (Windows), ou `~/Library/Application Support/duckduckgo-search-cli/cookies.json` (macOS). Permissões Unix são `0o600` (owner read+write only). **Trate este arquivo como trataria uma credencial** — ver `SECURITY.pt-BR.md`. Use `--no-cookie-persistence` para desabilitar.
- **Zero mudanças no schema JSON de saída**. Todos os campos da v0.7.2 permanecem presentes.
- **Novas dependências**: `wreq 6.0.0-rc.29`, `wreq-util 3.0.0-rc.12`, mais as transitivas `boring2 4.15.11`, `webpki-root-certs 1.0.7` e a toolchain C do BoringSSL.
- **Dependências removidas**: `reqwest 0.12.28`. `time 0.3.47` não é mais dep direta — puramente transitiva agora.
- **Contagem de testes: 292 lib** (era 279 na v0.7.2). +13 novos testes em `session_warmup` (5), `wreq_cookie_adapter` (3), e `probe_deep` (5). 0 warnings de clippy, 0 diff de fmt, 2 warnings de cargo-deny (RUSTSEC-2025-0057 + RUSTSEC-2025-0052, ambos já na lista de ignore).
- **Tamanho do binário**: +20 MB (BoringSSL é estaticamente vinculado). Tempo de build de release: ~40s mais longo que v0.7.2.


## Troubleshooting adicional (v0.7.3+)

1. **CAPTCHA interstitial detectado (v0.7.3+)** — rode `duckduckgo-search-cli --probe-deep -q -f json` para classificar o body da resposta. Se `status` for `captcha`, a resposta está bloqueada. O probe também reporta `sugestao_mitigacao` com próximos passos concretos (rotacionar proxy, trocar endpoint, back off). Trate o cookie jar como credencial: o arquivo `cookies.json` é gravado com permissões 0o600 e contém cookies de sessão do DuckDuckGo.
2. **Cookie jar crescendo sem controle** — cada invocação adiciona um cookie novo. O arquivo é reescrito inteiro a cada invocação, então o tamanho se mantém proporcional ao número de cookies únicos. Para resetar, apague o arquivo manualmente.
