<!-- Thanks for contributing! Fill in the relevant sections and delete the rest. -->

## Summary

<!-- 1–3 sentences: what changes, why it matters. -->

## Type of Change

- [ ] Bug fix (non-breaking)
- [ ] New feature (non-breaking)
- [ ] Breaking change (requires major version bump)
- [ ] Documentation only
- [ ] Refactor / internal (no behavior change)
- [ ] Test / CI / build only

## Checklist — 10-Gate Validation

Please confirm the following run green locally (see `CONTRIBUTING.md`):

- [ ] `cargo check-all` — compiles with `--all-targets --all-features --locked`
- [ ] `cargo lint` — clippy with `-D warnings`, zero warnings
- [ ] `cargo fmt --check` — zero formatting diffs
- [ ] `RUSTDOCFLAGS="-D warnings" cargo docs` — zero doc warnings
- [ ] `cargo test-all` — all tests pass (unit + integration + doctests)
- [ ] `cargo cov` — coverage ≥ 80% (CI enforces)
- [ ] `cargo audit` — zero known vulnerabilities
- [ ] `cargo deny check` — advisories / licenses / bans / sources all OK
- [ ] `cargo publish-check` — dry-run succeeds
- [ ] `cargo pkg-list` — no sensitive files in tarball

## Project-Specific Constraints

- [ ] **No cache** added (hard constraint per v2 blueprint).
- [ ] **No MCP / paid API / Chrome for the search phase** added.
- [ ] **`rustls-tls` only** — did not re-enable `native-tls`.
- [ ] **`println!` confined to `output.rs`** (sg -p 'println!($$$ARGS)' -l rust still zero outside it).
- [ ] Log messages and struct field names remain in **Brazilian Portuguese**.

## Related Issues / Links

<!-- Closes #N, relates to #M, references https://... -->

## Screenshots / Logs (if applicable)

<!-- Paste terminal output, JSON samples, error traces. -->

## Notes for Reviewers

<!-- Anything that makes review easier: trade-offs, alternative approaches
     considered, known limitations, follow-ups planned. -->
