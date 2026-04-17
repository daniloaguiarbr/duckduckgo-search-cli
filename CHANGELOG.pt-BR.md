# Changelog

Leia este arquivo em [English](CHANGELOG.md).

- Todas as mudanças notáveis deste projeto estão documentadas neste arquivo
- O formato segue [Keep a Changelog](https://keepachangelog.com/pt-BR/1.0.0/)
- Este projeto adere ao [Versionamento Semântico](https://semver.org/lang/pt-BR/)


## [Não Publicado]
### Adicionado
- `LICENSE-MIT` e `LICENSE-APACHE` — licença dupla conforme declaração SPDX em `Cargo.toml`
- `.pre-commit-config.yaml` com três grupos de hooks: (1) hooks padrão pre-commit (espaços em branco, EOF, validação YAML/TOML, finais de linha mistos), (2) hooks Rust (`cargo fmt` + `cargo clippy -D warnings`), (3) hook local `commit-msg` bloqueando `Co-authored-by:` de agentes de IA
- `.gitattributes` forçando LF em `.rs` / `.toml` / `.sh` / `.yml` / `.md` / HTML de fixture — previne corrupção silenciosa ao clonar no Windows com `core.autocrlf=true`
- `.editorconfig` normalizando UTF-8, LF, remoção de espaços em branco e indentação por linguagem (Rust/TOML 4, YAML/JSON/MD 2, Makefile tab)
- `.github/PULL_REQUEST_TEMPLATE.md` com checklist de 10 gates e restrições específicas do projeto
- `.github/ISSUE_TEMPLATE/bug_report.yml` + `feature_request.yml` + `config.yml` — triagem estruturada com dropdown de plataforma
- `Cross.toml` habilitando `cross build --target <t>` para targets ARM64/ARMv7 Linux
- `CONTRIBUTING.md` com matriz de validação de 10 gates, padrões de código e processo de release
- `.cargo/config.toml` expondo 8 aliases de desenvolvimento (`cargo check-all`, `cargo lint`, `cargo docs`, `cargo test-all`, `cargo cov`, `cargo cov-html`, `cargo publish-check`, `cargo pkg-list`)
- Doctests na API pública: `pipeline::combinar_e_deduplicar_queries`, `fetch_conteudo::extrair_host` e `search::formatar_kl`
- `SECURITY.md` documentando fluxo de divulgação privada via GitHub Security Advisories com SLA de 72h
- `.github/dependabot.yml` habilitando atualizações automáticas semanais de dependências para ecossistemas `cargo` e `github-actions`
- `rust-toolchain.toml` fixando `stable` com componentes `rustfmt` + `clippy` para builds reproduzíveis
- `.github/workflows/release.yml` disparado por tags `v*.*.*` executando pipeline de release em 5 estágios
- Job `msrv` em `ci.yml` extraindo `rust-version` do `Cargo.toml` e executando `cargo check` nessa toolchain
- `.github/workflows/ci.yml` aplicando matriz de validação de 10 gates em Ubuntu, macOS e Windows
- `deny.toml` com política de supply chain em quatro eixos (advisories/licenses/bans/sources)
- 22 novos testes elevando cobertura de 77,4% para 86,4% (linhas)
### Alterado
- Cobertura `parallel.rs` 50% → 81%; `pipeline.rs` 55% → 82%; `fetch_conteudo.rs` 68% → 85%; `output.rs` 70% → 87%


## [0.6.2] - 2026-04-17
### Adicionado
- 19 novos arquivos de documentação — conformidade completa com rules_rust_documentacao.md (28 gaps G01-G28)
- Documentação bilíngue EN+PT: HOW_TO_USE, CROSS_PLATFORM, AGENTS-GUIDE, COOKBOOK.pt-BR, INTEGRATIONS.pt-BR
- CODE_OF_CONDUCT.md + CODE_OF_CONDUCT.pt-BR.md — Contributor Covenant 2.1
- README.pt-BR.md, CHANGELOG.pt-BR.md, CONTRIBUTING.pt-BR.md, SECURITY.pt-BR.md
- docs/AGENTS.pt-BR.md — guia imperativo para LLMs em português
- docs/AGENTS-GUIDE.md + docs/AGENTS-GUIDE.pt-BR.md — guia persuasivo bilíngue
- llms.txt — arquivo compacto de orientação para LLMs (< 50 KB)
- llms-full.txt — concatenação completa de docs para contexto longo de LLMs
- eval-queries.json × 2 — 20 queries de avaliação EN + 20 PT-BR para skill testing
### Alterado
- README.md — link para README.pt-BR.md + quick install antes da linha 30
- CONTRIBUTING.md — MSRV Rust 1.75 explícito + PR checklist 8 itens + branching strategy + nextest
- SECURITY.md — tabela de versão específica v0.6.2 + política de embargo 90 dias + zero bold + zero emojis
- skill/SKILL.md (EN+PT) — seção Workflow com 5 passos numerados verificáveis


## [0.6.1] - 2026-04-17
### Corrigido
- `--timeout 0` agora retorna exit 2 (configuração inválida) em vez de executar busca com timeout zero e retornar exit 5
- `--output /tmp/../../etc/passwd` agora retorna exit 2 (configuração inválida) em vez de exit 1 — validação de path traversal movida para `montar_configuracoes()`, antes do início do pipeline
### Adicionado
- Método `validar_timeout_segundos()` em `ArgumentosCli` — rejeita valores 0 com mensagem de erro descritiva
- Verificação antecipada de path traversal em `montar_configuracoes()` — chama `paths::validar_caminho_saida()` no momento de validação da configuração, não no momento de escrita
- 2 testes E2E de regressão: `timeout_zero_retorna_exit_2` e `output_com_path_traversal_retorna_exit_2`
- 1 teste unitário: `validar_timeout_segundos_rejeita_zero`


## [0.6.0] - 2026-04-16
### Segurança
- Perfis de fingerprint de browser por família previnem detecção anti-bot do DuckDuckGo
- Headers `Sec-Fetch-*` e Client Hints por família imitam sessão de navegador real
- `Accept-Language` com q-values RFC 7231 elimina fingerprint de UA genérico
- Detecção de bloqueio silencioso com limiar de 5 KB previne resultados truncados
### Adicionado
- Enum `FamiliaBrowser` — variantes `Chrome`, `Firefox`, `Edge`, `Safari`
- Struct `PerfilBrowser` — encapsula família, versão e conjunto de headers por família
- Headers `Sec-Fetch-Dest`, `Sec-Fetch-Mode`, `Sec-Fetch-Site` por família em `http.rs`
- Client Hints (`Sec-Ch-Ua`, `Sec-Ch-Ua-Mobile`, `Sec-Ch-Ua-Platform`) para Chrome e Edge
- Detecção de anomalia HTTP 202 em `search.rs` com backoff exponencial automático
- Detecção de bloqueio silencioso — resposta com menos de 5 000 bytes é tratada como bloqueio
- `PerfilBrowser` propagado via `Configuracoes` para todos os módulos do pipeline
- Headers de paginação com `Sec-Fetch-Site: same-origin` para imitar navegação real
### Alterado
- `Accept-Language` atualizado para `pt-BR,pt;q=0.9,en-US;q=0.8,en;q=0.7` conforme RFC 7231
- Header `Accept` agora reflete o perfil completo do browser por família
- Delays de paginação aumentados de 500–1 000 ms para 800–1 500 ms
- Limiar de bloqueio silencioso aumentado de 100 para 5 000 bytes


## [0.5.0] - 2026-04-16
### Segurança
- Validação de path traversal em `--output` — rejeita componentes `..` e escritas em diretórios de sistema (`/etc`, `/usr`, `C:\Windows`)
- Mascaramento de credenciais de proxy — mensagens de erro não expõem mais senhas de URLs `--proxy http://user:pass@host`
### Adicionado
- `src/paths.rs` — validação centralizada de caminhos, criação de diretório pai e aplicação de permissões Unix
- `src/signals.rs` — restauração centralizada de SIGPIPE (Unix) e handler Ctrl+C/SIGINT (cross-platform)
- Enum `ErroCliDdg` com `thiserror` — 11 variantes de erro tipadas com métodos `exit_code()` e `codigo_erro()`
- `mascarar_url_proxy()` em `http.rs` — remove credenciais de URLs de proxy no contexto de erro
- 21 novos testes unitários em `paths.rs`, `signals.rs`, `error.rs` e `http.rs`
### Alterado
- `thiserror = "2"` adicionado às dependências para erros de domínio estruturados
- `src/main.rs` reduzido de 63 para 23 linhas — tratamento de sinais extraído para `signals.rs`
- Escritas de arquivo em `src/output.rs` agora validam caminhos via `paths::validar_caminho_saida()` antes do I/O
- `deny.toml` atualizado com exceção RUSTSEC-2026-0097 (rand 0.8 unsound com logger customizado — não aplicável)


## [0.4.4] - 2026-04-16
### Corrigido
- SIGPIPE restaurado para SIG_DFL no Unix — pipes para `jaq`, `head` e outros consumidores não perdem mais stdout silenciosamente
- Erros BrokenPipe detectados na cadeia anyhow e tratados como exit 0 (não exit 1) em todos os pontos de saída
### Adicionado
- `--help` agora exibe seções EXIT CODES (0–5) e PIPE USAGE via `after_long_help`
- 3 testes E2E para regressão de pipe: exit codes no help, exclusão do help curto, contagem de bytes no stdout
- Item 7 no troubleshooting do README: "Pipe para jaq/jq retorna vazio" com diagnóstico PIPESTATUS (EN + PT)
- `docs_rules/rules_rust.md`: SIGPIPE + BrokenPipe adicionados ao checklist de I/O
- `docs/AGENT_RULES.md`: regra R24 de segurança de pipe com diagnóstico PIPESTATUS
- `docs/COOKBOOK.md`: Receita 16 de diagnóstico de pipe (EN + PT)
- `docs/INTEGRATIONS.md`: cláusula de segurança de pipe no contrato base
- Seção de ramificação por exit code em ambos os arquivos de skill (EN + PT)


## [0.4.3] - 2026-04-15
### Alterado
- `README.md` — nova seção persuasiva "Agent Skill" (EN + PT) posicionada entre a tabela de agentes e a seção de Documentação, no pico de atenção do leitor
- Copywriting AIDA destacando a skill bilíngue empacotada em `skill/`: auto-ativação semântica sem slash command, 14 seções canônicas MUST/NEVER, contrato JSON anti-alucinação, economia de tokens em cada turno de busca, instalação em um comando (`git clone` + `cp -r`)
- Benefícios explícitos para LLMs (decisão automática de quando buscar) e desenvolvedores (zero prompt engineering, zero registro de ferramenta)
- Tarball do crates.io inalterado — skills continuam vivendo apenas no GitHub


## [0.4.2] - 2026-04-15
### Adicionado
- `skill/duckduckgo-search-cli-pt/SKILL.md` e `skill/duckduckgo-search-cli-en/SKILL.md` — Skills bilíngues para Claude Code, Claude Agent SDK e plataformas compatíveis com Agent Skills
- Cada skill traz frontmatter YAML com `name` único por idioma e `description` carregado de triggers semânticos para auto-invocação
- 14 seções H2 canônicas: Missão, Contrato de Invocação, Proibições Absolutas, Parsing com `jaq`, Schema JSON, Exit Codes, Batch, Fetch-Content, Endpoint, Retries, Receitas, Validação, Memória, Regra de Ouro
- Skills publicadas no GitHub, excluídas do tarball do crates.io
### Alterado
- `docs/AGENT_RULES.md` (833 linhas, +7,6%) — reescrita editorial aplicando copywriting AIDA: cada regra abre com benefício mensurável, linguagem imperativa MUST/NEVER reforçada, zero narrativa decorativa, zero negrito com asteriscos duplos, zero separador visual `---` entre seções
- `docs/COOKBOOK.md` (1082 linhas, −3,1%) — cada receita abre com o ganho concreto antes do comando, bullets curtos de 8 a 15 palavras, pipelines `jaq` + `xh` + `sd` preservados intactos
- `docs/INTEGRATIONS.md` (1212 linhas, +1,3%) — 16 agentes com tabela comparativa textual, snippets determinísticos por agente, zero emoji
### Meta
- `Cargo.toml` exclude ampliado para cobrir `skill/` e `skill/**` — skills ficam no GitHub e fora do tarball publicado no crates.io


## [0.4.1] - 2026-04-14
### Adicionado
- `docs/AGENT_RULES.md` (773 linhas) — regras imperativas bilíngue (EN+PT) com 30+ rules `MUST`/`NEVER` (R01..R30) para LLMs/agentes invocarem a CLI em produção
- Cobre invariantes core, contrato JSON, rate limiting, tratamento de erros, performance, segurança e anti-patterns
- Quick Reference Card ao final do documento
- `docs/COOKBOOK.md` (1117 linhas) — 15 receitas copy-paste bilíngue combinando `duckduckgo-search-cli` + `jaq` + `xh` + `sd` para casos reais
- Casos cobertos: research consolidado, ETL multi-query, extração de domínios, monitoramento com filtro temporal, content extraction com `--fetch-content`, comparação top 5 vs top 15, NDJSON para pipelines, function wrappers para bash
- `docs/INTEGRATIONS.md` (1196 linhas) — snippets prontos para 16 agentes/LLMs: Claude Code, OpenAI Codex, Gemini CLI, Cursor, Windsurf, Aider, Continue.dev, MiniMax, OpenCode, Paperclip, OpenClaw, Google Antigravity, GitHub Copilot CLI, Devin, Cline, Roo Code
- Cada agente documenta: pitch, mecanismo de shell, setup, snippet básico, snippet multi-query, regra de system prompt e ressalvas
- Seção Documentation no `README.md` (EN + PT) linkando os 3 guias
### Corrigido
- Cluster de badges e referências internas do `README.md` conferidas contra `daniloaguiarbr/duckduckgo-search-cli` (repo canônico)


## [0.4.0] - 2026-04-14
### Alterado (BREAKING)
- Default de `--num` / `-n` alterado de "todos os resultados da primeira página" (~11) para 15, com auto-paginação automática
- Quando o número efetivo excede 10, o binário busca 2 páginas por query para satisfazer o teto solicitado, desde que `--pages` não tenha sido customizado
- Auto-paginação: se `--num > 10` E `--pages` não foi customizado, o binário auto-eleva `--pages` para `ceil(num/10)` respeitando o teto de 5 páginas
- Impacto: mais requests por query (2x no caso default) e latência marginalmente maior, com cobertura completa dos resultados solicitados
### Adicionado
- Documentação no comentário do flag `--num` em `cli.rs` descrevendo a nova semântica de default e auto-paginação
- 4 novos testes unitários em `lib.rs::testes`: `montar_configuracoes_aplica_default_num_15_quando_omitido`, `montar_configuracoes_respeita_pages_explicito_acima_de_1`, `montar_configuracoes_auto_pagina_quando_num_maior_que_10`, `montar_configuracoes_nao_auto_pagina_quando_num_10_ou_menos`
- 2 novos testes wiremock em `tests/integracao_wiremock.rs`: `testa_default_num_15_auto_pagina_2_paginas`, `testa_auto_paginacao_respeita_pages_explicito`
### Guia de Migração
- Para preservar o comportamento antigo (1 página, ~11 resultados): passe `--pages 1 --num 10` explicitamente
- Quem já passava `--num 5` (ou qualquer valor <= 10): comportamento inalterado (sem auto-paginação, 1 página)
- Quem já passava `--num 20 --pages 2` ou similar: comportamento inalterado (respeita explícito do usuário)
- Quem confiava no default sem flags: agora recebe até 15 resultados em vez de ~11, com 1 request extra por query


## [0.3.0] - 2026-04-14
### Alterado (BREAKING)
- Campo `buscas_relacionadas` REMOVIDO de `SaidaBusca` e `SaidaBuscaMultipla.buscas[i]` — o endpoint `html.duckduckgo.com/html/` não expõe related searches no DOM atual; manter o campo sempre vazio era ruído
- Pipelines que parseavam `.buscas_relacionadas` precisam de ajuste
- Pool de User-Agents: removidos UAs de browsers de texto (`Lynx 2.9.0`, `w3m/0.5.3`, `Links 2.29`, `ELinks 0.16.1.1`) que faziam o DuckDuckGo retornar HTML degradado
- Substituídos por 6 UAs modernos validados empiricamente contra o endpoint `/html/`: Chrome 146 (Win/Mac/Linux), Edge 145 Windows, Firefox 134 Linux, Safari 17.6 macOS
- Firefox Win/Mac foram REMOVIDOS após retornarem anomalia HTTP 202 em validação real (heurística anti-bot do DDG)
### Corrigido
- Snippet duplicava título e URL no início: o seletor padrão tinha fallback `.result__body` (container pai) que fazia `text()` recursivo capturar título+URL+snippet concatenados — trocado por `.result__snippet` puro
- Pipelines como `jaq '.resultados[].snippet'` agora retornam apenas o texto descritivo do resultado
- Título "Official site": o DuckDuckGo renderiza literalmente este texto como label para domínios verificados — o scraper agora detecta este caso e substitui pelo `url_exibicao`
- O texto original é preservado no novo campo opcional `titulo_original` para auditoria
### Adicionado
- Campo `titulo_original: Option<String>` em `ResultadoBusca` — presente apenas quando o título foi substituído por heurística
- Serializado com `#[serde(skip_serializing_if = "Option::is_none")]` — não aparece no JSON quando ausente
- Resultados patrocinados (`.result--ad`) excluídos do container default via seletor `.result:not(.result--ad)`
### Removido
- Função `extrair_buscas_relacionadas` em `src/search.rs` (dead code com seletor hardcoded que nunca encontrava nada)
- Seção `[related_searches]` nos seletores default
### Guia de Migração (v0.2.x → v0.3.0)
- Pipelines `jaq '.buscas_relacionadas[]'`: campo não existe mais — remover do filtro ou tratar `null`
- Esperando snippet com prefixo título+URL? Agora vem só o texto descritivo — ajuste regex/parsing downstream se necessário
- Confiando em `titulo == "Official site"` para detectar sites verificados? Use `titulo_original.as_deref() == Some("Official site")`
- CONFIG EXTERNO LEGADO: usuários que rodaram `init-config` em versões anteriores possuem `~/.config/duckduckgo-search-cli/{selectors,user-agents}.toml` com defaults antigos — execute `duckduckgo-search-cli init-config --force` para aplicar as correções


## [0.2.0] - 2026-04-14
### Alterado (BREAKING)
- Schema JSON serializado agora usa nomes de campo em português brasileiro, alinhado com os exemplos `jaq` do README e com o invariante INVIOLÁVEL do blueprint v2 do projeto
- Pipelines que dependiam do schema em inglês da v0.1.0 precisam atualizar os seletores `jaq`
- Tabela de renomeações de campos:

| Antes (v0.1.0) | Depois (v0.2.0) |
|----------------|-----------------|
| `position` | `posicao` |
| `title` | `titulo` |
| `displayed_url` | `url_exibicao` |
| `content` | `conteudo` |
| `content_length` | `tamanho_conteudo` |
| `content_extraction_method` | `metodo_extracao_conteudo` |
| `execution_time_ms` | `tempo_execucao_ms` |
| `selectors_hash` | `hash_seletores` |
| `retries` | `retentativas` |
| `fallback_endpoint_used` | `usou_endpoint_fallback` |
| `concurrent_fetches` | `fetches_simultaneos` |
| `fetch_successes` | `sucessos_fetch` |
| `fetch_failures` | `falhas_fetch` |
| `chrome_used` | `usou_chrome` |
| `proxy_used` | `usou_proxy` |
| `engine` | `motor` |
| `region` | `regiao` |
| `results_count` | `quantidade_resultados` |
| `results` | `resultados` |
| `related_searches` | `buscas_relacionadas` |
| `pages_fetched` | `paginas_buscadas` |
| `error` | `erro` |
| `message` | `mensagem` |
| `metadata` | `metadados` |
| `queries_count` | `quantidade_queries` |
| `parallel` | `paralelismo` |
| `searches` | `buscas` |

- Campos inalterados: `url`, `snippet`, `query`, `endpoint`, `timestamp`, `user_agent`
### Corrigido
- Pipelines documentados no README (`jaq '.resultados[].titulo'`, etc.) agora funcionam end-to-end — em v0.1.0 retornavam `null` por divergência do schema (bug reportado pelo usuário)


## [0.1.0] - 2026-04-14
### Adicionado
- Pipeline de busca core contra o endpoint HTML do DuckDuckGo via HTTP puro (`html.duckduckgo.com/html/`)
- Fallback para endpoint lite via `--endpoint lite` para páginas sem JavaScript
- Modo multi-query com deduplicação automática, args posicionais, `--queries-file` e stdin
- Fan-out paralelo de queries com `--parallel` (1..=20), limitado por `tokio::JoinSet` + `Semaphore`
- `--pages` (1..=5) para coletar múltiplas páginas de resultado por query
- `--fetch-content` busca cada URL de resultado via HTTP puro, aplica readability e embute o texto limpo no JSON
- `--max-content-length` (1..=100 000) trunca conteúdo extraído respeitando fronteiras de palavras
- Fallback Chrome headless via `--features chrome` com detecção cross-platform e flags de stealth
- Flag `--chrome-path` para especificar manualmente o executável Chrome/Chromium
- `--proxy URL` + `--no-proxy` (HTTP/HTTPS/SOCKS5) com precedência sobre variáveis de ambiente
- `--global-timeout` (1..=3600 s) envolve todo o pipeline em `tokio::time::timeout`
- `--per-host-limit` (1..=10) limita fetches por host via mapa de `Semaphore` por host
- `--match-platform-ua` restringe o pool de user-agents à plataforma atual
- Modo NDJSON `--stream` emite um resultado por linha conforme extraídos
- Quatro formatos de saída: `json` (padrão), `text`, `markdown`, `auto` (TTY-aware)
- Arquivos de configuração externos: `selectors.toml` e `user-agents.toml` no diretório XDG config, sobrescrevendo defaults embutidos
- Subcomando `init-config` com `--force` e `--dry-run` para inicializar arquivos de configuração do usuário
- Exit codes: `0` sucesso, `1` runtime, `2` config, `3` bloqueio (anomalia HTTP 202), `4` timeout global, `5` zero resultados
- Inicialização de console UTF-8 no Windows via `SetConsoleOutputCP(65001)`
- Rustls-TLS em toda a CLI para builds cross-platform sem dependências adicionais
- `tracing` + `tracing-subscriber` com `RUST_LOG` respeitado; flags `--verbose` / `--quiet`
- 163 testes unitários + integração cobrindo parsing CLI, montagem de config, extração HTTP, fan-out paralelo, seletores e fluxos de busca via wiremock
### Segurança
- Todas as credenciais (`--proxy user:pass@host`) são mascaradas nos logs
- Criação de arquivo de saída aplica permissões Unix `0o644`


[Unreleased]: https://github.com/comandoaguiar/duckduckgo-search-cli/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/comandoaguiar/duckduckgo-search-cli/releases/tag/v0.1.0
