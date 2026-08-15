# adaq-data-stock-us

A Rust reimplementation of Python's [`yfinance`](https://github.com/ranaroussi/yfinance)
for fetching US (and global) equity market data from Yahoo Finance.

Async-first, with a blocking facade. Canonical return types are strongly-typed
structs (compile-time guarantees); tabular consumers can convert history via
`History::to_polars()` (feature `polars`) or `serde` (JSON, always on). HTTP uses
[`primp`](https://crates.io/crates/primp) with Chrome TLS impersonation to avoid
Yahoo rate-limiting.

## Features

- **History** — OHLCV bars with dividend/split adjustment and corporate actions.
- **Ticker identifiers** — construct a `Ticker` from a bare symbol, a
  `(symbol, MIC)` pair (e.g. `OR`/`XPAR` → `OR.PA`), or an ISIN
  (`US0378331005` → `AAPL`). ISIN↔ticker resolution mirrors `yfinance`.
- **Quote / fundamentals / options** — `info`, `fast_info`, holders,
  sustainability, analyst price targets, recommendation trend, three financial
  statements, and the option chain.
- **Search / lookup / domain / calendars / screener** — free-text search,
  security lookup, sector/industry/market snapshots, earnings/IPO/economic/split
  calendars, and the screener DSL.
- **Per-ticker news, earnings dates, and reverse ISIN** — `get_news`,
  `get_earnings_dates`, and `get_isin` parity.
- **Live stream** — WebSocket price stream decoding the Yahoo protobuf feed.
- **Auth** — Yahoo `T`/`Y` login-cookie injection with subscription-tier/user
  inspection.

## Example

```rust,no_run
use adaq_data_stock_us::blocking::Client;
use adaq_data_stock_us::history::{HistoryOptions, Interval};
use adaq_data_stock_us::TickerId;

fn main() -> adaq_data_stock_us::Result<()> {
    let client = Client::new()?;

    // Bare symbol
    let h = client.history("AAPL", &HistoryOptions::default())?;
    println!("AAPL bars: {}", h.bars.len());

    // (symbol, MIC) pair -> OR.PA
    let or = client.ticker_from_mic("OR", "XPAR")?;
    println!("{}", or.symbol());

    // ISIN -> AAPL
    let aapl = client.ticker_from_isin("US0378331005")?;
    println!("{}", aapl.symbol());

    // Any identifier
    let _ = client.ticker_from_id(TickerId::Symbol("MSFT".into()))?;
    Ok(())
}
```

## Parity

The crate tracks `yfinance` 1.6.0 (pinned at `vendor/yfinance` commit
`93eb4c2`). See [`docs/PARITY.md`](docs/PARITY.md) for the per-module status.
