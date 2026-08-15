//! Domain objects: `Sector`, `Industry`, `Market` (mirrors yfinance's
//! `domain/` package).

use serde::{Deserialize, Serialize};

use crate::error::{Result, YfError};
use crate::http::YfSession;
use crate::json::{get_f64, get_str, yf_result};

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
        let r =
            yf_result(&v, "finance").map_err(|_| YfError::DataMissing(format!("sector {key}")))?;
        Ok(Sector {
            name: get_str(r, &["name"]),
            symbol: get_str(r, &["symbol"]),
            ticker: get_str(r, &["ticker"]),
            overview: get_str(r, &["description"]),
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
        let r = yf_result(&v, "finance")
            .map_err(|_| YfError::DataMissing(format!("industry {key}")))?;
        Ok(Industry {
            name: get_str(r, &["name"]),
            symbol: get_str(r, &["symbol"]),
            ticker: get_str(r, &["ticker"]),
            sector_key: get_str(r, &["sectorKey"]),
            sector_name: get_str(r, &["sectorName"]),
            overview: get_str(r, &["description"]),
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
        let summary = yf_result(&v, "marketSummaryResponse")
            .ok()
            .and_then(|r| r.as_array())
            .map(|a| {
                a.iter()
                    .map(|row| MarketSummaryRow {
                        exchange: get_str(row, &["exchange"]),
                        short_name: get_str(row, &["shortName"]),
                        region: get_str(row, &["region"]),
                        price: get_f64(row, &["regularMarketPrice"])
                            .or_else(|| get_f64(row, &["price"])),
                        change: get_f64(row, &["regularMarketChange"]),
                        percent_change: get_f64(row, &["regularMarketChangePercent"]),
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
