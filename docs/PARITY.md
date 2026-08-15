# Parity Tracker — adaq-data-stock-us ↔ yfinance

This file records how each Rust module maps to its `yfinance` source of truth, the
pinned upstream version, and implementation status. It is the companion to
`docs/adr/0003-parity-mechanism.md`. Keep it in sync when aligning to a new release.

- **Upstream**: `yfinance` vendored at `vendor/yfinance/` (git submodule).
- **Pinned version**: `1.6.0`
- **Pinned commit**: `93eb4c2` (see `PARITY_PIN` at repo root)
- **Last aligned**: 2026-08-15

## Legend

| Status | Meaning |
|--------|---------|
| `todo` | Not started |
| `partial` | Core path works, edge cases / sub-fields missing |
| `done` | Aligned with upstream for the pinned version |

Phases: **P1** HTTP core + history + download; **P2** quote/fundamentals/options;
**P3** domain/search/calendars/screener; **P4** live/auth.

## Module map

| Rust module | yfinance source | Phase | Status |
|-------------|-----------------|-------|--------|
| `http` (`YfSession`) | `data.py`, `_http.py`, `const.py` | P1 | done |
| `cache` (sqlite) | `cache.py` | P1 | done |
| `error` (`YfError`) | `exceptions.py` | P1 | done |
| `history` | `scrapers/history.py`, `base.py` (`history`) | P1 | done |
| `ticker` | `ticker.py`, `base.py` | P1 | done |
| `download` (bulk) | `multi.py`, `tickers.py`, `shared.py` | P1 | done |
| `quote` (info/fast_info/holders/analysis/estimates/recommendations/upgrades_downgrades/valuation/calendar/sec_filings/funds/shares) | `scrapers/quote.py`, `scrapers/holders.py`, `scrapers/analysis.py`, `scrapers/funds.py`, `base.py` (`get_shares`/`get_shares_full`/`get_funds_data`/`get_valuation_measures`/`get_recommendations`/`get_upgrades_downgrades`/`get_calendar`/`get_sec_filings`) | P2 | done |
| `fundamentals` (3 statements) | `scrapers/fundamentals.py` | P2 | done |
| `options` (option chain) | `ticker.py` (`option_chain`) | P2 | done |
| `domain` (Sector/Industry/Market) | `domain/*.py` | P3 | done |
| `search` / `lookup` | `search.py`, `lookup.py` | P3 | done |
| `calendars` | `calendars.py` | P3 | done |
| `screener` | `screener/query.py`, `screener/screener.py` | P3 | done |
| `live` (WebSocket) | `live.py`, `pricing.proto` | P4 | done |
| `auth` (login) | `data.py` (`Auth`) | P4 | done |
| `config` | `config.py` | P1 | done |
| `news` (per-ticker) | `base.py` (`get_news` → `xhr/ncp`) | P2 | done |
| `earnings_dates` (per-ticker) | `base.py` (`get_earnings_dates`) | P2 | done |
| `isin` (reverse ticker→ISIN) | `base.py` (`get_isin`) | P2 | done |
| `earnings_estimate` / `revenue_estimate` / `earnings_history` / `eps_trend` / `eps_revisions` / `growth_estimates` | `scrapers/analysis.py` (quoteSummary modules `earningsTrend`, `revenueEstimates`, `epsTrend`, `epsRevisions`, `growthEstimates`) | P2 | done |
| `history_metadata` (per-ticker) | `base.py` (`get_history_metadata`) | P2 | done |

## Shared / cross-cutting

| Concern | yfinance source | Phase | Status |
|---------|-----------------|-------|--------|
| Parsing & price-repair | `utils.py` (`price_repair`/`solve_split_mul`) | P1 | partial |
| Endpoints & URL constants | `const.py` | P1 | done |
| ISIN / MIC resolution | `base.py` (ISIN↔ticker), `const.py` (`_MIC_TO_YAHOO_SUFFIX`) | P2 | done (`src/mic.rs`, `src/isin.rs`) |

### Price-repair scope (P1 `partial`)

Upstream `utils.price_repair`/`solve_split_mul` uses the `quoteSummary`
(`price` module) + `chart` endpoints and a multi-pass split-continuity solver to
detect and rescale gaps. The vendored pin (`93eb4c2`) does **not** ship the
`scrapers/` tree or those functions, so a conservative split-continuity repair
is implemented in `src/history.rs` instead:

- Drops non-positive OHLC bars (same as upstream guard).
- When `HistoryOptions::repair` is set and split actions exist, scales every bar
  before a split by the split factor `num/den` so post-split and pre-split series
  are continuous. This is safe in `auto`/`back`/`none` adjust modes because
  re-applying the auto/back-adjust factor yields the same result.

The full upstream multi-endpoint reconciliation (volume/close divergence,
currency repair, dividend-driven anomalies) is **not** reproduced; this is why
the row stays `partial`.
