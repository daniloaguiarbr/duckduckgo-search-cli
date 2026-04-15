---
name: duckduckgo-search-cli-pt
description: Use esta skill SEMPRE que o usuário pedir busca web, pesquisa na internet, consulta de documentação atualizada, grounding factual, verificação de URL, extração de conteúdo de páginas, coleta de evidências externas, enriquecimento RAG, fact-checking, lookup de versão de biblioteca, post-mortem de incidente, pricing atual de vendor, ou qualquer dado fora da knowledge cutoff. Dispara para triggers em português "busca no google", "pesquisa na web", "procure online", "verifique essa URL", "traga resultados atualizados". Invoca a CLI `duckduckgo-search-cli` v0.4.x via Bash com contrato JSON estável e zero API key. Versão em português brasileiro.
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
- JAMAIS execute sem `timeout` — pipelines travam indefinidamente.

```bash
timeout 60 duckduckgo-search-cli "<query>" -q -f json --num 15 | jaq '.resultados'
```

## Proibições Absolutas
- PROIBIDO usar `-f text` ou `-f markdown` para parsing programático.
- PROIBIDO omitir `-q` em qualquer pipeline que leia stdout.
- PROIBIDO usar `--stream` — flag reservada, SEM implementação em v0.4.x.
- PROIBIDO usar `--parallel` acima de 5 sem controle de IP de saída.
- PROIBIDO usar `--per-host-limit` acima de 2 — dispara anti-bot HTTP 202.
- PROIBIDO loops de retry em shell — use `--retries` nativo com backoff exponencial.
- PROIBIDO hardcodar API keys, proxies ou User-Agents em argumentos.
- PROIBIDO assumir `snippet`, `url_exibicao`, `titulo_original` sempre presentes.

## Parsing JSON Obrigatório com jaq
- SEMPRE use `jaq` (NUNCA `jq`) para processar o output JSON.
- SEMPRE aplique fallback `// ""` em campos opcionais.
- SEMPRE distinga root single-query (`.resultados`) de multi-query (`.buscas[]`).
- DEVE extrair latência via `.metadados.tempo_execucao_ms` para observabilidade.
- DEVE monitorar `.metadados.usou_endpoint_fallback` para detectar degradação de IP.

```bash
timeout 60 duckduckgo-search-cli "rust async runtime" -q -f json --num 15 \
  | jaq '.resultados[] | {
      posicao,
      titulo,
      url,
      snippet: (.snippet // ""),
      url_exibicao: (.url_exibicao // .url)
    }'
```

## Campos JSON Garantidos vs Opcionais
- GARANTIDOS não-null: `.query`, `.resultados[].posicao`, `.resultados[].titulo`, `.resultados[].url`.
- OPCIONAIS `Option<String>`: `.resultados[].snippet`, `.resultados[].url_exibicao`, `.resultados[].titulo_original`.
- METADADOS sempre presentes: `.metadados.tempo_execucao_ms`, `.metadados.quantidade_resultados`, `.metadados.usou_endpoint_fallback`.
- CONDICIONAIS com `--fetch-content`: `.resultados[].conteudo`, `.tamanho_conteudo`, `.metodo_extracao_conteudo`.

## Exit Codes Determinísticos
- Exit 0: sucesso — parse o stdout com `jaq`.
- Exit 1: erro runtime — leia stderr e reporte ao usuário.
- Exit 2: erro de argumento CLI — corrija flags antes de retentar.
- Exit 3: bloqueio anti-bot HTTP 202 — aguarde 300s e troque para `--endpoint lite`.
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

## Validação Pós-Invocação
- SEMPRE verifique exit code antes de parsear stdout.
- SEMPRE cheque `.metadados.usou_endpoint_fallback` e logue se `true`.
- SEMPRE confirme `.quantidade_resultados` maior que zero antes de agir nos dados.
- JAMAIS alucine conteúdo ausente — se o campo veio null, reporte ausência ao usuário.

## Integração com Memória
- DEVE citar a URL exata como fonte ao usar fato extraído desta skill.
- DEVE preferir resultado com `posicao` baixa (ranking DuckDuckGo) como fonte primária.
- JAMAIS combine fatos de múltiplos resultados sem atribuir cada um à sua URL.

## Regra de Ouro
- Na dúvida entre alucinar e invocar a CLI, INVOQUE a CLI sempre.
- Custo de 1 invocação é 60-300ms. Custo de alucinação é retrabalho e perda de confiança.
- SEMPRE prefira dado verificado com URL a suposição plausível sem fonte.
