//! Live streaming (mirrors yfinance's `live`): a WebSocket price stream from
//! `streamer.finance.yahoo.com`, decoding the `PricingData` protobuf frames.
//!
//! ```no_run
//! # async fn run() -> adaq_data_stock_us::Result<()> {
//! # use adaq_data_stock_us::live::LiveWebSocket;
//! let ws = LiveWebSocket::new();
//! ws.stream(&["AAPL", "MSFT"], |tick| {
//!     println!("{} price={}", tick.id, tick.price);
//! }).await?;
//! # Ok(()) }
//! ```

use std::time::{Duration, Instant};

use base64::Engine;
use futures::{SinkExt, StreamExt};
use prost::Message;
use serde::Serialize;
use tokio_tungstenite::tungstenite::Message as WsMessage;

use crate::error::{Result, YfError};

const STREAM_URL: &str = "wss://streamer.finance.yahoo.com/?version=2";
const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(15);

/// Decoded streaming price tick. Mirrors yfinance's `pricing_pb2.PricingData`.
#[derive(Clone, Message, Serialize)]
pub struct PricingData {
    #[prost(string, tag = "1")]
    pub id: String,
    #[prost(float, tag = "2")]
    pub price: f32,
    #[prost(sint64, tag = "3")]
    pub time: i64,
    #[prost(string, tag = "4")]
    pub currency: String,
    #[prost(string, tag = "5")]
    pub exchange: String,
    #[prost(int32, tag = "6")]
    pub quote_type: i32,
    #[prost(int32, tag = "7")]
    pub market_hours: i32,
    #[prost(float, tag = "8")]
    pub change_percent: f32,
    #[prost(sint64, tag = "9")]
    pub day_volume: i64,
    #[prost(float, tag = "10")]
    pub day_high: f32,
    #[prost(float, tag = "11")]
    pub day_low: f32,
    #[prost(float, tag = "12")]
    pub change: f32,
    #[prost(string, tag = "13")]
    pub short_name: String,
    #[prost(sint64, tag = "14")]
    pub expire_date: i64,
    #[prost(float, tag = "15")]
    pub open_price: f32,
    #[prost(float, tag = "16")]
    pub previous_close: f32,
    #[prost(float, tag = "17")]
    pub strike_price: f32,
    #[prost(string, tag = "18")]
    pub underlying_symbol: String,
    #[prost(sint64, tag = "19")]
    pub open_interest: i64,
    #[prost(sint64, tag = "20")]
    pub options_type: i64,
    #[prost(sint64, tag = "21")]
    pub mini_option: i64,
    #[prost(sint64, tag = "22")]
    pub last_size: i64,
    #[prost(float, tag = "23")]
    pub bid: f32,
    #[prost(sint64, tag = "24")]
    pub bid_size: i64,
    #[prost(float, tag = "25")]
    pub ask: f32,
    #[prost(sint64, tag = "26")]
    pub ask_size: i64,
    #[prost(sint64, tag = "27")]
    pub price_hint: i64,
    #[prost(sint64, tag = "28")]
    pub vol_24hr: i64,
    #[prost(sint64, tag = "29")]
    pub vol_all_currencies: i64,
    #[prost(string, tag = "30")]
    pub from_currency: String,
    #[prost(string, tag = "31")]
    pub last_market: String,
    #[prost(double, tag = "32")]
    pub circulating_supply: f64,
    #[prost(double, tag = "33")]
    pub market_cap: f64,
}

impl PricingData {
    /// Decode a base64-encoded `PricingData` protobuf frame.
    pub fn from_base64(base64_msg: &str) -> Result<PricingData> {
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(base64_msg.trim())
            .map_err(|e| YfError::msg(format!("invalid base64: {e}")))?;
        PricingData::decode(bytes.as_slice())
            .map_err(|e| YfError::msg(format!("invalid PricingData protobuf: {e}")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use prost::Message;

    #[test]
    fn pricing_data_roundtrip_matches_wire_format() {
        let tick = PricingData {
            id: "AAPL".into(),
            price: 191.5,
            time: 1_700_000_000,
            currency: "USD".into(),
            exchange: "NMS".into(),
            quote_type: 8,
            change_percent: 1.25,
            day_volume: 42_000_000,
            day_high: 192.0,
            day_low: 189.0,
            change: 2.5,
            short_name: "Apple Inc.".into(),
            bid: 191.4,
            ask: 191.6,
            market_cap: 2_900_000_000_000.0,
            ..Default::default()
        };
        let mut buf = Vec::new();
        tick.encode(&mut buf).expect("encode");
        let b64 = base64::engine::general_purpose::STANDARD.encode(&buf);
        let decoded = PricingData::from_base64(&b64).expect("decode");
        assert_eq!(decoded.id, "AAPL");
        assert_eq!(decoded.price, 191.5);
        assert_eq!(decoded.time, 1_700_000_000);
        assert_eq!(decoded.currency, "USD");
        assert_eq!(decoded.exchange, "NMS");
        assert_eq!(decoded.quote_type, 8);
        assert_eq!(decoded.change_percent, 1.25);
        assert_eq!(decoded.day_volume, 42_000_000);
        assert_eq!(decoded.day_high, 192.0);
        assert_eq!(decoded.day_low, 189.0);
        assert_eq!(decoded.change, 2.5);
        assert_eq!(decoded.short_name, "Apple Inc.");
        assert_eq!(decoded.bid, 191.4);
        assert_eq!(decoded.ask, 191.6);
        assert_eq!(decoded.market_cap, 2_900_000_000_000.0);
    }
}

/// Async Yahoo Finance live price stream. Mirrors yfinance's `AsyncWebSocket`.
#[derive(Debug, Clone)]
pub struct LiveWebSocket {
    url: String,
    verbose: bool,
}

impl Default for LiveWebSocket {
    fn default() -> Self {
        Self::new()
    }
}

impl LiveWebSocket {
    /// Create a stream client for the default Yahoo streamer URL.
    pub fn new() -> Self {
        Self {
            url: STREAM_URL.to_string(),
            verbose: false,
        }
    }

    /// Toggle verbose logging to stderr.
    pub fn verbose(mut self, verbose: bool) -> Self {
        self.verbose = verbose;
        self
    }

    /// Connect, subscribe to `symbols`, and invoke `handler` for each decoded
    /// tick until the connection closes or errors. A subscription heartbeat is
    /// sent every 15s, matching yfinance.
    pub async fn stream<F>(&self, symbols: &[&str], mut handler: F) -> Result<()>
    where
        F: FnMut(PricingData) + Send,
    {
        let (mut ws, _resp) = tokio_tungstenite::connect_async(&self.url)
            .await
            .map_err(|e| YfError::msg(format!("ws connect: {e}")))?;

        let subs: Vec<String> = symbols.iter().map(|s| s.to_string()).collect();
        if self.verbose {
            eprintln!("live: subscribing to {:?}", subs);
        }
        let subscribe_msg = serde_json::json!({ "subscribe": subs }).to_string();
        ws.send(WsMessage::Text(subscribe_msg.into()))
            .await
            .map_err(|e| YfError::msg(format!("ws send: {e}")))?;

        let mut last_sub = Instant::now();
        while let Some(item) = ws.next().await {
            let item = item.map_err(|e| YfError::msg(format!("ws recv: {e}")))?;
            match item {
                WsMessage::Text(t) => {
                    if let Ok(v) = serde_json::from_str::<serde_json::Value>(&t)
                        && let Some(b64) = v.get("message").and_then(|x| x.as_str())
                    {
                        match PricingData::from_base64(b64) {
                            Ok(p) => handler(p),
                            Err(e) => {
                                if self.verbose {
                                    eprintln!("live: decode error: {e}");
                                }
                            }
                        }
                    }
                }
                WsMessage::Ping(_) | WsMessage::Pong(_) => {}
                WsMessage::Close(_) => {
                    if self.verbose {
                        eprintln!("live: connection closed");
                    }
                    break;
                }
                _ => {}
            }

            if last_sub.elapsed() >= HEARTBEAT_INTERVAL {
                let heartbeat = serde_json::json!({ "subscribe": subs }).to_string();
                if let Err(e) = ws.send(WsMessage::Text(heartbeat.into())).await {
                    if self.verbose {
                        eprintln!("live: heartbeat error: {e}");
                    }
                    break;
                }
                last_sub = Instant::now();
            }
        }
        Ok(())
    }
}
