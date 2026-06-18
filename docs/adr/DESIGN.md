# seekit — Design Document

## 1. Overview

`seekit` is a Rust-based CLI web search tool supporting DuckDuckGo and SearXNG engines, designed for terminal users and automated Agent scenarios.

### Core Goals

- **Zero config, out of the box**: No API key, no registration required — install and use
- **Dual-mode output**: Terminal-friendly format + JSON format (Agent-friendly)
- **Embeddable**: Can be used as a standalone CLI tool or as a Rust library

---

## 2. Project Structure

```
seekit/
├── Cargo.toml
├── Cargo.lock
├── LICENSE                 # MIT
├── Makefile                # Unified command entry
├── README.md               # Project intro (English)
├── README.zh.md            # Project intro (Chinese)
├── AGENTS.md               # AI Agent guide (English)
├── CHANGELOG.md            # Changelog (English)
├── CONTRIBUTING.md         # Contributing guide
├── SECURITY.md             # Security policy
├── CODE_OF_CONDUCT.md      # Code of conduct
├── CODEOWNERS              # Code review owners
├── rust-toolchain.toml     # Toolchain lock
├── rustfmt.toml            # Format config
├── .editorconfig           # Editor config
├── .env.example            # Env vars example
├── .gitignore
├── .gitmessage             # Commit message template
├── .pre-commit-config.yaml # pre-commit hooks
│
├── src/
│   ├── main.rs              # CLI entry point
│   ├── lib.rs               # Public library API
│   ├── cli.rs               # CLI argument parsing (clap)
│   ├── engine/
│   │   ├── mod.rs          # EngineType enum + dispatch
│   │   ├── trait.rs        # SearchEngine trait + data structures
│   │   ├── duckduckgo.rs   # DuckDuckGo engine
│   │   ├── searxng.rs      # SearXNG engine
│   │   └── fusion.rs       # Multi-engine fusion (auto mode)
│   ├── config.rs             # Config file management
│   ├── cache.rs              # Result cache
│   ├── output.rs             # Output formatting
│   ├── fetcher.rs            # Page content fetcher
│   └── error.rs              # Unified error types
│
├── tests/
│   └── integration_test.rs   # Integration tests
│
├── docs/
│   └── adr/
│       └── DESIGN.md         # Architecture design document
│
└── .github/
    ├── workflows/
    │   └── ci.yml            # GitHub Actions CI
    ├── ISSUE_TEMPLATE/
    │   ├── bug_report.md
    │   ├── feature_request.md
    │   └── config.yml
    └── pull_request_template.md
```

---

## 3. Data Flow

```
User input (CLI args or Library API)
        │
        ▼
  ┌─────────────┐
  │  lib.rs     │  ← Parse args, init engine, handle cache
  └──────┬──────┘
         │
         ├── DuckDuckGo engine (default)
         │  ┌────────────────────────┐
         │  │  1. Build HTML search  │
         │  │     URL                │
         │  │  2. Random User-Agent  │
         │  │  3. HTTP request       │
         │  │     (retryable)        │
         │  │  4. HTML parse         │
         │  │  5. Ad filter + URL    │
         │  │     decode             │
         │  └────────────────────────┘
         │
         └── SearXNG engine (--engine searxng)
            ┌────────────────────────┐
            │  1. Build JSON API URL │
            │  2. HTTP request       │
            │  3. JSON parse         │
            │  4. URL dedup          │
            └────────────────────────┘
                    │
                    ▼
  ┌──────────────────────────────┐  ← Optional: --fetch
  │       Fetcher                │
  │  ┌────────────────────────┐  │
  │  │  1. Iterate result URLs│  │
  │  │  2. HTTP GET each page │  │
  │  │  3. HTML → Markdown    │  │
  │  │  4. Truncate to        │  │
  │  │     max_length         │  │
  │  └────────────────────────┘  │
  └──────────┬───────────────────┘
             │
             ▼
  ┌──────────────────────────────┐
  │       Output Formatter       │  ← Terminal / JSON / Raw
  └──────────────────────────────┘
```

### Fetch Path (when --fetch is enabled)

```
search returns Vec<SearchResult>
         │
         ▼
  fetcher::fetch_contents(results, config)
         │
         ├── Concurrent HTTP requests to each URL
         ├── HTML → Markdown conversion
         ├── Truncate to max_content_length
         └── Fill SearchResult.content
         │
         ▼
  Returns Vec<SearchResult> (with content field)

---

## 4. Core Data Structures

```rust
/// Unified search result entry
struct SearchResult {
    title: String,
    url: String,
    snippet: String,
    /// Page content (Markdown), only filled with --fetch
    content: Option<String>,
}

/// Unified search response (for Agent consumption)
struct SearchResponse {
    query: String,
    engine: String,
    results: Vec<SearchResult>,
    total_estimated: Option<usize>,
    took_ms: u64,
}

/// Output format
enum OutputFormat {
    Terminal,  // Color terminal output
    Json,      // JSON format (Agent-friendly)
    Raw,       // Plain text minimal output
}
```

---

## 5. CLI Interface

```bash
# Basic search (terminal output)
seekit "rust web framework"

# JSON output (Agent mode)
seekit --format json --max-results 10 "rust web framework"

# Raw output (pipe-friendly)
seekit --format raw "query" > results.txt

# Disable cache and safe search
seekit --no-cache --no-safe "query"

# Manage cache and config
seekit --clear-cache
seekit --init-config
```

### Parameter Reference

| Parameter | Short | Type | Default | Description |
|-----------|-------|------|---------|-------------|
| `<query>` | — | String | required | Search query |
| `--engine` | `-e` | String | `duckduckgo` | Search engine: duckduckgo / searxng |
| `--searxng-url` | — | String | — | SearXNG instance URL (required for searxng engine) |
| `--format` | `-f` | String | `terminal` | Output format: terminal / json / raw |
| `--fetch` | `-F` | bool | `false` | Fetch page content (Markdown) for results |
| `--max-content-length` | — | usize | `5000` | Max chars per fetched page (with --fetch) |
| `--max-results` | `-n` | usize | `10` | Max results |
| `--timeout` | `-t` | u64 | `10` | Request timeout in seconds |
| `--lang` | — | String | `en` | Search language (SearXNG engine) |
| `--no-safe` | — | bool | `false` | Disable safe search |
| `--no-cache` | — | bool | `false` | Skip cache |
| `--clear-cache` | — | bool | `false` | Clear all cached results |
| `--init-config` | — | bool | `false` | Generate default config file |

---

## 6. Engine: DuckDuckGo

### How It Works

Uses DuckDuckGo's [HTML endpoint](https://html.duckduckgo.com/), extracting search results via HTML parsing:

1. Sends GET request to `https://html.duckduckgo.com/html/?q=<query>&kp=<safe>`
2. Parses HTML, extracts title, URL, and snippet from `.result` containers
3. Decodes real URL from redirect link `uddg=` parameter
4. Filters ad results (detects DuckDuckGo ad tracking and Bing ads via URL parsing)

### Anti-Scraping Strategy

| Strategy | Implementation |
|----------|---------------|
| User-Agent rotation | 5 different browser UAs, randomly selected |
| Automatic retry | Up to 3 attempts, exponential backoff (2s → 4s) |
| CAPTCHA detection | Detects challenge/anomaly-modal keywords in page |

---

## 7. Engine: SearXNG

SearXNG is a self-hosted meta search engine aggregating 70+ engines (Google, Bing, DuckDuckGo, etc.) via JSON API.

### Prerequisites

- A running SearXNG instance (Docker deployment recommended)
- Instance must have JSON output enabled (`search.formats: [html, json]` in `settings.yml`)
- Rate limiter should be disabled (`server.limiter: false`) for CLI access

### Usage

```bash
seekit --engine searxng --searxng-url http://localhost:8080 "rust programming"
seekit -e searxng --searxng-url http://192.168.1.100:8888 -f json "rust"
```

### API Call

Sends GET request to SearXNG instance for JSON response:

```
GET /search?q=<query>&format=json&categories=general
```

Response contains `results[]` array, each result has `title`, `url`, `content` fields.

---

## 8. Cache

- **Cache key**: SHA256 hash of `(engine, query, max_results)`
- **Cache path**: `~/.cache/seekit/`
- **TTL**: 5 minutes
- **Commands**: `--no-cache` to skip cache, `--clear-cache` to clear all

---

## 9. Output Examples

### Terminal Output
```
10 results for 'rust programming' (duckduckgo engine, took 1828 ms)

  1. Rust Programming Language
     https://rust-lang.org/
     A language empowering everyone to build reliable and efficient software.

  2. Rust (programming language) - Wikipedia
     https://en.wikipedia.org/wiki/Rust_(programming_language)
     Rust supports multiple programming paradigms...
```

### JSON Output (Agent-friendly)
```json
{
  "query": "rust programming",
  "engine": "duckduckgo",
  "results": [
    {
      "title": "Rust Programming Language",
      "url": "https://rust-lang.org/",
      "snippet": "A language empowering everyone to build reliable and efficient software."
    }
  ],
  "total_estimated": 10,
  "took_ms": 1828
}
```

### JSON Error Output
```json
{
  "error": "CAPTCHA challenge detected by DuckDuckGo...",
  "query": "rust web framework",
  "engine": "duckduckgo"
}
```

---

## 10. Fetcher: Page Content Extraction

### Overview

The `fetcher.rs` module fetches page content for each search result. When `--fetch` is enabled, after the search phase completes, it sends HTTP requests to each result URL, converts HTML to Markdown, and fills `SearchResult.content`.

Dependencies: `scraper` (existing) — uses the same HTML parser already pulled in for DuckDuckGo result parsing.

### Design

```rust
/// Fetcher configuration
struct FetcherConfig {
    /// Max characters per page
    pub max_content_length: usize,
    /// Concurrency limit
    pub concurrency: usize,
}

/// Page content fetcher
struct Fetcher {
    client: reqwest::Client,
    config: FetcherConfig,
}

impl Fetcher {
    /// Create a new Fetcher instance
    pub fn new(config: FetcherConfig) -> Self;

    /// Fetch content for multiple URLs concurrently
    /// Uses futures::future::join_all for concurrency
    /// Per result: HTTP GET → HTML → Markdown → truncate
    pub async fn fetch(&self, results: &mut [SearchResult]);
}
```

### Data Flow

```
Vec<SearchResult> (from search phase)
        │
        ▼
fetcher.fetch(&mut results)
        │
        ├── Concurrent HTTP requests to N URLs
        ├── Each response: reqwest → response.text()
        ├── html2md::parse_html(raw_html) → Markdown String
        ├── Truncate to max_content_length
        └── Fill result.content = Some(markdown)
        │
        ▼
Vec<SearchResult> (with content field)
```

### Output Examples

In Terminal mode, content is appended indented after each result:

```
  1. Rust Programming Language
     https://rust-lang.org/
     A language empowering everyone to build reliable and efficient software.
     ── Content ──────────────────────
     Rust is a systems programming language ...

```

In JSON mode, each result includes a `content` field:

```json
{
  "title": "Rust Programming Language",
  "url": "https://rust-lang.org/",
  "snippet": "A language...",
  "content": "Rust is a systems programming language..."
}
```

### Cache Strategy

Page content is cached independently from search results:

- **Cache path**: `~/.cache/seekit/fetch/`
- **Cache key**: SHA256 hash of the URL
- **TTL**: Same as search result cache (controlled by `--cache-ttl`)

### Error Handling

- Single URL failure does not affect other URLs
- Failed results keep `content: None`
- Terminal output shows `[fetch failed]` marker

---

## 11. Library API (lib.rs public interface)

```rust
/// Execute a search (for embedding in other Rust programs)
pub async fn search(cli: &Cli) -> Result<SearchResponse>;

/// Run the CLI application (called by main.rs)
pub async fn run() -> anyhow::Result<()>;
```

---

## 12. Technology Stack

| Module | Library | Purpose |
|--------|---------|---------|
| CLI parsing | `clap` | Argument parsing |
| HTTP client | `reqwest` | Async HTTP requests |
| HTML parsing | `scraper` | HTML parsing (search results + page content extraction) |
| Serialization | `serde` + `serde_json` | JSON output |
| Async runtime | `tokio` | Async support |
| Error handling | `thiserror` + `anyhow` | Error types |
| Cache | `sha2` + `hex` | Cache key hashing |
| URL encoding | `urlencoding` | Search URL building |
| Config | `toml` | Config file parsing |
| Logging | `tracing` | Debugging / observability |

---

## 13. Configuration Management

Config file path (XDG spec):

```
~/.config/seekit/config.toml
```

```toml
[general]
max_results = 10
timeout = 10
safe_search = true
enable_cache = true
```

---

## 14. Extensibility

### Adding a New Search Engine

Implement the `SearchEngine` trait:

```rust
#[async_trait]
trait SearchEngine {
    fn name(&self) -> &'static str;
    async fn search(&self, query: &str, config: &EngineConfig)
        -> Result<Vec<SearchResult>>;
}
```

### Adding a New Output Format

Add a new formatting function in `output.rs` and register it.

### Adding a New Content Extractor

Add a new function in `fetcher.rs`. Currently only HTML → Markdown conversion is supported,
which can be extended to PDF extraction, JSON-LD extraction, etc.

---

## 15. Multi-Engine Parallel Search with Result Fusion

### Overview

Currently, seekit selects a single engine at a time (DuckDuckGo or SearXNG). This section
describes a client-side multi-engine fusion mode where both engines are queried in parallel and
their results are merged, deduplicated, and ranked.

This gives users two benefits:
- **More results**: Both DuckDuckGo and SearXNG contribute results
- **Better ranking**: Results appearing in both engines score higher (consensus signal)

No additional dependencies are required — the existing `reqwest` and `tokio` crates are sufficient.

### CLI Interface

A new engine alias `auto` queries all available engines in parallel:

```bash
# Use all available engines
seekit --engine auto "rust"
seekit -e auto "rust web framework"
```

The existing `--engine duckduckgo` and `--engine searxng` modes remain unchanged for users who
prefer a single engine.

### Data Flow

```
User input
        │
        ▼
  ┌─────────────┐
  │  lib.rs     │  ← Parse args, init engines
  └──────┬──────┘
         │
         ├── auto mode: spawn all engines concurrently
         │
         ├── Fork 1: DuckDuckGo engine
         │      └── HTTP request → HTML parse → ad filter
         │
         ├── Fork 2: SearXNG engine (if configured)
         │      └── HTTP request → JSON parse → URL dedup
         │
         └── Fork N: future engines
                └── ...
         │
         ▼
  ┌──────────────────────────────┐
  │       Result Merger          │
  │  ├── Collect all results     │
  │  ├── Normalize URLs          │
  │  ├── Dedup by normalized URL │
  │  ├── Score by consensus      │
  │  └── Sort by score (desc)    │
  └──────────┬───────────────────┘
             │
             ▼
  ┌──────────────────────────────┐
  │       Truncate + Output      │  ← max_results limit
  └──────────────────────────────┘
```

### Design

```rust
/// Multi-engine result with source tracking
struct ScoredResult {
    result: SearchResult,
    /// Set of engine names that found this result
    sources: Vec<String>,
    /// Consensus score
    score: f64,
}

/// Result merger — combines, deduplicates, and ranks results
struct ResultMerger;

impl ResultMerger {
    /// Merge results from multiple engines
    /// 1. Collect all results with source labels
    /// 2. Normalize URLs (strip trailing slashes, www prefix)
    /// 3. Dedup by normalized URL
    /// 4. Compute score per result: Σ (weight / position)
    /// 5. Sort by score descending
    pub fn merge(engine_results: Vec<(&str, Vec<SearchResult>)>, max_results: usize)
        -> Vec<ScoredResult>;
}
```

### Scoring Algorithm

Follows the same consensus-based approach used by SearXNG and a3s-search:

```rust
for each result:
    score = Σ (weight / position)

    where:
      weight     = engine_weight × num_engines_found
      position   = index in single-engine result list (1-based)
```

Key factors:
- **Engine weight**: DuckDuckGo = 1.0, SearXNG = 1.0 (configurable in future)
- **Consensus**: A result found by both engines gets `weight = 1.0 × 2 = 2.0`
- **Position**: Earlier positions score higher (1/1 vs 1/5)

Example: A result ranked #1 in DuckDuckGo and #3 in SearXNG:
```
score = (1.0 × 2) / 1 + (1.0 × 2) / 3 = 2.0 + 0.67 = 2.67
```

### Output

In Terminal mode, the engine label shows "auto" and each result displays its source engines:

```
15 results for 'rust' (auto engine, took 2340 ms)

  1. [ddg+searxng] Rust Programming Language              ★ 2.67
     https://rust-lang.org/
     A language empowering everyone...

  2. [ddg] Rust - Wikipedia                                ★ 1.00
     https://en.wikipedia.org/wiki/Rust
     Rust is a multi-paradigm language...
```

In JSON mode, each result includes a `score` and `sources` field:

```json
{
  "query": "rust",
  "engine": "auto",
  "results": [
    {
      "title": "Rust Programming Language",
      "url": "https://rust-lang.org/",
      "snippet": "A language empowering everyone...",
      "score": 2.67,
      "sources": ["duckduckgo", "searxng"]
    }
  ],
  "total_estimated": 15,
  "took_ms": 2340
}
```

### Cache Strategy

The cache key for auto mode uses the string `auto` as the engine component:

```
SHA256("auto:query:max_results")
```

When auto mode is used, individual engine results are not cached separately (to avoid
double-caching). The merged result is cached as a single entry.

### Integration Points

| Component | Change |
|-----------|--------|
| `engine/mod.rs` | Add `EngineType::Auto` variant |
| `cli.rs` | Add `auto` to engine help text |
| `lib.rs` | `create_engine()` returns `AutoEngine` wrapper |
| New file | `engine/fusion.rs` — `AutoEngine` + `ResultMerger` |
| `output.rs` | Show sources and score in terminal output |
| `SearchResponse` | Add optional `sources` and `score` fields |

### Error Handling

- If one engine fails (e.g. SearXNG unreachable), auto mode falls back to the other engine
- A warning is logged for each failed engine
- If all engines fail, returns the appropriate error
- This provides graceful degradation — one engine down doesn't break the search

---

## 16. Development Roadmap

| Phase | Content | Status |
|-------|---------|--------|
| **Phase 1** | Project skeleton, CLI parsing, DuckDuckGo engine, Terminal/JSON output, cache | ✅ Done |
| **Phase 2** | Anti-scraping (UA rotation, retry, CAPTCHA detection), config management | ✅ Done |
| **Phase 3** | Project polish (LICENSE, CI, pre-commit, tests, Agent docs) | ✅ Done |
| **Phase 4** | SearXNG engine (multi-engine dispatch, JSON API adapter) | ✅ Done |
| **Phase 5** | Test coverage, CI/CD, crates.io publishing | ❌ Pending |
| **Phase 6** | Page content fetching (scraper-based text extraction) | ✅ Done |
| **Phase 7** | Multi-engine parallel search with client-side fusion | ✅ Done |
