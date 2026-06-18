pub mod duckduckgo;
pub mod fusion;
pub mod searxng;
pub mod r#trait;

pub use r#trait::{EngineConfig, SearchEngine, SearchResult};

/// 支持的搜索引擎类型
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EngineType {
    DuckDuckGo,
    SearXNG,
    /// 自动使用所有可用引擎并行搜索并融合结果
    Auto,
}

impl std::str::FromStr for EngineType {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "duckduckgo" | "ddg" => Ok(Self::DuckDuckGo),
            "searxng" | "searx" => Ok(Self::SearXNG),
            "auto" | "all" | "multi" => Ok(Self::Auto),
            _ => Err(format!(
                "Unknown engine: {}. Use: duckduckgo, searxng, auto",
                s
            )),
        }
    }
}
