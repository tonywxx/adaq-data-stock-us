//! Options chain (mirrors yfinance's `Ticker.option_chain`).

use serde::{Deserialize, Serialize};

use crate::error::{Result, YfError};
use crate::http::YfSession;

/// A single option contract (call or put).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct OptionContract {
    pub contract_symbol: Option<String>,
    pub strike: Option<f64>,
    pub currency: Option<String>,
    pub last_price: Option<f64>,
    pub bid: Option<f64>,
    pub ask: Option<f64>,
    pub change: Option<f64>,
    pub percent_change: Option<f64>,
    pub volume: Option<f64>,
    pub open_interest: Option<f64>,
    pub implied_volatility: Option<f64>,
    pub in_the_money: Option<bool>,
    pub last_trade_date: Option<String>,
}

/// Options for one expiration, split into calls and puts.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExpirationOptions {
    pub expiration: String,
    pub calls: Vec<OptionContract>,
    pub puts: Vec<OptionContract>,
}

/// A full option chain for a ticker.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptionChain {
    pub ticker: String,
    pub expirations: Vec<String>,
    pub strikes: Vec<f64>,
    pub options: Vec<ExpirationOptions>,
}

impl OptionChain {
    /// List of expiration-date strings (mirrors yfinance's `Ticker.options`).
    pub fn expiration_dates(&self) -> &[String] {
        &self.expirations
    }
}

impl YfSession {
    /// Fetch the option chain for a ticker.
    pub async fn option_chain(&self, ticker: &str) -> Result<OptionChain> {
        let urls = YfSession::urls();
        let url = format!("{}/v7/finance/options/{}", urls.query2, ticker);
        let value = self.get_json(&url, &[]).await?;
        let result = value
            .get("optionChain")
            .and_then(|c| c.get("result"))
            .and_then(|r| r.as_array())
            .and_then(|a| a.first())
            .ok_or_else(|| YfError::DataMissing(format!("optionChain.result for {ticker}")))?;

        let expirations = result
            .get("expirationDates")
            .and_then(|e| e.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_i64())
                    .filter_map(|sec| chrono::DateTime::from_timestamp(sec, 0))
                    .map(|dt| dt.naive_utc().format("%Y-%m-%d").to_string())
                    .collect()
            })
            .unwrap_or_default();

        let strikes = result
            .get("strikes")
            .and_then(|s| s.as_array())
            .map(|a| a.iter().filter_map(|v| v.as_f64()).collect())
            .unwrap_or_default();

        let options = result
            .get("options")
            .and_then(|o| o.as_array())
            .map(|a| {
                a.iter()
                    .map(|o| {
                        let exp = o
                            .get("expirationDate")
                            .and_then(|e| e.as_i64())
                            .and_then(|sec| chrono::DateTime::from_timestamp(sec, 0))
                            .map(|dt| dt.naive_utc().format("%Y-%m-%d").to_string())
                            .unwrap_or_default();
                        ExpirationOptions {
                            expiration: exp,
                            calls: o
                                .get("calls")
                                .and_then(|c| c.as_array())
                                .map(|c| c.iter().filter_map(parse_contract).collect())
                                .unwrap_or_default(),
                            puts: o
                                .get("puts")
                                .and_then(|p| p.as_array())
                                .map(|p| p.iter().filter_map(parse_contract).collect())
                                .unwrap_or_default(),
                        }
                    })
                    .collect()
            })
            .unwrap_or_default();

        Ok(OptionChain {
            ticker: ticker.to_string(),
            expirations,
            strikes,
            options,
        })
    }
}

fn parse_contract(v: &serde_json::Value) -> Option<OptionContract> {
    let f = |p: &str| v.get(p).and_then(|x| x.as_f64());
    let s = |p: &str| v.get(p).and_then(|x| x.as_str()).map(String::from);
    let b = |p: &str| v.get(p).and_then(|x| x.as_bool());
    let trade = v
        .get("lastTradeDate")
        .and_then(|x| x.as_i64())
        .and_then(|sec| chrono::DateTime::from_timestamp(sec, 0))
        .map(|dt| dt.naive_utc().format("%Y-%m-%d %H:%M:%S").to_string());
    Some(OptionContract {
        contract_symbol: s("contractSymbol"),
        strike: f("strike"),
        currency: s("currency"),
        last_price: f("lastPrice"),
        bid: f("bid"),
        ask: f("ask"),
        change: f("change"),
        percent_change: f("percentChange"),
        volume: f("volume"),
        open_interest: f("openInterest"),
        implied_volatility: f("impliedVolatility"),
        in_the_money: b("inTheMoney"),
        last_trade_date: trade,
    })
}
