//! Analysis / estimates: the six yfinance `get_*_estimate` tables, the
//! recommendation trend, upgrade/downgrade history, and valuation measures.
//!
//! The six estimate tables (`earnings` / `revenue` / `eps_trend` / `eps_revisions`
//! / `growth` / `earnings_history`) share one shape — a single module in
//! `quoteSummary` whose array is keyed by `period`/`quarter` with a fixed metric
//! column set. They are produced by one descriptor-driven [`parse_named_table`],
//! eliminating the six near-identical methods that previously lived in
//! `quote.rs`.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::http::YfSession;
use crate::json::{get, get_f64, get_i64, get_str};
use crate::quote::{quote_summary, ts_to_date};

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

/// Descriptor for one `quoteSummary`-backed estimate table.
struct EstimateTable<'a> {
    module: &'a str,
    array_key: &'a str,
    label_key: &'a str,
    metrics: &'a [&'a str],
}

/// The six estimate tables, all produced by the same parse path.
const ESTIMATE_TABLES: &[(&str, EstimateTable)] = &[
    (
        "earnings_estimate",
        EstimateTable {
            module: "earningsTrend",
            array_key: "earningsTrend",
            label_key: "period",
            metrics: &[
                "numberOfAnalysts",
                "avg",
                "low",
                "high",
                "yearAgo",
                "growth",
            ],
        },
    ),
    (
        "revenue_estimate",
        EstimateTable {
            module: "revenueTrend",
            array_key: "revenueTrend",
            label_key: "period",
            metrics: &[
                "numberOfAnalysts",
                "avg",
                "low",
                "high",
                "yearAgo",
                "growth",
            ],
        },
    ),
    (
        "earnings_history",
        EstimateTable {
            module: "earningsHistory",
            array_key: "history",
            label_key: "quarter",
            metrics: &[
                "epsEstimate",
                "epsActual",
                "epsDifference",
                "surprisePercent",
            ],
        },
    ),
    (
        "eps_trend",
        EstimateTable {
            module: "epsTrend",
            array_key: "epsTrend",
            label_key: "period",
            metrics: &["current", "7daysAgo", "30daysAgo", "60daysAgo", "90daysAgo"],
        },
    ),
    (
        "eps_revisions",
        EstimateTable {
            module: "epsRevisions",
            array_key: "epsRevisions",
            label_key: "period",
            metrics: &[
                "upLast7days",
                "upLast30days",
                "downLast7days",
                "downLast30days",
            ],
        },
    ),
    (
        "growth_estimates",
        EstimateTable {
            module: "growth",
            array_key: "growth",
            label_key: "period",
            metrics: &["stock", "industry", "sector", "index"],
        },
    ),
];

fn parse_named_table(
    result: &Value,
    module: &str,
    array_key: &str,
    label_key: &str,
    metrics: &[&str],
) -> NamedTable {
    let arr = get(result, &[module, array_key])
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let mut index = Vec::with_capacity(arr.len());
    let mut values = Vec::with_capacity(arr.len());
    for obj in &arr {
        let label = get(obj, &[label_key])
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let row: Vec<Option<f64>> = metrics.iter().map(|m| get_f64(obj, &[m])).collect();
        index.push(label);
        values.push(row);
    }
    NamedTable {
        index,
        columns: metrics.iter().map(|s| s.to_string()).collect(),
        values,
    }
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
    pub async fn earnings_estimate(&self, ticker: &str) -> crate::error::Result<NamedTable> {
        self.estimate_table(ticker, "earnings_estimate").await
    }

    /// Revenue estimates table (mirrors `get_revenue_estimate`).
    pub async fn revenue_estimate(&self, ticker: &str) -> crate::error::Result<NamedTable> {
        self.estimate_table(ticker, "revenue_estimate").await
    }

    /// Reported vs estimated EPS history (mirrors `get_earnings_history`).
    pub async fn earnings_history(&self, ticker: &str) -> crate::error::Result<NamedTable> {
        self.estimate_table(ticker, "earnings_history").await
    }

    /// EPS revision trend table (mirrors `get_eps_trend`).
    pub async fn eps_trend(&self, ticker: &str) -> crate::error::Result<NamedTable> {
        self.estimate_table(ticker, "eps_trend").await
    }

    /// EPS revisions table (mirrors `get_eps_revisions`).
    pub async fn eps_revisions(&self, ticker: &str) -> crate::error::Result<NamedTable> {
        self.estimate_table(ticker, "eps_revisions").await
    }

    /// Growth estimates table (mirrors `get_growth_estimates`).
    pub async fn growth_estimates(&self, ticker: &str) -> crate::error::Result<NamedTable> {
        self.estimate_table(ticker, "growth_estimates").await
    }

    async fn estimate_table(&self, ticker: &str, name: &str) -> crate::error::Result<NamedTable> {
        let desc = ESTIMATE_TABLES
            .iter()
            .find(|(n, _)| *n == name)
            .map(|(_, d)| d)
            .expect("estimate table name must be in ESTIMATE_TABLES");
        let r = quote_summary(self, ticker, &[desc.module]).await?;
        Ok(parse_named_table(
            &r,
            desc.module,
            desc.array_key,
            desc.label_key,
            desc.metrics,
        ))
    }

    /// Recommendation trend history.
    pub async fn recommendation_trend(
        &self,
        ticker: &str,
    ) -> crate::error::Result<Vec<RecommendationTrend>> {
        let result = quote_summary(self, ticker, &["recommendationTrend"]).await?;
        let arr = get(&result, &["recommendationTrend", "trend"]).and_then(|v| v.as_array());
        Ok(arr
            .map(|a| {
                a.iter()
                    .map(|v| RecommendationTrend {
                        period: get_str(v, &["period"]),
                        strong_buy: get_i64(v, &["strongBuy"]),
                        buy: get_i64(v, &["buy"]),
                        hold: get_i64(v, &["hold"]),
                        sell: get_i64(v, &["sell"]),
                        strong_sell: get_i64(v, &["strongSell"]),
                    })
                    .collect()
            })
            .unwrap_or_default())
    }

    /// Recommendation summary (alias of `recommendation_trend`, mirrors
    /// `get_recommendations` / `get_recommendations_summary`).
    pub async fn recommendations(
        &self,
        ticker: &str,
    ) -> crate::error::Result<Vec<RecommendationTrend>> {
        self.recommendation_trend(ticker).await
    }

    /// Upgrades / downgrades (rating changes), mirrors `get_upgrades_downgrades`.
    pub async fn upgrades_downgrades(
        &self,
        ticker: &str,
    ) -> crate::error::Result<Vec<UpgradesDowngrades>> {
        let r = quote_summary(self, ticker, &["upgradeDowngradeHistory"]).await?;
        let arr = get(&r, &["upgradeDowngradeHistory", "history"])
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        Ok(arr
            .iter()
            .map(|o| UpgradesDowngrades {
                date: ts_to_date(get_f64(o, &["date", "raw"])),
                firm: get_str(o, &["firm"]),
                to_grade: get_str(o, &["toGrade"]),
                from_grade: get_str(o, &["fromGrade"]),
                action: get_str(o, &["action"]),
            })
            .collect())
    }

    /// Valuation measures table (mirrors `get_valuation_measures`). A single
    /// `Current` column is returned; the period-history columns from yfinance
    /// are sourced from a separate time-series not in `quoteSummary`, so are
    /// omitted here.
    pub async fn valuation_measures(&self, ticker: &str) -> crate::error::Result<NamedTable> {
        let modules = [
            "price",
            "summaryDetail",
            "defaultKeyStatistics",
            "financialData",
        ];
        let r = quote_summary(self, ticker, &modules).await?;
        Ok(parse_valuation_measures(&r))
    }
}

fn parse_valuation_measures(result: &Value) -> NamedTable {
    let measures = [
        ("Market Cap", get_f64(result, &["price", "marketCap"])),
        (
            "Enterprise Value",
            get_f64(result, &["defaultKeyStatistics", "enterpriseValue"]),
        ),
        (
            "Trailing P/E",
            get_f64(result, &["summaryDetail", "trailingPE"]),
        ),
        (
            "Forward P/E",
            get_f64(result, &["summaryDetail", "forwardPE"]),
        ),
        (
            "PEG Ratio",
            get_f64(result, &["defaultKeyStatistics", "pegRatio"]),
        ),
        (
            "Price/Book",
            get_f64(result, &["defaultKeyStatistics", "priceToBook"]),
        ),
        (
            "Enterprise Value/Revenue",
            get_f64(
                result,
                &["defaultKeyStatistics", "enterpriseValueToRevenue"],
            ),
        ),
        (
            "Enterprise Value/EBITDA",
            get_f64(result, &["defaultKeyStatistics", "enterpriseValueToEbitda"]),
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

    #[test]
    fn estimate_tables_registered() {
        // Guard against silent drift: all six estimate tables must be present.
        let names: Vec<&str> = ESTIMATE_TABLES.iter().map(|(n, _)| *n).collect();
        assert_eq!(names.len(), 6);
        assert!(names.contains(&"earnings_estimate"));
        assert!(names.contains(&"growth_estimates"));
    }
}
