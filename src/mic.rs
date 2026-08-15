//! Market Identifier Code (MIC) → Yahoo Finance suffix mapping and ISIN detection.
//!
//! Mirrors `yfinance.const._MIC_TO_YAHOO_SUFFIX` and `yfinance.utils.is_isin`.
//! A `(symbol, MIC)` pair (or a bare ISIN) is one of the ways yfinance's
//! `Ticker` accepts a security identifier; see `docs/PARITY.md`.

use crate::error::{Result, YfError};

/// Market Identifier Code → Yahoo Finance exchange suffix.
///
/// Mirrors `const._MIC_TO_YAHOO_SUFFIX`. An empty suffix means the symbol
/// carries no suffix on Yahoo (US primary exchanges `XNYS`/`XNAS`).
pub const MIC_TO_YAHOO_SUFFIX: &[(&str, &str)] = &[
    ("XCBT", "CBT"),
    ("XCME", "CME"),
    ("IFUS", "NYB"),
    ("CECS", "CMX"),
    ("XNYM", "NYM"),
    ("XNYS", ""),
    ("XNAS", ""),
    ("XBUE", "BA"),
    ("XVIE", "VI"),
    ("XASX", "AX"),
    ("XAUS", "XA"),
    ("XBRU", "BR"),
    ("BVMF", "SA"),
    ("CNSX", "CN"),
    ("NEOE", "NE"),
    ("XTSE", "TO"),
    ("XTSX", "V"),
    ("XSGO", "SN"),
    ("XSHG", "SS"),
    ("XSHE", "SZ"),
    ("XBOG", "CL"),
    ("XPRA", "PR"),
    ("XCSE", "CO"),
    ("XCAI", "CA"),
    ("XTAL", "TL"),
    ("CEUX", "XD"),
    ("XEUR", "NX"),
    ("XHEL", "HE"),
    ("XPAR", "PA"),
    ("XBER", "BE"),
    ("XBMS", "BM"),
    ("XDUS", "DU"),
    ("XFRA", "F"),
    ("XHAM", "HM"),
    ("XHAN", "HA"),
    ("XMUN", "MU"),
    ("XSTU", "SG"),
    ("XETR", "DE"),
    ("XATH", "AT"),
    ("XHKG", "HK"),
    ("XBUD", "BD"),
    ("XICE", "IC"),
    ("XBOM", "BO"),
    ("XNSE", "NS"),
    ("XIDX", "JK"),
    ("XDUB", "IR"),
    ("XTAE", "TA"),
    ("MTAA", "MI"),
    ("EUTL", "TI"),
    ("XTKS", "T"),
    ("XKFE", "KW"),
    ("XRIS", "RG"),
    ("XVIL", "VS"),
    ("XKLS", "KL"),
    ("XMEX", "MX"),
    ("XAMS", "AS"),
    ("XNZE", "NZ"),
    ("XOSL", "OL"),
    ("XPHS", "PS"),
    ("XWAR", "WA"),
    ("XLIS", "LS"),
    ("XQAT", "QA"),
    ("XBSE", "RO"),
    ("XSES", "SI"),
    ("XJSE", "JO"),
    ("XKRX", "KS"),
    ("KQKS", "KQ"),
    ("BMEX", "MC"),
    ("XSAU", "SR"),
    ("XSTO", "ST"),
    ("XSWX", "SW"),
    ("ROCO", "TWO"),
    ("XTAI", "TW"),
    ("XBKK", "BK"),
    ("XIST", "IS"),
    ("XDFM", "AE"),
    ("AQXE", "AQ"),
    ("XCHI", "XC"),
    ("XLON", "L"),
    ("ILSE", "IL"),
    ("XCAR", "CR"),
    ("XSTC", "VN"),
];

/// Return the Yahoo Finance suffix for a MIC, or `None` if unknown.
///
/// A leading dot on the MIC (e.g. `.PA`) is stripped, mirroring yfinance.
pub fn mic_to_suffix(mic: &str) -> Option<&'static str> {
    let mic = mic.strip_prefix('.').unwrap_or(mic);
    MIC_TO_YAHOO_SUFFIX
        .iter()
        .find(|(m, _)| m.eq_ignore_ascii_case(mic))
        .map(|(_, s)| *s)
}

/// True if `s` is a well-formed ISIN: 2 letters + 9 alphanumerics + 1 digit
/// (mirrors `utils.is_isin`'s `^([A-Z]{2})([A-Z0-9]{9})([0-9])$`).
pub fn is_isin(s: &str) -> bool {
    let b = s.as_bytes();
    if b.len() != 12 {
        return false;
    }
    b[0].is_ascii_alphabetic()
        && b[1].is_ascii_alphabetic()
        && (2..11).all(|i| b[i].is_ascii_alphanumeric())
        && b[11].is_ascii_digit()
}

/// Resolve a `(symbol, MIC)` pair into a Yahoo ticker symbol.
///
/// Mirrors `TickerBase.__init__`'s tuple handling: an unknown MIC is an error,
/// an empty suffix yields the bare symbol, otherwise `SYMBOL.SUFFIX`.
pub fn resolve_symbol(symbol: &str, mic: &str) -> Result<String> {
    let mic = mic.strip_prefix('.').unwrap_or(mic);
    let sfx =
        mic_to_suffix(mic).ok_or_else(|| YfError::msg(format!("Unknown MIC code: '{mic}'")))?;
    let symbol = symbol.to_ascii_uppercase();
    Ok(if sfx.is_empty() {
        symbol
    } else {
        format!("{symbol}.{sfx}")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn isin_detection() {
        assert!(is_isin("US0378331005")); // Apple
        assert!(is_isin("GB0002634946")); // BP
        assert!(!is_isin("AAPL"));
        assert!(!is_isin("US037833100")); // too short
        assert!(!is_isin("US037833100X")); // last char not a digit
        assert!(!is_isin("12378331005")); // first chars not letters
    }

    #[test]
    fn mic_suffix_lookup() {
        assert_eq!(mic_to_suffix("XPAR"), Some("PA"));
        assert_eq!(mic_to_suffix("xpar"), Some("PA")); // case-insensitive
        assert_eq!(mic_to_suffix("XNYS"), Some("")); // US, no suffix
        assert_eq!(mic_to_suffix(".PA"), None); // ".PA" is a suffix, not a MIC
        assert_eq!(mic_to_suffix("ZZZZ"), None);
    }

    #[test]
    fn resolve_symbol_pairs() {
        assert_eq!(resolve_symbol("OR", "XPAR").unwrap(), "OR.PA");
        assert_eq!(resolve_symbol("AAPL", "XNAS").unwrap(), "AAPL");
        assert!(resolve_symbol("AAPL", "ZZZZ").is_err());
        assert_eq!(resolve_symbol("mc", "XLON").unwrap(), "MC.L");
    }
}
