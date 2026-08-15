//! `quoteSummary` family: `info`, `fast_info`, holders, analysis, sustainability.
//!
//! Mirrors yfinance's `scrapers/quote.py` + `holders.py` + `analysis.py` +
//! `funds.py`. The `v10/finance/quoteSummary` endpoint takes a `modules` list;
//! we request the relevant modules and parse typed structs. The full raw JSON
//! is retained on [`Info::raw`] so no upstream field is lost.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::{Result, YfError};
use crate::http::YfSession;

const MODULES_INFO: &[&str] = &[
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
    value
        .get("quoteSummary")
        .and_then(|q| q.get("result"))
        .and_then(|r| r.as_array())
        .and_then(|a| a.first())
        .cloned()
        .ok_or_else(|| YfError::DataMissing(format!("quoteSummary.result for {ticker}")))
}

/// A flattened security information blob. Common fields are extracted; the full
/// upstream JSON is kept in [`Info::raw`].
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Info {
    pub symbol: Option<String>,
    pub short_name: Option<String>,
    pub long_name: Option<String>,
    pub exchange: Option<String>,
    pub quote_type: Option<String>,
    pub sector: Option<String>,
    pub industry: Option<String>,
    pub currency: Option<String>,
    pub market_cap: Option<f64>,
    pub trailing_pe: Option<f64>,
    pub forward_pe: Option<f64>,
    pub price: Option<f64>,
    pub previous_close: Option<f64>,
    pub regular_market_price: Option<f64>,
    pub fifty_two_week_high: Option<f64>,
    pub fifty_two_week_low: Option<f64>,
    pub dividend_yield: Option<f64>,
    pub beta: Option<f64>,
    pub shares_outstanding: Option<f64>,
    pub raw: Value,
}

impl Info {
    fn from_result(ticker: &str, result: &Value) -> Info {
        let get = |path: &[&str]| dig(result, path);
        let f = |path: &[&str]| get(path).and_then(|v| v.as_f64());
        Info {
            symbol: get(&["price", "symbol"]).and_then(|v| v.as_str().map(String::from)),
            short_name: get(&["price", "shortName"]).and_then(|v| v.as_str().map(String::from)),
            long_name: get(&["price", "longName"]).and_then(|v| v.as_str().map(String::from)),
            exchange: get(&["price", "exchangeName"]).and_then(|v| v.as_str().map(String::from)),
            quote_type: get(&["quoteType", "quoteType"]).and_then(|v| v.as_str().map(String::from)),
            sector: get(&["assetProfile", "sector"]).and_then(|v| v.as_str().map(String::from)),
            industry: get(&["assetProfile", "industry"]).and_then(|v| v.as_str().map(String::from)),
            currency: get(&["price", "currency"]).and_then(|v| v.as_str().map(String::from)),
            market_cap: f(&["price", "marketCap"]),
            trailing_pe: f(&["summaryDetail", "trailingPE"]),
            forward_pe: f(&["summaryDetail", "forwardPE"]),
            price: f(&["financialData", "currentPrice", "raw"])
                .or_else(|| f(&["price", "regularMarketPrice", "raw"])),
            previous_close: f(&["price", "regularMarketPreviousClose", "raw"]),
            regular_market_price: f(&["price", "regularMarketPrice", "raw"]),
            fifty_two_week_high: f(&["price", "fiftyTwoWeekHigh", "raw"]),
            fifty_two_week_low: f(&["price", "fiftyTwoWeekLow", "raw"]),
            dividend_yield: f(&["summaryDetail", "dividendYield", "raw"]),
            beta: f(&["defaultKeyStatistics", "beta", "raw"]),
            shares_outstanding: f(&["price", "sharesOutstanding"]),
            raw: result.clone(),
        }
        .with_symbol_fallback(ticker)
    }

    fn with_symbol_fallback(mut self, ticker: &str) -> Self {
        if self.symbol.is_none() {
            self.symbol = Some(ticker.to_string());
        }
        self
    }
}

/// A fast, history-derived info subset (mirrors yfinance's `FastInfo`).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FastInfo {
    pub currency: Option<String>,
    pub quote_type: Option<String>,
    pub exchange: Option<String>,
    pub timezone: Option<String>,
    pub shares: Option<f64>,
    pub market_cap: Option<f64>,
    pub last_price: Option<f64>,
    pub previous_close: Option<f64>,
    pub open: Option<f64>,
    pub day_high: Option<f64>,
    pub day_low: Option<f64>,
    pub last_volume: Option<f64>,
    pub fifty_day_average: Option<f64>,
    pub two_hundred_day_average: Option<f64>,
    pub year_high: Option<f64>,
    pub year_low: Option<f64>,
    pub year_change: Option<f64>,
}

impl FastInfo {
    fn from_result(result: &Value) -> FastInfo {
        let get = |path: &[&str]| dig(result, path);
        let f = |path: &[&str]| get(path).and_then(|v| v.as_f64());
        FastInfo {
            currency: get(&["price", "currency"]).and_then(|v| v.as_str().map(String::from)),
            quote_type: get(&["quoteType", "quoteType"]).and_then(|v| v.as_str().map(String::from)),
            exchange: get(&["price", "exchangeName"]).and_then(|v| v.as_str().map(String::from)),
            timezone: get(&["quoteType", "exchangeTimezoneName"])
                .and_then(|v| v.as_str().map(String::from)),
            shares: f(&["price", "sharesOutstanding"]),
            market_cap: f(&["price", "marketCap"]),
            last_price: f(&["price", "regularMarketPrice", "raw"]),
            previous_close: f(&["price", "regularMarketPreviousClose", "raw"]),
            open: f(&["price", "regularMarketOpen", "raw"]),
            day_high: f(&["price", "regularMarketDayHigh", "raw"]),
            day_low: f(&["price", "regularMarketDayLow", "raw"]),
            last_volume: f(&["price", "regularMarketVolume", "raw"]),
            fifty_day_average: f(&["price", "fiftyDayAverage", "raw"]),
            two_hundred_day_average: f(&["price", "twoHundredDayAverage", "raw"]),
            year_high: f(&["price", "fiftyTwoWeekHigh", "raw"]),
            year_low: f(&["price", "fiftyTwoWeekLow", "raw"]),
            year_change: f(&["price", "fiftyTwoWeekChange", "raw"]),
        }
    }
}

/// Security holders (mirrors yfinance's holders getters).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Holders {
    pub major: Vec<HolderRow>,
    pub institutional: Vec<HolderRow>,
    pub mutual_fund: Vec<HolderRow>,
    pub insider_purchases: Vec<HolderRow>,
    pub insider_transactions: Vec<HolderRow>,
    pub insider_roster: Vec<HolderRow>,
}

/// A single holders table row.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct HolderRow {
    pub name: Option<String>,
    pub pct_in_floats: Option<f64>,
    pub position: Option<f64>,
    pub value: Option<f64>,
    pub date: Option<String>,
}

/// Sustainability / ESG scores (mirrors yfinance's `sustainability`).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Sustainability {
    pub esg_score: Option<f64>,
    pub environment_score: Option<f64>,
    pub social_score: Option<f64>,
    pub governance_score: Option<f64>,
    pub total_esg: Option<f64>,
    pub percentile: Option<f64>,
    pub raw: Value,
}

/// Analyst price targets (mirrors yfinance's `analyst_price_targets`).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AnalystPriceTargets {
    pub current: Option<f64>,
    pub low: Option<f64>,
    pub high: Option<f64>,
    pub mean: Option<f64>,
    pub median: Option<f64>,
    pub num_analysts: Option<f64>,
}

/// Recommendation trend (mirrors yfinance's `recommendation_trend`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecommendationTrend {
    pub period: Option<String>,
    pub strong_buy: Option<i64>,
    pub buy: Option<i64>,
    pub hold: Option<i64>,
    pub sell: Option<i64>,
    pub strong_sell: Option<i64>,
}

impl YfSession {
    /// `info` blob for a ticker.
    pub async fn info(&self, ticker: &str) -> Result<Info> {
        let result = quote_summary(self, ticker, MODULES_INFO).await?;
        Ok(Info::from_result(ticker, &result))
    }

    /// `fast_info` subset for a ticker.
    pub async fn fast_info(&self, ticker: &str) -> Result<FastInfo> {
        let modules = [
            "price",
            "quoteType",
            "summaryDetail",
            "defaultKeyStatistics",
        ];
        let result = quote_summary(self, ticker, &modules).await?;
        Ok(FastInfo::from_result(&result))
    }

    /// Holders (major / institutional / mutual-fund / insider).
    pub async fn holders(&self, ticker: &str) -> Result<Holders> {
        let modules = [
            "majorHoldersBreakdown",
            "institutionOwnership",
            "fundOwnership",
            "insiderTransactions",
            "insiderHolders",
            "netSharePurchaseActivity",
        ];
        let result = quote_summary(self, ticker, &modules).await?;
        Ok(Holders {
            major: parse_holder_table(&result, &["majorHoldersBreakdown", "holders"]),
            institutional: parse_holder_table(&result, &["institutionOwnership", "holders"]),
            mutual_fund: parse_holder_table(&result, &["fundOwnership", "holders"]),
            insider_purchases: parse_holder_table(
                &result,
                &["netSharePurchaseActivity", "purchases"],
            ),
            insider_transactions: parse_holder_table(
                &result,
                &["insiderTransactions", "transactions"],
            ),
            insider_roster: parse_holder_table(&result, &["insiderHolders", "holders"]),
        })
    }

    /// Sustainability / ESG.
    pub async fn sustainability(&self, ticker: &str) -> Result<Sustainability> {
        let result = quote_summary(self, ticker, &["esgScores"]).await?;
        let esg = dig(&result, &["esgScores"]).unwrap_or(&Value::Null);
        let f = |p: &[&str]| dig(esg, p).and_then(|v| v.as_f64());
        Ok(Sustainability {
            esg_score: f(&["esgScore"]),
            environment_score: f(&["environmentScore"]),
            social_score: f(&["socialScore"]),
            governance_score: f(&["governanceScore"]),
            total_esg: f(&["totalEsg"]),
            percentile: f(&["percentile"]),
            raw: esg.clone(),
        })
    }

    /// Analyst price targets.
    pub async fn analyst_price_targets(&self, ticker: &str) -> Result<AnalystPriceTargets> {
        let result = quote_summary(self, ticker, &["financialData", "price"]).await?;
        let fd = dig(&result, &["financialData"]).unwrap_or(&Value::Null);
        let f = |p: &[&str]| dig(fd, p).and_then(|v| v.as_f64());
        Ok(AnalystPriceTargets {
            current: f(&["currentPrice", "raw"]),
            low: f(&["targetLowPrice", "raw"]),
            high: f(&["targetHighPrice", "raw"]),
            mean: f(&["targetMeanPrice", "raw"]),
            median: f(&["targetMedianPrice", "raw"]),
            num_analysts: dig(fd, &["numberOfAnalystOpinions", "raw"]).and_then(|v| v.as_f64()),
        })
    }

    /// Recommendation trend history.
    pub async fn recommendation_trend(&self, ticker: &str) -> Result<Vec<RecommendationTrend>> {
        let result = quote_summary(self, ticker, &["recommendationTrend"]).await?;
        let arr = dig(&result, &["recommendationTrend", "trend"]).and_then(|v| v.as_array());
        Ok(arr
            .map(|a| {
                a.iter()
                    .map(|v| {
                        let i = |p: &[&str]| dig(v, p).and_then(|x| x.as_i64());
                        let s = |p: &[&str]| dig(v, p).and_then(|x| x.as_str().map(String::from));
                        RecommendationTrend {
                            period: s(&["period"]),
                            strong_buy: i(&["strongBuy"]),
                            buy: i(&["buy"]),
                            hold: i(&["hold"]),
                            sell: i(&["sell"]),
                            strong_sell: i(&["strongSell"]),
                        }
                    })
                    .collect()
            })
            .unwrap_or_default())
    }
}

fn parse_holder_table(result: &Value, path: &[&str]) -> Vec<HolderRow> {
    dig(result, path)
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .map(|v| HolderRow {
                    name: dig(v, &["name"]).and_then(|x| x.as_str().map(String::from)),
                    pct_in_floats: dig(v, &["pctHeld", "raw"]).and_then(|x| x.as_f64()),
                    position: dig(v, &["positionDirect", "raw"]).and_then(|x| x.as_f64()),
                    value: dig(v, &["value", "raw"]).and_then(|x| x.as_f64()),
                    date: dig(v, &["reportDate"]).and_then(|x| x.as_str().map(String::from)),
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Dig a nested JSON path, tolerating missing nodes. At the leaf, unwrap
/// yfinance's `{"raw": x, "fmt": ".."}` numeric wrapper. Explicit `raw` path
/// segments still resolve correctly (they already land on the raw value).
fn dig<'a>(v: &'a Value, path: &[&str]) -> Option<&'a Value> {
    let mut cur = v;
    for p in path {
        cur = cur.get(p)?;
    }
    if let Some(raw) = cur.get("raw") {
        return Some(raw);
    }
    Some(cur)
}
