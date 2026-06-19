# seekit 用户手册

## 安装

### 快速安装（Linux / macOS）

```bash
curl -fsSL https://raw.githubusercontent.com/noisystreet/seekit/main/install.sh | sh
```

自动检测系统架构，从 GitHub Releases 下载最新二进制并安装到 `/usr/local/bin`（或 `~/.local/bin`）。

### Homebrew

```bash
brew install noisystreet/tap/seekit
```

### Cargo

```bash
cargo install seekit
```

### 从源码构建

```bash
git clone <repo-url> && cd seekit
make build
```

构建产物位于 `target/debug/seekit`。也可直接安装：

```bash
make install          # 复制到 /usr/local/bin
make install-home     # 安装到 ~/.cargo/bin
```

或使用 cargo 安装：

```bash
cargo install --path .
# 之后可直接调用
seekit "query"
```

---

## 快速入门

```bash
# 最简用法（DuckDuckGo，终端输出）
seekit "rust programming"

# JSON 输出（Agent 调用模式）
seekit --format json "rust web framework"

# 指定结果数量
seekit --max-results 5 "rust programming"

# 使用 SearXNG 引擎
seekit --engine searxng --searxng-url http://localhost:8080 "rust"
```

---

## 搜索引擎

### DuckDuckGo（默认）

零配置，无需 API Key，开箱即用。

```bash
seekit "query"
seekit --engine duckduckgo "query"
seekit -e ddg "query"           # 快捷别名
```

**反爬说明**：DuckDuckGo HTML 端点在短时间内高频请求可能触发 CAPTCHA 验证。工具内置了三次重试 + 指数退避 + User-Agent 轮换，如频繁遇到限流，建议：

- 降低请求频率，间隔几秒再试
- 使用 `--no-safe` 可能减少检测概率
- 换用 SearXNG 引擎
- 配置代理：`seekit --proxy http://127.0.0.1:7890 "query"`

### SearXNG（自部署）

SearXNG 是一个元搜索引擎，聚合 Google、Bing、DuckDuckGo 等 70+ 引擎的结果。需要自部署实例。

#### 前提条件

1. 运行中的 SearXNG 实例（推荐 Docker 部署）
2. 实例需开启 JSON 输出
3. 建议关闭限流器

#### 快速部署 SearXNG

```yaml
# docker-compose.yml
services:
  searxng:
    image: searxng/searxng:latest
    ports:
      - "8080:8080"
    volumes:
      - ./searxng:/etc/searxng:rw
    environment:
      - SEARXNG_BASE_URL=http://localhost:8080/
    restart: unless-stopped
```

```bash
mkdir -p searxng
docker compose up -d
```

默认的 `deploy/searxng/settings.yml` 启用了 Google、Bing、DuckDuckGo、Brave、百度、搜狗等搜索引擎。SearXNG 的出站代理配置参见[代理与网络](#代理与网络)章节。

#### 使用

```bash
# 默认地址 http://localhost:8080，默认语言英文
seekit --engine searxng "query"

# 自定义地址
seekit --engine searxng --searxng-url http://192.168.1.100:8888 "query"

# 快捷别名
seekit -e searx --searxng-url http://localhost:8080 "query"

# 指定搜索语言（SearXNG 引擎）
seekit -e searxng --lang zh "rust"       # 中文结果优先
seekit -e searxng --lang ja "rust"       # 日文结果优先
seekit -e searxng --lang "" "rust"       # 不限制语言
```

> **注意**：指定搜索语言仅对 SearXNG 引擎生效。

---

## 输出格式

### terminal（默认）

彩色终端输出，显示编号、标题、URL、摘要：

```
10 results for 'rust programming' (duckduckgo engine, took 1828 ms)

  1. Rust Programming Language
     https://rust-lang.org/
     A language empowering everyone to build reliable and efficient software.

  2. Rust (programming language) - Wikipedia
     https://en.wikipedia.org/wiki/Rust_(programming_language)
     Rust supports multiple programming paradigms...
```

### json（Agent 调用）

结构化 JSON 输出，适合被其他程序解析：

```json
{
  "query": "rust programming",
  "engine": "duckduckgo",
  "results": [
    {
      "title": "Rust Programming Language",
      "url": "https://rust-lang.org/",
      "snippet": "A language empowering everyone..."
    }
  ],
  "total_estimated": 10,
  "took_ms": 1828
}
```

错误时输出 JSON 格式错误信息：

```json
{
  "error": "No results found for query: xxx",
  "query": "xxx",
  "engine": "duckduckgo"
}
```

### raw（管道友好）

纯文本精简格式，每行一个结果，Tab 分隔：

```
1       Rust Programming Language    https://rust-lang.org/
2       Rust (programming language)  https://en.wikipedia.org/wiki/Rust_(programming_language)
```

适合管道处理：

```bash
seekit --format raw "query" | cut -f 2   # 只提取 URL
seekit --format raw "query" > results.txt
```

---

## 缓存管理

工具默认对搜索结果进行缓存，避免重复请求。

```bash
# 跳过本次缓存的读取和写入
seekit --no-cache "query"

# 清空所有缓存
seekit --clear-cache
```

- **缓存路径**：`~/.cache/seekit/`（XDG 规范）
- **默认 TTL**：5 分钟（可通过 `--cache-ttl` 配置）
- **缓存键**：`(引擎名, 查询词, 结果数)` 的 SHA256 哈希

---

## MCP 服务（AI Agent 集成）

seekit 支持 [Model Context Protocol](https://modelcontextprotocol.io/) 协议，AI Agent（Claude Desktop、Gemini 等）可直接调用搜索能力。

```bash
# 启动 MCP stdio 服务
seekit --mcp
```

### 可用工具

| 工具 | 说明 |
|------|------|
| `search` | 通过 DuckDuckGo、SearXNG 或 auto 模式搜索网页 |
| `fetch` | 获取 URL 内容并转换为 Markdown |

### 工具参数

#### `search` 参数

| 参数 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `query` | string | — | 搜索关键词（必填） |
| `engine` | string | `duckduckgo` | 引擎: `duckduckgo`, `searxng`, `auto` |
| `max_results` | integer | `10` | 最大结果数 |
| `page` | integer | `1` | 页码 |
| `lang` | string | `en` | 搜索语言（仅 SearXNG） |
| `searxng_url` | string | — | SearXNG 实例地址 |
| `proxy` | string | — | HTTP 代理地址（如 `http://127.0.0.1:7890`） |

#### `fetch` 参数

| 参数 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `url` | string | — | 要获取的 URL（必填） |
| `max_content_length` | integer | `5000` | 内容最大字符数 |
| `proxy` | string | — | HTTP 代理地址 |

### Trae IDE 配置

在项目根目录创建 `.trae/mcp.json`：

```json
{
  "mcpServers": {
    "seekit": {
      "command": "/path/to/seekit",
      "args": ["--mcp"],
      "env": {}
    }
  }
}
```

然后在 **设置 > MCP** 中打开 **启用项目级 MCP** 开关。

### Claude Desktop 配置

添加到 `claude_desktop_config.json`：

```json
{
  "mcpServers": {
    "seekit": {
      "command": "seekit",
      "args": ["--mcp"]
    }
  }
}
```

### 手动测试

```bash
echo '{"jsonrpc":"2.0","id":1,"method":"tools/call",
  "params":{"name":"search","arguments":{"query":"rust"}}}' | seekit --mcp
```

## 代理与网络

当 DuckDuckGo 或 SearXNG 无法直接访问外部网络时，需要配置代理。

### 通过 CLI 参数（`--proxy`）

```bash
seekit --proxy http://127.0.0.1:7890 "query"
```

### 通过环境变量

| 变量 | 说明 | 示例 |
|------|------|------|
| `HTTPS_PROXY` / `https_proxy` | HTTPS 代理 | `http://localhost:8118` |
| `HTTP_PROXY` / `http_proxy` | HTTP 代理 | `http://localhost:8118` |
| `ALL_PROXY` / `all_proxy` | 备用代理 | `http://localhost:8118` |

优先级：`--proxy` 命令行参数 > `HTTPS_PROXY` > `http_proxy` > `ALL_PROXY`。

```bash
HTTPS_PROXY=http://localhost:8118 seekit "query"
```

### SearXNG 出站代理

自部署 SearXNG 需要在 `settings.yml` 中配置出站代理：

```yaml
outgoing:
  proxies:
    http: http://localhost:8118
    https: http://localhost:8118
```

---

## 配置管理

配置文件遵循 XDG 规范，位于 `~/.config/seekit/config.toml`。

```bash
# 生成默认配置文件
seekit --init-config
```

默认内容：

```toml
[general]
max_results = 10
timeout = 10
safe_search = true
enable_cache = true
cache_ttl_secs = 300
```

---

## 完整参数列表

```
Usage: seekit [OPTIONS] [QUERY]

Arguments:
  [QUERY]             搜索关键词（--clear-cache 或 --init-config 时可选）

Options:
  -e, --engine <ENGINE>            搜索引擎: duckduckgo, searxng, auto [default: duckduckgo]
      --searxng-url <SEARXNG_URL>  SearXNG 实例地址（--engine searxng 时使用）
      --proxy <PROXY>              HTTP 代理地址（如 http://127.0.0.1:7890）
      --lang <LANG>                搜索语言（如 en, zh, ja），仅 SearXNG 引擎生效 [default: en]
  -f, --format <FORMAT>            输出格式: terminal, json, raw [default: terminal]
  -n, --max-results <MAX_RESULTS>  最大结果数 [default: 10]
  -p, --page <PAGE>                页码 [default: 1]
  -t, --timeout <TIMEOUT>          请求超时（秒）[default: 10]
  -F, --fetch                      获取页面内容（HTML 转 Markdown）
      --max-content-length <LEN>   每页最大字符数 [default: 5000]
      --cache-ttl <CACHE_TTL>      缓存 TTL（秒）[default: 300]
      --no-safe                    禁用安全搜索
      --no-cache                   跳过缓存
      --clear-cache                清空缓存
      --init-config                生成默认配置文件
  -o, --output <FILE>              输出文件（从扩展名自动识别格式）
      --completion <SHELL>         生成 Shell 自动补全脚本
      --mcp                        启动 MCP stdio 服务
  -h, --help                       Print help
  -V, --version                    Print version
```

---

## 场景示例

### 日常搜索

```bash
seekit "rust async await tutorial"
seekit -n 5 "rust web framework 2024"
```

### Agent 调用

```bash
# 搜索并返回 JSON
seekit -f json "latest rust version" | jq '.results[].title'

# 搜索+提取 URL
seekit -f raw "rust documentation" | cut -f 2

# 错误时也返回 JSON
seekit -f json "some_rare_query_xyz_123"
# → {"error": "No results found for query: ...", "engine": "duckduckgo", "query": "..."}
```

### 脚本管道

```bash
# 批量搜索关键词
for q in "rust" "go" "python"; do
    seekit -f raw -n 3 "$q" >> results.tsv
done

# 搜索并过滤域名
seekit -f json "rust" | jq '.results[] | select(.url | contains("github.com")).url'
```

### 搜索语言控制

```bash
# 英文结果（默认）
seekit -e searxng "rust web framework"

# 中文结果
seekit -e searxng --lang zh "rust web framework"

# 日文结果
seekit -e searxng --lang ja "rust"

# 不限制语言
seekit -e searxng --lang "" "rust"
```

### 自部署 SearXNG

```bash
seekit -e searxng --searxng-url http://localhost:8080 "rust programming"
seekit -e searxng --searxng-url http://localhost:8080 -f json "rust programming"
seekit -e searxng --searxng-url http://localhost:8080 -n 20 "rust programming"
```

### 调试/排错

```bash
# 查看详细日志
RUST_LOG=seekit=debug seekit "query"

# 禁用缓存，确保获取最新结果
seekit --no-cache "query"

# 清空缓存后重试
seekit --clear-cache && seekit "query"

# 测试 SearXNG 实例是否正常
curl "http://localhost:8080/search?q=test&format=json"
```
