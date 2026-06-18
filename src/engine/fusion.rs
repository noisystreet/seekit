use std::sync::Arc;

use tracing::{debug, info, warn};

use super::duckduckgo::DuckDuckGo;
use super::r#trait::{EngineConfig, SearchEngine, SearchResult};
use super::searxng::SearXNG;
use crate::error::{Result, SearchError};

/// 结果融合器 — 合并、去重、评分、排序
pub struct ResultMerger;

impl ResultMerger {
    /// 合并多引擎结果
    ///
    /// `engine_results`: Vec<(engine_name, Vec<SearchResult>)>
    /// 返回评分降序的融合结果，并填充 score/sources 字段
    pub fn merge(
        engine_results: Vec<(&str, Vec<SearchResult>)>,
        max_results: usize,
    ) -> Vec<SearchResult> {
        if engine_results.is_empty() {
            return vec![];
        }

        let num_engines = engine_results.len() as f64;

        // 收集所有结果并标记来源
        let mut combined: Vec<ScoredEntry> = Vec::new();

        for (engine_name, results) in &engine_results {
            for (pos, result) in results.iter().enumerate() {
                let url = normalize_url(&result.url);
                let position = (pos + 1) as f64;

                // 查找是否已有相同 URL 的结果
                if let Some(entry) = combined
                    .iter_mut()
                    .find(|e: &&mut ScoredEntry| e.normalized_url == url)
                {
                    // 已有此结果：增加来源
                    if !entry.sources.contains(&engine_name.to_string()) {
                        entry.sources.push(engine_name.to_string());
                    }
                    // 累加评分
                    let weight = num_engines;
                    entry.score += weight / position;
                } else {
                    // 新结果
                    let weight = num_engines;
                    combined.push(ScoredEntry {
                        result: result.clone(),
                        normalized_url: url,
                        sources: vec![engine_name.to_string()],
                        score: weight / position,
                    });
                }
            }
        }

        // 按评分降序排序
        combined.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // 截断并填充 score/sources
        combined
            .into_iter()
            .take(max_results)
            .map(|entry| {
                let mut result = entry.result;
                result.score = Some(entry.score);
                result.sources = Some(entry.sources);
                result
            })
            .collect()
    }
}

/// 内部带评分和来源的条目
struct ScoredEntry {
    result: SearchResult,
    normalized_url: String,
    sources: Vec<String>,
    score: f64,
}

/// URL 归一化：去除尾部斜杠、www 前缀
fn normalize_url(url: &str) -> String {
    let s = url.trim_end_matches('/');
    if let Some(rest) = s.strip_prefix("https://www.") {
        format!("https://{}", rest)
    } else if let Some(rest) = s.strip_prefix("http://www.") {
        format!("http://{}", rest)
    } else {
        s.to_string()
    }
}

/// Auto 搜索引擎 — 并行查询所有引擎并融合结果
pub struct AutoEngine {
    duckduckgo: Arc<DuckDuckGo>,
    searxng: Arc<SearXNG>,
}

impl AutoEngine {
    /// 创建 Auto 引擎
    pub fn new(searxng_base_url: &str) -> Result<Self> {
        Ok(Self {
            duckduckgo: Arc::new(DuckDuckGo::new()?),
            searxng: Arc::new(SearXNG::new(searxng_base_url)?),
        })
    }
}

#[async_trait::async_trait]
impl SearchEngine for AutoEngine {
    fn name(&self) -> &'static str {
        "auto"
    }

    async fn search(&self, query: &str, config: &EngineConfig) -> Result<Vec<SearchResult>> {
        debug!("Auto engine: searching with all engines");

        let ddg = self.duckduckgo.clone();
        let searxng = self.searxng.clone();
        let query_owned = query.to_string();
        let config_ddg = config.clone();
        let config_searxng = config.clone();
        let query_ddg = query_owned.clone();

        // 并行触发所有引擎
        let ddg_handle = tokio::spawn(async move {
            let result = ddg.search(&query_ddg, &config_ddg).await;
            (result, "duckduckgo")
        });

        let searxng_handle = tokio::spawn(async move {
            let result = searxng.search(&query_owned, &config_searxng).await;
            (result, "searxng")
        });

        // 收集结果，处理失败
        let mut engine_results: Vec<(&str, Vec<SearchResult>)> = Vec::new();

        // DDG
        match ddg_handle.await {
            Ok((Ok(results), name)) => {
                debug!("Auto: {} returned {} results", name, results.len());
                engine_results.push((name, results));
            }
            Ok((Err(e), name)) => {
                warn!("Auto engine: {} failed: {}", name, e);
            }
            Err(e) => {
                warn!("Auto engine: duckduckgo task panicked: {}", e);
            }
        }

        // SearXNG
        match searxng_handle.await {
            Ok((Ok(results), name)) => {
                debug!("Auto: {} returned {} results", name, results.len());
                engine_results.push((name, results));
            }
            Ok((Err(e), name)) => {
                warn!("Auto engine: {} failed: {}", name, e);
            }
            Err(e) => {
                warn!("Auto engine: searxng task panicked: {}", e);
            }
        }

        if engine_results.is_empty() {
            return Err(SearchError::NoResults {
                query: query.to_string(),
            });
        }

        let merged = ResultMerger::merge(engine_results, config.max_results);
        info!(
            "Auto engine: {} merged results for '{}'",
            merged.len(),
            query
        );
        Ok(merged)
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

    // ── normalize_url ─────────────────────────────────────

    #[test]
    fn test_normalize_url_strip_trailing_slash() {
        assert_eq!(normalize_url("https://example.com/"), "https://example.com");
    }

    #[test]
    fn test_normalize_url_strip_www() {
        assert_eq!(
            normalize_url("https://www.example.com/page"),
            "https://example.com/page"
        );
    }

    #[test]
    fn test_normalize_url_strip_http_www() {
        assert_eq!(
            normalize_url("http://www.example.com/"),
            "http://example.com"
        );
    }

    #[test]
    fn test_normalize_url_no_change() {
        assert_eq!(
            normalize_url("https://example.com/path"),
            "https://example.com/path"
        );
    }

    // ── merge ──────────────────────────────────────────────

    #[test]
    fn test_merge_single_engine() {
        let results = vec![(
            "ddg",
            vec![make_result("Title", "https://example.com", "Snippet")],
        )];
        let merged = ResultMerger::merge(results, 10);
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].title, "Title");
        assert!(merged[0].score.is_some());
        assert!(merged[0].sources.is_some());
    }

    #[test]
    fn test_merge_dedup_by_url() {
        let results = vec![
            (
                "ddg",
                vec![make_result("Dup Title", "https://example.com", "DDG")],
            ),
            (
                "searxng",
                vec![make_result(
                    "SearXNG Title",
                    "https://example.com",
                    "SearXNG",
                )],
            ),
        ];
        let merged = ResultMerger::merge(results, 10);
        // 同一 URL 应合并为一条
        assert_eq!(merged.len(), 1);
        // 来源应包含两个引擎
        let sources = merged[0].sources.as_ref().unwrap();
        assert_eq!(sources.len(), 2);
        assert!(sources.contains(&"ddg".to_string()));
        assert!(sources.contains(&"searxng".to_string()));
    }

    #[test]
    fn test_merge_all_unique() {
        let results = vec![
            (
                "ddg",
                vec![
                    make_result("A", "https://example.com/a", ""),
                    make_result("B", "https://example.com/b", ""),
                ],
            ),
            (
                "searxng",
                vec![make_result("C", "https://example.com/c", "")],
            ),
        ];
        let merged = ResultMerger::merge(results, 10);
        assert_eq!(merged.len(), 3);
    }

    #[test]
    fn test_merge_scoring_order() {
        // 两个引擎都返回相同顺序时，第一条应有最高分
        let results = vec![
            (
                "ddg",
                vec![
                    make_result("First", "https://example.com/1", ""),
                    make_result("Second", "https://example.com/2", ""),
                ],
            ),
            (
                "searxng",
                vec![
                    make_result("First", "https://example.com/1", ""),
                    make_result("Second", "https://example.com/2", ""),
                ],
            ),
        ];
        let merged = ResultMerger::merge(results, 10);
        assert_eq!(merged.len(), 2);
        // 第一条在两个引擎都出现，应评分更高
        assert!(merged[0].score.unwrap() > merged[1].score.unwrap());
        // 第一条应有两个来源
        assert_eq!(merged[0].sources.as_ref().unwrap().len(), 2);
    }

    #[test]
    fn test_merge_empty_input() {
        let merged = ResultMerger::merge(vec![], 10);
        assert!(merged.is_empty());
    }

    #[test]
    fn test_merge_truncate() {
        let results = vec![(
            "ddg",
            (0..5)
                .map(|i| {
                    make_result(
                        &format!("Title {}", i),
                        &format!("https://example.com/{}", i),
                        "",
                    )
                })
                .collect(),
        )];
        let merged = ResultMerger::merge(results, 3);
        assert_eq!(merged.len(), 3);
    }

    #[test]
    fn test_normalize_url_dedup() {
        // www.example.com 和 example.com 应归一化为相同 URL
        let results = vec![
            (
                "ddg",
                vec![make_result("A", "https://www.example.com/page", "")],
            ),
            (
                "searxng",
                vec![make_result("A", "https://example.com/page/", "")],
            ),
        ];
        let merged = ResultMerger::merge(results, 10);
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].sources.as_ref().unwrap().len(), 2);
    }
}
