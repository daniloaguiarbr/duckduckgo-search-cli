# Integrations

`duckduckgo-search-cli` integrates with 16+ AI agents and automation platforms
via its stable JSON contract, deterministic exit codes, and zero-dependency
binary install. This file is a pointer to the full integration catalog.

## Full Catalog

See [`docs/INTEGRATIONS.md`](docs/INTEGRATIONS.md) for the complete
integration guide, including:

- 16 supported AI agents (Claude, GPT, Gemini, Cursor, OpenCode, etc.)
- Flag aliases introduced in each version
- Summary table consolidating all integrations
- Per-platform installation recipes
- Exit code semantics for agent decision-making
- Per-integration snippets with `timeout`, `jaq`, and `PIPESTATUS`

## Quick Reference

```bash
# Canonical invocation
timeout 60 duckduckgo-search-cli -q -f json --num 15 "query"

# Exit codes
0  success         → parse .resultados
1  runtime error   → read stderr; retry once with -v
2  config error    → re-run init-config --force
3  anti-bot block  → back off 300+ s; switch --endpoint lite
4  global timeout  → raise --global-timeout; reduce --parallel
5  zero results    → refine query or try different --lang

# Current version: v0.6.5
```

## v0.6.5 Highlights for Integrations

- **MP-26 FIX**: Windows build now compiles. Use `cargo install duckduckgo-search-cli`
  on any platform without manual patches.
- **CI-01 FIX**: CI matrix now green on all 3 SOs (Linux/macOS/Windows).
  Agents running on Windows runners can rely on the binary.
- **WS-12 Circuit breaker**: `--fetch-content --parallel` no longer cascades
  failures across hosts — one slow domain won't block the rest of the crawl.
- **WS-25 ProgressBar**: `indicatif` output to stderr auto-hides in pipes,
  so JSON pipelines on stdout stay clean.

See `CHANGELOG.md` for the complete v0.6.5 changelog and migration notes
from earlier versions.
