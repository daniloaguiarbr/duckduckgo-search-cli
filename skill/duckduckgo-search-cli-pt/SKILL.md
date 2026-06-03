---
name: duckduckgo-search-cli-pt
description: Use esta skill SEMPRE que o usuário pedir busca web, pesquisa na internet, consulta de documentação atualizada, grounding factual, verificação de URL, extração de conteúdo de páginas, coleta de evidências externas, enriquecimento RAG, fact-checking, lookup de versão de biblioteca, post-mortem de incidente, pricing atual de vendor, ou qualquer dado fora da knowledge cutoff. Dispara para triggers em português "busca no google", "pesquisa na web", "procure online", "verifique essa URL", "traga resultados atualizados". Invoca a CLI `duckduckgo-search-cli` v0.6.4 via Bash com contrato JSON estável, zero API key, pool adaptativo de 12 identidades anti-bot com rotação em cascata de 5 níveis (HTTP 202/403/429), perfis de fingerprint Sec-Fetch-* por família de browser, validação de path traversal no --output, mascaramento automático de credenciais em mensagens de erro, e campo JSON `identidade_usada` para visibilidade diagnóstica. Versão em português brasileiro.
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
- SEMPRE execute `duckduckgo-search-cli --probe` antes de lançar queries reais em sessões longas (v0.6.4+) para detectar bloqueios anti-bot cedo.
- JAMAIS execute sem `timeout` — pipelines travam indefinidamente.

```bash
# Verificação de saúde pré-voo v0.6.4
timeout 15 duckduckgo-search-cli --probe

# Invocação padrão
timeout 60 duckduckgo-search-cli "<query>" -q -f json --num 15 | jaq '.resultados'
```

## Proibições Absolutas
- PROIBIDO usar `-f text` ou `-f markdown` para parsing programático.
- PROIBIDO omitir `-q` em qualquer pipeline que leia stdout.
- PROIBIDO usar `--stream` — flag reservada, SEM implementação em v0.6.4.
- PROIBIDO usar `--parallel` acima de 5 sem controle de IP de saída.
- PROIBIDO usar `--per-host-limit` acima de 2 — dispara anti-bot HTTP 202.
- PROIBIDO loops de retry em shell — use `--retries` nativo com backoff exponencial.
- PROIBIDO hardcodar API keys, proxies ou User-Agents em argumentos.
- PROIBIDO assumir `snippet`, `url_exibicao`, `titulo_original` sempre presentes.
- PROIBIDO passar `--output` com `..` no path — v0.6.4 rejeita path traversal
- PROIBIDO passar `--output` apontando para `/etc`, `/usr` ou `C:\Windows` — dirs de sistema bloqueados
- PROIBIDO hardcodar `--identity-profile` em CI — deixe o pool de 12 identidades adaptar (v0.6.4+)
- PROIBIDO ler `.metadados.identidade_usada` ou `.metadados.nivel_cascata` como campos garantidos — ambos são `Option<T>` (v0.6.4+)

## Parsing JSON Obrigatório com jaq
- SEMPRE use `jaq` (NUNCA `jq`) para processar o output JSON.
- SEMPRE aplique fallback `// ""` em campos opcionais.
- SEMPRE distinga root single-query (`.resultados`) de multi-query (`.buscas[]`).
- DEVE extrair latência via `.metadados.tempo_execucao_ms` para observabilidade.
- DEVE monitorar `.metadados.usou_endpoint_fallback` para detectar degradação de IP.
- DEVE extrair identidade via `.metadados.identidade_usada` (v0.6.4+) para visibilidade diagnóstica — use `// "n/a"` como fallback.
- DEVE inspecionar `.metadados.nivel_cascata` (v0.6.4+) para detectar esgotamento da cascata anti-bot — use `// 0` como fallback.

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
- OPCIONAIS `Option<String>` (v0.6.4+): `.metadados.identidade_usada` — tag de identidade `<família>-<plataforma>-<16hex>` que produziu a resposta.
- OPCIONAIS `Option<u32>` (v0.6.4+): `.metadados.nivel_cascata` — nível de cascata atingido durante a requisição (0..=4).
- METADADOS sempre presentes: `.metadados.tempo_execucao_ms`, `.metadados.quantidade_resultados`, `.metadados.usou_endpoint_fallback`.
- CONDICIONAIS com `--fetch-content`: `.resultados[].conteudo`, `.tamanho_conteudo`, `.metodo_extracao_conteudo`.

## Exit Codes Determinísticos
- Exit 0: sucesso — parse o stdout com `jaq`.
- Exit 1: erro runtime — leia stderr e reporte ao usuário.
- Exit 2: erro de argumento CLI — corrija flags antes de retentar.
- Exit 3: bloqueio anti-bot HTTP 202 — a cascata v0.6.4 JÁ rotacionou até 5 identidades internamente. Aguarde 300s, depois troque para `--endpoint lite` e rotacione proxy.
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
- Identidade usada: `| jaq -r '.metadados.identidade_usada // "n/a"'` (v0.6.4+)
- Nível de cascata: `| jaq '.metadados.nivel_cascata // 0'` (v0.6.4+)

## v0.6.4 — Pool Adaptativo de Identidades Anti-Bot (WS-26)

> **Nota**: v0.6.4 foi publicada no lugar da planejada v0.7.0 para preservar o conjunto de features em desenvolvimento sob um número de patch estável. O binário lançado é funcionalmente idêntico ao que seria v0.7.0. Zero breaking changes em relação à v0.6.3.

### Verificação Pré-Voo Obrigatória
- DEVE executar `duckduckgo-search-cli --probe` em CI antes de lançar queries reais — envia 1 requisição mínima, exit 0 se acessível, 1 se bloqueado.
- DEVE inspecionar `.metadados.nivel_cascata` após exit 3 — a cascata já rotacionou até 5 identidades. Se `nivel_cascata == 4`, o próprio IP está esgotado.

### Novas Flags CLI (v0.6.4)
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

### Receitas Anti-Bot v0.6.4
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
| ausente | Cascata não foi ativada (comportamento padrão em v0.6.4) | Nenhuma |

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

## Regra de Ouro
- Na dúvida entre alucinar e invocar a CLI, INVOQUE a CLI sempre.
- Custo de 1 invocação é 60-300ms. Custo de alucinação é retrabalho e perda de confiança.
- SEMPRE prefira dado verificado com URL a suposição plausível sem fonte.


## Garantias de Segurança (v0.6.0 + v0.6.4)

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
