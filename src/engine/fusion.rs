use std::sync::Arc;

use tracing::{debug, info, warn};

use super::bing::Bing;
use super::brave::Brave;
use super::duckduckgo::DuckDuckGo;
use super::google::Google;
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
    google: Arc<Google>,
    bing: Arc<Bing>,
    brave: Arc<Brave>,
    searxng: Arc<SearXNG>,
}

impl AutoEngine {
    /// 创建 Auto 引擎
    pub fn new(searxng_base_url: &str, proxy_url: Option<&str>) -> Result<Self> {
        Ok(Self {
            duckduckgo: Arc::new(DuckDuckGo::new(proxy_url)?),
            google: Arc::new(Google::new(proxy_url)?),
            bing: Arc::new(Bing::new(proxy_url)?),
            brave: Arc::new(Brave::new(proxy_url)?),
            searxng: Arc::new(SearXNG::new(searxng_base_url, proxy_url)?),
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
        let google = self.google.clone();
        let bing = self.bing.clone();
        let brave = self.brave.clone();
        let searxng = self.searxng.clone();
        let query_owned = query.to_string();
        let config_ddg = config.clone();
        let config_google = config.clone();
        let config_bing = config.clone();
        let config_brave = config.clone();
        let config_searxng = config.clone();
        let query_ddg = query_owned.clone();
        let query_google = query_owned.clone();
        let query_bing = query_owned.clone();
        let query_brave = query_owned.clone();

        // 并行触发所有引擎
        let ddg_handle = tokio::spawn(async move {
            let result = ddg.search(&query_ddg, &config_ddg).await;
            (result, "duckduckgo")
        });

        let google_handle = tokio::spawn(async move {
            let result = google.search(&query_google, &config_google).await;
            (result, "google")
        });

        let bing_handle = tokio::spawn(async move {
            let result = bing.search(&query_bing, &config_bing).await;
            (result, "bing")
        });

        let brave_handle = tokio::spawn(async move {
            let result = brave.search(&query_brave, &config_brave).await;
            (result, "brave")
        });

        let searxng_handle = tokio::spawn(async move {
            let result = searxng.search(&query_owned, &config_searxng).await;
            (result, "searxng")
        });

        // 收集结果，处理失败
        let mut engine_results: Vec<(&str, Vec<SearchResult>)> = Vec::new();

        for handle in [
            ddg_handle,
            google_handle,
            bing_handle,
            brave_handle,
            searxng_handle,
        ] {
            match handle.await {
                Ok((Ok(results), name)) => {
                    debug!("Auto: {} returned {} results", name, results.len());
                    engine_results.push((name, results));
                }
                Ok((Err(e), name)) => {
                    warn!("Auto engine: {} failed: {}", name, e);
                }
                Err(e) => {
                    warn!("Auto engine: task panicked: {}", e);
                }
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

    // ── normalize_url 更多边界 ────────────────────────────

    #[test]
    fn test_normalize_url_www_https_bare() {
        assert_eq!(
            normalize_url("https://www.example.com"),
            "https://example.com"
        );
    }

    #[test]
    fn test_normalize_url_www_http_with_path() {
        assert_eq!(
            normalize_url("http://www.example.com/path"),
            "http://example.com/path"
        );
    }

    #[test]
    fn test_normalize_url_trailing_slash_no_www() {
        assert_eq!(normalize_url("http://example.com/"), "http://example.com");
    }

    #[test]
    fn test_normalize_url_multiple_trailing_slashes() {
        assert_eq!(
            normalize_url("https://example.com/path///"),
            "https://example.com/path"
        );
    }

    #[test]
    fn test_normalize_url_empty() {
        assert_eq!(normalize_url(""), "");
    }

    // ── merge 更多场景 ─────────────────────────────────────

    #[test]
    fn test_merge_empty_engine_with_other_results() {
        // 某些引擎返回空，其他引擎有结果
        let results = vec![
            ("ddg", vec![]),
            (
                "searxng",
                vec![make_result("A", "https://example.com/a", "")],
            ),
        ];
        let merged = ResultMerger::merge(results, 10);
        assert_eq!(merged.len(), 1);
    }

    #[test]
    fn test_merge_all_engines_return_empty() {
        let results = vec![("ddg", vec![]), ("searxng", vec![])];
        let merged = ResultMerger::merge(results, 10);
        assert_eq!(merged.len(), 0);
    }

    #[test]
    fn test_merge_http_https_not_deduped() {
        // http 和 https 是不同的协议，normalize_url 不会合并它们
        let results = vec![
            (
                "ddg",
                vec![make_result("HTTP", "http://example.com/page", "")],
            ),
            (
                "searxng",
                vec![make_result("HTTPS", "https://example.com/page", "")],
            ),
        ];
        let merged = ResultMerger::merge(results, 10);
        assert_eq!(merged.len(), 2);
    }

    #[test]
    fn test_merge_three_engines_same_url() {
        // 三个引擎返回同一 URL，应合并为一条，sources 包含三个引擎
        let results = vec![
            ("ddg", vec![make_result("A", "https://example.com", "")]),
            ("google", vec![make_result("A", "https://example.com", "")]),
            ("bing", vec![make_result("A", "https://example.com", "")]),
        ];
        let merged = ResultMerger::merge(results, 10);
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].sources.as_ref().unwrap().len(), 3);
    }

    #[test]
    fn test_merge_max_results_zero() {
        let results = vec![("ddg", vec![make_result("A", "https://example.com/a", "")])];
        let merged = ResultMerger::merge(results, 0);
        assert_eq!(merged.len(), 0);
    }

    #[test]
    fn test_merge_score_exact_calculation() {
        // 2 引擎，同一 URL 都在 pos 1
        // score = 2/1 + 2/1 = 4.0
        let results = vec![
            ("ddg", vec![make_result("A", "https://example.com/a", "")]),
            (
                "searxng",
                vec![make_result("A", "https://example.com/a", "")],
            ),
        ];
        let merged = ResultMerger::merge(results, 10);
        assert_eq!(merged.len(), 1);
        assert!((merged[0].score.unwrap() - 4.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_merge_score_position_matters() {
        // 2 引擎
        // ddg:   pos 1 (score=2/1=2.0) -> other_url
        //        pos 2 (score=2/2=1.0) -> target_url
        // searxng: pos 1 (score=2/1=2.0) -> target_url
        // target_url 总分 = 1.0 + 2.0 = 3.0
        // other_url 总分 = 2.0
        let results = vec![
            (
                "ddg",
                vec![
                    make_result("Other", "https://example.com/other", ""),
                    make_result("Target", "https://example.com/target", ""),
                ],
            ),
            (
                "searxng",
                vec![make_result("Target", "https://example.com/target", "")],
            ),
        ];
        let merged = ResultMerger::merge(results, 10);
        assert_eq!(merged.len(), 2);
        // Target 在 ddg 的 pos 2 和 searxng 的 pos 1 出现
        assert!((merged[0].score.unwrap() - 3.0).abs() < f64::EPSILON);
        assert!((merged[1].score.unwrap() - 2.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_merge_same_engine_duplicate_url_accumulates_score() {
        // 同一引擎返回两次相同 URL
        // num_engines=1, pos 1: 1/1=1.0, pos 2: 1/2=0.5 → total=1.5
        // sources 中不应重复添加同一引擎名
        let results = vec![(
            "ddg",
            vec![
                make_result("A", "https://example.com/a", ""),
                make_result("A", "https://example.com/a", ""),
            ],
        )];
        let merged = ResultMerger::merge(results, 10);
        assert_eq!(merged.len(), 1);
        assert!((merged[0].score.unwrap() - 1.5).abs() < f64::EPSILON);
        assert_eq!(merged[0].sources.as_ref().unwrap().len(), 1);
    }
}
