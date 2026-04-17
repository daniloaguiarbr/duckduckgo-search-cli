# Livro de Receitas

> duckduckgo-search-cli — receitas executáveis que se integram a qualquer pipeline LLM em menos de 60 segundos.


## Índice
### Receitas em Português
- [Receita 01 — Top 5 resultados como CSV em 1 comando](#receita-01--top-5-resultados-como-csv-em-1-comando)
- [Receita 02 — Relatório Markdown arquivado em disco](#receita-02--relatório-markdown-arquivado-em-disco)
- [Receita 03 — Pesquisa paralela multi-query com pontuação de deduplicação](#receita-03--pesquisa-paralela-multi-query-com-pontuação-de-deduplicação)
- [Receita 04 — Construtor de whitelist de domínios para filtros RAG](#receita-04--construtor-de-whitelist-de-domínios-para-filtros-rag)
- [Receita 05 — Monitor de notícias das últimas 24h com snapshots com timestamp](#receita-05--monitor-de-notícias-das-últimas-24h-com-snapshots-com-timestamp)
- [Receita 06 — Payload de pesquisa profunda pronto para a janela de contexto do LLM](#receita-06--payload-de-pesquisa-profunda-pronto-para-a-janela-de-contexto-do-llm)
- [Receita 07 — Crawling seguro com rate-limit abaixo de thresholds anti-abuso](#receita-07--crawling-seguro-com-rate-limit-abaixo-de-thresholds-anti-abuso)
- [Receita 08 — Busca via proxy com verificação de vazamento de IP](#receita-08--busca-via-proxy-com-verificação-de-vazamento-de-ip)
- [Receita 09 — Pipeline zero-ruído para cron e systemd](#receita-09--pipeline-zero-ruído-para-cron-e-systemd)
- [Receita 10 — Detector de bloqueio anti-bot com roteamento por exit code](#receita-10--detector-de-bloqueio-anti-bot-com-roteamento-por-exit-code)
- [Receita 11 — Auditoria de amplitude: gap de cobertura top 5 vs top 15](#receita-11--auditoria-de-amplitude-gap-de-cobertura-top-5-vs-top-15)
- [Receita 12 — Comparação Markdown lado-a-lado de duas queries](#receita-12--comparação-markdown-lado-a-lado-de-duas-queries)
- [Receita 13 — Exportação NDJSON para ClickHouse, BigQuery e DuckDB](#receita-13--exportação-ndjson-para-clickhouse-bigquery-e-duckdb)
- [Receita 14 — Pipeline busca-para-sumarização com LLM local](#receita-14--pipeline-busca-para-sumarização-com-llm-local)
- [Receita 15 — Função bash com defaults seguros e opinativos](#receita-15--função-bash-com-defaults-seguros-e-opinativos)
- [Receita 16 — Diagnóstico de pipe com PIPESTATUS](#receita-16--diagnóstico-de-pipe-com-pipestatus)
- [Receita 17 — Anti-bloqueio com perfis de browser v0.6.0](#receita-17--anti-bloqueio-com-perfis-de-browser-v060)
- [Tabela Receita para Caso de Uso](#tabela-receita-para-caso-de-uso)


## RECEITAS EM PORTUGUÊS

### Receita 01 — Top 5 resultados como CSV em 1 comando
- Ganho: extraia 5 pares título+URL ranqueados como CSV em menos de 200ms sem parser nem scraper.
- Problema: agentes LLM desperdiçam tokens parseando JSON bruto em formato tabular para ferramentas downstream.
- Benefício: `-q` direciona todo o tracing para stderr, deixando stdout como JSON puro para pipe.
- Benefício: `jaq -r` emite linhas CSV diretamente — sem arquivos intermediários, sem dependências extras.
- Benefício: `timeout 30` limita o comando com precisão contra requisições travadas em pipelines de CI.
- Resultado: linhas CSV prontas para colar, consumíveis por qualquer planilha, carregador ETL ou contexto de agente.

```bash
timeout 30 duckduckgo-search-cli -q -n 5 -f json "rust async runtimes 2026" \
  | jaq -r '.resultados[] | [.posicao, .titulo, .url] | @csv'
```

Saída esperada:
```
1,"Tokio — runtime assíncrono para Rust","https://tokio.rs/"
2,"async-std: Versão assíncrona da std","https://async.rs/"
3,"smol — runtime async pequeno e rápido","https://github.com/smol-rs/smol"
4,"Escolhendo um runtime async em Rust (2026)","https://blog.rust-lang.org/..."
5,"Comparando runtimes async em Rust","https://example.com/..."
```


## Receita 02 — Relatório Markdown arquivado em disco
- Ganho: gere um relatório Markdown revisável por humanos para qualquer query com 1 flag.
- Problema: equipes perdem contexto de pesquisa quando os resultados existem apenas em abas do navegador.
- Benefício: `-o` cria diretórios pai atomicamente e v0.5.0 valida segurança do path — rejeita `..` e diretórios de sistema.
- Benefício: o formatter `markdown` gera artefatos prontos para PR com títulos, URLs e snippets.
- Benefício: `-n 15` captura 3x mais sinal do que a visualização padrão de top-5.
- Resultado: um arquivo `.md` durável revisável no GitHub, VS Code ou `glow` sem pós-processamento.

```bash
timeout 45 duckduckgo-search-cli -q \
  -n 15 \
  -f markdown \
  -o reports/rust-webassembly.md \
  "rust webassembly edge computing"
```

Saída esperada:
```
(sem stdout; arquivo gravado)
$ bat -p reports/rust-webassembly.md | head -6
# Search results — rust webassembly edge computing
_Fetched: 2026-04-14T12:34:56Z — 15 results_

1. WASM na borda com Rust — https://example.com/...
   > Snippet curto descrevendo a página...
```


## Receita 03 — Pesquisa paralela multi-query com pontuação de deduplicação
- Ganho: execute 5 queries simultaneamente e ranqueie URLs por frequência de citação cruzada em 1 pipeline.
- Problema: queries sequenciais perdem quais fontes aparecem consistentemente entre subtópicos.
- Benefício: `--queries-file` com `--parallel 5` faz fan-out de 5 buscas preservando a polidez por host.
- Benefício: o array `buscas[]` no JSON de saída contém todos os resultados em 1 único arquivo consolidado.
- Benefício: `uniq -c | sort -rn` ranqueia URLs pela frequência com que aparecem entre as queries.
- Resultado: uma lista ranqueada identificando fontes canônicas — a base para seleção de fontes em RAG.

```bash
printf '%s\n' \
  "rust async runtimes" \
  "tokio vs async-std" \
  "rust runtime benchmark" \
  "rust executor design" \
  "glommio runtime" > /tmp/queries.txt

timeout 90 duckduckgo-search-cli -q \
  --queries-file /tmp/queries.txt \
  --parallel 5 \
  -n 10 \
  -f json \
  -o /tmp/multi.json

jaq -r '.buscas[].resultados[].url' /tmp/multi.json \
  | sort \
  | uniq -c \
  | sort -rn \
  | head -10
```

Saída esperada:
```
      4 https://tokio.rs/
      3 https://github.com/async-rs/async-std
      2 https://docs.rs/tokio/latest/tokio/
      1 https://blog.rust-lang.org/async-book
      1 https://github.com/smol-rs/smol
```


## Receita 04 — Construtor de whitelist de domínios para filtros RAG
- Ganho: extraia uma lista deduplicada de domínios de origem confiáveis de qualquer tópico de pesquisa em 1 pipeline.
- Problema: sistemas RAG ingerem fontes de baixa qualidade quando nenhum filtro de domínio é aplicado.
- Benefício: `rg -oP` extrai apenas esquema e host — descarta componentes de path ruidosos.
- Benefício: `sort -u` gera uma lista estável ordenada alfabeticamente adequada para arquivos de política.
- Benefício: o pipe direto do stdout evita gravar arquivos de resultado intermediários.
- Resultado: uma allow-list pronta para uso para grounding de LLM, política de conteúdo ou filtros de ingestão de documentos.

```bash
printf '%s\n' \
  "postgres tuning best practices" \
  "postgres vacuum autovacuum" \
  "postgres wal tuning" > /tmp/pg.txt

timeout 120 duckduckgo-search-cli -q \
  --queries-file /tmp/pg.txt \
  -n 20 \
  -f json \
  | jaq -r '.buscas[].resultados[].url' \
  | rg -oP '^https?://[^/]+' \
  | sort -u \
  > /tmp/pg-whitelist.txt

bat -p /tmp/pg-whitelist.txt
```

Saída esperada:
```
https://pgdash.io
https://postgresqlco.nf
https://wiki.postgresql.org
https://www.crunchydata.com
https://www.enterprisedb.com
https://www.postgresql.org
```


## Receita 05 — Monitor de notícias das últimas 24h com snapshots com timestamp
- Ganho: capture um snapshot diário dos resultados das últimas 24h de qualquer tópico com nomes de arquivo seguros para rotação.
- Problema: jobs de cron sobrescrevem snapshots anteriores quando os nomes de arquivo são estáticos.
- Benefício: `--time-filter d` mapeia para o parâmetro `df=d` do DuckDuckGo, restringindo às últimas 24 horas.
- Benefício: a variável `${STAMP}` no nome do arquivo impede sobrescrita entre invocações.
- Benefício: cada arquivo JSON é autocontido e consultável independentemente após o fato.
- Resultado: um arquivo rotativo de snapshots com data pronto para diff, análise de tendências ou workflows de alerta.

```bash
STAMP=$(date -u +%Y%m%dT%H%M%SZ)
mkdir -p /var/log/ddg-monitor

timeout 60 duckduckgo-search-cli -q \
  --time-filter d \
  -n 20 \
  -f json \
  -o /var/log/ddg-monitor/ai-safety-${STAMP}.json \
  "ai safety regulation"

jaq -r '.resultados[] | "\(.posicao). \(.titulo) — \(.url)"' \
  /var/log/ddg-monitor/ai-safety-${STAMP}.json \
  | head -5
```

Saída esperada:
```
1. Início da aplicação do AI Act na UE — https://...
2. Novo benchmark de segurança em IA divulgado — https://...
3. Anthropic publica atualização sobre interpretabilidade — https://...
4. Reestruturação na governança da OpenAI — https://...
5. Audiência no Senado sobre modelos de fronteira — https://...
```


## Receita 06 — Payload de pesquisa profunda pronto para a janela de contexto do LLM
- Ganho: busque os 10 primeiros resultados com até 5k caracteres de conteúdo de página por resultado em 1 comando.
- Problema: LLMs alimentados apenas com snippets perdem o detalhe necessário para síntese precisa.
- Benefício: `--fetch-content` popula o campo `conteudo` com texto sem HTML por resultado.
- Benefício: `--max-content-length 5000` limita o uso de tokens preservando conteúdo significativo da página.
- Benefício: o pipe pelo `jaq` produz um arquivo Markdown seccionado com `##` que cabe diretamente em uma janela de contexto.
- Resultado: um payload de contexto longo pronto para LLM sem scrapers intermediários nem sessões de navegador.

```bash
timeout 180 duckduckgo-search-cli -q \
  -n 10 \
  --fetch-content \
  --max-content-length 5000 \
  -f json \
  -o /tmp/deep.json \
  "differential privacy federated learning"

jaq -r '
  .resultados[]
  | "## \(.titulo)\nURL: \(.url)\n\n\(.conteudo // "(sem conteúdo)")\n\n---"
' /tmp/deep.json > /tmp/llm-context.md

wc -l /tmp/llm-context.md
bat -p /tmp/llm-context.md | head -20
```

Saída esperada:
```
1243 /tmp/llm-context.md
## Uma introdução à privacidade diferencial
URL: https://example.org/dp-primer

Privacidade diferencial é um framework matemático...
(até 5000 caracteres)
---
```


## Receita 07 — Crawling seguro com rate-limit abaixo de thresholds anti-abuso
- Ganho: execute pesquisa multi-query sem acionar defesas anti-bot usando 3 flags.
- Problema: queries paralelas sem limite por host atingem os throttles anti-abuso do DuckDuckGo.
- Benefício: `--parallel 2` limita a concorrência a 2 queries simultâneas.
- Benefício: `--per-host-limit 1` garante 1 requisição em voo por host por vez.
- Benefício: `--retries 3` absorve falhas transitórias sem intervenção do operador.
- Benefício: `--global-timeout 280` garante que o job inteiro encerra limpo dentro do `timeout 300`.
- Resultado: execução de pesquisa polida que sobrevive a execuções longas sem acionar bloqueios.

```bash
timeout 300 duckduckgo-search-cli -q \
  --queries-file /tmp/sensitive.txt \
  --parallel 2 \
  --per-host-limit 1 \
  --retries 3 \
  --timeout 30 \
  --global-timeout 280 \
  -n 15 \
  -f json \
  -o /tmp/safe-research.json

jaq -r '.quantidade_queries, (.buscas[].metadados.tempo_execucao_ms)' /tmp/safe-research.json
```

Saída esperada:
```
5
1823
2104
1987
2231
1902
```


## Receita 08 — Busca via proxy com verificação de vazamento de IP
- Ganho: verifique que todo o tráfego foi roteado por um proxy SOCKS5 com 1 campo JSON autoritativo.
- Problema: ferramentas com proxy frequentemente voltam silenciosamente para conexões diretas quando o proxy está inacessível.
- Benefício: `metadados.usou_proxy` confirma conexão de proxy e v0.5.0 mascara credenciais de proxy em toda saída de erro automaticamente.
- Benefício: `false` é um sinal inequívoco de que o proxy nunca foi conectado e o IP real vazou.
- Benefício: `jaq` extrai apenas os 3 campos que importam — sem parsing do conjunto de resultados completo.
- Resultado: verificação de proxy em uma linha que serve como smoke test para qualquer ambiente tunelado.

```bash
timeout 60 duckduckgo-search-cli -q \
  --proxy socks5://127.0.0.1:1080 \
  -n 10 \
  -f json \
  "teste de conteudo restrito por geoip" \
  | jaq '.metadados | {usou_proxy, user_agent, tempo_execucao_ms}'
```

Saída esperada:
```json
{
  "usou_proxy": true,
  "user_agent": "Mozilla/5.0 (...)",
  "tempo_execucao_ms": 2134
}
```


## Receita 09 — Pipeline zero-ruído para cron e systemd
- Ganho: execute snapshots de busca horários sem supervisão com exit codes limpos e sem poluição de log.
- Problema: jobs de cron que emitem ruído de tracing poluem logs do sistema e acionam alertas falsos.
- Benefício: `-q` direciona todo o tracing para stderr e para longe da captura de stdout do cron.
- Benefício: `--global-timeout` é definido menor que o `timeout` externo para que a CLI encerre limpa.
- Benefício: a CLI encerra com um exit code significativo em vez de ser SIGKILL'd pelo timer externo.
- Resultado: um job de snapshot silencioso por hora que gera artefatos de auditoria observáveis por exit code.

```bash
# /etc/cron.d/ddg-snapshot
# 15 * * * * user timeout 120 duckduckgo-search-cli -q \
#   --queries-file /etc/ddg/watchlist.txt \
#   --global-timeout 110 \
#   --retries 2 \
#   -n 15 \
#   -f json \
#   -o /var/log/ddg/$(date -u +\%Y\%m\%dT\%H).json \
#   2>> /var/log/ddg/errors.log
```

Saída esperada:
```
(sem stdout; snapshots JSON horários aterrissam em /var/log/ddg/; erros, se houver, acumulam em errors.log)
```


## Receita 10 — Detector de bloqueio anti-bot com roteamento por exit code
- Ganho: distinga bloqueios HTTP-202 anti-bot de falhas reais sem parsear corpos de resposta.
- Problema: tratamento de erro genérico retenta toda falha da mesma forma, desperdiçando orçamento de rate-limit.
- Benefício: exit code 3 é reservado exclusivamente para a assinatura anti-bot HTTP-202.
- Benefício: rotear no exit code 3 permite que a lógica de retry direcione apenas bloqueios, como rotacionar proxy.
- Benefício: exit codes 4 e 5 surfaciam timeouts globais e zero resultados como estados observáveis separados.
- Resultado: uma função shell observável que registra cada classe de resultado no destino correto.

```bash
run_ddg() {
  local q="$1"
  timeout 30 duckduckgo-search-cli -q -n 10 -f json "$q" > /tmp/out.json
  local ec=$?
  case $ec in
    0) echo "OK: $q" ;;
    3) echo "BLOQUEADO: $q" >&2 ;;
    4) echo "TIMEOUT_GLOBAL: $q" >&2 ;;
    5) echo "ZERO_RESULTADOS: $q" >&2 ;;
    *) echo "FALHA($ec): $q" >&2 ;;
  esac
  return $ec
}

run_ddg "query legítima"
run_ddg "query provavelmente bloqueada que parece bot"
```

Saída esperada:
```
OK: query legítima
BLOQUEADO: query provavelmente bloqueada que parece bot
```


## Receita 11 — Auditoria de amplitude: gap de cobertura top 5 vs top 15
- Ganho: identifique exatamente quais URLs uma query top-5 perde em comparação ao top-15 com diferença de conjuntos.
- Problema: definir o padrão para top-5 perde fontes significativas que ranqueiam entre as posições 6 e 15.
- Benefício: dois arquivos JSON independentes permitem comparação limpa de conjuntos sem estado compartilhado.
- Benefício: `sort -u` normaliza ambas as listas para que `comm -13` calcule a diferença exata de conjuntos.
- Benefício: a saída nomeia apenas URLs únicas no conjunto mais amplo — sem falsos positivos.
- Resultado: uma auditoria baseada em evidências que quantifica o custo de amplitude de uma configuração `--num` estreita.

```bash
Q="llm inference benchmarking"

timeout 30 duckduckgo-search-cli -q -n 5  -f json "$Q" > /tmp/top5.json
timeout 30 duckduckgo-search-cli -q -n 15 -f json "$Q" > /tmp/top15.json

jaq -r '.resultados[].url' /tmp/top5.json  | sort -u > /tmp/urls5.txt
jaq -r '.resultados[].url' /tmp/top15.json | sort -u > /tmp/urls15.txt

echo "=== Apenas no top 15 (perdidos no top 5) ==="
comm -13 /tmp/urls5.txt /tmp/urls15.txt
```

Saída esperada:
```
=== Apenas no top 15 (perdidos no top 5) ===
https://arxiv.org/abs/2404.12345
https://github.com/some-lab/llm-bench
https://huggingface.co/blog/...
...
```


## Receita 12 — Comparação Markdown lado-a-lado de duas queries
- Ganho: renderize duas queries como uma tabela Markdown com ranks correspondentes em 10 linhas de shell.
- Problema: comparar duas estratégias de busca requer um layout visual lado-a-lado sem navegador.
- Benefício: dois payloads JSON independentes mantêm a comparação portátil e reproduzível.
- Benefício: o acesso indexado via `jaq` extrai cada título por posição de rank sem dependência de jq.
- Benefício: a tabela resultante é renderizada nativamente no GitHub, VS Code e `glow` sem ferramentas extras.
- Resultado: um artefato de comparação Markdown pronto para commit produzido em 1 execução de pipeline.

```bash
Q1="rust web framework axum"
Q2="rust web framework actix"

timeout 30 duckduckgo-search-cli -q -n 5 -f json "$Q1" > /tmp/a.json
timeout 30 duckduckgo-search-cli -q -n 5 -f json "$Q2" > /tmp/b.json

{
  echo "| # | $Q1 | $Q2 |"
  echo "|---|-----|-----|"
  for i in $(seq 1 5); do
    T1=$(jaq -r ".resultados[$((i-1))].titulo" /tmp/a.json)
    T2=$(jaq -r ".resultados[$((i-1))].titulo" /tmp/b.json)
    echo "| $i | $T1 | $T2 |"
  done
} > /tmp/compare.md

bat -p /tmp/compare.md
```

Saída esperada:
```
| # | rust web framework axum | rust web framework actix |
|---|-----|-----|
| 1 | Axum — framework web ergonômico | Actix Web — poderoso e pragmático |
| 2 | Começando com Axum | Quickstart do Actix Web |
| 3 | Axum + middleware Tower | Guia de middleware do Actix-web |
...
```


## Receita 13 — Exportação NDJSON para ClickHouse, BigQuery e DuckDB
- Ganho: achate uma execução multi-query em 1 objeto JSON por linha pronto para `COPY FROM` direto.
- Problema: arrays JSON aninhados requerem transformação antes da ingestão em datastores colunares.
- Benefício: `jaq -c` emite NDJSON compacto com 1 objeto por linha — formato nativo para loaders em massa.
- Benefício: o schema achatado inclui campos `query` e `ts` para agrupamento e particionamento.
- Benefício: 10 queries com 15 resultados cada produz exatamente 150 linhas — previsível para dimensionamento de pipeline.
- Resultado: um arquivo `.ndjson` carregável em qualquer store colunar com um único comando `COPY`.

```bash
timeout 120 duckduckgo-search-cli -q \
  --queries-file /tmp/etl-queries.txt \
  -n 15 \
  -f json \
  | jaq -c '
    .buscas[] as $b
    | $b.resultados[]
    | {
        query: $b.query,
        ts: $b.timestamp,
        posicao: .posicao,
        titulo: .titulo,
        url: .url,
        snippet: .snippet
      }
  ' > /tmp/results.ndjson

wc -l /tmp/results.ndjson
bat -p -r 1:3 /tmp/results.ndjson
```

Saída esperada:
```
150 /tmp/results.ndjson
{"query":"q1","ts":"2026-04-14T12:00:00Z","posicao":1,"titulo":"...","url":"...","snippet":"..."}
{"query":"q1","ts":"2026-04-14T12:00:00Z","posicao":2,"titulo":"...","url":"...","snippet":"..."}
{"query":"q1","ts":"2026-04-14T12:00:00Z","posicao":3,"titulo":"...","url":"...","snippet":"..."}
```


## Receita 14 — Pipeline busca-para-sumarização com LLM local
- Ganho: transforme uma query de busca em uma sumarização de 5 bullets ancorada em fontes buscadas em 2 comandos.
- Problema: LLMs locais alucinam sem contexto de grounding, mas montar esse contexto requer um scraper.
- Benefício: `--fetch-content --max-content-length 3000` entrega texto de página sem HTML dentro do JSON.
- Benefício: `jaq` formata o JSON multi-resultado na string única que a API de chat estilo OpenAI espera.
- Benefício: `xh` cuida da serialização JSON do corpo da requisição automaticamente — sem flags de curl.
- Resultado: um pipeline de sumarização ancorada de query para bullets com citações sem navegador nem scraper.

```bash
timeout 60 duckduckgo-search-cli -q \
  -n 10 --fetch-content --max-content-length 3000 \
  -f json \
  "o que é retrieval augmented generation" \
  > /tmp/rag.json

CONTEXT=$(jaq -r '[.resultados[] | "- \(.titulo): \(.conteudo // .snippet)"] | join("\n")' /tmp/rag.json)

timeout 60 xh POST http://127.0.0.1:11434/v1/chat/completions \
  model=llama3.1 \
  messages:='[
    {"role":"system","content":"Resuma as fontes em 5 bullets. Cite URLs."},
    {"role":"user","content":"'"$CONTEXT"'"}
  ]' \
  | jaq -r '.choices[0].message.content'
```

Saída esperada:
```
- RAG combina retrieval + geração para ancorar LLMs com contexto fresco (https://...).
- Embeddings + banco vetorial são a camada canônica de retrieval (https://...).
- Estratégia de chunking afeta materialmente a qualidade da resposta (https://...).
- Re-ranking aumenta a precisão@k antes da chamada ao LLM (https://...).
- Avaliação tipicamente usa faithfulness + context recall (https://...).
```


## Receita 15 — Função bash com defaults seguros e opinativos
- Ganho: codifique timeout, retries, fetch-content e saída JSON em 1 chamada de função reutilizável.
- Problema: operadores esquecem combinações seguras de flags e produzem execuções de busca travadas ou não confiáveis.
- Benefício: a função codifica `--retries 3`, `--timeout 20` e `--global-timeout 110` em um único lugar.
- Benefício: `--fetch-content --max-content-length 8000` entrega conteúdo profundo sem comandos extras.
- Benefício: o nome de arquivo com timestamp automático impede sobrescrita de execuções anteriores da mesma query.
- Benefício: o repasse do exit code permite que pipelines upstream ramifiquem em sucesso ou falha.
- Resultado: um comando de pesquisa repetível, auditável e sem colisão em que sua equipe pode confiar em produção.

```bash
# Adicionar ao ~/.bashrc ou ~/.zshrc
ddg-deep() {
  local query="$*"
  if [ -z "$query" ]; then
    echo "uso: ddg-deep <query...>" >&2
    return 2
  fi
  local slug
  slug=$(echo "$query" | tr -cs '[:alnum:]' '-' | sed 's/-$//')
  local out="./ddg-${slug}-$(date -u +%Y%m%dT%H%M%SZ).json"
  timeout 120 duckduckgo-search-cli -q \
    -n 15 \
    --retries 3 \
    --timeout 20 \
    --global-timeout 110 \
    --fetch-content \
    --max-content-length 8000 \
    -f json \
    -o "$out" \
    "$query"
  local ec=$?
  if [ $ec -eq 0 ]; then
    echo "Salvo: $out"
    jaq -r '.resultados[] | "\(.posicao). \(.titulo)"' "$out" | head -5
  else
    echo "ddg-deep falhou com exit code $ec" >&2
  fi
  return $ec
}

# Uso:
ddg-deep "comparação de runtimes async em rust 2026"
```

Saída esperada:
```
Salvo: ./ddg-comparacao-de-runtimes-async-em-rust-2026-20260414T153000Z.json
1. Tokio — runtime assíncrono para Rust
2. async-std: Versão assíncrona da std
3. smol — runtime async pequeno e rápido
4. Comparando runtimes async em Rust — edição 2026
5. Glommio — runtime thread-per-core
```


## Receita 16 — Diagnóstico de pipe com PIPESTATUS
- Ganho: detecte falhas silenciosas do CLI ocultas pela semântica de exit code do shell em pipes.
- Problema: `cmd | jaq` reporta apenas o exit code do `jaq` — um exit 5 (zero resultados) do CLI é invisível.
- Benefício: `${PIPESTATUS[0]}` captura o exit code do CLI mesmo dentro de um pipe.
- Benefício: roteamento por PIPESTATUS previne perda silenciosa de dados em pipelines automatizados.
- Resultado: pipe observável que expõe exit codes do CLI e do consumidor.

```bash
timeout 60 duckduckgo-search-cli "rust async" -q -n 5 -f json \
  | jaq -r '.resultados[].url'
echo "CLI=${PIPESTATUS[0]} JQ=${PIPESTATUS[1]}"
# CLI=0 JQ=0  → sucesso
# CLI=5 JQ=0  → zero resultados (jaq recebeu array vazio)
# CLI=3 JQ=0  → bloqueio anti-bot
# CLI=4 JQ=0  → timeout global
```


## Receita 17 — Anti-bloqueio com perfis de browser v0.6.0
- Ganho: use o perfil `PerfilBrowser` embutido para reduzir bloqueios HTTP 202 e truncamentos silenciosos.
- Problema: User-Agent genérico dispara desafios anti-bot do DuckDuckGo sistematicamente.
- Benefício: headers `Sec-Fetch-*` por família e Client Hints imitam sessão real de browser.
- Benefício: detecção de HTTP 202 anomaly reenvia com backoff exponencial automaticamente.
- Benefício: detecção de bloqueio silencioso (limiar 5 KB) trata respostas truncadas como bloqueios.
- Resultado: menos eventos exit-3 e menos falsos positivos de zero resultados em pipelines automatizados.

```bash
# Perfis de fingerprint v0.6.0 ativam automaticamente — nenhuma flag necessária
timeout 60 duckduckgo-search-cli "rust async runtime" -q -f json --num 15 \
  | jaq '.resultados[:5]'

# Se exit 3 ainda ocorrer, rotacione IP e tente com endpoint lite
timeout 60 duckduckgo-search-cli "query" -q -f json --num 15 \
  --proxy socks5://127.0.0.1:9050 --endpoint lite \
  | jaq '.resultados'

# Handler respeitando exit codes v0.6.0 (3 = bloqueio, 5 = zero resultados)
timeout 60 duckduckgo-search-cli "query" -q -f json --num 15 > /tmp/r.json
case $? in
  0) jaq '.resultados' /tmp/r.json ;;
  3) echo "bloqueio anti-bot — aguarde 300s, rotacione proxy ou use --endpoint lite" >&2 ;;
  5) echo "zero resultados — refine a query ou mude --lang" >&2 ;;
  *) echo "erro $?" >&2; exit $? ;;
esac
```


## Tabela Receita para Caso de Uso

| Receita | Caso de uso | Ferramentas |
|---|---|---|
| 01 | Triagem rápida top-N em uma linha | `duckduckgo-search-cli`, `jaq`, `timeout` |
| 02 | Relatório Markdown arquivado | `duckduckgo-search-cli`, `bat`, `timeout` |
| 03 | Pesquisa multi-query consolidada | `duckduckgo-search-cli`, `jaq`, `sort`, `uniq`, `timeout` |
| 04 | Construção de whitelist de domínios | `duckduckgo-search-cli`, `jaq`, `rg`, `sort`, `bat`, `timeout` |
| 05 | Monitoramento 24h agendado | `duckduckgo-search-cli`, `jaq`, `date`, `timeout` |
| 06 | Contexto longo para RAG/LLM | `duckduckgo-search-cli --fetch-content`, `jaq`, `bat`, `timeout` |
| 07 | Crawling polido rate-limited | `duckduckgo-search-cli`, `jaq`, `timeout` |
| 08 | Verificação de roteamento por proxy | `duckduckgo-search-cli --proxy`, `jaq`, `timeout` |
| 09 | Snapshot horário não-supervisionado | `duckduckgo-search-cli`, `cron`/`systemd`, `timeout` |
| 10 | Observabilidade de bloqueios anti-bot | `duckduckgo-search-cli` (exit code 3), `bash case`, `timeout` |
| 11 | Auditoria de amplitude de resultados | `duckduckgo-search-cli`, `jaq`, `comm`, `sort`, `timeout` |
| 12 | Comparação A/B em Markdown | `duckduckgo-search-cli`, `jaq`, `bat`, `timeout` |
| 13 | Exportação NDJSON para ETL | `duckduckgo-search-cli`, `jaq -c`, `bat`, `timeout` |
| 14 | Pipeline busca para sumarização com LLM | `duckduckgo-search-cli --fetch-content`, `jaq`, `xh`, `timeout` |
| 15 | Defaults opinativos reutilizáveis | `duckduckgo-search-cli`, função bash, `jaq`, `date`, `timeout` |
| 16 | Diagnóstico de pipe com PIPESTATUS | `duckduckgo-search-cli`, `jaq`, `PIPESTATUS`, `timeout` |
| 17 | Anti-bloqueio com perfis de browser v0.6.0 | `duckduckgo-search-cli`, `jaq`, `bash case`, `timeout` |

_Fim do Livro de Receitas._
