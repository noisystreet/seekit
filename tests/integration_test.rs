use clap::Parser;
use seekit::cli::Cli;
use seekit::engine::{EngineConfig, EngineType, SearchResult};
use seekit::output::{OutputFormat, SearchResponse};

/// 验证 SearchResult 结构体的基本属性
#[test]
fn test_search_result_creation() {
    let result = SearchResult {
        title: "Rust Programming Language".to_string(),
        url: "https://rust-lang.org".to_string(),
        snippet: "A language empowering everyone.".to_string(),
        content: None,
        score: None,
        sources: None,
    };

    assert_eq!(result.title, "Rust Programming Language");
    assert_eq!(result.url, "https://rust-lang.org");
    assert_eq!(result.snippet, "A language empowering everyone.");
}

/// 验证 EngineConfig 默认值
#[test]
fn test_engine_config_default() {
    let config = EngineConfig::default();
    assert_eq!(config.max_results, 10);
    assert_eq!(config.timeout_secs, 10);
    assert!(config.safe_search);
    assert!(config.lang.is_none());
    assert_eq!(config.page, 1);
}

/// 验证 EngineConfig 自定义
#[test]
fn test_engine_config_custom() {
    let config = EngineConfig {
        max_results: 5,
        timeout_secs: 30,
        safe_search: false,
        lang: Some("zh".to_string()),
        page: 2,
    };

    assert_eq!(config.max_results, 5);
    assert_eq!(config.timeout_secs, 30);
    assert!(!config.safe_search);
    assert_eq!(config.lang.as_deref(), Some("zh"));
    assert_eq!(config.page, 2);
}

/// 验证 EngineType 解析
#[test]
fn test_engine_type_from_str() {
    assert_eq!(
        "duckduckgo".parse::<EngineType>().unwrap(),
        EngineType::DuckDuckGo
    );
    assert_eq!("ddg".parse::<EngineType>().unwrap(), EngineType::DuckDuckGo);
    assert_eq!(
        "searxng".parse::<EngineType>().unwrap(),
        EngineType::SearXNG
    );
    assert_eq!("searx".parse::<EngineType>().unwrap(), EngineType::SearXNG);
    assert!("invalid".parse::<EngineType>().is_err());
}

/// 验证 OutputFormat 解析
#[test]
fn test_output_format_parse() {
    assert_eq!(
        "terminal".parse::<OutputFormat>().unwrap(),
        OutputFormat::Terminal
    );
    assert_eq!("json".parse::<OutputFormat>().unwrap(), OutputFormat::Json);
    assert_eq!("raw".parse::<OutputFormat>().unwrap(), OutputFormat::Raw);
    assert!("invalid".parse::<OutputFormat>().is_err());
}

/// 验证 CLI 参数结构
#[test]
fn test_cli_defaults() {
    let cli = Cli::parse_from(&["seekit", "test query"]);

    assert_eq!(cli.query.as_deref(), Some("test query"));
    assert_eq!(cli.engine, "duckduckgo");
    assert!(cli.searxng_url.is_none());
    assert_eq!(cli.lang, "en");
    assert_eq!(cli.format, "terminal");
    assert_eq!(cli.max_results, 10);
    assert_eq!(cli.cache_ttl, 300);
    assert_eq!(cli.timeout, 10);
    assert!(!cli.no_safe);
    assert!(!cli.no_cache);
    assert!(!cli.clear_cache);
    assert!(!cli.init_config);
}

/// 验证 CLI 自定义参数
#[test]
fn test_cli_custom_args() {
    let cli = Cli::parse_from(&[
        "seekit",
        "rust",
        "--engine",
        "searxng",
        "--searxng-url",
        "http://localhost:8888",
        "--format",
        "json",
        "--max-results",
        "5",
        "--timeout",
        "30",
        "--no-safe",
        "--no-cache",
    ]);

    assert_eq!(cli.query.as_deref(), Some("rust"));
    assert_eq!(cli.engine, "searxng");
    assert_eq!(cli.searxng_url.as_deref(), Some("http://localhost:8888"));
    assert_eq!(cli.format, "json");
    assert_eq!(cli.max_results, 5);
    assert_eq!(cli.timeout, 30);
    assert!(cli.no_safe);
    assert!(cli.no_cache);
}

/// 验证 CLI --lang 参数
#[test]
fn test_cli_lang() {
    let cli = Cli::parse_from(&["seekit", "--lang", "zh", "test"]);
    assert_eq!(cli.lang, "zh");
}
#[test]
fn test_cli_lang_default_en() {
    let cli = Cli::parse_from(&["seekit", "test"]);
    assert_eq!(cli.lang, "en");
}

/// 验证 CLI --cache-ttl 参数
#[test]
fn test_cli_cache_ttl() {
    let cli = Cli::parse_from(&["seekit", "--cache-ttl", "60", "q"]);
    assert_eq!(cli.cache_ttl, 60);
}

#[test]
fn test_cli_cache_ttl_zero() {
    let cli = Cli::parse_from(&["seekit", "--cache-ttl", "0", "q"]);
    assert_eq!(cli.cache_ttl, 0);
}

/// 验证 CLI --fetch 和 --max-content-length 参数
#[test]
fn test_cli_fetch_flag() {
    let cli = Cli::parse_from(&["seekit", "--fetch", "q"]);
    assert!(cli.fetch);
}

#[test]
fn test_cli_fetch_default_false() {
    let cli = Cli::parse_from(&["seekit", "q"]);
    assert!(!cli.fetch);
}

#[test]
fn test_cli_max_content_length() {
    let cli = Cli::parse_from(&["seekit", "--max-content-length", "1000", "q"]);
    assert_eq!(cli.max_content_length, 1000);
}

#[test]
fn test_cli_fetch_short_flag() {
    let cli = Cli::parse_from(&["seekit", "-F", "q"]);
    assert!(cli.fetch);
}

/// 验证 CLI --clear-cache 和 --init-config 不需要 query
#[test]
fn test_cli_clear_cache_no_query() {
    let cli = Cli::parse_from(&["seekit", "--clear-cache"]);
    assert!(cli.clear_cache);
    assert!(cli.query.is_none());
}

#[test]
fn test_cli_init_config_no_query() {
    let cli = Cli::parse_from(&["seekit", "--init-config"]);
    assert!(cli.init_config);
    assert!(cli.query.is_none());
}

/// 验证 CLI 引擎快捷别名
#[test]
fn test_cli_engine_ddg_alias() {
    let cli = Cli::parse_from(&["seekit", "--engine", "ddg", "query"]);
    assert_eq!(cli.engine, "ddg");
}

#[test]
fn test_cli_engine_searx_alias() {
    let cli = Cli::parse_from(&["seekit", "--engine", "searx", "query"]);
    assert_eq!(cli.engine, "searx");
}

/// 验证 EngineType::Auto 别名
#[test]
fn test_engine_type_auto() {
    assert_eq!("auto".parse::<EngineType>().unwrap(), EngineType::Auto);
    assert_eq!("all".parse::<EngineType>().unwrap(), EngineType::Auto);
    assert_eq!("multi".parse::<EngineType>().unwrap(), EngineType::Auto);
}

#[test]
fn test_cli_engine_auto() {
    let cli = Cli::parse_from(&["seekit", "--engine", "auto", "query"]);
    assert_eq!(cli.engine, "auto");
}

/// 验证 SearchResponse JSON 序列化
#[test]
fn test_search_response_json_serialization() {
    let results = vec![SearchResult {
        title: "Test".to_string(),
        url: "https://test.com".to_string(),
        snippet: "Test snippet".to_string(),
        content: None,
        score: None,
        sources: None,
    }];

    let response = SearchResponse {
        query: "test".to_string(),
        engine: "duckduckgo".to_string(),
        results,
        total_estimated: Some(1),
        took_ms: 100,
    };

    let json = serde_json::to_string_pretty(&response).unwrap();
    assert!(json.contains("\"query\": \"test\""));
    assert!(json.contains("\"engine\": \"duckduckgo\""));
    assert!(json.contains("\"title\": \"Test\""));
    assert!(json.contains("\"url\": \"https://test.com\""));
}

/// 验证空结果搜索响应
#[test]
fn test_empty_search_response() {
    let response = SearchResponse {
        query: "empty".to_string(),
        engine: "duckduckgo".to_string(),
        results: vec![],
        total_estimated: Some(0),
        took_ms: 0,
    };

    assert!(response.results.is_empty());
    assert_eq!(response.total_estimated, Some(0));
}
