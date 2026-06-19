# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Google direct engine (`--engine google`): HTML scraper with CAPTCHA detection, UA rotation, exponential backoff
- Bing direct engine (`--engine bing`): HTML scraper with UA rotation, CAPTCHA detection, ad filtering
- Auto mode now queries 4 engines in parallel: DuckDuckGo, Google, Bing, SearXNG
- 16 new unit tests for Google and Bing engines

## [0.2.0] - 2026-06-19

### Added

- `--proxy` CLI flag: explicit HTTP proxy configuration (overrides env vars)
- Proxy support for DuckDuckGo, SearXNG, Auto engines, and Fetcher
- MCP tools (`search`, `fetch`) now accept `proxy` parameter
- `client_builder_with_proxy()` helper in `engine/mod.rs`
- Project-level MCP configuration: `.trae/mcp.json` for Trae IDE
- Baidu and Sogou search engines in SearXNG default config

### Changed

- `client_builder_with_proxy` signature: takes explicit `proxy_url: Option<&str>`
- SearXNG default settings: removed outbound proxy config, added Baidu/Sogou
- Manual (EN/CN): reorganized proxy docs, added MCP tool parameters, Trae IDE config

## [Unreleased]
<<<<<<<
=======

### Added

- Release workflow: multi-platform builds (linux x86_64/aarch64, macOS x86_64/arm64) via GitHub Actions
- `install.sh` — one-command installer: `curl -fsSL https://raw.githubusercontent.com/noisystreet/seekit/main/install.sh | sh`
- Homebrew formula (`homebrew/Formula/seekit.rb`) for `brew install noisystreet/tap/seekit`
- `make install` and `make install-home` targets for local builds
- Installation docs in MANUAL (EN/CN)

## [0.1.1] - 2026-06-18

### Fixed
>>>>>>>

### Added

- Release workflow: multi-platform builds (linux x86_64/aarch64, macOS x86_64/arm64) via GitHub Actions
- `install.sh` — one-command installer: `curl -fsSL https://raw.githubusercontent.com/noisystreet/seekit/main/install.sh | sh`
- Homebrew formula (`homebrew/Formula/seekit.rb`) for `brew install noisystreet/tap/seekit`
- `make install` and `make install-home` targets for local builds
- Installation docs in MANUAL (EN/CN)

## [0.1.1] - 2026-06-18

### Fixed

- Cross-compilation for aarch64-unknown-linux-gnu (switch to rustls-tls)
- Release workflow: publish job no longer depends on build matrix
- README badges pointing to crates.io and docs.rs

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
