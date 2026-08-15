//! The main [`Client`] entry point and the `Ticker` handle, plus the blocking
//! facade. Mirrors yfinance's `Ticker` + module-level `download()`.

use std::sync::Arc;

use crate::config::Config;
use crate::error::{Result, YfError};
use crate::history::{History, HistoryOptions};
use crate::http::YfSession;

/// Async Yahoo Finance client. Cheap to clone (shares one session + cache).
#[derive(Clone)]
pub struct Client {
    session: Arc<YfSession>,
}

impl Client {
    /// Create a client with default configuration.
    pub fn new() -> Result<Self> {
        Ok(Self {
            session: Arc::new(YfSession::new(Config::default())?),
        })
    }

    /// Create a client with explicit configuration.
    pub fn with_config(config: Config) -> Result<Self> {
        Ok(Self {
            session: Arc::new(YfSession::new(config)?),
        })
    }

    /// Access the underlying session.
    pub fn session(&self) -> &YfSession {
        &self.session
    }

    /// Fetch price history for a single ticker.
    pub async fn history(&self, ticker: &str, opts: &HistoryOptions) -> Result<History> {
        self.session.history(ticker, opts).await
    }

    // --- P2: quote / fundamentals / options ---

    /// `info` blob (mirrors `Ticker.info`).
    pub async fn info(&self, ticker: &str) -> Result<crate::quote::Info> {
        self.session.info(ticker).await
    }

    /// `fast_info` subset (mirrors `Ticker.fast_info`).
    pub async fn fast_info(&self, ticker: &str) -> Result<crate::quote::FastInfo> {
        self.session.fast_info(ticker).await
    }

    /// Holders (mirrors `Ticker.holders`).
    pub async fn holders(&self, ticker: &str) -> Result<crate::quote::Holders> {
        self.session.holders(ticker).await
    }

    /// Sustainability / ESG (mirrors `Ticker.sustainability`).
    pub async fn sustainability(&self, ticker: &str) -> Result<crate::quote::Sustainability> {
        self.session.sustainability(ticker).await
    }

    /// Analyst price targets (mirrors `Ticker.analyst_price_targets`).
    pub async fn analyst_price_targets(
        &self,
        ticker: &str,
    ) -> Result<crate::quote::AnalystPriceTargets> {
        self.session.analyst_price_targets(ticker).await
    }

    /// Recommendation trend (mirrors `Ticker.recommendation_trend`).
    pub async fn recommendation_trend(
        &self,
        ticker: &str,
    ) -> Result<Vec<crate::quote::RecommendationTrend>> {
        self.session.recommendation_trend(ticker).await
    }

    /// Financial statement (mirrors `Ticker.get_income_stmt` etc.).
    pub async fn financials(
        &self,
        ticker: &str,
        statement: crate::fundamentals::Statement,
        freq: crate::fundamentals::Freq,
    ) -> Result<crate::fundamentals::Financials> {
        self.session.financials(ticker, statement, freq).await
    }

    /// Option chain (mirrors `Ticker.option_chain`).
    pub async fn option_chain(&self, ticker: &str) -> Result<crate::options::OptionChain> {
        self.session.option_chain(ticker).await
    }

    // --- P3: search / lookup / domain / calendars / screener ---

    /// Free-text search (mirrors `yfinance.Search`).
    pub async fn search(
        &self,
        query: &str,
        quotes_count: usize,
        news_count: usize,
    ) -> Result<crate::search::SearchResult> {
        self.session.search(query, quotes_count, news_count).await
    }

    /// Security lookup by query and type (mirrors `yfinance.Lookup`).
    pub async fn lookup(
        &self,
        query: &str,
        limit: usize,
        lookup_type: &str,
    ) -> Result<crate::lookup::LookupResult> {
        self.session.lookup(query, limit, lookup_type).await
    }

    /// Market sector snapshot (mirrors `yfinance.domain.Sector`).
    pub async fn sector(&self, key: &str) -> Result<crate::domain::Sector> {
        self.session.sector(key).await
    }

    /// Market industry snapshot (mirrors `yfinance.domain.Industry`).
    pub async fn industry(&self, key: &str) -> Result<crate::domain::Industry> {
        self.session.industry(key).await
    }

    /// Market summary for a region (mirrors `yfinance.domain.Market`).
    pub async fn market(
        &self,
        region: crate::domain::MarketRegion,
    ) -> Result<crate::domain::Market> {
        self.session.market(region).await
    }

    /// Earnings calendar between two `YYYY-MM-DD` dates.
    pub async fn earnings_calendar(
        &self,
        start: &str,
        end: &str,
        limit: usize,
    ) -> Result<Vec<crate::calendars::EarningsEvent>> {
        self.session.earnings_calendar(start, end, limit).await
    }

    /// IPO calendar between two `YYYY-MM-DD` dates.
    pub async fn ipo_calendar(
        &self,
        start: &str,
        end: &str,
        limit: usize,
    ) -> Result<Vec<crate::calendars::IpoEvent>> {
        self.session.ipo_calendar(start, end, limit).await
    }

    /// Economic calendar between two `YYYY-MM-DD` dates.
    pub async fn economic_calendar(
        &self,
        start: &str,
        end: &str,
        limit: usize,
    ) -> Result<Vec<crate::calendars::EconomicEvent>> {
        self.session.economic_calendar(start, end, limit).await
    }

    /// Splits calendar between two `YYYY-MM-DD` dates.
    pub async fn splits_calendar(
        &self,
        start: &str,
        end: &str,
        limit: usize,
    ) -> Result<Vec<crate::calendars::SplitEvent>> {
        self.session.splits_calendar(start, end, limit).await
    }

    /// Run a screener query (mirrors `yfinance.screen`).
    pub async fn screen(
        &self,
        query: impl Into<crate::screener::ScreenerQuery>,
        opts: &crate::screener::ScreenOptions,
    ) -> Result<crate::screener::ScreenerResult> {
        self.session.screen(query, opts).await
    }

    // --- P4: auth / live ---

    /// An auth/entitlement helper bound to this client's session.
    /// Mirrors `yfinance.Auth`.
    pub fn auth(&self) -> crate::auth::Auth {
        crate::auth::Auth::new((*self.session).clone())
    }

    /// A live-streaming client. Mirrors `yfinance.AsyncWebSocket` /
    /// `yfinance.WebSocket` (no session needed).
    pub fn live(&self) -> crate::live::LiveWebSocket {
        crate::live::LiveWebSocket::new()
    }
}

impl Default for Client {
    fn default() -> Self {
        Self::new().expect("default client should build")
    }
}

/// A handle to a single security, mirroring yfinance's `Ticker`.
#[derive(Clone)]
pub struct Ticker {
    client: Client,
    symbol: String,
}

impl Ticker {
    /// Create a ticker handle under the given client.
    pub fn new(symbol: impl Into<String>, client: Client) -> Self {
        Self {
            client,
            symbol: symbol.into(),
        }
    }

    /// The ticker symbol.
    pub fn symbol(&self) -> &str {
        &self.symbol
    }

    /// The client this ticker uses.
    pub fn client(&self) -> &Client {
        &self.client
    }

    /// Price history (see [`Client::history`]).
    pub async fn history(&self, opts: &HistoryOptions) -> Result<History> {
        self.client.history(&self.symbol, opts).await
    }

    /// Dividends as a vector (from history actions).
    pub async fn dividends(&self, opts: &HistoryOptions) -> Result<Vec<crate::history::Dividend>> {
        let mut o = opts.clone();
        o.actions = true;
        let h = self.client.history(&self.symbol, &o).await?;
        Ok(h.actions.map(|a| a.dividends).unwrap_or_default())
    }

    /// Stock splits as a vector (from history actions).
    pub async fn splits(&self, opts: &HistoryOptions) -> Result<Vec<crate::history::Split>> {
        let mut o = opts.clone();
        o.actions = true;
        let h = self.client.history(&self.symbol, &o).await?;
        Ok(h.actions.map(|a| a.splits).unwrap_or_default())
    }

    /// All corporate actions (dividends, splits, capital gains).
    pub async fn actions(&self, opts: &HistoryOptions) -> Result<Option<crate::history::Actions>> {
        let mut o = opts.clone();
        o.actions = true;
        let h = self.client.history(&self.symbol, &o).await?;
        Ok(h.actions)
    }

    // --- P2: quote / fundamentals / options ---

    /// `info` (mirrors `Ticker.info`).
    pub async fn info(&self) -> Result<crate::quote::Info> {
        self.client.info(&self.symbol).await
    }

    /// `fast_info` (mirrors `Ticker.fast_info`).
    pub async fn fast_info(&self) -> Result<crate::quote::FastInfo> {
        self.client.fast_info(&self.symbol).await
    }

    /// Holders (mirrors `Ticker.holders`).
    pub async fn holders(&self) -> Result<crate::quote::Holders> {
        self.client.holders(&self.symbol).await
    }

    /// Sustainability (mirrors `Ticker.sustainability`).
    pub async fn sustainability(&self) -> Result<crate::quote::Sustainability> {
        self.client.sustainability(&self.symbol).await
    }

    /// Analyst price targets (mirrors `Ticker.analyst_price_targets`).
    pub async fn analyst_price_targets(&self) -> Result<crate::quote::AnalystPriceTargets> {
        self.client.analyst_price_targets(&self.symbol).await
    }

    /// Recommendation trend (mirrors `Ticker.recommendation_trend`).
    pub async fn recommendation_trend(&self) -> Result<Vec<crate::quote::RecommendationTrend>> {
        self.client.recommendation_trend(&self.symbol).await
    }

    /// Financial statement (mirrors `Ticker.get_income_stmt` etc.).
    pub async fn financials(
        &self,
        statement: crate::fundamentals::Statement,
        freq: crate::fundamentals::Freq,
    ) -> Result<crate::fundamentals::Financials> {
        self.client.financials(&self.symbol, statement, freq).await
    }

    /// Option chain (mirrors `Ticker.option_chain`).
    pub async fn option_chain(&self) -> Result<crate::options::OptionChain> {
        self.client.option_chain(&self.symbol).await
    }
}

/// Result of a bulk [`download`]: per-ticker histories plus any lenient errors.
#[derive(Debug, Clone)]
pub struct DownloadResult {
    pub histories: Vec<History>,
    pub errors: Vec<(String, String)>,
}

/// Bulk download of history for many tickers, concurrently.
///
/// In lenient mode (default), per-ticker failures are collected into
/// `errors` instead of aborting the whole batch — mirroring yfinance's
/// `hide_exceptions` "Failed downloads" behaviour.
pub async fn download(
    tickers: &[&str],
    opts: &HistoryOptions,
    client: &Client,
) -> Result<DownloadResult> {
    let lenient = client.session().config().lenient;
    let futures = tickers.iter().map(|t| async move {
        match client.history(t, opts).await {
            Ok(h) => Ok((t.to_string(), h)),
            Err(e) => Err((t.to_string(), e)),
        }
    });
    let outcomes = futures::future::join_all(futures).await;
    let mut histories = Vec::new();
    let mut errors = Vec::new();
    for r in outcomes {
        match r {
            Ok((_, h)) => histories.push(h),
            Err((sym, e)) => {
                if lenient {
                    errors.push((sym, e.to_string()));
                } else {
                    return Err(YfError::msg(format!("download failed for {sym}: {e}")));
                }
            }
        }
    }
    Ok(DownloadResult { histories, errors })
}
