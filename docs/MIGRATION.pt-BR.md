# Guia de Migração

Este guia cobre caminhos de migração entre versões do `duckduckgo-search-cli`.
Cada seção documenta mudanças que quebram compatibilidade, mudanças aditivas
e instruções de rollback.

## Migração v0.8.5 → v0.8.6

### O que muda
- **Stack TLS substituida (GAP-WS-066)** — `wreq` (BoringSSL) substituido por `reqwest` + `rustls-tls` (TLS puro Rust). NASM, CMake, Perl e MSVC NAO sao mais necessarios em nenhuma plataforma.
- `src/wreq_cookie_adapter.rs` renomeado para `src/cookie_adapter.rs` — persistencia de cookies reescrita para `reqwest::cookie::Jar`
- Descompressao brotli removida — DuckDuckGo nunca serve brotli para endpoints HTML
- Fallback HTTP perde emulacao de fingerprint TLS do BoringSSL — Chrome headed (primario desde v0.8.0) produz fingerprint real de navegador
- `build.rs` simplificado: todos os preflights de BoringSSL removidos (deteccao de nasm, cmake, cl, perl)
- ADR-0001 substituido por ADR-0008

### Migração passo-a-passo

```bash
# 1. Atualizar
cargo install duckduckgo-search-cli --version 0.8.6 --force

# 2. Verificar — sem erros de NASM/CMake/Perl no Windows
duckduckgo-search-cli --probe
```

### Mudancas que quebram para usuarios da biblioteca
- `wreq::Client` → `reqwest::Client` em todos os tipos publicos
- `Arc<dyn wreq::cookie::CookieStore>` → `Arc<reqwest::cookie::Jar>` em `SearchConfig`
- `wreq_cookie_adapter::PersistentJar` → `cookie_adapter::PersistentJar`
- `wreq::header::*` → `reqwest::header::*` em todos os imports

### Rollback
```bash
cargo install duckduckgo-search-cli --version 0.8.5 --force
# Nota: v0.8.5 exige NASM+CMake+Perl+MSVC no Windows
```


## Migração v0.8.4 → v0.8.5

### O que muda
- **Chrome headed dentro de Xvfb (GAP-WS-065, CRÍTICO)** — `--headless=new` (introduzido na v0.8.1) é detectado pelo Cloudflare via fingerprinting JS (`navigator.webdriver`, artefatos CDP). Chrome agora roda em modo HEADED dentro de um display virtual Xvfb privado que a CLI auto-spawna via `spawn_virtual_display()`. O usuário vê ZERO janelas.
- Nova função `spawn_virtual_display()` em `src/browser.rs` cria `Xvfb :99` com tela virtual 1920x1080
- Chrome recebe `DISPLAY=:99` via `builder.env()` — apenas o processo filho Chrome usa o display virtual
- Xvfb é encerrado automaticamente via `Drop` do `ChromeBrowser`
- Fallback: se Xvfb não estiver instalado, Chrome cai para headless (com risco de anti-bot)
- Nova env var `DUCKDUCKGO_CHROME_HEADLESS=1` para forçar modo headless explicitamente
- Novo requisito de sistema: pacote `xvfb` no Linux (`xorg-x11-server-Xvfb` no Fedora, `xvfb` no Debian/Ubuntu)

### Migração passo-a-passo

```bash
# 1. Instalar Xvfb (apenas Linux)
# Debian/Ubuntu:
sudo apt install xvfb
# Fedora:
sudo dnf install xorg-x11-server-Xvfb

# 2. Atualizar duckduckgo-search-cli
cargo install duckduckgo-search-cli --version 0.8.5 --force

# 3. Uso normal — Chrome roda headed dentro de Xvfb, ZERO janelas visíveis
duckduckgo-search-cli "test query" -q -f json --num 3

# 4. Forçar headless (não recomendado — Cloudflare detecta)
DUCKDUCKGO_CHROME_HEADLESS=1 duckduckgo-search-cli "test" -q -f json --num 3
```

### Rollback
```bash
cargo install duckduckgo-search-cli --version 0.8.4 --force
```

## Migração v0.8.3 → v0.8.4

### O que muda
- **Fix de cascade_level_observed (GAP-WS-064, BAIXA)** — `cascade_level_observed` no success path de `parallel.rs` era hardcoded como `None`. Agora usa a mesma lógica `derive_cascade_level_from_attempts` do `pipeline.rs`. Batch queries e sub-queries do deep-research agora reportam telemetria correta de nível de cascata.

### Rollback
```bash
cargo install duckduckgo-search-cli --version 0.8.3 --force
```

## Migração v0.8.2 → v0.8.3

### O que muda
- **Fix de chrome_attempted (GAP-WS-062, BAIXA)** — `chrome_attempted` em `parallel.rs` era `cfg!(feature = "chrome")` (constante de compilação). Agora verifica `DUCKDUCKGO_SEARCH_CLI_NO_CHROME=1` em runtime. Batch queries reportam `tentou_chrome: false` corretamente quando Chrome está desabilitado.
- **Fix de identity_used (GAP-WS-063, BAIXA)** — `identity_used` no success path de `parallel.rs` era hardcoded como `None`. Agora chama `identity_tag_for_cli_identity()`. Batch queries com `--identity-profile` agora reportam a identidade usada.

### Rollback
```bash
cargo install duckduckgo-search-cli --version 0.8.2 --force
```

## Migração v0.8.1 → v0.8.2

### O que muda
- **deep-research herda flags do root (GAP-WS-061, MÉDIA)** — `execute_deep_research` agora recebe `CliArgs` do comando root. Anteriormente, deep-research usava defaults hardcoded (`lang=en`, `country=us`, `num=10`, `retries=2`) ignorando flags do usuário. Agora `--num`, `--lang`, `--country`, `--endpoint`, `--retries`, `--proxy`, `--timeout`, `--parallel`, `--max-content-length`, `--identity-profile`, `--allow-lite-fallback` e `--pre-flight` propagam para sub-queries do deep-research.

### Rollback
```bash
cargo install duckduckgo-search-cli --version 0.8.1 --force
```

## Migração v0.8.0 → v0.8.1

### O que muda
- Chrome agora roda em modo headless (`--headless=new`) por PADRÃO em todas as plataformas (GAP-WS-060)
- Anteriormente (v0.8.0), Chrome abria janela GUI visível em qualquer desktop com `$DISPLAY` setado
- `DUCKDUCKGO_CHROME_VISIBLE=1` habilita modo headed para depuração
- `DUCKDUCKGO_CHROME_XVFB=1` habilita modo headed via xvfb-run para evasão anti-bot em servidores headless
- ZERO janelas Chrome visíveis durante execução normal da CLI
- Função `which_xvfb_run()` renomeada para `is_xvfb_requested()` com semântica correta
- `xvfb-run` deixa de ser requisito de runtime — é opt-in via `DUCKDUCKGO_CHROME_XVFB=1`

### Migração passo-a-passo

```bash
# 1. Atualizar duckduckgo-search-cli
cargo install duckduckgo-search-cli --version 0.8.1 --force

# 2. Verificar que Chrome roda em modo headless (sem janela visível)
duckduckgo-search-cli "test query" -q -f json --num 3 | jaq '.metadados.usou_chrome'
# Esperado: true (sem janela visível)

# 3. Para depuração com janela visível (opt-in)
DUCKDUCKGO_CHROME_VISIBLE=1 duckduckgo-search-cli "test" -q -f json --num 3

# 4. Para evasão anti-bot via xvfb-run em servidores (opt-in)
DUCKDUCKGO_CHROME_XVFB=1 duckduckgo-search-cli "test" -q -f json --num 3
```

### Mudanças no schema JSON

Nenhuma mudança de schema. v0.8.1 preserva todos os campos da v0.8.0.

### Notas de compatibilidade
- v0.8.1 é API-compatível com v0.8.0 (sem remoções de campos JSON)
- `xvfb-run` não é mais requisito de runtime — Chrome roda headless por padrão
- Usuários que dependiam de `xvfb-run` para evasão anti-bot devem setar `DUCKDUCKGO_CHROME_XVFB=1`

### Rollback

```bash
cargo install duckduckgo-search-cli --version 0.8.0 --force
```

### Veja também
- `CHANGELOG.pt-BR.md` — changelog completo


## Migração v0.7.x → v0.8.0

### O que muda
- Modo Chrome headed agora é o transporte PRIMARY de busca (mudança arquitetural)
- Cliente HTTP wreq é usado APENAS para `--fetch-content` e `--probe`
- Novo requisito de sistema: Google Chrome ou Chromium deve estar instalado
- Novo requisito de sistema: `xvfb-run` em Linux headless (pacote `xvfb`)
- Novo campo de metadados `tentou_chrome` (bool) na saída JSON
- Campo `usou_chrome` agora `true` quando busca Chrome-primary tem sucesso
- 17 sinais JavaScript stealth injetados via CDP para bypass do anti-bot Cloudflare
- Subcomando deep-research agora usa pipeline Chrome via `parallel.rs`
- Inicialização de tracing movida antes do dispatch de subcomando (corrige `-q` em deep-research)
- Campo `causa_zero` adicionado com 5 variantes causais para diagnóstico de zero resultados
- Exit code 6 (`SUSPECTED_BLOCK`) adicionado para cenários de zero resultado não legítimos
- Descompressão de resposta HTTP (gzip, deflate, brotli) agora automática
- Nova env var `DUCKDUCKGO_ZERO_CAUSE_STRICT` para opt-out de BC do exit 6

### Migração passo-a-passo

```bash
# 1. Instalar Chrome (se ainda não estiver instalado)
# Debian/Ubuntu:
sudo apt install google-chrome-stable
# Ou Chromium:
sudo apt install chromium-browser

# 2. Instalar xvfb para servidores Linux headless
sudo apt install xvfb

# 3. Atualizar duckduckgo-search-cli
cargo install duckduckgo-search-cli --version 0.8.0 --force

# 4. Verificar detecção do Chrome
duckduckgo-search-cli "test query" -q -f json --num 3 | jaq '.metadados.usou_chrome'
# Esperado: true

# 5. Verificar que xvfb funciona (apenas servidor headless)
xvfb-run --auto-servernum duckduckgo-search-cli "test" -q -f json --num 3
```

### Mudanças no schema JSON

| Campo                          | Status    | Notas                                         |
|--------------------------------|-----------|-----------------------------------------------|
| `.metadados.usou_chrome`       | ALTERADO  | Agora `true` para Chrome-primary (era fallback)|
| `.metadados.tentou_chrome`     | NOVO      | `bool` — `true` quando feature `chrome` ativa |
| `.metadados.causa_zero`        | NOVO      | `Option<String>` — classificação causal       |
| `.metadados.sugestao_proxima_acao` | NOVO  | `Option<String>` — próxima ação legível       |
| `.metadados.bytes_brutos`      | NOVO      | `Option<u64>` — bytes brutos antes da descompressão|
| `.metadados.bytes_descomprimidos` | NOVO   | `Option<u64>` — bytes após descompressão      |

### Notas de compatibilidade
- v0.8.0 é API-compatível com v0.7.x (sem remoções de campos JSON)
- Exit code 6 é ADITIVO (exit 5 preservado via `DUCKDUCKGO_ZERO_CAUSE_STRICT=false`)
- Feature Chrome é ON por padrão; use `--no-default-features` para desabilitar
- Modo wreq-only ainda funciona mas NÃO contorna anti-bot Cloudflare

### Rollback

```bash
cargo install duckduckgo-search-cli --version 0.7.10 --force
```

### Veja também
- `CHANGELOG.pt-BR.md` — changelog completo
- `docs/decisions/0004-zero-cause-classification-v0-8-0.md` — ADR de causa zero
- `docs/decisions/0005-http-decompression-v0-8-0.md` — ADR de descompressão
- `docs/decisions/0006-stealth-shell-classification-v0-8-0.md` — ADR de stealth shell
- `docs/decisions/0007-chrome-primary-transport-v0-8-0.md` — ADR de Chrome headed como transporte primário


## Migração v0.7.2 → v0.7.3

### O que muda
- **QUEBRA DE AMBIENTE DE BUILD (apenas builds do código-fonte)**: A stack TLS mudou de `rustls` para BoringSSL via `wreq 6.0.0-rc.29`. Compilar do código-fonte no Linux agora requer `cmake`, `perl`, `pkg-config` e `libclang-dev`. **Compilar do código-fonte no Windows MSVC requer QUATRO ferramentas** (NASM, CMake 3.20+, MSVC C/C++ toolchain, Strawberry Perl — fechados como GAP-WS-28/29/30/31 progressivamente em v0.7.4 e v0.7.5). **`cargo install` SEMPRE compila do código-fonte** — o crates.io não distribui binários pré-compilados para nenhuma plataforma, então esses pré-requisitos aplicam-se a todo usuário Windows, não apenas ao CI. Veja `docs/INSTALL-WINDOWS.pt-BR.md` para configuração passo a passo. A matrix `.github/workflows/release.yml` instala esses pacotes automaticamente.
- **GAP-WS-27 fechado**: O interstitial de CAPTCHA no macOS está corrigido. A mesma query que retornava `quantidade_resultados: 0` na v0.7.2 retorna 5 resultados na v0.7.3 na mesma máquina. Ver `gaps.md` e `docs/decisions/0001-tls-boring-via-wreq.md`.
- **Novas flags CLI (aditivas)**:
  - `--no-warmup` — pula o warm-up `GET https://duckduckgo.com/` antes da primeira query real
  - `--no-cookie-persistence` — mantém cookies em memória apenas; nunca grava `cookies.json` em disco
  - `--cookies-path <PATH>` — sobrescreve o path XDG padrão do cookie jar
  - `--probe-deep` — executa uma query real e classifica o body como `ok` ou `captcha` baseado em marcadores Cloudflare e DuckDuckGo
  - `--allow-lite-fallback` — opt-in para fallback automático do endpoint `html` para `lite` quando `--probe-deep` (ou retentativas de zero resultados) detectam CAPTCHA
- **Novo estado persistente: cookie jar**: Um arquivo `cookies.json` agora é gravado em `~/.config/duckduckgo-search-cli/cookies.json` (Linux), `%APPDATA%\duckduckgo-search-cli\cookies.json` (Windows), ou `~/Library/Application Support/duckduckgo-search-cli/cookies.json` (macOS). Permissões Unix são `0o600` (owner read+write only). Trate este arquivo como trataria uma credencial — ver `SECURITY.pt-BR.md`. Use `--no-cookie-persistence` para desabilitar.
- **Zero mudanças no schema JSON de saída**. Todos os campos da v0.7.2 permanecem presentes. Nenhum campo `Option<T>` novo adicionado no nível superior.
- **Novas dependências**: `wreq 6.0.0-rc.29`, `wreq-util 3.0.0-rc.12`, mais as transitivas `boring2 4.15.11`, `webpki-root-certs 1.0.7`, e a toolchain C do BoringSSL.
- **Dependências removidas**: `reqwest 0.12.28`. `time 0.3.47` não é mais dep direta — puramente transitiva agora.
- **Contagem de testes: 292 lib** (era 279 na v0.7.2). +13 novos testes em `session_warmup` (5), `wreq_cookie_adapter` (3), e `probe_deep` (5). 0 warnings de clippy, 0 diff de fmt, 2 warnings de cargo-deny (RUSTSEC-2025-0057 + RUSTSEC-2025-0052, ambos já na lista de ignore).
- **Tamanho do binário**: +20 MB (BoringSSL estaticamente vinculado). Tempo de build de release: ~40s mais longo que v0.7.2 (BoringSSL compila).

### Migração passo-a-passo

```bash
# Atualize para v0.7.3 (pré-requisitos de build obrigatórios — veja abaixo)
cargo install duckduckgo-search-cli --version 0.7.3 --force

# Verifique a nova versão
duckduckgo-search-cli --version
# duckduckgo-search-cli 0.7.3

# Verifique a correção do GAP-WS-27 no macOS
duckduckgo-search-cli "rust wreq emulation browser fingerprint" -q -f json --num 5
# Esperado: 5 resultados em menos de 2 segundos, sem CAPTCHA

# Teste o novo probe-deep (detecção de CAPTCHA)
duckduckgo-search-cli --probe-deep -q -f json
# Esperado: {"status": "ok", "cascata_motivo": "none", "sugestao_mitigacao": "..."}

# Build do código-fonte (apenas se for compilar — não é necessário para `cargo install`)
sudo apt install cmake perl pkg-config libclang-dev
git checkout v0.7.3
cargo build --release
```

### Mudanças no schema JSON

Nenhuma mudança de schema. v0.7.3 preserva todos os campos da v0.7.2:

| Campo                          | Status    | Notas                                       |
|--------------------------------|-----------|---------------------------------------------|
| `.resultados[].titulo`         | inalterado | Sempre presente quando não vazio           |
| `.resultados[].url`            | inalterado | Sempre presente quando não vazio           |
| `.metadados.identidade_usada`  | inalterado | `Option<String>` — v0.6.4+                |
| `.metadados.nivel_cascata`     | inalterado | `Option<u32>` (0..=4) — v0.6.4+           |
| `.metadados.usou_endpoint_fallback` | inalterado | `bool` — v0.6.0+                        |

O arquivo `cookies.json` é estado interno e não é exposto no schema JSON de saída.

### Notas de compatibilidade
- O binário v0.7.3 é API-compatível com v0.7.2 (sem remoções de flag CLI, sem remoções de campo JSON)
- Os alvos de build de v0.7.3 permanecem inalterados: `x86_64-unknown-linux-gnu`, `x86_64-unknown-linux-musl`, `aarch64-apple-darwin`, `x86_64-apple-darwin`, `x86_64-pc-windows-msvc`
- Binários v0.7.2 que funcionavam em Linux/macOS continuam funcionando — sem upgrade urgente necessário **a menos que** você tenha sido afetado pelo CAPTCHA do macOS (GAP-WS-27)
- Usuários de macOS que tiveram queries com zero resultados na v0.7.2 devem atualizar para v0.7.3 para corrigir o CAPTCHA. A correção é estrutural (fingerprint TLS), não uma solução alternativa.
- A nova dependência `wreq 6.0.0-rc.29` é instalada automaticamente por `cargo install`.

### Rollback

Se precisar voltar para v0.7.2 (ex.: algum problema inesperado de build do BoringSSL):

```bash
# Instalar uma versão específica mais antiga
cargo install duckduckgo-search-cli --version 0.7.2 --force
```

> **Nota**: v0.7.2 foi a versão afetada pelo GAP-WS-27. Voltar reintroduz o bug do CAPTCHA do macOS. Faça isso apenas se v0.7.3 tiver um problema crítico na sua plataforma.

### Veja também

- `CHANGELOG.pt-BR.md` — changelog completo
- `gaps.md` — entrada do GAP-WS-27 com reprodução empírica
- `docs/decisions/0001-tls-boring-via-wreq.md` — decisão arquitetural
- `docs/CROSS_PLATFORM.pt-BR.md` — pré-requisitos de build do BoringSSL
- `SECURITY.pt-BR.md` — tratamento do cookie jar
- `README.pt-BR.md` — overview e início rápido


## Migração v0.7.1 → v0.7.2


## Migração v0.7.3 → v0.7.4

### O Que Muda
- **Release de experiência de build** — mesmas flags, mesmo schema JSON, zero mudanças quebrantes
- **GAP-WS-28 fechado** — preflight no `build.rs` detecta o assembler NASM no PATH antes de invocar o build CMake do BoringSSL
- Sem o NASM, o build falha em segundos com a correção exata em vez de minutos de erros crípticos do CMake
- Nova env var `DDG_SKIP_NASM_CHECK=1` como escape hatch para ambientes de build customizados
- Matrix de CI em `.github/workflows/release.yml` agora instala NASM via Chocolatey na imagem Windows-2022

### Passo a Passo

```bash
# Atualize para v0.7.4
cargo install duckduckgo-search-cli --version 0.7.4 --force

# Verifique
duckduckgo-search-cli --version
# duckduckgo-search-cli 0.7.4
```

### Mudanças no Schema JSON

Nenhuma. v0.7.4 preserva todos os campos de v0.7.3, v0.7.2, e v0.6.x. O preflight roda apenas em tempo de build e não afeta o contrato JSON em runtime.

### Notas de Compatibilidade
- Binário v0.7.4 é compatível em API com v0.7.3, v0.7.2, e v0.6.x
- Targets de build da v0.7.4 inalterados em relação à v0.7.3
- Binários v0.7.3 continuam funcionando — upgrade é opcional, recomendado apenas para usuários Windows MSVC que esbarraram no erro de build do NASM

### Rollback

```bash
cargo install duckduckgo-search-cli --version 0.7.3 --force
```

### Ver Também
- `gaps.md` — GAP-WS-28 (falha de build do Windows NASM)
- `CHANGELOG.pt-BR.md` — release notes da v0.7.4


## Migração v0.7.4 → v0.7.5

### O Que Muda
- **Release de experiência de build e documentação** — mesmas flags, mesmo schema JSON, zero mudanças quebrantes
- **Detecção pré-voo de 4 ferramentas de build**: v0.7.5 adiciona preflight no `build.rs` que detecta se a toolchain local tem os quatro pré-requisitos de build do BoringSSL no Windows MSVC: NASM, CMake 3.20+, toolchain MSVC C/C++ (cl.exe, link.exe), Strawberry Perl
- **4 escape hatches** para falhas de build no Windows: mensagens de erro acionáveis com o comando exato de `cargo install` para baixar a ferramenta faltante
- **`cargo install` SEMPRE compila do código-fonte** — o crates.io NÃO distribui binários pré-compilados para nenhuma plataforma; o pré-requisito das 4 ferramentas aplica-se a todo usuário Windows, não apenas ao CI
- **Matrix CI (`windows-2022`)** em `.github/workflows/ci.yml` e `.github/workflows/release.yml` agora verifica E instala CMake 3.20+, Strawberry Perl, MSVC C/C++ Build Tools, além de NASM (já presente desde v0.7.4)
- Veja as entradas WS-29, WS-30, WS-31, WS-32, WS-33, WS-34, WS-35, WS-36, WS-37 em `gaps.md` para a análise completa da cadeia de gaps de experiência de build

### Migração passo-a-passo

```bash
# Atualize para v0.7.5 (pré-requisitos de build obrigatórios ao compilar do source)
cargo install duckduckgo-search-cli --version 0.7.5 --force

# Verifique a nova versão
duckduckgo-search-cli --version
# duckduckgo-search-cli 0.7.5

# (Windows MSVC) siga docs/INSTALL-WINDOWS.pt-BR.md para o setup das 4 ferramentas
# (Linux) sudo apt install cmake perl pkg-config libclang-dev
```

### Mudanças no schema JSON

Nenhuma mudança de schema. v0.7.5 preserva todos os campos da v0.7.4, da v0.7.3 e de toda a série v0.6.x. O preflight do `build.rs` roda apenas em build time e não afeta o contrato JSON de runtime.

### Notas de compatibilidade
- O binário v0.7.5 é API-compatível com v0.7.4, v0.7.3 e v0.6.x (sem remoções de flag CLI, sem remoções de campo JSON)
- Os alvos de build de v0.7.5 permanecem inalterados: `x86_64-unknown-linux-gnu`, `x86_64-unknown-linux-musl`, `aarch64-apple-darwin`, `x86_64-apple-darwin`, `x86_64-pc-windows-msvc`
- Binários v0.7.4 continuam funcionando — o upgrade é opcional, recomendado apenas se você quiser os detectores preflight e a matrix CI melhorada
- O novo preflight do `build.rs` adiciona custo zero em runtime — roda apenas no `cargo build`

### Rollback

```bash
cargo install duckduckgo-search-cli --version 0.7.4 --force
```

### Veja também

- `gaps.md` — WS-29 até WS-37 (cadeia de gaps de experiência de build)
- `docs/INSTALL-WINDOWS.pt-BR.md` — passo a passo das 4 ferramentas no Windows MSVC
- `CHANGELOG.pt-BR.md` — release notes da v0.7.5

### O que muda
- **Zero quebras.** Todas as flags CLI, schemas JSON de saída e exit codes da v0.7.1 permanecem inalterados.
- **Correção de advisory de segurança (RUSTSEC-2026-0009)**: `time 0.3.40` denial-of-service via RFC 2822 stack exhaustion estava sendo puxado transitivamente via `cookie_store 0.22.0` → `reqwest 0.12.28`. v0.7.2 fixa `time = "0.3.47"` como dep direta para sobrescrever a constraint transitiva.
- **Migração `rand` 0.10**: dev-deps (proptest 1.11+, getrandom 0.4+) unificaram em rand 0.10 e os métodos de conveniência migraram de `Rng` para `RngExt`. Todos os call sites internos atualizados: `random_range`, `random_bool`, `random`, e `IndexedRandom::choose`.
- **Bump de MSRV**: `rust-version` saltou de 1.75 para 1.88 (requerido por `time 0.3.47+` e `rand 0.10`).
- **Correção de higiene de CI**: 6 erros latentes de clippy que estavam quebrando silenciosamente a matrix CI na v0.7.1 são capturados agora por `cargo clippy --all-targets --all-features -- -D warnings`.

### Migração passo-a-passo

```bash
# Atualize para v0.7.2
cargo install duckduckgo-search-cli --version 0.7.2 --force

# Verifique a nova versão
duckduckgo-search-cli --version
# duckduckgo-search-cli 0.7.2

# Verifique que a migração do rand 0.10 funciona
duckduckgo-search-cli "rust async" -q -f json | jaq '.resultados[].titulo'
```

### Mudanças no schema JSON

Nenhuma mudança de schema. v0.7.2 preserva todos os campos da v0.7.1.

### Rollback

```bash
cargo install duckduckgo-search-cli --version 0.7.1 --force
```


## Migração v0.7.0 → v0.7.1

### O que muda
- **Zero quebras.** Todas as flags CLI, schemas JSON de saída e exit codes da v0.7.0 permanecem inalterados.
- **Migração de dependência (interna)**: `rand` saltou de `0.8` para `0.9` para alinhar com `proptest 1.11+` (dev-dep). Todos os call sites internos atualizados.
- **Bump de MSRV**: `rust-version` saltou de `1.75` para `1.85` para satisfazer MSRV de `rand 0.9` e a onda de deps transitivas edition-2024.
- **Limpeza do builder reqwest**: chamadas `ClientBuilder::gzip(true)` e `.brotli(true)` removidas.
- **Higiene de CI**: dois warnings de `actionlint` shellcheck corrigidos.
- **Ignore de advisory de segurança**: `RUSTSEC-2026-0009` (time 0.3.40 DoS) adicionado à lista de ignore do `deny.toml`.

### Migração passo-a-passo

```bash
# Atualize para v0.7.1
cargo install duckduckgo-search-cli --version 0.7.1 --force

# Verifique
duckduckgo-search-cli --version
# duckduckgo-search-cli 0.7.1
```

### Mudanças no schema JSON

Nenhuma mudança de schema. v0.7.1 preserva todos os campos da v0.7.0.

### Rollback

```bash
cargo install duckduckgo-search-cli --version 0.7.0 --force
```


## Migração v0.6.x → v0.7.0

### O que muda
- **Apenas aditivo** — v0.7.0 é totalmente retrocompatível com v0.6.x. O subcomando `buscar`, o schema JSON de configuração padrão, cada flag existente e cada exit code permanecem byte-for-byte idênticos.
- **Novo subcomando público** `deep-research` para pesquisa multi-hop por LLM. Operadores que não invocam `deep-research` não veem mudança observável.
- **Quatro novos módulos públicos** em `lib.rs` — `deep_research`, `decomposition`, `aggregation`, `synthesis` — composíveis a partir de crates downstream.
- **Novas dependências diretas** em `Cargo.toml`: `url = "2"`, `regex = "1"`, e `proptest = "1"` (dev-only). As três são adições puras; nenhuma dependência foi atualizada ou removida.

### O que atualizar no seu pipeline
- Se você roteiriza contra o enum `Subcommand`, adicione um braço de match para `Subcommand::DeepResearch(DeepResearchArgs)`.
- Se você consome `lib::run` diretamente, roteie `args.subcommand` para `lib::execute_deep_research` (o helper que constrói um `Config` padrão e chama o pipeline).
- Se você fixa uma versão mínima suportada em `Cargo.toml` de uma crate downstream, atualize para `duckduckgo-search-cli = "0.7"`.
- Nenhuma migração de schema JSON é necessária: os schemas `SearchOutput` e `MultiSearchOutput` permanecem inalterados.

### Rollback
- Fixe em `duckduckgo-search-cli = "0.6.5"` em crates downstream; o binário no crates.io é totalmente retrocompatível.

## Migração v0.6.4 → v0.6.5

### What Changes
- **No breaking changes** — v0.6.5 is fully backward-compatible with v0.6.4
- Windows build was broken in v0.6.4 and is fixed in v0.6.5
- CI now passes on all 3 SOs (Linux/macOS/Windows) — v0.6.4 had failing CI
- New `--fetch-content` long crawls now show a ProgressBar on stderr (auto-hidden in pipes)
- 5 new property tests in `extraction.rs`, 4 new circuit breaker tests, 1 new wiremock test

### Step-by-Step Migration

```bash
# Update to v0.6.5
cargo install duckduckgo-search-cli --version 0.6.5 --force

# Verify the new version
duckduckgo-search-cli --version
# duckduckgo-search-cli 0.6.5
```

### JSON Schema Changes

No schema changes. v0.6.5 preserves all v0.6.4 fields:

| Field                          | Status    | Notes                                      |
|--------------------------------|-----------|--------------------------------------------|
| `.resultados[].titulo`         | unchanged | Always present when non-empty              |
| `.resultados[].url`            | unchanged | Always present when non-empty              |
| `.metadados.identidade_usada`  | unchanged | `Option<String>` — v0.6.4+                |
| `.metadados.nivel_cascata`     | unchanged | `Option<u32>` (0..=4) — v0.6.4+           |

### Compatibility Notes

- v0.6.5 binary is API-compatible with v0.6.4 (no CLI flag removals, no JSON field removals)
- v0.6.5 build targets are unchanged: `x86_64-unknown-linux-gnu`,
  `x86_64-unknown-linux-musl`, `aarch64-apple-darwin`,
  `x86_64-apple-darwin`, `x86_64-pc-windows-msvc`
- v0.6.4 binaries that worked on Linux/macOS continue to work — no urgent upgrade required
- v0.6.4 binaries that failed on Windows will succeed after upgrading to v0.6.5

### Rollback

If you need to roll back to v0.6.4 (e.g., for Windows users until you can deploy v0.6.5):

```bash
# Install a specific older version
cargo install duckduckgo-search-cli --version 0.6.4 --force
```

> **Note**: v0.6.4 was published with a broken Windows build. It is recommended
> to upgrade to v0.6.5 as soon as possible on Windows. On Linux/macOS, v0.6.4
> is functional and can be retained if needed.

### See Also

- `CHANGELOG.md` — full changelog
- `docs/CROSS_PLATFORM.md` — platform-specific notes
- `SECURITY.md` — vulnerability disclosure
- `README.md` — overview and quick start


## Migração v0.6.3 → v0.6.4

### What Changes
- New `--probe` flag for pre-flight health checks
- New `--identity-profile <auto|chrome-win|...>` flag to pin identity
- New `--seed` semantics (now also controls identity pool rotation)
- New optional JSON fields `.metadados.identidade_usada` and `.metadados.nivel_cascata`
- New 12-identity adaptive anti-bot pool (WS-26)

### Step-by-Step Migration

```bash
# Update to v0.6.4
cargo install duckduckgo-search-cli --version 0.6.4 --force

# Verify
duckduckgo-search-cli --version
```

### JSON Schema Changes

All new fields are `Option<T>` (additive, non-breaking):

| Field                          | Type           | Added in   | Notes                            |
|--------------------------------|----------------|------------|----------------------------------|
| `.metadados.identidade_usada`  | `Option<String>` | v0.6.4     | Format `<family>-<platform>-<16hex>` |
| `.metadados.nivel_cascata`     | `Option<u32>`    | v0.6.4     | Cascade level 0..=4              |

### Compatibility Notes
- v0.6.4 is API-compatible with v0.6.3 (no breaking changes)
- All 313 tests in v0.6.4 pass identically against v0.6.3 schemas

### Rollback

```bash
cargo install duckduckgo-search-cli --version 0.6.3 --force
```


## Migração v0.6.2 → v0.6.3

### What Changes
- All `///` doc comments translated from Portuguese to English
- Zero code behavior changes

### Step-by-Step Migration

```bash
cargo install duckduckgo-search-cli --version 0.6.3 --force
```

### JSON Schema Changes
None. v0.6.3 is binary-compatible with v0.6.2.

### Rollback

```bash
cargo install duckduckgo-search-cli --version 0.6.2 --force
```


## Migração v0.6.1 → v0.6.2

### What Changes
- Documentation-only release: 19 new bilingual files (EN + PT-BR)
- `llms.txt`, `llms-full.txt` added for LLM discovery
- `CODE_OF_CONDUCT.md` (Contributor Covenant 2.1)
- `eval-queries.json` × 2 (20 EN + 20 PT-BR)

### Step-by-Step Migration
None required — documentation-only release.

### JSON Schema Changes
None.


## Migração v0.6.0 → v0.6.1

### What Changes
- `--timeout 0` now returns exit code 2 (invalid config) instead of executing a search with zero timeout
- `--output /tmp/../../etc/passwd` now returns exit code 2 (invalid config) instead of exit 1
- New `validar_timeout_segundos()` method on `CliArgs`
- Early path traversal check in `montar_configuracacoes()`

### Step-by-Step Migration
None required for valid usage. Pipelines that previously relied on `--timeout 0`
or path-traversal commands will now exit with code 2 instead of 5/1.

### JSON Schema Changes
None.

### Rollback

```bash
cargo install duckduckgo-search-cli --version 0.6.0 --force
```


## Migração v0.5.x → v0.6.0

### What Changes
- Browser fingerprint profiles added (4 profiles)
- Anti-bot evasion layer (per-profile User-Agent + Sec-CH-UA + Sec-Fetch headers)
- New `--browser-profile` flag
- New `--no-browser-fingerprint` flag to disable
- New `.metadados.user_agent` field in JSON

### Step-by-Step Migration

```bash
# Update to v0.6.0
cargo install duckduckgo-search-cli --version 0.6.0 --force
```

### JSON Schema Changes

New field: `.metadados.user_agent` (string). Always present from v0.6.0 onwards.

### Rollback

```bash
cargo install duckduckgo-search-cli --version 0.5.0 --force
```


## Migração v0.7.5 → v0.7.6

### O que muda
- **GAP-WS-48 (CRÍTICO, install) — fix de mesmo dia para `cargo install`** por conflito `alloc-no-stdlib 2.0.4` vs `3.0.0` que quebrava installs limpos.
- Sem quebras em flags CLI, schemas JSON de saída ou exit codes.
- Sem mudanças de comportamento de runtime em relação à v0.7.5; o único diff é na resolução de dependências no `cargo install`.
- Veja entrada GAP-WS-48 em `gaps.md` para o rastro do conflito.

### Migração passo-a-passo

```bash
# Atualize para v0.7.6
cargo install duckduckgo-search-cli --version 0.7.6 --force

# Verifique a nova versão
duckduckgo-search-cli --version
# duckduckgo-search-cli 0.7.6
```

### Mudanças no schema JSON

Nenhuma mudança de schema. A v0.7.6 preserva todos os campos da v0.7.5, da v0.7.4 e de toda a série v0.6.x.

### Notas de compatibilidade
- Binário v0.7.6 é API-compatível com v0.7.5, v0.7.4, v0.7.3 e v0.6.x
- Alvos de build de v0.7.6 permanecem inalterados em relação à v0.7.5
- Binários v0.7.5 continuam funcionando — upgrade é opcional, recomendado para usuários que encontraram o conflito de install

### Validação
- `cargo install --version 0.7.6 --force` succeeds em toolchain limpa
- `duckduckgo-search-cli --version` reporta 0.7.6
- `duckduckgo-search-cli "rust" -q -f json` retorna o envelope JSON esperado

### Rollback

```bash
cargo install duckduckgo-search-cli --version 0.7.5 --force
```

### Veja também
- `gaps.md` — GAP-WS-48 (fix de install de mesmo dia)
- `CHANGELOG.pt-BR.md` — release notes da v0.7.6


## Migração v0.7.6 → v0.7.7

### O que muda
- **GAP-WS-49 (CRÍTICO, query) — emulação de fingerprint TLS restaurada** via `wreq 6.0.0-rc.29` + `wreq-util 3.0.0-rc.12` (feature `emulation`).
- A v0.7.6 resolveu o `cargo install` mas o binário publicado produzia queries com zero resultados porque BoringSSL sem emulação gera fingerprint JA3/JA4 que o Cloudflare Bot Management flagra.
- A v0.7.7 readiciona `wreq-util = { version = "3.0.0-rc", default-features = false, features = ["emulation"] }` mais a feature `brotli` no `wreq` e 2 pins diretos para tornar `cargo install` reprodutível.
- Veja entrada GAP-WS-49 em `gaps.md` para causa raiz completa e passos de reprodução.

### Migração passo-a-passo

```bash
# Atualize para v0.7.7
cargo install duckduckgo-search-cli --version 0.7.7 --force

# Verifique a nova versão
duckduckgo-search-cli --version
# duckduckgo-search-cli 0.7.7

# Verifique que uma query real retorna resultados não-zero
duckduckgo-search-cli "rust async runtime" -q -f json | jaq '.quantidade_resultados'
# Espere: >0
```

### Mudanças no schema JSON

Nenhuma mudança de schema. A v0.7.7 preserva todos os campos da v0.7.6, da v0.7.5 e de toda a série v0.6.x.

### Notas de compatibilidade
- Binário v0.7.7 é API-compatível com v0.7.6, v0.7.5, v0.7.4, v0.7.3 e v0.6.x
- Alvos de build de v0.7.7 permanecem inalterados em relação à v0.7.6
- Binários v0.7.6 continuam funcionando mas produzem conjuntos de resultados vazios por causa do GAP-WS-49

### Validação
- `cargo install --version 0.7.7 --force` succeeds em toolchain limpa
- `duckduckgo-search-cli --probe-deep -q -f json` reporta `status: "ok"`
- 5/5 queries de amostra retornam `quantidade_resultados > 0`
- `duckduckgo-search-cli --version` reporta 0.7.7

### Rollback

```bash
cargo install duckduckgo-search-cli --version 0.7.6 --force
```

### Veja também
- `gaps.md` — GAP-WS-49 (regressão de fingerprint TLS)
- `CHANGELOG.pt-BR.md` — release notes da v0.7.7
- `docs/decisions/0002-anti-bot-detector-overhaul-v0-7-8.pt-BR.md` — contexto do follow-up v0.7.8


## Migração v0.7.7 → v0.7.8

### O que muda
- **Renovação do detector anti-bot (GAP-WS-50, CRÍTICO)** — `CLOUDFLARE_MARKERS` e `DDG_MARKERS` em `src/probe_deep.rs` expandidos para reconhecer o novo interstitial `anomaly-modal` que o DDG lançou em 2026-06-14.
- **Calibração do probe-deep (GAP-WS-51, ALTO)** — query `q=rust` substituída pelo pangrama de 9 palavras `the quick brown fox jumps over the lazy dog` via constante `PROBE_CALIBRATION_QUERY` em `src/lib.rs`.
- **Opt-in de fallback lite (GAP-WS-52, ALTO)** — `--allow-lite-fallback` agora consulta `detectar_interstitial` antes de acionar; sem fallback Lite silencioso quando o usuário não fez opt-in.
- **Níveis de verbose (GAP-WS-53, BAIXO)** — `-v` agora é `ArgAction::Count`; `-vv` e `-vvv` funcionam por convenção Unix.
- **Supply chain (GAP-WS-54, MÉDIO)** — `scraper` saltou de 0.20.0 para 0.27.0 para limpar RUSTSEC-2025-0057 via `fxhash 0.2.1`.
- **Drift de docs (GAP-WS-55, BAIXO)** — comentário sobre wreq no `Cargo.toml` reescrito para refletir o pin real em `wreq 6.0.0-rc.29`.
- **Subcomando oculto (GAP-WS-56, BAIXO)** — `buscar` recebe `#[command(hide = true)]`; sem mais `--help` duplicado.
- **Retries honrados (GAP-WS-57, MÉDIO)** — `--retries N` propaga para `execute_with_retry` com clamp `[1, 10]`; `--retries 999` não aciona mais anti-bot.
- Veja entradas WS-50 até WS-57 em `gaps.md` para a cadeia completa.

### Migração passo-a-passo

```bash
# Atualize para v0.7.8
cargo install duckduckgo-search-cli --version 0.7.8 --force

# Verifique a nova versão
duckduckgo-search-cli --version
# duckduckgo-search-cli 0.7.8

# Verifique o novo comportamento de probe-deep
duckduckgo-search-cli --probe-deep -q -f json | jaq '.status'
# Espere: "ok" ou "captcha" (classificação honesta)

# Verifique os níveis de verbose
duckduckgo-search-cli -vvv --version
# Espere: imprime versão E logs de nível trace no stderr
```

### Mudanças no schema JSON

Nenhuma mudança de schema. A v0.7.8 preserva todos os campos da v0.7.7, da v0.7.6 e de toda a série v0.6.x. O campo `metadados.retentativas` agora é populado corretamente quando `--retries N` é usado.

### Notas de compatibilidade
- Binário v0.7.8 é API-compatível com v0.7.7, v0.7.6, v0.7.5, v0.7.4, v0.7.3 e v0.6.x
- Alvos de build de v0.7.8 permanecem inalterados em relação à v0.7.7
- Subcomando `buscar` ainda funciona quando invocado diretamente; apenas oculto do `--help`
- Valores de `--retries` acima de 10 agora são clampados com aviso em vez de acionar anti-bot
- Binários v0.7.7 continuam funcionando mas perdem a detecção do interstitial `anomaly-modal`

### Validação
- `cargo install --version 0.7.8 --force` succeeds em toolchain limpa
- `cargo audit --deny warnings` reporta 0 advisories
- `duckduckgo-search-cli --probe-deep -q -f json` retorna `status: "ok"` em ambientes limpos
- 5/5 queries de amostra retornam `quantidade_resultados > 0`
- `duckduckgo-search-cli -vv "rust" -q -f json` emite logs de nível DEBUG no stderr
- `duckduckgo-search-cli "rust" -q -f json --retries 5` popula `metadados.retentativas = 5`
- 305 lib + 18 testes de integration passando; 0 advisories não-ignorados

### Rollback

```bash
cargo install duckduckgo-search-cli --version 0.7.7 --force
```

### Veja também
- `gaps.md` — GAP-WS-50 até GAP-WS-57 (cadeia de renovação do detector anti-bot)
- `docs/decisions/0002-anti-bot-detector-overhaul-v0-7-8.pt-BR.md` — ADR para os 8 gaps
- `CHANGELOG.pt-BR.md` — release notes da v0.7.8
- `docs/COOKBOOK.pt-BR.md` — Receita 19 (detector), Receita 20 (verbose), Receita 21 (retries)


## Migração v0.7.8 → v0.7.9

### O que muda
- **Detecção de ghost-block (GAP-WS-58, CRÍTICO)** — `detectar_interstitial` em `src/probe_deep.rs` agora classifica body abaixo de 4KB sem `result-page-signal` como `InterstitialKind::Cloudflare`. Helper `has_result_page_signal` checa classes DDG (`nrn-react-div`, `react-article`, `module--results`, `js-react-aria-results`).
- **Marcadores 2026 (GAP-WS-59, ALTO)** — 5 marcadores Cloudflare novos + 1 marker DDG novo. `CLOUDFLARE_MARKERS` e `DDG_MARKERS` atualizados.
- **Flag global (GAP-WS-59, ALTO)** — `--allow-lite-fallback` e `--pre-flight` hoisted para `RootArgs` com `global = true`. Fecha o caminho `unexpected argument` em subcomandos.
- **Config.pre_flight adicionado** com default `false` (opt-in para preservar comportamento v0.7.8).
- **Helper `detectar_interstitial_com_match` (P1)** — retorna `(&'static str, InterstitialKind)` com marker literal.
- **Helper `sugestao_mitigacao_com_marker` (P4b)** — injeta marker real na mensagem de mitigação.
- **Campo `SearchMetadata.pre_flight_fired: bool` (P3)** — presente no envelope quando pre-flight ativo e ghost-block detectado.

### Migração passo-a-passo

```bash
# Atualize para v0.7.9
cargo install duckduckgo-search-cli --version 0.7.9 --force

# Verifique a nova versão
duckduckgo-search-cli --version
# duckduckgo-search-cli 0.7.9

# Verifique detecção de ghost-block
duckduckgo-search-cli --probe-deep -q -f json | jaq '.status, .cascata_motivo'
# Esperado: "captcha", "cloudflare" (ou "ok" com marker)

# Verifique flag global com deep-research
duckduckgo-search-cli -q -f json --allow-lite-fallback deep-research "x"
# Esperado: sem erro "unexpected argument"

# Verifique pre-flight gate
duckduckgo-search-cli --pre-flight "rust" -q -f json | jaq '.metadados.pre_flight_disparado'
# Esperado: true (ghost-block) ou false (clean)
```

### Mudanças no schema JSON
- **Adicionado** `SearchMetadata.pre_flight_fired: bool` — `false` em v0.7.8 (não presente), pode ser `true` em v0.7.9.
- Todos os campos de v0.7.8 preservados byte-for-byte.

### Consumidores — o que quebra
- **Nada quebra.** v0.7.9 é totalmente retrocompatível com v0.7.8.
- Consumidores lendo `metadados.pre_flight_disparado` devem tratar `null` (v0.7.8) e `false` (v0.7.9 sem pre-flight) como equivalentes.

### Rollback

```bash
cargo install duckduckgo-search-cli --version 0.7.8 --force
```


## Migração v0.7.9 → v0.7.10

### O que muda
- **Propagação do pino de identidade (GAP-WS-60, CRÍTICO)** — `--identity-profile` agora propaga para `failure_output` e `error_output` via novo helper `identity_tag_for_cli_identity` em `src/identity.rs`. Antes, o pino `identidade_usada` era `null` em qualquer falha.
- **Bench wiring (GAP-AUD-002, MÉDIO)** — `cargo bench --bench pre_f_light_latency` agora roda Criterion corretamente após adicionar `[[bench]] harness = false` em `Cargo.toml`.
- **Pre-flight scheduler (P5)** — quando `--pre-flight` está setado, o pipeline roda probe mínimo em ~140ms antes da busca real e aborta com `pre_flight_blocked` (exit 3).
- **`--require-results` (P4)** — em `deep-research`, exit 4 quando fan-out zero.
- **B1 fix (CRÍTICO)** — `--pre-flight` não emite mais dois JSON concatenados no stdout.
- **B2 fix (CRÍTICO)** — `pre_flight_blocked` retorna exit 3 (era 0).
- **B3 fix (MÉDIO)** — `--global-timeout` é `global = true`, aceito em subcomandos.
- **B4 fix (CRÍTICO)** — `--probe-deep` standalone retorna exit 3 quando detecta captcha (era 0).
- **Proxy detection (P7)** — novo módulo `src/proxy_detection.rs` com `ProxyKind::{None, Transparent, Cloudflare, Corporate}`. 8 testes cobrindo ISPs BR (Vivo Fiber, Gigaweb, Cloudflare).
- **DDG class watch (P19)** — novo módulo `src/ddg_class_watch.rs` para monitoramento runtime de templates DDG.
- **Snapshot test (P6/P17)** — `insta = "1"` adicionado, snapshot test para os 8 marcadores Cloudflare 2026.
- **Pre-publish gate (regra 1264)** — `scripts/pre-publish-gate.sh` 7 gates sequenciais antes de `cargo publish` real.
- **Skill sync** — `skill/duckduckgo-search-cli-{en,pt}/eval-queries.json` +4 queries (q47-q50).

### Migração passo-a-passo

```bash
# Atualize para v0.7.10
cargo install duckduckgo-search-cli --version 0.7.10 --force

# Verifique a nova versão
duckduckgo-search-cli --version
# duckduckgo-search-cli 0.7.10

# Verifique pino de identidade em failure paths
duckduckgo-search-cli --identity-profile chrome-linux --global-timeout 1 "x" -q -f json \
  | jaq -r '.metadados.identidade_usada'
# Esperado: "chrome-linux-33333333cccc0003" (era: null em v0.7.9)

# Verifique pre-flight gate
duckduckgo-search-cli --pre-flight "rust" -q -f json | jaq '.metadados.pre_flight_disparado'
# Esperado: true (ghost-block) ou false (clean)

# Verifique exit code do probe-deep
duckduckgo-search-cli --probe-deep -q -f json; echo $?
# Esperado: 3 (captcha) ou 0 (ok)

# Verifique require-results em deep-research
duckduckgo-search-cli -q -f json deep-research --require-results "unique_xyz_test" 2>&1; echo $?
# Esperado: 4 (zero resultados) com stderr "exiting non-zero"

# Verifique bench wiring
cargo bench --bench pre_f_light_latency --offline
# Esperado: 6 cenários Criterion reportados (era: "running 0 tests")
```

### Mudanças no schema JSON
- **Formato canônico do pino de identidade** — `identidade_usada` agora usa `<family>-<platform>-<seed16hex>` (ex.: `chrome-linux-33333333cccc0003`) em sucesso E falha. Antes, era FNV-1a(UA) em sucesso e `null` em falha.
- **Todos os campos de v0.7.9 preservados** byte-for-byte.

### Consumidores — o que quebra
- **Nada quebra.** v0.7.10 é totalmente retrocompatível com v0.7.9.
- Consumidores lendo `metadados.identidade_usada` devem tratar o novo formato canônico e `null` (sem pino) como esperados.
- Exit codes: `3` agora é possível em `--probe-deep` e `--pre-flight` onde v0.7.9 retornava `0`.

### Rollback

```bash
cargo install duckduckgo-search-cli --version 0.7.9 --force
```
