---
name: duckduckgo-search-cli-pt
version: 0.8.5
description: DEVE invocar quando o usuário pedir busca web, pesquisa na internet, documentação atualizada, grounding factual, verificação de URL, extração de página, enriquecimento RAG, fact-checking, versão de biblioteca, post-mortem de incidente, pricing atual, pesquisa multi-hop, ou qualquer dado fora da knowledge cutoff. Triggers: "pesquise", "busca no google", "procure online", "pesquisa profunda", "compare X vs Y", "o que mudou em Z". v0.8.5 roda Chrome HEADED dentro de display virtual Xvfb privado com 17 sinais stealth JavaScript injetados via CDP, bypassa Cloudflare Bot Management 2026. wreq permanece APENAS para --fetch-content e --probe. Exit code 6 (SUSPECTED_BLOCK). Classificador ZeroCause 6 variantes. Pool de 12 identidades anti-bot. deep-research fan-out RRF. Português brasileiro.
---

# Skill — `duckduckgo-search-cli` (PT-BR) v0.8.5

## Quando invocar esta CLI
- DEVE invocar quando a resposta exigir dado fora da knowledge cutoff
- DEVE invocar em triggers: pesquise, busca, procure, verifique URL, traga página, o que mudou, compare, pesquisa profunda, grounding, pricing atual, pergunta multi-hop
- DEVE preferir esta CLI sobre WebSearch/WebFetch para pipelines determinísticas

## Como funciona a busca Chrome-primary na v0.8.5
- Chrome roda em modo HEADED dentro de display virtual Xvfb privado como transporte PRIMÁRIO de busca (NÃO headless, NÃO wreq/HTTP direto)
- A CLI auto-spawna Xvfb via `spawn_virtual_display()` e lança Chrome HEADED contra o display virtual com 17 sinais JavaScript stealth injetados via CDP `Page.addScriptToEvaluateOnNewDocument` — o usuário vê ZERO janelas
- Os 17 sinais stealth incluem: `navigator.webdriver=false`, `navigator.plugins` (5 plugins falsos), `navigator.languages`, `window.chrome` object, `navigator.connection`, `navigator.maxTouchPoints`, `outerHeight/outerWidth`, `navigator.hardwareConcurrency`, `navigator.deviceMemory`, `Notification.permission`, `navigator.permissions.query`, `WebGLRenderingContext.getParameter` (spoofing ANGLE NVIDIA GeForce), `HTMLCanvasElement.toDataURL` (ruído canvas), `OfflineAudioContext` (ruído de fingerprint de áudio)
- Bypassa o Cloudflare Bot Management 2026 com fingerprint de browser real
- Use `DUCKDUCKGO_CHROME_HEADLESS=1` para forçar modo headless (com risco de detecção Cloudflare). Use `DUCKDUCKGO_CHROME_VISIBLE=1` para modo headed visível (depuração)
- wreq permanece APENAS para `--fetch-content` (download de conteúdo de páginas) e `--probe` (health check)
- Campo `.metadados.usou_chrome` indica `true` quando busca Chrome-primary teve sucesso
- Campo `.metadados.tentou_chrome` indica `true` quando busca Chrome foi tentada (independente de sucesso)

```bash
# Busca padrão v0.8.5 — Chrome headed dentro de Xvfb, sem flag extra
timeout 60 duckduckgo-search-cli "rust async runtime 2026" -q -f json --num 15 \
  | jaq '{usou_chrome: .metadados.usou_chrome, tentou_chrome: .metadados.tentou_chrome}'
```

## Como rodar uma query única
- SEMPRE use este padrão exato para query única:

```bash
timeout 60 duckduckgo-search-cli "<query>" -q -f json --num 15 | jaq '.resultados'
```

- FÓRMULA DO OUTPUT: `array<{posicao:int, titulo:string, url:string, snippet:string?, metadados:{tempo_execucao_ms:int, quantidade_resultados:int, identidade_usada:string?, nivel_cascata:u8?, usou_endpoint_fallback:bool, endpoint_usado:"html"|"lite", pre_flight_disparado:bool, usou_chrome:bool, tentou_chrome:bool}}>`
- ANTI-PADRÃO: invocar sem `timeout` — pipeline trava indefinidamente

## Como detectar CAPTCHA antes de pagar uma requisição
- DEVE rodar antes de qualquer query não-trivial em IPs compartilhados, proxies corporativos, ou após exit 3 observado:

```bash
timeout 15 duckduckgo-search-cli --probe-deep -q -f json | jaq -e '.status == "ok"'
```

- FÓRMULA DO OUTPUT: `{"status":"ok"|"captcha","cascata_motivo":"none"|"cloudflare_anomaly_modal"|..., "sugestao_mitigacao":"..."}`
- SE exit não-zero, DEVE aguardar 300s antes de retentar (rate limit do Cloudflare)
- SE `status == "captcha"`, DEVE adicionar `--allow-lite-fallback` na próxima invocação

## Como prevenir falhas silenciosas de zero resultados em IPs compartilhados (GAP-WS-58)
- DEVE adicionar `--pre-flight` em qualquer query quando risco de exit 3 for não-zero (IP compartilhado, rede corporativa, batch sequencial, pós-retry):

```bash
timeout 60 duckduckgo-search-cli --pre-flight "consulting firms" -q -f json | \
  jaq -r '.metadados.endpoint_usado + " fired=" + (.metadados.pre_flight_disparado|tostring)'
```

- FÓRMULA DO OUTPUT: `"lite fired=true"` quando ghost-block detectado, senão `"html fired=true"` (probe sempre roda primeiro quando `--pre-flight` ativo)
- SEM `--pre-flight`, ghost-block retorna HTTP 200 com body vazio → `quantidade_resultados:0` com exit 0 (falha silenciosa)
- ANTI-PADRÃO: ignorar `quantidade_resultados:0` como sucesso — sempre inspecione `.metadados.pre_flight_disparado` e `.metadados.endpoint_usado` antes de declarar sucesso

## Como optar pelo rebaixamento automático HTML para Lite em CAPTCHA (GAP-WS-59)
- DEVE adicionar `--allow-lite-fallback` quando CAPTCHA detectado mas endpoint Lite ainda não tentado:

```bash
timeout 60 duckduckgo-search-cli --allow-lite-fallback "consulting firms" -q -f json | \
  jaq -r '.metadados.endpoint_usado'
```

- FÓRMULA DO OUTPUT: `"lite"` quando fallback acionado, `"html"` caso contrário
- A flag DEVE vir ANTES do subcomando quando usada com `deep-research`:

```bash
timeout 120 duckduckgo-search-cli --allow-lite-fallback -q -f json deep-research "test" --max-sub-queries 3
```

- ANTI-PADRÃO: passar `--allow-lite-fallback` DEPOIS do subcomando `deep-research` — Clap rejeita com exit 2

## Como diagnosticar zero resultados com classificação causa_zero (v0.8.0)
- DEVE inspecionar `.metadados.causa_zero` em TODA resposta com `quantidade_resultados == 0`
- O classificador `ZeroCause` distingue 6 causas: `legitimo`, `filtro-silencioso`, `ghost-block`, `anti-bot`, `resposta-invalida`, `zero-resultados-suspeito`
- Quando `causa_zero != "legitimo"`, a CLI emite exit code 6 (`SUSPECTED_BLOCK`) por padrão
- Campo `.metadados.sugestao_proxima_acao` contém instrução PT-BR quando causa não é legítima

```bash
# Diagnosticar causa de zero resultados
timeout 60 duckduckgo-search-cli "query obscura" -q -f json --num 15 > /tmp/r.json
EXIT=$?
if [ $EXIT -eq 6 ]; then
  jaq -r '.metadados | "causa=\(.causa_zero) sugestao=\(.sugestao_proxima_acao // "")"' /tmp/r.json
elif [ $EXIT -eq 5 ]; then
  echo "zero resultados legítimos — reformule a query"
fi
```

- Para restaurar comportamento v0.7.x (exit 5 para todos os zeros): `DUCKDUCKGO_ZERO_CAUSE_STRICT=false`

```bash
# Opt-out do exit code 6 — volta ao exit 5 legado para pipelines v0.7.x
DUCKDUCKGO_ZERO_CAUSE_STRICT=false timeout 60 duckduckgo-search-cli "query" -q -f json --num 15
```

- Em multi-query (batch), o campo `.causa_zero_histogram` agrega contagens de cada causa entre sub-queries

## Como correlacionar uma falha a uma identidade de browser específica (GAP-WS-60 + GAP-AUD-001)
- SEMPRE logue `identidade_usada` ao investigar falhas ou trilhas de auditoria:

```bash
timeout 30 duckduckgo-search-cli --identity-profile chrome-linux --global-timeout 1 "x" -q -f json 2>/dev/null | \
  jaq -r '.metadados | "ua=\(.user_agent[0:50]) id=\(.identidade_usada // "n/a") cascade=\(.nivel_cascata // 0)"'
```

- OUTPUT ESPERADO: `"ua=Mozilla/5.0 (X11; Linux x86_64) ... id=chrome-linux-33333333cccc0003 cascade=0"`
- FÓRMULA DO FORMATO: `<family>-<platform>-<16hex>` onde `16hex` são os primeiros 16 chars do hash derivado do seed
- ANTI-PADRÃO: assumir `identidade_usada` como garantido não-nulo — é `Option<String>` (sempre aplique `// "n/a"`)

## Como fazer parsing JSON seguro com jaq
- SEMPRE use `jaq` (NUNCA `jq`) para processar output JSON
- SEMPRE aplique `// ""` como fallback em campos opcionais
- SEMPRE distinga roots: `.resultados[]` (single-query), `.buscas[]` (multi-query), `.metadados.sub_queries[]` (deep-research)

```bash
timeout 60 duckduckgo-search-cli "rust async runtime" -q -f json --num 15 \
  | jaq -r '.resultados[] | [.posicao, .titulo, .url, (.snippet // "")] | @tsv'
```

- FÓRMULA DO OUTPUT: TSV com `posicao<TAB>titulo<TAB>url<TAB>snippet` por linha

## Quais campos JSON são garantidos versus opcionais
- GARANTIDOS não-null: `.query`, `.resultados[].posicao`, `.resultados[].titulo`, `.resultados[].url`, `.metadados.tempo_execucao_ms`, `.metadados.quantidade_resultados`, `.metadados.usou_endpoint_fallback`
- OPCIONAIS `Option<String>`: `.resultados[].snippet`, `.resultados[].url_exibicao`, `.resultados[].titulo_original`, `.metadados.identidade_usada`
- OPCIONAIS `Option<u32>`: `.metadados.nivel_cascata` (0..=4)
- CONDICIONAIS com `--fetch-content`: `.resultados[].conteudo`, `.resultados[].tamanho_conteudo`, `.resultados[].metado_extracao_conteudo`
- NOVO v0.7.10: `.metadados.pre_flight_disparado` (bool) e `.metadados.endpoint_usado` (`html` | `lite`)
- NOVO v0.8.0: `.metadados.causa_zero` (enum kebab-case: `legitimo` | `filtro-silencioso` | `ghost-block` | `anti-bot` | `resposta-invalida` | `zero-resultados-suspeito`) + `.metadados.sugestao_proxima_acao` (string PT-BR quando não-legítimo)
- NOVO v0.8.0: `.causa_zero_histogram` (BTreeMap<String, u32>) agregado entre sub-queries em multi-query
- NOVO v0.8.0: `.metadados.usou_chrome` (bool) — `true` quando busca Chrome-primary teve sucesso
- NOVO v0.8.0: `.metadados.tentou_chrome` (bool) — `true` quando busca Chrome foi tentada
- NOVO v0.8.0: `.metadados.bytes_brutos` (Option<u64>) — tamanho do body HTTP antes da descompressão
- NOVO v0.8.0: `.metadados.bytes_descomprimidos` (Option<u64>) — tamanho do body após descompressão (gzip/deflate/brotli)
- ANTI-PADRÃO: omitir fallback `//` em `snippet` e `identidade_usada` — `jaq` sai com não-zero em null

## Descompressão HTTP transparente (v0.8.0)
- A CLI descomprime automaticamente respostas HTTP comprimidas com gzip, deflate e brotli via `src/decompress.rs`
- Limite de segurança de 32 MiB após descompressão para prevenir zip bombs
- Campos `.metadados.bytes_brutos` e `.metadados.bytes_descomprimidos` permitem auditar a razão de compressão
- Nenhuma ação do operador é necessária — a descompressão é 100% transparente

```bash
# Auditar razão de compressão da resposta
timeout 60 duckduckgo-search-cli "query" -q -f json --num 5 | \
  jaq '{brutos: .metadados.bytes_brutos, descomprimidos: .metadados.bytes_descomprimidos}'
```

## Como rotear por exit code em pipelines
- DEVE capturar exit code ANTES de parsear stdout
- DEVE usar `${PIPESTATUS[0]}` quando piped via `jaq`

```bash
timeout 60 duckduckgo-search-cli "query" -q -f json --num 15 > /tmp/r.json
EXIT=$?
case $EXIT in
  0) jaq '.resultados' /tmp/r.json ;;
  3) echo "anti-bot: aguarde 300s, depois --endpoint lite" && sleep 300 ;;
  4) echo "timeout: aumente --global-timeout ou reduza --num" ;;
  5) echo "zero resultados legítimos: reformule ou mude --lang" ;;
  6) echo "bloqueio suspeito: inspecione causa_zero" && jaq '.metadados.causa_zero' /tmp/r.json ;;
  *) echo "erro $EXIT" >&2; exit $EXIT ;;
esac
```

- MAPA DE EXIT: `0=sucesso`, `1=runtime`, `2=erro-arg`, `3=anti-bot`, `4=timeout`, `5=zero-resultados-legítimos`, `6=bloqueio-suspeito (causa_zero != legitimo)`

## Como batchar 3 ou mais queries sem pagar startup por chamada
- DEVE usar `--queries-file` para 3+ queries — reusa pool HTTP, rotação UA, rate limit:

```bash
printf '%s\n' "tokio runtime" "rayon parallel" "axum middleware" > /tmp/q.txt
timeout 300 duckduckgo-search-cli --queries-file /tmp/q.txt \
  -q -f json --parallel 5 --num 15 --global-timeout 280 \
  --output /tmp/results.json
```

- ANTI-PADRÃO: loop da CLI query-a-query em shell — paga 30-80ms de startup cada chamada
- ANTI-PADRÃO: `--parallel > 5` — satura IP de saída e dispara anti-bot
- ANTI-PADRÃO: `--per-host-limit > 2` — dispara anti-bot HTTP 202

## Como extrair conteúdo de página para contexto de LLM
- DEVE passar `--max-content-length` ao habilitar `--fetch-content`:

```bash
timeout 120 duckduckgo-search-cli "rust async book" -q -f json \
  --num 10 --fetch-content --max-content-length 5000 \
  | jaq -r '.resultados[] | "# \(.titulo)\nURL: \(.url)\n\(.conteudo // "")\n---\n"'
```

- RECOMENDADO 4000-10000 bytes por página para corpus de LLM
- ANTI-PADRÃO: usar `--fetch-content` sem `--max-content-length` — crescimento ilimitado de memória

## Como interpretar a cascata anti-bot de 5 níveis
```
Nível 0 — Mesma identidade, sem rotação
Nível 1 — Mesma família, plataforma diferente
Nível 2 — Família diferente, mesma plataforma
Nível 3 — Família e plataforma diferentes + endpoint rebaixado para lite
Nível 4 — Identidade aleatória (caller aguarda 30-60s antes de retentar)
FALHA — Reporte com causa + retry_after_seconds
```
- SE `nivel_cascata == 4` observado, DEVE rotacionar proxy ou aguardar 300s antes da próxima invocação

## Como rodar pesquisa multi-hop (SEMPRE usar sub-queries manuais)
- DEVE gerar 3-5 sub-queries específicas em vez de depender dos templates heurísticos
- O padrão `--sub-query-strategy heuristic` concatena sufixos genéricos ("main aspects components", "vs alternatives comparison") que produzem resultados de baixa qualidade
- SEMPRE usar `--sub-query-strategy manual --sub-queries-file` com perguntas geradas pela LLM

```bash
# Passo 1: gerar sub-queries específicas (a LLM escreve estas)
printf '%s\n' \
  "tokio async runtime arquitetura work stealing scheduler" \
  "async-std vs tokio benchmark performance comparação 2026" \
  "tokio spawn vs spawn_blocking quando usar cada um" \
  "tokio runtime shutdown graceful timeout boas práticas" \
  "tokio channels mpsc watch broadcast diferenças" \
  > /tmp/sub-queries.txt

# Passo 2: rodar deep-research com sub-queries manuais
timeout 120 duckduckgo-search-cli -q -f json deep-research "tokio vs async-std 2026" \
  --sub-query-strategy manual --sub-queries-file /tmp/sub-queries.txt \
  --aggregate rrf \
  | jaq '.resultados[] | {titulo, url, score}'
```

- ANTI-PADRÃO: usar estratégia heurística padrão — produz sub-queries genéricas de baixa qualidade
- ANTI-PADRÃO: copiar a query do usuário como sub-query — adicionar ângulos específicos
- Cada sub-query DEVE atingir um aspecto distinto: arquitetura, benchmarks, pricing, limitações, comparações
- FÓRMULA DO OUTPUT: `.sintese` (Markdown), `.metadados.sub_queries[]` (status por sub-query), `.resultados[]` (agregado via RRF)
- MAPA DE EXIT: `0=sucesso`, `1=sub-query-falhou`, `2=erro-arg`, `3=anti-bot-durante-fanout`, `4=timeout`, `5=zero-agregado`, `6=bloqueio-suspeito`
- COMBINE com `--pre-flight` para ambientes bloqueados:

```bash
timeout 120 duckduckgo-search-cli --pre-flight -q -f json deep-research "rust async 2026" \
  --sub-query-strategy manual --sub-queries-file /tmp/sub-queries.txt --max-sub-queries 5
```

## Como configurar retries e timeouts sem disparar anti-bot
- DEVE usar `--retries 2` (clamp `[1, 10]`, GAP-WS-57 v0.7.8 — flag agora é honrada)
- DEVE usar `--timeout 20` por requisição HTTP individual
- DEVE usar `--global-timeout 60` (única) ou `300` (batch)
- ANTI-PADRÃO: `--retries > 10` — trigger garantido de anti-bot
- ANTI-PADRÃO: loops de retry em shell — use `--retries` nativo com backoff exponencial

## Como descobrir e usar cada flag
- `--probe` — health check mínimo (v0.6.4+)
- `--probe-deep` — detector CAPTCHA via query real (v0.7.3+)
- `--pre-flight` — auto-rota via probe-deep primeiro (v0.7.10+, GAP-WS-58)
- `--allow-lite-fallback` — opt-in rebaixamento HTML→Lite (v0.7.3+, GAP-WS-59)
- `--identity-profile <name>` — pina identidade (auto/chrome-win/chrome-mac/chrome-linux/edge-win/firefox-linux/safari-mac)
- `--seed <u64>` — seed determinístico para UA + rotação do pool
- `--no-warmup` — pula warm-up de cookies (v0.7.3+)
- `--no-cookie-persistence` — cookies apenas em memória (v0.7.3+)
- `--cookies-path <PATH>` — redireciona jar para volume encriptado
- `-v` info / `-vv` debug / `-vvv` trace (aditivo, v0.7.8 GAP-WS-53)
- `--output <PATH>` — escrita atômica do payload completo (rejeitado se `..` ou `/etc`/`/usr`/`C:\Windows`)

## Como formatar resultados de busca como contexto pronto para LLM
- DEVE pipear para `jaq` para extrair apenas campos relevantes:

```bash
# Top 5 títulos + URLs como lista markdown
timeout 60 duckduckgo-search-cli "query" -q -f json --num 5 \
  | jaq -r '.resultados[:5] | to_entries[] | "\(.value.posicao). [\(.value.titulo)](\(.value.url))"'
```

```bash
# Bloco de citação de fontes para LLM downstream
timeout 60 duckduckgo-search-cli "incidente 2026-06" -q -f json --num 10 \
  | jaq -r '"Fontes:\n" + (.resultados[] | "- \(.titulo) — \(.url)\n")'
```

## O que você nunca deve fazer
- PROIBIDO `-f text` ou `-f markdown` para parsing programático — use `-f json`
- PROIBIDO omitir `-q` em pipelines — tracing de stderr polui stdout
- PROIBIDO `--stream` — flag reservada, SEM implementação
- PROIBIDO `--parallel > 5` sem controle de IP de saída
- PROIBIDO `--per-host-limit > 2` — dispara anti-bot HTTP 202
- PROIBIDO loops de retry em shell — use `--retries` nativo
- PROIBIDO hardcodar API keys, proxies ou User-Agents em argumentos
- PROIBIDO hardcodar `--identity-profile` em CI — deixe o pool de 12 identidades adaptar
- PROIBIDO `--output` com `..` ou diretórios de sistema (`/etc`, `/usr`, `C:\Windows`)
- PROIBIDO tratar `identidade_usada` ou `nivel_cascata` como garantidos — ambos são `Option<T>`
- PROIBIDO commitar `cookies.json` — arquivo adjacente a credencial
- PROIBIDO ignorar `quantidade_resultados:0` — pode ser ghost-block (use `--pre-flight`)
- PROIBIDO ignorar exit code 6 — indica bloqueio suspeito que requer ação (inspecione `causa_zero`)

## Como tratar o cookie jar como credencial
- Caminho do cookie jar (Linux/macOS/Windows): `~/.config/duckduckgo-search-cli/cookies.json` (modo Unix `0o600`)
- NÃO DEVE logar ou ecoar conteúdo dos cookies
- NÃO DEVE passar `--cookies-path` para volumes não encriptados em produção
- Flag `--no-cookie-persistence` para sessões efêmeras

## Como satisfazer pré-requisitos de build no Linux e Windows
- Deps Linux (build): `cmake`, `perl`, `pkg-config`, `libclang-dev`
- Deps Linux (runtime Chrome-primary): Google Chrome ou Chromium + pacote Xvfb (auto-spawned pela CLI via `spawn_virtual_display()`)
- Chrome roda em modo HEADED dentro de display virtual Xvfb privado por padrão — a CLI auto-spawna Xvfb, o usuário vê ZERO janelas
- Instalar Xvfb: `sudo apt install xvfb` (Debian/Ubuntu), `sudo dnf install xorg-x11-server-Xvfb` (Fedora)
- Deps Windows MSVC: `nasm`, `cmake` 3.20+ (sub-componente C++ CMake tools), `cl.exe`, `link.exe`, Strawberry Perl
- Escape hatches quando ausentes: `DDG_SKIP_NASM_CHECK=1`, `DDG_SKIP_CMAKE_CHECK=1`, `DDG_SKIP_MSVC_CHECK=1`, `DDG_SKIP_PERL_CHECK=1`
- `cargo install` SEMPRE compila do fonte — crates.io NÃO envia binários pré-construídos

```bash
# Instalar deps runtime Chrome-primary em Debian/Ubuntu
sudo apt-get install -y google-chrome-stable
# OU com Chromium
sudo apt-get install -y chromium-browser
# OBRIGATÓRIO: Xvfb para display virtual do Chrome (auto-spawned pela CLI)
sudo apt-get install -y xvfb
```

## Como instalar ou atualizar para v0.8.5

```bash
cargo install duckduckgo-search-cli --version 0.8.5 --locked --force
```

## APÊNDICE — Notas de Migração (v0.7.10 → v0.8.5)
- Chrome roda HEADED dentro de display virtual Xvfb privado como transporte PRIMÁRIO de busca — a CLI auto-spawna Xvfb via `spawn_virtual_display()` com 17 sinais stealth JavaScript injetados via CDP para bypassar Cloudflare Bot Management 2026
- NOVAS DEPENDÊNCIAS DE RUNTIME: Google Chrome (ou Chromium) + Xvfb (Linux, auto-spawned pela CLI via `spawn_virtual_display()`)
- wreq permanece APENAS para `--fetch-content` e `--probe` — NÃO é mais usado para buscas primárias
- NOVO EXIT CODE 6 (`SUSPECTED_BLOCK`): emitido quando `causa_zero != Legitimo` e `DUCKDUCKGO_ZERO_CAUSE_STRICT` não é `false`
- Para restaurar comportamento exit 5 legado: `DUCKDUCKGO_ZERO_CAUSE_STRICT=false`
- Classificação `ZeroCause` com 6 variantes: `legitimo`, `filtro-silencioso`, `ghost-block`, `anti-bot`, `resposta-invalida`, `zero-resultados-suspeito`
- Descompressão HTTP transparente (gzip, deflate, brotli) via `src/decompress.rs` com limite de 32 MiB
- NOVOS CAMPOS DE METADADOS: `bytes_brutos`, `bytes_descomprimidos`, `usou_chrome` (bool), `tentou_chrome` (bool), `cascata_nivel_observado`
- `.causa_zero_histogram` (BTreeMap) em multi-query agrega contagens por causa entre sub-queries
- `--pre-flight` disponível para prevenção automática de ghost-block (GAP-WS-58)
- `--allow-lite-fallback` funciona em qualquer posição relativa ao subcomando `deep-research` (correção GAP-WS-59)
- Campo `identidade_usada` presente em paths de falha a partir da v0.7.10 (GAP-AUD-001)
- `--retries` honrado pelo executor paralelo desde v0.7.8 (GAP-WS-57)
- Stack TLS: BoringSSL via `wreq 6.0.0-rc.29` desde v0.7.3 (era rustls)
- Veja CHANGELOG.pt-BR.md e README.pt-BR.md para histórico completo
