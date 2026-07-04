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
}
