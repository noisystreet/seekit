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
- Three output formats: Terminal / JSON / Raw
- Disk cache with configurable TTL
- TOML configuration management (XDG spec path)
- Anti-scraping: User-Agent rotation, exponential backoff retry, CAPTCHA detection
- Ad result filtering
- GitHub Actions CI (lint / build / test / security audit)
- Bilingual documentation (English / Chinese)
