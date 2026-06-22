# Suporte Multiplataforma


## Por Que Zero Dependências Importam
- **v0.8.6+**: `duckduckgo-search-cli` usa `reqwest` + `rustls-tls` — TLS puro Rust com zero dependências nativas de C. `cmake`, `perl`, `pkg-config`, `libclang-dev` e NASM NAO sao mais necessarios
- **v0.8.0+**: Chrome headed (via `chromiumoxide`) e o transporte de busca primario — o stack TLS do cliente HTTP importa menos para anti-bot
- Instalar o binário pré-compilado em um container Alpine recém-criado exige zero pacotes extras do sistema
- Builds musl estáticos vinculam cada byte em tempo de compilação — o binário roda em qualquer kernel Linux
- Sem Java Virtual Machine, sem runtime Python, sem gerenciador de processos Node.js para instalar
- O tempo de inicialização fica abaixo de 100 milissegundos porque o runtime Rust é uma camada estática fina
- A codificação UTF-8 no Windows é aplicada automaticamente via `SetConsoleOutputCP(65001)` na inicialização
- SIGPIPE é resetado em `main.rs` para que cadeias de pipes Unix nunca produzam erros `BrokenPipe` espúrios
- **v0.7.3–v0.8.5 apenas**: a compilação do BoringSSL exigia `cmake`, `perl`, `pkg-config` e `libclang-dev` no Linux. Isto NAO e mais necessario a partir da v0.8.6


## Matriz de Suporte

| Target | SO | Status | Observações |
|---|---|---|---|
| `x86_64-unknown-linux-gnu` | Ubuntu, Debian, Fedora, RHEL | Suportado | Requer glibc 2.17+ |
| `x86_64-unknown-linux-musl` | Alpine, containers mínimos | Suportado | Binário estático, zero deps do sistema |
| `aarch64-apple-darwin` | Apple Silicon M1/M2/M3 | Suportado | Desempenho ARM64 nativo |
| `x86_64-apple-darwin` | Intel Mac | Suportado | Parte do binário Universal macOS |
| `x86_64-pc-windows-msvc` | Windows 10/11 | Suportado | UTF-8 configurado automaticamente |


## Linux
### glibc — x86_64-unknown-linux-gnu
- Targeia Ubuntu 20.04+, Debian 11+, Fedora 37+, RHEL 8+
- Requer glibc versão 2.17 ou superior — presente em todas as distribuições atuais
- Baixe o binário pré-compilado do GitHub Releases ou instale via `cargo install`
- **v0.8.6+**: compilar do codigo-fonte exige apenas o toolchain Rust — sem compilador C, `cmake`, `perl`, `pkg-config` ou `libclang-dev` (TLS e puro Rust via `reqwest` + `rustls`)
- **v0.7.3–v0.8.5 apenas**: compilar do codigo-fonte exigia a toolchain C do BoringSSL (`cmake`, `perl`, `pkg-config`, `libclang-dev`). Isto NAO e mais necessario a partir da v0.8.6
- Funciona dentro do WSL2 (Windows Subsystem for Linux) sem nenhuma configuração extra
### musl — x86_64-unknown-linux-musl
- Targeia Alpine Linux, containers Docker mínimos e ambientes embarcados
- O binário é 100% vinculado estaticamente — zero dependências de runtime no sistema host
- Funciona em imagens Docker `FROM scratch` porque nenhuma libc é carregada em runtime
- Compile localmente com `cargo build --release --target x86_64-unknown-linux-musl`
- Requer `musl-tools` na máquina de build: `apt install musl-tools` no Debian ou `apk add musl-dev` no Alpine
- Binários musl pré-compilados são anexados aos GitHub Releases (quando publicados) com verificação via `SHA256SUMS.txt`


## macOS
### Apple Silicon — aarch64-apple-darwin
- Roda nativamente em processadores M1, M2 e M3 sem tradução Rosetta
- A execução ARM64 nativa elimina completamente o overhead de tradução de instruções
- Disponível como binário independente ou como parte do binário Universal macOS mesclado com `lipo`
- Instale via `cargo install duckduckgo-search-cli` para compilar para a arquitetura do host
### Intel — x86_64-apple-darwin
- Targeia Macs Intel Core i5/i7/i9 rodando macOS 10.15 Catalina ou superior
- Roda sob Rosetta 2 no Apple Silicon sem penalidade de desempenho para a maioria das cargas de trabalho
- O binário Universal inclui ambas as fatias — o macOS seleciona a fatia correta automaticamente
### Gatekeeper e Primeira Execução
- Binários pré-compilados baixados do GitHub não são assinados — o Gatekeeper os coloca em quarentena na primeira execução
- Remova a flag de quarentena uma única vez com este comando:

```bash
xattr -dr com.apple.quarantine /usr/local/bin/duckduckgo-search-cli
```

- Alternativamente, instale via `cargo install` — binários compilados pelo Cargo ignoram o Gatekeeper
- Assinatura ad-hoc para builds locais: `codesign -s - /usr/local/bin/duckduckgo-search-cli`


## Windows
### Pré-requisitos
- Windows 10 versão 1903 ou superior, ou Windows 11 (qualquer versão)
- PowerShell 5.1+ ou PowerShell 7+ — ambos funcionam sem configuração adicional
- Adicione o binário a um diretório no `%PATH%` como uma pasta de ferramentas personalizada
- Instale via `cargo install duckduckgo-search-cli` — o Cargo coloca o binário em `%USERPROFILE%\.cargo\bin`
- **v0.8.6+**: nenhuma ferramenta extra alem do toolchain Rust — TLS e puro Rust via `reqwest` + `rustls`
- **v0.7.3–v0.8.5 apenas**: o build nativo MSVC exigia quatro ferramentas extras — (1) assembler NASM, (2) CMake 3.20+, (3) MSVC C/C++ toolchain, (4) Strawberry Perl. Nenhuma dessas e necessaria a partir da v0.8.6
### Saída UTF-8 no Console
- `main.rs` chama `SetConsoleOutputCP(65001)` na inicialização — UTF-8 está ativo antes de qualquer saída ser escrita
- Windows Terminal e PowerShell 7 exibem caracteres acentuados e glifos CJK sem distorção
- O `cmd.exe` legado se beneficia da mesma troca automática de página de código — sem necessidade de `chcp 65001` manual
- Nenhuma ação do usuário é necessária — a codificação correta é definida programaticamente em cada invocação
### Uso no PowerShell
- Sintaxe de pipeline padrão funciona sem modificação: `duckduckgo-search-cli "rust async" | Select-String "tokio"`
- Saída JSON integra nativamente: `duckduckgo-search-cli -f json "query" | ConvertFrom-Json`
- Exit codes aparecem em `$LASTEXITCODE` — ramifique com `if ($LASTEXITCODE -ne 0)`
- Use `--output result.json` para saída em arquivo ao fazer pipe entre processos no PowerShell
### v0.6.5 — Correção de Cast HANDLE no Windows (MP-26)
- **v0.6.4 era impossível de compilar no Windows.** `windows-sys 0.59+`
  mudou o tipo `HANDLE` de `isize` para `*mut c_void`, mas o código de
  inicialização de plataforma em `src/platform.rs` usava casts `handle as isize`.
  `cargo install` no Windows falhava com 4 erros E0308.
- **v0.6.5 corrige isto** usando `!handle.is_null() && handle != INVALID_HANDLE_VALUE`
  e passando o `HANDLE` diretamente para `GetConsoleMode` e `SetConsoleMode`
  (cuja assinatura moderna aceita `HANDLE` por valor, não `isize`).
- **Reabilita builds Windows no CI**: o CI da v0.6.4 falhava silenciosamente
  em `windows-latest`. v0.6.5 adiciona smoke tests de `--version` e `--help`
  na matrix para que regressões futuras no Windows sejam detectadas antes
  da release.


## Docker e Containers
### Imagem Alpine Mínima
- Use o binário do target musl para o menor footprint possível de imagem
- A imagem base Alpine adiciona aproximadamente 7 MB — a imagem combinada fica abaixo de 12 MB
- Nenhum passo `apk add` é necessário em runtime — cada dependência está compilada no binário
- Variáveis de ambiente para proxy, idioma e configurações de timeout funcionam dentro de containers
### Exemplo de Dockerfile

```dockerfile
FROM rust:1.88-alpine AS builder
RUN apk add --no-cache musl-dev
WORKDIR /app
COPY . .
RUN cargo build --release --target x86_64-unknown-linux-musl

FROM alpine:3.19
COPY --from=builder /app/target/x86_64-unknown-linux-musl/release/duckduckgo-search-cli /usr/local/bin/
ENTRYPOINT ["duckduckgo-search-cli"]
```

- O estágio de build produz um binário totalmente estático usando o toolchain musl
- O estágio final copia apenas o binário — nenhum toolchain Rust é incluído na imagem de runtime
- Substitua `alpine:3.19` por `scratch` para produzir um container absolutamente mínimo
- Monte um volume gravável se você usa `--output` para persistir resultados fora do container


## Compatibilidade de Shell
### Bash e Zsh
- Faça pipe da saída diretamente para `jaq`, `rg` ou qualquer ferramenta compatível com POSIX sem escaping
- Verificação de exit code: `duckduckgo-search-cli -f json "query" && echo OK || echo "Exit: $?"`
- Expansão de chaves e divisão de palavras se comportam normalmente — coloque consultas multi-palavra entre aspas duplas
- Funções e aliases de shell se compõem facilmente porque o binário escreve em stdout e não lê nada do stdin
### Fish
- O shell Fish trata o binário de forma idêntica a qualquer outro comando externo
- Variável de status após o comando: `if test $status -eq 0`
- Strings de consulta com espaços requerem aspas duplas: `duckduckgo-search-cli "consulta com espaços"`
- Use blocos `begin ... end` para capturar exit codes em pipelines do Fish
### PowerShell
- Os resultados fazem pipe para `ConvertFrom-Json` para acesso nativo a objetos em scripts PowerShell
- A flag `-q` suprime o tracing para stderr — stdout limpo para parsing com `ConvertFrom-Json`
- Saída em arquivo: `duckduckgo-search-cli -f json -o result.json "query"; if ($?) { Get-Content result.json }`
- Funciona de forma idêntica no PowerShell 5.1 e PowerShell 7 no Windows e no macOS
### Nushell
- O pipeline estruturado do Nushell aceita saída JSON nativamente via `from json`
- Exemplo: `duckduckgo-search-cli -f json "query" | from json | get resultados`
- O binário escreve resultados em stdout e diagnósticos em stderr — o Nushell respeita essa separação
- Verificação de exit code: `if ($env.LAST_EXIT_CODE != 0) { error make {msg: "busca falhou"} }`


## Tamanho do Binário e Tempo de Inicialização
- `x86_64-unknown-linux-gnu`: aproximadamente 3,8 MB de binário de release com strip
- `x86_64-unknown-linux-musl`: aproximadamente 4,2 MB de binário de release estático
- `aarch64-apple-darwin`: aproximadamente 3,5 MB de binário de release com strip
- `x86_64-apple-darwin`: aproximadamente 3,8 MB de binário de release com strip
- `x86_64-pc-windows-msvc`: aproximadamente 4,0 MB de binário de release com strip
- Tempo de inicialização em todos os targets: abaixo de 100 milissegundos medidos a partir de cold start
- Sem fase de compilação JIT — o Rust compila para código de máquina nativo em tempo de build
- Footprint de memória por requisição de busca: abaixo de 20 MB de resident set size em uso típico


## Compilando a Partir do Código-Fonte
### Pré-requisitos
- Toolchain Rust versão 1.88 ou superior — instale via `rustup` em rustup.rs
- Para targets musl no Linux: `sudo apt install musl-tools` ou `apk add musl-dev` no Alpine
- **v0.8.6+**: nenhuma dependencia de build adicional alem do toolchain Rust em qualquer plataforma. TLS e puro Rust via `reqwest` + `rustls`. macOS ainda precisa de `xcode-select --install` para o linker
- **v0.7.3–v0.8.5 apenas (BoringSSL)**: exigia `cmake`, `perl`, `pkg-config`, `libclang-dev` no Linux; Visual Studio Build Tools 2019+ com NASM, CMake, Strawberry Perl no Windows. Ver `scripts/install-windows.ps1` e `docs/INSTALL-WINDOWS.pt-BR.md` para instrucoes historicas de setup
- Compilação cruzada: `rustup target add <target>` antes de executar `cargo build`
- Para o binário Universal macOS: adicione os targets `aarch64-apple-darwin` e `x86_64-apple-darwin`
### Comandos de Build por Target

```bash
# Linux glibc (padrão em hosts Linux)
cargo build --release

# Linux musl — binário totalmente estático
rustup target add x86_64-unknown-linux-musl
cargo build --release --target x86_64-unknown-linux-musl

# macOS Apple Silicon
rustup target add aarch64-apple-darwin
cargo build --release --target aarch64-apple-darwin

# macOS Intel
rustup target add x86_64-apple-darwin
cargo build --release --target x86_64-apple-darwin

# Binário Universal macOS (une as duas fatias macOS)
lipo -create -output duckduckgo-search-cli-universal \
  target/aarch64-apple-darwin/release/duckduckgo-search-cli \
  target/x86_64-apple-darwin/release/duckduckgo-search-cli

# Windows MSVC — execute no Windows com o toolchain MSVC instalado
cargo build --release --target x86_64-pc-windows-msvc
```


## Instalação
### cargo install (todas as plataformas)
- Instalação padrão com um único comando em todas as plataformas suportadas:

```bash
cargo install duckduckgo-search-cli
```

- O Cargo busca a crate do crates.io, compila para a arquitetura do host e coloca o binário em `~/.cargo/bin`
- A Versao Minima Suportada do Rust (MSRV) e 1.88 — execute `rustup update` se seu toolchain for mais antigo
- **v0.8.6+**: nenhuma dependencia adicional de sistema em qualquer plataforma — TLS e puro Rust via `reqwest` + `rustls`
- **v0.7.3–v0.8.5 apenas**: adicionalmente requeria `cmake`, `perl`, `pkg-config` e `libclang-dev` no Linux para a stack BoringSSL
- Verifique a instalacao: `duckduckgo-search-cli --version`
### Binários Pré-compilados
- Binários pré-compilados para todos os cinco targets são anexados aos GitHub Releases quando o pipeline de release os publica (`cargo install` sempre compila do source)
- Cada release inclui um arquivo `SHA256SUMS.txt` para verificação de integridade antes da execução
- Baixe, verifique e instale no Linux ou macOS:

```bash
# Substitua X.Y.Z pela versão de release desejada
curl -LO https://github.com/daniloaguiarbr/duckduckgo-search-cli/releases/download/vX.Y.Z/duckduckgo-search-cli-x86_64-unknown-linux-musl.tar.gz
sha256sum --check SHA256SUMS.txt
tar -xzf duckduckgo-search-cli-x86_64-unknown-linux-musl.tar.gz
chmod +x duckduckgo-search-cli
sudo mv duckduckgo-search-cli /usr/local/bin/
duckduckgo-search-cli --version   # espere 0.7.7 (0.7.8 na working tree)
```

- Reporte problemas específicos de plataforma no rastreador de issues do repositório no GitHub


## v0.7.6 — Correção do `cargo install` (GAP-WS-48)

**A v0.7.5 era impossível de compilar via `cargo install` em máquinas
limpas.** Em 2026-06-14, `cargo install duckduckgo-search-cli` falhava
com 36 erros do tipo `E0277 the trait bound 'StandardAlloc: alloc::Allocator<T> is not satisfied`
porque o solver puxava `alloc-no-stdlib 3.0.0` (transitivamente de
`brotli-decompressor 5.0.2`) que colide com a expectativa do
`brotli 8.0.3` de `alloc-no-stdlib = "2.0"`.

**A v0.7.6 corrige isto** removendo a dep não utilizada `wreq-util` e
abandonando a feature `brotli` do `wreq` (DuckDuckGo nunca serve
`Content-Encoding: br`). O grafo de dependências volta ao estado limpo
e `cargo install` sucede em ~35,7s.

**GAP-WS-48 residual — NÃO totalmente fechado sem `--locked`**: mesmo
com o fix da v0.7.6, `cargo install` sem `--locked` ainda pode quebrar
em 2026-06-14+ porque o solver pode escolher a `alloc-stdlib 0.2.3`
publicada recentemente (que depende de `alloc-no-stdlib >=2.0.4, <4`) e
regenerar o lockfile com a versão conflitante 3.0.0. A receita robusta é:

```bash
# Sempre use --locked para respeitar o Cargo.lock commitado
cargo install duckduckgo-search-cli --locked

# Fixe a versão E o lock
cargo install duckduckgo-search-cli --version 0.7.7 --locked
```

A v0.7.7 commita um `Cargo.lock` preparado com
`cargo update -p alloc-no-stdlib@3.0.0 --precise 2.0.4` para que
`--locked` rejeite a resolução ruim. Sem `--locked`, o solver é livre
para reintroduzir o conflito.


## v0.7.7 — Correção do Fingerprint TLS (GAP-WS-49)

**A v0.7.6 publicou um binário que passava em todos os smoke tests mas
retornava ZERO resultados reais.** O `wreq 6.0.0-rc.29` sozinho NÃO
inclui a feature `emulation`; a emulação de fingerprint JA3/JA4 vivia
no `wreq-util 3.0.0-rc.12` via `default = ["emulation"]`. A v0.7.6
havia removido o `wreq-util` para fechar o GAP-WS-48 de `cargo install`,
e o handshake BoringSSL-sem-emulação tornou-se trivialmente detectável
pelo Cloudflare Bot Management. A DDG servia `anomaly-modal` (45
ocorrências no HTML body) para cada query real.

**A v0.7.7 corrige isto** re-adicionando `wreq-util 3.0.0-rc.12` com
`default-features = false, features = ["emulation"]` e três pins diretos
no `Cargo.toml`:

- `brotli-decompressor = "=5.0.1"` (a 5.0.2 publicada em 2026-06-14
  alarga o range de `alloc-no-stdlib` e puxa 3.0.0)
- `alloc-no-stdlib = "=2.0.4"` (5.0.1+ exige esta versão exata)
- Feature `"brotli"` do `wreq` re-habilitada (mandatória para `emulation`)

O resultado: queries reais voltam a retornar 5+ resultados com fingerprint
TLS JA3/JA4 idêntico ao Chrome/Safari, correspondendo à sonda de
navegador que a DDG espera. `cargo build --release --offline` sucede em
24,04s (mais rápido que v0.7.6 porque `brotli-decompressor 5.0.1` é
menor que 5.0.2).

**Ressalva para `cargo install`**: use `--locked` (veja nota do GAP-WS-48
residual acima). Sem `--locked`, o solver pode puxar
`alloc-stdlib 0.2.3` e o conflito retorna.


## v0.7.8 — Renovação do Detector Anti-Bot + Verbose Acumulado (8 gaps)

**A v0.7.8 (working tree, aguardando tag)** fecha 8 gaps na superfície
de detecção anti-bot. Ver `docs/decisions/0002-anti-bot-detector-overhaul-v0-7-8.md`
para a decisão arquitetural completa. Mudanças principais:

- **`detectar_interstitial` expandido** (GAP-WS-50): `CLOUDFLARE_MARKERS`
  cresceu para 8 entradas (`anomaly-modal`, `anomaly-modal__mask`,
  `anomaly-modal__title`, `anomaly.js?cc=botnet`, `cf-turnstile`,
  `cf-spinner`, `Just a moment`, `cf-mitigated`) mais 1 marcador DDG
  novo (`Unfortunately, bots use DuckDuckGo too.`). O detector agora
  captura o interstitial `anomaly-modal` pós-2026 que a v0.7.7 perdeu.
- **Probe-deep usa query de calibração longa** (GAP-WS-51): o literal
  `q=rust` (4 chars) foi substituído pelo pangrama de 9 palavras
  `the quick brown fox jumps over the lazy dog` exposto como
  `PROBE_CALIBRATION_QUERY` em `src/lib.rs:91, 509`. Queries longas
  acionam o bot scoring upstream de forma confiável, tornando o probe
  honesto.
- **`--allow-lite-fallback` agora consulta o detector** (GAP-WS-52): o
  predicado em `src/search.rs:559` migrou de
  `accumulated_results.is_empty()` para
  `detectar_interstitial(&first_html) != InterstitialKind::None`. Quando
  a flag está OFF e o detector ainda flagra interstitial, um
  `tracing::warn!` estruturado é emitido com
  `kind = interstitial_kind.as_str()`.
- **Verbose agora é cumulativo** (GAP-WS-53): `-v` → `info`, `-vv` →
  `debug`, `-vvv` → `trace`. `RUST_LOG` continua sobrescrevendo.
- **`scraper` bumpado para 0.27** (GAP-WS-54): fecha RUSTSEC-2025-0057
  (`fxhash 0.2.1` unmaintained). `cargo audit --deny warnings` agora é
  gate de CI em `ci.yml` e `release.yml`.
- **Comentário do `wreq` reescrito** (GAP-WS-55): o texto anterior
  alegava uma "regressão para 5.3.0" que nunca aconteceu. O novo
  comentário documenta o pin real em `wreq 6.0.0-rc.29` e os três pins
  diretos.
- **Subcomando `buscar` escondido** (GAP-WS-56): `#[command(hide = true)]`
  mantém invocável mas remove do `--help` para reduzir ruído.
- **`--retries` agora é honrado** (GAP-WS-57): o valor estava hard-coded
  para 1 em `src/parallel.rs:644`; corrigido para ler `cfg.retries` com
  clamp `[1, 10]` para que `--retries 999` não acione anti-bot.

**Impacto multiplataforma**: zero breaking changes. Schema JSON e exit
codes inalterados. Tamanho do binário inalterado. Delta de tempo de
build dentro de ±5% em todos os targets. O novo `scraper 0.27` pode
serializar `Selector` levemente diferente, mas nenhum call site
precisou de refactor.


## Matriz Comparativa v0.7.5 → v0.7.8

| Concern | v0.7.5 | v0.7.7 | v0.7.8 |
|---|---|---|---|
| `cargo install` no Linux | Quebrado (GAP-WS-48) | Funciona com `--locked` | Funciona com `--locked` |
| Queries reais retornam resultados | Sim | Sim (restaurado via fix TLS) | Sim (com markers melhores) |
| Detecta DDG `anomaly-modal` | Não | Não | Sim (8 markers novos) |
| Probe-deep sinal honesto | Query curta `rust` | Query curta `rust` | Pangrama 9-palavras |
| Fallback opt-in honrado | Predicado invertido | Predicado invertido | Guiado por detector |
| `-vv` flag de debug | Não suportado | Não suportado | Sim (`ArgAction::Count`) |
| `cargo audit` limpo | 1 advisory transitivo | 1 advisory transitivo | Limpo (RUSTSEC-2025-0057 fechado) |
| Subcomando `buscar` | Visível no `--help` | Visível no `--help` | Escondido |
| `--retries N` honrado | Não (hard-coded 1) | Não (hard-coded 1) | Sim (clamp `[1, 10]`) |


## GAP-WS-48 Residual — Quando o Sintoma Retorna

Se um usuário reportar `E0277 the trait bound 'StandardAlloc: alloc::Allocator<T> is not satisfied`
em `cargo install` da v0.7.7 ou v0.7.8, a causa é quase sempre uma
destas:

1. **Faltou `--locked`**: o solver regenerou o lockfile e puxou
   `alloc-stdlib 0.2.3` → `alloc-no-stdlib 3.0.0`. Correção:
   `cargo install duckduckgo-search-cli --locked`.
2. **Misturando lock da v0.7.6 com source da v0.7.7**: alguns usuários
   cachearam o lock da v0.7.6 e esqueceram de atualizar. Correção:
   `cargo update` ou remova `Cargo.lock` e reconstrua com `--locked`.
3. **Mirror de registry customizado**: o mirror pode estar desatualizado
   e servir `brotli-decompressor 5.0.2` em vez de 5.0.1. Correção:
   configure o mirror para upstream crates.io, ou use um `Cargo.lock`
   mais recente.

A receita robusta para máquinas limpas é:

```bash
# Linux/macOS — versão explícita + lock travado
cargo install duckduckgo-search-cli --version 0.7.7 --locked

# Windows MSVC — o mesmo, mais developer shell para cl.exe
cargo install duckduckgo-search-cli --version 0.7.7 --locked
```

Verifique após a instalação:

```bash
duckduckgo-search-cli --version          # espere 0.7.7 (ou 0.7.8)
duckduckgo-search-cli -q -n 5 "rust async runtime"  # espere 5 resultados
```

## Requisitos do Chrome (v0.8.5)
- Linux: `sudo apt install google-chrome-stable xvfb` (Debian/Ubuntu)
- Linux: `sudo dnf install google-chrome-stable xorg-x11-server-Xvfb` (Fedora)
- Linux: Xvfb é auto-spawned pela CLI via `spawn_virtual_display()` (v0.8.5+) — sem necessidade de `xvfb-run` manual
- Linux: se Xvfb não estiver instalado, Chrome cai para headless (com risco de detecção anti-bot)
- macOS: Instale o Chrome em https://www.google.com/chrome/ (Chrome roda headless no macOS)
- Windows: Instale o Chrome em https://www.google.com/chrome/ (Chrome roda headless no Windows)
- Chrome é auto-detectado via `detect_chrome()` em `src/browser.rs`
- Compilar sem Chrome: `cargo build --no-default-features`


Leia este documento em [English](CROSS_PLATFORM.md).
