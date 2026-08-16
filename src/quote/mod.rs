//! `quoteSummary` family: `info`, `fast_info`, holders, analysis, sustainability,
//! funds, and calendar endpoints.
//!
//! Mirrors yfinance's `scrapers/quote.py` + `holders.py` + `analysis.py` +
//! `funds.py`. The `v10/finance/quoteSummary` endpoint takes a `modules` list;
//! we request the relevant modules and parse typed structs. The full raw JSON
//! is retained on [`Info::raw`] so no upstream field is lost.
//!
//! This module is split into concept submodules:
//! - [`info`] — `Info` / `FastInfo` (+ `shares` / `shares_full`)
//! - [`holders`] — `Holders` / `HolderRow`
//! - [`summary`] — `Sustainability` / `AnalystPriceTargets`
//! - [`analysis`] — estimates / recommendation / upgrade-downgrade tables
//! - [`funds`] — `FundsData` (mutual-fund / ETF)
//! - [`calendar`] — `Calendar` / `SecFiling`
//!
//! All public types and the shared [`quote_summary`] fetcher are re-exported at
//! this crate-root module so existing callers (`client`, `blocking`, `lib`) keep
//! referencing `crate::quote::*` unchanged.

use crate::error::{Result, YfError};
use crate::http::YfSession;
use crate::json::yf_result_first;
use serde_json::Value;

mod analysis;
mod calendar;
mod funds;
mod holders;
mod info;
mod summary;

pub use analysis::{NamedTable, RecommendationTrend, UpgradesDowngrades};
pub use calendar::{Calendar, SecFiling};
pub use funds::{FundHolding, FundsData};
pub use holders::{HolderRow, Holders};
pub use info::{FastInfo, Info};
pub use summary::{AnalystPriceTargets, Sustainability};

/// Default `modules` list for the full [`Info`] blob (mirrors yfinance's
/// `Ticker.info` request).
pub const MODULES_INFO: &[&str] = &[
    "assetProfile",
    "incomeStatementHistory",
    "balanceSheetHistory",
    "cashflowStatementHistory",
    "incomeStatementHistoryQuarterly",
    "balanceSheetHistoryQuarterly",
    "cashflowStatementHistoryQuarterly",
    "defaultKeyStatistics",
    "financialData",
    "calendarEvents",
    "secFilings",
    "recommendationTrend",
    "upgradeDowngradeHistory",
    "institutionOwnership",
    "fundOwnership",
    "majorHoldersBreakdown",
    "insiderTransactions",
    "insiderHolders",
    "netSharePurchaseActivity",
    "earnings",
    "earningsHistory",
    "earningsTrend",
    "quoteType",
    "price",
    "summaryDetail",
    "esgScores",
    "fundProfile",
    "fundOverview",
    "topHoldings",
    "fundPerformance",
];

/// Fetch a raw `quoteSummary` result for the given modules.
pub async fn quote_summary(session: &YfSession, ticker: &str, modules: &[&str]) -> Result<Value> {
    let urls = YfSession::urls();
    let url = format!("{}/v10/finance/quoteSummary/{}", urls.query2, ticker);
    let params = vec![("modules", modules.join(","))];
    let value = session.get_json(&url, &params).await?;
    yf_result_first(&value, "quoteSummary")
        .map_err(|_| YfError::DataMissing(format!("quoteSummary.result for {ticker}")))
        .cloned()
}

/// Convert an epoch-seconds `Option<f64>` into a UTC datetime, tolerating
/// missing nodes. Shared by the submodules.
pub(crate) fn ts_to_date(sec: Option<f64>) -> Option<chrono::DateTime<chrono::Utc>> {
    sec.and_then(|s| chrono::DateTime::from_timestamp(s as i64, 0))
}
