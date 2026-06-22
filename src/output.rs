// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload: CPU-light (string formatting and stdout I/O)
//! Formatting and emission of the final result to stdout or a file.
//!
//! **INVIOLABLE RULE**: this is the ONLY module authorized to use `println!`
//! or `write!`/`writeln!` to `stdout`/output file. All other modules
//! must use `tracing::*` for logs (which go to stderr).
//!
//! Supported formats:
//! - `json` (default in pipe / whenever LLM consumes): JSON pretty-print.
//! - `text` (default in TTY): compact format optimized for LLM tokens and
//!   human reading — `[N] title / URL / snippet`.
//! - `markdown`: Markdown rendering (ideal for `.md` files / GitHub).
//! - `auto`: TTY detection — `text` in interactive terminal, `json` in pipe.
//!
//! Output routing:
//! - Without `--output PATH`: writes to `stdout`.
//! - With `--output PATH`: creates parent directories if needed, writes to
//!   the file with 0o644 permissions on Unix.

use crate::error::CliError;
use crate::pipeline::PipelineResult;
use crate::types::{MultiSearchOutput, OutputFormat, SearchOutput, SearchResult};
use std::fmt::Write as FmtWrite;
use std::fs;
use std::io::{self, Write};
use std::path::Path;

/// Prints the search result in the specified format and destination.
///
/// `output_path = None` → stdout. `Some(path)` → file (with creation of
/// parent directories if absent).
///
/// # Errors
///
/// Returns an error if writing to stdout or the output file fails, or if
/// JSON serialization of the result fails.
pub fn emit_result(
    result: &PipelineResult,
    format: OutputFormat,
    output_path: Option<&Path>,
) -> Result<(), CliError> {
    // Stream already emitted incrementally — nothing to do here.
    if matches!(result, PipelineResult::Stream(_)) {
        tracing::info!("PipelineResult::Stream — output already emitted via streaming");
        return Ok(());
    }

    let resolved_format = resolve_auto_format(format, output_path);
    let text = match result {
        PipelineResult::Single(output) => format_single(output.as_ref(), resolved_format)?,
        PipelineResult::Multi(output) => format_multi(output.as_ref(), resolved_format)?,
        PipelineResult::Stream(_) => {
            // GAP-OPS-008 (v0.8.0): unreachable!() replaced with proper Err propagation.
            // Stream variant should be consumed by the streaming consumer BEFORE emit_result
            // is called. If we reach this branch, it is a programming error (invariant violation),
            // not a condition to abort the process. Returning Err preserves cleanup paths and
            // gives the caller a structured error to log/report instead of a panic stack trace.
            return Err(CliError::InvalidConfig {
                message: "PipelineResult::Stream reached emit_result; stream variants must be consumed by the streaming consumer before non-streaming emit".to_string(),
            });
        }
    };

    match output_path {
        Some(path) => write_to_file(path, &text),
        None => write_to_stdout(&text),
    }
}

/// Backwards-compatible wrapper for callers that still use only (result, format).
/// Kept to reduce churn in existing tests; new call-sites should use
/// `emit_result` with an explicit `output_path`.
///
/// # Errors
///
/// Returns an error if writing to stdout fails or if JSON serialization fails.
pub fn emit(output: &SearchOutput, format: OutputFormat) -> Result<(), CliError> {
    let resolved_format = resolve_auto_format(format, None);
    let text = format_single(output, resolved_format)?;
    write_to_stdout(&text)
}

/// Backwards-compatible wrapper for multi-query.
///
/// # Errors
///
/// Returns an error if writing to stdout fails or if JSON serialization fails.
pub fn emit_multi(output: &MultiSearchOutput, format: OutputFormat) -> Result<(), CliError> {
    let resolved_format = resolve_auto_format(format, None);
    let text = format_multi(output, resolved_format)?;
    write_to_stdout(&text)
}

/// Resolves `OutputFormat::Auto` to the concrete format based on TTY detection.
///
/// - Outputting to file (`output_path = Some`) → JSON (stable and parseable).
/// - Auto + stdout TTY → Text (ergonomic for humans).
/// - Auto + stdout pipe → JSON (programmatic consumption).
fn resolve_auto_format(format: OutputFormat, output_path: Option<&Path>) -> OutputFormat {
    match format {
        OutputFormat::Auto => {
            if output_path.is_some() {
                OutputFormat::Json
            } else if crate::platform::stdout_is_tty() {
                OutputFormat::Text
            } else {
                OutputFormat::Json
            }
        }
        other => other,
    }
}

fn format_single(output: &SearchOutput, format: OutputFormat) -> Result<String, CliError> {
    match format {
        OutputFormat::Json | OutputFormat::Auto => {
            serde_json::to_string_pretty(output).map_err(|e| CliError::InvalidConfig {
                message: format!("failed to serialize search output as JSON: {e}"),
            })
        }
        OutputFormat::Text => Ok(format_single_text(output)),
        OutputFormat::Markdown => Ok(format_single_markdown(output)),
    }
}

fn format_multi(output: &MultiSearchOutput, format: OutputFormat) -> Result<String, CliError> {
    match format {
        OutputFormat::Json | OutputFormat::Auto => {
            serde_json::to_string_pretty(output).map_err(|e| CliError::InvalidConfig {
                message: format!("failed to serialize multi-search output as JSON: {e}"),
            })
        }
        OutputFormat::Text => Ok(format_multi_text(output)),
        OutputFormat::Markdown => Ok(format_multi_markdown(output)),
    }
}

/// `text` format for single-query — compact, optimized for LLM tokens.
///
/// ```text
/// Query: <query> | Engine: duckduckgo | Endpoint: html | Results: N
///
/// [1] <title>
///     <url>
///     <snippet>
///
/// [2] ...
/// ```
fn format_single_text(output: &SearchOutput) -> String {
    let mut buffer = String::with_capacity(100 + output.results.len() * 200);
    buffer.push_str(&format_header_text(output));
    if output.results.is_empty() {
        buffer.push_str("\n(sem resultados)\n");
        return buffer;
    }
    for result_item in &output.results {
        buffer.push('\n');
        buffer.push_str(&format_result_text(result_item));
    }
    buffer
}

fn format_multi_text(output: &MultiSearchOutput) -> String {
    let mut buffer = String::with_capacity(100 + output.searches.len() * 800);
    let _ = writeln!(
        buffer,
        "Queries: {} | Parallel: {} | Timestamp: {}",
        output.query_count, output.parallelism, output.timestamp
    );
    for (i, search) in output.searches.iter().enumerate() {
        let _ = write!(buffer, "\n========== Query #{} ==========\n", i + 1);
        buffer.push_str(&format_single_text(search));
    }
    buffer
}

fn format_header_text(output: &SearchOutput) -> String {
    format!(
        "Query: {} | Engine: {} | Endpoint: {} | Results: {}\n",
        output.query, output.engine, output.endpoint, output.result_count
    )
}

fn format_result_text(r: &SearchResult) -> String {
    let mut block = String::with_capacity(300);
    let _ = writeln!(block, "[{}] {}", r.position, r.title);
    if let Some(original) = &r.original_title {
        if !original.is_empty() {
            let _ = writeln!(block, "    (original: {})", original);
        }
    }
    let _ = writeln!(block, "    {}", r.url);
    if let Some(snippet) = &r.snippet {
        if !snippet.is_empty() {
            let _ = writeln!(block, "    {}", snippet);
        }
    }
    block
}

/// `markdown` format for single-query — ideal for `.md` files and GitHub.
///
/// ```markdown
/// # Resultados: <query>
///
/// **Motor:** duckduckgo | **Endpoint:** html | **Total:** N
///
/// ## 1. [<title>](<url>)
///
/// <snippet>
///
/// ---
///
/// ## 2. ...
/// ```
fn format_single_markdown(output: &SearchOutput) -> String {
    let mut buffer = String::with_capacity(200 + output.results.len() * 300);
    let _ = write!(buffer, "# Resultados: {}\n\n", output.query);
    let _ = write!(
        buffer,
        "**Motor:** {} | **Endpoint:** {} | **Total:** {}\n\n",
        output.engine, output.endpoint, output.result_count
    );
    if output.results.is_empty() {
        buffer.push_str("_Nenhum resultado encontrado._\n");
        return buffer;
    }
    for (i, r) in output.results.iter().enumerate() {
        if i > 0 {
            buffer.push_str("---\n\n");
        }
        let _ = write!(
            buffer,
            "## {}. [{}]({})\n\n",
            r.position,
            escapar_markdown(&r.title),
            r.url
        );
        if let Some(original) = &r.original_title {
            if !original.is_empty() {
                let _ = write!(
                    buffer,
                    "_Original title: {}_\n\n",
                    escapar_markdown(original)
                );
            }
        }
        if let Some(snippet) = &r.snippet {
            if !snippet.is_empty() {
                let _ = write!(buffer, "{}\n\n", escapar_markdown(snippet));
            }
        }
        if let Some(url_exibicao) = &r.display_url {
            if !url_exibicao.is_empty() {
                let _ = write!(buffer, "`{}`\n\n", url_exibicao);
            }
        }
    }
    buffer
}

fn format_multi_markdown(output: &MultiSearchOutput) -> String {
    let mut buffer = String::with_capacity(200 + output.searches.len() * 1200);
    let _ = write!(
        buffer,
        "# Multiple Searches ({} queries)\n\n",
        output.query_count
    );
    let _ = write!(
        buffer,
        "**Paralelismo:** {} | **Timestamp:** {}\n\n",
        output.parallelism, output.timestamp
    );
    for (i, search) in output.searches.iter().enumerate() {
        if i > 0 {
            buffer.push_str("\n---\n\n");
        }
        buffer.push_str(&format_single_markdown(search));
    }
    buffer
}

/// Escapes Markdown characters that could break rendering in titles
/// or snippets. Conservative: only escapes `[`, `]`, `*`, and backticks.
fn escapar_markdown(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + text.len() / 8);
    for ch in text.chars() {
        match ch {
            '\\' | '*' | '[' | ']' | '`' => {
                out.push('\\');
                out.push(ch);
            }
            _ => out.push(ch),
        }
    }
    out
}

/// Emits an error message to stderr. Centralizes all stderr output
/// to this module (MP-06: no `eprintln!` outside `output.rs`).
pub fn emit_stderr(msg: &str) {
    let _ = writeln!(std::io::stderr(), "{msg}");
}

fn write_to_stdout(content: &str) -> Result<(), CliError> {
    let stdout = io::stdout();
    let lock = stdout.lock();
    let mut writer = io::BufWriter::new(lock);
    writeln!(writer, "{content}").map_err(|e| map_io(e, "failed to write to stdout"))?;
    writer
        .flush()
        .map_err(|e| map_io(e, "failed to flush stdout"))?;
    Ok(())
}

#[cold]
fn map_io(e: io::Error, ctx: &str) -> CliError {
    if e.kind() == io::ErrorKind::BrokenPipe {
        CliError::BrokenPipe
    } else {
        CliError::PathError {
            message: format!("{ctx}: {e}"),
        }
    }
}

/// Checks whether a `CliError` is a `BrokenPipe` variant. Broken pipe indicates
/// the pipe reader closed (e.g. `| jaq`, `| head`) — normal behavior in Unix
/// pipelines, NOT an error.
#[inline]
pub(crate) fn is_broken_pipe(error: &CliError) -> bool {
    matches!(error, CliError::BrokenPipe)
}

/// Public: prints ONE line terminated with `\n` to stdout, with immediate flush.
/// Used by auxiliary subcommands (e.g. `init-config`) that need to emit JSON.
///
/// # Errors
///
/// Returns an error if writing to stdout fails or if the pipe is broken.
pub fn print_line_stdout(content: &str) -> Result<(), CliError> {
    write_to_stdout(content)
}

/// Public: emits a `SearchOutput` as ONE NDJSON line (compact JSON + `\n`).
///
/// If `output_file = Some`, opens the file in append mode and writes — used by
/// the `--stream` multi-query consumer to write streaming without holding everything in memory.
/// If `None`, writes to stdout with immediate flush (for real-time pipes).
///
/// # Errors
///
/// Returns an error if JSON serialization fails, if creating or appending to the
/// output file fails, or if writing to stdout fails.
pub fn emit_ndjson(
    output: &crate::types::SearchOutput,
    output_file: Option<&Path>,
) -> Result<(), CliError> {
    match output_file {
        Some(path) => {
            let line = serde_json::to_string(output).map_err(|e| CliError::InvalidConfig {
                message: format!("failed to serialize search output as NDJSON: {e}"),
            })?;
            append_line_to_file(path, &line)
        }
        None => {
            let stdout = io::stdout();
            let lock = stdout.lock();
            let mut writer = io::BufWriter::new(lock);
            serde_json::to_writer(&mut writer, output).map_err(|e| CliError::InvalidConfig {
                message: format!("failed to serialize NDJSON: {e}"),
            })?;
            writeln!(writer).map_err(|e| map_io(e, "failed to write NDJSON newline"))?;
            writer
                .flush()
                .map_err(|e| map_io(e, "failed to flush stdout"))?;
            Ok(())
        }
    }
}

/// Emits a text block (`text` format) in streaming mode, representing ONE query.
///
/// # Errors
///
/// Returns an error if writing to the output file or stdout fails.
pub fn emit_stream_text(
    index: usize,
    output: &crate::types::SearchOutput,
    output_file: Option<&Path>,
) -> Result<(), CliError> {
    let mut block = String::with_capacity(900);
    let _ = writeln!(block, "========== Query #{} ==========", index + 1);
    block.push_str(&format_single_text(output));
    emit_block_stream(&block, output_file)
}

/// Emits a Markdown block in streaming mode, representing ONE query.
///
/// # Errors
///
/// Returns an error if writing to the output file or stdout fails.
pub fn emit_stream_markdown(
    index: usize,
    output: &crate::types::SearchOutput,
    output_file: Option<&Path>,
) -> Result<(), CliError> {
    let mut block = String::with_capacity(1200);
    if index > 0 {
        block.push_str("\n---\n\n");
    }
    block.push_str(&format_single_markdown(output));
    emit_block_stream(&block, output_file)
}

/// Emits `block` to stdout or appends to the indicated file. Used by text/md streams.
fn emit_block_stream(block: &str, output_file: Option<&Path>) -> Result<(), CliError> {
    match output_file {
        Some(path) => append_line_to_file(path, block),
        None => {
            let stdout = io::stdout();
            let lock = stdout.lock();
            let mut writer = io::BufWriter::new(lock);
            write!(writer, "{block}")
                .map_err(|e| map_io(e, "failed to write streaming block to stdout"))?;
            writer
                .flush()
                .map_err(|e| map_io(e, "failed to flush stdout"))?;
            Ok(())
        }
    }
}

/// Appends ONE line to a file (append + create mode), applying 0o644 on Unix on
/// first creation. Creates parent directories if needed.
fn append_line_to_file(path: &Path, line: &str) -> Result<(), CliError> {
    use std::fs::OpenOptions;
    crate::paths::validate_output_path(path)?;
    crate::paths::create_parent_dirs(path)?;
    let needed_create = !path.exists();
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|e| CliError::PathError {
            message: format!("failed to open (append) {}: {e}", path.display()),
        })?;
    writeln!(file, "{line}").map_err(|e| CliError::PathError {
        message: format!("failed to write to {}: {e}", path.display()),
    })?;
    file.flush().map_err(|e| CliError::PathError {
        message: format!("failed to flush {}: {e}", path.display()),
    })?;
    drop(file);

    #[cfg(unix)]
    if needed_create {
        crate::paths::apply_permissions_644(path)?;
    }
    #[cfg(not(unix))]
    let _ = needed_create;

    Ok(())
}

/// Writes `content` to `path`, creating parent directories if needed.
/// Applies 0o644 permissions on Unix (owner writes, everyone reads).
fn write_to_file(path: &Path, content: &str) -> Result<(), CliError> {
    crate::paths::validate_output_path(path)?;
    crate::paths::create_parent_dirs(path)?;
    fs::write(path, content).map_err(|e| CliError::PathError {
        message: format!("failed to write file {}: {e}", path.display()),
    })?;

    crate::paths::apply_permissions_644(path)?;

    tracing::info!(path = %path.display(), bytes = content.len(), "output written to file");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{SearchMetadata, SearchResult};
    use std::collections::BTreeMap;

    fn test_output() -> SearchOutput {
        SearchOutput {
            query: "teste".to_string(),
            engine: "duckduckgo".to_string(),
            endpoint: "html".to_string(),
            timestamp: "2026-04-14T00:00:00+00:00".to_string(),
            region: "br-pt".to_string(),
            result_count: 1,
            results: vec![SearchResult {
                position: 1,
                title: "Título com [colchetes]".to_string(),
                url: "https://exemplo.com".to_string(),
                display_url: Some("exemplo.com".to_string()),
                snippet: Some("Descrição com *asteriscos* e `backticks`".to_string()),
                original_title: None,
                content: None,
                content_size: None,
                content_extraction_method: None,
            }],
            pages_fetched: 1,
            error: None,
            message: None,
            metadata: SearchMetadata {
                execution_time_ms: 100,
                selectors_hash: "abc1234567890def".to_string(),
                retries: 0,
                retries_configured: None,
                used_fallback_endpoint: false,
                concurrent_fetches: 0,
                fetch_successes: 0,
                fetch_failures: 0,
                used_chrome: false,
                chrome_attempted: false,
                user_agent: "Mozilla/5.0".to_string(),
                used_proxy: false,
                identity_used: None,
                cascade_level: None,
                pre_flight_fired: false,
                zero_cause: None,
                sugestao_proxima_acao: None,
                bytes_raw: None,
                bytes_decompressed: None,
                cascade_level_observed: None,
            },
        }
    }

    #[test]
    fn resolve_auto_format_for_file_always_json() {
        let path = Path::new("/tmp/teste.json");
        assert_eq!(
            resolve_auto_format(OutputFormat::Auto, Some(path)),
            OutputFormat::Json
        );
    }

    #[test]
    fn resolver_formato_auto_preserva_formatos_concretos() {
        assert_eq!(
            resolve_auto_format(OutputFormat::Json, None),
            OutputFormat::Json
        );
        assert_eq!(
            resolve_auto_format(OutputFormat::Text, None),
            OutputFormat::Text
        );
        assert_eq!(
            resolve_auto_format(OutputFormat::Markdown, None),
            OutputFormat::Markdown
        );
    }

    #[test]
    fn format_single_text_includes_query_and_results() {
        let output = test_output();
        let text = format_single_text(&output);
        assert!(text.contains("Query: teste"));
        assert!(text.contains("Engine: duckduckgo"));
        assert!(text.contains("Endpoint: html"));
        assert!(text.contains("Results: 1"));
        assert!(text.contains("[1] Título com [colchetes]"));
        assert!(text.contains("https://exemplo.com"));
        assert!(text.contains("Descrição com *asteriscos*"));
    }

    #[test]
    fn format_single_text_handles_zero_results() {
        let mut output = test_output();
        output.result_count = 0;
        output.results = vec![];
        let text = format_single_text(&output);
        assert!(text.contains("Results: 0"));
        assert!(text.contains("(sem resultados)"));
    }

    #[test]
    fn format_single_markdown_includes_h1_and_links() {
        let output = test_output();
        let md = format_single_markdown(&output);
        assert!(md.starts_with("# Resultados: teste\n\n"));
        assert!(md.contains("**Motor:** duckduckgo"));
        assert!(md.contains("**Total:** 1"));
        // Title with brackets must be escaped.
        assert!(md.contains("[Título com \\[colchetes\\]](https://exemplo.com)"));
        // Snippet com asteriscos e backticks devem ser escapados.
        assert!(md.contains("Descrição com \\*asteriscos\\* e \\`backticks\\`"));
        // url_exibicao deve aparecer entre crases.
        assert!(md.contains("`exemplo.com`"));
    }

    #[test]
    fn format_single_markdown_no_results_emits_warning() {
        let mut output = test_output();
        output.result_count = 0;
        output.results = vec![];
        let md = format_single_markdown(&output);
        assert!(md.contains("# Resultados: teste"));
        assert!(md.contains("_Nenhum resultado encontrado._"));
    }

    #[test]
    fn format_result_with_original_title_shows_annotation_text() {
        // "Official site" heuristic: titulo was replaced by url_exibicao,
        // titulo_original preserves the literal text. Both must appear in text.
        let mut output = test_output();
        output.results = vec![SearchResult {
            position: 1,
            title: "saofidelis.rj.gov.br".to_string(),
            url: "https://saofidelis.rj.gov.br".to_string(),
            display_url: Some("saofidelis.rj.gov.br".to_string()),
            snippet: Some("Prefeitura de São Fidélis".to_string()),
            original_title: Some("Official site".to_string()),
            content: None,
            content_size: None,
            content_extraction_method: None,
        }];
        let text = format_single_text(&output);
        assert!(text.contains("[1] saofidelis.rj.gov.br"));
        assert!(
            text.contains("(original: Official site)"),
            "text deve exibir titulo_original quando presente"
        );
    }

    #[test]
    fn format_result_with_original_title_shows_annotation_markdown() {
        let mut output = test_output();
        output.results = vec![SearchResult {
            position: 1,
            title: "saofidelis.rj.gov.br".to_string(),
            url: "https://saofidelis.rj.gov.br".to_string(),
            display_url: Some("saofidelis.rj.gov.br".to_string()),
            snippet: Some("Prefeitura".to_string()),
            original_title: Some("Official site".to_string()),
            content: None,
            content_size: None,
            content_extraction_method: None,
        }];
        let md = format_single_markdown(&output);
        assert!(md.contains("[saofidelis.rj.gov.br](https://saofidelis.rj.gov.br)"));
        assert!(
            md.contains("_Original title: Official site_"),
            "markdown deve exibir titulo_original em itálico quando presente"
        );
    }

    #[test]
    fn format_result_without_original_title_no_annotation() {
        // titulo_original = None → no noise in output.
        let output = test_output();
        let text = format_single_text(&output);
        let md = format_single_markdown(&output);
        assert!(!text.contains("(original:"));
        assert!(!md.contains("_Original title:"));
    }

    #[test]
    fn json_omits_original_title_when_absent() {
        // skip_serializing_if = "Option::is_none" ensures the field does not
        // appear in JSON when None — preserves minimal compatibility.
        let output = test_output();
        let json = serde_json::to_string(&output).expect("serialize");
        assert!(
            !json.contains("titulo_original"),
            "JSON não deve expor titulo_original quando é None"
        );
    }

    #[test]
    fn json_includes_original_title_when_present() {
        let mut output = test_output();
        output.results[0].original_title = Some("Official site".to_string());
        let json = serde_json::to_string(&output).expect("serialize");
        assert!(json.contains("\"titulo_original\":\"Official site\""));
    }

    #[test]
    fn json_no_longer_contains_related_searches_field() {
        // Regression v0.3.0: schema dropped `buscas_relacionadas` (BREAKING).
        let output = test_output();
        let json = serde_json::to_string(&output).expect("serialize");
        assert!(
            !json.contains("buscas_relacionadas"),
            "v0.3.0 removeu buscas_relacionadas do schema JSON"
        );
    }

    #[test]
    fn format_multi_text_includes_separators_per_query() {
        let output = MultiSearchOutput {
            query_count: 2,
            timestamp: "2026-04-14T00:00:00+00:00".to_string(),
            parallelism: 3,
            searches: vec![test_output(), test_output()],
            causa_zero_histogram: BTreeMap::new(),
        };
        let text = format_multi_text(&output);
        assert!(text.contains("Queries: 2"));
        assert!(text.contains("Parallel: 3"));
        assert!(text.contains("========== Query #1 =========="));
        assert!(text.contains("========== Query #2 =========="));
    }

    #[test]
    fn format_multi_markdown_includes_overall_h1() {
        let output = MultiSearchOutput {
            query_count: 2,
            timestamp: "2026-04-14T00:00:00+00:00".to_string(),
            parallelism: 3,
            searches: vec![test_output(), test_output()],
            causa_zero_histogram: BTreeMap::new(),
        };
        let md = format_multi_markdown(&output);
        assert!(md.starts_with("# Multiple Searches (2 queries)"));
        assert!(md.contains("**Paralelismo:** 3"));
        // Each inner search must appear with its own H1.
        assert_eq!(md.matches("# Resultados: teste").count(), 2);
    }

    #[test]
    fn escapar_markdown_protege_caracteres_problematicos() {
        assert_eq!(escapar_markdown("a*b"), "a\\*b");
        assert_eq!(escapar_markdown("a[b]"), "a\\[b\\]");
        assert_eq!(escapar_markdown("a`b"), "a\\`b");
        assert_eq!(escapar_markdown("texto normal"), "texto normal");
    }

    #[test]
    fn write_to_file_creates_parent_dirs() {
        let temp = std::env::temp_dir().join(format!("ddgcli-output-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&temp);
        let file = temp.join("sub").join("nested").join("saida.txt");
        write_to_file(&file, "conteudo de teste\nlinha 2\n")
            .expect("should write file with parent directories");
        let lido = fs::read_to_string(&file).expect("file should exist");
        assert_eq!(lido, "conteudo de teste\nlinha 2\n");
        fs::remove_dir_all(&temp).ok();
    }

    #[cfg(unix)]
    #[test]
    fn write_to_file_applies_644_permissions_on_unix() {
        use std::os::unix::fs::PermissionsExt;
        let file =
            std::env::temp_dir().join(format!("ddgcli-perms-test-{}.txt", std::process::id()));
        let _ = fs::remove_file(&file);
        write_to_file(&file, "x").expect("should write");
        let metadata = fs::metadata(&file).expect("should get metadata");
        let modo = metadata.permissions().mode() & 0o777;
        assert_eq!(modo, 0o644, "permissões devem ser 0o644 (foi {modo:o})");
        fs::remove_file(&file).ok();
    }

    #[test]
    fn emitir_json_single_via_serde_continua_estavel() {
        // Regression guarantee: JSON serialization of the struct does not change.
        let output = test_output();
        let json = serde_json::to_string_pretty(&output).expect("serialization should work");
        assert!(json.contains("\"query\": \"teste\""));
        assert!(json.contains("\"quantidade_resultados\": 1"));
        assert!(json.contains("\"motor\": \"duckduckgo\""));
    }

    // -----------------------------------------------------------------------
    // Cobertura dos caminhos de streaming/arquivo
    // -----------------------------------------------------------------------

    #[test]
    fn emit_ndjson_to_file_writes_single_parseable_line() {
        let dir = tempfile::tempdir().expect("create tempdir");
        let file = dir.path().join("ndjson.log");
        let output = test_output();
        emit_ndjson(&output, Some(&file)).expect("ndjson should write");
        let content = fs::read_to_string(&file).expect("read file");
        let lines: Vec<&str> = content.lines().filter(|l| !l.trim().is_empty()).collect();
        assert_eq!(lines.len(), 1, "NDJSON = 1 linha por chamada");
        let _: serde_json::Value =
            serde_json::from_str(lines[0]).expect("NDJSON line should be valid JSON");
    }

    #[test]
    fn emit_ndjson_two_calls_append_without_truncating() {
        let dir = tempfile::tempdir().expect("create tempdir");
        let file = dir.path().join("ndjson.log");
        let output = test_output();
        emit_ndjson(&output, Some(&file)).expect("1st write");
        emit_ndjson(&output, Some(&file)).expect("2nd write (append)");
        let content = fs::read_to_string(&file).expect("read");
        let lines: Vec<&str> = content.lines().filter(|l| !l.trim().is_empty()).collect();
        assert_eq!(lines.len(), 2, "modo append: 2 chamadas = 2 linhas");
    }

    #[test]
    fn emit_ndjson_creates_parent_dirs_when_missing() {
        let dir = tempfile::tempdir().expect("create tempdir");
        // Path with 2 non-existent levels.
        let file = dir.path().join("sub/outro/out.ndjson");
        assert!(!file.parent().unwrap().exists());
        emit_ndjson(&test_output(), Some(&file)).expect("should create parents");
        assert!(file.exists(), "arquivo criado");
        assert!(file.parent().unwrap().exists(), "diretório pai criado");
    }

    #[test]
    fn emit_stream_text_to_file_includes_query_header() {
        let dir = tempfile::tempdir().expect("create tempdir");
        let file = dir.path().join("stream.txt");
        emit_stream_text(0, &test_output(), Some(&file)).expect("stream text");
        emit_stream_text(1, &test_output(), Some(&file)).expect("stream text 2");
        let content = fs::read_to_string(&file).expect("read");
        assert!(content.contains("========== Query #1 =========="));
        assert!(content.contains("========== Query #2 =========="));
        assert!(content.contains("Query: teste"));
    }

    #[test]
    fn emit_stream_markdown_separates_queries_with_divider_from_second() {
        let dir = tempfile::tempdir().expect("create tempdir");
        let file = dir.path().join("stream.md");
        emit_stream_markdown(0, &test_output(), Some(&file)).expect("1st");
        emit_stream_markdown(1, &test_output(), Some(&file)).expect("2nd");
        let content = fs::read_to_string(&file).expect("read");
        // Separador "\n---\n" deve aparecer APENAS entre blocos (uma vez para 2 queries).
        let ocorrencias = content.matches("\n---\n").count();
        assert_eq!(
            ocorrencias, 1,
            "divisor apenas entre queries (1 para 2 blocos)"
        );
        assert!(content.contains("# Resultados: teste"));
    }

    #[test]
    fn emit_result_stream_is_noop_and_does_not_create_file() {
        use crate::parallel::StreamStats;
        use crate::pipeline::PipelineResult;
        let dir = tempfile::tempdir().expect("create tempdir");
        let file = dir.path().join("nao-cria.json");
        let stream_stats = StreamStats {
            total: 3,
            successes: 3,
            errors: 0,
            start_timestamp: "2026-04-14T00:00:00Z".to_string(),
            parallelism: 2,
        };
        let res = PipelineResult::Stream(stream_stats);
        emit_result(&res, OutputFormat::Json, Some(&file)).expect("no-op OK");
        assert!(
            !file.exists(),
            "Stream não deve escrever nada em emit_result"
        );
    }

    #[test]
    fn emit_result_single_to_file_writes_formatted_json() {
        use crate::pipeline::PipelineResult;
        let dir = tempfile::tempdir().expect("create tempdir");
        let file = dir.path().join("saida.json");
        let res = PipelineResult::Single(Box::new(test_output()));
        emit_result(&res, OutputFormat::Json, Some(&file)).expect("emit");
        let content = fs::read_to_string(&file).expect("read");
        let _: serde_json::Value =
            serde_json::from_str(&content).expect("content should be valid JSON");
        assert!(content.contains("\"query\": \"teste\""));
    }

    #[test]
    fn emit_result_multi_text_to_file_contains_both_queries() {
        use crate::pipeline::PipelineResult;
        use crate::types::MultiSearchOutput;
        let dir = tempfile::tempdir().expect("create tempdir");
        let file = dir.path().join("multi.txt");
        let mut output1 = test_output();
        output1.query = "alpha".into();
        let mut output2 = test_output();
        output2.query = "beta".into();
        let multi = MultiSearchOutput {
            query_count: 2,
            timestamp: "2026-04-14T00:00:00Z".into(),
            parallelism: 2,
            searches: vec![output1, output2],
            causa_zero_histogram: BTreeMap::new(),
        };
        let res = PipelineResult::Multi(Box::new(multi));
        emit_result(&res, OutputFormat::Text, Some(&file)).expect("emit");
        let content = fs::read_to_string(&file).expect("read");
        assert!(content.contains("Query: alpha"));
        assert!(content.contains("Query: beta"));
    }

    #[test]
    fn emit_result_auto_to_file_writes_json() {
        // Auto + file → JSON (deterministic, does not depend on TTY).
        use crate::pipeline::PipelineResult;
        let dir = tempfile::tempdir().expect("create tempdir");
        let file = dir.path().join("auto.out");
        let res = PipelineResult::Single(Box::new(test_output()));
        emit_result(&res, OutputFormat::Auto, Some(&file)).expect("emit");
        let content = fs::read_to_string(&file).expect("read");
        // JSON starts with `{` and has "query".
        assert!(content.trim_start().starts_with('{'));
        assert!(content.contains("\"query\""));
    }

    #[test]
    fn is_broken_pipe_detects_broken_pipe() {
        assert!(is_broken_pipe(&CliError::BrokenPipe));
    }

    #[test]
    fn is_broken_pipe_rejects_other_errors() {
        let err = CliError::PathError {
            message: "not found".into(),
        };
        assert!(!is_broken_pipe(&err));
    }

    #[test]
    fn is_broken_pipe_rejects_network_error() {
        let err = CliError::NetworkError {
            message: "timeout".into(),
        };
        assert!(!is_broken_pipe(&err));
    }
}
