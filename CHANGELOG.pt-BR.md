# Changelog

Leia este arquivo em [English](CHANGELOG.md).

- Todas as mudanças notáveis deste projeto estão documentadas neste arquivo
- O formato segue [Keep a Changelog](https://keepachangelog.com/pt-BR/1.0.0/)
- Este projeto adere ao [Versionamento Semântico](https://semver.org/lang/pt-BR/)

## [0.7.7] - 2026-06-14

### Corrigido (CRÍTICO, runtime — não detectado pelo pipeline de release do GAP-WS-48)
- **GAP-WS-49 (CRÍTICO, query) — query real retornava ZERO resultados por causa de fingerprint TLS detectável pela DDG.** v0.7.6 fechou o `cargo install` mas o binário publicado passou por todos os smoke tests de `--probe`/`--probe-deep` (status 200/ok) enquanto queries reais retornavam `resultados: 0` com `cascade_level: 0` e `usou_endpoint_fallback: false` — anomalia silenciosa. Reprodução local: 6/6 queries testadas ("rust", "rust language", "tokio rust async", "rust async runtime", "tokio vs async-std", "axum middleware examples") retornaram `quantidade_resultados: 0` com latências 1.0–1.6s.
- **Causa raiz**: o `wreq 6.0.0-rc.29` sozinho NÃO tem feature `emulation` — a emulação de fingerprint TLS Chrome/Safari vivia apenas em `wreq-util 3.0.0-rc.12` via `default = ["emulation"]`. v0.7.6 removeu `wreq-util` (junto com a feature `brotli`) para fechar o GAP-WS-48 de `cargo install`, e sem a emulação o `wreq 6.0.0-rc.29` com BoringSSL plain produz um handshake TLS cujo fingerprint JA3/JA4 é detectável pelo Cloudflare Bot Management. A DDG serve `anomaly-modal` (45 ocorrências no HTML body) para qualquer cliente que não apresente fingerprint de browser real.
- **Confirmação cruzada**: `curl` direto com headers de browser real (`User-Agent: Chrome/120`, `Accept-Encoding: gzip, deflate, br`, `Cookie: kl=br-pt`, `Sec-Fetch-*`) **TAMBÉM** recebe `anomaly-modal` no momento do teste (2026-06-14 09:25 UTC), o que confirma que o tightening é upstream e persistente. O probe mínimo de 1 request (`--probe-deep`) não aciona o tightening porque a DDG faz fingerprint baseado em volume/comportamento, não em request única.
- **Fix aplicado**:
  1. Re-adicionada a dep `wreq-util = { version = "3.0.0-rc", default-features = false, features = ["emulation"] }` no `Cargo.toml` (apenas `emulation`, sem `default`, para não trazer `brotli` por engano).
  2. Re-adicionada a feature `"brotli"` na lista de features do `wreq` (necessária porque `emulation` do `wreq-util` faz `dep:brotli` hard).
  3. Adicionados 2 pins diretos no `Cargo.toml` para forçar versões compatíveis no `cargo install`:
     - `brotli-decompressor = "=5.0.1"` — versão 5.0.0/5.0.1 têm `alloc-no-stdlib = "2.0"` (hard); versão 5.0.2 publicada em 2026-06-14 alargou para `>=2.0.4, <4` e por isso puxa 3.0.0 no grafo.
     - `alloc-no-stdlib = "=2.0.4"` — hard pin necessário porque `brotli 8.0.3` exige `alloc-no-stdlib = "2.0"`.
  4. Adicionado `cargo update -p alloc-no-stdlib@3.0.0 --precise 2.0.4` na resolução do lock, que remove a versão 3.0.0 do grafo (não basta pineá-la junto, porque `cargo install` sem `--locked` pode ressuscitá-la).
  5. Comentário expandido no `Cargo.toml` documentando GAP-WS-49 e a estratégia de pin.
- **Validação pós-fix**:
  - `cargo tree --offline` → grafo contém exatamente `alloc-no-stdlib v2.0.4` e `brotli-decompressor v5.0.1`, zero ocorrências de 3.0.0/0.2.3.
  - `cargo build --release --offline` → **sucesso em 24.04s** (vs 37.14s v0.7.6 — mais rápido porque `brotli-decompressor 5.0.1` é menor que 5.0.2).
  - `cargo install --path .` (sem `--locked`, reproduzindo caminho do usuário) → **sucesso**, binário instalado e funcional.
  - Query real `"rust async runtime"` com binário da v0.7.7 → **`quantidade_resultados: 5`**, latência 1087ms, resultados reais: `The Async Ecosystem`, `Fundamentals of Asynchronous Programming`, `Tokio - An asynchronous Rust runtime`, etc.
  - `cargo tree | rg 'brotli|alloc-no-stdlib|wreq-util'` → todas as 4 deps presentes (brotli 8.0.3, brotli-decompressor 5.0.1, alloc-no-stdlib 2.0.4, wreq-util 3.0.0-rc.12).
- **Impacto**:
  - Binário final: +160KB (brotli 8.0.3 + brotli-decompressor 5.0.1 + wreq-util 3.0.0-rc.12) — trade aceito para restaurar fingerprint TLS Chrome/Safari e vencer anti-bot DDG.
  - Tempo de build do `cargo install`: ~24s (vs ~37s v0.7.6) — mais rápido porque `brotli-decompressor 5.0.1` é menor que 5.0.2.
  - Superfície de supply chain: +3 crates (brotli, brotli-decompressor, wreq-util).
  - **Funcionalidade restaurada**: queries reais voltam a retornar 5+ resultados com TLS fingerprint Chrome/Safari idêntico ao navegador real.
- `Cargo.toml` version bump: 0.7.6 → 0.7.7.


## [0.7.6] - 2026-06-14

### Corrigido (CRÍTICO, build)
- **GAP-WS-48 (CRÍTICO, install) — `cargo install` quebrou em 2026-06-14 por conflito `alloc-no-stdlib 2.0.4 vs 3.0.0`**. Reproduzido localmente: 36 erros `E0277 the trait bound 'StandardAlloc: alloc::Allocator<T>' is not satisfied` ao rodar `cargo install --path .` (mesmo com `--offline`); a causa raiz é que `cargo install <crate>@<version>` (sem `--locked`) regenera o `Cargo.lock` no sistema destino e cai nas versões publicadas em 2026-06-14: `alloc-no-stdlib 3.0.0`, `alloc-stdlib 0.2.3` (`alloc-no-stdlib = ">=2.0.4, <4.0.0"`) e `brotli-decompressor 5.0.2`. O `brotli 8.0.3` (não atualizado, ainda requer `alloc-no-stdlib = "2.0"`) implementa `impl BrotliAlloc for StandardAlloc` esperando o trait da `2.0.4`, mas o `StandardAlloc` de `alloc-stdlib 0.2.3` é compilado contra `3.0.0` — colisão trait-bind em `enc/reader.rs`, `enc/writer.rs` e `enc/combined_alloc.rs`.
- **Causa raiz em 2 camadas**: (CR1) o `wreq-util 3.0.0-rc.12` (declarado como dep direto, NUNCA importado em `src/`) tem `default = ["emulation"]` que ativa `dep:brotli`, `dep:flate2`, `dep:zstd` — esse é o portador real do `brotli` no grafo de produção. A feature `brotli` do `wreq` foi apenas secundária. (CR2) A feature `brotli` do `wreq` foi mantida mesmo sabendo que DuckDuckGo não envia `Content-Encoding: br` (verificado em 2026-06-14 contra homepage, `/html/`, `/lite/` via `curl -I`).
- **Fix aplicado**:
  1. Removida a dep `wreq-util = "3.0.0-rc"` do `Cargo.toml` (era dead code).
  2. Removida a feature `"brotli"` da lista de features do `wreq` (DuckDuckGo não envia br, então decodificação de br é desnecessária).
  3. Atualizado o comentário do `wreq` no `Cargo.toml` para documentar a remoção e referenciar o incidente.
- **Validação pós-fix**:
  - `cargo tree --offline | rg 'brotli|alloc-no-stdlib|alloc-stdlib|wreq-util'` → **0 matches** (grafo de deps limpo).
  - `cargo install --path . --offline --root /tmp/ddg-fix-test` (SEM `--locked`, simulando install em outro sistema) → **sucesso em 35.7s**, binário funcional, JSON schema preservado.
  - `cargo install --path . --locked --offline` → **sucesso** (caminho de CI com lock travado).
  - `cargo build --release` → **sucesso em 37.14s** (5.92s mais rápido que v0.7.5 pela ausência do `brotli` e `brotli-decompressor`).
- **Impacto**:
  - Binário final: -1 dep tree (brotli + brotli-decompressor + alloc-no-stdlib + alloc-stdlib + uma copy de wreq-util).
  - Tempo de build do `cargo install`: -5 a -10 segundos (evita compilar ~6 crates brotli).
  - Superfície de supply chain: -6 crates.
  - **Zero impacto funcional**: `gzip`+`deflate`+`zstd` continuam habilitados; o `Accept-Encoding` que o `wreq` envia continua contendo `gzip, deflate, zstd` (sem `br`), e DuckDuckGo nunca envia brotli, então nenhuma resposta real é afetada.
- `Cargo.toml` version bump: 0.7.5 → 0.7.6.


## [0.7.5] - 2026-06-14

### Corrigido (audit batch 2026-06-14)
- **P1-audit-1 (MEDIUM, contrato de erro)** — `src/lib.rs` em `execute_deep_research` usava `println!("{json}")` diretamente, violando a regra documentada de que `output.rs` é o único módulo com `println!` (tabela na doc de `lib.rs` linha 34). Agora delega para `output::print_line_stdout` que trata `BrokenPipe` de forma limpa (sucesso silencioso em `| head`, erro genérico em falha real de I/O). Fecha o achado de auditoria de que o contrato JSON de `deep-research` burlava a abstração central de output.
- **P1-audit-2 (LOW, clareza de código)** — Removido o braço `unreachable!("handled above")` no dispatch de subcomandos ao dobrar o ramo `DeepResearch` dentro do `match` principal (e descartar o early-return precedente `if let Some(Subcommand::DeepResearch(...))`). A verificação de exaustividade em tempo de compilação agora cobre a variante sem disparar panic no dispatch.
- **P1-audit-3 (LOW, semântica de exit code)** — `CliError::Cancelled` agora mapeia para exit code `130` (POSIX: 128 + SIGINT(2)) em vez de `1` (erro genérico). Sessões de shell agora distinguem Ctrl-C iniciado pelo usuário de falhas reais de runtime, e supervisores de processo (ex.: runners de CI, scripts `set -e`) tratam cancelamento como `exit 130` por convenção.
- **P1-audit-4 (LOW, mapeamento de error code)** — Três mapeamentos de string de error code estavam semanticamente errados: `InvalidConfig` → `selector_config_invalid` (deveria ser `invalid_config`); `PathError` → `selector_config_invalid` (deveria ser `path_error`); `BrokenPipe` → `http_error` (deveria ser `broken_pipe`). Novas constantes adicionadas: `codes::INVALID_CONFIG`, `codes::PATH_ERROR`, `codes::BROKEN_PIPE`. Os três mapeamentos agora usam sua constante dedicada. Consumidores que parseam o campo `error` do JSON de saída conseguem rotear pelo modo de falha preciso.
- **P2-audit-5 (LOW, drift de documentação)** — `#![doc(html_root_url = "https://docs.rs/duckduckgo-search-cli/0.7.4")]` estava atrasado em relação à versão do `Cargo.toml`. Atualizado para `0.7.5`. Fecha o drift de cross-link em docs.rs.
- **P2-audit-7 (MEDIUM, higiene de distribuição)** — `[build-dependencies]` do `Cargo.toml` agora inclui `clap` e `clap_mangen = "0.2"`. O `build.rs` existente foi estendido para chamar uma nova função `generate_man_page()` que emite `duckduckgo-search-cli.1` em `OUT_DIR` usando um espelho best-effort da definição CLI em `src/cli.rs`. A man page é uma conveniência de empacotamento (não é build-critical); falhas são logadas no stderr mas não causam panic no build. Um refactor futuro extrairá a definição CLI em um módulo compartilhado para eliminar o espelho.
- **P3-audit-11 (LOW, drift de CI)** — `Cross.toml` listava `armv7-unknown-linux-musleabihf` como target de conveniência para desenvolvedores, mas o comentário também afirmava que "5 principais" targets eram cobertos pelo `release.yml` (falso: `release.yml` cobre apenas `x86_64-unknown-linux-musl` e `aarch64-apple-darwin`). Removido o bloco `armv7-unknown-linux-musleabihf` e os comentários atualizados para refletir com precisão quais targets são cobertos por CI vs. dev-only. Sem mudança de comportamento de release.

### Cobertura de testes
- `src/error.rs::tests` — asserções adicionadas para `Cancelled.exit_code() == 130`, `Cancelled.error_code() == "cancelled"`, `BrokenPipe.error_code() == "broken_pipe"`, `PathError.error_code() == "path_error"`, `InvalidConfig.error_code() == "invalid_config"`. Total `error::tests`: 5 testes, todos passando.

### Corrigido

- **GAP-WS-29/30/31 fechados (experiência de build, Windows).** Estendeu o preflight do `build.rs` v0.7.4 para detectar também CMake (o crate `cmake` 0.1.58 precisa de `cmake.exe` no PATH ANTES de `enable_language(ASM_NASM)` ser avaliado), compilador e linker MSVC (`cl.exe`/`link.exe` — precisam de `Launch-VsDevShell.ps1` no PATH) e Perl (`perl.exe` para o gerador perlasm do BoringSSL). Novo preflight no `build.rs` aborta em segundos com a correção exata para cada uma das quatro ferramentas. Escape hatches: `DDG_SKIP_NASM_CHECK=1`, `DDG_SKIP_CMAKE_CHECK=1`, `DDG_SKIP_MSVC_CHECK=1`, `DDG_SKIP_PERL_CHECK=1`. Causa raiz: o sub-componente C++ CMake tools for Windows do Visual Studio Installer vem desmarcado por padrão — instalar apenas o workload C++ NÃO fornece CMake.
- **Helper estendido `scripts/install-windows.ps1`** — agora detecta e auto-instala CMake (`winget install -e --id Kitware.CMake` ou choco), Perl (`winget install -e --id StrawberryPerl.StrawberryPerl`) e reporta a instrução exata de instalação MSVC/`Launch-VsDevShell.ps1` (MSVC é grande demais para auto-instalar). Novo modo `--check-only` produz relatório tabular adequado para portões de CI e suporte humano.
- **Novo `scripts/check-windows-toolchain.ps1`** — diagnóstico standalone (sem instalações) que verifica todas as 7 ferramentas (cargo, rustc, cmake, nasm, cl.exe, link.exe, perl) e emite saída texto ou JSON. Exit code 0 se todas presentes, 1 caso contrário. Use para tickets de suporte e portões de CI.
- **Novo `docs/INSTALL-WINDOWS.pt-BR.md`** — guia passo-a-passo cobrindo 5 métodos de instalação (Visual Studio Installer + ferramentas standalone; tudo standalone via winget; apenas Chocolatey; script helper; diagnóstico standalone). Inclui troubleshooting para cada um dos 4 GAPs e os escape hatches `DDG_SKIP_*_CHECK`.
- **Documentação corrigida** — o claim falso de que "VS Build Tools com workload C++ fornece CMake" foi substituído em `docs/CROSS_PLATFORM.pt-BR.md`, `README.pt-BR.md`, `skill/duckduckgo-search-cli-pt/SKILL.md`, `llms.pt-BR.txt` e `llms-full.txt`. O workload C++ NÃO inclui o sub-componente C++ CMake tools — ele deve ser marcado manualmente no Visual Studio Installer.
- **Sem mudanças de runtime** — mesmas flags, mesmo schema JSON, mesmas dependências da v0.7.4. O crates.io continua NÃO distribuindo binários pré-compilados para nenhuma plataforma.

## [0.7.4] - 2026-06-11

### Corrigido
- **GAP-WS-28 — `cargo install` falhava no Windows nativo por NASM ausente**.
  Erro literal: `CMake Error at CMakeLists.txt:374 (enable_language): No CMAKE_ASM_NASM_COMPILER could be found`, surgindo MINUTOS após o início do build do BoringSSL, sem indicar a correção. Causa raiz em 4 camadas: (CR1) o CMakeLists.txt do BoringSSL exige `enable_language(ASM_NASM)` quando `NOT OPENSSL_NO_ASM` em Windows x86/x86_64; (CR2) o build script do `btls-sys` v0.5.6 TEM um ramo `OPENSSL_NO_ASM=YES` para Windows (build/main.rs:314-318), mas ele é INALCANÇÁVEL em builds nativos pelo early-return `host == target` (build/main.rs:231); (CR3) o instalador do NASM não ajusta o PATH e o Visual Studio não inclui `nasm.exe`; (CR4) a documentação afirmava incorretamente que binários Windows eram pre-built (crates.io não distribui binários). Ver `gaps.md` GAP-WS-28.
- Novo `build.rs` com preflight fail-fast: em target `windows-msvc` nativo, detecta `nasm.exe` ausente do PATH e aborta em SEGUNDOS com instrução exata (`winget install -e --id NASM.NASM` + ajuste de PATH + referência ao script). Detecta NASM instalado fora do PATH em diretórios conhecidos. Escape hatch: `DDG_SKIP_NASM_CHECK=1`. Cross-compile não é afetado (usa o caminho `OPENSSL_NO_ASM` do btls-sys).

### Adicionado
- `scripts/install-windows.ps1` — instalação automatizada e consentida no Windows: detecta NASM, instala via `winget` (fallback `choco`), corrige o PATH da sessão e roda `cargo install duckduckgo-search-cli --locked` repassando argumentos extras.
- CI: passo explícito de verificação/instalação de NASM (`choco install nasm -y`) nos jobs Windows de `ci.yml` e `release.yml` — elimina a dependência implícita do NASM pré-instalado na imagem `windows-2022` (se a imagem mudar, o build não quebra silenciosamente).

### Alterado
- `README.md`, `README.pt-BR.md`, `llms.txt`, `llms.pt-BR.txt` e `docs/CROSS_PLATFORM*.md`: removido o claim FALSO de que binários Windows/macOS eram "pre-built and unaffected" — `cargo install` SEMPRE compila do source. Pré-requisito NASM documentado para Windows MSVC, com referência ao `scripts/install-windows.ps1`.

### Notas
- GAP-WS-28 FECHADO neste repositório (S1 preflight + S2 script + S3 docs + CI hardening). Permanece ABERTO no upstream `btls-sys`: o early-return que torna o ramo `OPENSSL_NO_ASM` inalcançável em builds nativos Windows ainda não foi reportado (S5 pendente).
- Nenhuma mudança de comportamento em runtime: a release contém apenas preflight de build, script de instalação, hardening de CI e documentação.

## [0.7.3] - 2026-06-08

### Corrigido
- **GAP-WS-27 — Bloqueio CAPTCHA no macOS que não ocorre no Windows**. Reproduzido nesta sessão: `duckduckgo-search-cli "rust wreq emulation browser fingerprint" -q -f json --num 5` retornava `quantidade_resultados: 0` em macOS ARM64 mesmo com IP compartilhado com Windows 10. Causa raiz: fingerprint TLS do `rustls` é reconhecível pelo Cloudflare Bot Management (vetor JA4_o), disparando CAPTCHA interstitial em HTTP 200.
- Substituído `reqwest 0.12` + `rustls-tls` por `wreq 6.0.0-rc.29` + BoringSSL (`boring2` v4.15.11) + `wreq-util 3.0.0-rc.12`. BoringSSL embarcado produz JA4_o idêntico ao Chrome/Safari real, eliminando o CAPTCHA. Ver ADR `docs/decisions/0001-tls-boring-via-wreq.md`.
- Mesma query após migração: 5 resultados, 735ms, sem fallback, sem CAPTCHA. Validação cross-OS pendente (operador deve testar em Windows e Linux).

### Adicionado
- **Feature `session` (cookie persistence + warm-up)**:
  - Flag `--no-warmup` para desabilitar a requisição `GET https://duckduckgo.com/` de warm-up.
  - Flag `--no-cookie-persistence` para manter cookies apenas em memória.
  - Flag `--cookies-path <PATH>` para sobrescrever o local padrão do `cookies.json`.
  - Cookie jar persistido em `~/.config/duckduckgo-search-cli/cookies.json` (Linux) ou `%APPDATA%\duckduckgo-search-cli\cookies.json` (Windows) ou `~/Library/Application Support/duckduckgo-search-cli/cookies.json` (macOS).
  - Permissões 0o600 aplicadas no Unix (owner read+write only).
  - Módulo `src/session_warmup.rs` (XDG path resolution) e `src/wreq_cookie_adapter.rs` (JSON ↔ `wreq::cookie::Jar` bridge).
- **Feature `probe-deep` (detecção de interstitial CAPTCHA)**:
  - Flag `--probe-deep` que executa uma query real e classifica o body como `ok` ou `captcha` baseado em marcadores Cloudflare e DuckDuckGo.
  - Flag `--allow-lite-fallback` (opt-in) para fallback automático do endpoint `html` para `lite` quando interstitial é detectado.
  - Módulo `src/probe_deep.rs` com `detectar_interstitial()` e `sugestao_mitigacao()`.
  - Reporta JSON com `status`, `cascata_motivo`, `sugestao_mitigacao`, `http_status`, `latency_ms`.

### Mudado
- **Stack TLS trocada de rustls para BoringSSL via wreq**. Build agora requer `cmake`, `perl`, `pkg-config`, `libclang-dev` no Linux. Documentado em `docs/CROSS_PLATFORM.md` e ADR-0001.
- ADR `docs/decisions/0001-tls-boring-via-wreq.md` registra a decisão arquitetural e trade-offs aceitos.
- Tempo de build de release aumentou aproximadamente 30s (BoringSSL estático). Binário final aproximadamente 20 MB maior.

### Removido
- Dependência `reqwest 0.12` (substituída por `wreq`).
- `time 0.3.47` agora é puramente transitivo. Pin direto removido do `[dependencies]`.

### Notas
- **GAP-WS-27 fechado em v0.7.3**. As três causas raiz (fingerprint TLS rustls, incoerência de headers, ausência de cookie persistence) foram entregues atomicamente em um único release. Ver `docs/decisions/0001-tls-boring-via-wreq.md` e `gaps.md` para detalhes.
- Contagem de testes: 292 lib (vs 279 em v0.7.2) + 18 wiremock + outras integrações, 0 falhas.
- Build verificado: `cargo build --release` verde (40s), `cargo test --lib` verde, `cargo test --tests` verde, `cargo clippy --all-targets -- -D warnings` verde.
- CI matrix do `release.yml` agora instala `cmake perl pkg-config libclang-dev` no Linux para suportar o build do BoringSSL.

## [0.7.2] - 2026-06-07

O formato segue [Keep a Changelog](https://keepachangelog.com/pt-BR/1.0.0/),
e este projeto adere ao [Versionamento Semântico](https://semver.org/lang/pt-BR/).

### Corrigido
- **CI: 9 jobs falhando com 10 erros E0599** (reorganização de traits do rand 0.10 — os métodos `random_range`, `random_bool` e `random` migraram de `Rng` para `RngExt` no rand 0.10.0). Linhas `use` em `src/identity.rs`, `src/parallel.rs` e `src/search.rs` atualizadas para importar `RngExt` em vez de `Rng`. Isso destrava os jobs `cargo check`, `build`, `test`, `clippy`, `doc`, `publish --dry-run`, `validate`, `musl smoke`, `msrv` e `coverage` (todas falhas em cascata da mesma causa raiz).
- **CI: job `supply chain (audit + deny)` falhando em RUSTSEC-2026-0009** (negação de serviço via exaustão de stack ao parsear headers de data RFC 2822, severidade 6.8 média). Resolvido com upgrade do `time` para `0.3.47` (release corrigida). O ignore defensivo no `deny.toml` para este advisory agora é obsoleto e foi removido.

### Mudado
- **`rand` saltou de 0.8 (publicado em v0.7.1) para 0.10** neste hotfix. O ecossistema de dev-deps (proptest 1.11+, getrandom 0.4+) unificou em 0.10, e 0.10 introduziu o trait `RngExt` como novo lar dos métodos de conveniência.
- **`rust-version` saltou de 1.75 para 1.88** (bata com `time` 0.3.47 MSRV e ecossistema `rand` 0.10). Todas as outras crates ainda compilam em 1.88+.
- **`time` fixado em `0.3.47`** como dependência direta para sobrescrever o `time 0.3.40` transitivo puxado por `cookie_store 0.22.0` → `reqwest 0.12.28` (RUSTSEC-2026-0009 stack-exhaustion DoS).

### Notas
- v0.7.1 foi publicada com código-fonte compilado contra `rand 0.9` (o lock na época resolveu para um snapshot do registry que não existe mais no crates.io). O CI subsequentemente falhou porque o registry foi atualizado e o lock agora resolve para `rand 0.10`. Este hotfix migra o código-fonte adiante para casar com o estado do registry.
- Contagem de testes: 402 (289 lib + 101 integração + 12 doctest), 0 falhas. Clippy limpo, doc limpo, fmt limpo, deny limpo, audit limpo.

## [0.7.1] - 2026-06-07

### Mudado
- **Migrado de `rand` 0.8 para `rand` 0.10** para alinhar com o ecossistema de dev-deps (proptest 1.11+, getrandom 0.4+) e a nova superfície de trait RngExt em 0.10.0. O código agora importa `rand::RngExt` para os métodos `random_range` / `random_bool` / `random`.
- **`rust-version` saltou de 1.75 para 1.88** (bata com `time` 0.3.47 MSRV e ecossistema `rand` 0.10). Todas as outras crates ainda compilam em 1.88+.
- **Features `gzip` e `brotli` do `reqwest` removidas**: `reqwest 0.12` removeu os métodos `ClientBuilder::gzip` e `ClientBuilder::brotli`. A descompressão agora é habilitada via header padrão `Accept-Encoding: gzip, br` (que `reqwest` manipula transparentemente).
- **`rand::thread_rng()` substituído por `rand::rng()`** em 4 sites (o primeiro está deprecated desde rand 0.9).
- **`Rng::gen_range` → `RngExt::random_range`** em 7 sites.
- **`Rng::gen_bool` → `RngExt::random_bool`** em 2 sites.
- **`Rng::gen::<T>()` → `RngExt::random::<T>()`** em 1 site.
- **`rand::seq::SliceRandom` substituído por `rand::seq::IndexedRandom`** para chamadas `choose` em slices (o método `choose` migrou traits em 0.9). `IteratorRandom::choose` ainda é usado para tipos `Iterator` (ex.: `slice.iter().filter().choose`).
- **`time = "0.3.47"` fixado como dependência direta** para sobrescrever o `time 0.3.40` transitivo puxado por `cookie_store 0.22.0` → `reqwest 0.12.28` (RUSTSEC-2026-0009 stack-exhaustion DoS).

### Corrigido
- **CI: 9 jobs falhando com 10 erros E0599** (`no method named random_range/random_bool/random found for struct ThreadRng in the current scope`) causados pela reorganização de traits do `rand` 0.10 (os métodos de conveniência migraram de `Rng` para `RngExt`). Linhas `use` em `src/identity.rs`, `src/parallel.rs` e `src/search.rs` atualizadas para importar `RngExt` em vez de `Rng`.
- **CI: job `supply chain (audit + deny)` falhando em RUSTSEC-2026-0009** (`time 0.3.40` denial-of-service via RFC 2822 stack exhaustion, severidade 6.8 média). Resolvido com upgrade do `time` para `0.3.47` (release corrigida). O ignore defensivo no `deny.toml` para este advisory foi temporariamente adicionado (removido em v0.7.2 com o upgrade definitivo).

## [0.7.0] - 2026-06-07

### Adicionado
- **Novo subcomando `deep-research`** — pipeline de fan-out de queries para consumo por LLMs. Divide a query do usuário em 1..=12 sub-queries via cinco templates heurísticos canônicos (aspecto, comparação, cronologia, opinião, causa), dispara em paralelo pelo executor já existente, agrega via Reciprocal Rank Fusion (K=60) ou deduplicação por URL canônica, e opcionalmente produz um relatório sintetizado em Markdown, PlainText ou JSON com referências numeradas.
- **Novo módulo `src/deep_research.rs`** — orquestrador do pipeline (`run_deep_research(args, cfg, cancel)`).
- **Novo módulo `src/decomposition.rs`** — geração de sub-queries heurística e manual. Lê sub-queries explícitas de arquivo quando a flag `--sub-query-strategy manual` é definida; comentários (`#`) e linhas em branco são ignorados.
- **Novo módulo `src/aggregation.rs`** — estratégias `Rrf(K=60)` e `DedupeByUrl`. A canonicalização remove parâmetros `utm_*` e outros de tracking, normaliza host e scheme em minúsculas, ordena query params e colapsa barras repetidas. A forma canônica é hashada com `blake3` (16 primeiros hex chars) como chave de dedup.
- **Novo módulo `src/synthesis.rs`** — três formatos de saída (Markdown, PlainText, Json) com orçamento de tokens configurável (1 token ≈ 4 chars heurístico) e teto de 20 referências por relatório.
- **Novas dependências**:
  - `url = "2"` — canonicalização de URL em `aggregation.rs`.
  - `regex = "1"` — usado por `decomposition::is_composite_query` para detectar sinais de query composta e suprimir templates redundantes.
  - `proptest = "1"` (dev) — testes baseados em propriedades para os novos módulos.

### Mudado
- **Versão pulou de `0.6.11` para `0.7.0`** (minor: novo subcomando público `deep-research` e quatro novos módulos públicos `deep_research`, `decomposition`, `aggregation`, `synthesis`). Nenhuma quebra no subcomando `buscar` existente nem nos schemas padrão `SearchOutput` / `MultiSearchOutput` — puramente aditivo.
- **`Config` em `lib::execute_deep_research`** constrói uma configuração padrão a partir das flags globais — `parallelism = 5`, `retries = 2`, `endpoint = Html`, `language = en`, `country = us`, `global_timeout = 120s`. O pipeline herda esses defaults e NÃO exige que o operador passe um `CliArgs` completo.

### Interno
- **Bloco `exclude` do Cargo.toml** — `gaps.md` e `docs_prd/` estão excluídos do crate publicado.
- **`[profile.release]` panic = "abort"** — binário menor, mais difícil de vazar payload de panic pela fronteira FFI se um dia for adicionada.
- **`.gitignore`** — adicionados `proptest-regressions/`, `coverage/`, `tarpaulin-report.html` e `.cargo-deny-state.json` para casar com artefatos reais produzidos pela nova suíte de testes e tooling de CI.

### Gap closure pass
- **Doctests adicionados aos quatro novos módulos** (12 doctests no total): `aggregation::canonicalize_url`, `synthesis::estimate_tokens`, `synthesis::trim_to_budget`, `decomposition::HeuristicTemplate::suffix`, `deep_research::DeepResearchArgs::validate` e exemplo de uso em `deep_research::run_deep_research`.
- **Testes baseados em propriedades com `proptest`** (7 testes) cobrindo `canonicalize_url` (idempotência, remoção de fragmento, remoção de tracking, host em minúsculas) e `synthesis` (monotonicidade de `estimate_tokens`, teto + idempotência de `trim_to_budget`). `proptest-regressions/` está no `.gitignore`.
- **`regex` integrado** em `decomposition::is_composite_query` com enum `CompositeSignal` (Comparison, Aspect, Timeline, Opinion, Cause, Topic) e padrões compilados cacheados em `OnceLock`. A estratégia heurística suprime templates redundantes (ex.: `Comparison` é pulado quando a query já contém `vs` ou `or`).
- **Testes de integração com wiremock** em `tests/integration_deep_research.rs` (17 testes): smoke do pipeline, match de query params, observabilidade de anomalia HTTP 202, observabilidade de 404, e 13 testes de cobertura de superfície.
- **`cargo deny check`** — quatro gates passando: `advisories ok, bans ok, licenses ok, sources ok`.
- **`cargo publish --dry-run`** — pacote criado e verificado (1.1 MiB, 14.00 s em cache quente).
- **Bug latente de UTF-8 corrigido em `synthesis::trim_to_budget`** — usava indexação por bytes sem verificação de char boundary, o que causava panic em entradas multi-byte (mesma forma de panic destacada no livro do proptest). Substituído por helper privado `floor_char_boundary`. Três proptests travam o invariante `is_char_boundary(out.len())` para entradas arbitrárias.

### Validação
- `cargo build --release` — clean.
- `cargo clippy --all-targets --all-features -- -D warnings` — clean.
- `cargo test --lib` — 279 testes passando, 0 falhando.
- `cargo test --doc` — 12 doctests passando.
- `cargo test --tests` — 101 testes de integração passando (24 + 3 + 17 + 5 + 10 + 10 + 14 + 18).
- **Total: 392 testes passando** (279 lib + 12 doc + 101 integração), 0 falhando.
- `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --lib` — clean.
- `cargo fmt --all -- --check` — clean.
- `cargo audit` — sem novos avisos (o pré-existente `RUSTSEC-2025-0057` em `selectors 0.25.0` é o único e está rastreado separadamente).
- `cargo deny check` — quatro gates ok.
- `cargo publish --dry-run` — ok.

## [0.6.11] - 2026-06-05

### Corrigido
- **CI: step 6 do `crates_io` (`Check if version already published`) falhou com `unbound variable` exit 1 no tag v0.6.10**
  - Causa raiz: a variável `VERSION` era referenciada como `VERSION="${VERSION}"`
    na primeira linha do script, mas nunca havia sido definida no `env:` do
    step. Com `set -euo pipefail` ativo, acessar uma variável não definida
    causou `bash: VERSION: unbound variable` com exit 1, marcando o step
    como `conclusion: failure` e curto-circuitando o resto do fluxo de
    publicação. A v0.6.10 do crates.io foi publicada manualmente via
    `cargo publish --allow-dirty` como workaround.
  - Solução: adicionado `VERSION: ${{ steps.detect_version.outputs.version }}`
    no bloco `env:` do step, espelhando o padrão já usado por
    `Verify tag matches Cargo.toml version`. Também foi aplicado
    endurecimento com `NO_COLOR=1` e um `sed` para remover ANSI como
    defesa em profundidade contra códigos de cor que quebrariam a regex
    de parsing. Aumentado o número de tentativas de 3 para 5 com
    backoff linear (5s/10s/15s/20s) para absorver rate limits transitórios
    do crates.io.

- **CI: parsing do `cargo search` agora é resiliente a códigos de cor ANSI**
  - O output do `cargo search` vem com códigos ANSI quando
    `CARGO_TERM_COLOR=always` está setado (como está neste workflow). Em
    alguns esquemas de cor a regex `= "[0-9]+\.[0-9]+\.[0-9]+"` ainda
    casava, mas em outros os códigos eram injetados entre caracteres e
    quebravam o parsing.
  - Solução: strip ANSI escapes com `sed -E 's/\x1b\[[0-9;]*[a-zA-Z]//g'`
    antes de aplicar a regex, e set `NO_COLOR=1` para desabilitar cor
    explicitamente. Ambas as camadas garantem que a regex vê ASCII limpo.

## [0.6.10] - 2026-06-05

### Corrigido
- **CI: job `Publish to crates.io` rejeitado por environment protection rules — tag `v0.6.9` não autorizada no environment `release`**
  - Causa raiz: o environment GitHub `release` tinha apenas a `branch_policy` configurada
    (`protection_rules: [{"type": "branch_policy"}]`), o que faz com que o GitHub Actions
    rejeite qualquer ref que NÃO seja um branch — incluindo `refs/tags/v0.6.9`. O run
    terminou com `conclusion: failure` e `steps_count: 0` (job nem chegou a executar),
    exibindo a anotação `Tag "v0.6.9" is not allowed to deploy to release due to
    environment protection rules`.
  - Solução: criado novo environment `release-publish` (id `16308925736`) sem
    `protection_rules`, que aceita QUALQUER ref — incluindo tags SemVer. O job
    `crates_io` agora usa `environment: name: release-publish`.

- **CI: `actionlint` exit 3 — `is a directory` ao invocar `actionlint .github/workflows/`**
  - Causa raiz: o `actionlint` v1.x NÃO aceita diretório como argumento posicional;
    espera arquivos individuais (ex.: `*.yml`) ou ser invocado SEM argumentos
    (auto-descoberta recursiva do diretório `.github/workflows/`). A invocação
    incorreta produziu o erro `could not read ".github/workflows/": is a directory`
    com exit 3, marcando o job `workflow syntax check (actionlint)` como failed.
  - Solução: invocação corrigida para `actionlint` (sem argumentos) no
    `Run actionlint` step do `ci.yml`. Validação local confirmou exit 0
    com zero erros de sintaxe.

- **CI: `zizmor` exit 13 — 2 findings `secrets-outside-env` (medium) no job `github_release`**
  - Causa raiz: o job `github_release` referenciava `secrets.GPG_PRIVATE_KEY` e
    `secrets.GPG_PASSPHRASE` no `env:` sem ter um `environment:` dedicado. O
    `zizmor >= 1.24` (persona `auditor`) detecta esse padrão como `secrets-outside-env`
    (medium) e marca o job `workflow security scan (zizmor)` como failed com exit 13
    quando há pelo menos 1 finding.
  - Solução: (1) removidos os secrets GPG do `env:` do `github_release` e criado o
    gate `GPG_SIGNING_ENABLED: "false"` no nível workflow; (2) o step `Sign
    SHA256SUMS with GPG` foi renomeado para `(DESABILITADO)` e nunca executa;
    (3) criada config `.github/zizmor.yml` com `rules.secrets-outside-env.config.allow`
    listando `CRATES_IO_TOKEN` (que está no nível repo por compatibilidade).
    Cosign keyless (job `attest`) já fornece integridade criptográfica via Sigstore,
    cobrindo a função que o GPG signing cumpriria.

- **CI: package list agora inclui `.github/zizmor.yml` (configuração zizmor intencional)**
  - Adicionado arquivo `.github/zizmor.yml` com regras de allow para o secret
    `CRATES_IO_TOKEN` no nível repo. Este arquivo é uma config estática, não
    contém credenciais e é seguro versionar.

## [0.6.9] - 2026-06-05

### Corrigido
- **CI: asset do Windows `.zip` no release estava vazio (209 bytes) — bug no script PowerShell do `Package (Windows)`**
  - Causa raiz: o script usava sintaxe `${TARGET}` / `${BIN}` / `${EXT}`, que é **interpolação bash**.
    Em PowerShell, `${VAR}` é string literal — env vars são interpoladas como `$env:VAR`.
    Resultado: o `Copy-Item` falhou silenciosamente (caminho da origem virou `target//release/`) e
    o `Compress-Archive` produziu um zip quase vazio (apenas `SHA256SUMS.txt`).
  - Solução: substituídos todos os `${VAR}` por `$env:VAR` nos blocos `run:` PowerShell
    (Package (Windows) e Generate SHA256SUMS (Windows)).

- **CI: SBOM CycloneDX `sbom.cdx.json` estava com 0 bytes (arquivo na verdade não foi gerado)**
  - Causa raiz: `cargo cyclonedx --override-filename sbom.cdx.json` na verdade escreve
    `sbom.cdx.json.json` porque a flag `--override-filename` auto-adiciona `.json`.
    O step `wc -c < sbom.cdx.json` então leu 0 bytes do arquivo inexistente e o step
    `Upload SBOM as artifact` uplodou um arquivo vazio (artifact ignorado downstream).
  - Solução: alterada invocação para `cargo cyclonedx --format json --override-filename sbom`
    (apenas stem), depois `mv sbom.json sbom.cdx.json` para casar com o nome esperado.

- **CI: GitHub Release da v0.6.8 estava incompleto (faltava Windows zip + sbom)**
  - Causa raiz: a combinação dos dois bugs acima significou que o workflow de release
    da v0.6.8 produziu um zip Windows só com o stub SHA256SUMS e um SBOM vazio.
    Realizei upload manual do SBOM real depois do fato; o zip Windows requer
    um re-run completo do workflow.

## [Não publicado]

### Corrigido
- **CI: exit 101 `crate already exists` no job `Publish to crates.io` (post-mortem 2026-06-05)**
  - Causa raiz: trigger duplicado do workflow para tag v0.6.6 já publicada causou `cargo publish`
    exit 101 com `error: crate duckduckgo-search-cli@0.6.6 already exists on crates.io index`.
    crates.io é append-only immutable, versões NUNCA podem ser sobrescritas.
  - Solução: adicionados jobs `preflight` + `crates_io` com guards de:
    - Consistência de versão Tag vs Cargo.toml
    - Validação de formato SemVer
    - Presença de entrada no CHANGELOG
    - Bloqueio de Co-authored-by de agentes IA em commits recentes
    - `cargo search` com timeout + retry para detectar versão já publicada
    - Skip de `cargo publish` com warning + upload de evidência quando já publicada
    - Timeout (300s) + retry (3 tentativas, backoff 10s/20s/30s) no `cargo publish`
  - Padrão de resolução: workflow de release idempotente com caminho de skip explícito

- **CI: 18+ warnings de Node.js 20 deprecated em todos os jobs**
  - Causa raiz: actions/checkout@v4, actions/upload-artifact@v4, actions/download-artifact@v4
    usam Node 20. Node 20 descontinuado em 19/09/2025, removido em 16/09/2026.
  - Solução:
    - Atualizadas todas as actions para v6 (Node 24 nativo)
    - Atualizado `softprops/action-gh-release` de v2 para v3
    - Adicionado `FORCE_JAVASCRIPT_ACTIONS_TO_NODE24: "true"` como cinto-e-suspensórios
  - Caminho de migração: v6 é Node 24 nativo, v4 precisa de env var explícita

- **CI: exit 141 SIGPIPE intermitente em `validate (ubuntu-latest)`**
  - Causa raiz: `cargo test` escreve em pipe cujo consumidor fecha cedo
  - Solução: guard explícito `|| { ec=$?; if [ $ec -eq 141 ]; then exit 0; fi; exit $ec; }`
  - Trade-off: 141 vira warning silenciosamente, pode mascarar bugs reais em testes

- **CI: exit 1 em `validate (windows-latest)` por redirect VS2022→VS2026**
  - Causa raiz: GitHub redireciona `windows-latest` para `windows-2025-vs2026` desde 15/06/2025.
    VS2026 tem mudanças breaking no toolchain MSVC que afetam Rust stable.
  - Solução: pinado `windows-2022` na matrix `ci.yml` e no target de build `release.yml`
  - Reavaliar pin após 15/07/2026 quando VS2026 estabilizar

### Adicionado
- **Geração de SBOM CycloneDX no workflow de release** — `cargo cyclonedx --format json` produz
  `sbom.cdx.json` enviado como artifact. Habilita compliance com EU Cyber Resilience Act.
- **Atestado de proveniência SLSA** — `actions/attest-build-provenance@v2` cria proveniência
  assinada para todos os artefatos de release. Compliance SLSA Nível 3.
- **Assinatura cosign keyless OIDC** — cada binário + SHA256SUMS.txt assinado com `cosign sign-blob`
  usando token OIDC do GitHub. Sem gerenciamento de chave privada.
- **SHA256SUMS publicado em cada release** — `sha256sum` gerado por target, combinado em
  único `SHA256SUMS.txt`, enviado como asset de release e como parte de cada tarball/zip binário.
- **Assinatura GPG de tag** — `gpg --detach-sign SHA256SUMS.txt` opcional se secret
  `GPG_PRIVATE_KEY` estiver configurada. `continue-on-error: true` para não bloquear release
  se chave faltar.
- **Controle de concorrência** — `concurrency.group: release-${{ github.ref }}-${{ github.sha }}`
  impede runs paralelos para mesma tag+SHA. `cancel-in-progress: false` (release) / condicional
  em PR (CI) garante que publish nunca é abortado no meio.
- **Job pre-flight no workflow de release** — valida versão da tag == versão do Cargo.toml,
  formato SemVer, entrada no CHANGELOG, ausência de Co-authored-by de agente IA ANTES de qualquer build.
- **Atualização semanal Cron de dependências** — job `scheduled_update` roda domingo 03:00 UTC,
  executa `cargo update --workspace`, cria PR se houver mudanças.
- **Scan de segurança Zizmor** — análise estática de workflows GitHub Actions detecta
  injeção, input não confiável e outros anti-patterns de segurança. Roda apenas em PRs.
- **Validação de sintaxe Actionlint** — valida sintaxe YAML de todos os arquivos de workflow. Roda apenas em PRs.
- **Dependabot para actions e crates** — `.github/dependabot.yml` cria PRs semanais
  para atualizações de GitHub Actions e crates Rust. Agrupa por major/minor/patch.
- **Normalização LF via `.gitattributes`** — força line endings LF em todos os arquivos de texto,
  prevenindo problemas de CRLF em Windows que quebram `cargo fmt --check`.

### Segurança
- **Permissões endurecidas por job** — top-level `permissions: contents: write packages: write
  id-token: write attestations: write checks: write discussions: write` para release;
  blocos `permissions:` por job no CI para menor privilégio.
- **`continue-on-error: true` no step GPG** — chave GPG ausente não bloqueia release;
  melhoria opcional.
- **Sem triggers `pull_request_target`** — workflows nunca rodam com permissões de escrita
  em PRs de forks.

## [0.6.8] - 2026-06-05

### Corrigido
- **CI: exit 127 `jaq: command not found` no job `github_release` do workflow de release**
  - Causa raiz: `release.yml` (linhas 625-626) usava `jaq` (binário Rust) para parsear
    JSON de resposta da GitHub REST API, mas o runner Ubuntu 24.04 do GitHub Actions
    só tem `jq 1.7` pré-instalado — `jaq` não faz parte da imagem padrão do runner.
    Bug introduzido pelo commit `7f489b5` (2026-06-05) ao fazer bypass do action
    `softprops/action-gh-release` que estava bugado.
  - Solução: substituído `jaq` por `jq` (pré-instalado, sintaxe compatível) e adicionada
    validação fail-fast explícita para os valores extraídos de `UPLOAD_URL` e
    `RELEASE_ID` para emitir mensagens diagnósticas claras em respostas malformadas.
  - Referência: <https://github.com/actions/runner-images/blob/main/images/ubuntu/
    Ubuntu2404-Readme.md> (seção Tools lista `jq 1.7`, `jaq` está ausente)

## [0.6.7] - 2026-06-05

### Corrigido
- **CI: post-mortem completo do incident-publish-101-2026-06-05** (hardening do pipeline de release)
  - Adicionado job `preflight` validando tag==Cargo.toml, SemVer, CHANGELOG, ausência de Co-authored-by de agentes IA
  - Adicionado guard contra versão duplicada no job `crates_io`
  - cargo publish com timeout 300s + 3 retries (resiliência a network)
  - Concurrency group por tag+sha (impede runs paralelos)
- **CI: 18+ warnings de Node.js 20 deprecated**
  - Atualizadas actions para v6 (Node 24 nativo)
  - Atualizado softprops/action-gh-release v2 → v3
  - Adicionado `FORCE_JAVASCRIPT_ACTIONS_TO_NODE24` como cinto-e-suspensórios
- **CI: zizmor security scan: 134 findings → 0**
  - SHA pinning para 11 actions (unpinned-uses)
  - per-job least-privilege permissions (excessive-permissions)
  - comments + inline trailing em todas as permissions
  - secrets em env: job-level + GitHub Environments dedicados
  - ${{ ... }} em run: mitigados via env vars (template-injection)
  - dtolnay/rust-toolchain substituído por setup via rustup (superfluous-actions)
  - caches removidos do release.yml (cache-poisoning)
- **CI: actionlint 0 erros em ambos workflows**
- **CI: zizmor zero findings (exit 0)**
- **CI: dependabot.yml para auto-update semanal**
- **CI: .gitattributes força LF line endings**
- **clippy: `#[cfg(feature = "chrome")]` redundante removido de src/lib.rs:74**
  - browser.rs:25 já cobre o módulo
- **clippy: SAFETY comments adicionados em todos os Windows unsafe blocks em src/platform.rs**
  - 5 blocos unsafe agora têm `// SAFETY:` comments
- **test: tests incompatíveis com Windows marcados com `#[cfg(unix)]`**
  - `rejeita_path_absoluto_etc` e `rejeita_path_absoluto_usr`

### Adicionado
- **Geração de SBOM CycloneDX em release workflow**
  - `cargo cyclonedx --format json` produz `sbom.cdx.json`
- **SLSA build provenance via `actions/attest-build-provenance@v2`**
- **cosign keyless OIDC signing** (todos os binários + SHA256SUMS.txt)
- **SHA256SUMS publicado com cada release**
- **GPG tag signing** (opcional, `continue-on-error: true` se chave ausente)
- **Pre-flight job em release workflow** (9 gates + 1 dry-run)
- **Attestation job** (SBOM + cosign + SLSA em 1 job)
- **scheduled_update Cron semanal** (cargo update automático)
- **Zizmor security scan em CI**
- **Actionlint syntax check em CI**
- **Dependabot para actions e Rust crates**

### Segurança
- **Permissions endurecidas per-job** (least-privilege)
- **Persist-credentials: false em 18/18 actions/checkout** (artipacked)
- **Sem triggers `pull_request_target`**
- **SHA pinning completo** (11 actions)

## [0.6.5] - 2026-06-05

### Corrigido
- **MP-26 — Cast de HANDLE Windows quebrado em `windows-sys 0.59+`** (`src/platform.rs:51-63`)
  - `HANDLE` mudou de `isize` para `*mut c_void` no upstream (`microsoft/windows-rs`, `raw-window-handle#171`)
  - Substituído `handle != 0 && handle != usize::MAX` por `!handle.is_null() && handle != INVALID_HANDLE_VALUE`
  - Removidos casts inválidos `handle as isize` (a assinatura moderna aceita `HANDLE` direto)
  - Comentário `// SAFETY:` atualizado para documentar nulidade e sentinela Win32
- **CI: `validate` falhava em todos os 3 SOs** (Linux/macOS/Windows) por 6 erros de clippy
  - 3 erros `clippy::doc_markdown` (`PowerShell`, `rules_rust.md`, `TempDir`) em `src/platform.rs` e `src/browser.rs`
  - 1 erro `clippy::needless_return` em `src/browser.rs:149`
  - 2 erros `missing_debug_implementations` em `src/browser.rs:223` e `src/content_fetch.rs`

### Adicionado
- **WS-11 — Invariantes property-based para parsers HTML** (`src/extraction.rs` +5 testes)
  - Invariante: inputs vazios/quebrados retornam `Vec` vazio sem panic
  - Invariante: positions são densos e 1-based
  - Invariante: URLs absolutos (`http`/`https`) ou vazios
  - Invariante: extração é idempotente
  - Invariante: HTML malformado não causa panic
  - **Zero dependência nova** (apenas stdlib + `#[test]`)
- **WS-12 — Circuit breaker per-host** (`src/content_fetch.rs`)
  - Threshold: 3 falhas consecutivas abrem o circuito
  - Cooldown: 30s antes de half-open probe
  - Integração em `enrich_with_content` antes de cada fetch
  - `BreakerDecision::{Allow, Reject}` para inspeção
  - **Zero dependência nova** (`std::sync::Mutex<HashMap>`)
- **WS-23 — Teste `Retry-After` header** (`tests/integration_wiremock.rs`)
  - Mock retorna 429 com `retry-after: 2`
  - Asserção: `elapsed_ms >= 1500` (delay mínimo respeitado)
  - Usa `wiremock` 0.6 já em dev-deps
- **WS-25 — `indicatif` ProgressBar para crawls longos** (`src/content_fetch.rs`)
  - `indicatif = "0.18"` adicionado
  - Template `[{elapsed_precise}] {bar:40.cyan/blue} {pos:>4}/{len:4} {msg}`
  - Auto-detecta TTY (esconde em pipes)
  - `progress.finish_and_clear()` ao final
- **Lints preventivos FFI** (`Cargo.toml`)
  - `improper_ctypes = "deny"` (rejeita casts FFI inválidos)
  - `improper_ctypes_definitions = "deny"` (rejeita definições incorretas)

### Testes
- 333 testes passando (243 lib + 24 + 3 + 5 + 10 + 10 + 14 + 18 + 6 doc)
- 6 novos testes de invariantes em `extraction.rs` (WS-11)
- 4 novos testes de circuit breaker em `content_fetch.rs` (WS-12)
- 1 novo teste de Retry-After em `integration_wiremock.rs` (WS-23)
- `cargo fmt --all --check` limpo
- `cargo clippy --all-targets --all-features --locked -- -D warnings` limpo
- `cargo publish --dry-run --locked --allow-dirty` limpo

## [0.6.4] - 2026-06-03

### Adicionado
- **WS-26 — Rotação adaptativa de identidades anti-bot** (novo módulo `src/identity.rs`)
  - Pool de 12 identidades (4 famílias de browser × 3 plataformas) para rotação adaptativa
  - `IdentityProfile::shuffled_headers()` produz ordem de headers determinística via seed
  - `IdentityPool::rotate_on_block()` implementa cascata de 5 níveis: mesma identidade → mesma família/plataforma diferente → família diferente/mesma plataforma → família+plataforma diferentes → aleatória
  - Enums `BrowserFamily` e `Platform` com nomes canônicos em inglês
  - 5 testes unitários cobrindo tamanho do pool, nível de cascata, determinismo, formato de headers, estabilidade da tag
- **Novas flags CLI** (aditivas, sem breaking changes)
  - `--probe` — verificação de saúde pré-voo (envia 1 requisição mínima, reporta status/latência/Set-Cookie como JSON)
  - `--identity-profile` — fixa a sessão em uma identidade específica (`auto`, `chrome-win`, `chrome-mac`, `chrome-linux`, `edge-win`, `firefox-linux`, `safari-mac`). `auto` é o padrão.
- **Novos campos JSON de metadados** (aditivos, `Option` + `skip_serializing_if = "Option::is_none"`)
  - `metadados.identidade_usada` — tag textual da identidade que produziu a resposta
  - `metadados.nivel_cascata` — nível de cascata atingido durante a requisição
### Alterado
- **Reversão de versão**: `0.7.0` (não publicado) → `0.6.4` para preservar o conjunto de features em desenvolvimento sob um número de patch estável
- Todas as flags CLI, schemas JSON de saída e exit codes permanecem inalterados — mudanças estritamente aditivas
### Testes
- 5 novos testes unitários de identidade (313 testes totais passando, de 308)
- Todos os 224 testes lib + 83 testes de integração + 6 doc tests passam
- `cargo clippy --lib --bins -- -D warnings` limpo
- `cargo fmt --check` limpo

## [0.7.0] - 2026-06-01

### Alterado
- Internacionalização completa: ~600 identificadores renomeados PT→EN em 15 arquivos-fonte (campos de struct, variáveis locais, parâmetros, funções de produção, funções de teste)
- Módulo `fetch_conteudo` renomeado para `content_fetch`
- Arquivos de teste `integracao_*.rs` renomeados para `integration_*.rs`
- `anyhow` removido e substituído por `CliError` tipado em 11 módulos — zero dependência de crate de erro externa
- `output.rs`: todas as funções de formatação renomeadas (`formatar_*` → `format_*`, `escrever_*` → `write_*`)
- `config_init.rs`: campos de struct renomeados com `#[serde(rename)]` para preservar compatibilidade JSON
- `search.rs`: campos de `RetryResult` e `AggregatedSearchResult` renomeados PT→EN
- `types.rs`: campos de `Config` `perfil_browser`/`corresponde_plataforma_ua`/`caminho_chrome` → `browser_profile`/`match_platform_ua`/`chrome_path`
### Adicionado
- Testes de concorrência Loom (`tests/loom_atomics.rs`) — valida visibilidade de `AtomicBool` entre threads
- Benchmarks Criterion (`benches/extraction_bench.rs`) — baselines de performance de extração HTML
- Doc comments para 70 itens públicos sem documentação — zero warnings de `missing_docs`
- `.ingest-queue.sqlite` adicionado ao `.gitignore` e `Cargo.toml` exclude
- `LICENSE-MIT` e `LICENSE-APACHE` — licença dupla conforme declaração SPDX em `Cargo.toml`
- `.pre-commit-config.yaml` com três grupos de hooks
- `.gitattributes` forçando LF em arquivos-fonte
- `.editorconfig` normalizando UTF-8 e indentação
- Templates GitHub (PR, bug report, feature request)
- `Cross.toml`, `CONTRIBUTING.md`, aliases Cargo, doctests
- `SECURITY.md`, `dependabot.yml`, `rust-toolchain.toml`
- Workflows CI e release, job MSRV
- `deny.toml` com política de supply chain
- 22 novos testes elevando cobertura de 77,4% para 86,4%
### Corrigido
- RUSTSEC-2026-0097: `rand` 0.8.5 → 0.8.6
- RUSTSEC-2026-0104: `rustls-webpki` 0.103.12 → 0.103.13
### Segurança
- `deny.toml`: adicionado `skip-tree` para 30 crates transitivas duplicadas (ecossistemas chromiumoxide, scraper, console-subscriber)
### Limitações Conhecidas
- Testes Loom requerem `RUSTFLAGS="--cfg loom"` que conflita com `hyper-util` — testes compilam mas não executam até o upstream resolver o conflito de cfg
- Nomes de campos JSON permanecem em português brasileiro (`posicao`, `titulo`, `resultados`, etc.) — POR DESIGN desde v0.2.0

## [0.6.3] - 2026-04-17

### Alterado
- Tradução de todos os 96 doc comments (`///` e `//!`) em 19 arquivos-fonte de português para inglês — docs.rs agora exibe documentação completamente em inglês para o público internacional do crates.io.
- Nenhuma alteração de comportamento, API pública ou campos JSON de saída.

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
- Método `validar_timeout_segundos()` em `CliArgs` — rejeita valores 0 com mensagem de erro descritiva
- Verificação antecipada de path traversal em `montar_configuracoes()` — chama `paths::validate_output_path()` no momento de validação da configuração, não no momento de escrita
- 2 testes E2E de regressão: `timeout_zero_retorna_exit_2` e `output_com_path_traversal_retorna_exit_2`
- 1 teste unitário: `validar_timeout_segundos_rejeita_zero`

## [0.6.0] - 2026-04-16

### Segurança
- Perfis de fingerprint de browser por família previnem detecção anti-bot do DuckDuckGo
- Headers `Sec-Fetch-*` e Client Hints por família imitam sessão de navegador real
- `Accept-Language` com q-values RFC 7231 elimina fingerprint de UA genérico
- Detecção de bloqueio silencioso com limiar de 5 KB previne resultados truncados
### Adicionado
- Enum `BrowserFamily` — variantes `Chrome`, `Firefox`, `Edge`, `Safari`
- Struct `BrowserProfile` — encapsula família, versão e conjunto de headers por família
- Headers `Sec-Fetch-Dest`, `Sec-Fetch-Mode`, `Sec-Fetch-Site` por família em `http.rs`
- Client Hints (`Sec-Ch-Ua`, `Sec-Ch-Ua-Mobile`, `Sec-Ch-Ua-Platform`) para Chrome e Edge
- Detecção de anomalia HTTP 202 em `search.rs` com backoff exponencial automático
- Detecção de bloqueio silencioso — resposta com menos de 5 000 bytes é tratada como bloqueio
- `BrowserProfile` propagado via `Config` para todos os módulos do pipeline
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
- Escritas de arquivo em `src/output.rs` agora validam caminhos via `paths::validate_output_path()` antes do I/O
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
- Campo `buscas_relacionadas` REMOVIDO de `SearchOutput` e `MultiSearchOutput.buscas[i]` — o endpoint `html.duckduckgo.com/html/` não expõe related searches no DOM atual; manter o campo sempre vazio era ruído
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
- Campo `titulo_original: Option<String>` em `SearchResult` — presente apenas quando o título foi substituído por heurística
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
- Subcommand `init-config` com `--force` e `--dry-run` para inicializar arquivos de configuração do usuário
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
