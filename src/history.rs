//! Price history: typed [`History`]/[`Bar`] parsed from the `v8/finance/chart`
//! endpoint, with dividend/split adjustment and corporate actions.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::error::{Result, YfError};
use crate::http::YfSession;

/// Bar interval. Mirrors yfinance's `interval` argument.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Interval {
    Min1,
    Min2,
    Min5,
    Min15,
    Min30,
    Min60,
    Min90,
    Hour1,
    Day1,
    Day5,
    Week1,
    Month1,
    Month3,
}

impl Interval {
    /// yfinance wire string for this interval.
    pub fn as_str(&self) -> &'static str {
        match self {
            Interval::Min1 => "1m",
            Interval::Min2 => "2m",
            Interval::Min5 => "5m",
            Interval::Min15 => "15m",
            Interval::Min30 => "30m",
            Interval::Min60 => "60m",
            Interval::Min90 => "90m",
            Interval::Hour1 => "1h",
            Interval::Day1 => "1d",
            Interval::Day5 => "5d",
            Interval::Week1 => "1wk",
            Interval::Month1 => "1mo",
            Interval::Month3 => "3mo",
        }
    }
}

/// Options for a [`YfSession::history`] call.
#[derive(Debug, Clone)]
pub struct HistoryOptions {
    /// Lookback period string (e.g. `"1mo"`, `"max"`). Ignored if start/end set.
    pub period: String,
    /// Bar interval.
    pub interval: Interval,
    /// Inclusive start (epoch seconds or datetime).
    pub start: Option<DateTime<Utc>>,
    /// Exclusive end.
    pub end: Option<DateTime<Utc>>,
    /// Include pre/post-market bars.
    pub prepost: bool,
    /// Include corporate actions (dividends/splits/capital gains).
    pub actions: bool,
    /// Auto-adjust OHLC by dividends/splits (yfinance default: true).
    pub auto_adjust: bool,
    /// Back-adjust instead of auto-adjust.
    pub back_adjust: bool,
    /// Keep rows with NaN (otherwise drop).
    pub keepna: bool,
    /// Repair bad Yahoo data: drop non-positive OHLC bars and make the price
    /// series split-continuous using declared split events (mirrors yfinance's
    /// `repair=True`). Off by default to preserve raw behaviour.
    pub repair: bool,
}

impl Default for HistoryOptions {
    fn default() -> Self {
        Self {
            period: "1mo".into(),
            interval: Interval::Day1,
            start: None,
            end: None,
            prepost: false,
            actions: false,
            auto_adjust: true,
            back_adjust: false,
            keepna: false,
            repair: false,
        }
    }
}

/// A single OHLCV bar.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bar {
    pub datetime: DateTime<Utc>,
    pub open: Option<f64>,
    pub high: Option<f64>,
    pub low: Option<f64>,
    pub close: Option<f64>,
    pub adj_close: Option<f64>,
    pub volume: Option<f64>,
}

/// Metadata returned alongside a chart.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct HistoryMeta {
    pub currency: Option<String>,
    pub exchange: Option<String>,
    pub instrument_type: Option<String>,
    pub timezone: Option<String>,
    pub first_trade_date: Option<i64>,
    pub regular_market_price: Option<f64>,
    pub fifty_two_week_high: Option<f64>,
    pub fifty_two_week_low: Option<f64>,
}

/// A dividend event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Dividend {
    pub date: DateTime<Utc>,
    pub amount: f64,
}

/// A stock split event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Split {
    pub date: DateTime<Utc>,
    pub numerator: f64,
    pub denominator: f64,
}

/// A capital-gains distribution (mutual funds).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapitalGain {
    pub date: DateTime<Utc>,
    pub amount: f64,
}

/// Corporate actions attached to a [`History`].
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Actions {
    pub dividends: Vec<Dividend>,
    pub splits: Vec<Split>,
    pub capital_gains: Vec<CapitalGain>,
}

/// A time series of price bars for a ticker.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct History {
    pub ticker: String,
    pub bars: Vec<Bar>,
    pub meta: HistoryMeta,
    pub actions: Option<Actions>,
}

impl History {
    /// Build a `History` from a raw `v8/finance/chart` JSON response.
    pub(crate) fn from_chart(
        ticker: &str,
        value: &serde_json::Value,
        opts: &HistoryOptions,
    ) -> Result<History> {
        let result = value
            .get("chart")
            .and_then(|c| c.get("result"))
            .and_then(|r| r.as_array())
            .and_then(|a| a.first())
            .ok_or_else(|| YfError::DataMissing("chart.result missing".into()))?;

        let meta = result
            .get("meta")
            .cloned()
            .unwrap_or(serde_json::Value::Null);
        let meta = parse_meta(&meta);

        let timestamps = result
            .get("timestamp")
            .and_then(|t| t.as_array())
            .map(|a| a.iter().filter_map(|v| v.as_i64()).collect::<Vec<_>>())
            .unwrap_or_default();

        let quote = result
            .get("indicators")
            .and_then(|i| i.get("quote"))
            .and_then(|q| q.as_array())
            .and_then(|a| a.first())
            .cloned()
            .unwrap_or(serde_json::Value::Null);

        let open = arr_f64(&quote, "open");
        let high = arr_f64(&quote, "high");
        let low = arr_f64(&quote, "low");
        let close = arr_f64(&quote, "close");
        let volume = arr_f64(&quote, "volume");

        let adjclose = result
            .get("indicators")
            .and_then(|i| i.get("adjclose"))
            .and_then(|a| a.as_array())
            .and_then(|a| a.first())
            .and_then(|ac| ac.get("adjclose"))
            .and_then(|v| v.as_array())
            .map(|a| a.iter().map(|v| v.as_f64()).collect::<Vec<_>>())
            .unwrap_or_default();

        let n = timestamps.len();
        let mut bars: Vec<Bar> = Vec::with_capacity(n);
        for i in 0..n {
            let dt = match DateTime::from_timestamp(ts_i64(&timestamps, i), 0) {
                Some(d) => d,
                None => continue,
            };
            let c = idx(&close, i);
            let adj = idx(&adjclose, i);
            let factor = if opts.auto_adjust {
                ratio(adj, c)
            } else if opts.back_adjust {
                ratio(c, adj)
            } else {
                None
            };
            let scale = |v: Option<f64>| match (v, factor) {
                (Some(x), Some(f)) => Some(x * f),
                (Some(x), None) => Some(x),
                (None, _) => None,
            };
            bars.push(Bar {
                datetime: dt,
                open: scale(idx(&open, i)),
                high: scale(idx(&high, i)),
                low: scale(idx(&low, i)),
                close: scale(c),
                adj_close: if opts.auto_adjust { c } else { adj },
                volume: idx(&volume, i),
            });
        }

        // Price-repair: drop non-positive OHLC and make the series split-continuous
        // using declared split events (mirrors yfinance `repair=True`).
        if opts.repair {
            let splits = parse_splits(result);
            repair_bars(&mut bars, &splits);
        }

        let actions = if opts.actions {
            Some(parse_actions(result))
        } else {
            None
        };

        if !opts.keepna {
            bars.retain(|b| {
                b.open.is_some() && b.high.is_some() && b.low.is_some() && b.close.is_some()
            });
        }

        Ok(History {
            ticker: ticker.to_string(),
            bars,
            meta,
            actions,
        })
    }

    /// Convert to a `polars` DataFrame (requires the `polars` feature).
    #[cfg(feature = "polars")]
    pub fn to_polars(&self) -> Result<polars::prelude::DataFrame> {
        use polars::prelude::*;
        let n = self.bars.len();
        let datetime: Vec<i64> = self
            .bars
            .iter()
            .map(|b| b.datetime.timestamp_millis())
            .collect();
        let col = |f: fn(&Bar) -> Option<f64>| {
            Series::new(
                "".into(),
                (0..n).map(|i| f(&self.bars[i])).collect::<Vec<_>>(),
            )
        };
        let df = df!(
            "datetime" => datetime,
            "open" => col(|b| b.open),
            "high" => col(|b| b.high),
            "low" => col(|b| b.low),
            "close" => col(|b| b.close),
            "adj_close" => col(|b| b.adj_close),
            "volume" => col(|b| b.volume),
        )
        .map_err(|e| YfError::msg(e.to_string()))?;
        Ok(df)
    }
}

impl YfSession {
    /// Fetch price history for a ticker.
    pub async fn history(&self, ticker: &str, opts: &HistoryOptions) -> Result<History> {
        let urls = Self::urls();
        let url = format!("{}/v8/finance/chart/{}", urls.query2, ticker);

        let mut params: Vec<(&str, String)> = Vec::new();
        params.push(("interval", opts.interval.as_str().to_string()));
        params.push(("includePrePost", opts.prepost.to_string()));
        params.push(("events", "div,split,capitalGains".to_string()));
        match (opts.start, opts.end) {
            (Some(s), Some(e)) => {
                params.push(("period1", s.timestamp().to_string()));
                params.push(("period2", e.timestamp().to_string()));
            }
            _ => {
                params.push(("range", opts.period.clone()));
            }
        }

        let value = self.get_json(&url, &params).await?;
        let err = value
            .get("chart")
            .and_then(|c| c.get("error"))
            .and_then(|e| e.get("description"))
            .and_then(|d| d.as_str());
        if let Some(desc) = err
            && !desc.is_empty()
        {
            return Err(YfError::TickerMissing(format!("{ticker}: {desc}")));
        }
        History::from_chart(ticker, &value, opts)
    }

    /// Fetch just the chart metadata for a ticker (mirrors `get_history_metadata`).
    pub async fn history_metadata(&self, ticker: &str) -> Result<HistoryMeta> {
        let urls = Self::urls();
        let url = format!("{}/v8/finance/chart/{}", urls.query2, ticker);
        let params = vec![
            ("interval", "1d".to_string()),
            ("range", "1d".to_string()),
            ("events", "div,split,capitalGains".to_string()),
        ];
        let value = self.get_json(&url, &params).await?;
        let err = value
            .get("chart")
            .and_then(|c| c.get("error"))
            .and_then(|e| e.get("description"))
            .and_then(|d| d.as_str());
        if let Some(desc) = err
            && !desc.is_empty()
        {
            return Err(YfError::TickerMissing(format!("{ticker}: {desc}")));
        }
        let result = value
            .get("chart")
            .and_then(|c| c.get("result"))
            .and_then(|r| r.as_array())
            .and_then(|a| a.first())
            .ok_or_else(|| YfError::DataMissing("chart.result missing".into()))?;
        let meta = result
            .get("meta")
            .cloned()
            .unwrap_or(serde_json::Value::Null);
        Ok(parse_meta(&meta))
    }
}

// ---- helpers ----

fn parse_meta(v: &serde_json::Value) -> HistoryMeta {
    let get = |k: &str| v.get(k).and_then(|x| x.as_str()).map(|s| s.to_string());
    let getf = |k: &str| v.get(k).and_then(|x| x.as_f64());
    let geti = |k: &str| v.get(k).and_then(|x| x.as_i64());
    HistoryMeta {
        currency: get("currency"),
        exchange: get("exchangeName"),
        instrument_type: get("instrumentType"),
        timezone: get("timezone"),
        first_trade_date: geti("firstTradeDate"),
        regular_market_price: getf("regularMarketPrice"),
        fifty_two_week_high: getf("fiftyTwoWeekHigh"),
        fifty_two_week_low: getf("fiftyTwoWeekLow"),
    }
}

fn parse_splits(result: &serde_json::Value) -> Vec<Split> {
    let mut splits = Vec::new();
    if let Some(split_evts) = result
        .get("events")
        .and_then(|e| e.get("split"))
        .and_then(|d| d.as_object())
    {
        for (_k, v) in split_evts {
            if let (Some(date), Some(num), Some(den)) = (
                v.get("date").and_then(|x| x.as_i64()),
                v.get("numerator").and_then(|x| x.as_f64()),
                v.get("denominator").and_then(|x| x.as_f64()),
            ) && let Some(dt) = DateTime::from_timestamp(date, 0)
            {
                splits.push(Split {
                    date: dt,
                    numerator: num,
                    denominator: den,
                });
            }
        }
    }
    splits
}

/// Drop non-positive OHLC bars and scale pre-split bars by each split factor so
/// the price series is continuous across splits (mirrors yfinance's
/// `repair=True` split handling). Scaling pre-split bars by `num/den` and then
/// re-applying the auto/back-adjust factor yields the same adjusted price, so
/// this is safe in all adjustment modes.
fn repair_bars(bars: &mut Vec<Bar>, splits: &[Split]) {
    bars.retain(|b| {
        [b.open, b.high, b.low, b.close]
            .iter()
            .all(|x| x.map(|v| v > 0.0).unwrap_or(true))
    });
    for s in splits {
        let f = s.numerator / s.denominator;
        if !f.is_finite() || f <= 0.0 {
            continue;
        }
        for b in bars.iter_mut() {
            if b.datetime < s.date {
                if let Some(o) = &mut b.open {
                    *o *= f;
                }
                if let Some(h) = &mut b.high {
                    *h *= f;
                }
                if let Some(l) = &mut b.low {
                    *l *= f;
                }
                if let Some(c) = &mut b.close {
                    *c *= f;
                }
                if let Some(v) = &mut b.volume {
                    *v /= f;
                }
            }
        }
    }
}

fn parse_actions(result: &serde_json::Value) -> Actions {
    let mut actions = Actions::default();
    if let Some(events) = result.get("events") {
        if let Some(divs) = events.get("div").and_then(|d| d.as_object()) {
            for (_k, v) in divs {
                if let (Some(date), Some(amount)) = (
                    v.get("date").and_then(|x| x.as_i64()),
                    v.get("amount").and_then(|x| x.as_f64()),
                ) && let Some(dt) = DateTime::from_timestamp(date, 0)
                {
                    actions.dividends.push(Dividend { date: dt, amount });
                }
            }
        }
        actions.splits = parse_splits(result);
        if let Some(cg) = events.get("capitalGains").and_then(|d| d.as_object()) {
            for (_k, v) in cg {
                if let (Some(date), Some(amount)) = (
                    v.get("date").and_then(|x| x.as_i64()),
                    v.get("amount").and_then(|x| x.as_f64()),
                ) && let Some(dt) = DateTime::from_timestamp(date, 0)
                {
                    actions.capital_gains.push(CapitalGain { date: dt, amount });
                }
            }
        }
    }
    actions
}

fn arr_f64(v: &serde_json::Value, key: &str) -> Vec<Option<f64>> {
    v.get(key)
        .and_then(|a| a.as_array())
        .map(|a| a.iter().map(|x| x.as_f64()).collect())
        .unwrap_or_default()
}

fn idx(a: &[Option<f64>], i: usize) -> Option<f64> {
    a.get(i).copied().flatten()
}

fn ts_i64(a: &[i64], i: usize) -> i64 {
    a.get(i).copied().unwrap_or(0)
}

fn ratio(num: Option<f64>, den: Option<f64>) -> Option<f64> {
    match (num, den) {
        (Some(n), Some(d)) if d != 0.0 && n.is_finite() && d.is_finite() => Some(n / d),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bar(dt: DateTime<Utc>, close: f64) -> Bar {
        Bar {
            datetime: dt,
            open: Some(close),
            high: Some(close),
            low: Some(close),
            close: Some(close),
            adj_close: Some(close),
            volume: Some(1000.0),
        }
    }

    #[test]
    fn repair_scales_pre_split_bars() {
        let d0 = DateTime::from_timestamp(1_600_000_000, 0).unwrap();
        let d1 = DateTime::from_timestamp(1_600_086_400, 0).unwrap(); // +1 day
        let split = Split {
            date: d1,
            numerator: 2.0,
            denominator: 1.0,
        };
        let mut bars = vec![bar(d0, 100.0), bar(d1, 50.0)];
        repair_bars(&mut bars, &[split]);
        // Pre-split bar scaled up by 2 (price drops 2x after split).
        assert_eq!(bars[0].close, Some(200.0));
        assert_eq!(bars[1].close, Some(50.0));
    }

    #[test]
    fn repair_drops_nonpositive_bars() {
        let d0 = DateTime::from_timestamp(1_600_000_000, 0).unwrap();
        let d1 = DateTime::from_timestamp(1_600_086_400, 0).unwrap();
        let mut bars = vec![
            bar(d0, 100.0),
            Bar {
                close: Some(0.0),
                ..bar(d1, 0.0)
            },
        ];
        repair_bars(&mut bars, &[]);
        assert_eq!(bars.len(), 1);
        assert_eq!(bars[0].close, Some(100.0));
    }
}
