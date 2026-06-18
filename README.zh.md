# seekit

[![CI](https://github.com/noisystreet/seekit/actions/workflows/ci.yml/badge.svg)](https://github.com/noisystreet/seekit/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/seekit)](https://crates.io/crates/seekit)
[![docs.rs](https://img.shields.io/docsrs/seekit)](https://docs.rs/seekit)
[![codecov](https://codecov.io/gh/noisystreet/seekit/branch/main/graph/badge.svg)](https://codecov.io/gh/noisystreet/seekit)

[English Version](README.md)

一个基于 Rust 的命令行 Web 搜索工具，支持 DuckDuckGo 和 SearXNG 引擎，面向终端用户和 Agent 调用场景。

## 特性

- **零配置**：DuckDuckGo 引擎无需 API Key，安装即用
- **双引擎**：DuckDuckGo（开箱即用）+ SearXNG（自部署元搜索引擎）
- **三输出格式**：终端彩色表格 / JSON（Agent 友好）/ 纯文本（管道友好）
- **缓存**：磁盘缓存，TTL 可配置（默认 5 分钟）
- **反爬对抗**：User-Agent 轮换、自动重试、CAPTCHA 检测
- **可嵌入**：既可作为 CLI 工具，也可作为 Rust 库嵌入

## 快速开始

```bash
# 搜索（默认 DuckDuckGo）
cargo run -- "rust programming"

# JSON 输出（Agent 调用模式）
cargo run -- --format json "rust"

# 使用 SearXNG 引擎
cargo run -- --engine searxng --searxng-url http://localhost:8080 "rust"

# 纯文本输出（管道友好）
cargo run -- --format raw "rust" | cut -f 2   # 只提取 URL

# 管理缓存
cargo run -- --clear-cache
```

## 文档

- [用户手册](MANUAL.zh.md) — 安装、搜索、引擎切换、输出格式、完整示例
- [设计文档](adr/DESIGN.md) — 架构、数据流、接口设计
- [贡献指南](../CONTRIBUTING.md) — 如何参与开发
- [Agent 指南](../AGENTS.md) — 面向 AI 辅助开发的简要说明

## 快速参考

| 用法 | 命令 |
|------|------|
| 基本搜索 | `seekit "query"` |
| JSON 输出 | `seekit -f json "query"` |
| 指定引擎 | `seekit -e searxng --searxng-url http://localhost:8080 "query"` |
| 限制结果 | `seekit -n 3 "query"` |
| 跳过缓存 | `seekit --no-cache "query"` |

## 许可证

MIT © 2026 seekit contributors

## 免责声明

本工具通过程序化方式访问 DuckDuckGo 的 HTML 接口。用户需自行遵守 DuckDuckGo 的服务条款和相关频率限制。本工具内置了 CAPTCHA 检测和指数退避机制，以尽可能减少对服务器的负担。
