//! Demonstrates ticker identifier resolution and per-ticker news / earnings /
//! ISIN. Run: `cargo run --example ticker_id`
//!
//! Covers the yfinance `Ticker` construction forms (`symbol`, `(symbol, MIC)`,
//! ISIN) plus `get_news` / `get_earnings_dates` / `get_isin` parity.

use adaq_data_stock_us::TickerId;
use adaq_data_stock_us::blocking::Client;

fn main() -> adaq_data_stock_us::Result<()> {
    let client = Client::new()?;

    // 1. MIC pair -> "OR.PA"
    let or = client.ticker_from_mic("OR", "XPAR")?;
    println!("[mic] OR/XPAR resolved to {}", or.symbol());

    // 2. ISIN -> "AAPL"
    let aapl = client.ticker_from_isin("US0378331005")?;
    println!("[isin] US0378331005 -> {}", aapl.symbol());

    // 3. Any TickerId
    let _ = client.ticker_from_id(TickerId::Symbol("MSFT".into()))?;

    // 4. Per-ticker news
    let news = client.news("AAPL", 3, "news")?;
    println!("[news] {} articles", news.len());
    for a in news.iter().take(3) {
        println!(
            "  - {} ({})",
            a.title.as_deref().unwrap_or("?"),
            a.publisher.as_deref().unwrap_or("?")
        );
    }

    // 5. Per-ticker earnings dates
    let eds = client.earnings_dates("AAPL", 4)?;
    println!("[earnings_dates] {} entries", eds.len());
    for e in &eds {
        println!(
            "  - {:?} est={:?} actual={:?} surprise%={:?}",
            e.date, e.eps_estimate, e.eps_actual, e.surprise_pct
        );
    }

    // 6. Reverse lookup: ticker -> ISIN
    let isin = client.isin("AAPL")?;
    println!("[isin reverse] AAPL -> {}", isin);

    Ok(())
}
