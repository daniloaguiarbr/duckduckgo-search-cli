# Instalando duckduckgo-search-cli no Windows (v0.8.6+)

Desde a v0.8.6, `duckduckgo-search-cli` usa `reqwest` com `rustls-tls` no lugar de `wreq`/BoringSSL. Isso elimina a necessidade de NASM, CMake, Perl e MSVC. O unico pre-requisito e o Rust.


## Pre-requisitos

- Windows 10 versao 1903 ou superior, ou Windows 11
- Toolchain Rust instalada via [rustup](https://rustup.rs/)


## Instalacao

```powershell
cargo install duckduckgo-search-cli
duckduckgo-search-cli --version
```

So isso. Sem shell especial, sem compiladores extras, sem assembler.


## Opcional: Chrome (para transporte de busca headed)

- Chrome/Chromium e opcional, usado apenas ao compilar com a feature `chrome` ou ao usar `--fetch-content` com modo Chrome headed
- Instale o Google Chrome em https://www.google.com/chrome/
- Sem necessidade de `xvfb` no Windows (display nativo e usado)
- Chrome e auto-detectado nos caminhos de instalacao padrao


## Historico: v0.7.3 a v0.8.5 (era BoringSSL)

As versoes v0.7.3 a v0.8.5 dependiam de `wreq`/BoringSSL, que exigia quatro ferramentas nativas de build no Windows:

1. Assembler NASM
2. CMake 3.20+
3. Compilador + linker MSVC (Visual Studio Build Tools)
4. Strawberry Perl

Se voce esta instalando uma versao mais antiga (v0.7.3 a v0.8.5), ainda precisa dessas ferramentas. Consulte a [versao v0.8.5 deste documento](https://github.com/daniloaguiarbr/duckduckgo-search-cli/blob/v0.8.5/docs/INSTALL-WINDOWS.pt-BR.md) para o guia passo a passo completo.

Desde a v0.8.6, nenhuma delas e necessaria.


## Troubleshooting

### `cargo install` falha com erros de rede

Certifique-se de que sua toolchain Rust esta atualizada: `rustup update stable`

### Quer instalar uma versao especifica

```powershell
cargo install duckduckgo-search-cli --version 0.8.6 --force
```


## Veja tambem

- `docs/CROSS_PLATFORM.md` — overview de pre-requisitos de build por plataforma
