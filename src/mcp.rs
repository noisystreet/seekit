/// MCP (Model Context Protocol) stdio server
///
/// Implements JSON-RPC 2.0 over stdio for AI Agent integration.
/// Exposes `search` and `fetch` tools.
///
/// Protocol: https://modelcontextprotocol.io/
use crate::cli::Cli;
use crate::{fetcher, search};

/// MCP JSON-RPC 请求
#[derive(Debug, serde::Deserialize)]
struct JsonRpcRequest {
    #[allow(dead_code)]
    jsonrpc: String,
    id: Option<serde_json::Value>,
    method: String,
    #[serde(default)]
    params: serde_json::Value,
}

/// MCP JSON-RPC 响应
#[derive(Debug, serde::Serialize)]
struct JsonRpcResponse {
    jsonrpc: &'static str,
    id: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<JsonRpcError>,
}

#[derive(Debug, serde::Serialize)]
struct JsonRpcError {
    code: i32,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<serde_json::Value>,
}

impl JsonRpcResponse {
    fn success(id: Option<serde_json::Value>, result: serde_json::Value) -> Self {
        Self {
            jsonrpc: "2.0",
            id,
            result: Some(result),
            error: None,
        }
    }

    fn error(id: Option<serde_json::Value>, code: i32, message: String) -> Self {
        Self {
            jsonrpc: "2.0",
            id,
            result: None,
            error: Some(JsonRpcError {
                code,
                message,
                data: None,
            }),
        }
    }
}

/// 服务器信息
fn server_info() -> serde_json::Value {
    serde_json::json!({
        "protocolVersion": "2025-06-18",
        "capabilities": {
            "tools": {}
        },
        "serverInfo": {
            "name": "seekit",
            "version": env!("CARGO_PKG_VERSION")
        }
    })
}

/// MCP Tool 定义
fn tool_search_schema() -> serde_json::Value {
    serde_json::json!({
        "name": "search",
        "description": "Search the web using DuckDuckGo, SearXNG, or multi-engine auto mode. Returns title, URL, and snippet for each result.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "query": { "type": "string", "description": "Search query" },
                "engine": {
                    "type": "string",
                    "enum": ["duckduckgo", "searxng", "auto"],
                    "default": "duckduckgo",
                    "description": "Search engine to use"
                },
                "max_results": {
                    "type": "integer",
                    "default": 10,
                    "description": "Maximum number of results"
                },
                "page": {
                    "type": "integer",
                    "default": 1,
                    "description": "Page number (starts at 1)"
                },
                "lang": {
                    "type": "string",
                    "default": "en",
                    "description": "Search language (e.g. en, zh, ja). Used by SearXNG."
                },
                "searxng_url": {
                    "type": "string",
                    "description": "SearXNG instance URL (required when engine=searxng or auto)"
                }
            },
            "required": ["query"]
        }
    })
}

fn tool_fetch_schema() -> serde_json::Value {
    serde_json::json!({
        "name": "fetch",
        "description": "Fetch a URL and convert its content to Markdown text. Useful for reading full article content.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "url": {
                    "type": "string",
                    "format": "uri",
                    "description": "URL to fetch"
                },
                "max_content_length": {
                    "type": "integer",
                    "default": 5000,
                    "description": "Maximum characters of content to return"
                }
            },
            "required": ["url"]
        }
    })
}

/// 处理 `tools/list`
fn handle_list_tools() -> serde_json::Value {
    serde_json::json!({
        "tools": [tool_search_schema(), tool_fetch_schema()]
    })
}

/// 解析工具调用参数中的字符串
fn arg_str<'a>(args: &'a serde_json::Map<String, serde_json::Value>, key: &str) -> Option<&'a str> {
    args.get(key).and_then(|v| v.as_str())
}

/// 解析工具调用参数中的 u64
fn arg_u64(args: &serde_json::Map<String, serde_json::Value>, key: &str) -> u64 {
    args.get(key).and_then(|v| v.as_u64()).unwrap_or(0)
}

/// 执行 search 工具
async fn handle_search_call(
    args: &serde_json::Map<String, serde_json::Value>,
) -> std::result::Result<serde_json::Value, (i32, String)> {
    let query = arg_str(args, "query")
        .ok_or_else(|| (-32602, "Missing required argument: query".to_string()))?;
    let engine = arg_str(args, "engine").unwrap_or("duckduckgo");
    let max_results = arg_u64(args, "max_results").max(1) as usize;
    let page = arg_u64(args, "page").max(1) as u32;
    let lang = arg_str(args, "lang").unwrap_or("en");
    let searxng_url = arg_str(args, "searxng_url");

    let mut cli_args = vec![
        "seekit".to_string(),
        query.to_string(),
        "--engine".to_string(),
        engine.to_string(),
        "--max-results".to_string(),
        max_results.to_string(),
        "--page".to_string(),
        page.to_string(),
        "--lang".to_string(),
        lang.to_string(),
        "--format".to_string(),
        "json".to_string(),
    ];
    if let Some(url) = searxng_url {
        cli_args.push("--searxng-url".to_string());
        cli_args.push(url.to_string());
    }

    let cli = <Cli as clap::Parser>::parse_from(&cli_args);
    let response = search(&cli)
        .await
        .map_err(|e| (-32603, format!("Search failed: {}", e)))?;

    let text = response
        .results
        .iter()
        .enumerate()
        .map(|(i, r)| format!("{}. [{}]({})\n   {}", i + 1, r.title, r.url, r.snippet))
        .collect::<Vec<_>>()
        .join("\n\n");

    let summary = if text.is_empty() {
        format!("No results found for '{}'", query)
    } else {
        format!(
            "Search results for '{}' ({} engine, {:?} results):\n\n{}",
            query,
            response.engine,
            response.total_estimated.unwrap_or(0),
            text
        )
    };

    Ok(serde_json::json!({
        "content": [{"type": "text", "text": summary}]
    }))
}

/// 执行 fetch 工具
async fn handle_fetch_call(
    args: &serde_json::Map<String, serde_json::Value>,
) -> std::result::Result<serde_json::Value, (i32, String)> {
    let url = arg_str(args, "url")
        .ok_or_else(|| (-32602, "Missing required argument: url".to_string()))?;
    let max_content_length = arg_u64(args, "max_content_length").max(1) as usize;

    let fetcher_config = fetcher::FetcherConfig {
        max_content_length,
        ..Default::default()
    };
    let fetcher = fetcher::Fetcher::new(fetcher_config).map_err(|e| (-32603, e.to_string()))?;

    let mut search_results = vec![crate::engine::SearchResult {
        title: String::new(),
        url: url.to_string(),
        snippet: String::new(),
        content: None,
        score: None,
        sources: None,
    }];

    fetcher.fetch(&mut search_results).await;

    let content = search_results
        .first()
        .and_then(|r| r.content.as_deref())
        .unwrap_or("Failed to fetch content")
        .to_string();

    Ok(serde_json::json!({
        "content": [{"type": "text", "text": content}]
    }))
}

/// 分发 MCP 请求
async fn dispatch_request(
    request: JsonRpcRequest,
) -> std::result::Result<JsonRpcResponse, (JsonRpcResponse, bool)> {
    match request.method.as_str() {
        "initialize" => Ok(JsonRpcResponse::success(request.id, server_info())),
        "notifications/initialized" => Err((
            JsonRpcResponse::success(request.id, serde_json::json!(null)),
            true,
        )),
        "tools/list" => Ok(JsonRpcResponse::success(request.id, handle_list_tools())),
        "tools/call" => {
            let result = handle_call_tool(&request.params).await;
            Ok(match result {
                Ok(value) => JsonRpcResponse::success(request.id, value),
                Err((code, msg)) => JsonRpcResponse::error(request.id, code, msg),
            })
        }
        _ => Ok(JsonRpcResponse::error(
            request.id,
            -32601,
            format!("Method not found: {}", request.method),
        )),
    }
}

/// 处理 `tools/call`
async fn handle_call_tool(
    params: &serde_json::Value,
) -> std::result::Result<serde_json::Value, (i32, String)> {
    let name = params
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| (-32602, "Missing tool name".to_string()))?;

    let arguments = params
        .get("arguments")
        .and_then(|v| v.as_object())
        .ok_or_else(|| (-32602, "Missing arguments".to_string()))?;

    match name {
        "search" => handle_search_call(arguments).await,
        "fetch" => handle_fetch_call(arguments).await,
        _ => Err((-32601, format!("Unknown tool: {}", name))),
    }
}

/// 写入一行 JSON 到 stdout
async fn write_response<W: tokio::io::AsyncWriteExt + Unpin>(
    writer: &mut W,
    response: &JsonRpcResponse,
) -> anyhow::Result<()> {
    let mut line = serde_json::to_string(response)?;
    line.push('\n');
    writer.write_all(line.as_bytes()).await?;
    writer.flush().await?;
    Ok(())
}

/// 启动 MCP stdio server
pub async fn run_mcp_server() -> anyhow::Result<()> {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();
    let mut reader = tokio::io::BufReader::new(stdin);
    let mut writer = tokio::io::BufWriter::new(stdout);

    // 日志走 stderr
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "seekit=warn".into()),
        )
        .with_writer(std::io::stderr)
        .try_init();

    // 发送 server 就绪通知
    write_response(&mut writer, &JsonRpcResponse::success(None, server_info())).await?;

    // 主循环
    use tokio::io::AsyncBufReadExt;
    let mut buf = String::new();

    loop {
        buf.clear();
        let n = reader.read_line(&mut buf).await?;
        if n == 0 {
            break;
        }

        let line = buf.trim();
        if line.is_empty() {
            continue;
        }

        process_line(&mut writer, line).await?;
    }

    Ok(())
}

/// 处理一行 JSON-RPC 请求
async fn process_line<W: tokio::io::AsyncWriteExt + Unpin>(
    writer: &mut W,
    line: &str,
) -> anyhow::Result<()> {
    let request: JsonRpcRequest = match serde_json::from_str(line) {
        Ok(req) => req,
        Err(e) => {
            write_response(
                writer,
                &JsonRpcResponse::error(None, -32700, format!("Parse error: {}", e)),
            )
            .await?;
            return Ok(());
        }
    };

    match dispatch_request(request).await {
        Ok(response) => {
            write_response(writer, &response).await?;
        }
        Err((response, skip)) => {
            if !skip {
                write_response(writer, &response).await?;
            }
        }
    }

    Ok(())
}
