# Changelog / 更新日志

All notable changes to this project are documented here. This file follows
[Keep a Changelog](https://keepachangelog.com/) conventions and is bilingual
(English / 简体中文).

本文件记录项目的所有重要变更，遵循
[Keep a Changelog](https://keepachangelog.com/) 规范，内容采用中英双语
（English / 简体中文）。

---

## [0.1.2] — 2026-08-19

### Added / 新增
- **EN** Add a CI workflow that builds and tests the project on push and pull
  requests, renamed from "Rust" to "CI".
- **中文** 新增 CI 工作流，在 push 与 pull request 时自动构建并运行测试；
  工作流名称由 "Rust" 变更为 "CI"。

- **EN** Introduce an injectable `Transport` seam in `YfSession` (production
  `PrimpTransport` + an offline `MockTransport`) so the session's crumb,
  locale, consent, and retry/backoff glue can be exercised without network
  access. `YfSession` keeps all policy; the seam owns only "send → status +
  body".
- **中文** 为 `YfSession` 引入可注入的 `Transport` 网络抽象（生产用
  `PrimpTransport`，离线测试用 `MockTransport`），使会话的 crumb、locale、
  consent 与重试/退避逻辑可在无网络环境下验证。`YfSession` 仍掌控全部策略，
  抽象层只负责"发送请求 → 返回状态码与响应体"。

- **EN** Add offline unit tests for `YfSession` covering crumb injection,
  locale append, 5xx retry, 401 crumb re-fetch (`reset_auth`), terminal
  429/404, `get_text`, `post_json`, crumb caching, and login/entitlement
  inspection.
- **中文** 新增 `YfSession` 离线单元测试，覆盖 crumb 注入、locale 追加、5xx
  重试、401 后重新获取 crumb（`reset_auth`）、终态 429/404、`get_text`、
  `post_json`、crumb 缓存，以及登录/权益校验。

### Changed / 变更
- **EN** `YfSession` now routes every network call through the `Transport`
  trait; the production `PrimpTransport` wraps the existing `primp` client.
  Added the `async-trait` dependency to support trait objects.
- **中文** `YfSession` 的所有网络调用现统一经由 `Transport` 抽象；生产实现
  `PrimpTransport` 封装既有 `primp` 客户端，并新增 `async-trait` 依赖以支持
  trait 对象。

- **EN** `README` now links to and documents the `CHANGELOG.md` release history.
- **中文** `README` 现新增指向并说明 `CHANGELOG.md` 的目录链接与章节。

### Fixed / 修复
- **EN** `get_json` now re-fetches the crumb after a 401 (`reset_auth`) instead
  of reusing the invalidated crumb on the retry, matching yfinance behavior.
- **中文** `get_json` 在 401（`reset_auth`）后重新获取 crumb，而非在重试时复用
  已失效的 crumb，与 yfinance 行为保持一致。

---

## [0.1.1] — 2026-08-15

### Added / 新增
- **EN** `YfSession`: a shared, thread-safe HTTP session handling Yahoo cookies,
  crumb, and consent.
- **中文** `YfSession`：共享、线程安全的 HTTP 会话，负责 Yahoo cookie、crumb
  及用户授权（consent）处理。

- **EN** Retry/backoff logic for HTTP requests in `YfSession` to improve
  resilience against transient failures.
- **中文** 在 `YfSession` 中新增请求重试与退避（retry/backoff）逻辑，提升对
  瞬时失败的容错能力。

### Changed / 变更
- **EN** Refactored raw JSON accessors in `fundamentals.rs` to delegate to the
  shared `json` module helpers.
- **中文** 重构 `fundamentals.rs` 中的原始 JSON 访问器，改用统一的 `json` 模块
  辅助函数。

- **EN** Updated dependencies and improved overall code quality.
- **中文** 升级依赖版本，并改进整体代码质量。

---

## [0.1.0] — 2026-08-15

### Added / 新增
- **EN** Initial release of the yfinance-compatible Rust crate, implementing the
  full yfinance 1.6.0 surface across phases P1–P4:
  - HTTP core, price `History` (intervals, periods, corporate actions, dividend/
    split adjustment, price repair).
  - Concurrent bulk `download` with lenient (collect-errors) mode.
  - Quote, fundamentals, options, analysis & estimates.
  - Domain snapshots, free-text `search`, typed `lookup`, and earnings / IPO /
    economic / splits calendars.
  - `screener` with predefined screens and a typed query-builder DSL.
  - Live WebSocket streaming and Yahoo authentication.
- **中文** 首个正式版本，发布与 yfinance 1.6.0 兼容的 Rust 库，覆盖 P1–P4 全部
  接口：
  - HTTP 核心、价格 `History`（多种周期、区间、公司行动、除权/拆股价格调整、
    价格修复）。
  - 并发批量 `download`，支持宽容（收集错误而非中断）模式。
  - 报价、基本面、期权、分析与预期数据。
  - 行业/板块快照、全文 `search`、类型化 `lookup`，以及财报 / IPO / 经济 /
    拆股日历。
  - `screener` 预置筛选器与类型化查询构造 DSL。
  - 实时 WebSocket 行情流与 Yahoo 登录认证。

- **EN** Blocking façade (`blocking::Client` / `blocking::Ticker`) over the
  async core, plus an async `Client`.
- **中文** 在异步内核之上提供同步门面（`blocking::Client` / `blocking::Ticker`）
  以及异步 `Client`。

- **EN** Ticker identifier resolution by symbol, `(symbol, MIC)` pair, and ISIN,
  including reverse ticker→ISIN lookup.
- **中文** 支持以代码、`(symbol, MIC)` 组合与 ISIN 三种方式解析标的信息，
  并支持由代码反查 ISIN。

- **EN** Per-ticker news, earnings dates, and `YfError` taxonomy mirroring the
  yfinance exception hierarchy.
- **中文** 单标的新闻、财报日期，以及对应 yfinance 异常体系的 `YfError` 错误
  分类。

- **EN** On-disk sqlite cache for crumb / timezone / ISIN mappings.
- **中文** 基于本地 sqlite 的缓存，保存 crumb / 时区 / ISIN 映射。

- **EN** Optional `polars` feature converting `History` to a `DataFrame`.
- **中文** 可选 `polars` 特性，支持将 `History` 转换为 `DataFrame`。

- **EN** GitHub Actions workflow for building and testing the project.
- **中文** 新增 GitHub Actions 工作流，用于项目构建与测试。

---

[Unreleased]: https://github.com/tonywxx/adaq-data-stock-us/compare/v0.1.1...HEAD
[0.1.1]: https://github.com/tonywxx/adaq-data-stock-us/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/tonywxx/adaq-data-stock-us/releases/tag/v0.1.0
