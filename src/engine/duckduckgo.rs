use std::time::Duration;

use scraper::{Html, Selector};
use tracing::{debug, info, warn};

use super::r#trait::{EngineConfig, SearchEngine, SearchResult};
use crate::error::{Result, SearchError};

/// DuckDuckGo 搜索引擎实现
///
/// 使用 DuckDuckGo 的 HTML 端点（非 JavaScript 版本）来获取搜索结果。
/// 这是一个免 API Key 的方案，通过解析 HTML 页面提取搜索结果。
pub struct DuckDuckGo {
    client: reqwest::Client,
    user_agents: Vec<&'static str>,
}

impl DuckDuckGo {
    /// 轮换的 User-Agent 列表，降低被识别为爬虫的风险
    const USER_AGENTS: &'static [&'static str] = &[
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
        "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/119.0.0.0 Safari/537.36",
        "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:121.0) Gecko/20100101 Firefox/121.0",
        "Mozilla/5.0 (Macintosh; Intel Mac OS X 10.15; rv:120.0) Gecko/20100101 Firefox/120.0",
    ];

    pub fn new() -> Result<Self> {
        let client = reqwest::Client::builder()
            .user_agent(Self::USER_AGENTS[0])
            .timeout(Duration::from_secs(15))
            .build()
            .map_err(SearchError::Http)?;

        Ok(Self {
            client,
            user_agents: Self::USER_AGENTS.to_vec(),
        })
    }

    /// 检查错误是否为 CAPTCHA
    fn is_captcha(err: &SearchError) -> bool {
        matches!(err, SearchError::HtmlParse(msg) if msg.contains("CAPTCHA"))
    }

    /// 检查页面是否包含反爬检测标记
    fn is_captcha_page(html: &str) -> bool {
        html.contains("captcha")
            || html.contains("verify")
            || html.contains("challenge")
            || html.contains("anomaly-modal")
    }

    /// 处理单个 HTTP 搜索请求
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
        let mut results = self.parse_results(&html)?;

        if results.is_empty() {
            if Self::is_captcha_page(&html) {
                return Err(SearchError::HtmlParse(
                    "CAPTCHA challenge detected by DuckDuckGo. Try again later or reduce request frequency.".into(),
                ));
            }
            return Err(SearchError::NoResults {
                query: "".to_string(),
            });
        }

        if results.len() > config.max_results {
            results.truncate(config.max_results);
        }

        info!(
            "DuckDuckGo returned {} results for '{}'",
            results.len(),
            "query"
        );
        Ok(results)
    }

    /// 随机选择一个 User-Agent
    fn random_user_agent(&self) -> &str {
        use std::time::{SystemTime, UNIX_EPOCH};
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .subsec_nanos() as usize;
        self.user_agents[nanos % self.user_agents.len()]
    }

    /// 构建 DuckDuckGo HTML 搜索 URL
    fn build_url(&self, query: &str, config: &EngineConfig) -> String {
        let encoded = urlencoding::encode(query);
        let safe = if config.safe_search { "1" } else { "-1" };
        format!(
            "https://html.duckduckgo.com/html/?q={}&kp={}",
            encoded, safe
        )
    }

    /// 从 HTML 页面中解析搜索结果
    fn parse_results(&self, html: &str) -> Result<Vec<SearchResult>> {
        let document = Html::parse_document(html);
        let mut results = Vec::new();

        // DuckDuckGo HTML 页面的结果容器（跳过广告 .result--ad）
        let result_selector = Selector::parse(".result:not(.result--ad)")
            .map_err(|e| SearchError::HtmlParse(e.to_string()))?;

        let title_selector = Selector::parse(".result__title a")
            .map_err(|e| SearchError::HtmlParse(e.to_string()))?;

        let snippet_selector = Selector::parse(".result__snippet")
            .map_err(|e| SearchError::HtmlParse(e.to_string()))?;

        for element in document.select(&result_selector) {
            // 提取标题和 URL
            let title_link = element.select(&title_selector).next();
            let (title, url) = match title_link {
                Some(link) => {
                    let t = link.text().collect::<Vec<_>>().join("").trim().to_string();
                    let u = link.value().attr("href").unwrap_or("").to_string();
                    (t, u)
                }
                None => continue,
            };

            // 提取摘要
            let snippet = element
                .select(&snippet_selector)
                .next()
                .map(|s| s.text().collect::<Vec<_>>().join("").trim().to_string())
                .unwrap_or_default();

            if title.is_empty() || url.is_empty() {
                continue;
            }

            // DuckDuckGo 的链接是重定向链接，需提取真实 URL
            let real_url = extract_real_url(&url);

            // 通过 URL 解析跳过广告结果
            if is_ad_url(&real_url) {
                continue;
            }

            results.push(SearchResult {
                title,
                url: real_url,
                snippet,
                content: None,
                score: None,
                sources: None,
            });
        }

        Ok(results)
    }
}

/// 判断是否为广告 URL
fn is_ad_url(raw_url: &str) -> bool {
    // 简单字符串解析代替 url::Url::parse，减少编译依赖
    // 提取 host 部分：从 "://" 之后到下一个 "/" 之前
    if let Some(rest) = raw_url.split_once("://") {
        let after_protocol = rest.1;
        let host_end = after_protocol.find('/').unwrap_or(after_protocol.len());
        let host = &after_protocol[..host_end];
        let path = &after_protocol[host_end..];

        // DuckDuckGo 广告点击追踪
        if host.contains("duckduckgo.com") && path.contains("y.js") {
            return true;
        }
        // Bing 广告
        if host.contains("bing.com") && path.contains("aclick") {
            return true;
        }
    }
    false
}

/// 从 DuckDuckGo 的重定向链接中提取真实 URL
///
/// DuckDuckGo HTML 页面的链接格式:
/// - 普通结果: /l/?uddg=https%3A%2F%2Fexample.com&rut=xxx
/// - 广告结果: 长 URL（is_ad_url 会过滤掉）
fn extract_real_url(uddg_url: &str) -> String {
    // 优先提取 uddg= 参数中的 URL
    if let Some(query_start) = uddg_url.find("uddg=") {
        let encoded = &uddg_url[query_start + 5..];
        let end = encoded.find('&').unwrap_or(encoded.len());
        let url_encoded = &encoded[..end];
        if let Ok(decoded) = urlencoding::decode(url_encoded) {
            return decoded.into_owned();
        }
    }

    // 如果是完整 HTTP URL，直接返回
    if uddg_url.starts_with("http://") || uddg_url.starts_with("https://") {
        return uddg_url.to_string();
    }

    // 相对路径补全
    format!("https://duckduckgo.com{}", uddg_url)
}

#[async_trait::async_trait]
impl SearchEngine for DuckDuckGo {
    fn name(&self) -> &'static str {
        "duckduckgo"
    }

    async fn search(&self, query: &str, config: &EngineConfig) -> Result<Vec<SearchResult>> {
        let url = self.build_url(query, config);
        debug!("Searching DuckDuckGo: {}", url);

        let mut last_error = None;
        for attempt in 0..3 {
            let ua = self.random_user_agent();
            debug!("Attempt {} with UA: {}", attempt + 1, ua);

            match self.try_request(&url, ua, config).await {
                Ok(results) => return Ok(results),
                Err(e) => {
                    if Self::is_captcha(&e) && attempt < 2 {
                        warn!(
                            "DDG CAPTCHA (attempt {}), waiting and retrying",
                            attempt + 1
                        );
                        tokio::time::sleep(Duration::from_secs(2u64.pow(attempt as u32))).await;
                    } else if e.is_http() && attempt < 2 {
                        warn!("DDG HTTP error (attempt {}), retrying", attempt + 1);
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

    // ── extract_real_url ──────────────────────────────────

    #[test]
    fn test_extract_real_url_with_uddg() {
        let url = "/l/?uddg=https%3A%2F%2Frust-lang.org&rut=abc123";
        assert_eq!(extract_real_url(url), "https://rust-lang.org");
    }

    #[test]
    fn test_extract_real_url_without_uddg() {
        let url = "https://example.com/page";
        assert_eq!(extract_real_url(url), "https://example.com/page");
    }

    #[test]
    fn test_extract_real_url_relative_path() {
        let url = "/some/path";
        assert_eq!(extract_real_url(url), "https://duckduckgo.com/some/path");
    }

    #[test]
    fn test_extract_real_url_uddg_no_trailing_params() {
        let url = "/l/?uddg=https%3A%2F%2Fexample.com";
        assert_eq!(extract_real_url(url), "https://example.com");
    }

    // ── is_ad_url ──────────────────────────────────────────

    #[test]
    fn test_is_ad_url_duckduckgo_yjs() {
        let url = "https://duckduckgo.com/y.js?ad_domain=example.com";
        assert!(is_ad_url(url));
    }

    #[test]
    fn test_is_ad_url_bing_aclick() {
        let url = "https://www.bing.com/aclick?ld=abc123";
        assert!(is_ad_url(url));
    }

    #[test]
    fn test_is_ad_url_normal_url() {
        let url = "https://rust-lang.org/";
        assert!(!is_ad_url(url));
    }

    #[test]
    fn test_is_ad_url_duckduckgo_not_ad() {
        let url = "https://duckduckgo.com/search?q=rust";
        assert!(!is_ad_url(url));
    }

    #[test]
    fn test_is_ad_url_invalid_url() {
        assert!(!is_ad_url("not a url"));
    }

    // ── build_url ──────────────────────────────────────────

    #[test]
    fn test_build_url_safe_search() {
        let ddg = DuckDuckGo::new().unwrap();
        let config = EngineConfig {
            safe_search: true,
            ..Default::default()
        };
        let url = ddg.build_url("rust", &config);
        assert!(url.contains("kp=1"));
        assert!(url.contains("q=rust"));
    }

    #[test]
    fn test_build_url_no_safe_search() {
        let ddg = DuckDuckGo::new().unwrap();
        let config = EngineConfig {
            safe_search: false,
            ..Default::default()
        };
        let url = ddg.build_url("rust", &config);
        assert!(url.contains("kp=-1"));
    }

    #[test]
    fn test_build_url_query_encoding() {
        let ddg = DuckDuckGo::new().unwrap();
        let config = EngineConfig::default();
        let url = ddg.build_url("rust web framework", &config);
        assert!(url.contains("rust+web+framework") || url.contains("rust%20web%20framework"));
    }

    // ── parse_results ──────────────────────────────────────

    /// 模拟 DuckDuckGo HTML 搜索结果的正常片段
    fn sample_html() -> &'static str {
        r#"
<div class="result">
  <div class="result__title">
    <a href="/l/?uddg=https%3A%2F%2Frust-lang.org&rut=abc">Rust Programming Language</a>
  </div>
  <div class="result__snippet">A language empowering everyone.</div>
</div>
<div class="result">
  <div class="result__title">
    <a href="/l/?uddg=https%3A%2F%2Fen.wikipedia.org%2Fwiki%2FRust&rut=def">Rust - Wikipedia</a>
  </div>
  <div class="result__snippet">Rust is a multi-paradigm language.</div>
</div>
"#
    }

    #[test]
    fn test_parse_results_normal() {
        let ddg = DuckDuckGo::new().unwrap();
        let results = ddg.parse_results(sample_html()).unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].title, "Rust Programming Language");
        assert_eq!(results[0].url, "https://rust-lang.org");
        assert_eq!(results[0].snippet, "A language empowering everyone.");
        assert_eq!(results[1].title, "Rust - Wikipedia");
        assert_eq!(results[1].url, "https://en.wikipedia.org/wiki/Rust");
    }

    #[test]
    fn test_parse_results_empty_html() {
        let ddg = DuckDuckGo::new().unwrap();
        let results = ddg.parse_results("<html><body></body></html>").unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_parse_results_with_ad_skipped() {
        let html = r#"
<div class="result result--ad">
  <div class="result__title">
    <a href="/l/?uddg=https%3A%2F%2Fad.example.com&rut=ad">Ad</a>
  </div>
  <div class="result__snippet">This is an ad</div>
</div>
<div class="result">
  <div class="result__title">
    <a href="/l/?uddg=https%3A%2F%2Freal.example.com&rut=real">Real Result</a>
  </div>
  <div class="result__snippet">Real content</div>
</div>
"#;
        let ddg = DuckDuckGo::new().unwrap();
        let results = ddg.parse_results(html).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].url, "https://real.example.com");
    }

    #[test]
    fn test_parse_results_skip_ad_url() {
        let html = r#"
<div class="result">
  <div class="result__title">
    <a href="https://duckduckgo.com/y.js?ad_domain=spam">Ad Link</a>
  </div>
  <div class="result__snippet">spam</div>
</div>
<div class="result">
  <div class="result__title">
    <a href="/l/?uddg=https%3A%2F%2Freal.example.com&rut=real">Real</a>
  </div>
</div>
"#;
        let ddg = DuckDuckGo::new().unwrap();
        let results = ddg.parse_results(html).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].url, "https://real.example.com");
    }

    #[test]
    fn test_parse_results_no_title_skipped() {
        let html = r#"
<div class="result">
  <div class="result__title">
    <a href="/l/?uddg=https%3A%2F%2Fexample.com"></a>
  </div>
</div>
"#;
        let ddg = DuckDuckGo::new().unwrap();
        let results = ddg.parse_results(html).unwrap();
        assert!(results.is_empty());
    }

    // ── random_user_agent ──────────────────────────────────

    #[test]
    fn test_random_user_agent_returns_valid() {
        let ddg = DuckDuckGo::new().unwrap();
        let ua = ddg.random_user_agent();
        assert!(!ua.is_empty());
        assert!(ua.contains("Mozilla") || ua.contains("Chrome") || ua.contains("Firefox"));
    }
}
