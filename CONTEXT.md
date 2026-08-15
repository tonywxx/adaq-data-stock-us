# adaq-data-stock-us

A Rust reimplementation of the Python `yfinance` library for fetching US equity market data from Yahoo Finance. Ships as a library crate (`adaq-data-stock-us`) published to crates.io, with a tracked parity relationship back to `yfinance`.

## Language

**Ticker**:
A security identifier (Yahoo symbol, optionally an ISIN or a `(symbol, MIC)` pair). The unit a caller queries; wraps all per-security data fetches.
_Avoid_: symbol (a ticker may be more than a bare symbol), instrument, asset

**History**:
The time series of OHLCV price bars for a ticker, optionally including corporate actions.
_Avoid_: prices, chart

**Interval**:
The spacing between price bars — e.g. `1d`, `1m`, `1wk`. Drives how many bars span a given period.
_Avoid_: granularity, frequency, resolution

**Period**:
A lookback window for history — e.g. `1mo`, `max`. Mutually exclusive with explicit start/end.
_Avoid_: range, window

**Actions**:
Corporate actions attached to history: dividends, stock splits, and capital gains. Returned as extra columns/series.
_Avoid_: events (overloaded), corporate actions

**Auto-adjust / Back-adjust**:
Dividend/split price adjustment. *Auto-adjust* rescales the OHLC bars and keeps raw close; *back-adjust* keeps `Adj Close` and rescales the rest.
_Avoid_: adjust (ambiguous)

**QuoteSummary**:
The Yahoo `quoteSummary` endpoint family returning a security's metadata (info, holders, analysis, sustainability, …) via named modules.
_Avoid_: quote, summary

**FastInfo**:
A lightweight subset of quote fields (last price, market cap, day range, averages…) derived from price history instead of a full `quoteSummary` call.
_Avoid_: quick info, mini quote

**Crumb**:
A Yahoo auth token appended to request params; fetched from a `getcrumb` endpoint once a consent cookie exists.
_Avoid_: token, auth

**Consent flow**:
The handling of Yahoo's `consent.yahoo.com` GDPR redirect — re-submitting a consent form to obtain cookies that unblock data endpoints.
_Avoid_: auth flow, login

**Lenient mode**:
Bulk-download resilience: when a single ticker fails, the batch continues and errors are collected rather than aborting the whole download. Mirrors yfinance's `hide_exceptions`.
_Avoid_: resilient mode, soft errors

**Parity**:
The alignment status of a Rust module against its corresponding `yfinance` source — recorded in `docs/PARITY.md`.
_Avoid_: sync, equivalence

**YfSession**:
The shared HTTP session: persistent cookie jar, crumb, consent handling, and retry/backoff. One per process, thread-safe.
_Avoid_: client (too generic), session

**MIC**:
Market Identification Code. A `(symbol, MIC)` pair resolves to a Yahoo exchange suffix via a fixed map.
_Avoid_: exchange (MIC is the code, not the venue name)

**ISIN**:
International Securities Identification Number. Tickers may be given as ISINs and are resolved to a Yahoo symbol (cached).
_Avoid_: id, code
