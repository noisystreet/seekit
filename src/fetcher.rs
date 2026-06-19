use scraper::node::Node;
use scraper::{ElementRef, Html, Selector};
use tracing::{debug, warn};

use crate::engine::SearchResult;
use crate::error::{Result, SearchError};

/// Fetcher 配置
#[derive(Debug, Clone)]
pub struct FetcherConfig {
    /// 单个页面最大字符数
    pub max_content_length: usize,
    /// 并发请求数
    pub concurrency: usize,
    /// HTTP 代理 URL（可选）
    pub proxy_url: Option<String>,
}

impl Default for FetcherConfig {
    fn default() -> Self {
        Self {
            max_content_length: 5000,
            concurrency: 5,
            proxy_url: None,
        }
    }
}

/// 页面内容提取器
///
/// 对搜索结果中的每个 URL 发起 HTTP 请求，将 HTML 转换为纯文本，
/// 填充到 `SearchResult.content` 字段。
pub struct Fetcher {
    client: reqwest::Client,
    config: FetcherConfig,
}

impl Fetcher {
    /// 创建新的 Fetcher 实例
    pub fn new(config: FetcherConfig) -> Result<Self> {
        let client = crate::engine::client_builder_with_proxy(config.proxy_url.as_deref(), 15)
            .build()
            .map_err(SearchError::Http)?;

        Ok(Self { client, config })
    }

    /// 并行获取多个 URL 的页面内容
    ///
    /// 遍历 results，对每个有 URL 的结果发起 HTTP 请求，
    /// 将 HTML 转换为纯文本并填充到 content 字段。
    /// 单个 URL 失败不影响其他结果。
    pub async fn fetch(&self, results: &mut [SearchResult]) {
        let max_content_length = self.config.max_content_length;

        // 对每个结果发起请求（并发执行）
        let mut handles = Vec::new();
        for result in results.iter() {
            let client = self.client.clone();
            let url = result.url.clone();
            handles.push(tokio::spawn(async move {
                fetch_single_page(&client, &url, max_content_length).await
            }));
        }

        let mut contents: Vec<Option<String>> = Vec::new();
        for handle in handles {
            match handle.await {
                Ok(content) => contents.push(content),
                Err(e) => {
                    warn!("Fetcher task failed: {}", e);
                    contents.push(None);
                }
            }
        }

        // 填充结果
        for (result, content) in results.iter_mut().zip(contents) {
            if let Some(text) = content {
                result.content = Some(text);
            }
        }
    }
}

/// 获取单个 URL 的页面内容并转换为纯文本
async fn fetch_single_page(
    client: &reqwest::Client,
    url: &str,
    max_length: usize,
) -> Option<String> {
    debug!("Fetching: {}", url);

    let response = match client.get(url).send().await {
        Ok(resp) => resp,
        Err(e) => {
            warn!("Fetch failed for {}: {}", url, e);
            return None;
        }
    };

    let status = response.status();
    if !status.is_success() {
        warn!("Fetch returned {} for {}", status, url);
        return None;
    }

    let html = match response.text().await {
        Ok(text) => text,
        Err(e) => {
            warn!("Failed to read body for {}: {}", url, e);
            return None;
        }
    };

    let text = extract_clean_text(&html);

    if text.is_empty() {
        return None;
    }

    // 截断至 max_length（按字符截断，避免 UTF-8 边界问题）
    let original_len = text.chars().count();
    let truncated: String = text.chars().take(max_length).collect();
    if original_len > max_length {
        return Some(format!("{}...", truncated));
    }
    Some(truncated)
}

/// 从 HTML 中提取清洗后的纯文本
///
/// 策略：
/// 1. 优先定位文章主内容区（article, main, .content 等）
/// 2. 遍历 DOM 时跳过 script/style/nav/footer 等无用元素
/// 3. 只提取 Text 节点的内容
/// 4. 后处理清理噪声行和多余空白
fn extract_clean_text(html: &str) -> String {
    let document = Html::parse_document(html);
    let mut text = String::new();

    // 尝试定位主要内容区域（优先级从高到低）
    let content_selectors = [
        "article",
        "[role=main]",
        "main",
        ".post-content",
        ".article-content",
        ".entry-content",
        ".content",
        "#content",
        ".post",
        ".article",
    ];

    let mut extracted = false;
    for selector_str in &content_selectors {
        if let Ok(sel) = Selector::parse(selector_str) {
            if let Some(element) = document.select(&sel).next() {
                collect_text_safe(&element, &mut text);
                extracted = true;
                break;
            }
        }
    }

    // 没有找到内容容器，从 body 提取
    if !extracted {
        if let Ok(body_sel) = Selector::parse("body") {
            if let Some(body) = document.select(&body_sel).next() {
                collect_text_safe(&body, &mut text);
            } else {
                // 连 body 都没有，从根元素提取
                collect_text_safe(&document.root_element(), &mut text);
            }
        }
    }

    // 后处理：清理噪声行和多余空白
    clean_text_lines(&text)
}

/// 需要跳过的不需要的标签
const SKIP_TAGS: &[&str] = &[
    "script", "style", "noscript", "iframe", "svg", "nav", "footer", "header", "aside", "form",
    "input", "button", "select", "textarea",
];

/// 安全地收集文本，跳过无用元素
fn collect_text_safe(element: &ElementRef, output: &mut String) {
    // 跳过不需要的元素
    let tag_name = element.value().name.local.as_ref();
    if SKIP_TAGS.contains(&tag_name) {
        return;
    }

    for child in element.children() {
        match child.value() {
            Node::Element(_) => {
                if let Some(child_elem) = ElementRef::wrap(child) {
                    collect_text_safe(&child_elem, output);
                }
            }
            Node::Text(text) => {
                let trimmed = text.trim();
                if !trimmed.is_empty() {
                    output.push_str(trimmed);
                    output.push(' ');
                }
            }
            _ => {}
        }
    }
}

/// 清理文本：去除噪声行、合并多余空白
fn clean_text_lines(text: &str) -> String {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .filter(|line| line.chars().count() >= 12)
        .filter(|line| is_meaningful_text(line))
        .map(|s| s.to_string())
        .collect::<Vec<_>>()
        .join("\n")
}

/// 判断一行文本是否为有意义的正文内容（而非 JS/URL/噪声）
fn is_meaningful_text(line: &str) -> bool {
    // 跳过纯符号/数字行
    if line
        .chars()
        .all(|c| c.is_ascii_punctuation() || c.is_ascii_digit() || c.is_whitespace())
    {
        return false;
    }

    // 跳过 JSON/JS 特征行
    if has_js_prefix(line) {
        return false;
    }

    // 跳过 minified JS 行
    if is_minified_js(line) {
        return false;
    }

    // 跳过 URL 和路径行
    if line.starts_with("http") || line.starts_with("//") || line.starts_with("/") {
        return false;
    }

    true
}

/// 检查是否以 JS 代码特征开头
fn has_js_prefix(line: &str) -> bool {
    line.starts_with("var ")
        || line.starts_with("function ")
        || line.starts_with("if (")
        || line.starts_with("try {")
        || line.starts_with("} catch")
        || line.starts_with("window.")
        || line.starts_with("document.")
        || line.starts_with("glb=")
        || line.starts_with("_$jsvmprt")
        || line.starts_with("Reflect.")
}

/// 检查是否为 minified JS 代码（长行无空格，含函数/类型特征）
fn is_minified_js(line: &str) -> bool {
    // 长行无空格且含 JS 特征
    if line.len() > 60
        && !line.contains(' ')
        && (line.contains("=function") || line.contains("typeof") || line.contains("var glb"))
    {
        return true;
    }

    // 含特定 JS 关键字
    line.contains("_$jsvmprt")
        || line.contains("||Reflect.")
        || line.contains("Reflect.construct")
        || line.contains("=function")
        || line.contains("==typeof")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fetcher_config_default() {
        let config = FetcherConfig::default();
        assert_eq!(config.max_content_length, 5000);
        assert_eq!(config.concurrency, 5);
    }

    #[test]
    fn test_fetcher_config_custom() {
        let config = FetcherConfig {
            max_content_length: 1000,
            concurrency: 3,
            proxy_url: None,
        };
        assert_eq!(config.max_content_length, 1000);
        assert_eq!(config.concurrency, 3);
    }

    #[test]
    fn test_clean_text_lines_removes_short_lines() {
        let input = "这是一个很长的有意义的句子内容哈\n短\n\n\n又一个很长有意义的句子内容哈";
        let result = clean_text_lines(input);
        assert!(!result.contains("短"));
        assert!(result.contains("这是一个很长的有意义的句子内容哈"));
        assert!(result.contains("又一个很长有意义的句子内容哈"));
    }

    #[test]
    fn test_clean_text_lines_removes_js_lines() {
        let input = "这是一段正常的内容描述文字哈\nvar x = 1;\n这是另一段正常的内容文字描述\nfunction test() {}\nvar glb;(glb=\"undefined\"==typeof window?global:window)._$jsvmprt=function(b,e,f)";
        let result = clean_text_lines(input);
        assert!(!result.contains("var x"));
        assert!(!result.contains("function test"));
        assert!(!result.contains("glb="));
        assert!(!result.contains("==typeof"));
        assert!(!result.contains("=function"));
        assert!(!result.contains("_$jsvmprt"));
        assert!(result.contains("这是一段正常的内容描述文字哈"));
        assert!(result.contains("这是另一段正常的内容文字描述"));
    }

    #[test]
    fn test_clean_text_lines_removes_minified_js() {
        // 模拟今日头条页面的 minified JS 行
        let js_line = "var glb;(glb=\"undefined\"==typeof window?global:window)._$jsvmprt=function(b,e,f){function a(){if(\"undefined\"==typeof Reflect||!Reflect.construct)return!1;if(Reflect.construct.sham)return!1;if(\"function\"==typeof Proxy)return!0;try{return Date.prototype.toString.call(Reflect.construct(Date,[],(function(){try{this}return!1})))}}";
        let input = format!(
            "这是一段正常的内容文字描述段落\n{}\n还有一段正常的内容文字描述更多",
            js_line
        );
        let result = clean_text_lines(&input);
        assert!(!result.contains("_$jsvmprt"), "应过滤 _$jsvmprt 行");
        assert!(!result.contains("=function"), "应过滤 =function 行");
        assert!(!result.contains("typeof"), "应过滤 typeof 行");
        assert!(result.contains("这是一段正常的内容文字描述段落"));
        assert!(result.contains("还有一段正常的内容文字描述更多"));
    }

    #[test]
    fn test_clean_text_lines_removes_url_lines() {
        let input =
            "这是一段正常的内容描述文字哈\nhttps://example.com/path\n这是更多正常内容文字描述";
        let result = clean_text_lines(input);
        assert!(!result.contains("https://"));
        assert!(result.contains("这是一段正常的内容描述文字哈"));
    }

    #[test]
    fn test_collect_text_safe_skips_script() {
        let html = r#"
<html><body>
<p>这是一段正常的段落内容文字描述</p>
<script>var x = 1;</script>
<p>这是更多正常内容文字描述</p>
</body></html>"#;
        let doc = Html::parse_document(html);
        let sel = Selector::parse("body").unwrap();
        let body = doc.select(&sel).next().unwrap();
        let mut text = String::new();
        collect_text_safe(&body, &mut text);
        assert!(text.contains("这是一段正常的段落内容文字描述"));
        assert!(text.contains("这是更多正常内容文字描述"));
        assert!(!text.contains("var x"));
    }

    #[test]
    fn test_extract_clean_text_finds_article() {
        let html = r#"
<html><body>
<nav>这是导航链接内容文字描述</nav>
<div class="content">
<article><h1>文章标题文字内容</h1><p>文章主要内容段落文字描述</p></article>
</div>
<footer>这是页脚信息文字内容描述</footer>
</body></html>"#;
        let text = extract_clean_text(html);
        assert!(text.contains("文章主要内容段落文字描述"));
        // nav 和 footer 的内容应该被过滤掉
        assert!(!text.contains("这是导航链接内容文字描述"));
        assert!(!text.contains("这是页脚信息文字内容描述"));
    }

    #[test]
    fn test_extract_clean_text_fallback_body() {
        let html = r#"
<html><body>
<p>这是一段正文内容描述文字哈</p>
<script>var a = 1;</script>
</body></html>"#;
        let text = extract_clean_text(html);
        assert!(text.contains("这是一段正文内容描述文字哈"));
        assert!(!text.contains("var a"));
    }

    #[test]
    fn test_clean_text_lines_collapses_empty_lines() {
        let input = "第一行内容文字描述示例啊\n\n\n\n\n第二行内容文字描述示例哈";
        let result = clean_text_lines(input);
        assert_eq!(result.lines().count(), 2);
    }

    #[test]
    fn test_clean_text_lines_removes_punctuation_only() {
        let input = "这是一段正常的内容文字\n!@#$%^&*()\n这是更多正常内容文字";
        let result = clean_text_lines(input);
        assert!(!result.contains("!@#$%"));
    }
}
