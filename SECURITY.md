# Security Policy

## Supported Versions

Only the latest minor release of `duckduckgo-search-cli` receives security
updates. Older versions are not backported.

| Version | Supported |
| ------- | --------- |
| latest  | ✅        |
| older   | ❌        |

## Reporting a Vulnerability

**Do NOT open a public GitHub issue for security vulnerabilities.**

Report privately via GitHub Security Advisories:

1. Go to <https://github.com/comandoaguiar/duckduckgo-search-cli/security/advisories/new>
2. Fill in the advisory form with:
   - A clear description of the issue.
   - Reproduction steps (minimal example preferred).
   - The version(s) affected.
   - Any mitigation you've identified.

You should receive an initial response within **72 hours**. A coordinated
disclosure timeline will be agreed upon before any public announcement.

## Scope

Vulnerabilities of interest include, but are not limited to:

- **HTTP request construction flaws** that could enable SSRF, header
  injection, or request smuggling against DuckDuckGo or fetched URLs.
- **HTML parsing weaknesses** in the extraction pipeline that could be
  triggered by a hostile server response (e.g., DoS via crafted DOM, XXE
  despite HTML context, CPU-bomb selectors).
- **Credential leakage** through `--proxy user:pass@...` handling in logs,
  error messages, or the output JSON (masking should prevent this — report
  any leak).
- **Path traversal or symlink attacks** against the output file path
  (`-o, --output`) or the XDG config directory.
- **TLS misconfiguration** that could enable MITM (the project uses `rustls`
  — report any fallback to unsafe cipher suites).
- **Supply chain issues** in pinned transitive dependencies not yet
  documented in `deny.toml`.

## Out of Scope

- Denial of service caused by the user passing pathological flags
  (`--parallel 20 --pages 5 --fetch-content` on thousands of queries is
  expected to consume significant resources).
- Vulnerabilities in DuckDuckGo itself — report those to DuckDuckGo.
- Vulnerabilities in Chrome/Chromium used under `--features chrome` —
  report those to the Chromium project.
- Issues requiring a compromised local user account or write access to
  `$XDG_CONFIG_HOME`.

## Security Design Assumptions

- The CLI is **stateless** (no cache, no persistent credentials except
  optional `.cargo/credentials`) — each invocation is an isolated event.
- The CLI uses **`rustls-tls`** exclusively — no dependency on system
  OpenSSL/SChannel/SecureTransport.
- The CLI **does not execute JavaScript** for the search phase — the
  DuckDuckGo HTML/Lite endpoints are parsed as static HTML.
- When `--fetch-content` is active, fetched pages are parsed with
  `scraper` (which uses `html5ever`); untrusted HTML is expected.
- Output files are created with **`0o644`** on Unix (owner writes, world
  reads). Nothing is written outside the path the user passed.

## Related Supply Chain Automation

The project runs, on every push and pull request:

- `cargo audit` against the RustSec advisory database.
- `cargo deny check advisories licenses bans sources` with the policy
  declared in `deny.toml`.
- `dependabot` (weekly) opens PRs for `cargo` and `github-actions`
  dependency updates.

See `.github/workflows/ci.yml` and `.github/dependabot.yml` for details.
