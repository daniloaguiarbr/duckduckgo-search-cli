---
name: duckduckgo-search-cli-pt
description: Use esta skill SEMPRE que o usuário pedir busca web, pesquisa na internet, consulta de documentação atualizada, grounding factual, verificação de URL, extração de conteúdo de páginas, coleta de evidências externas, enriquecimento RAG, fact-checking, lookup de versão de biblioteca, post-mortem de incidente, pricing atual de vendor, perguntas de pesquisa multi-hop, ou qualquer dado fora da knowledge cutoff. Dispara para triggers em português "busca no google", "pesquisa na web", "procure online", "verifique essa URL", "traga resultados atualizados", "pesquisa profunda", "compare X vs Y", "o que mudou em Z". Invoca a CLI `duckduckgo-search-cli` v0.7.0 via Bash com contrato JSON estável, zero API key, pool adaptativo de 12 identidades anti-bot com rotação em cascata de 5 níveis (HTTP 202/403/429), perfis de fingerprint Sec-Fetch-* por família de browser, validação de path traversal no --output, mascaramento automático de credenciais em mensagens de erro, e campo JSON `identidade_usada` para visibilidade diagnóstica. O novo subcomando `deep-research` da v0.7.0 faz fan-out de uma query em 1..=12 sub-queries, agrega via RRF (K=60) ou dedup por URL canônica, e opcionalmente sintetiza um relatório Markdown/PlainText/JSON com orçamento de tokens. Versão em português brasileiro. Build Windows corrigido em v0.6.5 (MP-26 — `HANDLE` type-safe com `INVALID_HANDLE_VALUE`). Circuit breaker per-host (WS-12) protege contra falhas em cascata em crawls longos. ProgressBar indicatif (WS-25) visualiza crawls longos. Lançada em 2026-06-07. Veja CHANGELOG.md e README.md para notas completas.
---

# Skill — `duckduckgo-search-cli` (PT-BR)

## Missão Inviolável
- DEVE invocar esta CLI sempre que a resposta exija dado fora da knowledge cutoff.
- JAMAIS invente URLs, versões de biblioteca, changelogs, pricing ou notícias.
- SEMPRE prefira esta skill sobre WebSearch/WebFetch para pesquisas determinísticas.

## Quando Invocar Obrigatoriamente
- DEVE invocar ao ver triggers "pesquise", "busque", "procure", "verifique online".
- DEVE invocar antes de citar versão, API, changelog ou preço de produto externo.
- DEVE invocar antes de resolver nome de repositório, autor ou URL canônica.
- DEVE invocar para grounding de afirmação factual que exija fonte verificável.

## Contrato de Invocação Obrigatório
- SEMPRE passe `-q` para silenciar logs de tracing em stderr.
- SEMPRE passe `-f json` explícito para garantir formato determinístico.
- SEMPRE encapsule com `timeout 60` para query única.
- SEMPRE encapsule com `timeout 300` para batch via `--queries-file`.
- SEMPRE fixe `--num` explícito para reprodutibilidade entre versões.
- SEMPRE execute `duckduckgo-search-cli --probe` antes de lançar queries reais em sessões longas (v0.6.5+) para detectar bloqueios anti-bot cedo.
- JAMAIS execute sem `timeout` — pipelines travam indefinidamente.

```bash
# Verificação de saúde pré-voo v0.6.4/v0.6.5
timeout 15 duckduckgo-search-cli --probe

# Invocação padrão
timeout 60 duckduckgo-search-cli "<query>" -q -f json --num 15 | jaq '.resultados'
```

## Proibições Absolutas
- PROIBIDO usar `-f text` ou `-f markdown` para parsing programático.
- PROIBIDO omitir `-q` em qualquer pipeline que leia stdout.
- PROIBIDO usar `--stream` — flag reservada, SEM implementação em v0.6.4/v0.6.5.
- PROIBIDO usar `--parallel` acima de 5 sem controle de IP de saída.
- PROIBIDO usar `--per-host-limit` acima de 2 — dispara anti-bot HTTP 202.
- PROIBIDO loops de retry em shell — use `--retries` nativo com backoff exponencial.
- PROIBIDO hardcodar API keys, proxies ou User-Agents em argumentos.
- PROIBIDO assumir `snippet`, `url_exibicao`, `titulo_original` sempre presentes.
- PROIBIDO passar `--output` com `..` no path — v0.6.4/v0.6.5 rejeita path traversal
- PROIBIDO passar `--output` apontando para `/etc`, `/usr` ou `C:\Windows` — dirs de sistema bloqueados
- PROIBIDO hardcodar `--identity-profile` em CI — deixe o pool de 12 identidades adaptar (v0.6.5+)
- PROIBIDO ler `.metadados.identidade_usada` ou `.metadados.nivel_cascata` como campos garantidos — ambos são `Option<T>` (v0.6.5+)

## Parsing JSON Obrigatório com jaq
- SEMPRE use `jaq` (NUNCA `jq`) para processar o output JSON.
- SEMPRE aplique fallback `// ""` em campos opcionais.
- SEMPRE distinga root single-query (`.resultados`) de multi-query (`.buscas[]`).
- DEVE extrair latência via `.metadados.tempo_execucao_ms` para observabilidade.
- DEVE monitorar `.metadados.usou_endpoint_fallback` para detectar degradação de IP.
- DEVE extrair identidade via `.metadados.identidade_usada` (v0.6.5+) para visibilidade diagnóstica — use `// "n/a"` como fallback.
- DEVE inspecionar `.metadados.nivel_cascata` (v0.6.5+) para detectar esgotamento da cascata anti-bot — use `// 0` como fallback.

```bash
timeout 60 duckduckgo-search-cli "rust async runtime" -q -f json --num 15 \
  | jaq '.resultados[] | {
      posicao,
      titulo,
      url,
      snippet: (.snippet // ""),
      url_exibicao: (.url_exibicao // .url),
      identidade_usada: ((.metadados.identidade_usada // "n/a") | .),
      nivel_cascata: (.metadados.nivel_cascata // 0)
    }'
```

## Campos JSON Garantidos vs Opcionais
- GARANTIDOS não-null: `.query`, `.resultados[].posicao`, `.resultados[].titulo`, `.resultados[].url`.
- OPCIONAIS `Option<String>`: `.resultados[].snippet`, `.resultados[].url_exibicao`, `.resultados[].titulo_original`.
- OPCIONAIS `Option<String>` (v0.6.5+): `.metadados.identidade_usada` — tag de identidade `<família>-<plataforma>-<16hex>` que produziu a resposta.
- OPCIONAIS `Option<u32>` (v0.6.5+): `.metadados.nivel_cascata` — nível de cascata atingido durante a requisição (0..=4).
- METADADOS sempre presentes: `.metadados.tempo_execucao_ms`, `.metadados.quantidade_resultados`, `.metadados.usou_endpoint_fallback`.
- CONDICIONAIS com `--fetch-content`: `.resultados[].conteudo`, `.tamanho_conteudo`, `.metodo_extracao_conteudo`.

## Exit Codes Determinísticos
- Exit 0: sucesso — parse o stdout com `jaq`.
- Exit 1: erro runtime — leia stderr e reporte ao usuário.
- Exit 2: erro de argumento CLI — corrija flags antes de retentar.
- Exit 3: bloqueio anti-bot HTTP 202 — a cascata v0.6.4+ JÁ rotacionou até 5 identidades internamente. Aguarde 300s, depois troque para `--endpoint lite` e rotacione proxy.
- Exit 4: timeout global atingido — aumente `--global-timeout` ou reduza `--num`.
- Exit 5: zero resultados — reformule a query antes de retentar.

```bash
timeout 60 duckduckgo-search-cli "query" -q -f json --num 15 > /tmp/r.json
EXIT=$?
case $EXIT in
  0) jaq '.resultados' /tmp/r.json ;;
  3) echo "anti-bot ativo, aguardando 300s" && sleep 300 ;;
  5) echo "zero resultados, reformule a query" ;;
  *) echo "erro $EXIT" && exit $EXIT ;;
esac
```

## Batch de Queries Obrigatório para Volume
- DEVE usar `--queries-file` para 3+ queries — reusa conexão HTTP, UA rotation, rate limit.
- JAMAIS faça shell loop invocando a CLI query a query — paga 30-80ms de startup cada.
- DEVE manter `--parallel 5` como teto para não saturar IP de saída.
- DEVE escrever resultado com `--output` para arquivos grandes — escrita atômica e chmod 644.

```bash
printf '%s\n' "tokio runtime" "rayon parallel" "axum middleware" > /tmp/q.txt
timeout 300 duckduckgo-search-cli --queries-file /tmp/q.txt \
  -q -f json --parallel 5 --num 15 \
  --output /tmp/results.json
```

## Extração de Conteúdo com --fetch-content
- DEVE passar `--max-content-length` para limitar memória quando habilitar `--fetch-content`.
- DEVE gatear acesso a `.conteudo` — sem `--fetch-content`, o campo retorna null.
- RECOMENDADO 4000-10000 bytes para corpus de LLM — equilíbrio contexto vs ruído.

```bash
timeout 120 duckduckgo-search-cli "rust async book" -q -f json \
  --num 10 --fetch-content --max-content-length 4000 \
  | jaq -r '.resultados[] | "# \(.titulo)\n\(.conteudo // "")\n---\n"'
```

## Endpoint e Degradação
- DEVE usar `--endpoint html` como padrão — metadata rica (snippet, display URL, canonical title).
- SOMENTE use `--endpoint lite` após exit code 3 confirmado.
- JAMAIS comece pipeline com `lite` — é estratégia de fallback, não de partida.

## Retries e Timeouts Canônicos
- DEVE usar `--retries 2` como padrão — 3 apenas em rede instável.
- DEVE usar `--timeout 20` por requisição HTTP individual.
- DEVE usar `--global-timeout 60` para query única, 300 para batch.
- JAMAIS use `--retries` acima de 10 — trigger garantido de anti-bot.

## Receitas de Referência Rápida
- Apenas URLs: `| jaq -r '.resultados[].url'`.
- Apenas títulos: `| jaq -r '.resultados[].titulo'`.
- Top N resultados: `| jaq '.resultados[:5]'`.
- Filtrar por domínio: `| jaq '.resultados[] | select(.url | contains("github.com"))'`.
- Contagem: `| jaq '.quantidade_resultados'`.
- Latência: `| jaq '.metadados.tempo_execucao_ms'`.
- Identidade usada: `| jaq -r '.metadados.identidade_usada // "n/a"'` (v0.6.5+)
- Nível de cascata: `| jaq '.metadados.nivel_cascata // 0'` (v0.6.5+)
- Probe de saúde (v0.6.4+): `timeout 15 duckduckgo-search-cli --probe`.
- Crawl longo com circuit breaker (v0.6.5+): combine `--queries-file` com `--parallel 5 --retries 2 --global-timeout 580`.
- Install cross-platform (v0.6.5+): `cargo install duckduckgo-search-cli --version 0.6.5 --force` funciona em Linux, macOS e Windows.
- Barra de progresso em arquivo (v0.6.5+): redirecione stderr para arquivo de log com `2> /tmp/progress.log` para manter o stdout JSON limpo.

## v0.6.4/v0.6.5 — Pool Adaptativo de Identidades Anti-Bot (WS-26)

> **Nota**: v0.6.4 foi publicada originalmente no lugar da planejada v0.7.0; v0.6.5 (2026-06-05) adicionou MP-26/WS-11/12/23/25/CI-01 para preservar o conjunto de features em desenvolvimento sob um número de patch estável. v0.7.0 (lançada em 2026-06-07) substitui ambas com o novo subcomando `deep-research` e quatro novos módulos públicos. Zero breaking changes em relação à v0.6.5.

### Verificação Pré-Voo Obrigatória
- DEVE executar `duckduckgo-search-cli --probe` em CI antes de lançar queries reais — envia 1 requisição mínima, exit 0 se acessível, 1 se bloqueado.
- DEVE inspecionar `.metadados.nivel_cascata` após exit 3 — a cascata já rotacionou até 5 identidades. Se `nivel_cascata == 4`, o próprio IP está esgotado.


## v0.6.5 — Gaps Resolvidos (Seção Dedicada)

v0.6.5 (lançada em 2026-06-05) fecha 7 gaps herdados da v0.6.4. As
seções abaixo são leitura OBRIGATÓRIA para qualquer agente que invoca
a CLI no Windows ou em crawls longos.

### MP-26 — Correção de Type-Safety do HANDLE no Windows

**Problema resolvido em v0.6.5**: o binário v0.6.4 não compilava no
Windows. O tipo `HANDLE` mudou de `isize` (windows-sys 0.52) para
`*mut c_void` (windows-sys 0.59), quebrando 4 erros E0308 mismatched-type
em `src/platform.rs`.

**O que isso significa para os agentes**:
- O mesmo comando `cargo install duckduckgo-search-cli --version 0.6.5 --force`
  agora funciona em Linux, macOS E Windows.
- O binário Windows usa a sentinela `INVALID_HANDLE_VALUE` de
  `windows_sys::Win32::Foundation` (NÃO comparação mágica com `usize::MAX`).
- O bloco `unsafe` possui documentação SAFETY completa descrevendo
  checagens de nulidade e sentinela.
- Lints `improper_ctypes` e `improper_ctypes_definitions` são `deny`
  no `Cargo.toml` — drift futuro de tipo FFI é bloqueado em compile-time.

**Receita do agente — Verificar install no Windows**:
```bash
# Após cargo install no Windows (PowerShell 5.1+ ou 7+)
duckduckgo-search-cli --version
# Esperado: duckduckgo-search-cli 0.6.5
duckduckgo-search-cli --help
# Esperado: texto de help completo em stderr, exit 0
```

### WS-12 — Circuit Breaker Per-Host

**Problema resolvido em v0.6.5**: crawls longos (>50 páginas)
travavam re-tentando hosts com falha. Após 3 falhas consecutivas em
um único host, o crawl ficava em loop infinito consumindo todo o
`--global-timeout`.

**O que isso significa para os agentes**:
- A CLI abre um circuit breaker per host após 3 falhas consecutivas.
- O breaker fica aberto por 30 segundos — requisições para esse host
  são curto-circuitadas sem consumir recursos de rede.
- Um único sucesso reseta o contador de falhas.
- O estado half-open é alcançável após a janela de cooldown.
- Cada invocação da CLI tem um breaker fresh (sem estado compartilhado
  entre invocações).

**Receita do agente — Crawl longo com circuit breaker**:
```bash
# 100 páginas, 5 em paralelo, com circuit breaker automático
timeout 600 duckduckgo-search-cli \
  --queries-file /tmp/100-queries.txt \
  -q -f json --parallel 5 --per-host-limit 1 \
  --fetch-content --max-content-length 10000 \
  --retries 2 --timeout 30 \
  --global-timeout 580 > /tmp/long-crawl.json

# Se um host falhar 3x, requisições para ele são curto-circuitadas por 30s.
# Outros hosts continuam a ser raspados em paralelo.
# Wall time reduzido de "travado re-tentando" para "segue em frente".
```

**Interação com --parallel**:
- O circuit breaker é per-host, independente de `--parallel`.
- `--parallel 5` significa 5 requisições concorrentes entre hosts distintos.
- Se 3 dessas 5 falharem no mesmo host, esse host entra em cooldown.
- Os 2 hosts restantes continuam normalmente.
- Após 30s, o host em cooldown é re-avaliado (estado half-open).

### WS-25 — ProgressBar indicatif para Crawls Longos

**Problema resolvido em v0.6.5**: crawls longos (>10 URLs com
`--fetch-content`) não davam feedback visual. Usuários não sabiam
se o crawl estava progredindo ou travado.

**O que isso significa para os agentes**:
- A CLI exibe uma barra de progresso em stderr para qualquer crawl
  com `--fetch-content` e >5 URLs.
- O formato da barra é
  `[{elapsed_precise}] {bar:40.cyan/blue} {pos:>4}/{len:4} {msg}`.
- A barra avança por task completada.
- A barra é limpa ao terminar (`finish_and_clear`) para não poluir
  consumidores downstream de stderr.
- A barra NUNCA é escrita no stdout — output JSON permanece limpo.

**Receita do agente — Crawl longo com visibilidade de progresso**:
```bash
# stderr mostra a barra de progresso; stdout mostra o JSON
timeout 300 duckduckgo-search-cli \
  --queries-file /tmp/50-queries.txt \
  -q -f json --fetch-content --max-content-length 5000 \
  --parallel 3 --global-timeout 280 \
  --output /tmp/results.json 2> /tmp/progress.log
# /tmp/results.json contém o payload JSON
# /tmp/progress.log contém os eventos da barra de progresso
```

### WS-11 — Testes Property-Based para Parser HTML

**Problema resolvido em v0.6.5**: a migração v0.6.3 → v0.6.4 quebrou
o parser HTML para inputs vazios e HTML malformado. A release v0.6.4
não tinha cobertura de teste de regressão para invariantes do parser.

**O que isso significa para os agentes**:
- 5 property tests em `src/extraction.rs` validam que o parser nunca
  causa panic em HTML malformado, retorna `Vec` vazio para inputs
  vazios, emite posições densas e 1-based, normaliza URLs para paths
  absolutos, e é determinístico.
- Agentes podem confiar que HTML malformado da natureza não quebra
  a CLI.

### WS-23 — Header Retry-After Respeitado

**Problema resolvido em v0.6.5**: respostas HTTP 429 com header
`Retry-After` não eram honradas — a CLI re-tentava imediatamente,
disparando a cascata anti-bot.

**O que isso significa para os agentes**:
- A CLI respeita o header `Retry-After` em segundos.
- Um teste wiremock em `tests/integration_wiremock.rs` valida que
  o delay de backoff é pelo menos `Retry-After` segundos, com 500ms
  de margem para overhead do scheduler CI.
- Agentes não precisam implementar sua própria lógica de
  `Retry-After`.

### CI-01 — 6 Erros Latentes de Clippy Corrigidos

**Problema resolvido em v0.6.5**: v0.6.4 foi publicada com 6 erros
de clippy que falhavam o CI nos 3 SOs (Linux, macOS, Windows). Os
erros eram:
- `clippy::doc_markdown` em `PowerShell`, `rules_rust.md`, `TempDir`
- `clippy::needless_return` em browser.rs:149
- `missing_debug_implementations` em `ChromeBrowser`
- `missing_debug_implementations` em `CircuitBreakerMap`

**O que isso significa para os agentes**:
- `cargo clippy --all-targets --all-features -- -D warnings` passa.
- CI matrix retorna success em todos os 3 SOs.
- 333 testes passam (243 lib + 90 integration + 6 doc tests).
- Lints `improper_ctypes`, `missing_safety_doc` e
  `unsafe_op_in_unsafe_fn` agora são `deny` para prevenir regressões.

### Novas Flags CLI (v0.6.4+, preservadas em v0.6.5)
- `--probe` — verificação de saúde pré-voo, 1 requisição mínima, relatório JSON.
- `--identity-profile <nome>` — fixa uma identidade do pool de 12. Padrão `auto` rotaciona adaptativamente. Nomes válidos: `auto`, `chrome-win`, `chrome-mac`, `chrome-linux`, `edge-win`, `firefox-linux`, `safari-mac`.
- `--seed <u64>` — seed determinístico para seleção de UA E rotação do pool de identidades. Use para debug reproduzível.

### Estratégia de Cascata (5 Níveis)

```
Nível 0 — Mesma identidade (sem rotação)
  ↓ (HTTP 202/403/429)
Nível 1 — Mesma família, plataforma diferente
  ↓ (ainda bloqueado)
Nível 2 — Família diferente, mesma plataforma
  ↓ (ainda bloqueado)
Nível 3 — Família e plataforma diferentes + endpoint rebaixado para lite
  ↓ (ainda bloqueado)
Nível 4 — Identidade aleatória (caller deve aguardar 30-60s antes de retentar)
  ↓ (ainda bloqueado)
FALHA — Reportar com causa específica + retry_after_seconds recomendado
```

### Receitas Anti-Bot v0.6.4+ (v0.6.5)
```bash
# Verificação de saúde pré-voo antes de queries reais
timeout 15 duckduckgo-search-cli --probe && \
  timeout 30 duckduckgo-search-cli "consulta" -q -f json --num 15

# Fixa uma identidade específica para testes reproduzíveis
timeout 30 duckduckgo-search-cli "consulta" -q -f json --num 15 --identity-profile chrome-linux

# Diagnostica qual identidade produziu a resposta
timeout 30 duckduckgo-search-cli "consulta" -q -f json --num 15 | \
  jaq -r '.metadados.identidade_usada // "n/a"'

# Detecta esgotamento de cascata em logs de CI
timeout 30 duckduckgo-search-cli "consulta" -q -f json --num 15 | \
  jaq '.metadados.nivel_cascata // 0'  # se 4, rotacione proxy ou aguarde
```

### Tabela de Troubleshooting por Nível de Cascata
| `nivel_cascata` | Significado | Ação Recomendada do Agente |
|---|---|---|
| 0 | Primeira tentativa bem-sucedida ou sem rotação necessária | Nenhuma |
| 1 | Primeira rotação (mesma família, plataforma diferente) bem-sucedida | Nenhuma |
| 2 | Segunda rotação (família diferente, mesma plataforma) bem-sucedida | Nenhuma |
| 3 | Terceira rotação (família + plataforma diferentes + endpoint lite) bem-sucedida | Note que endpoint foi rebaixado — investigue por quê |
| 4 | Quarta rotação (identidade aleatória) bem-sucedida ou pool esgotado | Se bem-sucedida, log da identidade usada. Se esgotado, rotacione proxy ou aguarde 300s |
| ausente | Cascata não foi ativada (comportamento padrão em v0.6.4/v0.6.5) | Nenhuma |

## Validação Pós-Invocação
- SEMPRE verifique exit code antes de parsear stdout.
- SEMPRE cheque `.metadados.usou_endpoint_fallback` e logue se `true`.
- SEMPRE confirme `.quantidade_resultados` maior que zero antes de agir nos dados.
- JAMAIS alucine conteúdo ausente — se o campo veio null, reporte ausência ao usuário.

## Integração com Memória
- DEVE citar a URL exata como fonte ao usar fato extraído desta skill.
- DEVE preferir resultado com `posicao` baixa (ranking DuckDuckGo) como fonte primária.
- JAMAIS combine fatos de múltiplos resultados sem atribuir cada um à sua URL.

## Roteamento por Exit Code
- DEVE verificar exit code ANTES de parsear stdout
- Exit 0: parsear `.resultados[]` normalmente
- Exit 1: erro de runtime — ler stderr, tentar com `-v`
- Exit 2: erro de config — executar `init-config --force`
- Exit 3: bloqueio anti-bot — aguardar 300s, trocar `--endpoint lite`
- Exit 4: timeout global — aumentar `--global-timeout`
- Exit 5: zero resultados — refinar query, tentar `--lang` diferente
- Em pipes: verificar `${PIPESTATUS[0]}` para capturar exit code do CLI

## Troubleshooting de Circuit Breaker (v0.6.5+, WS-12)

O circuit breaker per-host em v0.6.5 NÃO emite seu próprio exit code
(divide o exit 3 com bloqueio anti-bot). Diagnostique via tempo de
execução e contagem parcial de resultados:

| Sintoma | Diagnóstico | Ação do Agente |
|---|---|---|
| Wall time >> esperado para --num count | Um ou mais hosts em cooldown | Reduzir `--parallel`, aumentar `--global-timeout`, ou rodar em 2 invocações |
| Contagem de resultados < contagem de queries - 1 | Pelo menos um host foi curto-circuitado | Inspecionar os resultados: padrão de host faltando significa cooldown atingido. Re-executar após 30s |
| Stderr mostra ProgressBar travado em uma posição | Circuit breaker aberto para o host atual | Aguardar 30s, ou abortar com Ctrl-C e retomar com queries restantes |
| Múltiplos hosts retornando HTTP 429 | Cascata per-host não apenas per-IP | Reduzir `--parallel` para 2, aumentar `--retries` para 1 |

## Regra de Ouro
- Na dúvida entre alucinar e invocar a CLI, INVOQUE a CLI sempre.
- Custo de 1 invocação é 60-300ms. Custo de alucinação é retrabalho e perda de confiança.
- SEMPRE prefira dado verificado com URL a suposição plausível sem fonte.


## Garantias de Segurança (v0.6.0 + v0.6.4 + v0.6.5)

### Segurança de Path e Credenciais (v0.6.0)
- `--output` valida paths ANTES de escrever — `..` e diretórios de sistema rejeitados automaticamente
- Credenciais de proxy em URLs `--proxy` JAMAIS aparecem em mensagens de erro ou stderr
- Mascaramento transforma `http://user:pass@host` em `http://us***@host` em toda saída de erro
- Agentes geram nomes de arquivo dinâmicos sem validação manual — o CLI rejeita paths inseguros
- SIGPIPE restaurado no Unix — pipes para `jaq`, `head`, `wc` terminam limpos sem erros EPIPE
- BrokenPipe detectado na cadeia de erros — retorna exit 0 em vez de propagar como exit 1
- Erros tipados via enum `ErroCliDdg` — 11 variantes com mapeamento determinístico de `exit_code()`

### Anti-Bloqueio (v0.6.0 + v0.6.4)
- v0.6.0: `BrowserProfile` injeta headers `Sec-Fetch-*` por família e Client Hints — NUNCA adicione headers duplicados
- v0.6.0: Detecção de HTTP 202 anomaly com backoff exponencial roda automaticamente — confie no exit code 3, não faça retry próprio
- v0.6.0: Detecção de bloqueio silencioso — respostas abaixo de 5 KB são tratadas como bloqueios, não como sucesso
- v0.6.4: Pool adaptativo de 12 identidades anti-bot (WS-26) — 4 famílias de browser × 3 plataformas com rotação em cascata de 5 níveis
- v0.6.4: `--probe` para verificações de saúde pré-voo em CI antes de lançar queries reais
- v0.6.4: `--identity-profile` e `--seed` dão controle determinístico sobre o pool adaptativo
- v0.6.4: `metadados.identidade_usada` e `metadados.nivel_cascata` dão visibilidade diagnóstica — use `// "n/a"` e `// 0` como fallbacks respectivamente


## Workflow
- Passo 1 — invocar a busca: `duckduckgo-search-cli -f json -n 10 "consulta"`
- Passo 2 — capturar o exit code: verificar `$?` imediatamente após o comando
- Passo 3 — parsear resultados JSON com jaq: `jaq -r '.resultados[] | .titulo + " " + .url'`
- Passo 4 — filtrar campos relevantes: `jaq '.resultados[] | {titulo: .titulo, url: .url, snippet: .snippet}'`
- Passo 5 — retornar resultados estruturados ao LLM como contexto para raciocínio posterior


## v0.7.0 — Subcomando Deep Research

Para perguntas de pesquisa multi-hop, o subcomando `deep-research` faz fan-out de uma query em até 12 sub-queries, agrega os resultados e opcionalmente sintetiza um relatório em Markdown.

```bash
# 1. Fan-out heurístico padrão (5 sub-queries, agregação RRF, sem síntese).
timeout 60 duckduckgo-search-cli -q -f json deep-research "melhor cliente http rust 2026" \
  | jaq '.resultados[] | {titulo, url, score}'

# 2. Relatório Markdown com orçamento de tokens.
timeout 120 duckduckgo-search-cli -q -f json deep-research "tokio vs async-std 2026" \
  --synthesize --synth-format markdown --budget-tokens 1500 \
  | jaq -r '.sintese'

# 3. Sub-queries manuais de arquivo (comentários `#` e linhas vazias ignorados).
cat > /tmp/qs.txt <<EOF
# Visão geral
o que é tokio runtime 2026
# Comparação
tokio vs async-std
EOF
timeout 60 duckduckgo-search-cli -q -f json deep-research "tokio 2026" \
  --sub-queries-file /tmp/qs.txt --aggregate dedupe-by-url
```

### Schema de saída do Deep Research (v0.7.0+)
- `.metadados.query_original` — entrada do usuário
- `.metadados.sub_queries[]` — cada sub-query gerada com `texto`, `estrategia`, `status`, `elapsed_ms`
- `.metadados.total_resultados_unicos` — contagem deduplicada
- `.metadados.tempo_total_ms` — latência end-to-end
- `.resultados[].score` — normalizado em `[0.0, 1.0]`, maior é melhor
- `.resultados[].fontes[]` — sub-queries que produziram o resultado (rastreabilidade)
- `.sintese` — presente apenas quando `--synthesize` está ativo

O subcomando herda toda flag global (`-q -f json`, `--num`, `--lang`, `--country`, `--parallel`, `--endpoint`, `--proxy`, `--retries`, `--global-timeout`, `--fetch-content`, `--max-content-length`) e adiciona:

- `--max-sub-queries N` — teto do fan-out (1..=12, padrão 5)
- `--sub-query-strategy` — `heuristic` (padrão) ou `manual`
- `--sub-queries-file PATH` — obrigatório para `manual`; comentários e linhas vazias são ignorados
- `--aggregate` — `rrf` (padrão, K=60) ou `dedupe-by-url`
- `--synthesize` — produz o relatório final
- `--budget-tokens N` — teto de tamanho da síntese (1 token ≈ 4 chars)
- `--synth-format` — `markdown` (padrão), `plain` ou `json`

### Templates Heurísticos (5 — fan-out embutido)
A estratégia `--sub-query-strategy heuristic` (padrão) aplica 5 templates canônicos à query do usuário:
- `aspect` — explora dimensões distintas do tópico
- `comparison` — expõe alternativas (pulado quando a query já contém `vs` ou `or`)
- `timeline` — ordena resultados por recência e evolução
- `opinion` — expõe opiniões, reviews e experiências
- `cause` — expõe causas, consequências e raízes

Quando a query do usuário é detectada como composta via `is_composite_query` (regex-backed, 6 tipos de sinais), templates redundantes são suprimidos. Resultado: o fan-out produz 1..=12 sub-queries (limitado por `--max-sub-queries`).

### Defaults do Pipeline
`run_deep_research` constrói um `Config` padrão a partir das flags globais: `parallelism=5`, `retries=2`, `endpoint=Html`, `language=en`, `country=us`, `global_timeout=120s`. O pipeline herda esses defaults; o operador NÃO precisa passar um `CliArgs` completo.

### Semântica de `--depth`
`--depth N` controla rodadas de reflexão (0..=3, padrão 0). Quando `depth > 0`, o pipeline PLANÉJA sub-queries de follow-up com base na primeira passada mas NÃO AS EXECUTA na v0.7.0. Use `--depth 0` para forçar execução end-to-end.

### Cross-Reference: RRF (K=60)
`--aggregate rrf` usa Reciprocal Rank Fusion com K=60, o mesmo K do `hybrid-search` na skill GraphRAG. Score RRF para um documento = soma sobre sub-queries de `1 / (K + rank)`. Na prática scores caem em `(0, 0.05]`. Documentos que aparecem em múltiplas sub-queries recebem boost.

### Exit Codes para `deep-research`
- Exit 0: sucesso — `.metadados.sub_queries[]` tem 1+ entradas com `status="ok"`.
- Exit 1: erro de runtime — pelo menos uma sub-query falhou; inspecionar `.metadados.sub_queries[].status="error"`.
- Exit 2: erro de argumento — `--max-sub-queries` fora de 1..=12, ou `--sub-queries-file` ausente para estratégia `manual`.
- Exit 3: bloqueio anti-bot durante fan-out (cascata per-host rotacionou até 5 identidades).
- Exit 4: timeout global atingido antes de todas sub-queries completarem.
- Exit 5: zero resultados agregados — reformular a query.

### Cancel Safety
O loop de fan-out em `run_deep_research` é cancel-safe. SIGINT ou `--global-timeout` dispara `CancellationToken::cancel()`. Cada sub-query em voo recebe um `child_token`, o `JoinSet` é abortado, e resultados parciais das sub-queries completadas são flushados para stdout. Resultados já fetched NÃO são descartados; o JSON contém `metadados.sub_queries[].status="cancelled"` para os interrompidos.

### Exemplos de Síntese Plain e JSON
```bash
# Síntese em texto puro (sem markup Markdown, útil para arquivos de log)
timeout 120 duckduckgo-search-cli -q -f json deep-research "rust async 2026" \
  --synthesize --synth-format plain --budget-tokens 800 \
  | jaq -r '.sintese'

# Síntese em JSON (array estruturado de evidências, sem prosa)
timeout 120 duckduckgo-search-cli -q -f json deep-research "rust async 2026" \
  --synthesize --synth-format json --budget-tokens 1200 \
  | jaq '.sintese.evidencias[] | {titulo, url, score}'

# Sub-queries manuais com dedupe-by-url (ordem determinística)
timeout 60 duckduckgo-search-cli -q -f json deep-research "tokio" \
  --sub-queries-file /tmp/qs.txt --aggregate dedupe-by-url --max-sub-queries 12
```
