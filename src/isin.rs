//! ISIN ↔ ticker resolution.
//!
//! - [`resolve_isin`] maps an ISIN to a Yahoo ticker (mirrors
//!   `utils.get_ticker_by_isin`).
//! - [`isin_for_ticker`] is the reverse lookup (mirrors `Ticker.get_isin`,
//!   marked experimental upstream).

use crate::error::{Result, YfError};
use crate::http::YfSession;
use crate::mic::is_isin;

/// Resolve an ISIN to a Yahoo ticker symbol.
///
/// Mirrors `utils.get_ticker_by_isin`: validate the ISIN, check the cached
/// mapping, otherwise run a Yahoo `Search` for the ISIN and take the first
/// matching quote's symbol. The result is cached for future calls.
pub async fn resolve_isin(session: &YfSession, isin: &str) -> Result<String> {
    if !is_isin(isin) {
        return Err(YfError::msg(format!("Invalid ISIN number: {isin}")));
    }
    if let Some(t) = session.cache().get_isin(isin) {
        return Ok(t);
    }
    let res = session.search(isin, 1, 0).await?;
    let ticker = res
        .quotes
        .first()
        .and_then(|q| q.symbol.clone())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| YfError::msg(format!("Invalid ISIN number: {isin}")))?;
    session.cache().set_isin(isin, &ticker);
    Ok(ticker)
}

/// Reverse lookup: resolve a Yahoo ticker to its ISIN.
///
/// Mirrors `Ticker.get_isin` (experimental). Indicators and indices (`-`/`^`
/// symbols) have no ISIN and yield `"-"`. Otherwise the ticker's short name is
/// searched via Business Insider's suggest endpoint and the embedded ISIN is
/// extracted.
pub async fn isin_for_ticker(session: &YfSession, ticker: &str) -> Result<String> {
    if ticker.contains('-') || ticker.contains('^') {
        return Ok("-".to_string());
    }

    // Prefer the human-readable name; fall back to the raw symbol.
    let q = match session.info(ticker).await.ok().and_then(|i| i.short_name) {
        Some(name) if !name.is_empty() => name,
        _ => ticker.to_string(),
    };

    let url = "https://markets.businessinsider.com/ajax/SearchController_Suggest";
    let params = vec![
        ("max_results", "25".to_string()),
        ("query", encode_query(&q)),
    ];
    let data = session.get_text(url, &params).await?;

    // Try the most specific anchor first ("<TICKER>|"), then a bare quote.
    let mut anchor = format!("\"{ticker}|");
    if !data.contains(&anchor) {
        if data.to_lowercase().contains(&q.to_lowercase()) {
            anchor = "\"|".to_string();
            if !data.contains(&anchor) {
                return Ok("-".to_string());
            }
        } else {
            return Ok("-".to_string());
        }
    }

    let after = data.split_once(&anchor).map(|(_, rest)| rest).unwrap_or("");
    let isin = after
        .split('"')
        .next()
        .unwrap_or("")
        .split('|')
        .next()
        .unwrap_or("")
        .to_string();
    if isin.is_empty() {
        Ok("-".to_string())
    } else {
        Ok(isin)
    }
}

/// Minimal percent-encoding for query parameters (spaces and a few reserved
/// characters). Sufficient for ticker symbols and short names.
fn encode_query(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' | '.' | '~' => out.push(c),
            ' ' => out.push_str("%20"),
            _ => {
                for b in c.to_string().bytes() {
                    out.push_str(&format!("%{b:02X}"));
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_spaces_and_symbols() {
        assert_eq!(encode_query("Apple Inc."), "Apple%20Inc.");
        assert_eq!(encode_query("AAPL"), "AAPL");
        assert_eq!(encode_query("BRK.B"), "BRK.B");
    }

    #[test]
    fn reverse_isin_parsing() {
        // "AAPL|US0378331005|..." -> US0378331005
        let html = r#"foo "AAPL|US0378331005|Apple Inc." bar"#;
        let out = html.split_once("\"AAPL|").map(|(_, r)| r).unwrap_or("");
        let isin = out
            .split('"')
            .next()
            .unwrap_or("")
            .split('|')
            .next()
            .unwrap_or("");
        assert_eq!(isin, "US0378331005");
    }
}
