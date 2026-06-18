use std::time::Duration;

use scraper::Html;
use tracing::{debug, warn};

use crate::engine::SearchResult;
use crate::error::{Result, SearchError};

/// Fetcher 配置
#[derive(Debug, Clone)]
pub struct FetcherConfig {
    /// 单个页面最大字符数
    pub max_content_length: usize,
    /// 并发请求数
    pub concurrency: usize,
}

impl Default for FetcherConfig {
    fn default() -> Self {
        Self {
            max_content_length: 5000,
            concurrency: 5,
        }
    }
}

/// 页面内容提取器
///
/// 对搜索结果中的每个 URL 发起 HTTP 请求，将 HTML 转换为纯文本，
/// 填充到 `SearchResult.content` 字段。
pub struct Fetcher {
    client: reqwest::Client,
    config: FetcherConfig,
}

impl Fetcher {
    /// 创建新的 Fetcher 实例
    pub fn new(config: FetcherConfig) -> Result<Self> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(15))
            .build()
            .map_err(SearchError::Http)?;

        Ok(Self { client, config })
    }

    /// 并行获取多个 URL 的页面内容
    ///
    /// 遍历 results，对每个有 URL 的结果发起 HTTP 请求，
    /// 将 HTML 转换为纯文本并填充到 content 字段。
    /// 单个 URL 失败不影响其他结果。
    pub async fn fetch(&self, results: &mut [SearchResult]) {
        let max_content_length = self.config.max_content_length;

        // 对每个结果发起请求（并发执行）
        let mut handles = Vec::new();
        for result in results.iter() {
            let client = self.client.clone();
            let url = result.url.clone();
            handles.push(tokio::spawn(async move {
                fetch_single_page(&client, &url, max_content_length).await
            }));
        }

        let mut contents: Vec<Option<String>> = Vec::new();
        for handle in handles {
            match handle.await {
                Ok(content) => contents.push(content),
                Err(e) => {
                    warn!("Fetcher task failed: {}", e);
                    contents.push(None);
                }
            }
        }

        // 填充结果
        for (result, content) in results.iter_mut().zip(contents) {
            if let Some(text) = content {
                result.content = Some(text);
            }
        }
    }
}

/// 获取单个 URL 的页面内容并转换为纯文本
async fn fetch_single_page(
    client: &reqwest::Client,
    url: &str,
    max_length: usize,
) -> Option<String> {
    debug!("Fetching: {}", url);

    let response = match client.get(url).send().await {
        Ok(resp) => resp,
        Err(e) => {
            warn!("Fetch failed for {}: {}", url, e);
            return None;
        }
    };

    let status = response.status();
    if !status.is_success() {
        warn!("Fetch returned {} for {}", status, url);
        return None;
    }

    let html = match response.text().await {
        Ok(text) => text,
        Err(e) => {
            warn!("Failed to read body for {}: {}", url, e);
            return None;
        }
    };

    // 使用 scraper 解析 HTML 并提取纯文本（代替 html2md）
    let document = Html::parse_document(&html);
    let text: String = document
        .root_element()
        .text()
        .collect::<Vec<&str>>()
        .join(" ")
        .split_whitespace()
        .collect::<Vec<&str>>()
        .join(" ");

    if text.is_empty() {
        return None;
    }

    // 截断至 max_length
    let truncated = if text.len() > max_length {
        format!("{}...", &text[..max_length])
    } else {
        text
    };

    Some(truncated)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fetcher_config_default() {
        let config = FetcherConfig::default();
        assert_eq!(config.max_content_length, 5000);
        assert_eq!(config.concurrency, 5);
    }

    #[test]
    fn test_fetcher_config_custom() {
        let config = FetcherConfig {
            max_content_length: 1000,
            concurrency: 3,
        };
        assert_eq!(config.max_content_length, 1000);
        assert_eq!(config.concurrency, 3);
    }
}
