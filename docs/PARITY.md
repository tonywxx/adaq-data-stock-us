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
| `quote` (info/fast_info/holders/analysis/funds) | `scrapers/quote.py`, `scrapers/holders.py`, `scrapers/analysis.py`, `scrapers/funds.py` | P2 | done |
| `fundamentals` (3 statements) | `scrapers/fundamentals.py` | P2 | done |
| `options` (option chain) | `ticker.py` (`option_chain`) | P2 | done |
| `domain` (Sector/Industry/Market) | `domain/*.py` | P3 | done |
| `search` / `lookup` | `search.py`, `lookup.py` | P3 | done |
| `calendars` | `calendars.py` | P3 | done |
| `screener` | `screener/query.py`, `screener/screener.py` | P3 | done |
| `live` (WebSocket) | `live.py`, `pricing.proto` | P4 | done |
| `auth` (login) | `data.py` (`Auth`) | P4 | done |
| `config` | `config.py` | P1 | done |

## Shared / cross-cutting

| Concern | yfinance source | Phase | Status |
|---------|-----------------|-------|--------|
| Parsing & price-repair | `utils.py` | P1 | partial |
| Endpoints & URL constants | `const.py` | P1 | done |
| ISIN / MIC resolution | `base.py` (`get_isin`), `const.py` (`_MIC_TO_YAHOO_SUFFIX`) | P2 | todo |
