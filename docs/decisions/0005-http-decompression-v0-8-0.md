# ADR-0005 — Descompressão HTTP transparente (Bug #1, v0.8.0)

- Status: Accepted (2026-06-19). Note: wreq references in this ADR are historical; wreq was replaced by reqwest+rustls in v0.8.6 (ADR-0008)
- Decisor: lead
- Contexto: GAP-AUD-003 — classificador de zero-result rotulava Cloudflare challenge (14KB com `anomaly-modal`) como `Legitimo` em produção bloqueada porque o body chegava como bytes gzip-comprimidos.

## Contexto e problema

A CLI envia `accept-encoding: gzip, deflate, br` desde v0.6.5 mas `wreq 6.0.0-rc.29` (stack TLS via BoringSSL) não descomprime automaticamente — comportamento confirmado por reprodução local em 2026-06-19.

Resultado: `first_html` chega como bytes gzip-comprimidos (9247 bytes binários vs 14180 bytes texto plano — taxa de compressão 65.2% consistente com gzip level-6 default para HTML repetitivo). `detectar_interstitial_com_match` em `src/probe_deep.rs:175` faz `body.contains("anomaly-modal")` em bytes binários → retorna `(NO_MARKER_SENTINEL, None)` → classificador rotula `Legitimo` em ambiente comprovadamente bloqueado pelo Cloudflare.

A regra `docs_rules/rules_rust_http_clients.md` linha 89 diz: "NUNCA aceitar cliente HTTP sem auto-decompress quando `accept-encoding` é enviado". Esta ADR é a aceitação formal do work-around enquanto `wreq` não adota o upstream.

## Opções consideradas

### Opção 1 — Trocar para `reqwest 0.12` com `gzip`/`brotli` features (rejeitada)

Avaliada e rejeitada. Reverteria o ADR-0001 (BoringSSL via `wreq` para evitar CAPTCHA do Cloudflare no macOS). Sem `wreq`, a issue de fingerprint TLS volta e o bug original do GAP-WS-27 reaparece.

### Opção 2 — Adicionar `flate2` + `brotli-decompressor` e descomprimir manualmente (escolhida)

Implementada em `src/decompress.rs` (novo módulo). Wrapper `decompress::response_body_string` substitui `response.text().await` em 7 call sites:

| Antes | Depois |
|---|---|
| `src/search.rs:403` (primeira página) | `decompress::response_body_string(response).await` |
| `src/search.rs:565` (`first_html` que alimenta classificador) | idem |
| `src/search.rs:686` (fallback Lite) | idem |
| `src/search.rs:776` (paginação) | idem |
| `src/lib.rs:637` (probe_deep health-check) | idem |
| `src/pipeline.rs:311` (warmup) | idem |
| `src/content.rs:180` (`response.bytes()` em content fetch) | idem |

Cap de 32 MiB (`DECOMPRESSION_MAX_OUTPUT`) protege contra gzip bombs.

## Consequências

### Positivas

- GAP-AUD-003 Bug #1 fechado — classificador detecta Cloudflare challenge corretamente em produção bloqueada.
- Wrapper é transparente para call sites — mesma assinatura `Result<String, CliError>`.
- Suporte a `gzip`, `deflate`, `br` (Brotli) com detecção via header `Content-Encoding`.
- 6 testes E2E em `tests/integration_decompression.rs` (identity, gzip, deflate, br, oversize, unsupported) + 1 regression test em `tests/integration_audit_gap_aud_003.rs` (Bug #1 reproduction com fixture 14KB real pré-comprimido).
- `flate2 = "1"` adicionada ao `Cargo.toml` (~80KB binário, sem transitive risky).
- `brotli-decompressor = "=5.0.1"` já pinada desde v0.7.7 (GAP-WS-49 fix).

### Negativas

- CPU-bound work em contexto async — usa `tokio::task::spawn_blocking` para não bloquear o reactor.
- Variantes de erro novas em `CliError` (`PayloadTooLarge`, `UnsupportedEncoding`, `InvalidUtf8`, `DecompressionIo`) — `#[non_exhaustive]` mantém compatibilidade forward.
- Wrapper não cobre `deflate` raw (apenas zlib via `ZlibDecoder`); DDG pode usar `deflate` com header RFC 1951 ou 1950 — testado com wiremock E2E.

## Work-around futuro (v0.9.0+)

Se `wreq` upstream adicionar auto-decompression antes de v0.9.0:
1. Marcar `decompress::response_body_string` como `#[deprecated]`.
2. Manter o wrapper por ≥1 minor para BC.
3. Migrar para `response.text().await` direto quando o upstream estabilizar.

## No-go para reversão

- Reverter para `reqwest+rustls` quebraria o GAP-WS-27 (CAPTCHA macOS).
- Reverter para `wreq::Response::text()` sem descompressão quebraria o GAP-AUD-003 (classificador rotula Cloudflare como Legitimo).
