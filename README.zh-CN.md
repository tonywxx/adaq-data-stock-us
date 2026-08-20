# adaq-data-stock-us

[`yfinance`](https://github.com/ranaroussi/yfinance) 的 **Rust** 重写实现，用于从 Yahoo Finance 获取美股（及全球）证券市场数据。

- **异步优先（async-first）**，同时提供功能完整的**同步（blocking）封装**，方便非异步场景直接调用。
- **强类型返回值**（编译期保障）；需要表格数据的用户可通过 `polars` 特性将历史数据转换为 `DataFrame`（可选特性），或通过 `serde` 转为 JSON（默认开启）。
- 使用 [`primp`](https://crates.io/crates/primp) 进行 **Chrome TLS 指纹伪装**，规避 Yahoo 限流；底层为共享的、线程安全的 HTTP 会话（Cookie 容器、crumb、同意流程处理、重试/退避）。
- **高度对齐 yfinance**：每个方法都对应 yfinance 的某个接口（`Ticker.info`、`Ticker.option_chain`、`download`、`screen`、`Search`、`Lookup`、`Calendars`、`AsyncWebSocket`、`Auth` ……）。详见[与 yfinance 的兼容性](#与-yfinance-的兼容性)。

📖 English docs / 英文文档：[README.md](README.md)

---

## 目录

- [功能特性总览](#功能特性总览)
- [环境依赖](#环境依赖)
- [安装](#安装)
- [快速上手](#快速上手)
- [配置说明](#配置说明)
- [使用指南](#使用指南)
  - [行情历史数据](#行情历史数据)
  - [标的标识符（代码 / MIC 对 / ISIN）](#标的标识符代码--mic-对--isin)
  - [行情报价、基本面与期权](#行情报价基本面与期权)
  - [分析与预测数据](#分析与预测数据)
  - [搜索、查询、板块与市场概览](#搜索查询板块与市场概览)
  - [条件选股（Screener）](#条件选股screener)
  - [个股新闻、财报日期与 ISIN](#个股新闻财报日期与-isin)
  - [实时行情流](#实时行情流)
  - [登录鉴权](#登录鉴权)
  - [批量下载](#批量下载)
- [特性开关（Feature Flags）](#特性开关feature-flags)
- [本地缓存](#本地缓存)
- [错误处理](#错误处理)
- [与 yfinance 的兼容性](#与-yfinance-的兼容性)
- [示例代码](#示例代码)
- [项目结构](#项目结构)
- [更新日志](#更新日志)
- [许可证](#许可证)

---

## 功能特性总览

yfinance 的全部阶段（P1–P4）均已实现，按模块归类如下：

**P1 — HTTP 核心、行情历史与批量下载**
- 类型化的行情 `History` / `Bar`，解析自 `v8/finance/chart` 接口。
- 周期（Interval）：`1m, 2m, 5m, 15m, 30m, 60m, 90m, 1h, 1d, 5d, 1wk, 1mo, 3mo`。
- 时间段 / 起止日期区间（period/start/end），盘前盘后数据（`prepost`）。
- 公司行为（Corporate Actions）：分红、拆股、资本利得（`actions`）。
- 分红/拆股价格复权 —— 自动复权（默认）或回退复权（back-adjust）。
- `keepna` 保留缺失 OHLC 的行；`repair` 进行拆分连续性的价格修复。
- 多标的**并发批量 `download`**，支持宽松（收集错误）模式。

**P2 — 报价、基本面、期权、分析**
- `info` 综合信息（常用字段已扁平化，完整原始 JSON 保留在 `Info::raw`）。
- `fast_info` 由行情历史派生的轻量子集（最新价、均线、52 周区间……）。
- `holders`（主要 / 机构 / 共同基金 / 内部人买入、交易、名册）。
- `sustainability`（ESG 评分）。
- `analyst_price_targets`、`recommendation_trend`、`recommendations`、`upgrades_downgrades`。
- 三大财务报表（`income` / `balance-sheet` / `cash-flow`）× 年度 / 季度。
- 完整 `option_chain`（到期日、行权价、看涨/看跌合约）。
- 预测表：`earnings_estimate`、`revenue_estimate`、`earnings_history`、
  `eps_trend`、`eps_revisions`、`growth_estimates`、`valuation_measures`。
- 个股 `calendar`、`sec_filings`、当前及完整 `shares` / `shares_full`、`funds_data`。
- 个股 `news`、`earnings_dates`、反向 `isin` 查询。

**P3 — 板块、搜索、日历、选股**
- `domain`：行业（sector）/ 子行业（industry）快照，以及按地区的市场概览。
- `search`（全文搜索）与 `lookup`（按类型：equity / etf / mutualfund / index / …）。
- 日历：**财报（earnings）**、**IPO**、**宏观经济（economic）**、**拆股（splits）**，按日期区间查询。
- `screener`：预置选股条件（`day_gainers`、`most_actives` 等）以及类型化查询 DSL
  （`EquityQuery` / `FundQuery` / `ETFQuery`）。

**P4 — 实时行情流与鉴权**
- **WebSocket 实时行情流**，解析 Yahoo 的 `PricingData` protobuf 数据（15 秒心跳保活）。
- `Auth`：注入 Yahoo `T`/`Y` 登录 Cookie，并查询订阅等级 / 用户信息。

---

## 环境依赖

| 工具 | 版本 |
|------|------|
| Rust | **≥ 1.85**（本 crate 使用 2024 edition） |
| Cargo | 随 Rust 工具链提供 |
| 网络 | 需可访问 `query1.finance.yahoo.com`、`query2.finance.yahoo.com` 及行情流服务（出站 HTTPS） |

无需任何系统级依赖 —— `sqlite`（`rusqlite` 已内置 bundled）与 TLS（`rustls`）均静态编译，**不需要 C 编译器**。

---

## 安装

在 `Cargo.toml` 中添加依赖。如需表格化输出，额外开启 `polars` 特性：

```toml
[dependencies]
adaq-data-stock-us = "0.1"
# 可选：为 History 提供 DataFrame 转换能力
adaq-data-stock-us = { version = "0.1", features = ["polars"] }
```

或使用 `cargo add`：

```sh
cargo add adaq-data-stock-us
cargo add adaq-data-stock-us --features polars   # 可选
```

本 crate 以**库（library）**形式使用。可运行的演示位于 `src/main.rs`（标的标识符解析）以及 `examples/` 目录（详见[示例代码](#示例代码)）。

---

## 快速上手

### 同步 API（无需 `async`）

```rust,no_run
use adaq_data_stock_us::blocking::Client;
use adaq_data_stock_us::history::{HistoryOptions, Interval};

fn main() -> adaq_data_stock_us::Result<()> {
    let client = Client::new()?;

    // 行情历史（默认：1d 周期、最近 1 个月、自动复权）
    let opts = HistoryOptions {
        interval: Interval::Day1,
        period: "5d".into(),
        ..Default::default()
    };
    let hist = client.history("AAPL", &opts)?;
    println!("AAPL bars: {}", hist.bars.len());

    // 快速报价
    let info = client.info("AAPL")?;
    println!("{}  市值: {:?}", info.short_name.unwrap_or_default(), info.market_cap);
    Ok(())
}
```

### 异步 API

```rust,no_run
use adaq_data_stock_us::{Client, HistoryOptions, Interval};

#[tokio::main]
async fn main() -> adaq_data_stock_us::Result<()> {
    let client = Client::new()?;
    let hist = client
        .history("AAPL", &HistoryOptions { interval: Interval::Day1, period: "1mo".into(), ..Default::default() })
        .await?;
    println!("bars: {}", hist.bars.len());
    Ok(())
}
```

> **提示：** 两种 API 暴露**完全相同**的方法集。同步客户端在底层通过共享的多线程 Tokio 运行时执行异步调用，可依据自身应用场景自由选择。

---

## 配置说明

通过 `Config` 与 `Client::with_config`（或 `blocking::Client::with_config`）调整 HTTP 会话：

```rust,no_run
use adaq_data_stock_us::{Client, Config};
use std::path::PathBuf;

fn main() -> adaq_data_stock_us::Result<()> {
    let config = Config::default()
        .proxy("http://127.0.0.1:7890")   // 可选 HTTP 代理
        .retries(3)                        // 瞬时失败时的重试次数
        .timeout_secs(45)                  // 单请求超时（秒）
        .locale("en", "US")                // summary/visualization 请求的本地化
        .lenient(true)                     // 批量下载：收集错误而非整体中断
        .cache_dir(PathBuf::from("./cache")) // 本地缓存目录
        // .cookies("<T>", "<Y>")          // 注入 Yahoo 登录 Cookie
        ;

    let client = Client::with_config(config)?;
    let _ = client;
    Ok(())
}
```

| 字段 | 默认值 | 说明 |
|------|--------|------|
| `proxy` | `None` | 可选 HTTP 代理 URL。 |
| `retries` | `0` | 瞬时失败时的重试次数。 |
| `timeout_secs` | `30` | 单请求超时时间（秒）。 |
| `user_agent` | Chrome UA | 每个请求携带的 User-Agent。 |
| `locale` | `en` / `US` | `quoteSummary` / visualization 请求的本地化。 |
| `lenient` | `true` | 批量[`download`](#批量下载)吞掉单标的错误（对应 yfinance 的 `hide_exceptions`）。 |
| `cache_dir` | 临时目录 | sqlite 缓存文件所在目录。 |
| `cookie_t` / `cookie_y` | `None` | Yahoo `T` / `Y` 登录 Cookie（见[登录鉴权](#登录鉴权)）。 |

---

## 使用指南

### 行情历史数据

```rust,no_run
use adaq_data_stock_us::blocking::Client;
use adaq_data_stock_us::history::{HistoryOptions, Interval};

fn main() -> adaq_data_stock_us::Result<()> {
    let client = Client::new()?;

    let opts = HistoryOptions {
        interval: Interval::Day1,
        period: "6mo".into(),
        actions: true,        // 包含分红 / 拆股 / 资本利得
        auto_adjust: true,    // 按分红与拆股自动复权
        repair: true,         // 拆分连续性价格修复
        ..Default::default()
    };
    let h = client.history("AAPL", &opts)?;

    for bar in h.bars.iter().take(3) {
        println!("{}  O={:?} H={:?} L={:?} C={:?} V={:?}",
            bar.datetime, bar.open, bar.high, bar.low, bar.close, bar.volume);
    }
    if let Some(meta) = Some(&h.meta) {
        println!("currency={:?} exchange={:?}", meta.currency, meta.exchange);
    }
    if let Some(actions) = &h.actions {
        println!("dividends={}, splits={}", actions.dividends.len(), actions.splits.len());
    }
    Ok(())
}
```

- **周期（Interval）** 为 `Interval` 枚举（`Min1` … `Month3`）。分钟级周期（如 `1m`/`2m`/…）受 Yahoo 区间限制。
- **`actions: true`** 会附带 `History::actions`（分红、拆股、资本利得）。
- **`auto_adjust`**（默认）缩放 OHLC 并保留原始收盘价；**`back_adjust`** 保留复权收盘价（Adj Close）并缩放其余字段；两者均设为 `false` 则为原始价格。
- **`repair: true`** 丢弃非正的 OHLC 行，并依据声明拆股事件使序列在拆股点连续（对应 yfinance 的 `repair=True`）。

### 标的标识符（代码 / MIC 对 / ISIN）

一个证券可用裸代码、`(symbol, MIC)` 对或 ISIN 表示 —— 对应 yfinance `Ticker` 构造函数的多种形式。

```rust,no_run
use adaq_data_stock_us::blocking::Client;
use adaq_data_stock_us::TickerId;

fn main() -> adaq_data_stock_us::Result<()> {
    let client = Client::new()?;

    // 裸代码
    let aapl = client.ticker("AAPL");
    println!("{}", aapl.symbol());

    // (symbol, MIC) 对 -> "OR.PA"
    let or = client.ticker_from_mic("OR", "XPAR")?;
    println!("{}", or.symbol());

    // ISIN -> "AAPL"  (US0378331005)
    let by_isin = client.ticker_from_isin("US0378331005")?;
    println!("{}", by_isin.symbol());

    // 任意标识符
    let _ = client.ticker_from_id(TickerId::Symbol("MSFT".into()))?;
    Ok(())
}
```

`(symbol, MIC)` 对通过 `MIC_TO_YAHOO_SUFFIX` 映射解析（如 `("OR", "XPAR") → "OR.PA"`；美股交易所 `XNYS`/`XNAS` 不带后缀）。ISIN→代码解析对应 `utils.get_ticker_by_isin`，结果会被缓存。

### 行情报价、基本面与期权

```rust,no_run
use adaq_data_stock_us::blocking::Client;
use adaq_data_stock_us::fundamentals::{Freq, Statement};

fn main() -> adaq_data_stock_us::Result<()> {
    let client = Client::new()?;

    // 综合信息（常用字段 + 完整原始 JSON 保存在 Info::raw）
    let info = client.info("AAPL")?;
    println!("sector={:?} industry={:?} pe={:?}", info.sector, info.industry, info.trailing_pe);

    // 由行情历史派生的轻量 fast_info
    let fi = client.fast_info("AAPL")?;
    println!("last={:?} 50d avg={:?}", fi.last_price, fi.fifty_day_average);

    // 股东（主要 / 机构 / 共同基金 / 内部人）
    let holders = client.holders("AAPL")?;
    println!("major holders: {}", holders.major.len());

    // 三大财务报表，支持年度或季度
    let fin = client.financials("AAPL", Statement::Income, Freq::Annual)?;
    println!("dates={:?}", fin.dates);
    if let Some(first) = fin.dates.first() {
        println!("totalRevenue @ {} = {:?}", first, fin.get("totalRevenue", first));
    }

    // 期权链
    let chain = client.option_chain("AAPL")?;
    println!("expirations={}, first={:?}", chain.expirations.len(), chain.expirations.first());
    Ok(())
}
```

### 分析与预测数据

```rust,no_run
use adaq_data_stock_us::blocking::Client;

fn main() -> adaq_data_stock_us::Result<()> {
    let client = Client::new()?;
    let ticker = "AAPL";

    let eps_est = client.earnings_estimate(ticker)?;     // NamedTable
    println!("earnings estimate rows: {:?}", eps_est.index);

    let targets = client.analyst_price_targets(ticker)?; // current/low/high/mean/median
    println!("analyst mean target: {:?}", targets.mean);

    let recs = client.recommendation_trend(ticker)?;     // 各周期 strongBuy..strongSell
    println!("recommendation periods: {}", recs.len());

    let changes = client.upgrades_downgrades(ticker)?;   // 评级变动历史
    println!("rating changes: {}", changes.len());

    let vals = client.valuation_measures(ticker)?;       // 市值、P/E、PEG、P/B、EV/EBITDA…
    println!("valuation rows: {:?}", vals.index);

    let cal = client.calendar(ticker)?;                  // 财报与分红日期
    println!("next earnings: {:?}", cal.earnings_date);

    let filings = client.sec_filings(ticker)?;           // SEC 文件
    println!("sec filings: {}", filings.len());

    let shares = client.shares(ticker)?;                 // 当前总股本
    println!("shares outstanding: {:?}", shares);

    let shares_full = client.shares_full(ticker, None, None)?; // 完整时间序列
    println!("shares time-series points: {}", shares_full.len());
    Ok(())
}
```

`NamedTable`（各预测方法返回）为带标签的矩阵，形状为 `index`（行）× `columns`（列），并提供 `get(row, col)` 查询，对应 yfinance 返回的 DataFrame 形态。

### 搜索、查询、板块与市场概览

```rust,no_run
use adaq_data_stock_us::blocking::Client;
use adaq_data_stock_us::domain::MarketRegion;

fn main() -> adaq_data_stock_us::Result<()> {
    let client = Client::new()?;

    // 全文搜索
    let s = client.search("apple", 5, 0)?;
    for q in &s.quotes { println!("{}  {}", q.symbol.unwrap_or_default(), q.short_name.unwrap_or_default()); }

    // 按类型查询（"equity"、"etf"、"mutualfund"、"index"、"cryptocurrency"…）
    let l = client.lookup("tesla", 5, "equity")?;
    println!("lookup results: {}", l.results.len());

    // 板块快照
    let sector = client.sector("technology")?;
    println!("sector: {:?}  top companies: {}", sector.name, sector.top_companies.len());
    let market = client.market(MarketRegion::Us)?;
    println!("US market rows: {}, status: {:?}", market.summary.len(), market.status);

    // 日历（按两个 YYYY-MM-DD 日期区间）
    let earn  = client.earnings_calendar("2026-08-01", "2026-08-15", 25)?;
    let ipo   = client.ipo_calendar("2026-08-01", "2026-08-15", 25)?;
    let econ  = client.economic_calendar("2026-08-01", "2026-08-15", 25)?;
    let split = client.splits_calendar("2026-08-01", "2026-08-15", 25)?;
    println!("earnings={} ipo={} economic={} splits={}", earn.len(), ipo.len(), econ.len(), split.len());
    Ok(())
}
```

### 条件选股（Screener）

既可按名称调用预置选股条件，也可用类型化 DSL 构建自定义查询。

```rust,no_run
use adaq_data_stock_us::blocking::Client;
use adaq_data_stock_us::screener::{Operand, Operator, Query, ScreenOptions};

fn main() -> adaq_data_stock_us::Result<()> {
    let client = Client::new()?;

    // 预置选股条件
    let res = client.screen("day_gainers", &ScreenOptions::default())?;
    println!("day_gainers total={:?} returned={}", res.total, res.quotes.len());

    // 自定义选股：percentchange > 3 且 region = us
    let q = Query::equity(Operator::And, vec![
        Operand::query(Query::equity(Operator::Gt, vec![
            Operand::field("percentchange"), Operand::value(3.0),
        ])),
        Operand::query(Query::equity(Operator::Eq, vec![
            Operand::field("region"), Operand::value("us"),
        ])),
    ]);
    let res = client.screen(q, &ScreenOptions::custom_defaults())?;
    for q in res.quotes.iter().take(5) {
        println!("{}  {:?}", q.symbol.unwrap_or_default(), q.percent_change);
    }
    Ok(())
}
```

可选预置选股条件包括 `day_gainers`、`day_losers`、`most_actives`、`aggressive_small_caps`、
`growth_technology_stocks`、`most_shorted_stocks` 等（内置于 yfinance 1.6.0）。

### 个股新闻、财报日期与 ISIN

```rust,no_run
use adaq_data_stock_us::blocking::Client;

fn main() -> adaq_data_stock_us::Result<()> {
    let client = Client::new()?;

    // 新闻。`tab` 为 "news"（默认）、"all" 或 "press releases"。
    let news = client.news("AAPL", 5, "news")?;
    for a in news.iter().take(3) {
        println!("{} — {}", a.title.unwrap_or_default(), a.publisher.unwrap_or_default());
        if let Some(url) = a.thumbnail_url() { println!("  thumb: {url}"); }
    }

    // 财报日期（最新在前；上限 100）
    let dates = client.earnings_dates("AAPL", 20)?;
    for d in dates.iter().take(3) {
        println!("earnings on {:?}  eps_est={:?} eps_actual={:?}",
            d.date, d.eps_estimate, d.eps_actual);
    }

    // 反向查询：代码 -> ISIN
    let isin = client.isin("AAPL")?;
    println!("AAPL ISIN: {isin}");
    Ok(())
}
```

### 实时行情流

```rust,no_run
use adaq_data_stock_us::{Client, LiveWebSocket, PricingData};

#[tokio::main]
async fn main() -> adaq_data_stock_us::Result<()> {
    let _client = Client::new()?;

    let ws = LiveWebSocket::new().verbose(true);
    ws.stream(&["AAPL", "MSFT"], |tick: PricingData| {
        println!("{} price={:.2} chg%={:.2} vol={}",
            tick.id, tick.price, tick.change_percent, tick.day_volume);
    }).await?;
    Ok(())
}
```

对于同步调用方，`blocking::Client::stream_live(symbols, handler)` 会在共享运行时上执行相同的行情流直至结束。

### 登录鉴权

```rust,no_run
use adaq_data_stock_us::Client;

#[tokio::main]
async fn main() -> adaq_data_stock_us::Result<()> {
    let client = Client::new()?;
    let auth = client.auth();

    // 注入 Yahoo T/Y 登录 Cookie（取自浏览器开发者工具）
    let ok = auth.set_login_cookies("<T-cookie>", "<Y-cookie>").await?;
    println!("cookies accepted: {ok}");

    println!("logged in: {}", auth.check_login().await?);
    println!("tier: {:?}", auth.subscription_tier().await?);
    println!("user guid: {:?}", auth.user().await?);
    Ok(())
}
```

### 批量下载

并发下载多个标的的历史数据。在宽松模式（默认）下，单个标的失败会被收集到 `errors` 中，而不会中断整个批次。

```rust,no_run
use adaq_data_stock_us::blocking::Client;
use adaq_data_stock_us::history::{HistoryOptions, Interval};

fn main() -> adaq_data_stock_us::Result<()> {
    let client = Client::new()?;
    let opts = HistoryOptions { interval: Interval::Day1, period: "1mo".into(), ..Default::default() };

    let result = client.download(&["AAPL", "MSFT", "NVDA"], &opts)?;
    println!("fetched: {}  errors: {}", result.histories.len(), result.errors.len());
    for (sym, err) in &result.errors {
        println!("  failed {sym}: {err}");
    }
    Ok(())
}
```

---

## 特性开关（Feature Flags）

| 特性 | 默认 | 作用 |
|------|------|------|
| `polars` | 关闭 | 新增 `History::to_polars()` → `polars::prelude::DataFrame`，便于表格化消费。 |

```toml
adaq-data-stock-us = { version = "0.1", features = ["polars"] }
```

```rust,ignore
let df = hist.to_polars()?;   // 需要开启 `polars` 特性
```

---

## 本地缓存

单个本地 **sqlite** 文件（`adaq-yfinance.db`）缓存 crumb、各标的时区以及 ISIN→代码映射。默认位于临时目录；可通过 `Config::cache_dir` 设置持久位置：

```rust,no_run
use adaq_data_stock_us::{Client, Config};
use std::path::PathBuf;

fn main() -> adaq_data_stock_us::Result<()> {
    let cfg = Config::default().cache_dir(PathBuf::from("./.adaq-cache"));
    let _client = Client::with_config(cfg)?;
    Ok(())
}
```

---

## 错误处理

所有 fallible 调用均返回 `adaq_data_stock_us::Result<T>` =
`Result<T, YfError>`。`YfError` 分类对应 yfinance 的异常体系：

| 变体 | 含义 |
|------|------|
| `Http` | HTTP 客户端的网络/传输失败。 |
| `Status { status, body }` | Yahoo 返回了非成功状态的 HTTP 状态码。 |
| `RateLimited` | Yahoo 对请求进行了限流（HTTP 429）。 |
| `Parse` | JSON 序列化/反序列化失败。 |
| `TickerMissing` | 标的无法找到 / 解析。 |
| `InvalidPeriod` | period / interval / range 组合非法。 |
| `DataMissing` | 响应中缺少期望的数据。 |
| `NotSupported` | 功能尚未实现 / 不支持。 |
| `Cache` | 本地 sqlite 缓存失败。 |
| `Io` | 文件系统 / IO 失败。 |
| `Msg` | 通用错误信息。 |

```rust,no_run
use adaq_data_stock_us::blocking::Client;
use adaq_data_stock_us::history::{HistoryOptions, Interval};

fn main() {
    let client = Client::new().expect("client build failed");
    match client.history("AAPL", &HistoryOptions { interval: Interval::Day1, period: "1mo".into(), ..Default::default() }) {
        Ok(h) => println!("bars: {}", h.bars.len()),
        Err(e) => eprintln!("request failed: {e}"),
    }
}
```

---

## 与 yfinance 的兼容性

本 crate 跟踪 **yfinance 1.6.0**（锁定在 `vendor/yfinance` 的 commit `93eb4c2`；见 `PARITY_PIN`）。各模块状态记录于 [`docs/PARITY.md`](docs/PARITY.md)，对齐机制见 [`docs/adr/0003-parity-mechanism.md`](docs/adr/0003-parity-mechanism.md)。

- **阶段 P1–P4：`done`** —— HTTP 核心、行情、下载、报价、基本面、期权、分析、板块、搜索、查询、日历、选股、实时、鉴权。
- **价格修复（price-repair）：`partial`** —— 已实现拆分连续性修复（丢弃非正行 + 按拆股因子缩放拆股前各行）；上游完整的多接口对账尚未复现。

可针对 vendored 子模块运行兼容性漂移检查：

```sh
cargo xtask parity
```

---

## 示例代码

可运行示例位于 `examples/` 目录（以及 `src/main.rs`）：

| 示例 | 覆盖内容 |
|------|----------|
| `main.rs` | 标的标识符解析（代码 / MIC / ISIN）。 |
| `quick` | 通过同步 API 获取行情历史与公司行为。 |
| `smoke` | 行情、`info`、`fast_info`、财务报表、期权链。 |
| `p3` | 搜索、查询、板块、日历、选股。 |
| `p4` | 实时 WebSocket 行情流 + 登录鉴权。 |
| `ticker_id` | MIC/ISIN 解析 + 个股新闻 / 财报 / 反向 ISIN。 |

运行方式：

```sh
cargo run --example quick
cargo run --example smoke
cargo run --example p3
cargo run --example p4
cargo run --example ticker_id
cargo run                       # 运行 src/main.rs
```

---

## 项目结构

```
src/
  lib.rs          API 公共导出（re-export）
  client.rs       Client 与 Ticker 句柄、download（异步）
  blocking.rs     异步 Client/Ticker 的同步封装
  http.rs         YfSession：Cookie、crumb、同意流程、重试
  config.rs       Config 与构建器
  cache.rs        sqlite 缓存（crumb / tz / isin）
  error.rs        YfError 分类
  history.rs      History / Bar / HistoryOptions / Interval / 公司行为 / 修复
  quote.rs        info、fast_info、holders、分析、预测、估值、日历、SEC、基金、股本
  fundamentals.rs 利润表 / 资产负债表 / 现金流量表
  options.rs      期权链
  news.rs         个股新闻
  earnings.rs     个股财报日期
  isin.rs         ISIN <-> 代码解析
  mic.rs          MIC -> Yahoo 后缀映射
  search.rs       全文搜索
  lookup.rs       类型化查询
  domain.rs       行业 / 子行业 / 市场
  calendars.rs    财报 / IPO / 宏观 / 拆股
  screener.rs     预置与自定义查询 DSL
  live.rs         WebSocket 实时行情流（PricingData protobuf）
  auth.rs         登录 Cookie 与订阅等级
docs/
  PARITY.md       yfinance 各模块兼容状态
  adr/            架构决策记录
  agents/         Agent 协作文档
vendor/yfinance/  vendored yfinance 子模块（已锁定版本）
xtask/            cargo xtask 兼容性漂移检查工具
```

---

## 更新日志

完整的双语（English / 简体中文）发布历史见 [CHANGELOG.md](CHANGELOG.md)。

## 许可证

基于 [Apache License 2.0](LICENSE) 许可证发布。
