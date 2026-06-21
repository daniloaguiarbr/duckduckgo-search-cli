# Cross-Platform Guide

> Current release: **v0.7.7** (published 2026-06-14, working tree **v0.7.8**). v0.7.7 fixes the critical GAP-WS-49 (TLS fingerprint regression in v0.7.6 — real queries returned 0 results). v0.7.8 (pending tag) closes 8 anti-bot detection gaps (see `docs/decisions/0002-anti-bot-detector-overhaul-v0-7-8.md`). Both retain all v0.7.3 features (BoringSSL TLS via `wreq 6.0.0-rc.29`, `session`, `probe-deep`); fully backward-compatible with v0.7.0–v0.7.3 in CLI flags and JSON output schema. Building requires `cmake`, `perl`, `pkg-config`, `libclang-dev` on Linux and the NASM assembler on Windows MSVC. `cargo install` ALWAYS compiles from source; GitHub Release binaries are available only when the release pipeline publishes them. **Residual GAP-WS-48 — see note below: `cargo install` WITHOUT `--locked` can still break on the `alloc-no-stdlib 2.0.4 vs 3.0.0` collision. Always pass `--locked`.**


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
- **v0.7.3+: building from source requires BoringSSL toolchain.** Install `cmake`, `perl`, `pkg-config`, and `libclang-dev` (Debian/Ubuntu: `apt install cmake perl pkg-config libclang-dev`; Fedora/RHEL: `dnf install cmake perl pkg-config clang-devel`). The build statically links BoringSSL via `wreq 6.0.0-rc.29`; pre-built binaries are not affected.
- Works inside WSL2 (Windows Subsystem for Linux) without any extra configuration
### musl — x86_64-unknown-linux-musl
- Targets Alpine Linux, minimal Docker containers, and embedded environments
- The binary is 100% statically linked — zero runtime dependencies on the host system
- Works in `FROM scratch` Docker images because no libc is loaded at runtime
- Build locally with `cargo build --release --target x86_64-unknown-linux-musl`
- Requires `musl-tools` on the build machine: `apt install musl-tools` on Debian or `apk add musl-dev` on Alpine
- **v0.7.3+: BoringSSL build adds cmake, perl, pkg-config, libclang-dev as additional build-time deps for the musl target**
- Pre-built musl binaries are attached to GitHub Releases (when published) as `SHA256SUMS.txt`-verified archives


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
### v0.6.5 — Windows HANDLE Cast Fix (MP-26)
- **v0.6.4 was unbuildable on Windows.** `windows-sys 0.59+` changed the
  `HANDLE` type from `isize` to `*mut c_void`, but the platform-init code
  in `src/platform.rs` used `handle as isize` casts. `cargo install` on
  Windows failed with 4 E0308 errors.
- **v0.6.5 fixes this** by using `!handle.is_null() && handle != INVALID_HANDLE_VALUE`
  and passing the `HANDLE` directly to `GetConsoleMode` and `SetConsoleMode`
  (whose modern signature accepts `HANDLE` by value, not `isize`).
- **Re-enable CI Windows builds**: v0.6.4 CI silently failed on `windows-latest`.
  v0.6.5 adds `--version` and `--help` smoke tests to the matrix so future
  Windows regressions are caught before release.


### v0.7.3 — BoringSSL Build Prerequisites and musl Toolchain

- **TLS stack changed from `rustls` to BoringSSL via `wreq 6.0.0-rc.29`.** BoringSSL
  is statically linked into the binary. On Linux, building from source now
  requires the C toolchain for BoringSSL compilation:

  ```bash
  # Debian / Ubuntu
  sudo apt-get update && sudo apt-get install -y \
    cmake perl pkg-config libclang-dev

  # Fedora / RHEL
  sudo dnf install -y cmake perl pkg-config clang-devel

  # Alpine (musl)
  sudo apk add cmake perl pkg-config clang-dev
  ```

- **CI matrix in `.github/workflows/release.yml`** automatically installs
  these packages in the `linux-x86_64-build` and `linux-x86_64-musl-build`
  jobs. End users installing the pre-built binary from crates.io do not
  need any of these tools.
- **Binary size**: +20 MB (BoringSSL is statically linked). Release build
  time: +30s to +90s depending on hardware (BoringSSL takes 30s to 90s
  to compile in release mode).
- **macOS Apple Silicon and Intel**: GitHub Release binaries (when published) need no extra deps.
  `cargo install` always compiles from source and requires Command Line Tools (`xcode-select --install`).
- **Windows MSVC**: `cargo install` always compiles from source — crates.io ships NO pre-built binaries. Requires Visual Studio
  Build Tools 2019+ with the C++ workload PLUS the NASM assembler (GAP-WS-28; preflight added in v0.7.4) PLUS CMake 3.20+ (GAP-WS-29) PLUS MSVC C/C++ toolchain (GAP-WS-30) PLUS Strawberry Perl (GAP-WS-31). v0.7.5 extends the preflight to detect all four tools. See `scripts/install-windows.ps1` and `docs/INSTALL-WINDOWS.md`.
- **Docker Alpine example** (v0.7.3+):

  ```dockerfile
  FROM rust:1.88-alpine AS builder
  RUN apk add --no-cache musl-dev cmake perl pkg-config clang-dev
  WORKDIR /app
  COPY . .
  RUN cargo build --release --target x86_64-unknown-linux-musl

  FROM alpine:3.19
  COPY --from=builder /app/target/x86_64-unknown-linux-musl/release/duckduckgo-search-cli /usr/local/bin/
  ENTRYPOINT ["duckduckgo-search-cli"]
  ```


## Docker and Containers
### Minimal Alpine Image
- Use the musl target binary for the smallest possible image footprint
- Alpine base image adds approximately 7 MB — the combined image stays under 12 MB
- No `apk add` step required at runtime — every dependency is compiled into the binary
- Environment variables for proxy, language, and timeout settings work inside containers
### Example Dockerfile

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
- Rust toolchain version 1.88 or newer — install via `rustup` from rustup.rs
- For musl targets on Linux: `sudo apt install musl-tools` or `apk add musl-dev` on Alpine
- **For v0.7.3+ (BoringSSL)**: `cmake`, `perl`, `pkg-config`, `libclang-dev` on Linux. macOS needs `xcode-select --install`. Windows needs Visual Studio Build Tools 2019+ with the C++ workload AND the C++ CMake tools for Windows sub-component (manually selected in the Visual Studio Installer — NOT included in the C++ workload by default) AND the NASM assembler (`winget install -e --id NASM.NASM`; the installer does not update PATH) AND Strawberry Perl (`winget install -e --id StrawberryPerl.StrawberryPerl`). MSVC tools (cl.exe, link.exe) require running `Launch-VsDevShell.ps1` in the same shell to set PATH, INCLUDE, and LIB. See `scripts/install-windows.ps1` and the new `docs/INSTALL-WINDOWS.md` for step-by-step instructions covering each prerequisite. (Closed GAP-WS-29/30/31 in v0.7.5.)
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
- Minimum Supported Rust Version (MSRV) is 1.88 (since v0.7.2) — run `rustup update` if your toolchain is older. v0.7.3+ additionally requires `cmake`, `perl`, `pkg-config`, and `libclang-dev` on Linux for the BoringSSL stack via `wreq 6.0.0-rc`.
- Verify the installation: `duckduckgo-search-cli --version` (expect `0.7.7` for the v0.7.7 release; v0.7.8 in working tree)
- **ALWAYS pass `--locked`** to avoid residual GAP-WS-48: `cargo install duckduckgo-search-cli --locked` (or pin the version too: `cargo install duckduckgo-search-cli --version 0.7.7 --locked`). The v0.7.7 `Cargo.lock` was prepared with `cargo update -p alloc-no-stdlib@3.0.0 --precise 2.0.4` to keep the dependency graph clean.
### Pre-built Binaries
- Pre-built binaries for all five targets are attached to GitHub Releases when the release pipeline publishes them (`cargo install` always compiles from source)
- Each release includes a `SHA256SUMS.txt` file for integrity verification before execution
- Download, verify, and install on Linux or macOS:

```bash
# Replace X.Y.Z with the target release version
curl -LO https://github.com/daniloaguiarbr/duckduckgo-search-cli/releases/download/vX.Y.Z/duckduckgo-search-cli-x86_64-unknown-linux-musl.tar.gz
sha256sum --check SHA256SUMS.txt
tar -xzf duckduckgo-search-cli-x86_64-unknown-linux-musl.tar.gz
chmod +x duckduckgo-search-cli
sudo mv duckduckgo-search-cli /usr/local/bin/
duckduckgo-search-cli --version   # expect 0.7.7 (v0.7.8 in working tree)
```

- Report platform-specific issues at the GitHub repository issue tracker


## v0.7.6 — `cargo install` Fix (GAP-WS-48)

**v0.7.5 was unbuildable via `cargo install` on fresh machines.** On
2026-06-14, `cargo install duckduckgo-search-cli` failed with 36 errors
of the form `E0277 the trait bound 'StandardAlloc: alloc::Allocator<T>'
is not satisfied` because the resolver pulled in `alloc-no-stdlib 3.0.0`
(transitively from `brotli-decompressor 5.0.2`) which collides with the
`brotli 8.0.3` expectation of `alloc-no-stdlib = "2.0"`.

**v0.7.6 fixes this** by removing the unused `wreq-util` dep and dropping
the `brotli` feature from `wreq` (DuckDuckGo never serves `Content-Encoding: br`).
The dependency graph returns to a clean state and `cargo install` succeeds
in ~35.7s.

**Residual GAP-WS-48 — NOT fully closed without `--locked`**: even with the
v0.7.6 fix, `cargo install` without `--locked` can still break on
2026-06-14+ because the resolver may pick the newly published
`alloc-stdlib 0.2.3` (which depends on `alloc-no-stdlib >=2.0.4, <4`) and
regenerate the lockfile with the conflicting version 3.0.0. The robust
recipe is:

```bash
# Always use --locked to respect the committed Cargo.lock
cargo install duckduckgo-search-cli --locked

# Pin both the version AND the lock
cargo install duckduckgo-search-cli --version 0.7.7 --locked
```

v0.7.7 commits a `Cargo.lock` prepared with
`cargo update -p alloc-no-stdlib@3.0.0 --precise 2.0.4` so that
`--locked` rejects the bad resolution. Without `--locked`, the resolver
is free to re-introduce the conflict.


## v0.7.7 — TLS Fingerprint Fix (GAP-WS-49)

**v0.7.6 published a binary that passed all smoke tests but returned ZERO
real results.** The `wreq 6.0.0-rc.29` alone does NOT include the
`emulation` feature; the JA3/JA4 TLS fingerprint emulation lived in
`wreq-util 3.0.0-rc.12` via `default = ["emulation"]`. v0.7.6 had
removed `wreq-util` to fix the GAP-WS-48 `cargo install` issue, and the
BoringSSL-without-emulation handshake became trivially detectable by
Cloudflare Bot Management. DDG served `anomaly-modal` (45 occurrences
in the HTML body) for every real query.

**v0.7.7 fixes this** by re-adding `wreq-util 3.0.0-rc.12` with
`default-features = false, features = ["emulation"]` and three direct
pins in `Cargo.toml`:

- `brotli-decompressor = "=5.0.1"` (5.0.2 published 2026-06-14 widens the
  `alloc-no-stdlib` range and pulls in 3.0.0)
- `alloc-no-stdlib = "=2.0.4"` (5.0.1+ requires this exact version)
- `wreq` feature `"brotli"` re-enabled (mandatory for `emulation`)

The result: real queries return 5+ results with TLS fingerprint JA3/JA4
identical to Chrome/Safari, matching the browser probe that DDG expects.
`cargo build --release --offline` succeeds in 24.04s (faster than v0.7.6
because `brotli-decompressor 5.0.1` is smaller than 5.0.2).

**Caveat for `cargo install`**: use `--locked` (see residual GAP-WS-48
note above). Without `--locked`, the solver may pull `alloc-stdlib 0.2.3`
and the conflict returns.


## v0.7.8 — Anti-Bot Detector Overhaul + Verbose Accumulated (8 gaps)

**v0.7.8 (working tree, pending tag)** closes 8 gaps in the anti-bot
detection surface. See `docs/decisions/0002-anti-bot-detector-overhaul-v0-7-8.md`
for the full architectural decision. Headline changes:

- **`detectar_interstitial` expanded** (GAP-WS-50): `CLOUDFLARE_MARKERS`
  grew to 8 entries (`anomaly-modal`, `anomaly-modal__mask`,
  `anomaly-modal__title`, `anomaly.js?cc=botnet`, `cf-turnstile`,
  `cf-spinner`, `Just a moment`, `cf-mitigated`) plus 1 new DDG marker
  (`Unfortunately, bots use DuckDuckGo too.`). The detector now catches
  the post-2026 `anomaly-modal` interstitial that v0.7.7 missed.
- **Probe-deep uses a long calibration query** (GAP-WS-51): the hard-coded
  `q=rust` (4 chars) was replaced with the 9-word pan-gram
  `the quick brown fox jumps over the lazy dog` exposed as
  `PROBE_CALIBRATION_QUERY` in `src/lib.rs:91, 509`. Long queries trigger
  the upstream bot scoring reliably so the probe is honest.
- **`--allow-lite-fallback` now consults the detector** (GAP-WS-52): the
  predicate in `src/search.rs:559` migrated from
  `accumulated_results.is_empty()` to
  `detectar_interstitial(&first_html) != InterstitialKind::None`. When
  the flag is OFF and the detector still flags interstitial, a structured
  `tracing::warn!` is emitted with `kind = interstitial_kind.as_str()`.
- **Verbose is now cumulative** (GAP-WS-53): `-v` → `info`, `-vv` →
  `debug`, `-vvv` → `trace`. `RUST_LOG` still overrides.
- **`scraper` bumped to 0.27** (GAP-WS-54): closes RUSTSEC-2025-0057
  (`fxhash 0.2.1` unmaintained). `cargo audit --deny warnings` is now a
  CI gate in `ci.yml` and `release.yml`.
- **`wreq` comment rewritten** (GAP-WS-55): the previous text claimed a
  "regression to 5.3.0" that never happened. The new comment documents
  the real pin in `wreq 6.0.0-rc.29` and the three direct pins.
- **`buscar` subcommand hidden** (GAP-WS-56): `#[command(hide = true)]`
  keeps it invocable but removes it from `--help` to reduce noise.
- **`--retries` is now honored** (GAP-WS-57): the value was hard-coded
  to 1 in `src/parallel.rs:644`; fixed to read `cfg.retries` with clamp
  `[1, 10]` so `--retries 999` cannot trigger anti-bot defenses.

**Cross-platform impact**: zero breaking changes. JSON schema and exit
codes are unchanged. Binary size is unchanged. Build time delta is within
±5% across all targets. The new `scraper 0.27` may serialize `Selector`
slightly differently but no call site needed refactor.


## v0.7.5 → v0.7.8 Comparison Matrix

| Concern | v0.7.5 | v0.7.7 | v0.7.8 |
|---|---|---|---|
| `cargo install` on Linux | Broken (GAP-WS-48) | Works with `--locked` | Works with `--locked` |
| Real queries return results | Yes | Yes (restored via TLS fix) | Yes (with better markers) |
| Detects DDG `anomaly-modal` | No | No | Yes (8 new markers) |
| Probe-deep honest signal | Short query `rust` | Short query `rust` | Long pan-gram 9-word |
| Fallback opt-in honored | Inverted predicate | Inverted predicate | Detector-driven |
| `-vv` debug flag | Not supported | Not supported | Yes (`ArgAction::Count`) |
| `cargo audit` clean | 1 transitive advisory | 1 transitive advisory | Clean (RUSTSEC-2025-0057 closed) |
| `buscar` subcommand | Visible in `--help` | Visible in `--help` | Hidden |
| `--retries N` honored | No (hard-coded 1) | No (hard-coded 1) | Yes (clamp `[1, 10]`) |


## Residual GAP-WS-48 — When the Symptom Returns

If a user reports `E0277 the trait bound 'StandardAlloc: alloc::Allocator<T> is not satisfied`
on `cargo install` of v0.7.7 or v0.7.8, the cause is almost always
one of these:

1. **Missing `--locked`**: the solver regenerated the lockfile and pulled
   `alloc-stdlib 0.2.3` → `alloc-no-stdlib 3.0.0`. Fix:
   `cargo install duckduckgo-search-cli --locked`.
2. **Mixing v0.7.6 lock with v0.7.7 source**: some users cached the
   v0.7.6 lock and forgot to refresh. Fix: `cargo update` or remove
   `Cargo.lock` and rebuild with `--locked`.
3. **Custom registry mirror**: the mirror may be stale and serve
   `brotli-decompressor 5.0.2` instead of 5.0.1. Fix: configure the
   mirror to upstream crates.io, or use a more recent `Cargo.lock`.

The robust recipe for fresh machines is:

```bash
# Linux/macOS — explicit version + locked lock
cargo install duckduckgo-search-cli --version 0.7.7 --locked

# Windows MSVC — same, plus developer shell for cl.exe
cargo install duckduckgo-search-cli --version 0.7.7 --locked
```

Verify after install:

```bash
duckduckgo-search-cli --version          # expect 0.7.7 (or 0.7.8)
duckduckgo-search-cli -q -n 5 "rust async runtime"  # expect 5 results
```

## Chrome Requirements (v0.8.5)
- Linux: `sudo apt install google-chrome-stable xvfb` (Debian/Ubuntu)
- Linux: `sudo dnf install google-chrome-stable xorg-x11-server-Xvfb` (Fedora)
- Linux: Xvfb is auto-spawned by the CLI via `spawn_virtual_display()` (v0.8.5+) — no manual `xvfb-run` needed
- Linux: if Xvfb is not installed, Chrome falls back to headless (with anti-bot detection risk)
- macOS: Install Chrome from https://www.google.com/chrome/ (Chrome runs headless on macOS)
- Windows: Install Chrome from https://www.google.com/chrome/ (Chrome runs headless on Windows)
- Chrome is auto-detected via `detect_chrome()` in `src/browser.rs`
- Build without Chrome: `cargo build --no-default-features`


Read this document in [Português](CROSS_PLATFORM.pt-BR.md).
