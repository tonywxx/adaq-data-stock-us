//! Per-ticker earnings dates (mirrors `Ticker.get_earnings_dates`).
//!
//! Uses Yahoo's `v1/finance/visualization` endpoint (the JSON path yfinance
//! falls back to after the HTML scrape endpoint stopped updating). See
//! `base.py::_get_earnings_dates_using_screener`.

use chrono::{DateTime, NaiveDateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::http::YfSession;
use crate::json::yf_result_first;

/// A single scheduled / reported earnings date for a ticker.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EarningsDate {
    pub date: Option<DateTime<Utc>>,
    pub eps_estimate: Option<f64>,
    pub eps_actual: Option<f64>,
    pub surprise_pct: Option<f64>,
    /// Raw event type code: `1`=call, `2`=earnings, `11`=meeting.
    pub event_type: Option<String>,
}

impl YfSession {
    /// Fetch up to `limit` (max 100) earnings dates for `ticker`, newest first
    /// (mirrors `get_earnings_dates`).
    pub async fn earnings_dates(&self, ticker: &str, limit: usize) -> Result<Vec<EarningsDate>> {
        if limit > 100 {
            return Err(crate::error::YfError::msg("Yahoo caps limit at 100"));
        }
        let urls = Self::urls();
        let url = format!("{}/v1/finance/visualization", urls.query1);
        let params = vec![
            ("lang", self.config().locale.lang.clone()),
            ("region", self.config().locale.region.clone()),
        ];
        let body = serde_json::json!({
            "size": limit,
            "query": {"operator": "eq", "operands": ["ticker", ticker]},
            "sortField": "startdatetime",
            "sortType": "DESC",
            "entityIdType": "earnings",
            "includeFields": [
                "startdatetime",
                "timeZoneShortName",
                "epsestimate",
                "epsactual",
                "epssurprisepct",
                "eventtype",
            ],
        });
        let v = self.post_json(&url, &params, &body).await?;
        let doc = yf_result_first(&v, "finance")
            .map_err(|_| {
                crate::error::YfError::DataMissing(format!("earnings dates missing for {ticker}"))
            })?
            .get("documents")
            .and_then(|d| d.as_array())
            .and_then(|a| a.first())
            .ok_or_else(|| {
                crate::error::YfError::DataMissing(format!("earnings dates missing for {ticker}"))
            })?;
        let empty: Vec<serde_json::Value> = Vec::new();
        let rows = doc.get("rows").and_then(|r| r.as_array()).unwrap_or(&empty);
        Ok(rows.iter().map(parse_row).collect())
    }
}

fn parse_row(row: &serde_json::Value) -> EarningsDate {
    let get = |k: &str| row.get(k);
    let f = |k: &str| get(k).and_then(|v| v.as_f64());
    EarningsDate {
        date: get("Event Start Date")
            .and_then(|v| v.as_str())
            .and_then(parse_datetime),
        eps_estimate: f("EPS Estimate"),
        eps_actual: f("Reported EPS"),
        surprise_pct: f("Surprise (%)"),
        event_type: get("Event Type").and_then(|v| v.as_str()).map(String::from),
    }
}

fn parse_datetime(s: &str) -> Option<DateTime<Utc>> {
    if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
        return Some(dt.with_timezone(&Utc));
    }
    if let Ok(naive) = NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S%.f") {
        return Some(DateTime::from_naive_utc_and_offset(naive, Utc));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_visualization_rows() {
        let row = json!({
            "Event Start Date": "2025-10-30T00:00:00.000Z",
            "Timezone short name": "EDT",
            "EPS Estimate": 2.97,
            "Reported EPS": 2.9,
            "Surprise (%)": -2.36,
            "Event Type": "1"
        });
        let e = parse_row(&row);
        assert!(e.date.is_some());
        assert_eq!(e.eps_estimate, Some(2.97));
        assert_eq!(e.eps_actual, Some(2.9));
        assert_eq!(e.surprise_pct, Some(-2.36));
        assert_eq!(e.event_type.as_deref(), Some("1"));
    }

    #[test]
    fn date_parser_handles_iso() {
        assert!(parse_datetime("2025-10-30T00:00:00.000Z").is_some());
        assert!(parse_datetime("not-a-date").is_none());
    }
}
