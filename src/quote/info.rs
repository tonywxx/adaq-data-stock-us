//! `Info` and `FastInfo` — the flattened security-information blobs.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::http::YfSession;
use crate::json::{get_f64, get_str};
use crate::quote::MODULES_INFO;

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
        Info {
            symbol: get_str(result, &["price", "symbol"]),
            short_name: get_str(result, &["price", "shortName"]),
            long_name: get_str(result, &["price", "longName"]),
            exchange: get_str(result, &["price", "exchangeName"]),
            quote_type: get_str(result, &["quoteType", "quoteType"]),
            sector: get_str(result, &["assetProfile", "sector"]),
            industry: get_str(result, &["assetProfile", "industry"]),
            currency: get_str(result, &["price", "currency"]),
            market_cap: get_f64(result, &["price", "marketCap"]),
            trailing_pe: get_f64(result, &["summaryDetail", "trailingPE"]),
            forward_pe: get_f64(result, &["summaryDetail", "forwardPE"]),
            price: get_f64(result, &["financialData", "currentPrice", "raw"])
                .or_else(|| get_f64(result, &["price", "regularMarketPrice", "raw"])),
            previous_close: get_f64(result, &["price", "regularMarketPreviousClose", "raw"]),
            regular_market_price: get_f64(result, &["price", "regularMarketPrice", "raw"]),
            fifty_two_week_high: get_f64(result, &["price", "fiftyTwoWeekHigh", "raw"]),
            fifty_two_week_low: get_f64(result, &["price", "fiftyTwoWeekLow", "raw"]),
            dividend_yield: get_f64(result, &["summaryDetail", "dividendYield", "raw"]),
            beta: get_f64(result, &["defaultKeyStatistics", "beta", "raw"]),
            shares_outstanding: get_f64(result, &["price", "sharesOutstanding"]),
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
        FastInfo {
            currency: get_str(result, &["price", "currency"]),
            quote_type: get_str(result, &["quoteType", "quoteType"]),
            exchange: get_str(result, &["price", "exchangeName"]),
            timezone: get_str(result, &["quoteType", "exchangeTimezoneName"]),
            shares: get_f64(result, &["price", "sharesOutstanding"]),
            market_cap: get_f64(result, &["price", "marketCap"]),
            last_price: get_f64(result, &["price", "regularMarketPrice", "raw"]),
            previous_close: get_f64(result, &["price", "regularMarketPreviousClose", "raw"]),
            open: get_f64(result, &["price", "regularMarketOpen", "raw"]),
            day_high: get_f64(result, &["price", "regularMarketDayHigh", "raw"]),
            day_low: get_f64(result, &["price", "regularMarketDayLow", "raw"]),
            last_volume: get_f64(result, &["price", "regularMarketVolume", "raw"]),
            fifty_day_average: get_f64(result, &["price", "fiftyDayAverage", "raw"]),
            two_hundred_day_average: get_f64(result, &["price", "twoHundredDayAverage", "raw"]),
            year_high: get_f64(result, &["price", "fiftyTwoWeekHigh", "raw"]),
            year_low: get_f64(result, &["price", "fiftyTwoWeekLow", "raw"]),
            year_change: get_f64(result, &["price", "fiftyTwoWeekChange", "raw"]),
        }
    }
}

impl YfSession {
    /// `info` blob for a ticker.
    pub async fn info(&self, ticker: &str) -> crate::error::Result<Info> {
        let result = crate::quote::quote_summary(self, ticker, MODULES_INFO).await?;
        Ok(Info::from_result(ticker, &result))
    }

    /// `fast_info` subset for a ticker.
    pub async fn fast_info(&self, ticker: &str) -> crate::error::Result<FastInfo> {
        let modules = [
            "price",
            "quoteType",
            "summaryDetail",
            "defaultKeyStatistics",
        ];
        let result = crate::quote::quote_summary(self, ticker, &modules).await?;
        Ok(FastInfo::from_result(&result))
    }

    /// Current shares outstanding (mirrors `get_shares`).
    pub async fn shares(&self, ticker: &str) -> crate::error::Result<Option<f64>> {
        let info = self.info(ticker).await?;
        Ok(info.shares_outstanding)
    }

    /// Full share-count time series (mirrors `get_shares_full`).
    pub async fn shares_full(
        &self,
        ticker: &str,
        start: Option<DateTime<Utc>>,
        end: Option<DateTime<Utc>>,
    ) -> crate::error::Result<Vec<(DateTime<Utc>, f64)>> {
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
        let series = crate::json::yf_result_first(&v, "timeseries")
            .ok()
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn info_symbol_fallback() {
        // symbol may be absent upstream; with_symbol_fallback backfills it.
        let v = serde_json::json!({"price": {"shortName": "Apple"}});
        let info = Info::from_result("AAPL", &v);
        assert_eq!(info.symbol.as_deref(), Some("AAPL"));
        assert_eq!(info.short_name.as_deref(), Some("Apple"));
    }

    #[test]
    fn fast_info_unwraps_raw() {
        let v = serde_json::json!({"price": {
            "currency": "USD",
            "regularMarketPrice": {"raw": 190.5, "fmt": "190.50"},
            "sharesOutstanding": 15500000000.0
        }});
        let f = FastInfo::from_result(&v);
        assert_eq!(f.currency.as_deref(), Some("USD"));
        assert_eq!(f.last_price, Some(190.5));
        assert_eq!(f.shares, Some(15_500_000_000.0));
    }
}
