# Testing Guide

This guide covers test execution, categorization, and CI integration for
`duckduckgo-search-cli`.

## Why Categorized Tests

The test suite is split into four categories to balance speed, isolation,
and coverage:

| Category       | Speed      | Isolation   | Real I/O  | Count (v0.6.5) |
|----------------|------------|-------------|-----------|----------------|
| Unit           | < 1 s      | per-fn      | none      | 243            |
| Integration    | < 30 s     | per-test    | localhost | 84             |
| Doc            | < 5 s      | per-doc     | none      | 6              |
| Loom           | n/a        | n/a         | n/a       | 0 (gated)      |

## Test Categories

### Unit Tests
Located in `src/**/tests` modules (mod tests). Fast, in-process, no I/O.
Run with:

```bash
cargo test --lib
```

### Integration Tests
Located in `tests/*.rs` files. Use wiremock (no real HTTP), assert_cmd (no real
subprocess spawn), and tempfile (no real FS writes outside tmpdir).

```bash
# All integration tests
cargo test --tests

# Single integration test file
cargo test --test integration_wiremock
```

### Doc Tests
Located in `///` examples throughout `src/`. Compiled and executed by `cargo test --doc`.

```bash
cargo test --doc
```

### Loom Tests
Located in `tests/loom_atomics.rs`. Gated by `--cfg loom`. NOT compiled by
default — requires explicit opt-in.

```bash
RUSTFLAGS="--cfg loom" cargo test --test loom_atomics --release
```

> **Known limitation**: Loom conflicts with `hyper-util` and currently
> compiles but does not run cleanly. Issue tracked upstream.


## How to Run

### Local Development

```bash
# Quick feedback loop
timeout 300 cargo test --all-features --locked

# Specific category
cargo test --lib --locked
cargo test --tests --locked
cargo test --doc --locked
```

### With Coverage

```bash
# Install cargo-llvm-cov
cargo install cargo-llvm-cov

# Run with HTML report
cargo llvm-cov --all-features --locked --html --open

# Run with text summary only
cargo llvm-cov --all-features --locked --summary-only
```

Minimum line coverage: **80%**. CI fails below this threshold.

### Property-Based Tests (v0.6.5, WS-11)

5 invariants in `src/extraction.rs`:

```bash
cargo test ws11_
# Run all 5 property tests:
# - ws11_invariant_empty_inputs_yield_empty_results
# - ws11_invariant_positions_are_dense_and_one_based
# - ws11_invariant_urls_are_normalized_to_absolute
# - ws11_invariant_extraction_is_idempotent
# - ws11_invariant_malformed_html_does_not_panic
```

### WireMock Retry-After Test (v0.6.5, WS-23)

```bash
cargo test --test integration_wiremock test_retry_after_header_respected
```

### Circuit Breaker Tests (v0.6.5, WS-12)

```bash
cargo test ws12_
# Tests: ws12_breaker_allows_when_closed,
#        ws12_breaker_opens_after_threshold_failures,
#        ws12_breaker_resets_on_success,
#        ws12_breaker_half_opens_after_cooldown
```


## Environment Variables

| Variable                        | Effect                                                |
|---------------------------------|-------------------------------------------------------|
| `RUST_TEST_THREADS`             | Number of parallel test threads (default 1)            |
| `RUST_BACKTRACE`                | Set to `1` or `full` for detailed backtraces           |
| `RUST_LOG`                      | Tracing filter (`debug`, `info`, `warn`, `error`)     |
| `CARGO_TERM_COLOR`              | Force ANSI colors (`always`, `never`, `auto`)         |
| `LOOM_MAX_PREEMPTIONS`          | Max preemption bound for loom tests                    |
| `WIREMOCK_LOG`                  | WireMock request/response logging                      |


## CI Profiles

Three CI jobs run the test suite:

1. **`validate` matrix** — `cargo test --all-features --locked` on Linux, macOS, Windows
2. **`msrv`** — `cargo check --all-targets --all-features --locked` on Rust 1.75
3. **`coverage`** — `cargo llvm-cov --all-features --locked --fail-under-lines 80` on Linux

Plus a manual `cargo nextest` profile available locally:

```toml
# .config/nextest.toml (not in repo, per project convention)
[profile.default]
retries = 2
test-threads = 1
```


## Troubleshooting

### `flaky::lazy_template` failures
Loom tests may be flaky. Re-run with:

```bash
RUSTFLAGS="--cfg loom" cargo test --test loom_atomics --release -- --test-threads=1
```

### `wiremock::MockServer` startup timeout
Increase the wait:

```bash
WIREMOCK_LOG=info cargo test --test integration_wiremock
```

### Coverage drops below 80%
Check the HTML report for uncovered lines:

```bash
cargo llvm-cov --html --open
```

The diff will show which lines are not exercised by the test suite. Add
unit or integration tests to cover the missing branches.

### Tests pass locally but fail in CI
- Check for environment-specific behavior (paths, timeouts, locale)
- Check for `Instant::now()` non-determinism in code under test
- Use `cargo nextest` with retries to detect flaky tests:

```bash
cargo nextest run --retries 3
```
