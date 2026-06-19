use std::time::Duration;

use scraper::{ElementRef, Html, Selector};
use tracing::{debug, info, warn};

use super::r#trait::{EngineConfig, SearchEngine, SearchResult};
use crate::error::{Result, SearchError};

/// Google 搜索引擎实现
///
/// 使用 Google 的 HTML 搜索结果页面（非 JavaScript 版本），通过解析
/// HTML 来提取搜索结果。Google 对爬虫限制较严，可能会触发 CAPTCHA。
pub struct Google {
    client: reqwest::Client,
    user_agents: Vec<&'static str>,
}

impl Google {
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
        html.contains("https://www.google.com/sorry/index")
            || html.contains("recaptcha")
            || html.contains("g-recaptcha")
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
                "CAPTCHA challenge detected by Google. Try again later or use a different proxy/IP.".into(),
            ));
        }

        let mut results = self.parse_results(&html)?;

        if results.is_empty() && html.contains("did not match any") {
            return Err(SearchError::NoResults {
                query: "".to_string(),
            });
        }

        if results.len() > config.max_results {
            results.truncate(config.max_results);
        }

        info!(
            "Google returned {} results for '{}'",
            results.len(),
            "query"
        );
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
        let safe = if config.safe_search {
            "&safe=active"
        } else {
            ""
        };
        // 每页约 10 条结果，start 参数做偏移
        let start = if config.page > 1 {
            format!("&start={}", (config.page - 1) * 10)
        } else {
            String::new()
        };
        format!(
            "https://www.google.com/search?q={}{}{}&hl=en",
            encoded, safe, start
        )
    }

    fn parse_results(&self, html: &str) -> Result<Vec<SearchResult>> {
        let document = Html::parse_document(html);
        let mut results = Vec::new();

        // 先用标准选择器解析
        let found = Self::parse_standard_results(&document, &mut results);

        // 标准选择器未找到时尝试 h3 回退
        if !found || results.is_empty() {
            Self::parse_h3_fallback(&document, &mut results);
        }

        Ok(results)
    }

    /// 使用标准选择器解析搜索结果
    fn parse_standard_results(document: &Html, results: &mut Vec<SearchResult>) -> bool {
        let selectors = [
            "div.g",
            "div[data-hveid]:not([data-hveid=''])",
            "div[jscontroller][data-hveid]",
        ];

        for sel_str in &selectors {
            let Ok(sel) = Selector::parse(sel_str) else {
                continue;
            };
            if document.select(&sel).next().is_none() {
                continue;
            }
            for element in document.select(&sel) {
                if is_ad_result(&element) {
                    continue;
                }
                let Some((title, url)) = Self::extract_title_url(&element) else {
                    continue;
                };
                if title.is_empty() || url.is_empty() {
                    continue;
                }
                let snippet = Self::extract_snippet(&element);
                results.push(SearchResult {
                    title,
                    url: extract_google_url(&url),
                    snippet,
                    content: None,
                    score: None,
                    sources: None,
                });
            }
            return true;
        }
        false
    }

    /// 提取标题和 URL
    fn extract_title_url(element: &scraper::ElementRef) -> Option<(String, String)> {
        let link_selectors = ["a[href^='/url']", "a[href^='http']", "a"];
        for sel_str in &link_selectors {
            if let Ok(sel) = Selector::parse(sel_str) {
                if let Some(link) = element.select(&sel).next() {
                    let href = link.value().attr("href").unwrap_or("");
                    if href.starts_with("http") || href.starts_with("/url") {
                        let title = link.text().collect::<Vec<_>>().join("").trim().to_string();
                        let url = href.to_string();
                        if !title.is_empty() && !url.is_empty() {
                            return Some((title, url));
                        }
                    }
                }
            }
        }
        None
    }

    /// 提取摘要
    fn extract_snippet(element: &scraper::ElementRef) -> String {
        let snippet_selectors = ["div.VwiC3b", "span.st", "div[data-sncf]"];
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

    /// h3 回退解析（当标准选择器失效时）
    fn parse_h3_fallback(document: &Html, results: &mut Vec<SearchResult>) {
        let Ok(h3_sel) = Selector::parse("h3") else {
            return;
        };
        for h3 in document.select(&h3_sel) {
            if let Some(parent_link) = h3.parent().and_then(|n| {
                n.parent().and_then(|p| {
                    ElementRef::wrap(p)
                        .and_then(|el| el.value().attr("href").map(|h| h.to_string()))
                })
            }) {
                let title = h3.text().collect::<Vec<_>>().join("").trim().to_string();
                let url = extract_google_url(&parent_link);
                if !title.is_empty() && !url.is_empty() {
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

/// 判断是否为广告结果
fn is_ad_result(element: &scraper::ElementRef) -> bool {
    element
        .value()
        .attr("data-text-ad")
        .or(element.value().attr("id"))
        .map(|a| a.contains("ad") || a.contains("ads"))
        .unwrap_or(false)
}

/// 从 Google 的 /url?q= 链接中提取真实 URL
fn extract_google_url(href: &str) -> String {
    // 仅处理 /url?q= 格式的重定向链接
    if href.starts_with("/url") && href.contains("?q=") {
        if let Some(q_start) = href.find("?q=") {
            let encoded = &href[q_start + 3..];
            let end = encoded.find('&').unwrap_or(encoded.len());
            let url_encoded = &encoded[..end];
            if let Ok(decoded) = urlencoding::decode(url_encoded) {
                return decoded.into_owned();
            }
        }
    }
    // 已经是真实 URL
    if href.starts_with("http://") || href.starts_with("https://") {
        return href.to_string();
    }
    href.to_string()
}

#[async_trait::async_trait]
impl SearchEngine for Google {
    fn name(&self) -> &'static str {
        "google"
    }

    async fn search(&self, query: &str, config: &EngineConfig) -> Result<Vec<SearchResult>> {
        let url = self.build_url(query, config);
        debug!("Searching Google: {}", url);

        let mut last_error = None;
        for attempt in 0..3 {
            let ua = self.random_user_agent();
            debug!("Attempt {} with UA: {}", attempt + 1, ua);

            match self.try_request(&url, ua, config).await {
                Ok(results) => return Ok(results),
                Err(e) => {
                    if e.is_http() && attempt < 2 {
                        warn!("Google HTTP error (attempt {}), retrying", attempt + 1);
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
    fn test_extract_google_url_via_q() {
        let url = "/url?q=https://rust-lang.org&sa=U&ved=0";
        assert_eq!(extract_google_url(url), "https://rust-lang.org");
    }

    #[test]
    fn test_extract_google_url_direct() {
        let url = "https://example.com";
        assert_eq!(extract_google_url(url), "https://example.com");
    }

    #[test]
    fn test_extract_google_url_no_q() {
        let url = "/search?q=test&hl=en";
        assert_eq!(extract_google_url(url), "/search?q=test&hl=en");
    }

    #[test]
    fn test_build_url_default() {
        let g = Google::new(None).unwrap();
        let config = EngineConfig::default();
        let url = g.build_url("rust", &config);
        assert!(url.contains("q=rust"));
        assert!(url.contains("&safe=active"));
        assert!(!url.contains("&start="));
    }

    #[test]
    fn test_build_url_with_page() {
        let g = Google::new(None).unwrap();
        let config = EngineConfig {
            page: 2,
            ..Default::default()
        };
        let url = g.build_url("rust", &config);
        assert!(url.contains("&start=10"));
    }

    #[test]
    fn test_build_url_no_safe() {
        let g = Google::new(None).unwrap();
        let config = EngineConfig {
            safe_search: false,
            ..Default::default()
        };
        let url = g.build_url("rust", &config);
        assert!(!url.contains("&safe="));
    }

    #[test]
    fn test_is_captcha_page_sorry() {
        assert!(Google::is_captcha_page(
            "https://www.google.com/sorry/index?continue=..."
        ));
    }

    #[test]
    fn test_is_captcha_page_recaptcha() {
        assert!(Google::is_captcha_page("g-recaptcha data-sitekey="));
    }

    #[test]
    fn test_is_captcha_page_normal() {
        assert!(!Google::is_captcha_page(
            "<html><body>normal page</body></html>"
        ));
    }

    /// 模拟 Google 搜索结果 HTML
    fn sample_html() -> &'static str {
        r#"
<div class="g">
  <div>
    <a href="/url?q=https://rust-lang.org&sa=U&ved=0ahUKE">
      <h3>Rust Programming Language</h3>
    </a>
    <div class="VwiC3b">A language empowering everyone to build reliable and efficient software.</div>
  </div>
</div>
<div class="g">
  <div>
    <a href="/url?q=https://en.wikipedia.org/wiki/Rust&sa=U&ved=1">
      <h3>Rust - Wikipedia</h3>
    </a>
    <div class="VwiC3b">Rust is a multi-paradigm, general-purpose programming language.</div>
  </div>
</div>
"#
    }

    #[test]
    fn test_parse_results_normal() {
        let g = Google::new(None).unwrap();
        let results = g.parse_results(sample_html()).unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].title, "Rust Programming Language");
        assert_eq!(results[0].url, "https://rust-lang.org");
        assert_eq!(
            results[0].snippet,
            "A language empowering everyone to build reliable and efficient software."
        );
        assert_eq!(results[1].title, "Rust - Wikipedia");
        assert_eq!(results[1].url, "https://en.wikipedia.org/wiki/Rust");
    }

    #[test]
    fn test_parse_results_empty() {
        let g = Google::new(None).unwrap();
        let results = g.parse_results("<html><body></body></html>").unwrap();
        assert!(results.is_empty());
    }
}
