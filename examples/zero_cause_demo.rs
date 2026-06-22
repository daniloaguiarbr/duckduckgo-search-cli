// SPDX-License-Identifier: MIT OR Apache-2.0
//! v0.8.0 — Example: zero-result causal classification (`causa_zero`).
//!
//! Demonstrates how to use the v0.8.0 `causa_zero` field in the JSON
//! envelope to programmatically distinguish "query legitimately
//! returned no results" from "we are being blocked by an anti-bot
//! system" — without parsing stderr.
//!
//! The classifier produces 5 possible values:
//! - `legitimo` — query genuinely returned no results
//! - `filtro-silencioso` — DDG dropped terms silently (reformulate)
//! - `ghost-block` — HTTP 200 with sub-4KB body, no markers
//! - `anti-bot` — Cloudflare/DDG interstitial detected
//! - `resposta-invalida` — empty body + zero metadata (upstream issue)
//!
//! When `causa_zero != "legitimo"`, the CLI exits with code 6
//! (`SUSPECTED_BLOCK`) and `sugestao_proxima_acao` is populated
//! with a human-readable hint.
//!
//! Run against a real DDG with:
//!   `cargo run --example zero_cause_demo -- "rust async tokio 2026"`
//!
//! The example invokes the real CLI pipeline (`duckduckgo_search_cli::run`)
//! with a query likely to trigger anti-bot, then inspects the resulting
//! JSON envelope via subprocess invocation.
//!
//! ## Expected output (against the real DDG, with a query likely to be blocked):
//!
//! ```text
//! --- zero_cause_demo ---
//! Zero-cause: "anti-bot"
//! Sugestão: "Anti-bot detectado (Cloudflare/DDG). Execute com --pre-flight..."
//! Exit code: 6
//! ```
//!
//! ## Restore legacy exit-5 behavior (BC opt-out)
//!
//! ```bash
//! DUCKDUCKGO_ZERO_CAUSE_STRICT=false duckduckgo-search-cli "query" -q -f json
//! ```
//!
//! The `causa_zero` field remains in the JSON (additive diagnostic);
//! only the exit code is suppressed from 6 to 5.
//!
//! ## Why this matters
//!
//! Pipelines that branch on exit code 5 (legacy "zero results") now see
//! exit code 6 ("suspected block") when the query is blocked. The
//! pipeline can then decide: retry with `--pre-flight`, switch to
//! `--endpoint lite`, or fall back to a different search engine — all
//! without parsing the stderr logs.

use std::process::{Command, ExitCode};

fn main() -> ExitCode {
    eprintln!("--- zero_cause_demo ---");
    eprintln!("Spawning the CLI with a query likely to trigger anti-bot...");
    eprintln!();

    let output = Command::new(
        std::env::var("CARGO_BIN_EXE_duckduckgo-search-cli")
            .unwrap_or_else(|_| "duckduckgo-search-cli".to_string()),
    )
    .args([
        "-q",
        "-f",
        "json",
        "--num",
        "5",
        "rust async tokio 2026 wreq anti-bot test",
    ])
    .output();

    match output {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            let exit_code = out.status.code().unwrap_or(-1);

            eprintln!("CLI exited with code: {exit_code}");
            eprintln!();

            // Parse the JSON envelope to extract `metadados.causa_zero`
            // and `metadados.sugestao_proxima_acao` using jaq-equivalent
            // string search. (We avoid a hard dep on jaq for an example.)
            let causa_zero = extract_json_field(&stdout, "causa_zero");
            let sugestao = extract_json_field(&stdout, "sugestao_proxima_acao");

            eprintln!("metadados.causa_zero:            {:?}", causa_zero);
            eprintln!("metadados.sugestao_proxima_acao: {:?}", sugestao);
            eprintln!();

            if let Some(c) = &causa_zero {
                eprintln!("Interpretation:");
                eprintln!("  causa_zero = \"{c}\" means:");
                match c.as_str() {
                    "legitimo" => eprintln!("    Query genuinely returned no results. Reformulate or try synonyms."),
                    "filtro-silencioso" => eprintln!("    DDG silently dropped terms. Try without quotes or with synonyms."),
                    "ghost-block" => eprintln!("    HTTP 200 with sub-4KB body. Wait 60s, switch IP, or use --pre-flight."),
                    "anti-bot" => eprintln!("    Cloudflare/DDG interstitial. Run with --pre-flight and persistent session."),
                    "resposta-invalida" => eprintln!("    Upstream sent invalid response. Check proxy/firewall or try --endpoint lite."),
                    other => eprintln!("    Unknown variant: {other}"),
                }
            } else {
                eprintln!("causa_zero is absent → query returned results (or exit was non-zero before classification).");
            }

            ExitCode::from(exit_code.max(0) as u8)
        }
        Err(e) => {
            eprintln!("failed to spawn CLI: {e}");
            ExitCode::FAILURE
        }
    }
}

/// Minimal JSON field extractor for a flat string value. Looks for
/// `"field":"value"` in the input. Sufficient for the demo; production
/// code should use `jaq` or `serde_json`.
fn extract_json_field(json: &str, field: &str) -> Option<String> {
    let needle = format!("\"{field}\":\"");
    let start = json.find(&needle)? + needle.len();
    let rest = &json[start..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}
