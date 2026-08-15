//! Lookup (mirrors yfinance's `Lookup`).

use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::http::YfSession;

const LOOKUP_URL: &str = "https://query1.finance.yahoo.com/v1/finance/lookup";

/// One lookup result row.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LookupRow {
    pub symbol: Option<String>,
    pub name: Option<String>,
    #[serde(rename = "exch")]
    pub exchange: Option<String>,
    #[serde(rename = "typeDisp")]
    pub type_disp: Option<String>,
    #[serde(rename = "industryDisp")]
    pub industry: Option<String>,
}

/// Lookup results for a query.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LookupResult {
    pub query: String,
    pub results: Vec<LookupRow>,
}

impl YfSession {
    /// Look up securities by query. `lookup_type` is e.g. `"all"`, `"equity"`,
    /// `"etf"`, `"mutualfund"`, `"index"`, `"future"`, `"currency"`,
    /// `"cryptocurrency"`.
    pub async fn lookup(
        &self,
        query: &str,
        limit: usize,
        lookup_type: &str,
    ) -> Result<LookupResult> {
        let params = vec![
            ("query", query.to_string()),
            ("count", limit.to_string()),
            ("type", lookup_type.to_string()),
        ];
        let v = self.get_json(LOOKUP_URL, &params).await?;
        let results = v
            .get("finance")
            .and_then(|f| f.get("result"))
            .and_then(|r| r.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|q| serde_json::from_value::<LookupRow>(q.clone()).ok())
                    .collect()
            })
            .unwrap_or_default();
        Ok(LookupResult {
            query: query.to_string(),
            results,
        })
    }
}
