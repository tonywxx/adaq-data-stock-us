# adaq-data-stock-us

A **Rust** reimplementation of Python's [`yfinance`](https://github.com/ranaroussi/yfinance)
for fetching US (and global) equity market data from Yahoo Finance.

- **Async-first**, with a fully-featured **blocking facade** for non-async callers.
- **Strongly-typed** return values (compile-time guarantees); tabular consumers can
  convert history to a [`polars`](https://crates.io/crates/polars) `DataFrame` (optional
  `polars` feature) or to JSON via `serde` (always on).
- **Chrome TLS impersonation** via [`primp`](https://crates.io/crates/primp) to avoid
  Yahoo rate-limiting, with a shared, thread-safe HTTP session (cookie jar, crumb,
  consent handling, retry/backoff).
- **Faithful yfinance parity**: every method mirrors a yfinance surface (`Ticker.info`,
  `Ticker.option_chain`, `download`, `screen`, `Search`, `Lookup`, `Calendars`,
  `AsyncWebSocket`, `Auth`, …). See [Parity](#parity-with-yfinance).

📖 中文文档 / Chinese docs: [README.zh-CN.md](README.zh-CN.md)

---

## Table of Contents

- [Feature Overview](#feature-overview)
- [Requirements](#requirements)
- [Installation](#installation)
- [Quick Start](#quick-start)
- [Configuration](#configuration)
- [Usage Guide](#usage-guide)
  - [Price History](#price-history)
  - [Ticker Identifiers (symbol / MIC pair / ISIN)](#ticker-identifiers-symbol--mic-pair--isin)
  - [Quote, Fundamentals & Options](#quote-fundamentals--options)
  - [Analysis & Estimates](#analysis--estimates)
  - [Search, Lookup, Domain & Calendars](#search-lookup-domain--calendars)
  - [Screener](#screener)
  - [Per-Ticker News, Earnings Dates & ISIN](#per-ticker-news-earnings-dates--isin)
  - [Live Streaming](#live-streaming)
  - [Authentication](#authentication)
  - [Bulk Download](#bulk-download)
- [Feature Flags](#feature-flags)
- [Caching](#caching)
- [Error Handling](#error-handling)
- [Parity with yfinance](#parity-with-yfinance)
- [Examples](#examples)
- [Project Layout](#project-layout)
- [Changelog](#changelog)
- [License](#license)

---

## Feature Overview

All phases (P1–P4) of the yfinance surface are implemented. Grouped by module:

**P1 — HTTP core, history & bulk download**
- Typed price `History` / `Bar` parsed from the `v8/finance/chart` endpoint.
- Intervals: `1m, 2m, 5m, 15m, 30m, 60m, 90m, 1h, 1d, 5d, 1wk, 1mo, 3mo`.
- Period/start/end ranges, pre/post-market bars (`prepost`).
- Corporate actions: dividends, stock splits, and capital gains (`actions`).
- Dividend/split price adjustment — auto-adjust (default) or back-adjust.
- `keepna` to retain rows with missing OHLC; `repair` for split-continuous price repair.
- Concurrent **bulk `download`** of many tickers, with lenient (collect-errors) mode.

**P2 — Quote, fundamentals, options, analysis**
- `info` blob (flattened common fields + full raw JSON preserved on `Info::raw`).
- `fast_info` subset derived from price history (last price, averages, 52-week range…).
- `holders` (major / institutional / mutual-fund / insider purchases, transactions, roster).
- `sustainability` (ESG scores).
- `analyst_price_targets`, `recommendation_trend`, `recommendations`, `upgrades_downgrades`.
- Three financial statements (`income` / `balance-sheet` / `cash-flow`) × annual / quarterly.
- Full `option_chain` (expirations, strikes, calls & puts).
- Estimates tables: `earnings_estimate`, `revenue_estimate`, `earnings_history`,
  `eps_trend`, `eps_revisions`, `growth_estimates`, `valuation_measures`.
- Per-ticker `calendar`, `sec_filings`, current & full `shares` / `shares_full`, `funds_data`.
- Per-ticker `news`, `earnings_dates`, reverse `isin`.

**P3 — Domain, search, calendars, screener**
- `domain`: sector / industry snapshots and market summaries by region.
- `search` (free-text) and `lookup` (by type: equity / etf / mutualfund / index / …).
- Calendars: **earnings**, **IPO**, **economic**, and **splits** between two dates.
- `screener`: predefined screens (`day_gainers`, `most_actives`, …) plus a typed
  query-builder DSL (`EquityQuery` / `FundQuery` / `ETFQuery`).

**P4 — Live streaming & auth**
- **WebSocket live stream** decoding Yahoo's `PricingData` protobuf feed (15s heartbeat).
- `Auth`: inject Yahoo `T`/`Y` login cookies and inspect subscription tier / user.

---

## Requirements

| Tool | Version |
|------|---------|
| Rust | **≥ 1.85** (the crate uses the 2024 edition) |
| Cargo | ships with the Rust toolchain |
| Network | outbound HTTPS to `query1.finance.yahoo.com`, `query2.finance.yahoo.com`, and the streamer |

No system libraries are required — `sqlite` (`rusqlite` bundled) and TLS (`rustls`) are
compiled in. A C compiler is **not** needed.

---

## Installation

Add the crate to your `Cargo.toml`. For tabular output also enable the `polars` feature:

```toml
[dependencies]
adaq-data-stock-us = "0.1"
# optional: DataFrame conversion for History
adaq-data-stock-us = { version = "0.1", features = ["polars"] }
```

Or with `cargo add`:

```sh
cargo add adaq-data-stock-us
cargo add adaq-data-stock-us --features polars   # optional
```

The crate is used as a **library**. A small runnable demo lives in `src/main.rs`
(ticker-identifier resolution) and the `examples/` directory (see [Examples](#examples)).

---

## Quick Start

### Blocking API (no `async` needed)

```rust,no_run
use adaq_data_stock_us::blocking::Client;
use adaq_data_stock_us::history::{HistoryOptions, Interval};

fn main() -> adaq_data_stock_us::Result<()> {
    let client = Client::new()?;

    // Price history (default: 1d interval, last 1 month, auto-adjusted)
    let opts = HistoryOptions {
        interval: Interval::Day1,
        period: "5d".into(),
        ..Default::default()
    };
    let hist = client.history("AAPL", &opts)?;
    println!("AAPL bars: {}", hist.bars.len());

    // Quick quote
    let info = client.info("AAPL")?;
    println!("{}  market cap: {:?}", info.short_name.unwrap_or_default(), info.market_cap);
    Ok(())
}
```

### Async API

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

> **Tip:** both APIs expose the *same* method set. The blocking client internally runs
> the async call on a shared multi-thread Tokio runtime, so you can pick whichever fits
> your application.

---

## Configuration

Tune the HTTP session via `Config` and `Client::with_config` (or `blocking::Client::with_config`):

```rust,no_run
use adaq_data_stock_us::{Client, Config};
use std::path::PathBuf;

fn main() -> adaq_data_stock_us::Result<()> {
    let config = Config::default()
        .proxy("http://127.0.0.1:7890")   // optional HTTP proxy
        .retries(3)                        // retries on transient failures
        .timeout_secs(45)                  // per-request timeout
        .locale("en", "US")                // locale for summary/visualization requests
        .lenient(true)                     // bulk download: collect errors instead of aborting
        .cache_dir(PathBuf::from("./cache")) // on-disk cache location
        // .cookies("<T>", "<Y>")          // inject Yahoo login cookies
        ;

    let client = Client::with_config(config)?;
    let _ = client;
    Ok(())
}
```

| Field | Default | Description |
|-------|---------|-------------|
| `proxy` | `None` | Optional HTTP proxy URL. |
| `retries` | `0` | Retry count on transient failures. |
| `timeout_secs` | `30` | Per-request timeout in seconds. |
| `user_agent` | Chrome UA | User-Agent sent with every request. |
| `locale` | `en` / `US` | Locale for `quoteSummary` / visualization requests. |
| `lenient` | `true` | Bulk [`download`](#bulk-download) swallows per-ticker errors (mirrors yfinance `hide_exceptions`). |
| `cache_dir` | temp dir | Directory for the sqlite cache file. |
| `cookie_t` / `cookie_y` | `None` | Yahoo `T` / `Y` login cookies (see [Authentication](#authentication)). |

---

## Usage Guide

### Price History

```rust,no_run
use adaq_data_stock_us::blocking::Client;
use adaq_data_stock_us::history::{HistoryOptions, Interval};

fn main() -> adaq_data_stock_us::Result<()> {
    let client = Client::new()?;

    let opts = HistoryOptions {
        interval: Interval::Day1,
        period: "6mo".into(),
        actions: true,        // include dividends / splits / capital gains
        auto_adjust: true,    // auto-adjust OHLC by dividends & splits
        repair: true,         // split-continuous price repair
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

- **Intervals** are the `Interval` enum (`Min1` … `Month3`). Intraday intervals
  (`1m`/`2m`/…) are subject to Yahoo's range limits.
- **`actions: true`** attaches `History::actions` (dividends, splits, capital gains).
- **`auto_adjust`** (default) rescales OHLC and keeps raw close; **`back_adjust`**
  keeps Adj Close and rescales the rest; set both `false` for raw prices.
- **`repair: true`** drops non-positive OHLC bars and makes the series split-continuous
  using declared split events (mirrors yfinance `repair=True`).

### Ticker Identifiers (symbol / MIC pair / ISIN)

A security can be addressed by a bare symbol, a `(symbol, MIC)` pair, or an ISIN —
mirroring yfinance's `Ticker` constructor forms.

```rust,no_run
use adaq_data_stock_us::blocking::Client;
use adaq_data_stock_us::TickerId;

fn main() -> adaq_data_stock_us::Result<()> {
    let client = Client::new()?;

    // Bare symbol
    let aapl = client.ticker("AAPL");
    println!("{}", aapl.symbol());

    // (symbol, MIC) pair -> "OR.PA"
    let or = client.ticker_from_mic("OR", "XPAR")?;
    println!("{}", or.symbol());

    // ISIN -> "AAPL"  (US0378331005)
    let by_isin = client.ticker_from_isin("US0378331005")?;
    println!("{}", by_isin.symbol());

    // Any identifier
    let _ = client.ticker_from_id(TickerId::Symbol("MSFT".into()))?;
    Ok(())
}
```

The `(symbol, MIC)` pair is resolved through the `MIC_TO_YAHOO_SUFFIX` map (e.g.
`("OR", "XPAR") → "OR.PA"`; US exchanges `XNYS`/`XNAS` carry no suffix). ISIN→ticker
resolution mirrors `utils.get_ticker_by_isin` and is cached.

### Quote, Fundamentals & Options

```rust,no_run
use adaq_data_stock_us::blocking::Client;
use adaq_data_stock_us::fundamentals::{Freq, Statement};

fn main() -> adaq_data_stock_us::Result<()> {
    let client = Client::new()?;

    // Full info blob (common fields + raw JSON on Info::raw)
    let info = client.info("AAPL")?;
    println!("sector={:?} industry={:?} pe={:?}", info.sector, info.industry, info.trailing_pe);

    // Lightweight, history-derived fast info
    let fi = client.fast_info("AAPL")?;
    println!("last={:?} 50d avg={:?}", fi.last_price, fi.fifty_day_average);

    // Holders (major / institutional / mutual-fund / insider)
    let holders = client.holders("AAPL")?;
    println!("major holders: {}", holders.major.len());

    // Three financial statements, annual or quarterly
    let fin = client.financials("AAPL", Statement::Income, Freq::Annual)?;
    println!("dates={:?}", fin.dates);
    if let Some(first) = fin.dates.first() {
        println!("totalRevenue @ {} = {:?}", first, fin.get("totalRevenue", first));
    }

    // Option chain
    let chain = client.option_chain("AAPL")?;
    println!("expirations={}, first={:?}", chain.expirations.len(), chain.expirations.first());
    Ok(())
}
```

### Analysis & Estimates

```rust,no_run
use adaq_data_stock_us::blocking::Client;

fn main() -> adaq_data_stock_us::Result<()> {
    let client = Client::new()?;
    let ticker = "AAPL";

    let eps_est = client.earnings_estimate(ticker)?;     // NamedTable
    println!("earnings estimate rows: {:?}", eps_est.index);

    let targets = client.analyst_price_targets(ticker)?; // current/low/high/mean/median
    println!("analyst mean target: {:?}", targets.mean);

    let recs = client.recommendation_trend(ticker)?;     // per-period strongBuy..strongSell
    println!("recommendation periods: {}", recs.len());

    let changes = client.upgrades_downgrades(ticker)?;   // rating change history
    println!("rating changes: {}", changes.len());

    let vals = client.valuation_measures(ticker)?;       // Market Cap, P/E, PEG, P/B, EV/EBITDA…
    println!("valuation rows: {:?}", vals.index);

    let cal = client.calendar(ticker)?;                  // earnings & dividend dates
    println!("next earnings: {:?}", cal.earnings_date);

    let filings = client.sec_filings(ticker)?;           // SEC filings
    println!("sec filings: {}", filings.len());

    let shares = client.shares(ticker)?;                 // current shares outstanding
    println!("shares outstanding: {:?}", shares);

    let shares_full = client.shares_full(ticker, None, None)?; // full time series
    println!("shares time-series points: {}", shares_full.len());
    Ok(())
}
```

`NamedTable` (returned by the estimate methods) is a labelled matrix with a
`index` (rows) × `columns` shape and a `get(row, col)` lookup, mirroring the
DataFrame shape yfinance returns.

### Search, Lookup, Domain & Calendars

```rust,no_run
use adaq_data_stock_us::blocking::Client;
use adaq_data_stock_us::domain::MarketRegion;

fn main() -> adaq_data_stock_us::Result<()> {
    let client = Client::new()?;

    // Free-text search
    let s = client.search("apple", 5, 0)?;
    for q in &s.quotes { println!("{}  {}", q.symbol.unwrap_or_default(), q.short_name.unwrap_or_default()); }

    // Lookup by type ("equity", "etf", "mutualfund", "index", "cryptocurrency"…)
    let l = client.lookup("tesla", 5, "equity")?;
    println!("lookup results: {}", l.results.len());

    // Domain snapshots
    let sector = client.sector("technology")?;
    println!("sector: {:?}  top companies: {}", sector.name, sector.top_companies.len());
    let market = client.market(MarketRegion::Us)?;
    println!("US market rows: {}, status: {:?}", market.summary.len(), market.status);

    // Calendars (between two YYYY-MM-DD dates)
    let earn  = client.earnings_calendar("2026-08-01", "2026-08-15", 25)?;
    let ipo   = client.ipo_calendar("2026-08-01", "2026-08-15", 25)?;
    let econ  = client.economic_calendar("2026-08-01", "2026-08-15", 25)?;
    let split = client.splits_calendar("2026-08-01", "2026-08-15", 25)?;
    println!("earnings={} ipo={} economic={} splits={}", earn.len(), ipo.len(), econ.len(), split.len());
    Ok(())
}
```

### Screener

Use a predefined screen by name, or build a custom query with the typed DSL.

```rust,no_run
use adaq_data_stock_us::blocking::Client;
use adaq_data_stock_us::screener::{Operand, Operator, Query, ScreenOptions};

fn main() -> adaq_data_stock_us::Result<()> {
    let client = Client::new()?;

    // Predefined screen
    let res = client.screen("day_gainers", &ScreenOptions::default())?;
    println!("day_gainers total={:?} returned={}", res.total, res.quotes.len());

    // Custom equity screen: percentchange > 3 AND region = us
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

Available predefined screens include `day_gainers`, `day_losers`, `most_actives`,
`aggressive_small_caps`, `growth_technology_stocks`, `most_shorted_stocks`, and more
(embedded from yfinance 1.6.0).

### Per-Ticker News, Earnings Dates & ISIN

```rust,no_run
use adaq_data_stock_us::blocking::Client;

fn main() -> adaq_data_stock_us::Result<()> {
    let client = Client::new()?;

    // News. `tab` is "news" (default), "all", or "press releases".
    let news = client.news("AAPL", 5, "news")?;
    for a in news.iter().take(3) {
        println!("{} — {}", a.title.unwrap_or_default(), a.publisher.unwrap_or_default());
        if let Some(url) = a.thumbnail_url() { println!("  thumb: {url}"); }
    }

    // Earnings dates (newest first; limit capped at 100)
    let dates = client.earnings_dates("AAPL", 20)?;
    for d in dates.iter().take(3) {
        println!("earnings on {:?}  eps_est={:?} eps_actual={:?}",
            d.date, d.eps_estimate, d.eps_actual);
    }

    // Reverse lookup: ticker -> ISIN
    let isin = client.isin("AAPL")?;
    println!("AAPL ISIN: {isin}");
    Ok(())
}
```

### Live Streaming

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

For blocking callers, `blocking::Client::stream_live(symbols, handler)` runs the same
stream to completion on the shared runtime.

### Authentication

```rust,no_run
use adaq_data_stock_us::Client;

#[tokio::main]
async fn main() -> adaq_data_stock_us::Result<()> {
    let client = Client::new()?;
    let auth = client.auth();

    // Inject Yahoo T/Y login cookies (from browser devtools)
    let ok = auth.set_login_cookies("<T-cookie>", "<Y-cookie>").await?;
    println!("cookies accepted: {ok}");

    println!("logged in: {}", auth.check_login().await?);
    println!("tier: {:?}", auth.subscription_tier().await?);
    println!("user guid: {:?}", auth.user().await?);
    Ok(())
}
```

### Bulk Download

Download history for many tickers concurrently. In lenient mode (the default), a
failing ticker is collected into `errors` rather than aborting the batch.

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

## Feature Flags

| Flag | Default | Effect |
|------|---------|--------|
| `polars` | off | Adds `History::to_polars()` → `polars::prelude::DataFrame` for tabular consumers. |

```toml
adaq-data-stock-us = { version = "0.1", features = ["polars"] }
```

```rust,ignore
let df = hist.to_polars()?;   // requires the `polars` feature
```

---

## Caching

A single on-disk **sqlite** file (`adaq-yfinance.db`) caches the crumb, per-ticker
timezone, and ISIN→ticker mappings. By default it lives in a temp directory; set a
persistent location with `Config::cache_dir`:

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

## Error Handling

All fallible calls return `adaq_data_stock_us::Result<T>` =
`Result<T, YfError>`. The `YfError` taxonomy mirrors yfinance's exception hierarchy:

| Variant | Meaning |
|---------|---------|
| `Http` | Network/transport failure from the HTTP client. |
| `Status { status, body }` | Yahoo returned a non-success HTTP status. |
| `RateLimited` | Yahoo rate-limited the request (HTTP 429). |
| `Parse` | JSON (de)serialization failure. |
| `TickerMissing` | The ticker could not be found / resolved. |
| `InvalidPeriod` | Invalid period / interval / range combination. |
| `DataMissing` | Expected data was missing from the response. |
| `NotSupported` | Feature not yet implemented / not supported. |
| `Cache` | Local sqlite cache failure. |
| `Io` | Filesystem / IO failure. |
| `Msg` | Generic message error. |

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

## Parity with yfinance

The crate tracks **yfinance 1.6.0** (pinned at `vendor/yfinance` commit `93eb4c2`;
see `PARITY_PIN`). Per-module status is recorded in [`docs/PARITY.md`](docs/PARITY.md),
and the alignment mechanism is documented in [`docs/adr/0003-parity-mechanism.md`](docs/adr/0003-parity-mechanism.md).

- **Phases P1–P4: `done`** — HTTP core, history, download, quote, fundamentals,
  options, analysis, domain, search, lookup, calendars, screener, live, auth.
- **Price-repair: `partial`** — split-continuous repair (drop non-positive bars +
  scale pre-split bars by the split factor) is implemented; the full upstream
  multi-endpoint reconciliation is not reproduced.

Run the parity drift checker against the vendored submodule:

```sh
cargo xtask parity
```

---

## Examples

Runnable examples live in `examples/` (also `src/main.rs`):

| Example | Covers |
|---------|--------|
| `main.rs` | Ticker-identifier resolution (symbol / MIC / ISIN). |
| `quick` | History + corporate actions via the blocking API. |
| `smoke` | History, `info`, `fast_info`, financials, option chain. |
| `p3` | Search, lookup, domain, calendars, screener. |
| `p4` | Live WebSocket streaming + authentication. |
| `ticker_id` | MIC/ISIN resolution + per-ticker news / earnings / reverse ISIN. |

Run any of them:

```sh
cargo run --example quick
cargo run --example smoke
cargo run --example p3
cargo run --example p4
cargo run --example ticker_id
cargo run                       # runs src/main.rs
```

---

## Project Layout

```
src/
  lib.rs          public API surface (re-exports)
  client.rs       Client + Ticker handle + download (async)
  blocking.rs     blocking facade over the async Client/Ticker
  http.rs         YfSession: cookies, crumb, consent, retry
  config.rs       Config + builder
  cache.rs        sqlite cache (crumb / tz / isin)
  error.rs        YfError taxonomy
  history.rs      History / Bar / HistoryOptions / Interval / actions / repair
  quote.rs        info, fast_info, holders, analysis, estimates, valuation, calendar, sec_filings, funds, shares
  fundamentals.rs income / balance-sheet / cash-flow statements
  options.rs      option chain
  news.rs         per-ticker news
  earnings.rs     per-ticker earnings dates
  isin.rs         ISIN <-> ticker resolution
  mic.rs          MIC -> Yahoo suffix map
  search.rs       free-text search
  lookup.rs       typed lookup
  domain.rs       sector / industry / market
  calendars.rs    earnings / IPO / economic / splits
  screener.rs     predefined + custom query DSL
  live.rs         WebSocket live stream (PricingData protobuf)
  auth.rs         login cookies + subscription tier
docs/
  PARITY.md       per-module yfinance parity status
  adr/            architecture decision records
  agents/         agent collaboration docs
vendor/yfinance/  vendored yfinance submodule (pinned)
xtask/            cargo xtask parity drift checker
```

---

## Changelog

See [CHANGELOG.md](CHANGELOG.md) for the full bilingual (English / 简体中文)
release history.

---

## License

Licensed under the [Apache License 2.0](LICENSE).
