# seekit Manual

## Installation

### Quick install (Linux / macOS)

```bash
curl -fsSL https://raw.githubusercontent.com/noisystreet/seekit/main/install.sh | sh
```

This automatically detects your OS and architecture, downloads the latest binary from GitHub Releases, and installs it to `/usr/local/bin` (or `~/.local/bin` as fallback).

### Homebrew

```bash
brew install noisystreet/tap/seekit
```

### Cargo

```bash
cargo install seekit
```

### Build from source

```bash
git clone <repo-url> && cd seekit
make build
```

Binary at `target/debug/seekit`. Or install directly:

```bash
make install          # sudo-ready, copies to /usr/local/bin
make install-home     # installs to ~/.cargo/bin
```

Or with cargo:

```bash
cargo install --path .
# Then use directly
seekit "query"
```

---

## Quick Start

```bash
# Minimal (DuckDuckGo, terminal output)
seekit "rust programming"

# JSON output (Agent mode)
seekit --format json "rust web framework"

# Specify result count
seekit --max-results 5 "rust programming"

# Use SearXNG engine
seekit --engine searxng --searxng-url http://localhost:8080 "rust"
```

---

## Search Engines

### DuckDuckGo (default)

Zero configuration, no API key required.

```bash
seekit "query"
seekit --engine duckduckgo "query"
seekit -e ddg "query"           # shortcut alias
```

**Anti-scraping note**: DuckDuckGo's HTML endpoint may trigger CAPTCHA under high-frequency requests. The tool has built-in retry (3 attempts) with exponential backoff and User-Agent rotation. If you frequently hit rate limits:

- Reduce request frequency
- Using `--no-safe` may reduce detection
- Switch to SearXNG engine

### SearXNG (self-hosted)

SearXNG is a meta search engine aggregating results from 70+ engines (Google, Bing, DuckDuckGo, etc.). Requires a self-hosted instance.

#### Prerequisites

1. A running SearXNG instance (Docker recommended)
2. Instance must have JSON output enabled
3. Rate limiter should be disabled

#### Quick Deploy

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

#### Usage

```bash
# Default URL http://localhost:8080, default language English
seekit --engine searxng "query"

# Custom URL
seekit --engine searxng --searxng-url http://192.168.1.100:8888 "query"

# Shortcut alias
seekit -e searx --searxng-url http://localhost:8080 "query"

# Specify search language (SearXNG only)
seekit -e searxng --lang zh "rust"       # Chinese results preferred
seekit -e searxng --lang ja "rust"       # Japanese results preferred
seekit -e searxng --lang "" "rust"       # No language restriction
```

> **Note**: Language filtering only works with the SearXNG engine.

---

## Output Formats

### terminal (default)

Color terminal output with numbered results, titles, URLs, and snippets:

```
10 results for 'rust programming' (duckduckgo engine, took 1828 ms)

  1. Rust Programming Language
     https://rust-lang.org/
     A language empowering everyone to build reliable and efficient software.

  2. Rust (programming language) - Wikipedia
     https://en.wikipedia.org/wiki/Rust_(programming_language)
     Rust supports multiple programming paradigms...
```

### json (Agent mode)

Structured JSON output, suitable for programmatic consumption:

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

Errors are also returned as JSON:

```json
{
  "error": "No results found for query: xxx",
  "query": "xxx",
  "engine": "duckduckgo"
}
```

### raw (pipe-friendly)

Tab-separated plain text, one result per line:

```
1       Rust Programming Language    https://rust-lang.org/
2       Rust (programming language)  https://en.wikipedia.org/wiki/Rust_(programming_language)
```

Ideal for piping:

```bash
seekit --format raw "query" | cut -f 2   # extract URLs only
seekit --format raw "query" > results.txt
```

---

## Cache Management

Results are cached by default to avoid redundant requests.

```bash
# Skip cache for this request
seekit --no-cache "query"

# Clear all cached results
seekit --clear-cache
```

- **Cache path**: `~/.cache/seekit/` (XDG spec)
- **Default TTL**: 5 minutes (configurable via `--cache-ttl`)
- **Cache key**: SHA256 hash of `(engine, query, max_results)`

---

## MCP Server (AI Agent Integration)

seekit supports the [Model Context Protocol](https://modelcontextprotocol.io/) for AI Agent integration. Start the MCP stdio server with:

```bash
seekit --mcp
```

### Available Tools

| Tool | Description |
|------|-------------|
| `search` | Search the web via DuckDuckGo, SearXNG, or auto mode |
| `fetch` | Fetch a URL and convert content to Markdown |

### Claude Desktop Configuration

Add to your `claude_desktop_config.json`:

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

### Manual Test

```bash
echo '{"jsonrpc":"2.0","id":1,"method":"tools/call",
  "params":{"name":"search","arguments":{"query":"rust programming"}}}' | seekit --mcp
```

---

## Configuration

Config file follows XDG spec at `~/.config/seekit/config.toml`.

```bash
# Generate default config file
seekit --init-config
```

Default content:

```toml
[general]
max_results = 10
timeout = 10
safe_search = true
enable_cache = true
cache_ttl_secs = 300
```

---

## Environment Variables

| Variable | Description | Example |
|----------|-------------|---------|
| `RUST_LOG` | Log level | `seekit=debug`, `seekit=warn` |
| `http_proxy` | HTTP proxy | `http://localhost:8118` |
| `https_proxy` | HTTPS proxy | `http://localhost:8118` |

```bash
RUST_LOG=seekit=debug seekit "query"
https_proxy=http://localhost:8118 seekit "query"
```

---

## Full CLI Reference

```
Usage: seekit [OPTIONS] [QUERY]

Arguments:
  [QUERY]             Search query (optional with --clear-cache or --init-config)

Options:
  -e, --engine <ENGINE>            Search engine: duckduckgo, searxng [default: duckduckgo]
      --searxng-url <SEARXNG_URL>  SearXNG instance URL (required for searxng engine)
      --lang <LANG>                Search language: en, zh, ja, etc. (SearXNG only) [default: en]
  -f, --format <FORMAT>            Output format: terminal, json, raw [default: terminal]
  -n, --max-results <MAX_RESULTS>  Max results [default: 10]
  -t, --timeout <TIMEOUT>          Request timeout in seconds [default: 10]
      --cache-ttl <CACHE_TTL>      Cache TTL in seconds [default: 300]
      --no-safe                    Disable safe search
      --no-cache                   Skip cache
      --clear-cache                Clear all cached results
      --init-config                Generate default config file
  -h, --help                       Print help
  -V, --version                    Print version
```

---

## Examples

### Daily Search

```bash
seekit "rust async await tutorial"
seekit -n 5 "rust web framework 2024"
```

### Agent Usage

```bash
# Search and get JSON
seekit -f json "latest rust version" | jq '.results[].title'

# Search and extract URLs
seekit -f raw "rust documentation" | cut -f 2

# Errors in JSON mode
seekit -f json "some_rare_query_xyz_123"
# → {"error": "No results found for query: ...", "engine": "duckduckgo", "query": "..."}
```

### Scripting

```bash
# Batch search
for q in "rust" "go" "python"; do
    seekit -f raw -n 3 "$q" >> results.tsv
done

# Filter results by domain
seekit -f json "rust" | jq '.results[] | select(.url | contains("github.com")).url'
```

### Language Control

```bash
# English results (default)
seekit -e searxng "rust web framework"

# Chinese results
seekit -e searxng --lang zh "rust web framework"

# Japanese results
seekit -e searxng --lang ja "rust"

# No language restriction
seekit -e searxng --lang "" "rust"
```

### Self-hosted SearXNG

```bash
# Deploy SearXNG first (see docker-compose above)
seekit -e searxng --searxng-url http://localhost:8080 "rust programming"
seekit -e searxng --searxng-url http://localhost:8080 -f json "rust programming"
seekit -e searxng --searxng-url http://localhost:8080 -n 20 "rust programming"
```

### Troubleshooting

```bash
# Enable debug logging
RUST_LOG=seekit=debug seekit "query"

# Force fresh results (skip cache)
seekit --no-cache "query"

# Clear cache and retry
seekit --clear-cache && seekit "query"

# Test SearXNG instance directly
curl "http://localhost:8080/search?q=test&format=json"
```
