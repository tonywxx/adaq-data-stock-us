//! P3 smoke test: search, lookup, domain, calendars, and screener.
//!
//! Run with: `cargo run --example p3`

use adaq_data_stock_us::Client;
use adaq_data_stock_us::domain::MarketRegion;
use adaq_data_stock_us::screener::{Operand, Operator, Query, ScreenOptions};

#[tokio::main]
async fn main() -> adaq_data_stock_us::Result<()> {
    let client = Client::new()?;

    // --- search ---
    let s = client.search("apple", 3, 0).await?;
    println!("search 'apple': {} quotes", s.quotes.len());
    for q in &s.quotes {
        println!(
            "  {}  {}",
            q.symbol.as_deref().unwrap_or("?"),
            q.short_name.as_deref().unwrap_or("")
        );
    }

    // --- lookup ---
    let l = client.lookup("tesla", 5, "equity").await?;
    println!("lookup 'tesla': {} results", l.results.len());

    // --- domain: market summary for US ---
    let m = client.market(MarketRegion::Us).await?;
    println!("market us: {} rows, status={:?}", m.summary.len(), m.status);

    // --- calendars: earnings this month ---
    let today = chrono::Utc::now().date_naive();
    let start = today.format("%Y-%m-%d").to_string();
    let end = (today + chrono::Duration::days(14))
        .format("%Y-%m-%d")
        .to_string();
    let earn = client.earnings_calendar(&start, &end, 25).await?;
    println!("earnings {}..{}: {} events", start, end, earn.len());
    for e in earn.iter().take(5) {
        println!(
            "  {}  {}  eps_est={:?}",
            e.ticker.as_deref().unwrap_or("?"),
            e.start_date.as_deref().unwrap_or(""),
            e.eps_estimate
        );
    }

    // --- screener: predefined ---
    let res = client
        .screen("day_gainers", &ScreenOptions::default())
        .await?;
    println!(
        "day_gainers: total={:?}, returned {}",
        res.total,
        res.quotes.len()
    );
    for q in res.quotes.iter().take(5) {
        println!(
            "  {}  price={:?}  change%={:?}",
            q.symbol.as_deref().unwrap_or("?"),
            q.price,
            q.percent_change
        );
    }

    // --- screener: custom ---
    let q = Query::equity(
        Operator::And,
        vec![
            Operand::query(Query::equity(
                Operator::Gt,
                vec![Operand::field("percentchange"), Operand::value(3.0)],
            )),
            Operand::query(Query::equity(
                Operator::Eq,
                vec![Operand::field("region"), Operand::value("us")],
            )),
        ],
    );
    let res2 = client.screen(q, &ScreenOptions::default()).await?;
    println!(
        "custom screen: total={:?}, returned {}",
        res2.total,
        res2.quotes.len()
    );

    Ok(())
}
