use std::time::Duration;

pub mod bing;
pub mod duckduckgo;
pub mod fusion;
pub mod google;
pub mod searxng;
pub mod r#trait;

pub use r#trait::{EngineConfig, SearchEngine, SearchResult};

/// 构建带可选代理的 reqwest ClientBuilder
///
/// 1. `proxy_url` 显式指定时优先使用
/// 2. 否则读取环境变量：`HTTPS_PROXY` > `https_proxy` > `HTTP_PROXY` > `http_proxy` > `ALL_PROXY` > `all_proxy`
///
/// 返回 ClientBuilder，调用者可继续链式配置（如 .user_agent()）后调用 .build()
pub fn client_builder_with_proxy(
    proxy_url: Option<&str>,
    timeout_secs: u64,
) -> reqwest::ClientBuilder {
    let mut builder = reqwest::Client::builder().timeout(Duration::from_secs(timeout_secs));

    // 优先使用显式指定的 proxy，否则从环境变量读取
    let proxy = if let Some(url) = proxy_url.filter(|u| !u.is_empty()) {
        Some(url.to_string())
    } else {
        std::env::var("HTTPS_PROXY")
            .or_else(|_| std::env::var("https_proxy"))
            .or_else(|_| std::env::var("HTTP_PROXY"))
            .or_else(|_| std::env::var("http_proxy"))
            .or_else(|_| std::env::var("ALL_PROXY"))
            .or_else(|_| std::env::var("all_proxy"))
            .ok()
            .filter(|u| !u.is_empty())
    };

    if let Some(ref url) = proxy {
        match reqwest::Proxy::all(url) {
            Ok(proxy) => {
                tracing::debug!("Using proxy: {}", url);
                builder = builder.proxy(proxy);
            }
            Err(e) => {
                tracing::warn!("Invalid proxy URL '{}': {}", url, e);
            }
        }
    }

    builder
}

/// 支持的搜索引擎类型
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EngineType {
    DuckDuckGo,
    Google,
    Bing,
    SearXNG,
    /// 自动使用所有可用引擎并行搜索并融合结果
    Auto,
}

impl std::str::FromStr for EngineType {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "duckduckgo" | "ddg" => Ok(Self::DuckDuckGo),
            "google" | "g" => Ok(Self::Google),
            "bing" | "b" => Ok(Self::Bing),
            "searxng" | "searx" => Ok(Self::SearXNG),
            "auto" | "all" | "multi" => Ok(Self::Auto),
            _ => Err(format!(
                "Unknown engine: {}. Use: duckduckgo, google, bing, searxng, auto",
                s
            )),
        }
    }
}
