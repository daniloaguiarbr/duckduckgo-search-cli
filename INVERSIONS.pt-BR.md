# Inversões Arquiteturais

`duckduckgo-search-cli` deliberadamente inverte vários defaults comuns do
ecossistema Rust. Este documento explica cada inversão, por que foi feita
e qual é o trade-off. Leia antes de propor uma alternativa "padrão" em
PRs — toda inversão aqui tem uma rationale registrada que uma escolha
"mais idiomática" quebraria silenciosamente.

## Inversao 1 — `wreq` em vez de `reqwest` (v0.7.3–v0.8.5, REVERTIDA na v0.8.6)

> **Status: REVERTIDA na v0.8.6** — substituida por `reqwest` + `rustls-tls` (ADR-0008). Chrome headed (v0.8.0+) fornece fingerprint TLS real de navegador, tornando emulacao BoringSSL redundante. A toolchain de build BoringSSL (NASM, CMake, Perl) bloqueava usuarios Windows no `cargo install`.

- **Expectativa default**: novos projetos Rust CLI usam `reqwest` com `rustls-tls`.
- **O que fizemos (v0.7.3)**: substituimos `reqwest 0.12 + rustls` por `wreq 6.0.0-rc.29`
  (vincula estaticamente BoringSSL).
- **Por que**: `rustls` produz um fingerprint TLS canonico que o Cloudflare
  Bot Management reconhece como nao-navegador, disparando interstitials
  de CAPTCHA no DuckDuckGo. `wreq` + BoringSSL produz um fingerprint
  identico ao Chrome e Safari, eliminando o CAPTCHA no macOS. Veja
  `docs/decisions/0001-tls-boring-via-wreq.md`.
- **Trade-off**: `wreq 6.0.0-rc` e release candidate (nao estavel 1.0);
  tempo de compilacao e ~40s mais longo devido a BoringSSL; builds
  requerem `cmake`, `perl`, `pkg-config`, `libclang-dev` no Linux e
  NASM/CMake/MSVC/Perl no Windows.
- **Por que revertida (v0.8.6)**: Chrome headed (transport primario desde v0.8.0) gera fingerprint TLS REAL de navegador, tornando emulacao wreq/BoringSSL redundante. A toolchain de build BoringSSL (NASM, CMake, Perl, MSVC) era barreira total para usuarios Windows (GAP-WS-066). Ver `docs/decisions/0008-reqwest-rustls-v0-8-6.md`.

## Inversão 2 — thiserror para libs, sem anyhow em código de biblioteca (v0.5.0+)

- **Expectativa default**: `anyhow::Result` é o padrão de fato para código
  Rust de aplicação.
- **O que fizemos**: definimos `enum CliError` (15 variantes) em
  `src/error.rs` via `thiserror`. Cada erro tem um `error_code()` e
  `exit_code()` tipados. Sem `anyhow` em `src/`.
- **Por quê**: exit codes (0..=6) e error codes (`http_error`,
  `rate_limited`, etc.) machine-readable são parte do contrato público.
  `anyhow` apagaria esses dados. Agentes de IA e CI scripts ramificam em
  `error_code` para decidir retry vs. fail.
- **Trade-off**: 15 braços de match em cada `?`. Novos tipos de erro
  requerem atualizar `exit_code()` e `error_code()`. Mitigação:
  o atributo `#[non_exhaustive]` em `CliError` permite compatibilidade
  forward para consumers downstream.
- **No-go para reversão**: remover erros tipados quebraria silenciosamente
  todo agente que casa em `error_code` para lógica de retry.

## Inversão 3 — `BTreeMap` para histograma em output multi-query (v0.8.0+)

- **Expectativa default**: `HashMap` para agregação.
- **O que fizemos**: `MultiSearchOutput.causa_zero_histogram: BTreeMap<String, u32>`.
- **Por quê**: ordem de iteração determinística entre runs é requerida
  para testes de golden-file snapshot e para output JSON reproduzível
  (snapshot tests via `insta = "1"`). `HashMap` introduz ordem
  aleatória de iteração → snapshot tests flaky.
- **Trade-off**: insert ligeiramente mais lento (O(log n) vs O(1)). O
  histograma tem <100 entradas na prática; custo é negligível.
- **No-go para reversão**: output JSON não-determinístico quebra o
  contrato de snapshot test.

## Inversão 4 — Nomes de campo em português brasileiro no JSON de saída (v0.2.0+)

- **Expectativa default**: ecossistema Rust usa identificadores em inglês.
- **O que fizemos**: campos de `SearchResult` serializam como `posicao`,
  `titulo`, `url`, `url_exibicao`, `snippet`, etc. (não `position`, `title`,
  `url`).
- **Por quê**: exemplos do README e receitas `jaq` em `docs/COOKBOOK.md`
  usam queries em português; campos em inglês quebravam esses pipelines
  (bug reportado pelo usuário em v0.1.0 → corrigido em v0.2.0). O naming
  em PT-BR é load-bearing no modelo mental do agente.
- **Trade-off**: pipelines de outros ecossistemas (`n8n`, `zapier`,
  `make.com`) precisam aprender os nomes de campo em português. A
  tabela de mapeamento completa está documentada em
  `docs/INTEGRATIONS.md`.
- **No-go para reversão**: mudar nomes de campo quebraria silenciosamente
  todo pipeline CI construído no contrato v0.2.0+. O guia de migração
  v0.1.0 → v0.2.0 foi um evento único.

## Inversão 5 — `#[serde(skip_serializing_if = "Option::is_none")]` em TODOS os campos Option

- **Expectativa default**: serializar `Option::None` como JSON `null`.
- **O que fizemos**: todo campo `Option<T>` em `types.rs` carrega
  `#[serde(skip_serializing_if = "Option::is_none")]`.
- **Por quê**: o envelope JSON deve ser mínimo — consumers não precisam
  diferenciar "campo ausente" de "campo é null". Campos ausentes
  significam "não aplicável para esta query" (ex., `causa_zero` é
  ausente quando results > 0, presente quando zero).
- **Trade-off**: pipelines não podem distinguir "campo faltando" de
  "campo era null na serialização". Mitigação: o SKILL.md documenta
  a semântica dos campos; o campo `causa_zero` é um aditivo diagnóstico
  (BC opt-out preserva o campo mesmo quando exit code é 5 legacy).
- **No-go para reversão**: ligar serialização de `null` dobraria o
  tamanho de cada output JSON e requereria todo consumer tratar ambos
  `null` e ausente.

## Inversão 6 — `--allow-lite-fallback` como OPT-IN (v0.7.8+)

- **Expectativa default**: fallback para endpoint lite quando html falha.
- **O que fizemos**: fallback requer flag explícita `--allow-lite-fallback`.
  Sem ela, detecção anti-bot retorna exit 3 com `cascata_motivo`
  populado no JSON, NÃO fallback silencioso.
- **Por quê**: fallback silencioso viola a intenção do usuário. O usuário
  pode querer saber que está sendo bloqueado (para fins de rate limit)
  em vez de receber resultados truncados de um endpoint degradado. v0.7.8
  GAP-WS-52 corrigiu o comportamento de fallback silencioso.
- **Trade-off**: usuários que dependiam do comportamento silencioso antigo
  precisam adicionar a flag explicitamente. O CHANGELOG.md Migration
  Guide documenta isso.
- **No-go para reversão**: fallback silencioso era um canal de
  comportamento covert que surpreendia integradores esperando opt-in
  explícito.

## Inversão 7 — `bin/safety-contracts` para gates de CI (v0.7.10+)

- **Expectativa default**: um único workflow CI roda todos os checks.
- **O que fizemos**: cada gate de CI é um script `bin/` discreto invocado
  individualmente pelo workflow. Exemplos: `bin/check-fmt`,
  `bin/check-clippy`, `bin/check-tests`, `bin/check-audit`,
  `bin/check-coverage`, `bin/check-version-drift`.
- **Por quê**: binários discretos deixam desenvolvedores rodarem o gate
  CI exato localmente antes de fazer push. Um único workflow `ci.yml`
  com bash embarcado era imensurável em isolamento.
- **Trade-off**: 9+ binários para manter. Mitigação: cada binário tem
  <50 linhas e tem um `README.md` por script.
- **No-go para reversão**: CI monolítico é um ponto de dor conhecido
  para debug de flakes.

## Inversão 8 — `atomwrite` como única ferramenta de edição de arquivo (v0.8.0+)

- **Expectativa default**: `std::fs::write` ou `tokio::fs::write` em
  código Rust, `sed -i`/`echo >` em scripts.
- **O que fizemos**: toda modificação de arquivo passa pela CLI
  `atomwrite` com `--expect-checksum` (locking otimista via BLAKE3)
  e escrita atômica (tempfile + fsync + rename).
- **Por quê**: um incidente de truncamento de `c24-framework34.html`
  (2026-06-15) no projeto upstream perdeu ~127 linhas de trabalho.
  `atomwrite` provê 6 camadas de defesa (L1 telemetria, L2 `--require-backup`,
  L3 `--confirm`, L4 `--preview`, L5 `--auto-rotate`, L6 `risk_assessment`
  no envelope). Veja ADR-0035.
- **Trade-off**: cada invocação de script tem uma cerimônia
  `CS=$(atomwrite read --json ...)`. Mitigação: aliases em `.cargo/config.toml`
  (`cargo check-all`, `cargo lint`, etc.) reduzem o boilerplate.
- **No-go para reversão**: sobrescritas silenciosas são exatamente o modo
  de falha que causou o incidente 2026-06-15.

## Inversão 9 — Sem telemetria, sem analytics, sem export OTLP (todas as versões)

- **Expectativa default**: CLIs de produção emitem telemetria de uso
  para endpoints controlados pelo vendor.
- **O que fizemos**: zero telemetria. `tracing` é usado para logs
  locais mas nunca exportado. Padrões `opentelemetry`, `OTLP`,
  `exporter` e `analytics` estão explicitamente ausentes da base
  de código. CI gate `rg -n 'opentelemetry|OTLP|exporter|tracing::span' src/` retorna 0.
- **Por quê**: privacidade primeiro. O usuário é o único dono dos seus
  dados de busca. Detecção anti-bot é mais difícil quando o fingerprint
  do cliente não inclui uma assinatura de agente de telemetria.
- **Trade-off**: zero observabilidade de uso em produção. Mitigação:
  logs locais `tracing` em stderr; flags `--verbose`/`-vv`/`-vvv`
  escalam verbosidade; o usuário pode fazer grep dos próprios logs.
- **No-go para reversão**: README e SKILL.md do projeto declaram
  explicitamente "sem telemetria". Adicionar telemetria requereria
  nova versão major.

## Como Propor uma Nova Inversão

1. Abra uma issue com a label "Proposta de Inversão".
2. Documente: qual default você está invertendo, por que o default
   falha no contexto deste projeto, qual é o trade-off, e um critério
   de no-go (quando esta inversão NÃO deve ser revertida).
3. Adicione uma seção a este arquivo seguindo o formato das inversões
   existentes.
4. Atualize o `description` do workspace `Cargo.toml` se a inversão
   afeta o contrato público.
5. Referencie a inversão na ADR relevante.
