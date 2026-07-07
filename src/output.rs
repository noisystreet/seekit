use std::time::Instant;

use serde::Serialize;

use crate::engine::SearchResult;

/// 统一的搜索响应（供 Agent/JSON 消费）
#[derive(Debug, Clone, Serialize)]
pub struct SearchResponse {
    pub query: String,
    pub engine: String,
    pub results: Vec<SearchResult>,
    pub total_estimated: Option<usize>,
    pub took_ms: u64,
}

/// 输出格式
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum OutputFormat {
    /// 终端彩色表格输出
    Terminal,
    /// JSON 格式输出（Agent 友好）
    Json,
    /// 纯文本精简输出
    Raw,
    /// CSV 格式输出
    Csv,
    /// Markdown 格式输出
    Markdown,
}

impl std::str::FromStr for OutputFormat {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "terminal" | "tty" | "table" => Ok(Self::Terminal),
            "json" => Ok(Self::Json),
            "raw" | "text" => Ok(Self::Raw),
            "csv" => Ok(Self::Csv),
            "markdown" | "md" => Ok(Self::Markdown),
            _ => Err(format!(
                "Unknown output format: {}. Use: terminal, json, raw, csv, or markdown",
                s
            )),
        }
    }
}

/// 格式化搜索响应并打印
pub fn print_response(response: &SearchResponse, format: OutputFormat) -> anyhow::Result<()> {
    match format {
        OutputFormat::Terminal => print_terminal(response),
        OutputFormat::Json => print_json(response)?,
        OutputFormat::Raw => print_raw(response),
        #[cfg(feature = "csv")]
        OutputFormat::Csv => print_csv(response)?,
        #[cfg(not(feature = "csv"))]
        OutputFormat::Csv => {
            anyhow::bail!("CSV output is not available. Install with: cargo install seekit -F csv")
        }
        OutputFormat::Markdown => print_markdown(response),
    }
    Ok(())
}

/// 终端表格输出
fn print_terminal(response: &SearchResponse) {
    let results = &response.results;

    if results.is_empty() {
        println!("No results found for '{}'", response.query);
        return;
    }

    println!(
        "{} results for '{}' ({} engine, took {} ms)",
        results.len(),
        response.query,
        response.engine,
        response.took_ms
    );
    println!();

    for (i, result) in results.iter().enumerate() {
        // 标题（带编号）
        print!("  \x1b[1m{}. {}\x1b[0m", i + 1, result.title);
        // 评分和来源标记
        if let (Some(score), Some(sources)) = (&result.score, &result.sources) {
            let sources_str = sources.join("+");
            print!(
                " \x1b[33m[{}]\x1b[0m \x1b[36m★ {:.2}\x1b[0m",
                sources_str, score
            );
        }
        println!();
        // URL（带颜色）
        println!("     \x1b[32m{}\x1b[0m", result.url);
        // 摘要
        if !result.snippet.is_empty() {
            println!("     {}", result.snippet);
        }
        // 页面内容（如果有）
        if let Some(content) = &result.content {
            if !content.is_empty() {
                println!("     \x1b[33m── Content ──────────────────────\x1b[0m");
                for line in content.lines().take(10) {
                    if line.chars().count() > 80 {
                        println!("     {}", line.chars().take(80).collect::<String>());
                    } else {
                        println!("     {}", line);
                    }
                }
                if content.lines().count() > 10 {
                    println!("     \x1b[33m... (truncated)\x1b[0m");
                }
            }
        }
        println!();
    }
}

/// JSON 格式输出
fn print_json(response: &SearchResponse) -> anyhow::Result<()> {
    let json = serde_json::to_string_pretty(response)?;
    println!("{}", json);
    Ok(())
}

/// 纯文本精简输出
fn print_raw(response: &SearchResponse) {
    for (i, result) in response.results.iter().enumerate() {
        println!("{}\t{}\t{}", i + 1, result.title, result.url);
    }
}

/// CSV 格式输出
#[cfg(feature = "csv")]
fn print_csv(response: &SearchResponse) -> anyhow::Result<()> {
    let mut wtr = csv::Writer::from_writer(std::io::stdout());
    // 写入标题行
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
    Ok(())
}

/// Markdown 格式输出
fn print_markdown(response: &SearchResponse) {
    let results = &response.results;

    if results.is_empty() {
        println!("No results found for '{}'", response.query);
        return;
    }

    println!("## Search Results: {}", response.query);
    println!();
    println!(
        "> Engine: {} | Results: {} | Time: {} ms",
        response.engine,
        results.len(),
        response.took_ms
    );
    println!();

    for (i, result) in results.iter().enumerate() {
        println!("{}. [**{}**]({})", i + 1, result.title, result.url);
        if !result.snippet.is_empty() {
            println!("   {}", result.snippet);
        }
        if let (Some(score), Some(sources)) = (&result.score, &result.sources) {
            let sources_str = sources.join(" + ");
            println!("   > Sources: {} | Score: {:.2}", sources_str, score);
        }
        if let Some(content) = &result.content {
            if !content.is_empty() {
                println!();
                for line in content.lines().take(5) {
                    println!("   {}", line);
                }
                if content.lines().count() > 5 {
                    println!("   *... (truncated)*");
                }
            }
        }
        println!();
    }
}

pub fn build_response(
    query: &str,
    engine: &str,
    results: Vec<SearchResult>,
    start: Instant,
) -> SearchResponse {
    SearchResponse {
        query: query.to_string(),
        engine: engine.to_string(),
        total_estimated: Some(results.len()),
        took_ms: start.elapsed().as_millis() as u64,
        results,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_result(title: &str, url: &str, snippet: &str) -> SearchResult {
        SearchResult {
            title: title.to_string(),
            url: url.to_string(),
            snippet: snippet.to_string(),
            content: None,
            score: None,
            sources: None,
        }
    }

    #[test]
    fn test_output_format_from_str() {
        assert_eq!(
            "terminal".parse::<OutputFormat>().unwrap(),
            OutputFormat::Terminal
        );
        assert_eq!("json".parse::<OutputFormat>().unwrap(), OutputFormat::Json);
        assert_eq!("raw".parse::<OutputFormat>().unwrap(), OutputFormat::Raw);
        assert_eq!("csv".parse::<OutputFormat>().unwrap(), OutputFormat::Csv);
        assert_eq!(
            "markdown".parse::<OutputFormat>().unwrap(),
            OutputFormat::Markdown
        );
        assert_eq!(
            "table".parse::<OutputFormat>().unwrap(),
            OutputFormat::Terminal
        );
        assert!("unknown".parse::<OutputFormat>().is_err());
    }

    #[test]
    fn test_build_response() {
        let results = vec![
            make_result("Title1", "https://example.com/1", "Snippet1"),
            make_result("Title2", "https://example.com/2", "Snippet2"),
        ];
        let response = build_response("test", "duckduckgo", results, Instant::now());
        assert_eq!(response.query, "test");
        assert_eq!(response.engine, "duckduckgo");
        assert_eq!(response.results.len(), 2);
        assert_eq!(response.results[0].title, "Title1");
        assert_eq!(response.results[1].url, "https://example.com/2");
    }

    #[test]
    fn test_build_response_empty() {
        let results = vec![];
        let response = build_response("empty", "duckduckgo", results, Instant::now());
        assert_eq!(response.results.len(), 0);
        assert_eq!(response.total_estimated, Some(0));
    }

    #[test]
    fn test_build_response_took_ms() {
        let results = vec![make_result("T", "https://example.com", "S")];
        let response = build_response("t", "ddg", results, Instant::now());
        assert!(response.took_ms < 1000);
    }

    #[test]
    fn test_print_json_output() {
        let results = vec![make_result("Rust", "https://rust-lang.org", "A language")];
        let response = build_response("rust", "duckduckgo", results, Instant::now());
        let json = serde_json::to_string_pretty(&response).unwrap();
        assert!(json.contains("rust"));
        assert!(json.contains("duckduckgo"));
        assert!(json.contains("https://rust-lang.org"));
    }

    #[test]
    fn test_chinese_truncation_preserves_characters() {
        let chinese_text =
            "这是一段测试文本，包含多个中文字符用于测试Unicode截断功能是否正确处理多字节字符";
        let truncated = chinese_text.chars().take(20).collect::<String>();
        assert_eq!(truncated.chars().count(), 20);
        assert!(truncated.starts_with("这是一段测试文本"));
        assert!(!truncated.is_empty());
    }

    // ── OutputFormat::from_str 扩展测试 ──

    #[test]
    fn test_output_format_aliases() {
        // tty → Terminal
        assert_eq!(
            "tty".parse::<OutputFormat>().unwrap(),
            OutputFormat::Terminal
        );
        // text → Raw
        assert_eq!("text".parse::<OutputFormat>().unwrap(), OutputFormat::Raw);
        // md → Markdown
        assert_eq!(
            "md".parse::<OutputFormat>().unwrap(),
            OutputFormat::Markdown
        );
        // 空字符串 → Err
        assert!("".parse::<OutputFormat>().is_err());
        // 带空格 → Err（当前实现不 trim）
        assert!(" json ".parse::<OutputFormat>().is_err());
    }

    #[test]
    fn test_output_format_case_insensitive() {
        assert_eq!("JSON".parse::<OutputFormat>().unwrap(), OutputFormat::Json);
        assert_eq!(
            "MarkDown".parse::<OutputFormat>().unwrap(),
            OutputFormat::Markdown
        );
        assert_eq!("Raw".parse::<OutputFormat>().unwrap(), OutputFormat::Raw);
        assert_eq!("Csv".parse::<OutputFormat>().unwrap(), OutputFormat::Csv);
        assert_eq!(
            "Tty".parse::<OutputFormat>().unwrap(),
            OutputFormat::Terminal
        );
        assert_eq!("Text".parse::<OutputFormat>().unwrap(), OutputFormat::Raw);
        assert_eq!(
            "Md".parse::<OutputFormat>().unwrap(),
            OutputFormat::Markdown
        );
    }

    #[test]
    fn test_output_format_error_message() {
        let err = "foobar".parse::<OutputFormat>().unwrap_err();
        assert!(err.contains("foobar"));
        assert!(err.contains("terminal, json, raw, csv, or markdown"));
    }

    // ── build_response 扩展测试 ──

    fn make_result_full(
        title: &str,
        url: &str,
        snippet: &str,
        content: Option<&str>,
        score: Option<f64>,
        sources: Option<Vec<&str>>,
    ) -> SearchResult {
        SearchResult {
            title: title.to_string(),
            url: url.to_string(),
            snippet: snippet.to_string(),
            content: content.map(|s| s.to_string()),
            score,
            sources: sources.map(|v| v.iter().map(|s| s.to_string()).collect()),
        }
    }

    #[test]
    fn test_build_response_with_all_fields() {
        let results = vec![make_result_full(
            "Rust Lang",
            "https://rust-lang.org",
            "A systems language",
            Some("Rust is fast and safe."),
            Some(0.95),
            Some(vec!["google", "duckduckgo"]),
        )];
        let response = build_response("rust", "fusion", results, Instant::now());
        assert_eq!(response.query, "rust");
        assert_eq!(response.engine, "fusion");
        assert_eq!(response.results.len(), 1);
        let r = &response.results[0];
        assert_eq!(r.title, "Rust Lang");
        assert_eq!(r.url, "https://rust-lang.org");
        assert_eq!(r.snippet, "A systems language");
        assert_eq!(r.content.as_deref(), Some("Rust is fast and safe."));
        assert_eq!(r.score, Some(0.95));
        assert_eq!(
            r.sources,
            Some(vec!["google".to_string(), "duckduckgo".to_string()])
        );
    }

    #[test]
    fn test_build_response_unicode_query() {
        let results = vec![make_result("结果1", "https://example.com/1", "摘要1")];
        let response = build_response("搜索查询", "bing", results, Instant::now());
        assert_eq!(response.query, "搜索查询");
        assert_eq!(response.results[0].title, "结果1");
    }

    // ── JSON 序列化结构测试 ──

    #[test]
    fn test_json_structure_has_all_keys() {
        let results = vec![make_result("Title", "https://example.com", "Snippet")];
        let response = build_response("test", "ddg", results, Instant::now());
        let json = serde_json::to_value(&response).unwrap();
        let map = json.as_object().unwrap();

        // 顶层字段
        assert!(map.contains_key("query"));
        assert!(map.contains_key("engine"));
        assert!(map.contains_key("results"));
        assert!(map.contains_key("total_estimated"));
        assert!(map.contains_key("took_ms"));

        assert_eq!(map["query"], "test");
        assert_eq!(map["engine"], "ddg");
        assert_eq!(map["total_estimated"], 1);
        assert!(map["took_ms"].as_u64().is_some());

        // results 是数组
        let results_arr = map["results"].as_array().unwrap();
        assert_eq!(results_arr.len(), 1);
        let r = &results_arr[0];
        assert!(r.get("title").is_some());
        assert!(r.get("url").is_some());
        assert!(r.get("snippet").is_some());
    }

    #[test]
    fn test_json_skip_serializing_score_sources_when_none() {
        let results = vec![make_result("Title", "https://example.com", "Snippet")];
        let response = build_response("t", "ddg", results, Instant::now());
        let json = serde_json::to_value(&response).unwrap();
        let result = &json["results"][0];
        // score 和 sources 为 None 时不应出现在 JSON 中（有 skip_serializing_if）
        assert!(result.get("score").is_none());
        assert!(result.get("sources").is_none());
        // content 没有 skip_serializing_if，None 时会序列化为 null
        assert!(result.get("content").is_some());
        assert!(result["content"].is_null());
    }

    #[test]
    fn test_json_includes_score_and_sources_when_present() {
        let results = vec![make_result_full(
            "Title",
            "https://example.com",
            "Snippet",
            Some("Content here"),
            Some(0.88),
            Some(vec!["engine_a", "engine_b"]),
        )];
        let response = build_response("t", "fusion", results, Instant::now());
        let json = serde_json::to_value(&response).unwrap();
        let result = &json["results"][0];

        assert_eq!(result["score"], serde_json::json!(0.88));
        assert_eq!(
            result["sources"],
            serde_json::json!(["engine_a", "engine_b"])
        );
        assert_eq!(result["content"], "Content here");
    }

    #[test]
    fn test_json_empty_results_array() {
        let results: Vec<SearchResult> = vec![];
        let response = build_response("empty", "ddg", results, Instant::now());
        let json = serde_json::to_value(&response).unwrap();
        assert_eq!(json["results"], serde_json::json!([]));
        assert_eq!(json["total_estimated"], 0);
    }

    // ── JSON 错误输出格式测试（输出.output.rs 级别的错误 JSON 结构验证）──

    #[test]
    fn test_json_error_format_structure() {
        // 模拟 handle_search_error 中使用的 JSON 错误结构
        let error_obj = serde_json::json!({
            "error": "No results found for query: test",
            "query": "test",
            "engine": "duckduckgo",
        });
        let json = serde_json::to_string_pretty(&error_obj).unwrap();
        assert!(json.contains("\"error\""));
        assert!(json.contains("\"query\""));
        assert!(json.contains("\"engine\""));
        assert!(json.contains("No results found for query: test"));
        assert!(json.contains("duckduckgo"));

        // 验证能正确反序列化
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["error"], "No results found for query: test");
        assert_eq!(parsed["query"], "test");
        assert_eq!(parsed["engine"], "duckduckgo");
    }

    #[test]
    fn test_json_error_format_with_special_chars() {
        let error_obj = serde_json::json!({
            "error": "HTTP 429: Too Many Requests (rate limited)",
            "query": "rust web 框架",
            "engine": "google",
        });
        let json = serde_json::to_string_pretty(&error_obj).unwrap();
        assert!(json.contains("429"));
        assert!(json.contains("rust web 框架"));

        // 反序列化验证中文正常
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["query"], "rust web 框架");
    }

    // ── Unicode 截断扩展测试 ──

    #[test]
    fn test_unicode_truncation_mixed_cjk_latin() {
        let mixed = "Rust语言 2024 版本特性 🦀 cool!";
        // 取前 10 个 char
        let truncated: String = mixed.chars().take(10).collect();
        assert_eq!(truncated.chars().count(), 10);
        // emoji 占用多个字节但 chars() 处理正确
        assert!(truncated.starts_with("Rust语言 2"));
    }

    #[test]
    fn test_unicode_truncation_edge_cases() {
        // 空字符串
        let empty = "";
        let truncated: String = empty.chars().take(10).collect();
        assert!(truncated.is_empty());

        // 少于截断长度
        let short = "你好";
        let truncated: String = short.chars().take(10).collect();
        assert_eq!(truncated, "你好");

        // 正好等于截断长度
        let exact = "🦀🦀🦀";
        let truncated: String = exact.chars().take(3).collect();
        assert_eq!(truncated, "🦀🦀🦀");
    }

    // ── SearchResponse 工具方法测试 ──

    #[test]
    fn test_search_response_display_trait() {
        let results = vec![make_result("Title", "https://example.com", "Snippet")];
        let response = build_response("test", "ddg", results, Instant::now());
        // Debug 输出应包含关键信息
        let debug_str = format!("{:?}", response);
        assert!(debug_str.contains("test"));
        assert!(debug_str.contains("ddg"));
        assert!(debug_str.contains("Title"));
    }
}
