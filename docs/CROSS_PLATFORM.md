# Cross-Platform Guide


## Support Matrix

| Target | OS | Status | Notes |
|---|---|---|---|
| `x86_64-unknown-linux-gnu` | Ubuntu, Debian, Fedora, RHEL | Supported | Requires glibc 2.17+ |
| `x86_64-unknown-linux-musl` | Alpine, minimal containers | Supported | Static binary, zero system deps |
| `aarch64-apple-darwin` | Apple Silicon M1/M2/M3 | Supported | Native ARM64 performance |
| `x86_64-apple-darwin` | Intel Mac | Supported | Part of macOS Universal binary |
| `x86_64-pc-windows-msvc` | Windows 10/11 | Supported | UTF-8 auto-configured |


## Linux
### glibc — x86_64-unknown-linux-gnu
- Targets Ubuntu 20.04+, Debian 11+, Fedora 37+, RHEL 8+
- Requires glibc version 2.17 or newer — present in every current distribution
- Download the pre-built binary from GitHub Releases or install via `cargo install`
- No shared TLS library needed — `rustls-tls` links statically in release builds
- Works inside WSL2 (Windows Subsystem for Linux) without any extra configuration
### musl — x86_64-unknown-linux-musl
- Targets Alpine Linux, minimal Docker containers, and embedded environments
- The binary is 100% statically linked — zero runtime dependencies on the host system
- Works in `FROM scratch` Docker images because no libc is loaded at runtime
- Build locally with `cargo build --release --target x86_64-unknown-linux-musl`
- Requires `musl-tools` on the build machine: `apt install musl-tools` on Debian or `apk add musl-dev` on Alpine
- Pre-built musl binaries are attached to every GitHub Release as `SHA256SUMS.txt`-verified archives


## macOS
### Apple Silicon — aarch64-apple-darwin
- Runs natively on M1, M2, and M3 processors without Rosetta translation
- Native ARM64 execution eliminates instruction translation overhead entirely
- Available as a standalone binary or as part of the macOS Universal binary merged with `lipo`
- Install via `cargo install duckduckgo-search-cli` to compile for the host architecture
### Intel — x86_64-apple-darwin
- Targets Intel Core i5/i7/i9 Macs running macOS 10.15 Catalina or newer
- Runs under Rosetta 2 on Apple Silicon without performance penalty for most workloads
- The Universal binary ships both slices — macOS selects the correct slice automatically
### Gatekeeper and First Run
- Pre-built binaries downloaded from GitHub are unsigned — Gatekeeper quarantines them on first launch
- Clear the quarantine flag once with this command:

```bash
xattr -dr com.apple.quarantine /usr/local/bin/duckduckgo-search-cli
```

- Alternatively, install from source via `cargo install` — Cargo-built binaries skip Gatekeeper
- Ad-hoc signing for local builds: `codesign -s - /usr/local/bin/duckduckgo-search-cli`


## Windows
### Prerequisites
- Windows 10 version 1903 or newer, or Windows 11 (any version)
- PowerShell 5.1+ or PowerShell 7+ — both work without additional configuration
- Add the binary to a directory on `%PATH%` such as a custom tools folder
- Install via `cargo install duckduckgo-search-cli` — Cargo places the binary in `%USERPROFILE%\.cargo\bin`
### UTF-8 Console Output
- `main.rs` calls `SetConsoleOutputCP(65001)` at startup — UTF-8 is active before any output is written
- Windows Terminal and PowerShell 7 display accented characters and CJK glyphs without mangling
- Legacy `cmd.exe` benefits from the same automatic code page switch — no manual `chcp 65001` needed
- No user action required — the correct encoding is set programmatically on every invocation
### PowerShell Usage
- Standard pipeline syntax works without modification: `duckduckgo-search-cli "rust async" | Select-String "tokio"`
- JSON output integrates natively: `duckduckgo-search-cli -f json "query" | ConvertFrom-Json`
- Exit codes surface in `$LASTEXITCODE` — branch on them with `if ($LASTEXITCODE -ne 0)`
- Use `--output result.json` for file-based output when piping across processes in PowerShell


## Docker and Containers
### Minimal Alpine Image
- Use the musl target binary for the smallest possible image footprint
- Alpine base image adds approximately 7 MB — the combined image stays under 12 MB
- No `apk add` step required at runtime — every dependency is compiled into the binary
- Environment variables for proxy, language, and timeout settings work inside containers
### Example Dockerfile

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

- The builder stage produces a fully static binary using the musl toolchain
- The final stage copies only the binary — no Rust toolchain is included in the runtime image
- Swap `alpine:3.19` for `scratch` to produce an absolutely minimal container
- Mount a writable volume if you use `--output` to persist results outside the container


## Shell Compatibility
### Bash and Zsh
- Pipe output directly to `jaq`, `rg`, or any POSIX-compliant tool without escaping
- Exit code check: `duckduckgo-search-cli -f json "query" && echo OK || echo "Exit: $?"`
- Brace expansion and word splitting behave normally — quote multi-word queries with double quotes
- Shell functions and aliases compose cleanly because the binary writes to stdout and reads nothing from stdin
### Fish
- Fish shell handles the binary identically to any other external command
- Status variable after the command: `if test $status -eq 0`
- Query strings with spaces require double quotes: `duckduckgo-search-cli "multi word query"`
- Use `begin ... end` blocks to capture exit codes across Fish pipelines
### PowerShell
- Results pipe into `ConvertFrom-Json` for native object access in PowerShell scripts
- The `-q` flag suppresses tracing to stderr — clean stdout for `ConvertFrom-Json` parsing
- File output: `duckduckgo-search-cli -f json -o result.json "query"; if ($?) { Get-Content result.json }`
- Works identically in PowerShell 5.1 and PowerShell 7 on Windows and macOS
### Nushell
- Nushell's structured pipeline accepts JSON output natively via `from json`
- Example: `duckduckgo-search-cli -f json "query" | from json | get resultados`
- The binary writes results to stdout and diagnostics to stderr — Nushell respects that separation
- Exit code check: `if ($env.LAST_EXIT_CODE != 0) { error make {msg: "search failed"} }`


## Binary Size and Startup Time
- `x86_64-unknown-linux-gnu`: approximately 3.8 MB stripped release binary
- `x86_64-unknown-linux-musl`: approximately 4.2 MB static release binary
- `aarch64-apple-darwin`: approximately 3.5 MB stripped release binary
- `x86_64-apple-darwin`: approximately 3.8 MB stripped release binary
- `x86_64-pc-windows-msvc`: approximately 4.0 MB stripped release binary
- Startup time across all targets: under 100 milliseconds measured from cold start
- No JIT compilation phase — Rust compiles to native machine code at build time
- Memory footprint per search request: under 20 MB resident set size in typical usage


## Building from Source
### Prerequisites
- Rust toolchain version 1.75 or newer — install via `rustup` from rustup.rs
- For musl targets on Linux: `sudo apt install musl-tools` or `apk add musl-dev` on Alpine
- Cross-compilation: `rustup target add <target>` before running `cargo build`
- For the macOS Universal binary: add both `aarch64-apple-darwin` and `x86_64-apple-darwin` targets
### Build Commands by Target

```bash
# Linux glibc (default on Linux hosts)
cargo build --release

# Linux musl — fully static binary
rustup target add x86_64-unknown-linux-musl
cargo build --release --target x86_64-unknown-linux-musl

# macOS Apple Silicon
rustup target add aarch64-apple-darwin
cargo build --release --target aarch64-apple-darwin

# macOS Intel
rustup target add x86_64-apple-darwin
cargo build --release --target x86_64-apple-darwin

# macOS Universal binary (merges both macOS slices)
lipo -create -output duckduckgo-search-cli-universal \
  target/aarch64-apple-darwin/release/duckduckgo-search-cli \
  target/x86_64-apple-darwin/release/duckduckgo-search-cli

# Windows MSVC — run on Windows with MSVC toolchain installed
cargo build --release --target x86_64-pc-windows-msvc
```


## Installation
### cargo install (all platforms)
- Standard one-command installation across every supported platform:

```bash
cargo install duckduckgo-search-cli
```

- Cargo fetches the crate from crates.io, compiles for the host architecture, and places the binary in `~/.cargo/bin`
- Minimum Supported Rust Version (MSRV) is 1.75 — run `rustup update` if your toolchain is older
- Verify the installation: `duckduckgo-search-cli --version`
### Pre-built Binaries
- Pre-built binaries for all five targets are attached to each GitHub Release
- Each release includes a `SHA256SUMS.txt` file for integrity verification before execution
- Download, verify, and install on Linux or macOS:

```bash
# Replace X.Y.Z with the target release version
curl -LO https://github.com/daniloaguiarbr/duckduckgo-search-cli/releases/download/vX.Y.Z/duckduckgo-search-cli-x86_64-unknown-linux-musl.tar.gz
sha256sum --check SHA256SUMS.txt
tar -xzf duckduckgo-search-cli-x86_64-unknown-linux-musl.tar.gz
chmod +x duckduckgo-search-cli
sudo mv duckduckgo-search-cli /usr/local/bin/
duckduckgo-search-cli --version
```

- Report platform-specific issues at the GitHub repository issue tracker

Read this document in [Português](CROSS_PLATFORM.pt-BR.md).
