#![forbid(unsafe_code)]

//! A CLI web search tool supporting DuckDuckGo, SearXNG, and multi-engine fusion.
//!
//! # Quick Start
//!
//! ```rust,no_run
//! use clap::Parser;
//! use seekit::cli::Cli;
//!
//! // Parse CLI args and run
//! # async fn example() -> anyhow::Result<()> {
//! let cli = Cli::parse_from(&["seekit", "rust programming"]);
//! let response = seekit::search(&cli).await?;
//! println!("Found {} results", response.total_estimated.unwrap_or(0));
//! # Ok(())
//! # }
//! ```
//!
//! # Features
//!
//! - **DuckDuckGo** — Zero-config, no API key required
//! - **SearXNG** — Self-hosted meta-search engine
//! - **Auto mode** — Parallel multi-engine search with result fusion
//! - **Page fetching** — Convert result HTML to Markdown (`--fetch`)
//! - **Multiple formats** — Terminal, JSON, Raw output
//! - **Disk cache** — Configurable TTL with SHA256 keyed cache

pub mod cache;
pub mod cli;
pub mod config;
pub mod engine;
pub mod error;
pub mod fetcher;
pub mod mcp;
pub mod output;
#[cfg(feature = "repl")]
pub mod repl;

use std::io::Write;
use std::time::Instant;

use clap::CommandFactory;
use clap_complete::{generate, Shell};
use cli::Cli;
use engine::{EngineConfig, EngineType, SearchEngine};
use error::Result;
use output::{build_response, print_response, OutputFormat, SearchResponse};

/// 根据 CLI 参数构建引擎配置
fn build_engine_config(cli: &Cli) -> EngineConfig {
    EngineConfig {
        max_results: cli.max_results,
        timeout_secs: cli.timeout,
        safe_search: !cli.no_safe,
        lang: if cli.lang.is_empty() {
            None
        } else {
            Some(cli.lang.clone())
        },
        page: cli.page,
    }
}

/// 检查缓存是否启用且命中，返回 (缓存命中的响应, 缓存实例)
fn check_cache(
    cli: &Cli,
    engine_name: &str,
    query: &str,
    start: Instant,
) -> (Option<SearchResponse>, Option<cache::SearchCache>) {
    if cli.max_results == 0 || cli.no_cache {
        return (None, None);
    }
    let c = cache::SearchCache::new().with_ttl(std::time::Duration::from_secs(cli.cache_ttl));
    if let Some(results) = c.get(engine_name, query, cli.max_results) {
        tracing::debug!("Cache hit for '{}'", query);
        return (
            Some(build_response(query, engine_name, results, start)),
            None,
        );
    }
    (None, Some(c))
}

/// 根据 CLI 参数和配置文件创建对应的搜索引擎实例
fn create_engine(cli: &Cli) -> Result<Box<dyn SearchEngine>> {
    let engine_type: EngineType = cli
        .engine
        .parse()
        .map_err(|e: String| error::SearchError::Config(e))?;

    let proxy_url = cli.proxy.as_deref();

    match engine_type {
        EngineType::DuckDuckGo => Ok(Box::new(engine::duckduckgo::DuckDuckGo::new(proxy_url)?)),
        EngineType::Google => Ok(Box::new(engine::google::Google::new(proxy_url)?)),
        EngineType::Bing => Ok(Box::new(engine::bing::Bing::new(proxy_url)?)),
        EngineType::Brave => Ok(Box::new(engine::brave::Brave::new(proxy_url)?)),
        EngineType::SearXNG => {
            let base_url = resolve_searxng_url(cli);
            Ok(Box::new(engine::searxng::SearXNG::new(
                &base_url, proxy_url,
            )?))
        }
        EngineType::Auto => {
            let base_url = resolve_searxng_url(cli);
            Ok(Box::new(engine::fusion::AutoEngine::new(
                &base_url, proxy_url,
            )?))
        }
    }
}

/// 解析 SearXNG URL：优先 CLI 参数，其次配置文件
fn resolve_searxng_url(cli: &Cli) -> String {
    cli.searxng_url
        .clone()
        .or_else(|| config::SearchConfig::load().general.searxng_url)
        .unwrap_or_else(|| "http://localhost:8080".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn test_resolve_searxng_url_default_when_no_cli_and_no_config() {
        // 无 CLI 参数、无配置文件时，应返回默认 URL
        let cli = Cli::parse_from(&["seekit", "test"]);
        let url = resolve_searxng_url(&cli);
        assert_eq!(url, "http://localhost:8080");
    }

    #[test]
    fn test_resolve_searxng_url_from_cli() {
        let cli = Cli::parse_from(&["seekit", "--searxng-url", "http://myhost:8888", "test"]);
        let url = resolve_searxng_url(&cli);
        assert_eq!(url, "http://myhost:8888");
    }
}

/// Execute a search with the given CLI configuration.
///
/// Parses the engine type, builds the search engine, checks cache,
/// executes the search, optionally fetches page content, and caches results.
///
/// Returns a [`SearchResponse`] containing results and metadata.
///
/// # Errors
///
/// Returns [`SearchError`] if the query is empty, engine creation fails,
/// no results are found, or the HTTP request fails.
pub async fn search(cli: &Cli) -> Result<SearchResponse> {
    let start = Instant::now();

    let query = cli.query.as_deref().unwrap_or("");
    if query.is_empty() {
        return Err(error::SearchError::NoResults {
            query: String::new(),
        });
    }

    // 引擎配置
    let engine_config = build_engine_config(cli);

    // 创建引擎
    let engine = create_engine(cli)?;
    let engine_name = engine.name();

    // 缓存处理
    let (cached_response, cache) = check_cache(cli, engine_name, query, start);
    if let Some(resp) = cached_response {
        return Ok(resp);
    }

    // 执行搜索
    let mut results = engine.search(query, &engine_config).await?;

    // 内容提取（--fetch 启用时）
    if cli.fetch {
        let fetcher_config = fetcher::FetcherConfig {
            max_content_length: cli.max_content_length,
            proxy_url: cli.proxy.clone(),
            ..Default::default()
        };
        let fetcher = fetcher::Fetcher::new(fetcher_config)?;
        fetcher.fetch(&mut results).await;
    }

    // 写入缓存
    if let Some(ref c) = cache {
        let _ = c.set(engine_name, query, cli.max_results, &results);
    }

    Ok(build_response(query, engine_name, results, start))
}

/// Run the CLI application.
///
/// Parses command-line arguments, initializes logging, handles admin commands
/// (`--clear-cache`, `--init-config`), validates the query, executes the search,
/// and prints the output in the requested format.
///
/// This is the entry point called by [`main`](crate::main).
pub async fn run() -> anyhow::Result<()> {
    let cli = <Cli as clap::Parser>::parse();

    // 初始化日志（MCP 模式下输出到 stderr，避免污染 JSON-RPC）
    let is_mcp = cli.mcp;
    let _ = if is_mcp {
        tracing_subscriber::fmt()
            .with_env_filter(
                tracing_subscriber::EnvFilter::try_from_default_env()
                    .unwrap_or_else(|_| "seekit=warn".into()),
            )
            .with_writer(std::io::stderr)
            .try_init()
    } else {
        tracing_subscriber::fmt()
            .with_env_filter(
                tracing_subscriber::EnvFilter::try_from_default_env()
                    .unwrap_or_else(|_| "seekit=warn".into()),
            )
            .try_init()
    };

    // 处理管理命令
    if cli.clear_cache {
        return handle_clear_cache();
    }
    if cli.init_config {
        return handle_init_config();
    }
    if let Some(ref shell_str) = cli.completion {
        return handle_completion(shell_str);
    }
    if cli.mcp {
        return mcp::run_mcp_server().await;
    }

    // REPL 交互模式
    #[cfg(feature = "repl")]
    if cli.repl {
        return repl::run_repl(cli).await;
    }
    #[cfg(not(feature = "repl"))]
    if cli.repl {
        anyhow::bail!("REPL mode is not available. Install with: cargo install seekit -F repl");
    }

    // 验证并准备搜索参数
    let (query, format) = prepare_search(&cli)?;

    // 执行搜索
    let response = search(&cli)
        .await
        .map_err(|e| handle_search_error(&format, &e, query, cli.engine.as_str(), &cli.output))?;

    // 输出结果
    write_output(&response, format, &cli.output)
}

/// 验证搜索参数并解析输出格式
fn prepare_search(cli: &Cli) -> anyhow::Result<(&str, OutputFormat)> {
    let query = cli.query.as_deref().unwrap_or("");
    if query.is_empty() {
        anyhow::bail!("搜索关键词不能为空。使用 --help 查看帮助。");
    }

    let mut format: OutputFormat = cli.format.parse().map_err(|e| anyhow::anyhow!("{}", e))?;

    // --output 未指定 --format 时，从文件名推断
    if cli.format == "terminal" {
        if let Some(ref path) = cli.output {
            format = detect_format_from_extension(path);
        }
    }

    Ok((query, format))
}

/// 写入输出到文件或 stdout
fn write_output(
    response: &SearchResponse,
    format: OutputFormat,
    output_path: &Option<String>,
) -> anyhow::Result<()> {
    if let Some(ref path) = output_path {
        let output = format_response_to_string(response, format)?;
        std::fs::write(path, output)?;
    } else {
        print_response(response, format)?;
    }
    Ok(())
}

/// 处理搜索错误，JSON 模式时输出格式化错误
fn handle_search_error(
    format: &OutputFormat,
    err: &error::SearchError,
    query: &str,
    engine: &str,
    output_path: &Option<String>,
) -> anyhow::Error {
    if *format == OutputFormat::Json {
        if let Ok(json) = serde_json::to_string_pretty(&serde_json::json!({
            "error": err.to_string(),
            "query": query,
            "engine": engine,
        })) {
            if let Some(ref path) = output_path {
                let _ = std::fs::write(path, &json);
            } else {
                println!("{}", json);
            }
        }
    }
    anyhow::anyhow!("{}", err)
}

/// 从文件扩展名推断输出格式
fn detect_format_from_extension(path: &str) -> OutputFormat {
    let lower = path.to_lowercase();
    if lower.ends_with(".json") {
        OutputFormat::Json
    } else if lower.ends_with(".csv") {
        OutputFormat::Csv
    } else if lower.ends_with(".md") || lower.ends_with(".markdown") {
        OutputFormat::Markdown
    } else {
        OutputFormat::Raw
    }
}

/// 将响应格式化为字符串（不打印）
fn format_response_to_string(
    response: &SearchResponse,
    format: OutputFormat,
) -> anyhow::Result<String> {
    match format {
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(response)?;
            Ok(json)
        }
        #[cfg(feature = "csv")]
        OutputFormat::Csv => format_csv_to_string(response),
        #[cfg(not(feature = "csv"))]
        OutputFormat::Csv => {
            anyhow::bail!("CSV output is not available. Install with: cargo install seekit -F csv")
        }
        OutputFormat::Markdown => Ok(format_markdown_to_string(response)),
        OutputFormat::Raw | OutputFormat::Terminal => {
            let mut s = String::new();
            for (i, result) in response.results.iter().enumerate() {
                s.push_str(&format!("{}\t{}\t{}\n", i + 1, result.title, result.url));
            }
            Ok(s)
        }
    }
}

/// 格式化为 CSV 字符串
#[cfg(feature = "csv")]
fn format_csv_to_string(response: &SearchResponse) -> anyhow::Result<String> {
    let mut buf = Vec::new();
    {
        let mut wtr = csv::Writer::from_writer(&mut buf);
        wtr.write_record(["#", "Title", "URL", "Snippet"])?;
        for (i, result) in response.results.iter().enumerate() {
            wtr.write_record(&[
                (i + 1).to_string(),
                result.title.clone(),
                result.url.clone(),
                result.snippet.clone(),
            ])?;
        }
        wtr.flush()?;
    }
    Ok(String::from_utf8(buf)?)
}

/// 格式化为 Markdown 字符串
fn format_markdown_to_string(response: &SearchResponse) -> String {
    let mut s = String::new();
    s.push_str(&format!("## Search Results: {}\n\n", response.query));
    s.push_str(&format!(
        "> Engine: {} | Results: {} | Time: {} ms\n\n",
        response.engine,
        response.results.len(),
        response.took_ms
    ));
    for (i, result) in response.results.iter().enumerate() {
        s.push_str(&format!(
            "{}. [**{}**]({})\n",
            i + 1,
            result.title,
            result.url
        ));
        if !result.snippet.is_empty() {
            s.push_str(&format!("   {}\n", result.snippet));
        }
        if let Some(content) = &result.content {
            if !content.is_empty() {
                s.push('\n');
                for line in content.lines().take(5) {
                    s.push_str(&format!("   {}\n", line));
                }
                if content.lines().count() > 5 {
                    s.push_str("   *... (truncated)*\n");
                }
            }
        }
        s.push('\n');
    }
    s
}

/// 清空缓存
fn handle_clear_cache() -> anyhow::Result<()> {
    let cache = cache::SearchCache::new();
    cache.clear()?;
    println!("Cache cleared.");
    Ok(())
}

/// 初始化默认配置文件
fn handle_init_config() -> anyhow::Result<()> {
    let config = config::SearchConfig::default();
    config.save()?;
    println!(
        "Default config created at: {}",
        config::SearchConfig::default_path().display()
    );
    Ok(())
}

/// 生成 shell 自动补全脚本
fn handle_completion(shell_str: &str) -> anyhow::Result<()> {
    let shell = match shell_str {
        "bash" => Shell::Bash,
        "zsh" => Shell::Zsh,
        "fish" => Shell::Fish,
        "powershell" | "pwsh" => Shell::PowerShell,
        "elvish" => Shell::Elvish,
        _ => anyhow::bail!(
            "Unknown shell: {}. Supported: bash, zsh, fish, powershell, elvish",
            shell_str
        ),
    };
    let mut cmd = Cli::command();
    let mut stdout = std::io::stdout();
    generate(shell, &mut cmd, "seekit", &mut stdout);
    stdout.flush()?;
    Ok(())
}
