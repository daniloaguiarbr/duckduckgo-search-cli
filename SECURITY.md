# Security Policy


## Supported Versions
- Only the latest release receives security updates
- Version 0.6.2 is the current supported version

| Version | Supported |
|---|---|
| 0.6.2 | yes |
| 0.6.1 | no |
| < 0.6.1 | no |


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
- In scope: TLS misconfiguration that could enable MITM — the project uses `rustls`, report any fallback to unsafe cipher suites
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
- No secrets or credentials are stored — the binary holds no authentication state
- The binary does not execute subprocesses or shell commands based on search results
- TLS is enforced via `rustls` — no plain HTTP connections to the search endpoint
- The CLI is stateless: no cache, no persistent credentials — each invocation is an isolated event
- The CLI does not execute JavaScript for the search phase — DuckDuckGo HTML/Lite endpoints are parsed as static HTML


## Related Supply Chain Automation
- `cargo audit` runs against the RustSec advisory database on every push and pull request
- `cargo deny check advisories licenses bans sources` runs with policy declared in `deny.toml`
- Dependabot (weekly) opens pull requests for `cargo` and `github-actions` dependency updates
- See `.github/workflows/ci.yml` and `.github/dependabot.yml` for details
