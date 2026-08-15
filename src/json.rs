//! Centralised JSON value accessors for Yahoo Finance responses.
//!
//! yfinance wraps many numeric/date fields in a `{"raw": x, "fmt": ".."}`
//! object. These helpers treat that wrapper as an implementation detail:
//! [`get`] and the typed accessors unwrap `raw` automatically, so a caller
//! reads a typed field without caring whether Yahoo sent a bare value or a
//! wrapped one. [`get_raw`] returns the wrapper object itself when the caller
//! wants the surrounding object or array (e.g. `sectorWeightings`, `holdings`).
//!
//! This module is the single seam for every "read a typed field out of a
//! Yahoo JSON blob" operation across the crate — replacing the previously
//! duplicated `dig` / `dig_f64` / `dig_str` helpers and the inline
//! `.get("raw")` reads scattered through `quote.rs`, `domain.rs`,
//! `screener.rs`, `fundamentals.rs`, and `options.rs`.

use chrono::{DateTime, NaiveDate, Utc};
use serde_json::Value;

use crate::error::{Result, YfError};

/// Dig a nested JSON path, tolerating missing nodes. At the leaf, unwraps
/// yfinance's `{"raw": x, "fmt": ".."}` numeric/date wrapper. An explicit
/// `raw` segment in `path` still resolves correctly (it lands on the raw
/// value, and `get("raw")` on a scalar is a no-op).
pub fn get<'a>(v: &'a Value, path: &[&str]) -> Option<&'a Value> {
    let mut cur = v;
    for p in path {
        cur = cur.get(p)?;
    }
    if let Some(raw) = cur.get("raw") {
        return Some(raw);
    }
    Some(cur)
}

/// Like [`get`] but returns the wrapper object unchanged (no `raw` unwrap).
/// Use when the caller wants the surrounding object or array rather than the
/// unwrapped scalar.
pub fn get_raw<'a>(v: &'a Value, path: &[&str]) -> Option<&'a Value> {
    let mut cur = v;
    for p in path {
        cur = cur.get(p)?;
    }
    Some(cur)
}

/// Typed accessors. All return `None` on a missing or wrong-typed node,
/// matching the prior `dig` / `dig_f64` semantics.
pub fn get_f64(v: &Value, path: &[&str]) -> Option<f64> {
    get(v, path).and_then(|x| x.as_f64())
}

/// Integer accessor with a float fallback (Yahoo often sends integers as
/// floats, e.g. `sharesOutstanding`).
pub fn get_i64(v: &Value, path: &[&str]) -> Option<i64> {
    get(v, path).and_then(|x| x.as_i64().or_else(|| x.as_f64().map(|f| f as i64)))
}

/// Unsigned integer accessor with a float fallback (e.g. screener volume).
pub fn get_u64(v: &Value, path: &[&str]) -> Option<u64> {
    get(v, path).and_then(|x| x.as_u64().or_else(|| x.as_f64().map(|f| f as u64)))
}

pub fn get_str(v: &Value, path: &[&str]) -> Option<String> {
    get(v, path).and_then(|x| x.as_str()).map(String::from)
}

/// Parse a date at `path`, auto-detecting the two Yahoo shapes: an
/// epoch-seconds number (`{"raw": 1700000000}`) or a `"YYYY-MM-DD"` string
/// (`{"raw": "2023-01-15"}`, as in SEC filings). Returns `None` if the node
/// is missing or unparseable.
pub fn get_date(v: &Value, path: &[&str]) -> Option<DateTime<Utc>> {
    match get(v, path) {
        Some(Value::Number(n)) => {
            let secs = n.as_f64()?;
            DateTime::from_timestamp(secs as i64, 0)
        }
        Some(Value::String(s)) => NaiveDate::parse_from_str(s, "%Y-%m-%d")
            .ok()
            .and_then(|d| d.and_hms_opt(0, 0, 0))
            .map(|naive| DateTime::from_naive_utc_and_offset(naive, Utc)),
        _ => None,
    }
}

/// Return the `result` array node of a Yahoo `finance` / `chart` /
/// `quoteSummary` / `optionChain` / `timeseries` / `marketSummaryResponse`
/// response. Some callers iterate the whole array (`lookup`, `market`); others
/// take the first element via [`yf_result_first`].
pub fn yf_result<'a>(v: &'a Value, key: &str) -> Result<&'a Value> {
    v.get(key)
        .and_then(|k| k.get("result"))
        .ok_or_else(|| YfError::DataMissing(format!("{key}.result missing")))
}

/// Convenience for the common case: the first element of [`yf_result`].
pub fn yf_result_first<'a>(v: &'a Value, key: &str) -> Result<&'a Value> {
    let r = yf_result(v, key)?;
    r.as_array()
        .and_then(|a| a.first())
        .ok_or_else(|| YfError::DataMissing(format!("{key}.result[0] missing")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn unwraps_raw_by_default() {
        let v = json!({"price": {"marketCap": {"raw": 3.0e12, "fmt": "3T"}}});
        assert_eq!(get_f64(&v, &["price", "marketCap"]), Some(3.0e12));
        // explicit "raw" segment still lands on the scalar
        assert_eq!(get_f64(&v, &["price", "marketCap", "raw"]), Some(3.0e12));
    }

    #[test]
    fn bare_number_no_wrapper() {
        let v = json!({"strike": 123.45});
        assert_eq!(get_f64(&v, &["strike"]), Some(123.45));
    }

    #[test]
    fn missing_node_is_none() {
        let v = json!({"a": 1});
        // path absent -> None
        assert_eq!(get_f64(&v, &["b", "c"]), None);
        // wrong type at present node -> None
        assert_eq!(get_str(&v, &["a"]), None);
        // key absent -> None (a present bare number is a valid epoch, not "missing")
        assert_eq!(get_date(&json!({}), &["a"]), None);
    }

    #[test]
    fn get_raw_returns_wrapper() {
        let v = json!({"x": {"raw": 5, "fmt": "5"}});
        let w = get_raw(&v, &["x"]).unwrap();
        assert!(w.get("raw").is_some());
        assert!(w.get("fmt").is_some());
    }

    #[test]
    fn get_str_unwraps_raw() {
        let v = json!({"name": {"raw": "Apple", "fmt": "Apple"}});
        assert_eq!(get_str(&v, &["name"]).as_deref(), Some("Apple"));
    }

    #[test]
    fn get_i64_and_u64_from_float() {
        let v = json!({"shares": {"raw": 1.5e9, "fmt": "1.5B"}});
        assert_eq!(get_i64(&v, &["shares"]), Some(1_500_000_000));
        assert_eq!(get_u64(&v, &["shares"]), Some(1_500_000_000));
    }

    #[test]
    fn get_date_epoch_and_string() {
        let epoch = json!({"d": {"raw": 1700000000, "fmt": "2023"}});
        let dt = get_date(&epoch, &["d"]).unwrap();
        assert_eq!(dt.format("%Y-%m-%d").to_string(), "2023-11-14");
        let s = json!({"d": {"raw": "2023-01-15", "fmt": "Jan 15, 2023"}});
        let dt = get_date(&s, &["d"]).unwrap();
        assert_eq!(dt.format("%Y-%m-%d").to_string(), "2023-01-15");
        assert_eq!(get_date(&json!({}), &["d"]), None);
    }

    #[test]
    fn yf_result_returns_array() {
        let v = json!({"finance": {"result": [{"a": 1}, {"b": 2}]}});
        let r = yf_result(&v, "finance").unwrap();
        assert_eq!(r.as_array().unwrap().len(), 2);
        let first = yf_result_first(&v, "finance").unwrap();
        assert_eq!(first.get("a").and_then(|x| x.as_i64()), Some(1));
    }

    #[test]
    fn yf_result_missing_errors() {
        assert!(yf_result(&json!({}), "finance").is_err());
        assert!(yf_result_first(&json!({"finance": {"result": []}}), "finance").is_err());
    }
}
