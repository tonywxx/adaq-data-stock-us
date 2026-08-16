//! Security holders (major / institutional / mutual-fund / insider).

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::http::YfSession;
use crate::json::{get, get_f64, get_str};
use crate::quote::quote_summary;

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

fn parse_holder_table(result: &Value, path: &[&str]) -> Vec<HolderRow> {
    get(result, path)
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .map(|v| HolderRow {
                    name: get_str(v, &["name"]),
                    pct_in_floats: get_f64(v, &["pctHeld", "raw"]),
                    position: get_f64(v, &["positionDirect", "raw"]),
                    value: get_f64(v, &["value", "raw"]),
                    date: get_str(v, &["reportDate"]),
                })
                .collect()
        })
        .unwrap_or_default()
}

impl YfSession {
    /// Holders (major / institutional / mutual-fund / insider).
    pub async fn holders(&self, ticker: &str) -> crate::error::Result<Holders> {
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_holder_table_unwraps_raw() {
        let r = serde_json::json!({
            "institutionOwnership": {"holders": [
                {"name": "Vanguard", "pctHeld": {"raw": 0.07}, "positionDirect": {"raw": 1.2e9}, "value": {"raw": 3.4e9}, "reportDate": "2024-01-01"}
            ]}
        });
        let rows = parse_holder_table(&r, &["institutionOwnership", "holders"]);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].name.as_deref(), Some("Vanguard"));
        assert_eq!(rows[0].pct_in_floats, Some(0.07));
        assert_eq!(rows[0].date.as_deref(), Some("2024-01-01"));
    }
}
