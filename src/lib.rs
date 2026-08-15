//! # adaq-data-stock-us
//!
//! A Rust reimplementation of Python's `yfinance` for fetching US equity market
//! data from Yahoo Finance.
//!
//! - Canonical return types are strongly-typed structs (compile-time guarantees);
//!   tabular consumers can convert via `History::to_polars()` (feature `polars`)
//!   or `serde` (JSON, always on). See `docs/adr/0002-typed-struct-canonical.md`.
//! - HTTP uses `primp` with Chrome TLS impersonation to avoid Yahoo rate-limiting.
//!   See `docs/adr/0001-async-primp.md`.
//! - Async-first, with a [`blocking`] facade for non-async callers.
//!
//! ```no_run
//! use adaq_data_stock_us::{Client, HistoryOptions, Interval};
//! # async fn run() -> adaq_data_stock_us::Result<()> {
//! let client = Client::new()?;
//! let hist = client.history("AAPL", &HistoryOptions::default()).await?;
//! println!("bars: {}", hist.bars.len());
//! # Ok(()) }
//! ```

pub mod auth;
pub mod blocking;
pub mod cache;
pub mod calendars;
pub mod client;
pub mod config;
pub mod domain;
pub mod earnings;
pub mod error;
pub mod fundamentals;
pub mod history;
pub mod http;
pub mod isin;
pub mod json;
pub mod live;
pub mod lookup;
pub mod mic;
pub mod news;
pub mod options;
pub mod quote;
pub mod screener;
pub mod search;

pub use auth::Auth;
pub use calendars::{
    CalendarColumn, CalendarOperand, CalendarQuery, CalendarResult, EarningsEvent, EconomicEvent,
    IpoEvent, SplitEvent,
};
pub use client::{Client, DownloadResult, Ticker, TickerId, download};
pub use config::Config;
pub use earnings::EarningsDate;
pub use error::{Result, YfError};
pub use fundamentals::{Financials, Freq, Statement};
pub use history::{
    Actions, Bar, CapitalGain, Dividend, History, HistoryMeta, HistoryOptions, Interval, Split,
};
pub use http::YfSession;
pub use isin::{isin_for_ticker, resolve_isin};
pub use live::{LiveWebSocket, PricingData};
pub use lookup::{LookupResult, LookupRow};
pub use mic::{MIC_TO_YAHOO_SUFFIX, is_isin, mic_to_suffix, resolve_symbol};
pub use news::NewsArticle;
pub use options::{ExpirationOptions, OptionChain, OptionContract};
pub use quote::{
    AnalystPriceTargets, Calendar, FastInfo, FundHolding, FundsData, HolderRow, Holders, Info,
    NamedTable, RecommendationTrend, SecFiling, Sustainability, UpgradesDowngrades,
};
pub use screener::{
    ETFQuery, EquityQuery, FundQuery, Operand, Operator, Query, QueryKind, ScreenOptions,
    ScreenerQuery, ScreenerQuote, ScreenerResult, ScreenerValue,
};
pub use search::{SearchNews, SearchQuote, SearchResult};

/// Run a future to completion on a shared multi-thread runtime. Used by the
/// [`blocking`] facade.
pub fn block_on<F: std::future::Future>(f: F) -> F::Output {
    static RT: std::sync::OnceLock<tokio::runtime::Runtime> = std::sync::OnceLock::new();
    let rt = RT.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("failed to build tokio runtime")
    });
    rt.block_on(f)
}
