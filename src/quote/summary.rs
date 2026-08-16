//! `Sustainability` / ESG scores and analyst price targets.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::http::YfSession;
use crate::json::{get, get_f64};
use crate::quote::quote_summary;

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

impl YfSession {
    /// Sustainability / ESG.
    pub async fn sustainability(&self, ticker: &str) -> crate::error::Result<Sustainability> {
        let result = quote_summary(self, ticker, &["esgScores"]).await?;
        let esg = get(&result, &["esgScores"]).unwrap_or(&Value::Null);
        let f = |p: &[&str]| get_f64(esg, p);
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
    pub async fn analyst_price_targets(
        &self,
        ticker: &str,
    ) -> crate::error::Result<AnalystPriceTargets> {
        let result = quote_summary(self, ticker, &["financialData", "price"]).await?;
        let fd = get(&result, &["financialData"]).unwrap_or(&Value::Null);
        let f = |p: &[&str]| get_f64(fd, p);
        Ok(AnalystPriceTargets {
            current: f(&["currentPrice", "raw"]),
            low: f(&["targetLowPrice", "raw"]),
            high: f(&["targetHighPrice", "raw"]),
            mean: f(&["targetMeanPrice", "raw"]),
            median: f(&["targetMedianPrice", "raw"]),
            num_analysts: f(&["numberOfAnalystOpinions", "raw"]),
        })
    }
}
