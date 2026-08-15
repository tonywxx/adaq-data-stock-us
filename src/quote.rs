//! `quoteSummary` family: `info`, `fast_info`, holders, analysis, sustainability.
//!
//! Mirrors yfinance's `scrapers/quote.py` + `holders.py` + `analysis.py` +
//! `funds.py`. The `v10/finance/quoteSummary` endpoint takes a `modules` list;
//! we request the relevant modules and parse typed structs. The full raw JSON
//! is retained on [`Info::raw`] so no upstream field is lost.

use chrono::{DateTime, NaiveDate, Utc};
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

// ---- Analysis / estimates (mirrors base.py get_*_estimate etc.) ----

/// A generic labelled table (rows indexed by `index`, columns by `columns`),
/// mirroring the DataFrame shape yfinance returns for estimates and trends.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NamedTable {
    pub index: Vec<String>,
    pub columns: Vec<String>,
    pub values: Vec<Vec<Option<f64>>>,
}

impl NamedTable {
    /// Look up a cell by row label and column name.
    pub fn get(&self, row: &str, col: &str) -> Option<f64> {
        let ri = self.index.iter().position(|r| r == row)?;
        let ci = self.columns.iter().position(|c| c == col)?;
        self.values
            .get(ri)
            .and_then(|r| r.get(ci))
            .copied()
            .flatten()
    }
}

fn parse_named_table(
    result: &Value,
    module: &str,
    array_key: &str,
    label_key: &str,
    metrics: &[&str],
) -> NamedTable {
    let arr = dig(result, &[module, array_key])
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let mut index = Vec::with_capacity(arr.len());
    let mut values = Vec::with_capacity(arr.len());
    for obj in &arr {
        let label = dig(obj, &[label_key])
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let row: Vec<Option<f64>> = metrics
            .iter()
            .map(|m| dig(obj, &[m]).and_then(|v| v.as_f64()))
            .collect();
        index.push(label);
        values.push(row);
    }
    NamedTable {
        index,
        columns: metrics.iter().map(|s| s.to_string()).collect(),
        values,
    }
}

fn ts_to_date(sec: Option<f64>) -> Option<DateTime<Utc>> {
    sec.and_then(|s| DateTime::from_timestamp(s as i64, 0))
}

/// A single upgrades/downgrades (rating change) event.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UpgradesDowngrades {
    pub date: Option<DateTime<Utc>>,
    pub firm: Option<String>,
    pub to_grade: Option<String>,
    pub from_grade: Option<String>,
    pub action: Option<String>,
}

impl YfSession {
    /// Earnings estimates table (mirrors `get_earnings_estimate`).
    pub async fn earnings_estimate(&self, ticker: &str) -> Result<NamedTable> {
        let r = quote_summary(self, ticker, &["earningsTrend"]).await?;
        Ok(parse_named_table(
            &r,
            "earningsTrend",
            "earningsTrend",
            "period",
            &[
                "numberOfAnalysts",
                "avg",
                "low",
                "high",
                "yearAgo",
                "growth",
            ],
        ))
    }

    /// Revenue estimates table (mirrors `get_revenue_estimate`).
    pub async fn revenue_estimate(&self, ticker: &str) -> Result<NamedTable> {
        let r = quote_summary(self, ticker, &["revenueTrend"]).await?;
        Ok(parse_named_table(
            &r,
            "revenueTrend",
            "revenueTrend",
            "period",
            &[
                "numberOfAnalysts",
                "avg",
                "low",
                "high",
                "yearAgo",
                "growth",
            ],
        ))
    }

    /// Reported vs estimated EPS history (mirrors `get_earnings_history`).
    pub async fn earnings_history(&self, ticker: &str) -> Result<NamedTable> {
        let r = quote_summary(self, ticker, &["earningsHistory"]).await?;
        Ok(parse_named_table(
            &r,
            "earningsHistory",
            "history",
            "quarter",
            &[
                "epsEstimate",
                "epsActual",
                "epsDifference",
                "surprisePercent",
            ],
        ))
    }

    /// EPS revision trend table (mirrors `get_eps_trend`).
    pub async fn eps_trend(&self, ticker: &str) -> Result<NamedTable> {
        let r = quote_summary(self, ticker, &["epsTrend"]).await?;
        Ok(parse_named_table(
            &r,
            "epsTrend",
            "epsTrend",
            "period",
            &["current", "7daysAgo", "30daysAgo", "60daysAgo", "90daysAgo"],
        ))
    }

    /// EPS revisions table (mirrors `get_eps_revisions`).
    pub async fn eps_revisions(&self, ticker: &str) -> Result<NamedTable> {
        let r = quote_summary(self, ticker, &["epsRevisions"]).await?;
        Ok(parse_named_table(
            &r,
            "epsRevisions",
            "epsRevisions",
            "period",
            &[
                "upLast7days",
                "upLast30days",
                "downLast7days",
                "downLast30days",
            ],
        ))
    }

    /// Growth estimates table (mirrors `get_growth_estimates`).
    pub async fn growth_estimates(&self, ticker: &str) -> Result<NamedTable> {
        let r = quote_summary(self, ticker, &["growth"]).await?;
        Ok(parse_named_table(
            &r,
            "growth",
            "growth",
            "period",
            &["stock", "industry", "sector", "index"],
        ))
    }

    /// Recommendation summary (alias of `recommendation_trend`, mirrors
    /// `get_recommendations` / `get_recommendations_summary`).
    pub async fn recommendations(&self, ticker: &str) -> Result<Vec<RecommendationTrend>> {
        self.recommendation_trend(ticker).await
    }

    /// Upgrades / downgrades (rating changes), mirrors `get_upgrades_downgrades`.
    pub async fn upgrades_downgrades(&self, ticker: &str) -> Result<Vec<UpgradesDowngrades>> {
        let r = quote_summary(self, ticker, &["upgradeDowngradeHistory"]).await?;
        let arr = dig(&r, &["upgradeDowngradeHistory", "history"])
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        Ok(arr
            .iter()
            .map(|o| UpgradesDowngrades {
                date: ts_to_date(dig(o, &["date", "raw"]).and_then(|v| v.as_f64())),
                firm: dig(o, &["firm"]).and_then(|v| v.as_str()).map(String::from),
                to_grade: dig(o, &["toGrade"])
                    .and_then(|v| v.as_str())
                    .map(String::from),
                from_grade: dig(o, &["fromGrade"])
                    .and_then(|v| v.as_str())
                    .map(String::from),
                action: dig(o, &["action"])
                    .and_then(|v| v.as_str())
                    .map(String::from),
            })
            .collect())
    }

    /// Valuation measures table (mirrors `get_valuation_measures`). A single
    /// `Current` column is returned; the period-history columns from yfinance
    /// are sourced from a separate time-series not in `quoteSummary`, so are
    /// omitted here.
    pub async fn valuation_measures(&self, ticker: &str) -> Result<NamedTable> {
        let modules = [
            "price",
            "summaryDetail",
            "defaultKeyStatistics",
            "financialData",
        ];
        let r = quote_summary(self, ticker, &modules).await?;
        Ok(parse_valuation_measures(&r))
    }

    /// Earnings / dividend calendar (mirrors `get_calendar`).
    pub async fn ticker_calendar(&self, ticker: &str) -> Result<Calendar> {
        let r = quote_summary(self, ticker, &["calendarEvents"]).await?;
        Ok(parse_calendar_events(&r))
    }

    /// SEC filings (mirrors `get_sec_filings`).
    pub async fn sec_filings(&self, ticker: &str) -> Result<Vec<SecFiling>> {
        let r = quote_summary(self, ticker, &["secFilings"]).await?;
        Ok(parse_sec_filings(&r))
    }

    /// Current shares outstanding (mirrors `get_shares`).
    pub async fn shares(&self, ticker: &str) -> Result<Option<f64>> {
        let info = self.info(ticker).await?;
        Ok(info.shares_outstanding)
    }

    /// Full share-count time series (mirrors `get_shares_full`).
    pub async fn shares_full(
        &self,
        ticker: &str,
        start: Option<DateTime<Utc>>,
        end: Option<DateTime<Utc>>,
    ) -> Result<Vec<(DateTime<Utc>, f64)>> {
        let urls = Self::urls();
        let url = format!(
            "{}/ws/fundamentals-timeseries/v1/finance/timeseries/{}",
            urls.query2, ticker
        );
        let now = Utc::now();
        let end = end.unwrap_or(now);
        let start = start.unwrap_or_else(|| end - chrono::Duration::days(548));
        let params = vec![
            ("symbol", ticker.to_string()),
            ("period1", start.timestamp().to_string()),
            ("period2", end.timestamp().to_string()),
        ];
        let v = self.get_json(&url, &params).await?;
        let series = v
            .get("timeseries")
            .and_then(|t| t.get("result"))
            .and_then(|r| r.as_array())
            .and_then(|a| a.first())
            .and_then(|r| r.get("shares_out"))
            .and_then(|s| s.as_array())
            .cloned()
            .unwrap_or_default();
        let mut out = Vec::with_capacity(series.len());
        for point in &series {
            let ts = point.get("timestamp").and_then(|x| x.as_i64());
            let val = point.get("value").and_then(|x| x.as_f64());
            if let (Some(ts), Some(val)) = (ts, val)
                && let Some(dt) = DateTime::from_timestamp(ts, 0)
            {
                out.push((dt, val));
            }
        }
        out.sort_by_key(|(dt, _)| *dt);
        Ok(out)
    }

    /// Mutual-fund / ETF data (mirrors `get_funds_data`).
    pub async fn funds_data(&self, ticker: &str) -> Result<FundsData> {
        let modules = [
            "fundProfile",
            "fundOverview",
            "topHoldings",
            "fundPerformance",
        ];
        let r = quote_summary(self, ticker, &modules).await?;
        let fp = dig(&r, &["fundProfile"]).unwrap_or(&Value::Null);
        let th = dig(&r, &["topHoldings"]).unwrap_or(&Value::Null);
        let f = |p: &[&str]| dig(&r, p).and_then(|v| v.as_f64());
        let fs = |p: &[&str]| dig(&r, p).and_then(|v| v.as_str()).map(String::from);
        let sector_weightings = dig(fp, &["sectorWeightings"])
            .and_then(|v| v.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|s| {
                        let (sector, weight) = s.as_object()?.iter().next()?;
                        Some((
                            sector.clone(),
                            weight
                                .get("raw")
                                .and_then(|x| x.as_f64())
                                .or_else(|| weight.as_f64()),
                        ))
                    })
                    .collect()
            })
            .unwrap_or_default();
        let top_holdings = dig(th, &["holdings"])
            .and_then(|v| v.as_array())
            .map(|a| {
                a.iter()
                    .map(|h| FundHolding {
                        symbol: dig(h, &["symbol"])
                            .and_then(|x| x.as_str())
                            .map(String::from),
                        name: dig(h, &["holdingName"])
                            .and_then(|x| x.as_str())
                            .map(String::from),
                        holding_percent: dig(h, &["holdingPercent", "raw"])
                            .and_then(|x| x.as_f64()),
                        value: dig(h, &["value", "raw"]).and_then(|x| x.as_f64()),
                    })
                    .collect()
            })
            .unwrap_or_default();
        Ok(FundsData {
            family: fs(&["fundProfile", "family"]),
            category_name: fs(&["fundProfile", "categoryName"]),
            legal_type: fs(&["fundProfile", "legalType"]),
            fund_inception_date: ts_to_date(f(&["fundProfile", "fundInceptionDate", "raw"])),
            nav_price: f(&["fundOverview", "navPrice", "raw"]),
            total_assets: f(&["fundOverview", "totalAssets", "raw"]),
            expense_ratio: f(&["fundOverview", "expenseRatio", "raw"]),
            ytd_return: f(&["fundPerformance", "ytdReturn", "raw"]),
            trailing_return_y1: f(&["fundPerformance", "trailingReturnYTD", "raw"]),
            sector_weightings,
            top_holdings,
            raw: r.clone(),
        })
    }
}

/// Earnings / dividend calendar (mirrors `Ticker.get_calendar`).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Calendar {
    pub earnings_date: Option<DateTime<Utc>>,
    pub earnings_time: Option<String>,
    pub eps_estimate: Option<f64>,
    pub revenue_estimate: Option<f64>,
    pub ex_dividend_date: Option<DateTime<Utc>>,
    pub dividend_date: Option<DateTime<Utc>>,
    pub previous_fiscal_year_end: Option<DateTime<Utc>>,
    pub next_fiscal_year_end: Option<DateTime<Utc>>,
    pub most_recent_quarter: Option<DateTime<Utc>>,
    pub next_quarter: Option<DateTime<Utc>>,
}

/// A single SEC filing (mirrors `Ticker.get_sec_filings`).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SecFiling {
    pub date: Option<DateTime<Utc>>,
    pub type_: Option<String>,
    pub title: Option<String>,
    pub url: Option<String>,
    pub editor: Option<String>,
}

/// Mutual-fund / ETF data (mirrors `Ticker.get_funds_data`).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FundsData {
    pub family: Option<String>,
    pub category_name: Option<String>,
    pub legal_type: Option<String>,
    pub fund_inception_date: Option<DateTime<Utc>>,
    pub nav_price: Option<f64>,
    pub total_assets: Option<f64>,
    pub expense_ratio: Option<f64>,
    pub ytd_return: Option<f64>,
    pub trailing_return_y1: Option<f64>,
    pub sector_weightings: Vec<(String, Option<f64>)>,
    pub top_holdings: Vec<FundHolding>,
    pub raw: Value,
}

/// A top holding within [`FundsData`].
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FundHolding {
    pub symbol: Option<String>,
    pub name: Option<String>,
    pub holding_percent: Option<f64>,
    pub value: Option<f64>,
}

fn parse_calendar_events(result: &Value) -> Calendar {
    let ce = dig(result, &["calendarEvents"]).unwrap_or(&Value::Null);
    let earnings = dig(ce, &["earnings"]).unwrap_or(&Value::Null);
    let dt = |p: &[&str]| ts_to_date(dig(ce, p).and_then(|v| v.as_f64()));
    Calendar {
        earnings_date: dt(&["earnings", "earningsDate", "raw"]).or_else(|| {
            ts_to_date(dig(earnings, &["earningsDate", "raw"]).and_then(|v| v.as_f64()))
        }),
        earnings_time: dig(ce, &["earnings", "earningsTime"])
            .and_then(|v| v.as_str())
            .or_else(|| dig(earnings, &["earningsTime"]).and_then(|v| v.as_str()))
            .map(String::from),
        eps_estimate: dig(earnings, &["epsEstimate", "raw"]).and_then(|v| v.as_f64()),
        revenue_estimate: dig(earnings, &["revenueEstimate", "raw"]).and_then(|v| v.as_f64()),
        ex_dividend_date: dt(&["exDividendDate", "raw"]),
        dividend_date: dt(&["dividendDate", "raw"]),
        previous_fiscal_year_end: dt(&["previousFiscalYearEnd", "raw"]),
        next_fiscal_year_end: dt(&["nextFiscalYearEnd", "raw"]),
        most_recent_quarter: dt(&["mostRecentQuarter", "raw"]),
        next_quarter: dt(&["nextQuarter", "raw"]),
    }
}

fn parse_sec_filings(result: &Value) -> Vec<SecFiling> {
    let arr = dig(result, &["secFilings", "filings"])
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    arr.iter()
        .map(|o| {
            // secFilings dates are date strings ("YYYY-MM-DD"), not epoch seconds.
            let date = dig(o, &["date", "raw"])
                .and_then(|v| v.as_str())
                .and_then(|s| {
                    NaiveDate::parse_from_str(s, "%Y-%m-%d")
                        .ok()
                        .and_then(|d| d.and_hms_opt(0, 0, 0))
                        .map(|naive| DateTime::from_naive_utc_and_offset(naive, Utc))
                });
            SecFiling {
                date,
                type_: dig(o, &["type"]).and_then(|v| v.as_str()).map(String::from),
                title: dig(o, &["title"])
                    .and_then(|v| v.as_str())
                    .map(String::from),
                url: dig(o, &["url"]).and_then(|v| v.as_str()).map(String::from),
                editor: dig(o, &["editor"])
                    .and_then(|v| v.as_str())
                    .map(String::from),
            }
        })
        .collect()
}

fn parse_valuation_measures(result: &Value) -> NamedTable {
    let measures = [
        (
            "Market Cap",
            dig(result, &["price", "marketCap"]).and_then(|v| v.as_f64()),
        ),
        (
            "Enterprise Value",
            dig(result, &["defaultKeyStatistics", "enterpriseValue"]).and_then(|v| v.as_f64()),
        ),
        (
            "Trailing P/E",
            dig(result, &["summaryDetail", "trailingPE"]).and_then(|v| v.as_f64()),
        ),
        (
            "Forward P/E",
            dig(result, &["summaryDetail", "forwardPE"]).and_then(|v| v.as_f64()),
        ),
        (
            "PEG Ratio",
            dig(result, &["defaultKeyStatistics", "pegRatio"]).and_then(|v| v.as_f64()),
        ),
        (
            "Price/Book",
            dig(result, &["defaultKeyStatistics", "priceToBook"]).and_then(|v| v.as_f64()),
        ),
        (
            "Enterprise Value/Revenue",
            dig(
                result,
                &["defaultKeyStatistics", "enterpriseValueToRevenue"],
            )
            .and_then(|v| v.as_f64()),
        ),
        (
            "Enterprise Value/EBITDA",
            dig(result, &["defaultKeyStatistics", "enterpriseValueToEbitda"])
                .and_then(|v| v.as_f64()),
        ),
    ];
    let index = measures.iter().map(|(m, _)| m.to_string()).collect();
    let values = measures.iter().map(|(_, v)| vec![*v]).collect();
    NamedTable {
        index,
        columns: vec!["Current".to_string()],
        values,
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn named_table_lookup() {
        let t = parse_named_table(
            &json!({"earningsTrend": {"earningsTrend": [
                {"period": "0q", "avg": {"raw": 1.5}, "high": {"raw": 2.0}},
                {"period": "0y", "avg": {"raw": 3.0}}
            ]}}),
            "earningsTrend",
            "earningsTrend",
            "period",
            &["avg", "high"],
        );
        assert_eq!(t.index, vec!["0q", "0y"]);
        assert_eq!(t.columns, vec!["avg", "high"]);
        assert_eq!(t.get("0q", "avg"), Some(1.5));
        assert_eq!(t.get("0q", "high"), Some(2.0));
        assert_eq!(t.get("0y", "high"), None);
        assert_eq!(t.get("missing", "avg"), None);
    }

    #[test]
    fn parses_calendar_events() {
        let r = json!({"calendarEvents": {
            "earnings": {"earningsDate": {"raw": 1700000000, "fmt": "2023-11-01"}, "earningsTime": "amc", "epsEstimate": {"raw": 1.2}},
            "exDividendDate": {"raw": 1690000000},
            "dividendDate": {"raw": 1695000000}
        }});
        let c = parse_calendar_events(&r);
        assert!(c.earnings_date.is_some());
        assert_eq!(c.earnings_time.as_deref(), Some("amc"));
        assert_eq!(c.eps_estimate, Some(1.2));
        assert!(c.ex_dividend_date.is_some());
    }

    #[test]
    fn parses_sec_filings_dates() {
        let r = json!({"secFilings": {"filings": [
            {"date": {"raw": "2023-01-15", "fmt": "Jan 15, 2023"}, "type": "10-K", "title": "Annual report"}
        ]}});
        let f = parse_sec_filings(&r);
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].type_.as_deref(), Some("10-K"));
        assert!(f[0].date.is_some());
    }

    #[test]
    fn parses_valuation_measures() {
        let r = json!({
            "price": {"marketCap": {"raw": 3_000_000_000_000.0}},
            "summaryDetail": {"trailingPE": {"raw": 30.0}},
            "defaultKeyStatistics": {"enterpriseValue": {"raw": 2_900_000_000_000.0}, "pegRatio": {"raw": 2.1}}
        });
        let v = parse_valuation_measures(&r);
        assert_eq!(v.columns, vec!["Current".to_string()]);
        assert_eq!(v.get("Market Cap", "Current"), Some(3_000_000_000_000.0));
        assert_eq!(v.get("Trailing P/E", "Current"), Some(30.0));
        assert_eq!(v.get("PEG Ratio", "Current"), Some(2.1));
    }
}
