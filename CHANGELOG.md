# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] - 2026-06-18

### Added

- Initial release of seekit
- DuckDuckGo search engine (HTML parsing, no API key required)
- SearXNG engine support (JSON API, self-hosted)
- Multi-engine fusion mode (`--engine auto`)
- Page content fetching (`--fetch`, HTML to Markdown)
- Pagination support (`--page` / `-p` for DuckDuckGo and SearXNG)
- Output formats: Terminal, JSON, Raw, CSV, Markdown
- File output (`--output` / `-o`, format auto-detected from extension)
- Disk cache with configurable TTL
- TOML configuration management (XDG spec path)
- Anti-scraping: User-Agent rotation, exponential backoff retry, CAPTCHA detection
- Ad result filtering
- GitHub Actions CI (lint / build / test / security audit)
- Dependabot for automated dependency updates
- Release workflow: multi-platform builds (linux x86_64/aarch64, macOS x86_64/arm64),
  GitHub Release with changelog, crates.io publish
- Bilingual documentation (English / Chinese)
