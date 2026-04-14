![crates.io](https://img.shields.io/crates/v/duckduckgo-search-cli)
![docs.rs](https://img.shields.io/docsrs/duckduckgo-search-cli)
![CI](https://github.com/comandoaguiar/duckduckgo-search-cli/actions/workflows/ci.yml/badge.svg)
![License](https://img.shields.io/crates/l/duckduckgo-search-cli)

> Structured DuckDuckGo search in pure Rust, built for LLM pipelines.

---

## English

### What is it?

- Rust CLI that queries `html.duckduckgo.com/html/` over plain HTTP.
- No paid API, no Chrome requirement in the hot path, no disk cache.
- Emits JSON, text, Markdown, or NDJSON streams suitable for piping into LLMs.
- Cross-platform: Linux (glibc, musl/Alpine, Flatpak, Snap, NixOS), macOS (Intel + Apple Silicon), Windows (cmd.exe + PowerShell).
- Feature-gated Chrome headless fallback (`--features chrome`) for JS-heavy pages.
- Parallel multi-query pipeline with per-host rate limiting and cancellation.

### Why?

- Drop-in primitive for retrieval-augmented generation: stable JSON schema, stable exit codes.
- Respects `.gitignore`-style ownership — zero hidden state, configuration lives in XDG dirs.
- Rustls-only TLS: no OpenSSL runtime, no SChannel surprises, static musl builds succeed out of the box.
- Explicit timeouts (`--timeout`, `--global-timeout`) and bounded concurrency (`--parallel`, `--per-host-limit`) make the tool safe inside automation.

### Quick Start

```bash
cargo install duckduckgo-search-cli
duckduckgo-search-cli init-config
duckduckgo-search-cli "rust async runtime" --num 5 --format json
duckduckgo-search-cli "rust async runtime" --fetch-content --max-content-length 5000 -o /tmp/out.json
```

### Commands

| Command                                  | Purpose                                                |
| ---------------------------------------- | ------------------------------------------------------ |
| `duckduckgo-search-cli <QUERY>...`       | Default search (equivalent to `buscar`).               |
| `duckduckgo-search-cli buscar <QUERY>..` | Explicit search subcommand.                            |
| `duckduckgo-search-cli init-config`      | Write `selectors.toml` and `user-agents.toml` to XDG.  |

### Flags

| Flag                       | Default    | Description                                                        |
| -------------------------- | ---------- | ------------------------------------------------------------------ |
| `-n`, `--num`              | (all)      | Max results per query per page.                                    |
| `-f`, `--format`           | `auto`     | `json`, `text`, `markdown`, or `auto` (TTY-aware).                 |
| `-o`, `--output`           | stdout     | Write to file (parent dirs auto-created, Unix 0o644).              |
| `-t`, `--timeout`          | `15`       | Per-request timeout (seconds).                                     |
| `--global-timeout`         | `60`       | Whole-pipeline timeout (1..=3600 seconds).                         |
| `-l`, `--lang`             | `pt`       | DuckDuckGo `kl` language code.                                     |
| `-c`, `--country`          | `br`       | DuckDuckGo `kl` country code.                                      |
| `-p`, `--parallel`         | `5`        | Concurrent requests (1..=20).                                      |
| `--pages`                  | `1`        | Pages to crawl per query (1..=5).                                  |
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

### Environment Variables

| Variable       | Description                                                 | Example                            |
| -------------- | ----------------------------------------------------------- | ---------------------------------- |
| `RUST_LOG`     | Overrides the `tracing-subscriber` filter.                  | `RUST_LOG=duckduckgo=debug`        |
| `HTTP_PROXY`   | Default HTTP proxy (lower priority than `--proxy`).         | `http://user:pass@proxy:8080`      |
| `HTTPS_PROXY`  | Default HTTPS proxy.                                        | `http://proxy:8443`                |
| `ALL_PROXY`    | Fallback proxy for any scheme.                              | `socks5://127.0.0.1:9050`          |
| `CHROME_PATH`  | Fallback Chrome path (feature `chrome`).                    | `/opt/google/chrome/chrome`        |

### Output Formats

- `json` (default for pipes): canonical schema with `results[]`, `metadados`, `related_searches`, stable field order.
- `text`: human-readable block `NN. Title\n   URL\n   snippet`.
- `markdown`: `- [Title](URL)\n  > snippet`.
- Stream (`--stream`): NDJSON where each line is one result; metadata emitted as the final line.

### Integration Patterns

```bash
# Pipe directly into an LLM preprocessing step.
duckduckgo-search-cli "site:example.com changelog 2025" --num 3 --format json | jaq '.resultados[].url'

# Feed extracted bodies into a summarizer.
duckduckgo-search-cli "tokio runtime internals" --fetch-content --max-content-length 4000 -f json \
  | jaq -r '.resultados[] | "# \(.titulo)\n\(.conteudo)\n"' > corpus.md

# Streaming mode with xargs.
duckduckgo-search-cli "wasm runtimes" --stream | choose 0 | xargs -I{} my-downloader {}

# Offline smoke test via wiremock-backed suite.
cargo test --test integracao_wiremock
```

### Exit Codes

| Code | Meaning                                                        |
| ---- | -------------------------------------------------------------- |
| 0    | Success.                                                       |
| 1    | Runtime error (network, parse, I/O).                           |
| 2    | Invalid configuration (CLI flag out of range, bad proxy URL).  |
| 3    | DuckDuckGo 202 block anomaly (soft-rate-limit).                |
| 4    | Global timeout exceeded.                                       |
| 5    | Zero results across all queries.                               |

### Troubleshooting

1. **HTTP 202 / block anomaly**: back off, raise `--retries`, rotate UA via `init-config` and tweak `user-agents.toml`.
2. **Rate limited (HTTP 429)**: increase `--per-host-limit` *reduction*, enable `--match-platform-ua`, or add `--proxy`.
3. **Zero results (exit 5)**: check `--lang`/`--country`, try `--endpoint lite`, and verify `--time-filter`.
4. **Chrome not found**: install Chromium via your package manager, or pass `--chrome-path /path/to/chrome`; feature must be compiled (`cargo install duckduckgo-search-cli --features chrome`).
5. **UTF-8 issues on Windows**: the binary auto-switches cmd.exe to code page 65001; if you see mojibake, set `chcp 65001` before the run.

See the [CHANGELOG](CHANGELOG.md) for release history.

License: MIT OR Apache-2.0.

---

## Português

### O que é?

- CLI em Rust que consulta `html.duckduckgo.com/html/` via HTTP puro.
- Sem API paga, sem Chrome no caminho quente, sem cache em disco.
- Emite JSON, texto, Markdown ou streams NDJSON prontos para alimentar LLMs.
- Multiplataforma: Linux (glibc, musl/Alpine, Flatpak, Snap, NixOS), macOS (Intel + Apple Silicon), Windows (cmd.exe + PowerShell).
- Fallback Chrome headless sob flag `--features chrome` para páginas com JavaScript pesado.
- Pipeline multi-query paralelo com rate-limit por host e cancelamento.

### Por quê?

- Primitivo plugável para RAG: schema JSON estável, códigos de saída estáveis.
- Sem estado oculto — configuração mora no diretório XDG.
- TLS só via rustls: sem OpenSSL em runtime, sem surpresa no SChannel, builds musl estáticas funcionam de primeira.
- Timeouts explícitos (`--timeout`, `--global-timeout`) e concorrência limitada (`--parallel`, `--per-host-limit`) deixam o binário seguro em automação.

### Início Rápido

```bash
cargo install duckduckgo-search-cli
duckduckgo-search-cli init-config
duckduckgo-search-cli "tokio async runtime" --num 5 --format json
duckduckgo-search-cli "tokio async runtime" --fetch-content --max-content-length 5000 -o /tmp/out.json
```

### Comandos

| Comando                                  | Propósito                                               |
| ---------------------------------------- | ------------------------------------------------------- |
| `duckduckgo-search-cli <QUERY>...`       | Busca padrão (equivalente a `buscar`).                  |
| `duckduckgo-search-cli buscar <QUERY>..` | Subcomando explícito de busca.                          |
| `duckduckgo-search-cli init-config`      | Grava `selectors.toml` e `user-agents.toml` no XDG.     |

### Flags

| Flag                       | Padrão     | Descrição                                                          |
| -------------------------- | ---------- | ------------------------------------------------------------------ |
| `-n`, `--num`              | (todos)    | Máximo de resultados por query por página.                         |
| `-f`, `--format`           | `auto`     | `json`, `text`, `markdown` ou `auto` (detecta TTY).                |
| `-o`, `--output`           | stdout     | Grava no arquivo (diretórios criados, permissão Unix 0o644).       |
| `-t`, `--timeout`          | `15`       | Timeout por request (segundos).                                    |
| `--global-timeout`         | `60`       | Timeout global do pipeline (1..=3600 segundos).                    |
| `-l`, `--lang`             | `pt`       | Código de idioma `kl` do DuckDuckGo.                               |
| `-c`, `--country`          | `br`       | Código de país `kl` do DuckDuckGo.                                 |
| `-p`, `--parallel`         | `5`        | Requests concorrentes (1..=20).                                    |
| `--pages`                  | `1`        | Páginas por query (1..=5).                                         |
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

### Variáveis de Ambiente

| Variável       | Descrição                                                       | Exemplo                            |
| -------------- | --------------------------------------------------------------- | ---------------------------------- |
| `RUST_LOG`     | Sobrescreve o filtro do `tracing-subscriber`.                   | `RUST_LOG=duckduckgo=debug`        |
| `HTTP_PROXY`   | Proxy HTTP padrão (prioridade menor que `--proxy`).             | `http://user:pass@proxy:8080`      |
| `HTTPS_PROXY`  | Proxy HTTPS padrão.                                             | `http://proxy:8443`                |
| `ALL_PROXY`    | Proxy fallback para qualquer scheme.                            | `socks5://127.0.0.1:9050`          |
| `CHROME_PATH`  | Caminho fallback para Chrome (feature `chrome`).                | `/opt/google/chrome/chrome`        |

### Formatos de Saída

- `json` (default em pipes): schema canônico com `resultados[]`, `metadados`, `buscas_relacionadas`.
- `text`: bloco legível `NN. Título\n   URL\n   snippet`.
- `markdown`: `- [Título](URL)\n  > snippet`.
- Stream (`--stream`): NDJSON, cada linha é um resultado; metadados na linha final.

### Padrões de Integração

```bash
# Pipe direto para pré-processamento em LLM.
duckduckgo-search-cli "site:example.com release 2025" --num 3 -f json | jaq '.resultados[].url'

# Enviar bodies extraídos para summarizer.
duckduckgo-search-cli "rust async tokio" --fetch-content --max-content-length 4000 -f json \
  | jaq -r '.resultados[] | "# \(.titulo)\n\(.conteudo)\n"' > corpus.md

# Modo streaming com xargs.
duckduckgo-search-cli "wasm runtimes" --stream | choose 0 | xargs -I{} downloader {}

# Testes offline com wiremock.
cargo test --test integracao_wiremock
```

### Códigos de Saída

| Código | Significado                                                 |
| ------ | ----------------------------------------------------------- |
| 0      | Sucesso.                                                    |
| 1      | Erro de runtime (rede, parse, I/O).                         |
| 2      | Configuração inválida (flag fora de faixa, proxy malformado).|
| 3      | Bloqueio DuckDuckGo (anomalia HTTP 202).                    |
| 4      | Timeout global excedido.                                    |
| 5      | Zero resultados em todas as queries.                        |

### Troubleshooting

1. **HTTP 202 / bloqueio**: aumente `--retries`, rotacione UA via `init-config` editando `user-agents.toml`.
2. **Rate limit (HTTP 429)**: reduza `--per-host-limit`, ative `--match-platform-ua` ou use `--proxy`.
3. **Zero resultados (exit 5)**: confira `--lang` e `--country`, tente `--endpoint lite`, revise `--time-filter`.
4. **Chrome não encontrado**: instale Chromium pelo gerenciador de pacotes ou passe `--chrome-path /caminho/chrome`; a feature precisa ser compilada (`cargo install duckduckgo-search-cli --features chrome`).
5. **Problemas UTF-8 no Windows**: o binário muda cmd.exe para code page 65001 automaticamente; se ver mojibake, execute `chcp 65001` antes.

Veja o [CHANGELOG](CHANGELOG.md) para o histórico completo.

Licença: MIT OR Apache-2.0.
