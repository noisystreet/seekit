# seekit

[![CI](https://github.com/noisystreet/seekit/actions/workflows/ci.yml/badge.svg)](https://github.com/noisystreet/seekit/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/seekit)](https://crates.io/crates/seekit)
[![docs.rs](https://img.shields.io/docsrs/seekit)](https://docs.rs/seekit)
[![codecov](https://codecov.io/gh/noisystreet/seekit/branch/main/graph/badge.svg)](https://codecov.io/gh/noisystreet/seekit)

[中文版](README.zh.md)

A Rust CLI web search tool supporting DuckDuckGo and SearXNG engines, designed for terminal and Agent usage.

## Features

- **Zero config**: DuckDuckGo engine works out of the box — no API key required
- **Dual engine**: DuckDuckGo (zero setup) + SearXNG (self-hosted meta search)
- **Three output formats**: Terminal color table / JSON (Agent-friendly) / Raw (pipe-friendly)
- **Caching**: Disk cache with configurable TTL (default 5 min)
- **Anti-scraping**: User-Agent rotation, automatic retry, CAPTCHA detection
- **Embeddable**: Use as CLI tool or as a Rust library
- **MCP Server**: AI Agent integration via Model Context Protocol (`--mcp`)

## Quick Start

```bash
# Search (default DuckDuckGo)
cargo run -- "rust programming"

# JSON output (Agent mode)
cargo run -- --format json "rust"

# Use SearXNG engine
cargo run -- --engine searxng --searxng-url http://localhost:8080 "rust"

# Raw output (pipe-friendly)
cargo run -- --format raw "rust" | cut -f 2

# Manage cache
cargo run -- --clear-cache
```

## Documentation

- [User Manual (English)](docs/MANUAL.md) — install, search, engines, output formats, examples
- [用户手册 (中文)](docs/MANUAL.zh.md) — 安装、搜索、引擎切换、完整示例
- [Design Document](docs/adr/DESIGN.md) — architecture, data flow, interface design
- [Contributing Guide](CONTRIBUTING.md) — how to participate
- [Agent Guide](AGENTS.md) — for AI-assisted development

## Quick Reference

| Usage | Command |
|-------|---------|
| Basic search | `seekit "query"` |
| JSON output | `seekit -f json "query"` |
| SearXNG engine | `seekit -e searxng --searxng-url http://localhost:8080 "query"` |
| Limit results | `seekit -n 3 "query"` |
| Skip cache | `seekit --no-cache "query"` |

## MCP Server (AI Agent Integration)

seekit supports the [Model Context Protocol](https://modelcontextprotocol.io/) for AI Agent integration.

```bash
# Start MCP stdio server
seekit --mcp
```

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

### Available Tools

| Tool | Description |
|------|-------------|
| `search` | Search the web via DuckDuckGo, SearXNG, or auto mode |
| `fetch` | Fetch a URL and convert content to Markdown |

### Manual Test

```bash
echo '{"jsonrpc":"2.0","id":1,"method":"tools/call",
  "params":{"name":"search","arguments":{"query":"rust programming"}}}' | seekit --mcp
```

## License

MIT © 2026 seekit contributors

## Disclaimer

This tool accesses DuckDuckGo's HTML interface programmatically. Users are responsible for complying with DuckDuckGo's Terms of Service and applicable rate limits. The tool includes CAPTCHA detection and exponential backoff to minimize server impact.
