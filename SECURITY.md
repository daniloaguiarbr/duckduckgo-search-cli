# Security Policy


## Supported Versions
- Only the latest minor and the previous minor receive security updates
- Version 0.7.7 is the current published version (GAP-WS-49 closed — TLS fingerprint regression restored via pinned `wreq-util`)
- Version 0.7.8 is in development on `main` (8 anti-bot detector gaps closed, see ADR `0002-anti-bot-detector-overhaul-v0-7-8.md`)

| Version | Supported |
|---|---|
| 0.7.8 | yes (in development; 8 anti-bot detector gaps closed, `scraper` 0.27 resolves RUSTSEC-2025-0057) |
| 0.7.7 | yes (current published; GAP-WS-49 fixed TLS fingerprint regression via pinned `wreq-util`) |
| 0.7.6 | yes (GAP-WS-48 closed — `cargo install` build conflict via `alloc-no-stdlib =2.0.4` downgrade) |
| 0.7.5 | yes (build prereq preflight covers NASM/CMake/MSVC/Perl on Windows) |
| 0.7.4 | yes (Windows NASM build preflight, GAP-WS-28) |
| 0.7.3 | yes (TLS stack fix — `rustls` replaced by BoringSSL via `wreq 6.0.0-rc.29`, GAP-WS-27) |
| 0.7.2 | partial (security backports only) |
| 0.7.1 | partial (security fixes only; MSRV 1.85) |
| 0.7.0 | no |
| 0.6.x | no |
| < 0.6.0 | no |


## Reporting a Vulnerability
- Report security vulnerabilities via GitHub private advisory: https://github.com/daniloaguiarbr/duckduckgo-search-cli/security/advisories/new
- Include a clear description of the vulnerability and steps to reproduce
- Include the version affected and the potential impact
- DO NOT open a public GitHub issue for security vulnerabilities
- Expect an acknowledgment within 72 hours


## Disclosure Policy
- Período de embargo: 90 dias a partir do recebimento do relatório
- A vulnerabilidade NÃO será divulgada publicamente antes do término do período de embargo
- Correção e divulgação coordenada ocorrem ao final do período de embargo
- Se uma correção não puder ser entregue em 90 dias, a timeline será comunicada ao reporter


## Scope
- In scope: HTTP request construction flaws that could enable SSRF, header injection, or request smuggling
- In scope: HTML parsing weaknesses in the extraction pipeline triggered by hostile server responses
- In scope: Credential leakage through `--proxy user:pass@...` handling in logs, error messages, or output JSON
- In scope: Path traversal or symlink attacks against the output file path (`-o, --output`) or the XDG config directory
- In scope: Cookie jar tampering — the v0.7.3+ `cookies.json` file contains session cookies from DuckDuckGo and is written with 0o600 Unix permissions. Report any way to read this file as another local user, or any way the CLI sends those cookies to a non-DuckDuckGo origin.
- In scope: TLS misconfiguration that could enable MITM — the project uses BoringSSL (statically linked by `wreq`) since v0.7.3, report any fallback to unsafe cipher suites
- In scope: Supply chain issues in pinned transitive dependencies not yet documented in `deny.toml`


## Out of Scope
- Denial of service caused by the user passing pathological flags is expected behavior
- Vulnerabilities in DuckDuckGo itself — report those to DuckDuckGo
- Vulnerabilities in Chrome/Chromium used under `--features chrome` — report those to the Chromium project
- Issues requiring a compromised local user account or write access to `$XDG_CONFIG_HOME`


## Security Design Assumptions
- This CLI is a read-only HTTP client — it performs no writes to remote systems
- All external inputs (query strings, output paths) are validated before use
- Path traversal attacks are blocked: output paths with `..` components are rejected with exit code 2
- Proxy URLs are masked in logs: credentials are replaced with `[...]` before any output
- **v0.7.3+**: A cookie jar is persisted to `~/.config/duckduckgo-search-cli/cookies.json` (Linux), `%APPDATA%\duckduckgo-search-cli\cookies.json` (Windows), or `~/Library/Application Support/duckduckgo-search-cli/cookies.json` (macOS). The file is written with Unix permissions `0o600` (owner read+write only). On Windows, the directory inherits the user's profile ACL. The cookies are session cookies issued by `duckduckgo.com` and `html.duckduckgo.com`. **Treat this file as you would treat any credential.** Use `--no-cookie-persistence` to keep cookies in memory only. Use `--cookies-path <PATH>` to relocate the file to an encrypted volume (e.g., a LUKS-mounted directory or a tmpfs restricted to your UID).
- **v0.7.8+**: Verbose flag surface expanded. `-v` is info, `-vv` is debug, `-vvv` is trace (GAP-WS-53). Operators investigating anomalies can escalate log detail without recompiling. The flag `conflicts_with = "quiet"` prevents contradictory intent. Use this when reporting a suspected vulnerability — `-vvv` output is the most useful diagnostic the maintainers can receive.
- The binary does not execute subprocesses or shell commands based on search results
- **v0.7.3+**: TLS is enforced via BoringSSL (statically linked by `wreq 6.0.0-rc.29`). No plain HTTP connections to the search endpoint. The BoringSSL build is reproducible; deviations in cipher suite selection are reported via `cargo deny check`.
- **v0.7.3+**: The CLI is no longer fully stateless. Cookie jar persistence adds state across invocations. This is a deliberate trade-off to reduce CAPTCHA rate on the DuckDuckGo server. The warm-up request (`GET https://duckduckgo.com/`) is idempotent and does not persist any user-identifying data beyond the cookies themselves.
- The CLI does not execute JavaScript for the search phase — DuckDuckGo HTML/Lite endpoints are parsed as static HTML


## Related Supply Chain Automation
- `cargo audit` runs against the RustSec advisory database on every push and pull request
- `cargo deny check advisories licenses bans sources` runs with policy declared in `deny.toml`
- Dependabot (weekly) opens pull requests for `cargo` and `github-actions` dependency updates
- See `.github/workflows/ci.yml` and `.github/dependabot.yml` for details


## v0.6.5 Security Improvements

- **MP-26 (HANDLE type-safety)**: `src/platform.rs:51-69` uses `is_null()` and
  `INVALID_HANDLE_VALUE` instead of `handle != 0` and `handle as isize`. The
  Win32 API now receives a properly-typed `HANDLE` (`*mut c_void`) per the
  `windows-sys 0.59+` ABI. Eliminates UB latent in v0.6.4.
- **CI-01 (clippy lints)**: `improper_ctypes` and `improper_ctypes_definitions`
  are now `deny` in `Cargo.toml`, preventing future FFI type drift. Missing
  `Debug` impls and `clippy::needless_return` regressions are now caught
  at `cargo clippy --all-targets --all-features -- -D warnings`.
- **Lints promoted to deny**: `missing_safety_doc` and `unsafe_op_in_unsafe_fn`
  prevent underspecified `unsafe` API surface.

For vulnerabilities in v0.6.4 specifically, the Windows HANDLE cast issue
was the most prominent: a build failure on Windows that could be triggered
by `cargo install duckduckgo-search-cli`. v0.6.5 ships the type-safe fix.


## v0.7.3 Security Improvements

- **GAP-WS-27 (TLS fingerprint)**: The Cloudflare Bot Management CAPTCHA
  interstitial that affected macOS users in v0.7.2 (HTTP 200 with
  `quantidade_resultados: 0`) is fixed. The TLS stack changed from `rustls`
  to BoringSSL (statically linked by `wreq 6.0.0-rc.29`).
- **BoringSSL pinned via `wreq 6.0.0-rc`**: BoringSSL is the same TLS
  library that Chrome and Android use in production. CVEs against
  BoringSSL are tracked by Chromium and addressed in upstream commits
  that `wreq` consumes on each release.
- **Cookie jar hardening (0o600)**: The `cookies.json` file written by
  the v0.7.3+ `session` feature is created with Unix permissions `0o600`
  (owner read+write only). On Windows, the file inherits the user's
  profile directory ACL.
- **Cookie jar location is XDG-aware**: Linux follows `XDG_CONFIG_HOME`
  (defaults to `~/.config`). Windows uses `%APPDATA%`. macOS uses
  `~/Library/Application Support`. The path is overridable via
  `--cookies-path <PATH>` to point at an encrypted volume.
- **Build-time supply chain**: Compiling from source now requires
  `cmake`, `perl`, `pkg-config`, and `libclang-dev` on Linux. These are
  C toolchain components that compile the BoringSSL static library.
  **`cargo install` always compiles from source** — crates.io does not
  distribute pre-built binaries for any platform. Every Windows user must
  satisfy the four BoringSSL build prerequisites (NASM, CMake, MSVC, Perl)
  themselves. See `gaps.md` GAP-WS-28/29/30/31 and `docs/INSTALL-WINDOWS.md`
  for the full prerequisite list and step-by-step setup.
- **MSRV unchanged from v0.7.2**: `rust-version = "1.88"`.


## v0.7.8 Security Improvements

- **RUSTSEC-2025-0057 (fxhash unmaintained) RESOLVED**: The transitive
  dependency `fxhash 0.2.1` (RUSTSEC-2025-0057, marked unmaintained by the
  RustSec advisory database) is gone in v0.7.8. The bump from `scraper
  0.20.0` to `scraper 0.27.0` removed the transitive path through
  `fxhash`. The `cargo audit --deny warnings` gate now runs clean for this
  advisory. `deny.toml` no longer needs the `RUSTSEC-2025-0057` ignore
  exception. Only the `async-std` (RUSTSEC-2025-0052) ignore remains,
  scoped to the optional `chrome` feature.
- **Supply chain gate hardened**: `cargo audit --deny warnings` is now a
  blocking gate in `.github/workflows/ci.yml` and
  `.github/workflows/release.yml`. Any new RUSTSEC advisory above
  `MEDIUM` severity will fail the PR build. The previous
  `cargo audit` invocation only warned.
- **Anti-bot detector rebalance (GAP-WS-52)**: The fallback predicate
  in `src/search.rs:567-572` now reads the real detector result instead
  of a fixed assumption. When `--allow-lite-fallback` is off but the
  detector flags a CAPTCHA interstitial, the CLI emits a structured
  `tracing::warn!` and continues to exit with the appropriate code —
  it does NOT silently fall back. This removes a covert behavior
  channel that could surprise integrators expecting explicit opt-in.
- **Verbose level surface (GAP-WS-53)**: `-vv` and `-vvv` flags added
  to `src/cli.rs` via `ArgAction::Count`. Operators can now escalate
  log verbosity without recompiling. The flag `conflicts_with = "quiet"`
  prevents contradictory intent.
- **`Buscar` subcommand hidden (GAP-WS-56)**: The legacy `Buscar`
  subcommand is marked `#[command(hide = true)]`. It remains callable
  for backward compatibility but disappears from `--help`. Reduces
  surface area for confused-deputy attacks against CI scripts that
  parse `--help` output.
- **`--retries` honored end-to-end (GAP-WS-57)**: The retry counter
  in `src/parallel.rs:644` now reads `config.retries` instead of a
  hard-coded constant. The previous behavior silently dropped the
  user-supplied `--retries` value in the `error_output` path.
- **Pinned `wreq 6.0.0-rc.29` (GAP-WS-55)**: The `wreq` block in
  `Cargo.toml` was rewritten. The previous release claimed
  `wreq 5.3.0` but the actual pin in use is `6.0.0-rc.29` with three
  direct pins (`wreq-util`, `brotli-decompressor =5.0.1`,
  `alloc-no-stdlib =2.0.4`). The Cargo.toml manifest now matches
  reality — eliminates a documentation-vs-code drift that made supply
  chain audits misleading.
- **MSRV unchanged from v0.7.7**: `rust-version = "1.88"`.

For vulnerabilities introduced or surfaced by v0.7.7 specifically, the
TLS fingerprint regression (GAP-WS-49) was the most prominent: a
`wreq-util` resolution failure that broke BoringSSL emulation on certain
Linux distributions. v0.7.7 ships the pinned-`wreq-util` fix and
restored normal operation.
