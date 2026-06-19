use std::time::Duration;

use scraper::{Html, Selector};
use tracing::{debug, info, warn};

use super::r#trait::{EngineConfig, SearchEngine, SearchResult};
use crate::error::{Result, SearchError};

/// Brave Search 搜索引擎实现
///
/// 使用 Brave Search 的 HTML 搜索结果页面（非 JavaScript 版本），通过解析
/// HTML 来提取搜索结果。Brave Search 使用自己的搜索索引，不依赖 Google/Bing。
pub struct Brave {
    client: reqwest::Client,
    user_agents: Vec<&'static str>,
}

impl Brave {
    const USER_AGENTS: &'static [&'static str] = &[
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
        "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/119.0.0.0 Safari/537.36",
        "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
    ];

    pub fn new(proxy_url: Option<&str>) -> Result<Self> {
        let client = super::client_builder_with_proxy(proxy_url, 15)
            .user_agent(Self::USER_AGENTS[0])
            .build()
            .map_err(SearchError::Http)?;

        Ok(Self {
            client,
            user_agents: Self::USER_AGENTS.to_vec(),
        })
    }

    fn is_captcha_page(html: &str) -> bool {
        html.contains("verify")
            || html.contains("challenge-platform")
            || html.contains("cf-browser-verification")
    }

    async fn try_request(
        &self,
        url: &str,
        ua: &str,
        config: &EngineConfig,
    ) -> Result<Vec<SearchResult>> {
        let response = self
            .client
            .get(url)
            .header(reqwest::header::USER_AGENT, ua)
            .header("Accept-Language", "en-US,en;q=0.9")
            .timeout(Duration::from_secs(config.timeout_secs))
            .send()
            .await
            .map_err(SearchError::Http)?;

        let status = response.status();
        if !status.is_success() {
            return Err(SearchError::Http(
                response.error_for_status().expect_err("already checked"),
            ));
        }

        let html = response.text().await.map_err(SearchError::Http)?;

        if Self::is_captcha_page(&html) {
            return Err(SearchError::HtmlParse(
                "CAPTCHA challenge detected by Brave Search. Try again later or use a different proxy/IP.".into(),
            ));
        }

        let mut results = self.parse_results(&html)?;

        if results.len() > config.max_results {
            results.truncate(config.max_results);
        }

        info!("Brave returned {} results for '{}'", results.len(), "query");
        Ok(results)
    }

    fn random_user_agent(&self) -> &str {
        use std::time::{SystemTime, UNIX_EPOCH};
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .subsec_nanos() as usize;
        self.user_agents[nanos % self.user_agents.len()]
    }

    fn build_url(&self, query: &str, config: &EngineConfig) -> String {
        let encoded = urlencoding::encode(query);
        // 每页约 10 条结果，offset 参数做偏移
        let offset = if config.page > 1 {
            format!("&offset={}", (config.page - 1) * 10)
        } else {
            String::new()
        };
        let safe = if config.safe_search {
            "&safesearch=strict"
        } else {
            "&safesearch=off"
        };
        format!(
            "https://search.brave.com/search?q={}{}{}",
            encoded, safe, offset
        )
    }

    fn parse_results(&self, html: &str) -> Result<Vec<SearchResult>> {
        let document = Html::parse_document(html);
        let mut results = Vec::new();

        // Brave Search 结果容器
        let result_selectors = [
            "div.snippet",
            "div[class*='snippet']",
            "div[data-type='web']",
        ];

        let mut found = false;
        for sel_str in &result_selectors {
            let Ok(sel) = Selector::parse(sel_str) else {
                continue;
            };
            if document.select(&sel).next().is_none() {
                continue;
            }
            found = true;

            for element in document.select(&sel) {
                let Some(result) = Self::extract_result(&element) else {
                    continue;
                };
                results.push(result);
            }
            break;
        }

        // 回退：搜索 h3 标签（通用方案）
        if !found || results.is_empty() {
            let Ok(h3_sel) = Selector::parse("h3") else {
                return Ok(results);
            };
            for h3 in document.select(&h3_sel) {
                // 查找父级链接
                if let Some(parent) = h3.parent() {
                    if let Some(grandparent) = parent.parent() {
                        if let Some(el) = scraper::ElementRef::wrap(grandparent) {
                            if let Some(link) = el
                                .select(
                                    &Selector::parse("a")
                                        .map_err(|e| SearchError::HtmlParse(e.to_string()))?,
                                )
                                .next()
                            {
                                let title =
                                    h3.text().collect::<Vec<_>>().join("").trim().to_string();
                                let url = link.value().attr("href").unwrap_or("").to_string();
                                if !title.is_empty() && !url.is_empty() && !url.starts_with('#') {
                                    results.push(SearchResult {
                                        title,
                                        url,
                                        snippet: String::new(),
                                        content: None,
                                        score: None,
                                        sources: None,
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(results)
    }

    /// 从 Brave 结果元素中提取 SearchResult
    fn extract_result(element: &scraper::ElementRef) -> Option<SearchResult> {
        // 标题选择器：多种可能性
        let title_selectors = [
            "span.snippet-title",
            "a[href]:not([href^='#']):not([href^='/'])",
            "h3 a",
            "h2 a",
            "a h3",
            "a",
        ];

        let (title, url) = Self::extract_title_url(element, &title_selectors)?;
        if title.is_empty() || url.is_empty() || url.starts_with('#') {
            return None;
        }

        let snippet = Self::extract_snippet(element);

        Some(SearchResult {
            title,
            url,
            snippet,
            content: None,
            score: None,
            sources: None,
        })
    }

    /// 提取标题和 URL
    fn extract_title_url(
        element: &scraper::ElementRef,
        selectors: &[&str],
    ) -> Option<(String, String)> {
        for sel_str in selectors {
            let Ok(sel) = Selector::parse(sel_str) else {
                continue;
            };
            if let Some(link) = element.select(&sel).next() {
                let href = link.value().attr("href").unwrap_or("");
                // 跳过无效链接
                if href.is_empty() || href.starts_with('#') || href.starts_with('/') {
                    continue;
                }
                let title = link.text().collect::<Vec<_>>().join("").trim().to_string();
                let url = if href.starts_with("http") {
                    href.to_string()
                } else {
                    format!("https://search.brave.com{}", href)
                };
                if !title.is_empty() {
                    return Some((title, url));
                }
            }
        }
        None
    }

    /// 提取摘要
    fn extract_snippet(element: &scraper::ElementRef) -> String {
        let snippet_selectors = [
            "p.snippet-description",
            "div.snippet-description",
            "span.snippet-description",
            "div[class*='description']",
            "p[class*='description']",
        ];
        for sel_str in &snippet_selectors {
            if let Ok(sel) = Selector::parse(sel_str) {
                if let Some(s) = element.select(&sel).next() {
                    let text = s.text().collect::<Vec<_>>().join("").trim().to_string();
                    if !text.is_empty() {
                        return text;
                    }
                }
            }
        }
        String::new()
    }
}

#[async_trait::async_trait]
impl SearchEngine for Brave {
    fn name(&self) -> &'static str {
        "brave"
    }

    async fn search(&self, query: &str, config: &EngineConfig) -> Result<Vec<SearchResult>> {
        let url = self.build_url(query, config);
        debug!("Searching Brave: {}", url);

        let mut last_error = None;
        for attempt in 0..3 {
            let ua = self.random_user_agent();
            debug!("Attempt {} with UA: {}", attempt + 1, ua);

            match self.try_request(&url, ua, config).await {
                Ok(results) => return Ok(results),
                Err(e) => {
                    if e.is_http() && attempt < 2 {
                        warn!("Brave HTTP error (attempt {}), retrying", attempt + 1);
                        tokio::time::sleep(Duration::from_secs(2u64.pow(attempt as u32))).await;
                    } else {
                        last_error = Some(e);
                        break;
                    }
                }
            }
        }

        Err(last_error.unwrap_or_else(|| SearchError::NoResults {
            query: query.to_string(),
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_url_default() {
        let b = Brave::new(None).unwrap();
        let config = EngineConfig::default();
        let url = b.build_url("rust", &config);
        assert!(url.contains("q=rust"));
        assert!(url.contains("&safesearch=strict"));
        assert!(!url.contains("&offset="));
    }

    #[test]
    fn test_build_url_with_page() {
        let b = Brave::new(None).unwrap();
        let config = EngineConfig {
            page: 2,
            ..Default::default()
        };
        let url = b.build_url("rust", &config);
        assert!(url.contains("&offset=10"));
    }

    #[test]
    fn test_build_url_no_safe() {
        let b = Brave::new(None).unwrap();
        let config = EngineConfig {
            safe_search: false,
            ..Default::default()
        };
        let url = b.build_url("rust", &config);
        assert!(url.contains("&safesearch=off"));
    }

    #[test]
    fn test_is_captcha_page() {
        assert!(Brave::is_captcha_page("cf-browser-verification"));
        assert!(!Brave::is_captcha_page("<html><body>normal</body></html>"));
    }

    /// 模拟 Brave Search 搜索结果 HTML
    fn sample_html() -> &'static str {
        r#"
<div class="snippet">
  <a href="https://rust-lang.org">
    <span class="snippet-title">Rust Programming Language</span>
  </a>
  <p class="snippet-description">A language empowering everyone to build reliable software.</p>
</div>
<div class="snippet">
  <a href="https://en.wikipedia.org/wiki/Rust_(programming_language)">
    <span class="snippet-title">Rust - Wikipedia</span>
  </a>
  <p class="snippet-description">Rust is a multi-paradigm programming language.</p>
</div>
"#
    }

    #[test]
    fn test_parse_results_normal() {
        let b = Brave::new(None).unwrap();
        let results = b.parse_results(sample_html()).unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].title, "Rust Programming Language");
        assert_eq!(results[0].url, "https://rust-lang.org");
        assert!(results[0].snippet.contains("reliable software"));
        assert_eq!(results[1].title, "Rust - Wikipedia");
        assert_eq!(
            results[1].url,
            "https://en.wikipedia.org/wiki/Rust_(programming_language)"
        );
    }

    #[test]
    fn test_parse_results_empty() {
        let b = Brave::new(None).unwrap();
        let results = b.parse_results("<html><body></body></html>").unwrap();
        assert!(results.is_empty());
    }
}
