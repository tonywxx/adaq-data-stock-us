# Typed structs as the canonical return type

The canonical return type for every public API is a **strongly-typed Rust struct**; tabular consumers get a thin, optional conversion layer to `polars` / `arrow` / `serde` (JSON).

**Why**
yfinance returns dynamic `pandas` DataFrames everywhere, so shape mismatches surface only at runtime. Typed structs give compile-time guarantees on field presence and types, and make "extend as needed" controllable — new fields are explicit, not silent DataFrame columns. The conversion layer exists because downstream quant code often wants a DataFrame; it is deliberately secondary, not the contract.

**Considered Options**
- Typed structs + optional `polars`/`arrow`/`serde` conversion (chosen): safe by default, opt into tables. `polars`/`arrow` are Cargo features off by default so the core stays lean; `serde` (JSON) is always on (near-zero cost, universally useful).
- `polars` DataFrame as the universal container (mirrors pandas): closest to yfinance ergonomics but forces a heavy dependency on every caller and loses compile-time field guarantees.
- Self-rolled lightweight DataFrame: avoids the heavy dep but reinvents parsing/alignment and invites bugs.

**Consequences**
- `history()` returns a `History` struct (with typed `Bar` rows), not a `DataFrame`. Callers wanting a table call `.to_polars()` behind the `polars` feature.
- Adding a yfinance field is a struct change + a conversion tweak, reviewed explicitly — no silent schema drift.
