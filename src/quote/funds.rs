//! Mutual-fund / ETF data (mirrors `Ticker.get_funds_data`).

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::http::YfSession;
use crate::json::{get, get_f64, get_str};
use crate::quote::{quote_summary, ts_to_date};

/// Mutual-fund / ETF data (mirrors `Ticker.get_funds_data`).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FundsData {
    pub family: Option<String>,
    pub category_name: Option<String>,
    pub legal_type: Option<String>,
    pub fund_inception_date: Option<chrono::DateTime<chrono::Utc>>,
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

impl YfSession {
    /// Mutual-fund / ETF data (mirrors `get_funds_data`).
    pub async fn funds_data(&self, ticker: &str) -> crate::error::Result<FundsData> {
        let modules = [
            "fundProfile",
            "fundOverview",
            "topHoldings",
            "fundPerformance",
        ];
        let r = quote_summary(self, ticker, &modules).await?;
        let fp = get(&r, &["fundProfile"]).unwrap_or(&Value::Null);
        let th = get(&r, &["topHoldings"]).unwrap_or(&Value::Null);
        let f = |p: &[&str]| get_f64(&r, p);
        let fs = |p: &[&str]| get_str(&r, p);
        let sector_weightings = get(fp, &["sectorWeightings"])
            .and_then(|v| v.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|s| {
                        let (sector, weight) = s.as_object()?.iter().next()?;
                        Some((sector.clone(), get_f64(weight, &[])))
                    })
                    .collect()
            })
            .unwrap_or_default();
        let top_holdings = get(th, &["holdings"])
            .and_then(|v| v.as_array())
            .map(|a| {
                a.iter()
                    .map(|h| FundHolding {
                        symbol: get_str(h, &["symbol"]),
                        name: get_str(h, &["holdingName"]),
                        holding_percent: get_f64(h, &["holdingPercent", "raw"]),
                        value: get_f64(h, &["value", "raw"]),
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
