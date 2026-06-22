fn main() {
    let _ = generate_man_page();
}

/// Generates `duckduckgo-search-cli.1` in `OUT_DIR` using `clap_mangen`.
///
/// The CLI definition here is a best-effort mirror of `src/cli.rs`; a future
/// refactor will extract the shared Command builder. Failures are
/// intentionally swallowed because the man page is a distribution
/// convenience, not a build-critical artifact.
fn generate_man_page() -> Result<(), Box<dyn std::error::Error>> {
    use clap::{Arg, ArgAction, Command, ValueHint};
    let cmd = Command::new("duckduckgo-search-cli")
        .version(env!("CARGO_PKG_VERSION"))
        .about("DuckDuckGo search via pure HTTP, JSON output for LLMs.")
        .long_about(
            "Rust CLI that queries the static DuckDuckGo HTML endpoint using pure \
             HTTP requests, no Chrome, no paid APIs, and no cache. Returns structured \
             organic results as JSON ready for LLM consumption.",
        )
        .arg(
            Arg::new("query")
                .value_name("QUERY")
                .help("Search query")
                .required(false)
                .value_hint(ValueHint::Other),
        )
        .arg(
            Arg::new("num")
                .short('n')
                .long("num")
                .value_name("N")
                .default_value("10")
                .help("Number of organic results to return"),
        )
        .arg(
            Arg::new("endpoint")
                .long("endpoint")
                .value_name("ENDPOINT")
                .value_parser(["html", "lite"])
                .default_value("html")
                .help("DuckDuckGo endpoint: html (rich metadata) or lite (fallback)"),
        )
        .arg(
            Arg::new("format")
                .short('f')
                .long("format")
                .value_name("FORMAT")
                .value_parser(["json", "text", "markdown"])
                .default_value("json")
                .help("Output format"),
        )
        .arg(
            Arg::new("parallel")
                .short('p')
                .long("parallel")
                .value_name("N")
                .default_value("5")
                .help("Parallelism degree for multi-query mode"),
        )
        .arg(
            Arg::new("global-timeout")
                .long("global-timeout")
                .value_name("SECONDS")
                .default_value("60")
                .help("Global timeout in seconds"),
        )
        .arg(
            Arg::new("fetch-content")
                .long("fetch-content")
                .action(ArgAction::SetTrue)
                .help("Fetch and extract readable text from each result URL"),
        )
        .arg(
            Arg::new("max-content-length")
                .long("max-content-length")
                .value_name("BYTES")
                .default_value("10000")
                .help("Maximum content length in characters when --fetch-content is set"),
        )
        .arg(
            Arg::new("quiet")
                .short('q')
                .long("quiet")
                .action(ArgAction::SetTrue)
                .help("Suppress tracing logs on stderr"),
        )
        .arg(
            Arg::new("verbose")
                .short('v')
                .long("verbose")
                .action(ArgAction::Count)
                .help("Increase tracing verbosity"),
        )
        .arg(
            Arg::new("probe")
                .long("probe")
                .action(ArgAction::SetTrue)
                .help("Run a pre-flight health check without performing a real query"),
        )
        .arg(
            Arg::new("probe-deep")
                .long("probe-deep")
                .action(ArgAction::SetTrue)
                .help("Run a deep probe to detect CAPTCHA interstitials"),
        )
        .arg(
            Arg::new("identity-profile")
                .long("identity-profile")
                .value_name("PROFILE")
                .help("Pin a browser identity profile (auto, chrome-win, chrome-mac, etc.)"),
        )
        .arg(
            Arg::new("output")
                .short('o')
                .long("output")
                .value_name("PATH")
                .value_hint(ValueHint::FilePath)
                .help("Write output to PATH instead of stdout"),
        )
        .arg(
            Arg::new("queries-file")
                .long("queries-file")
                .value_name("PATH")
                .value_hint(ValueHint::FilePath)
                .help("Read queries (one per line) from PATH for batch mode"),
        );

    let out_dir = std::path::PathBuf::from(std::env::var("OUT_DIR")?);
    let man = clap_mangen::Man::new(cmd);
    let mut buffer: Vec<u8> = Default::default();
    man.render(&mut buffer)?;
    std::fs::write(out_dir.join("duckduckgo-search-cli.1"), buffer)?;
    Ok(())
}
