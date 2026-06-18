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
pub mod output;

use std::time::Instant;

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

    match engine_type {
        EngineType::DuckDuckGo => {
            let engine = engine::duckduckgo::DuckDuckGo::new()?;
            Ok(Box::new(engine))
        }
        EngineType::SearXNG => {
            // 优先使用 CLI 参数 --searxng-url，其次从配置文件读取
            let base_url = cli
                .searxng_url
                .clone()
                .or_else(|| config::SearchConfig::load().general.searxng_url);
            let base_url = base_url.unwrap_or_else(|| "http://localhost:8080".to_string());
            Ok(Box::new(engine::searxng::SearXNG::new(&base_url)?))
        }
        EngineType::Auto => {
            let base_url = cli
                .searxng_url
                .clone()
                .or_else(|| config::SearchConfig::load().general.searxng_url)
                .unwrap_or_else(|| "http://localhost:8080".to_string());
            Ok(Box::new(engine::fusion::AutoEngine::new(&base_url)?))
        }
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

    // 初始化日志（使用 try_init 避免测试/库模式重复初始化导致 panic）
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "seekit=warn".into()),
        )
        .try_init();

    // 处理管理命令（不需要 query）
    if cli.clear_cache {
        return handle_clear_cache();
    }
    if cli.init_config {
        return handle_init_config();
    }

    // 检查 query 是否存在
    let query = cli.query.as_deref().unwrap_or("");
    if query.is_empty() {
        anyhow::bail!("搜索关键词不能为空。使用 --help 查看帮助。");
    }

    // 解析输出格式
    let format: OutputFormat = cli.format.parse().map_err(|e| anyhow::anyhow!("{}", e))?;

    // 执行搜索（JSON 模式下，错误以 JSON 格式输出，不再调用 process::exit）
    let response = search(&cli).await.map_err(|e| {
        if format == OutputFormat::Json {
            if let Ok(json) = serde_json::to_string_pretty(&serde_json::json!({
                "error": e.to_string(),
                "query": query,
                "engine": cli.engine,
            })) {
                println!("{}", json);
            }
        }
        anyhow::anyhow!("{}", e)
    })?;

    // 输出结果
    print_response(&response, format)?;

    Ok(())
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
