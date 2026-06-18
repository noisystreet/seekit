use std::path::PathBuf;
use std::time::{Duration, SystemTime};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::engine::SearchResult;
use crate::error::{Result, SearchError};

/// 缓存条目
#[derive(Debug, Serialize, Deserialize)]
struct CacheEntry {
    results: Vec<SearchResult>,
    created_at: u64, // Unix timestamp (秒)
}

/// 基于磁盘的搜索缓存
pub struct SearchCache {
    cache_dir: PathBuf,
    ttl: Duration,
}

impl SearchCache {
    /// 创建新的缓存实例
    pub fn new() -> Self {
        let cache_dir = dirs::cache_dir()
            .unwrap_or_else(|| PathBuf::from(".cache"))
            .join("seekit");

        Self {
            cache_dir,
            ttl: Duration::from_secs(300), // 默认 5 分钟
        }
    }

    /// 设置缓存 TTL
    pub fn with_ttl(mut self, ttl: Duration) -> Self {
        self.ttl = ttl;
        self
    }

    /// 设置缓存目录（主要用于测试）
    #[cfg(test)]
    pub fn with_cache_dir(mut self, path: PathBuf) -> Self {
        self.cache_dir = path;
        self
    }

    /// 生成缓存键：(engine, query, max_results) 的 SHA256 hash
    fn cache_key(engine: &str, query: &str, max_results: usize) -> String {
        let input = format!("{}:{}:{}", engine, query, max_results);
        let mut hasher = Sha256::new();
        hasher.update(input.as_bytes());
        let result = hasher.finalize();
        hex::encode(result)
    }

    /// 从缓存获取结果
    pub fn get(&self, engine: &str, query: &str, max_results: usize) -> Option<Vec<SearchResult>> {
        let key = Self::cache_key(engine, query, max_results);
        let path = self.cache_dir.join(&key);

        let content = std::fs::read_to_string(&path).ok()?;
        let entry: CacheEntry = serde_json::from_str(&content).ok()?;

        // 检查 TTL
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        if now - entry.created_at >= self.ttl.as_secs() {
            // 过期了，删除缓存
            let _ = std::fs::remove_file(&path);
            return None;
        }

        tracing::debug!("Cache hit for '{}' (engine: {})", query, engine);
        Some(entry.results)
    }

    /// 写入缓存
    pub fn set(
        &self,
        engine: &str,
        query: &str,
        max_results: usize,
        results: &[SearchResult],
    ) -> Result<()> {
        let key = Self::cache_key(engine, query, max_results);
        let path = self.cache_dir.join(&key);

        // 确保缓存目录存在
        std::fs::create_dir_all(&self.cache_dir)
            .map_err(|e| SearchError::Cache(format!("Failed to create cache dir: {}", e)))?;

        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let entry = CacheEntry {
            results: results.to_vec(),
            created_at: now,
        };

        let content = serde_json::to_string(&entry)
            .map_err(|e| SearchError::Cache(format!("Failed to serialize cache: {}", e)))?;

        std::fs::write(&path, content)
            .map_err(|e| SearchError::Cache(format!("Failed to write cache: {}", e)))?;

        tracing::debug!("Cache set for '{}' (engine: {})", query, engine);
        Ok(())
    }

    /// 清空所有缓存
    pub fn clear(&self) -> Result<()> {
        if self.cache_dir.exists() {
            std::fs::remove_dir_all(&self.cache_dir)
                .map_err(|e| SearchError::Cache(format!("Failed to clear cache: {}", e)))?;
        }
        Ok(())
    }
}

impl Default for SearchCache {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// 创建一个测试用的缓存实例（使用临时目录）
    fn test_cache() -> (SearchCache, PathBuf) {
        let dir = std::env::temp_dir().join(format!(
            "seekit_test_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let cache = SearchCache::new()
            .with_cache_dir(dir.clone())
            .with_ttl(Duration::from_secs(60));
        (cache, dir)
    }

    fn make_result(title: &str, url: &str) -> SearchResult {
        SearchResult {
            title: title.to_string(),
            url: url.to_string(),
            snippet: String::new(),
            content: None,
            score: None,
            sources: None,
        }
    }

    // ── cache_key ──────────────────────────────────────────

    #[test]
    fn test_cache_key_deterministic() {
        let k1 = SearchCache::cache_key("ddg", "rust", 10);
        let k2 = SearchCache::cache_key("ddg", "rust", 10);
        assert_eq!(k1, k2);
    }

    #[test]
    fn test_cache_key_differs_by_query() {
        let k1 = SearchCache::cache_key("ddg", "rust", 10);
        let k2 = SearchCache::cache_key("ddg", "go", 10);
        assert_ne!(k1, k2);
    }

    #[test]
    fn test_cache_key_differs_by_engine() {
        let k1 = SearchCache::cache_key("ddg", "rust", 10);
        let k2 = SearchCache::cache_key("searxng", "rust", 10);
        assert_ne!(k1, k2);
    }

    #[test]
    fn test_cache_key_differs_by_max_results() {
        let k1 = SearchCache::cache_key("ddg", "rust", 5);
        let k2 = SearchCache::cache_key("ddg", "rust", 10);
        assert_ne!(k1, k2);
    }

    #[test]
    fn test_cache_key_is_sha256_hex() {
        let key = SearchCache::cache_key("e", "q", 1);
        assert_eq!(key.len(), 64); // SHA256 hex = 64 chars
        assert!(key.chars().all(|c| c.is_ascii_hexdigit()));
    }

    // ── set / get ──────────────────────────────────────────

    #[test]
    fn test_cache_set_and_get() {
        let (cache, dir) = test_cache();
        let results = vec![make_result("Rust", "https://rust-lang.org")];
        cache.set("test_engine", "test_query", 5, &results).unwrap();

        let cached = cache.get("test_engine", "test_query", 5);
        assert!(cached.is_some());
        assert_eq!(cached.unwrap().len(), 1);

        // 清理
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_cache_get_miss() {
        let (cache, dir) = test_cache();
        assert!(cache.get("unknown", "query", 10).is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_cache_get_different_params() {
        let (cache, dir) = test_cache();
        let results = vec![make_result("Rust", "https://rust-lang.org")];
        cache.set("e", "q", 10, &results).unwrap();
        // 不同的 max_results 应该 miss
        assert!(cache.get("e", "q", 5).is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    // ── TTL ───────────────────────────────────────────────

    #[test]
    fn test_cache_ttl_expiry() {
        let dir = std::env::temp_dir().join(format!(
            "seekit_test_ttl_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));

        // 写入时用正常 TTL
        let cache = SearchCache::new()
            .with_cache_dir(dir.clone())
            .with_ttl(Duration::from_secs(60));
        let results = vec![make_result("Rust", "https://rust-lang.org")];
        cache.set("e", "q", 10, &results).unwrap();

        // 用零 TTL 读取，应过期
        let cache_strict = SearchCache::new()
            .with_cache_dir(dir.clone())
            .with_ttl(Duration::from_secs(0));
        assert!(cache_strict.get("e", "q", 10).is_none());

        let _ = std::fs::remove_dir_all(&dir);
    }

    // ── clear ─────────────────────────────────────────────

    #[test]
    fn test_cache_clear() {
        let (cache, dir) = test_cache();
        let results = vec![make_result("Rust", "https://rust-lang.org")];
        cache.set("e", "q", 10, &results).unwrap();
        assert!(cache.get("e", "q", 10).is_some());

        cache.clear().unwrap();
        assert!(cache.get("e", "q", 10).is_none());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_cache_clear_empty_dir() {
        let dir = std::env::temp_dir().join(format!(
            "seekit_test_clear_empty_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let cache = SearchCache::new()
            .with_cache_dir(dir.clone())
            .with_ttl(Duration::from_secs(60));
        // 目录不存在时 clear 不应报错
        assert!(cache.clear().is_ok());
        let _ = std::fs::remove_dir_all(&dir);
    }

    // ── 多结果缓存 ─────────────────────────────────────────

    #[test]
    fn test_cache_multiple_results() {
        let (cache, dir) = test_cache();
        let results = vec![
            make_result("A", "https://a.com"),
            make_result("B", "https://b.com"),
            make_result("C", "https://c.com"),
        ];
        cache.set("e", "q", 10, &results).unwrap();
        let cached = cache.get("e", "q", 10).unwrap();
        assert_eq!(cached.len(), 3);
        assert_eq!(cached[0].title, "A");
        assert_eq!(cached[2].url, "https://c.com");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
