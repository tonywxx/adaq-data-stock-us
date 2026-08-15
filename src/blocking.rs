//! Blocking (synchronous) facade over the async [`crate::Client`].
//!
//! Each method runs the corresponding async call on the shared runtime via
//! [`crate::block_on`]. This is the ergonomic entry point for callers who do
//! not want to write `async` (mirrors yfinance's synchronous API).

use crate::block_on;
use crate::client::{Client as AsyncClient, DownloadResult, Ticker as AsyncTicker};
use crate::config::Config;
use crate::error::Result;
use crate::history::{History, HistoryOptions};

/// Synchronous Yahoo Finance client.
#[derive(Clone)]
pub struct Client {
    inner: AsyncClient,
}

impl Client {
    /// Create a blocking client with default config.
    pub fn new() -> Result<Self> {
        Ok(Self {
            inner: AsyncClient::new()?,
        })
    }

    /// Create a blocking client with explicit config.
    pub fn with_config(config: Config) -> Result<Self> {
        Ok(Self {
            inner: AsyncClient::with_config(config)?,
        })
    }

    /// Price history for a single ticker.
    pub fn history(&self, ticker: &str, opts: &HistoryOptions) -> Result<History> {
        block_on(self.inner.history(ticker, opts))
    }

    /// Bulk download of many tickers.
    pub fn download(&self, tickers: &[&str], opts: &HistoryOptions) -> Result<DownloadResult> {
        block_on(crate::download(tickers, opts, &self.inner))
    }

    /// `info` blob.
    pub fn info(&self, ticker: &str) -> Result<crate::quote::Info> {
        block_on(self.inner.info(ticker))
    }

    /// `fast_info` subset.
    pub fn fast_info(&self, ticker: &str) -> Result<crate::quote::FastInfo> {
        block_on(self.inner.fast_info(ticker))
    }

    /// Holders.
    pub fn holders(&self, ticker: &str) -> Result<crate::quote::Holders> {
        block_on(self.inner.holders(ticker))
    }

    /// Sustainability / ESG.
    pub fn sustainability(&self, ticker: &str) -> Result<crate::quote::Sustainability> {
        block_on(self.inner.sustainability(ticker))
    }

    /// Analyst price targets.
    pub fn analyst_price_targets(&self, ticker: &str) -> Result<crate::quote::AnalystPriceTargets> {
        block_on(self.inner.analyst_price_targets(ticker))
    }

    /// Recommendation trend.
    pub fn recommendation_trend(
        &self,
        ticker: &str,
    ) -> Result<Vec<crate::quote::RecommendationTrend>> {
        block_on(self.inner.recommendation_trend(ticker))
    }

    /// Financial statement.
    pub fn financials(
        &self,
        ticker: &str,
        statement: crate::fundamentals::Statement,
        freq: crate::fundamentals::Freq,
    ) -> Result<crate::fundamentals::Financials> {
        block_on(self.inner.financials(ticker, statement, freq))
    }

    /// Option chain.
    pub fn option_chain(&self, ticker: &str) -> Result<crate::options::OptionChain> {
        block_on(self.inner.option_chain(ticker))
    }

    /// Free-text search.
    pub fn search(
        &self,
        query: &str,
        quotes_count: usize,
        news_count: usize,
    ) -> Result<crate::search::SearchResult> {
        block_on(self.inner.search(query, quotes_count, news_count))
    }

    /// Security lookup.
    pub fn lookup(
        &self,
        query: &str,
        limit: usize,
        lookup_type: &str,
    ) -> Result<crate::lookup::LookupResult> {
        block_on(self.inner.lookup(query, limit, lookup_type))
    }

    /// Market sector snapshot.
    pub fn sector(&self, key: &str) -> Result<crate::domain::Sector> {
        block_on(self.inner.sector(key))
    }

    /// Market industry snapshot.
    pub fn industry(&self, key: &str) -> Result<crate::domain::Industry> {
        block_on(self.inner.industry(key))
    }

    /// Market summary for a region.
    pub fn market(&self, region: crate::domain::MarketRegion) -> Result<crate::domain::Market> {
        block_on(self.inner.market(region))
    }

    /// Earnings calendar.
    pub fn earnings_calendar(
        &self,
        start: &str,
        end: &str,
        limit: usize,
    ) -> Result<Vec<crate::calendars::EarningsEvent>> {
        block_on(self.inner.earnings_calendar(start, end, limit))
    }

    /// IPO calendar.
    pub fn ipo_calendar(
        &self,
        start: &str,
        end: &str,
        limit: usize,
    ) -> Result<Vec<crate::calendars::IpoEvent>> {
        block_on(self.inner.ipo_calendar(start, end, limit))
    }

    /// Economic calendar.
    pub fn economic_calendar(
        &self,
        start: &str,
        end: &str,
        limit: usize,
    ) -> Result<Vec<crate::calendars::EconomicEvent>> {
        block_on(self.inner.economic_calendar(start, end, limit))
    }

    /// Splits calendar.
    pub fn splits_calendar(
        &self,
        start: &str,
        end: &str,
        limit: usize,
    ) -> Result<Vec<crate::calendars::SplitEvent>> {
        block_on(self.inner.splits_calendar(start, end, limit))
    }

    /// Run a screener query.
    pub fn screen(
        &self,
        query: impl Into<crate::screener::ScreenerQuery>,
        opts: &crate::screener::ScreenOptions,
    ) -> Result<crate::screener::ScreenerResult> {
        block_on(self.inner.screen(query, opts))
    }

    // --- Ticker identifier resolution + per-ticker news/earnings/ISIN ---

    /// Resolve an ISIN to a Yahoo ticker symbol.
    pub fn resolve_isin(&self, isin: &str) -> Result<String> {
        block_on(self.inner.resolve_isin(isin))
    }

    /// Latest news for a ticker. `tab` is `"news"`, `"all"`, or `"press releases"`.
    pub fn news(
        &self,
        ticker: &str,
        count: usize,
        tab: &str,
    ) -> Result<Vec<crate::news::NewsArticle>> {
        block_on(self.inner.news(ticker, count, tab))
    }

    /// Reverse lookup: ticker → ISIN.
    pub fn isin(&self, ticker: &str) -> Result<String> {
        block_on(self.inner.isin(ticker))
    }

    /// Scheduled / reported earnings dates for a ticker, newest first.
    pub fn earnings_dates(
        &self,
        ticker: &str,
        limit: usize,
    ) -> Result<Vec<crate::earnings::EarningsDate>> {
        block_on(self.inner.earnings_dates(ticker, limit))
    }

    /// A blocking ticker from a `(symbol, MIC)` pair.
    pub fn ticker_from_mic(&self, symbol: &str, mic: &str) -> Result<Ticker> {
        Ok(Ticker {
            inner: AsyncTicker::from_mic(symbol, mic, self.inner.clone())?,
        })
    }

    /// A blocking ticker from an ISIN.
    pub fn ticker_from_isin(&self, isin: &str) -> Result<Ticker> {
        Ok(Ticker {
            inner: block_on(AsyncTicker::from_isin(isin, self.inner.clone()))?,
        })
    }

    /// A blocking ticker from any [`crate::TickerId`].
    pub fn ticker_from_id(&self, id: crate::TickerId) -> Result<Ticker> {
        Ok(Ticker {
            inner: block_on(AsyncTicker::from_id(id, self.inner.clone()))?,
        })
    }

    /// An auth/entitlement helper bound to this client's session.
    pub fn auth(&self) -> crate::auth::Auth {
        self.inner.auth()
    }

    /// A live-streaming client (no blocking needed — the stream runs on its own
    /// async runtime inside the call).
    pub fn live(&self) -> crate::live::LiveWebSocket {
        self.inner.live()
    }

    /// Blocking live price stream. Runs [`LiveWebSocket::stream`] to completion
    /// on the shared runtime. `handler` is invoked for each decoded tick.
    pub fn stream_live<F>(&self, symbols: &[&str], handler: F) -> Result<()>
    where
        F: FnMut(crate::live::PricingData) + Send,
    {
        block_on(self.inner.live().stream(symbols, handler))
    }

    /// A blocking ticker handle.
    pub fn ticker(&self, symbol: &str) -> Ticker {
        Ticker {
            inner: AsyncTicker::new(symbol, self.inner.clone()),
        }
    }
}

impl Default for Client {
    fn default() -> Self {
        Self::new().expect("default blocking client should build")
    }
}

/// Synchronous ticker handle.
#[derive(Clone)]
pub struct Ticker {
    inner: AsyncTicker,
}

impl Ticker {
    /// The ticker symbol.
    pub fn symbol(&self) -> &str {
        self.inner.symbol()
    }

    /// Price history.
    pub fn history(&self, opts: &HistoryOptions) -> Result<History> {
        block_on(self.inner.history(opts))
    }

    /// Dividends.
    pub fn dividends(&self, opts: &HistoryOptions) -> Result<Vec<crate::history::Dividend>> {
        block_on(self.inner.dividends(opts))
    }

    /// Stock splits.
    pub fn splits(&self, opts: &HistoryOptions) -> Result<Vec<crate::history::Split>> {
        block_on(self.inner.splits(opts))
    }

    /// All corporate actions.
    pub fn actions(&self, opts: &HistoryOptions) -> Result<Option<crate::history::Actions>> {
        block_on(self.inner.actions(opts))
    }

    /// `info`.
    pub fn info(&self) -> Result<crate::quote::Info> {
        block_on(self.inner.info())
    }

    /// `fast_info`.
    pub fn fast_info(&self) -> Result<crate::quote::FastInfo> {
        block_on(self.inner.fast_info())
    }

    /// Holders.
    pub fn holders(&self) -> Result<crate::quote::Holders> {
        block_on(self.inner.holders())
    }

    /// Sustainability.
    pub fn sustainability(&self) -> Result<crate::quote::Sustainability> {
        block_on(self.inner.sustainability())
    }

    /// Analyst price targets.
    pub fn analyst_price_targets(&self) -> Result<crate::quote::AnalystPriceTargets> {
        block_on(self.inner.analyst_price_targets())
    }

    /// Recommendation trend.
    pub fn recommendation_trend(&self) -> Result<Vec<crate::quote::RecommendationTrend>> {
        block_on(self.inner.recommendation_trend())
    }

    /// Financial statement.
    pub fn financials(
        &self,
        statement: crate::fundamentals::Statement,
        freq: crate::fundamentals::Freq,
    ) -> Result<crate::fundamentals::Financials> {
        block_on(self.inner.financials(statement, freq))
    }

    /// Option chain.
    pub fn option_chain(&self) -> Result<crate::options::OptionChain> {
        block_on(self.inner.option_chain())
    }

    /// Latest news for this ticker.
    pub fn news(&self, count: usize, tab: &str) -> Result<Vec<crate::news::NewsArticle>> {
        block_on(self.inner.news(count, tab))
    }

    /// Reverse lookup: this ticker's ISIN.
    pub fn isin(&self) -> Result<String> {
        block_on(self.inner.isin())
    }

    /// Scheduled / reported earnings dates for this ticker, newest first.
    pub fn earnings_dates(&self, limit: usize) -> Result<Vec<crate::earnings::EarningsDate>> {
        block_on(self.inner.earnings_dates(limit))
    }
}
