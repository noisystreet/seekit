use clap::Parser;

/// seekit — A CLI web search tool
///
/// Supports DuckDuckGo, SearXNG, and auto (multi-engine fusion) modes.
/// DuckDuckGo is the default engine (zero config).
/// Use --engine searxng to switch to a self-hosted SearXNG instance.
/// Use --engine auto to query all available engines in parallel.
///
/// Output formats: terminal (default), json, raw, csv, markdown.
/// Use --output to write results to a file (format auto-detected from extension).
#[derive(Parser, Debug)]
#[command(name = "seekit")]
#[command(version, about)]
pub struct Cli {
    /// Search query (optional with --clear-cache or --init-config)
    pub query: Option<String>,

    /// Search engine: duckduckgo, searxng, auto
    #[arg(short = 'e', long, default_value = "duckduckgo")]
    pub engine: String,

    /// SearXNG instance URL (required for searxng engine)
    #[arg(long)]
    pub searxng_url: Option<String>,

    /// Search language: en, zh, ja, etc. (SearXNG only)
    #[arg(long, default_value = "en")]
    pub lang: String,

    /// Output format: terminal, json, raw, csv, markdown
    #[arg(short = 'f', long, default_value = "terminal")]
    pub format: String,

    /// Max results per page
    #[arg(short = 'n', long, default_value_t = 10)]
    pub max_results: usize,

    /// Page number for paginated results (starts at 1)
    #[arg(short = 'p', long, default_value_t = 1)]
    pub page: u32,

    /// Request timeout in seconds
    #[arg(short = 't', long, default_value_t = 10)]
    pub timeout: u64,

    /// Disable safe search
    #[arg(long)]
    pub no_safe: bool,

    /// Fetch page content (HTML → Markdown) for each result
    #[arg(short = 'F', long)]
    pub fetch: bool,

    /// Max characters per fetched page content (used with --fetch)
    #[arg(long, default_value_t = 5000)]
    pub max_content_length: usize,

    /// Cache TTL in seconds (default: 300)
    #[arg(long, default_value_t = 300)]
    pub cache_ttl: u64,

    /// Skip cache
    #[arg(long)]
    pub no_cache: bool,

    /// Clear all cached results
    #[arg(long)]
    pub clear_cache: bool,

    /// Generate default config file
    #[arg(long)]
    pub init_config: bool,

    /// Output file path (auto-detects format from extension: .json, .csv, .md, .txt)
    #[arg(short = 'o', long)]
    pub output: Option<String>,

    /// Generate shell completion script (bash, zsh, fish, powershell, elvish)
    #[arg(long, value_name = "SHELL")]
    pub completion: Option<String>,
}
