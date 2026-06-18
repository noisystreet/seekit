use serde::{Deserialize, Serialize};

/// 搜索工具配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchConfig {
    pub general: GeneralConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneralConfig {
    /// 最大结果数
    pub max_results: usize,
    /// 请求超时（秒）
    pub timeout: u64,
    /// 启用安全搜索
    pub safe_search: bool,
    /// 启用缓存
    pub enable_cache: bool,
    /// 缓存 TTL（秒），默认 300
    pub cache_ttl_secs: u64,
    /// SearXNG 实例地址（可选）
    pub searxng_url: Option<String>,
}

impl Default for SearchConfig {
    fn default() -> Self {
        Self {
            general: GeneralConfig {
                max_results: 10,
                timeout: 10,
                safe_search: true,
                enable_cache: true,
                cache_ttl_secs: 300,
                searxng_url: None,
            },
        }
    }
}

impl SearchConfig {
    /// 默认配置文件路径
    pub fn default_path() -> std::path::PathBuf {
        let config_dir = dirs::config_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join("seekit");
        config_dir.join("config.toml")
    }

    /// 加载配置文件，如果不存在则返回默认配置
    pub fn load() -> Self {
        let path = Self::default_path();
        if path.exists() {
            let content = std::fs::read_to_string(&path).unwrap_or_default();
            toml::from_str(&content).unwrap_or_default()
        } else {
            Self::default()
        }
    }

    /// 保存配置文件
    pub fn save(&self) -> anyhow::Result<()> {
        let path = Self::default_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = toml::to_string_pretty(self)?;
        std::fs::write(&path, content)?;
        Ok(())
    }
}
