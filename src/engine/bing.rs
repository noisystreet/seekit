use std::time::Duration;

use scraper::{ElementRef, Html, Selector};
use tracing::{debug, info, warn};

use super::r#trait::{EngineConfig, SearchEngine, SearchResult};
use crate::error::{Result, SearchError};

/// Bing 搜索引擎实现
///
/// 使用 Bing 的 HTML 搜索结果页面，通过解析 HTML 来提取搜索结果。
/// Bing 对爬虫限制中等，有地域重定向和 CAPTCHA 检测。
pub struct Bing {
    client: reqwest::Client,
    user_agents: Vec<&'static str>,
}

/// 判断 Bing 结果元素是否为广告
fn is_bing_ad(element: &scraper::ElementRef) -> bool {
    element
        .value()
        .attr("data-bm")
        .or_else(|| {
            element
                .parent()
                .and_then(ElementRef::wrap)
                .and_then(|p| p.value().attr("data-bm"))
        })
        .is_some()
}

impl Bing {
    const USER_AGENTS: &'static [&'static str] = &[
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
        "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/119.0.0.0 Safari/537.36",
        "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:121.0) Gecko/20100101 Firefox/121.0",
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
        html.contains("bing.com/captcha")
            || html.contains("g-recaptcha")
            || html.contains("captcha")
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
                "CAPTCHA challenge detected by Bing. Try again later or use a different proxy/IP."
                    .into(),
            ));
        }

        let mut results = self.parse_results(&html)?;

        if results.len() > config.max_results {
            results.truncate(config.max_results);
        }

        info!("Bing returned {} results for '{}'", results.len(), "query");
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
        // 每页约 10 条结果，first 参数做偏移
        let first = if config.page > 1 {
            format!("&first={}", (config.page - 1) * 10 + 1)
        } else {
            String::new()
        };
        format!(
            "https://www.bing.com/search?q={}&setlang=en{}",
            encoded, first
        )
    }

    fn parse_results(&self, html: &str) -> Result<Vec<SearchResult>> {
        let document = Html::parse_document(html);
        let mut results = Vec::new();

        let result_selector = Selector::parse("li.b_algo, div.b_algo")
            .map_err(|e| SearchError::HtmlParse(e.to_string()))?;

        for element in document.select(&result_selector) {
            if is_bing_ad(&element) {
                continue;
            }

            let Some(result) = Self::extract_result(&element) else {
                continue;
            };
            // 过滤掉 bing.com 域内的链接（如视频、图片标签等）
            if result.url.contains("www.bing.com") && !result.url.contains("bing.com/search") {
                continue;
            }
            results.push(result);
        }

        Ok(results)
    }

    /// 从 Bing 结果元素中提取 SearchResult
    fn extract_result(element: &scraper::ElementRef) -> Option<SearchResult> {
        let title_selector = Selector::parse("h2 a, .b_algo h2 a").ok()?;
        let snippet_selector = Selector::parse(".b_caption p, .b_lineclamp2").ok()?;

        let title_link = element.select(&title_selector).next()?;
        let title = title_link
            .text()
            .collect::<Vec<_>>()
            .join("")
            .trim()
            .to_string();
        let url = title_link.value().attr("href")?.to_string();

        if title.is_empty() || url.is_empty() {
            return None;
        }

        let snippet = element
            .select(&snippet_selector)
            .next()
            .map(|s| s.text().collect::<Vec<_>>().join("").trim().to_string())
            .unwrap_or_default();

        Some(SearchResult {
            title,
            url,
            snippet,
            content: None,
            score: None,
            sources: None,
        })
    }
}

#[async_trait::async_trait]
impl SearchEngine for Bing {
    fn name(&self) -> &'static str {
        "bing"
    }

    async fn search(&self, query: &str, config: &EngineConfig) -> Result<Vec<SearchResult>> {
        let url = self.build_url(query, config);
        debug!("Searching Bing: {}", url);

        let mut last_error = None;
        for attempt in 0..3 {
            let ua = self.random_user_agent();
            debug!("Attempt {} with UA: {}", attempt + 1, ua);

            match self.try_request(&url, ua, config).await {
                Ok(results) => return Ok(results),
                Err(e) => {
                    if e.is_http() && attempt < 2 {
                        warn!("Bing HTTP error (attempt {}), retrying", attempt + 1);
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
        let b = Bing::new(None).unwrap();
        let config = EngineConfig::default();
        let url = b.build_url("rust", &config);
        assert!(url.contains("q=rust"));
        assert!(!url.contains("&first="));
    }

    #[test]
    fn test_build_url_with_page() {
        let b = Bing::new(None).unwrap();
        let config = EngineConfig {
            page: 2,
            ..Default::default()
        };
        let url = b.build_url("rust", &config);
        assert!(url.contains("&first=11"));
    }

    #[test]
    fn test_is_captcha_page() {
        assert!(Bing::is_captcha_page("bing.com/captcha?q=verify"));
        assert!(!Bing::is_captcha_page("<html><body>normal</body></html>"));
    }

    /// 模拟 Bing 搜索结果 HTML
    fn sample_html() -> &'static str {
        r#"
<li class="b_algo">
  <h2><a href="https://rust-lang.org">Rust Programming Language</a></h2>
  <div class="b_caption"><p>A language empowering everyone to build reliable software.</p></div>
</li>
<li class="b_algo">
  <h2><a href="https://en.wikipedia.org/wiki/Rust_(programming_language)">Rust - Wikipedia</a></h2>
  <div class="b_caption"><p>Rust is a multi-paradigm programming language.</p></div>
</li>
"#
    }

    #[test]
    fn test_parse_results_normal() {
        let b = Bing::new(None).unwrap();
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
        let b = Bing::new(None).unwrap();
        let results = b.parse_results("<html><body></body></html>").unwrap();
        assert!(results.is_empty());
    }
}
