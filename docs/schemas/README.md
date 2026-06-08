# JSON Schemas Index

This directory contains machine-readable JSON schemas for the public output
contracts of `duckduckgo-search-cli`. Each schema is versioned and synchronized
with the Rust type definitions in `src/types.rs`.

## Available Schemas

The following output contracts are exposed by the CLI:

| Schema (planned) | Source Type | Output |
|------------------|-------------|--------|
| `search-output.schema.json` | `SearchOutput` | Single-query JSON root `{ query, resultados, metadados }` |
| `multi-search-output.schema.json` | `MultiSearchOutput` | Multi-query JSON root `{ quantidade_queries, buscas[] }` |
| `search-result.schema.json` | `SearchResult` | Individual result row |
| `search-metadata.schema.json` | `SearchMetadata` | Latency, identity, cascade level |
| `probe-output.schema.json` | `ProbeReport` | `--probe` JSON response |
| `probe-deep-output.schema.json` (v0.7.3+) | `ProbeDeepReport` | `--probe-deep` JSON response with `status`, `cascata_motivo`, `sugestao_mitigacao`, `http_status`, `latency_ms`, `endpoint` |
| `ndjson-event.schema.json` | (planned) | NDJSON streaming event |

> **Status (v0.7.3)**: Schemas remain planned. v0.7.3 ships the Rust types
> as the source of truth, documented in `docs/AGENTS.md`, `docs/AGENTS-GUIDE.md`,
> and the inline `///` doc comments on each struct. JSON schema files will be
> generated via `schemars` derive in a future version. The `probe_deep::ProbeDeepReport`
> type (added in v0.7.3 for the `--probe-deep` flag) is the newest entry awaiting
> schema generation.


## Generation Strategy

When schemas are added, the plan is:

1. Add `schemars = "0.8"` as a dev-dependency
2. Derive `JsonSchema` on each public type in `src/types.rs`
3. Generate schemas via `cargo run --bin dump-schemas -- output/schemas/`
4. Add a CI step that fails if any `*.schema.json` is out of sync with the Rust types
5. Validate every example in `docs/COOKBOOK.md` against the schemas on every push


## Schema Coverage Checklist

When schemas land, ensure each of these is generated:

- [ ] `search-output.schema.json`
- [ ] `multi-search-output.schema.json`
- [ ] `search-result.schema.json`
- [ ] `search-metadata.schema.json`
- [ ] `probe-output.schema.json`
- [ ] `probe-deep-output.schema.json` (v0.7.3+ — for `--probe-deep` flag)
- [ ] `config.schema.json` (for `init-config --dry-run` output)
- [ ] `init-config-output.schema.json` (for `init-config` output)
- [ ] `error-response.schema.json` (exit code 2 stderr format)


## Validation

Once generated, schemas can be validated with any JSON Schema validator
against real CLI output:

```bash
# Capture real output
timeout 30 duckduckgo-search-cli -q -f json "rust" > /tmp/out.json

# Validate against schema
jaq . /tmp/out.json | jsonschema -i /dev/stdin schemas/search-output.schema.json

# Validate probe-deep output (v0.7.3+)
timeout 15 duckduckgo-search-cli --probe-deep -q -f json > /tmp/probe.json
jaq . /tmp/probe.json | jsonschema -i /dev/stdin schemas/probe-deep-output.schema.json
```


## English

This file documents the JSON schema inventory for `duckduckgo-search-cli`.
The schemas are machine-readable contracts that allow agents, IDEs, and
type-safe clients to validate CLI output without running the binary.

## Portuguese Brasileiro

Este arquivo documenta o inventário de schemas JSON para `duckduckgo-search-cli`.
Os schemas são contratos legíveis por máquina que permitem a agentes, IDEs e
clientes type-safe validar a saída da CLI sem executar o binário. A versão
v0.7.3 adicionou o tipo `probe_deep::ProbeDeepReport` (saída da flag
`--probe-deep`) como o item mais recente aguardando geração de schema.
