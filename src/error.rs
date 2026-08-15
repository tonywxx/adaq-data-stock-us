//! Error types for the yfinance Rust port.

use thiserror::Error;

/// Result alias used throughout the crate.
pub type Result<T> = std::result::Result<T, YfError>;

/// Error taxonomy for this crate. Mirrors the spirit of yfinance's exception
/// hierarchy (`YFException` → subclasses) while staying idiomatic Rust. The
/// mapping back to yfinance categories is recorded in `docs/PARITY.md`.
#[derive(Debug, Error)]
pub enum YfError {
    /// Network/transport failure from the HTTP client.
    #[error("HTTP request failed: {0}")]
    Http(#[from] primp::Error),

    /// Yahoo returned a non-success status code.
    #[error("Yahoo returned HTTP {status}: {body}")]
    Status { status: u16, body: String },

    /// Yahoo rate-limited the request (HTTP 429).
    #[error("rate limited by Yahoo (HTTP 429)")]
    RateLimited,

    /// JSON (de)serialization failure.
    #[error("failed to parse response: {0}")]
    Parse(#[from] serde_json::Error),

    /// Response body was not valid UTF-8.
    #[error("invalid UTF-8 in response: {0}")]
    Utf8(#[from] std::string::FromUtf8Error),

    /// The requested ticker could not be found / resolved.
    #[error("ticker not found: {0}")]
    TickerMissing(String),

    /// Invalid period / interval / range combination.
    #[error("invalid period or interval: {0}")]
    InvalidPeriod(String),

    /// Expected data was missing from the response.
    #[error("missing data: {0}")]
    DataMissing(String),

    /// The requested feature is not yet implemented / not supported.
    #[error("not supported: {0}")]
    NotSupported(String),

    /// Local cache (sqlite) failure.
    #[error("cache error: {0}")]
    Cache(String),

    /// Filesystem / IO failure.
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    /// Generic message error (e.g. consent flow scrape issues).
    #[error("{0}")]
    Msg(String),
}

impl YfError {
    /// Convenience constructor for a plain message error.
    pub fn msg<S: Into<String>>(s: S) -> Self {
        YfError::Msg(s.into())
    }
}
