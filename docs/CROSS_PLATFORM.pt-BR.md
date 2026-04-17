# Suporte Multiplataforma


## Por Que Zero Dependências Importam
- `duckduckgo-search-cli` usa exclusivamente `rustls-tls` — sem OpenSSL, sem SChannel, sem surpresas com TLS nativo
- Instalar em um container Alpine recém-criado exige zero pacotes extras do sistema
- Builds musl estáticos vinculam cada byte em tempo de compilação — o binário roda em qualquer kernel Linux
- Sem Java Virtual Machine, sem runtime Python, sem gerenciador de processos Node.js para instalar
- O tempo de inicialização fica abaixo de 100 milissegundos porque o runtime Rust é uma camada estática fina
- O tamanho do binário após `strip = "symbols"` e `lto = "thin"` fica entre 3 MB e 5 MB por target
- A codificação UTF-8 no Windows é aplicada automaticamente via `SetConsoleOutputCP(65001)` na inicialização
- SIGPIPE é resetado em `main.rs` para que cadeias de pipes Unix nunca produzam erros `BrokenPipe` espúrios


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
- Nenhuma biblioteca TLS compartilhada é necessária — `rustls-tls` vincula estaticamente em builds de release
- Funciona dentro do WSL2 (Windows Subsystem for Linux) sem nenhuma configuração extra
### musl — x86_64-unknown-linux-musl
- Targeia Alpine Linux, containers Docker mínimos e ambientes embarcados
- O binário é 100% vinculado estaticamente — zero dependências de runtime no sistema host
- Funciona em imagens Docker `FROM scratch` porque nenhuma libc é carregada em runtime
- Compile localmente com `cargo build --release --target x86_64-unknown-linux-musl`
- Requer `musl-tools` na máquina de build: `apt install musl-tools` no Debian ou `apk add musl-dev` no Alpine
- Binários musl pré-compilados são anexados a cada GitHub Release com verificação via `SHA256SUMS.txt`


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
### Saída UTF-8 no Console
- `main.rs` chama `SetConsoleOutputCP(65001)` na inicialização — UTF-8 está ativo antes de qualquer saída ser escrita
- Windows Terminal e PowerShell 7 exibem caracteres acentuados e glifos CJK sem distorção
- O `cmd.exe` legado se beneficia da mesma troca automática de página de código — sem necessidade de `chcp 65001` manual
- Nenhuma ação do usuário é necessária — a codificação correta é definida programaticamente em cada invocação
### Uso no PowerShell
- A sintaxe de pipeline padrão funciona sem modificação: `duckduckgo-search-cli "rust async" | Select-String "tokio"`
- A saída JSON se integra nativamente: `duckduckgo-search-cli -f json "query" | ConvertFrom-Json`
- Os exit codes aparecem em `$LASTEXITCODE` — ramifique com `if ($LASTEXITCODE -ne 0)`
- Use `--output result.json` para saída baseada em arquivo ao fazer piping entre processos no PowerShell


## Docker e Containers
### Imagem Alpine Mínima
- Use o binário do target musl para o menor footprint possível de imagem
- A imagem base Alpine adiciona aproximadamente 7 MB — a imagem combinada fica abaixo de 12 MB
- Nenhum passo `apk add` é necessário em runtime — cada dependência está compilada no binário
- Variáveis de ambiente para proxy, idioma e configurações de timeout funcionam dentro de containers
### Exemplo de Dockerfile

```dockerfile
FROM rust:1.75-alpine AS builder
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
- Toolchain Rust versão 1.75 ou superior — instale via `rustup` em rustup.rs
- Para targets musl no Linux: `sudo apt install musl-tools` ou `apk add musl-dev` no Alpine
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
- A Versão Mínima Suportada do Rust (MSRV) é 1.75 — execute `rustup update` se seu toolchain for mais antigo
- Verifique a instalação: `duckduckgo-search-cli --version`
### Binários Pré-compilados
- Binários pré-compilados para todos os cinco targets são anexados a cada GitHub Release
- Cada release inclui um arquivo `SHA256SUMS.txt` para verificação de integridade antes da execução
- Baixe, verifique e instale no Linux ou macOS:

```bash
# Substitua X.Y.Z pela versão de release desejada
curl -LO https://github.com/daniloaguiarbr/duckduckgo-search-cli/releases/download/vX.Y.Z/duckduckgo-search-cli-x86_64-unknown-linux-musl.tar.gz
sha256sum --check SHA256SUMS.txt
tar -xzf duckduckgo-search-cli-x86_64-unknown-linux-musl.tar.gz
chmod +x duckduckgo-search-cli
sudo mv duckduckgo-search-cli /usr/local/bin/
duckduckgo-search-cli --version
```

- Reporte problemas específicos de plataforma no rastreador de issues do repositório no GitHub

Leia este documento em [English](CROSS_PLATFORM.md).
