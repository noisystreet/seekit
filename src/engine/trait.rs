use crate::error::Result;
use serde::{Deserialize, Serialize};

/// 统一的搜索结果条目
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub title: String,
    pub url: String,
    pub snippet: String,
    /// 页面内容（Markdown），仅 --fetch 时填充
    pub content: Option<String>,
    /// 共识评分（仅 auto 模式）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub score: Option<f64>,
    /// 来源引擎列表（仅 auto 模式）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sources: Option<Vec<String>>,
}

/// 引擎配置
#[derive(Debug, Clone)]
pub struct EngineConfig {
    pub max_results: usize,
    pub timeout_secs: u64,
    pub safe_search: bool,
    /// 搜索语言（如 "en", "zh", "ja"），None 表示不限制
    pub lang: Option<String>,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            max_results: 10,
            timeout_secs: 10,
            safe_search: true,
            lang: None,
        }
    }
}

/// 搜索引擎统一接口
#[async_trait::async_trait]
pub trait SearchEngine: Send + Sync {
    fn name(&self) -> &'static str;
    async fn search(&self, query: &str, config: &EngineConfig) -> Result<Vec<SearchResult>>;
}
