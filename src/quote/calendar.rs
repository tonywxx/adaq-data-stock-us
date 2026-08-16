//! Earnings / dividend calendar and SEC filings (mirrors yfinance's
//! `Ticker.get_calendar` / `Ticker.get_sec_filings`).

use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::http::YfSession;
use crate::json::{get, get_f64, get_str};
use crate::quote::{quote_summary, ts_to_date};

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

fn parse_calendar_events(result: &Value) -> Calendar {
    let ce = get(result, &["calendarEvents"]).unwrap_or(&Value::Null);
    let earnings = get(ce, &["earnings"]).unwrap_or(&Value::Null);
    let dt = |p: &[&str]| ts_to_date(get_f64(ce, p));
    Calendar {
        earnings_date: dt(&["earnings", "earningsDate", "raw"])
            .or_else(|| ts_to_date(get_f64(earnings, &["earningsDate", "raw"]))),
        earnings_time: get_str(ce, &["earnings", "earningsTime"])
            .or_else(|| get_str(earnings, &["earningsTime"])),
        eps_estimate: get_f64(earnings, &["epsEstimate", "raw"]),
        revenue_estimate: get_f64(earnings, &["revenueEstimate", "raw"]),
        ex_dividend_date: dt(&["exDividendDate", "raw"]),
        dividend_date: dt(&["dividendDate", "raw"]),
        previous_fiscal_year_end: dt(&["previousFiscalYearEnd", "raw"]),
        next_fiscal_year_end: dt(&["nextFiscalYearEnd", "raw"]),
        most_recent_quarter: dt(&["mostRecentQuarter", "raw"]),
        next_quarter: dt(&["nextQuarter", "raw"]),
    }
}

fn parse_sec_filings(result: &Value) -> Vec<SecFiling> {
    let arr = get(result, &["secFilings", "filings"])
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    arr.iter()
        .map(|o| {
            // secFilings dates are date strings ("YYYY-MM-DD"), not epoch seconds.
            let date = get_str(o, &["date", "raw"]).as_deref().and_then(|s| {
                NaiveDate::parse_from_str(s, "%Y-%m-%d")
                    .ok()
                    .and_then(|d| d.and_hms_opt(0, 0, 0))
                    .map(|naive| DateTime::from_naive_utc_and_offset(naive, Utc))
            });
            SecFiling {
                date,
                type_: get_str(o, &["type"]),
                title: get_str(o, &["title"]),
                url: get_str(o, &["url"]),
                editor: get_str(o, &["editor"]),
            }
        })
        .collect()
}

impl YfSession {
    /// Earnings / dividend calendar (mirrors `get_calendar`).
    pub async fn ticker_calendar(&self, ticker: &str) -> crate::error::Result<Calendar> {
        let r = quote_summary(self, ticker, &["calendarEvents"]).await?;
        Ok(parse_calendar_events(&r))
    }

    /// SEC filings (mirrors `get_sec_filings`).
    pub async fn sec_filings(&self, ticker: &str) -> crate::error::Result<Vec<SecFiling>> {
        let r = quote_summary(self, ticker, &["secFilings"]).await?;
        Ok(parse_sec_filings(&r))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

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
}
