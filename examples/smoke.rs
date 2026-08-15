//! Smoke test for P1+P2: history, info, fast_info, holders, financials, options.
//! Run: `cargo run --example smoke`

use adaq_data_stock_us::blocking::Client;
use adaq_data_stock_us::fundamentals::{Freq, Statement};
use adaq_data_stock_us::history::{HistoryOptions, Interval};

fn main() -> adaq_data_stock_us::Result<()> {
    let client = Client::new()?;

    let opts = HistoryOptions {
        interval: Interval::Day1,
        period: "5d".into(),
        ..Default::default()
    };
    let h = client.history("AAPL", &opts)?;
    println!("[history] AAPL bars={}", h.bars.len());

    let info = client.info("AAPL")?;
    println!(
        "[info] {} | {} | mktcap={:?} | pe={:?}",
        info.symbol.as_deref().unwrap_or("?"),
        info.short_name.as_deref().unwrap_or("?"),
        info.market_cap,
        info.trailing_pe
    );

    let fi = client.fast_info("AAPL")?;
    println!(
        "[fast_info] last={:?} mktcap={:?} prev={:?}",
        fi.last_price, fi.market_cap, fi.previous_close
    );

    let fin = client.financials("AAPL", Statement::Income, Freq::Annual)?;
    println!(
        "[financials/income/annual] dates={}, items={}, totalRevenue@{} = {:?}",
        fin.dates.len(),
        fin.items.len(),
        fin.dates.first().unwrap_or(&"?".into()),
        fin.get("totalRevenue", fin.dates.first().unwrap_or(&String::new()))
    );

    let chain = client.option_chain("AAPL")?;
    println!(
        "[options] expirations={}, first={:?}, strikes={}, contracts@first={}",
        chain.expirations.len(),
        chain.expirations.first(),
        chain.strikes.len(),
        chain
            .options
            .first()
            .map(|o| o.calls.len() + o.puts.len())
            .unwrap_or(0)
    );

    Ok(())
}
