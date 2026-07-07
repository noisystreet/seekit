use std::time::Duration;

use serde::Deserialize;
use tracing::{debug, info, warn};

use super::r#trait::{EngineConfig, SearchEngine, SearchResult};
use crate::error::{Result, SearchError};

/// SearXNG 搜索引擎实现
///
/// 调用自部署 SearXNG 实例的 JSON API。
/// SearXNG 是一个元搜索引擎，聚合 Google、Bing、DuckDuckGo 等 70+ 引擎的结果。
/// 需要用户自部署实例（推荐 Docker 部署）。
pub struct SearXNG {
    client: reqwest::Client,
    base_url: String,
}

/// SearXNG JSON API 响应结构
#[derive(Debug, Deserialize)]
struct SearXNGResponse {
    results: Vec<SearXNGResult>,
    #[serde(rename = "query")]
    _query: String,
}

/// SearXNG 单个结果
#[derive(Debug, Deserialize)]
struct SearXNGResult {
    title: Option<String>,
    url: Option<String>,
    content: Option<String>,
}

impl SearXNG {
    /// 创建 SearXNG 引擎
    ///
    /// `base_url`: SearXNG 实例地址，如 `http://localhost:8080`
    /// `proxy_url`: HTTP 代理地址（可选）
    pub fn new(base_url: &str, proxy_url: Option<&str>) -> Result<Self> {
        let client = super::client_builder_with_proxy(proxy_url, 30)
            .build()
            .map_err(SearchError::Http)?;

        // 去掉尾部斜杠
        let base_url = base_url.trim_end_matches('/').to_string();

        Ok(Self { client, base_url })
    }

    /// 构建 SearXNG JSON API 搜索 URL
    fn build_url(&self, query: &str, config: &EngineConfig) -> String {
        let encoded = urlencoding::encode(query);
        let categories = if config.safe_search {
            "general"
        } else {
            "general,images,news,videos"
        };

        // 语言参数（如 &language=en）
        let lang_param = config
            .lang
            .as_ref()
            .map(|l| format!("&language={}", l))
            .unwrap_or_default();

        // 分页参数（SearXNG 使用 pageno，从 1 开始）
        let page_param = format!("&pageno={}", config.page);

        format!(
            "{}/search?q={}&format=json&categories={}{}{}",
            self.base_url, encoded, categories, lang_param, page_param
        )
    }

    /// 发送 HTTP 请求
    async fn send_request(&self, url: &str, timeout_secs: u64) -> Result<reqwest::Response> {
        let response = self
            .client
            .get(url)
            .timeout(Duration::from_secs(timeout_secs))
            .send()
            .await
            .map_err(|e| {
                warn!("SearXNG request failed: {}", e);
                SearchError::Http(e)
            })?;

        let status = response.status();
        if !status.is_success() {
            warn!("SearXNG returned status: {}", status);
            return Err(SearchError::Http(
                response.error_for_status().expect_err("already checked"),
            ));
        }
        Ok(response)
    }

    /// 解析 JSON 响应并提取结果
    fn parse_response(text: &str, query: &str, max_results: usize) -> Result<Vec<SearchResult>> {
        if text.trim_start().starts_with('<') {
            warn!("SearXNG returned HTML instead of JSON — check base_url or instance status");
            return Err(SearchError::HtmlParse(
                "SearXNG returned HTML page. Ensure the instance is running and format=json is supported.".into(),
            ));
        }

        let searxng_response: SearXNGResponse = serde_json::from_str(text).map_err(|e| {
            warn!("Failed to parse SearXNG JSON response: {}", e);
            SearchError::HtmlParse(format!("Failed to parse SearXNG response: {}", e))
        })?;

        let mut results: Vec<SearchResult> = searxng_response
            .results
            .into_iter()
            .filter_map(|r| {
                let title = r.title?;
                let url = r.url?;
                let snippet = r.content.unwrap_or_default();
                if title.is_empty() || url.is_empty() {
                    return None;
                }
                Some(SearchResult {
                    title,
                    url,
                    snippet,
                    content: None,
                    score: None,
                    sources: None,
                })
            })
            .collect();

        results.dedup_by(|a, b| a.url == b.url);

        if results.is_empty() {
            return Err(SearchError::NoResults {
                query: query.to_string(),
            });
        }

        if results.len() > max_results {
            results.truncate(max_results);
        }

        info!("SearXNG returned {} results for '{}'", results.len(), query);
        Ok(results)
    }
}

#[async_trait::async_trait]
impl SearchEngine for SearXNG {
    fn name(&self) -> &'static str {
        "searxng"
    }

    async fn search(&self, query: &str, config: &EngineConfig) -> Result<Vec<SearchResult>> {
        let url = self.build_url(query, config);
        debug!("Searching SearXNG: {}", url);

        let response = self.send_request(&url, config.timeout_secs).await?;
        let text = response.text().await.map_err(SearchError::Http)?;

        SearXNG::parse_response(&text, query, config.max_results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── build_url ──────────────────────────────────────────

    #[test]
    fn test_build_url_default() {
        let engine = SearXNG::new("http://localhost:8080", None).unwrap();
        let config = EngineConfig {
            lang: Some("en".to_string()),
            ..Default::default()
        };
        let url = engine.build_url("rust", &config);
        assert_eq!(
            url,
            "http://localhost:8080/search?q=rust&format=json&categories=general&language=en&pageno=1"
        );
    }

    #[test]
    fn test_build_url_no_safe_search() {
        let engine = SearXNG::new("http://localhost:8080", None).unwrap();
        let config = EngineConfig {
            safe_search: false,
            ..Default::default()
        };
        let url = engine.build_url("rust", &config);
        assert!(url.contains("categories=general,images,news,videos"));
    }

    #[test]
    fn test_build_url_with_lang() {
        let engine = SearXNG::new("http://localhost:8080", None).unwrap();
        let config = EngineConfig {
            lang: Some("zh".to_string()),
            ..Default::default()
        };
        let url = engine.build_url("rust", &config);
        assert!(url.contains("language=zh"));
    }

    #[test]
    fn test_build_url_without_lang() {
        let engine = SearXNG::new("http://localhost:8080", None).unwrap();
        let config = EngineConfig {
            lang: None,
            ..Default::default()
        };
        let url = engine.build_url("rust", &config);
        assert!(!url.contains("language"));
    }

    #[test]
    fn test_build_url_base_url_slash_stripped() {
        let engine = SearXNG::new("http://localhost:8080/", None).unwrap();
        let config = EngineConfig {
            lang: Some("en".to_string()),
            ..Default::default()
        };
        let url = engine.build_url("test", &config);
        assert_eq!(
            url,
            "http://localhost:8080/search?q=test&format=json&categories=general&language=en&pageno=1"
        );
    }

    #[test]
    fn test_build_url_query_encoding() {
        let engine = SearXNG::new("http://localhost:8080", None).unwrap();
        let config = EngineConfig::default();
        let url = engine.build_url("hello world", &config);
        assert!(url.contains("q=hello+world") || url.contains("q=hello%20world"));
    }

    // ── JSON 反序列化 ─────────────────────────────────────

    #[test]
    fn test_searxng_response_deserialize() {
        let json = r#"{
            "query": "rust",
            "results": [
                {
                    "title": "Rust Lang",
                    "url": "https://rust-lang.org",
                    "content": "A language"
                }
            ]
        }"#;
        let resp: SearXNGResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp._query, "rust");
        assert_eq!(resp.results.len(), 1);
        assert_eq!(resp.results[0].title.as_deref(), Some("Rust Lang"));
        assert_eq!(
            resp.results[0].url.as_deref(),
            Some("https://rust-lang.org")
        );
        assert_eq!(resp.results[0].content.as_deref(), Some("A language"));
    }

    #[test]
    fn test_searxng_response_empty_results() {
        let json = r#"{"query": "test", "results": []}"#;
        let resp: SearXNGResponse = serde_json::from_str(json).unwrap();
        assert!(resp.results.is_empty());
    }

    #[test]
    fn test_searxng_result_with_missing_fields() {
        let json = r#"{
            "query": "rust",
            "results": [
                {"title": "Only Title", "url": null, "content": null}
            ]
        }"#;
        let resp: SearXNGResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.results.len(), 1);
        assert!(resp.results[0].url.is_none());
        assert!(resp.results[0].content.is_none());
    }

    // ── filter_map 逻辑测试 ───────────────────────────────

    #[test]
    fn test_filter_map_normal_result() {
        let result = SearXNGResult {
            title: Some("Title".to_string()),
            url: Some("https://example.com".to_string()),
            content: Some("Snippet.".to_string()),
        };
        let sr = SearchResult {
            title: result.title.unwrap(),
            url: result.url.unwrap(),
            snippet: result.content.unwrap_or_default(),
            content: None,
            score: None,
            sources: None,
        };
        assert_eq!(sr.title, "Title");
        assert_eq!(sr.url, "https://example.com");
        assert_eq!(sr.snippet, "Snippet.");
    }

    #[test]
    fn test_filter_map_empty_title_skipped() {
        let results = vec![
            SearXNGResult {
                title: Some("".to_string()),
                url: Some("https://example.com".to_string()),
                content: None,
            },
            SearXNGResult {
                title: Some("Valid".to_string()),
                url: Some("https://valid.com".to_string()),
                content: None,
            },
        ];
        let converted: Vec<SearchResult> = results
            .into_iter()
            .filter_map(|r| {
                let title = r.title?;
                let url = r.url?;
                if title.is_empty() || url.is_empty() {
                    return None;
                }
                Some(SearchResult {
                    title,
                    url,
                    snippet: r.content.unwrap_or_default(),
                    content: None,
                    score: None,
                    sources: None,
                })
            })
            .collect();
        assert_eq!(converted.len(), 1);
        assert_eq!(converted[0].title, "Valid");
    }

    #[test]
    fn test_dedup_by_url() {
        let mut results = vec![
            SearchResult {
                title: "A".to_string(),
                url: "https://example.com/a".to_string(),
                snippet: String::new(),
                content: None,
                score: None,
                sources: None,
            },
            SearchResult {
                title: "A (dup)".to_string(),
                url: "https://example.com/a".to_string(),
                snippet: String::new(),
                content: None,
                score: None,
                sources: None,
            },
            SearchResult {
                title: "B".to_string(),
                url: "https://example.com/b".to_string(),
                snippet: String::new(),
                content: None,
                score: None,
                sources: None,
            },
        ];
        results.dedup_by(|a, b| a.url == b.url);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].title, "A");
        assert_eq!(results[1].title, "B");
    }

    #[test]
    fn test_dedup_all_unique() {
        let mut results = vec![
            SearchResult {
                title: "A".to_string(),
                url: "https://example.com/a".to_string(),
                snippet: String::new(),
                content: None,
                score: None,
                sources: None,
            },
            SearchResult {
                title: "B".to_string(),
                url: "https://example.com/b".to_string(),
                snippet: String::new(),
                content: None,
                score: None,
                sources: None,
            },
        ];
        let len_before = results.len();
        results.dedup_by(|a, b| a.url == b.url);
        assert_eq!(results.len(), len_before);
    }

    // ── new() ─────────────────────────────────────────────

    #[test]
    fn test_new_without_proxy() {
        let engine = SearXNG::new("http://localhost:8080", None).unwrap();
        assert_eq!(engine.base_url, "http://localhost:8080");
    }

    #[test]
    fn test_new_with_proxy() {
        let engine = SearXNG::new("http://localhost:8080", Some("http://proxy:3128")).unwrap();
        assert_eq!(engine.base_url, "http://localhost:8080");
    }

    // ── build_url 扩展测试 ─────────────────────────────────

    #[test]
    fn test_build_url_page_2() {
        let engine = SearXNG::new("http://localhost:8080", None).unwrap();
        let config = EngineConfig {
            page: 2,
            ..Default::default()
        };
        let url = engine.build_url("rust", &config);
        assert!(url.contains("pageno=2"));
    }

    #[test]
    fn test_build_url_page_3() {
        let engine = SearXNG::new("http://localhost:8080", None).unwrap();
        let config = EngineConfig {
            page: 3,
            ..Default::default()
        };
        let url = engine.build_url("test", &config);
        assert!(url.contains("pageno=3"));
    }

    #[test]
    fn test_build_url_special_chars() {
        let engine = SearXNG::new("http://localhost:8080", None).unwrap();
        let config = EngineConfig::default();
        let url = engine.build_url("c++ & go", &config);
        // 特殊字符应被编码
        assert!(!url.contains("c++"));
        assert!(url.contains('q'));
        assert!(url.contains("format=json"));
    }

    // ── parse_response ───────────────────────────────────

    #[test]
    fn test_parse_response_html_error() {
        let err = SearXNG::parse_response("<html>404 Not Found</html>", "test", 10);
        assert!(err.is_err());
        match err {
            Err(SearchError::HtmlParse(msg)) => {
                assert!(msg.contains("HTML"));
            }
            _ => panic!("Expected HtmlParse error"),
        }
    }

    #[test]
    fn test_parse_response_invalid_json() {
        let err = SearXNG::parse_response("{broken json}", "test", 10);
        assert!(err.is_err());
        match err {
            Err(SearchError::HtmlParse(_)) => {} // OK
            _ => panic!("Expected HtmlParse error"),
        }
    }

    #[test]
    fn test_parse_response_empty_results() {
        let json = r#"{"query": "test", "results": []}"#;
        let err = SearXNG::parse_response(json, "test", 10);
        assert!(err.is_err());
        match err {
            Err(SearchError::NoResults { query }) => {
                assert_eq!(query, "test");
            }
            _ => panic!("Expected NoResults error"),
        }
    }

    #[test]
    fn test_parse_response_normal_results() {
        let json = r#"{
            "query": "rust",
            "results": [
                {
                    "title": "Rust Lang",
                    "url": "https://rust-lang.org",
                    "content": "A systems language"
                }
            ]
        }"#;
        let results = SearXNG::parse_response(json, "rust", 10).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "Rust Lang");
        assert_eq!(results[0].url, "https://rust-lang.org");
        assert_eq!(results[0].snippet, "A systems language");
    }

    #[test]
    fn test_parse_response_missing_title_none() {
        let json = r#"{
            "query": "test",
            "results": [
                {"title": null, "url": "https://example.com", "content": "no title"}
            ]
        }"#;
        let err = SearXNG::parse_response(json, "test", 10);
        assert!(err.is_err());
        assert!(matches!(err, Err(SearchError::NoResults { .. })));
    }

    #[test]
    fn test_parse_response_missing_url_none() {
        let json = r#"{
            "query": "test",
            "results": [
                {"title": "No URL", "url": null, "content": "missing url"}
            ]
        }"#;
        let err = SearXNG::parse_response(json, "test", 10);
        assert!(err.is_err());
        assert!(matches!(err, Err(SearchError::NoResults { .. })));
    }

    #[test]
    fn test_parse_response_empty_title_skipped() {
        let json = r#"{
            "query": "test",
            "results": [
                {"title": "", "url": "https://example.com/bad", "content": "empty title"},
                {"title": "Valid", "url": "https://example.com/ok", "content": "ok"}
            ]
        }"#;
        let results = SearXNG::parse_response(json, "test", 10).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "Valid");
    }

    #[test]
    fn test_parse_response_dedup() {
        let json = r#"{
            "query": "test",
            "results": [
                {"title": "First",  "url": "https://example.com/a", "content": "first"},
                {"title": "Second", "url": "https://example.com/a", "content": "second"},
                {"title": "Third",  "url": "https://example.com/b", "content": "third"}
            ]
        }"#;
        let results = SearXNG::parse_response(json, "test", 10).unwrap();
        assert_eq!(results.len(), 2, "duplicate URL should be deduped");
    }

    #[test]
    fn test_parse_response_max_results_truncation() {
        let json = r#"{
            "query": "test",
            "results": [
                {"title": "A", "url": "https://example.com/a", "content": "a"},
                {"title": "B", "url": "https://example.com/b", "content": "b"},
                {"title": "C", "url": "https://example.com/c", "content": "c"}
            ]
        }"#;
        let results = SearXNG::parse_response(json, "test", 2).unwrap();
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_parse_response_content_null_snippet_empty() {
        let json = r#"{
            "query": "test",
            "results": [
                {"title": "No Content", "url": "https://example.com", "content": null}
            ]
        }"#;
        let results = SearXNG::parse_response(json, "test", 10).unwrap();
        assert_eq!(results[0].snippet, "");
    }

    #[test]
    fn test_parse_response_content_missing_field_snippet_empty() {
        let json = r#"{
            "query": "test",
            "results": [
                {"title": "No Content", "url": "https://example.com"}
            ]
        }"#;
        let results = SearXNG::parse_response(json, "test", 10).unwrap();
        assert_eq!(results[0].snippet, "");
    }
}
