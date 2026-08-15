//! Domain objects: `Sector`, `Industry`, `Market` (mirrors yfinance's
//! `domain/` package).

use serde::{Deserialize, Serialize};

use crate::error::{Result, YfError};
use crate::http::YfSession;

/// Market region for [`Market`]. Mirrors yfinance's `MarketRegion`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum MarketRegion {
    #[default]
    Us,
    Gb,
    Asia,
    Europe,
    Rates,
    Commodities,
    Currencies,
    Cryptocurrencies,
}

impl MarketRegion {
    fn as_str(&self) -> &'static str {
        match self {
            MarketRegion::Us => "us",
            MarketRegion::Gb => "gb",
            MarketRegion::Asia => "asia",
            MarketRegion::Europe => "eu",
            MarketRegion::Rates => "rates",
            MarketRegion::Commodities => "commodities",
            MarketRegion::Currencies => "currencies",
            MarketRegion::Cryptocurrencies => "crypto",
        }
    }
}

/// A company referenced by a sector/industry.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Company {
    pub name: Option<String>,
    pub symbol: Option<String>,
    #[serde(rename = "quoteType")]
    pub quote_type: Option<String>,
}

/// A market sector (mirrors `Sector`).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Sector {
    pub name: Option<String>,
    pub symbol: Option<String>,
    pub ticker: Option<String>,
    pub overview: Option<String>,
    pub top_companies: Vec<Company>,
}

/// A market industry (mirrors `Industry`).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Industry {
    pub name: Option<String>,
    pub symbol: Option<String>,
    pub ticker: Option<String>,
    pub sector_key: Option<String>,
    pub sector_name: Option<String>,
    pub overview: Option<String>,
    pub top_companies: Vec<Company>,
    pub top_performing_companies: Vec<Company>,
    pub top_growth_companies: Vec<Company>,
}

/// One row of a market summary.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MarketSummaryRow {
    pub exchange: Option<String>,
    pub short_name: Option<String>,
    pub region: Option<String>,
    pub price: Option<f64>,
    pub change: Option<f64>,
    pub percent_change: Option<f64>,
}

/// A market snapshot (mirrors `Market`).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Market {
    pub region: MarketRegion,
    pub status: Option<String>,
    pub summary: Vec<MarketSummaryRow>,
}

fn finance_result(v: &serde_json::Value) -> Option<&serde_json::Value> {
    v.get("finance").and_then(|f| f.get("result"))
}

fn companies_from(v: &serde_json::Value) -> Vec<Company> {
    v.get("companies")
        .and_then(|c| c.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|q| serde_json::from_value::<Company>(q.clone()).ok())
                .collect()
        })
        .unwrap_or_default()
}

impl YfSession {
    /// Fetch a sector by key (e.g. `"technology"`).
    pub async fn sector(&self, key: &str) -> Result<Sector> {
        let url = format!("{}/v1/finance/sectors/{}", Self::urls().query1, key);
        let v = self.get_json(&url, &[]).await?;
        let r = finance_result(&v).ok_or_else(|| YfError::DataMissing(format!("sector {key}")))?;
        Ok(Sector {
            name: dig_str(r, &["name"]),
            symbol: dig_str(r, &["symbol"]),
            ticker: dig_str(r, &["ticker"]),
            overview: dig_str(r, &["description"]),
            top_companies: r
                .get("topCompanies")
                .map(companies_from)
                .unwrap_or_default(),
        })
    }

    /// Fetch an industry by key.
    pub async fn industry(&self, key: &str) -> Result<Industry> {
        let url = format!("{}/v1/finance/industries/{}", Self::urls().query1, key);
        let v = self.get_json(&url, &[]).await?;
        let r =
            finance_result(&v).ok_or_else(|| YfError::DataMissing(format!("industry {key}")))?;
        Ok(Industry {
            name: dig_str(r, &["name"]),
            symbol: dig_str(r, &["symbol"]),
            ticker: dig_str(r, &["ticker"]),
            sector_key: dig_str(r, &["sectorKey"]),
            sector_name: dig_str(r, &["sectorName"]),
            overview: dig_str(r, &["description"]),
            top_companies: r
                .get("topCompanies")
                .map(companies_from)
                .unwrap_or_default(),
            top_performing_companies: r
                .get("topPerformingCompanies")
                .map(companies_from)
                .unwrap_or_default(),
            top_growth_companies: r
                .get("topGrowthCompanies")
                .map(companies_from)
                .unwrap_or_default(),
        })
    }

    /// Fetch a market snapshot for a region.
    pub async fn market(&self, region: MarketRegion) -> Result<Market> {
        let urls = Self::urls();
        let summary_url = format!("{}/v6/finance/quote/marketSummary", urls.query1);
        let time_url = format!("{}/v6/finance/markettime", urls.query1);
        let params = vec![("region", region.as_str().to_string())];

        let v = self.get_json(&summary_url, &params).await?;
        let summary = v
            .get("marketSummaryResponse")
            .and_then(|m| m.get("result"))
            .and_then(|r| r.as_array())
            .map(|a| {
                a.iter()
                    .map(|row| MarketSummaryRow {
                        exchange: dig_str(row, &["exchange"]),
                        short_name: dig_str(row, &["shortName"]),
                        region: dig_str(row, &["region"]),
                        price: dig_f64(row, &["regularMarketPrice", "raw"])
                            .or_else(|| dig_f64(row, &["price", "raw"])),
                        change: dig_f64(row, &["regularMarketChange", "raw"]),
                        percent_change: dig_f64(row, &["regularMarketChangePercent", "raw"]),
                    })
                    .collect()
            })
            .unwrap_or_default();

        let mut status = None;
        if let Ok(t) = self.get_json(&time_url, &params).await {
            status = t
                .get("marketTime")
                .and_then(|m| m.get(region.as_str()))
                .and_then(|m| m.get("status"))
                .and_then(|s| s.as_str())
                .map(String::from);
        }

        Ok(Market {
            region,
            status,
            summary,
        })
    }
}

fn dig_str(v: &serde_json::Value, path: &[&str]) -> Option<String> {
    let mut cur = v;
    for p in path {
        cur = cur.get(p)?;
    }
    cur.as_str().map(String::from)
}

fn dig_f64(v: &serde_json::Value, path: &[&str]) -> Option<f64> {
    let mut cur = v;
    for p in path {
        cur = cur.get(p)?;
    }
    if let Some(raw) = cur.get("raw") {
        return raw.as_f64();
    }
    cur.as_f64()
}
