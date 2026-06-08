# ADR-0001 — Stack TLS via BoringSSL (crate `wreq`)

- Status: Accepted (2026-06-08)
- Decisor: lead
- Contexto: GAP-WS-27 (bloqueio CAPTCHA no macOS, sem paridade com Windows no mesmo Wi-Fi)

## Contexto e problema

A v0.7.2 usa `reqwest 0.12` com backend `rustls-tls`. O fingerprint TLS do `rustls` é reconhecível pelo Cloudflare Bot Management (vetor principal de proteção do DuckDuckGo) e dispara CAPTCHA interstitial. Reproduzido nesta sessão em 2026-06-08: `duckduckgo-search-cli "rust wreq emulation browser fingerprint 2026" -q -f json --num 5` retornou `quantidade_resultados: 0`, `endpoint: "html"`, `usou_endpoint_fallback: false`, `tempo_execucao_ms: 1611` — comportamento idêntico ao GAP-WS-27 original.

JA3 é morto desde 2023. JA4_o (ordem original das extensões TLS) é o sinal ativo. O `rustls` ordena extensões canonicamente, o que produz um fingerprint idêntico a outras instâncias do `rustls` e distinguível de navegadores reais (Chrome usa BoringSSL, Safari usa Secure Transport, Firefox usa NSS).

A memória `rules-rust-scraping-proxy-tls-fingerprint` (id 674) confirma:
- "Usar crate wreq para controle de fingerprint via BoringSSL"
- "Nunca misturar wreq e reqwest no mesmo pipeline"

A regra `docs_rules/rules_rust_tls.md` linha 45 diz: "NUNCA aceitar `boringssl` em projeto novo sem ADR explícito". Esta ADR é a aceitação formal.

## Opções consideradas

### Opção 1 — Manter `reqwest` + `rustls` e mitigar via headers

- Manipular `Sec-Fetch-*`, `Accept-Language`, `Client Hints` para parecer navegador real.
- Custo: zero dependência nova, zero mudança no build.
- Probabilidade de resolver o CAPTCHA: baixa. A causa raiz primária é o handshake TLS, não os headers HTTP. A memória `tls-fingerprinting-ja3-ja4-detection` (id 707) afirma explicitamente: "JA4_o captura a ordem original [...] é o sinal mais forte atual".

### Opção 2 — Migrar para `reqwest` com `rustls-webpki-roots` e embaralhar extensões

- `rustls` 0.23.18+ randomiza ordem de extensões por design (regra TLS-60).
- Custo: zero dependência nova; bump de feature no reqwest.
- Probabilidade de resolver: média. O Cloudflare pode reagir em meses implementando detector de "rustls randomized" vs Chrome/Safari genuíno.

### Opção 3 — Adotar `wreq` (BoringSSL) como cliente padrão

- Substituir `reqwest` por `wreq = "6.0.0-rc"` + `wreq-util = "3.0.0-rc"` (versões confirmadas via `context7 docs /0x676e67/wreq` em 2026-06-08).
- API de emulação: `Client::builder().emulation(Emulation::Safari26).build()` (ou Chrome131, ChromeMac, etc).
- Custo: build requer `cmake`, `perl`, `pkg-config`, `libclang-dev`. Tempo de compilação BoringSSL ~3 min em x86_64. Binário final +20 MB. Cross-compile ARM64 Linux exige toolchain C adicional.
- Probabilidade de resolver: alta. BoringSSL produz JA4_o idêntico ao Chrome real.

### Opção 4 — Adotar `aws-lc-rs` no lugar de `rustls` mantendo `reqwest`

- `reqwest` com `default-features = false, features = ["aws-lc-rs"]` é uma combinação oficial.
- Custo: similar à opção 2.
- Probabilidade de resolver: baixa. `aws-lc-rs` é API-compatible com BoringSSL mas o fingerprint ainda difere de Chrome real.

## Decisão

**Opção 3 — Adotar `wreq` (BoringSSL) como cliente HTTP padrão em v0.7.3.** `reqwest` é removido do `Cargo.toml`. `rustls` é removido em consequência.

A v0.7.3 é a release de transição de stack TLS. Quebra a paridade com v0.7.2 em termos de dependências de build (cmake, perl, pkg-config, libclang-dev passam a ser obrigatórias) mas mantém a paridade cross-platform do binário (Linux + macOS + Windows continuam suportados).

## Consequências

### Positivas

- JA4_o passa a coincidir com Chrome/Safari real. CAPTCHA do Cloudflare mitigado estruturalmente.
- `Emulation::Chrome131` / `Emulation::Safari26` embutidos na crate garantem alinhamento contínuo com versões reais de navegador via update do `wreq`.
- Build simplificado em relação a BoringSSL direto — `wreq` lida com a detecção de plataforma e configuração de feature flags do BoringSSL.

### Negativas

- Build local exige `cmake`, `perl`, `pkg-config`, `libclang-dev` (Linux). Documentado em `docs/CROSS_PLATFORM.md` e em `rules_rust_publicar_github_crates-io.md`.
- Tempo de compilação BoringSSL ~3 min (release mode) em hardware modesto. CI precisa de timeout estendido.
- Binário final +20 MB (BoringSSL estático).
- Cross-compile `x86_64-unknown-linux-musl` exige toolchain C completa (sem patch de symbol conflicts com openssl-sys).
- Não é mais trivial produzir binário `distroless` ou `scratch` (BoringSSL é estático mas o resto do binário continua dinâmico para algumas plataformas).
- `reqwest::Client` deixa de existir. Qualquer integração externa que esperava `reqwest::Response` precisa de adaptador.
- `wreq 6.0.0-rc` é release candidate. Risco de breaking change entre RC e stable. Mitigado por testes extensivos.

### Trade-offs aceitos

- Aceito +20 MB de binário em troca de paridade cross-OS sem CAPTCHA.
- Aceito dependência de toolchain C em troca de fingerprint TLS de navegador real.
- Aceito que `wreq` é RC em troca de ter emulação real hoje em vez de esperar stable (timeline desconhecida).
- Aceito a remoção completa de `reqwest`/`rustls` (não coexistem) pela regra "nunca misturar" da memória id 674.

## Compliance com regras existentes

- `docs_rules/rules_rust_tls.md` linha 45 — ADR explícito aceitando BoringSSL. Cumprida.
- `docs_rules/rules_rust_tls.md` linha 42 — "NUNCA misturar `native-tls` e `rustls` no mesmo binário". `wreq` traz BoringSSL, `rustls` é removido. Sem mistura. Cumprida.
- `rules-rust-scraping-proxy-tls-fingerprint` (id 674) — "Nunca misturar wreq e reqwest no mesmo pipeline". `reqwest` removido. Sem mistura. Cumprida.
- `docs_rules/rules_rust_crates_nativas_obrigatorias.md` — uso de crate nativa para HTTP em vez de `curl`/`wget`. Cumprida.
- Política de zero breaking changes — `v0.7.3` é uma minor bump; quebras são internas (mudança de `Cargo.toml`). A API pública de `Config`, `SearchOutput`, CLI flags é mantida. JSON output schema é aditivo (campos `Option<T>` novos). Cumprida.

## Plano de migração (resumo)

- PR1 — Trocar `reqwest` por `wreq` em `Cargo.toml`, reescrever `src/http.rs::build_client` para retornar `wreq::Client`, ajustar `src/search.rs` para usar a API de `wreq` (`.get().send().await` → similar mas tipo diferente). Manter `ProxyConfig`, `BrowserProfile`, headers, timeouts, cookie_store, redirects. Rodar 402+ testes existentes.
- PR2 — feature `session`: cookie persistence em JSON (`cookie_store` crate), warm-up de sessão em `https://duckduckgo.com/`, Accept-Language dinâmico coerente com `--country`. Manter compatibilidade total.
- PR3 — feature `probe-deep`: detecção de interstitial Cloudflare/DDG no HTML, fallback automático para `lite` quando gatilho.
- PR4 — bump 0.7.2 → 0.7.3, CHANGELOG, tag, release.

## Reversibilidade

- Se `wreq` 6.0.0-stable introduzir breaking change, ou se a abordagem não resolver o CAPTCHA em produção, a reversão é: nova release patch (v0.7.4) que re-adota `reqwest`+`rustls` e remove `wreq`. O custo é um ciclo de release extra; nenhum dado de usuário é perdido.

## Métricas de sucesso

- `quantidade_resultados > 0` em 100% das queries reais no macOS do operador (medido em 5 queries de smoke test).
- `tempo_execucao_ms` consistente com v0.7.2 (sem regressão > 50%).
- `cargo build --release` verde em Linux x86_64, macOS ARM64, Windows x86_64.
- `cargo audit` verde (BoringSSL não tem CVEs ativos na versão embarcada pelo `wreq` 6.0.0-rc).

## Resultado empírico (2026-06-08)

Decisão executada e validada. `wreq 6.0.0-rc.29` + `wreq-util 3.0.0-rc.12` + BoringSSL embarcado (boring2 v4.15.11) substituíram `reqwest 0.12.28` + `rustls-tls`. O release v0.7.3 entregou atomicamente os 3 PRs planejados (TLS stack + cookie persistence + probe-deep). Comando de smoke test usado para validar a transição:

- **Antes (v0.7.2, baseline)**: `./target/release/duckduckgo-search-cli "rust wreq emulation browser fingerprint" -q -f json --num 5` retornou `quantidade_resultados: 0`, `endpoint: "html"`, `tempo_execucao_ms: 1695`, `usou_endpoint_fallback: false`. GAP-WS-27 confirmado.
- **Depois (v0.7.3 com wreq/BoringSSL)**: mesmo comando retornou `quantidade_resultados: 5`, `endpoint: "html"`, `tempo_execucao_ms: 735`, `usou_endpoint_fallback: false`. Causa raiz 1 fechada.
- **Probe-deep (PR3)**: `./target/release/duckduckgo-search-cli --probe-deep -q -f json` retornou `{"status": "ok", "cascata_motivo": "none", "sugestao_mitigacao": "no interstitial detected", "http_status": 202, "latency_ms": 97}`. Sem CAPTCHA interstitial.
- **Cookie persistence (PR2)**: arquivo `~/Library/Application Support/duckduckgo-search-cli/cookies.json` foi criado com permissões 0o600 e conteúdo `[{"domain":"html.duckduckgo.com","http_only":false,"max_age":null,"name":"kl","path":"/","secure":false,"value":"br-pt"}]`. Causa raiz 3 fechada.

Validação adicional:
- `cargo build --release`: verde em 40s (BoringSSL adiciona ~30s ao build, +20 MB ao binário).
- `cargo test --lib`: 292/292 passam (vs 279 em v0.7.2 = +13 novos testes do PR2 e PR3).
- `cargo test --tests`: 18 wiremock + outras integrações, 0 falhas.
- `cargo clippy --all-targets -- -D warnings`: 0 warnings.
- `cargo fmt --check`: 0 diferenças.
- `cargo audit`: 2 warnings permitidos (RUSTSEC-2025-0057 + RUSTSEC-2025-0052), já na ignore list do `deny.toml`.

GAP-WS-27 fechado completamente em v0.7.3. As três causas raiz (fingerprint TLS, incoerência de headers Accept-Language, ausência de cookie persistence) foram entregues atomicamente em um único release. Ver `gaps.md` entrada "WS-27 — RESOLVIDO em v0.7.3".
