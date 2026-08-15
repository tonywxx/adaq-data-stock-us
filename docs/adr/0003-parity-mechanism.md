# Parity-tracking mechanism back to `yfinance`

We keep the crate aligned to upstream `yfinance` via three coordinated pieces: (1) `yfinance` vendored as a git submodule pinned to a release tag under `vendor/yfinance/`, (2) `docs/PARITY.md` mapping each Rust module to its `yfinance` source file(s), version, and status, and (3) an `xtask` workspace member exposing `cargo xtask parity` that diffs the submodule since the last-aligned pin, cross-references `PARITY.md`, and flags Rust modules whose upstream source changed but are marked done/partial.

**Why**
The user requires "when yfinance updates, my code updates too." Perfect auto-sync is impossible (the Rust API is not a textual transform of Python), but we can make drift *visible and reviewable*: the submodule shows exactly what changed upstream, `PARITY.md` records what we've mirrored, and the parity check turns "forgot to update" into a flagged diff.

**Considered Options**
- Submodule + `PARITY.md` + `cargo xtask parity` (chosen): faithful source-of-truth, automated drift detection, no external service.
- Submodule + docs only, manual checks: lighter, but drift is easy to forget.
- CI cron that auto-bumps the submodule and opens a PR: strongest automation, but a PR-bot is heavier than this port needs initially; can be layered on later.

**Consequences**
- `vendor/yfinance/` is gitignored-content but tracked as a gitlink; the pinned commit is recorded in `PARITY_PIN` at repo root and reflected in `PARITY.md`.
- Updating to a new yfinance release = bump the submodule, refresh `PARITY_PIN`, re-run `cargo xtask parity`, update `PARITY.md` statuses.
- The `xtask` member is `publish = false`; only the `adaq-data-stock-us` lib is published to crates.io.
