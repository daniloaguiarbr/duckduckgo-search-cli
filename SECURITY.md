# Security Policy


## Supported Versions
- Only the latest minor and the previous minor receive security updates
- Version 0.7.3 is the current supported version

| Version | Supported |
|---|---|
| 0.7.3 | yes |
| 0.7.2 | yes (security backports; v0.7.3 is recommended for the TLS stack fix) |
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
- **v0.7.3+**: A cookie jar is persisted to `~/.config/duckduckgo-search-cli/cookies.json` (Linux), `%APPDATA%\duckduckgo-search-cli\cookies.json` (Windows), or `~/Library/Application Support/duckduckgo-search-cli/cookies.json` (macOS). The file is written with Unix permissions `0o600` (owner read+write only). On Windows, the directory inherits the user's profile ACL. The cookies are session cookies issued by `duckduckgo.com` and `html.duckduckgo.com`. **Treat this file as you would treat any credential.** Use `--no-cookie-persistence` to keep cookies in memory only. Use `--cookies-path <PATH>` to relocate the file to an encrypted volume.
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
  End users installing the pre-built binary from crates.io are not
  affected — only source builds and the CI matrix are.
- **MSRV unchanged from v0.7.2**: `rust-version = "1.88"`.
