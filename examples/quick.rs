//! Quick smoke test: fetch AAPL history + actions via the blocking API.
//! Run with: `cargo run --example quick`

use adaq_data_stock_us::blocking::Client;
use adaq_data_stock_us::history::{HistoryOptions, Interval};

fn main() -> adaq_data_stock_us::Result<()> {
    let client = Client::new()?;
    let opts = HistoryOptions {
        interval: Interval::Day1,
        period: "5d".into(),
        actions: true,
        ..Default::default()
    };

    let hist = client.history("AAPL", &opts)?;
    println!("AAPL bars: {}", hist.bars.len());
    if let Some(first) = hist.bars.first() {
        println!(
            "first bar: {} O={:?} H={:?} L={:?} C={:?} V={:?}",
            first.datetime, first.open, first.high, first.low, first.close, first.volume
        );
    }
    if let Some(actions) = &hist.actions {
        println!(
            "actions: {} dividends, {} splits",
            actions.dividends.len(),
            actions.splits.len()
        );
    }
    if let Some(meta) = Some(&hist.meta) {
        println!(
            "currency={:?} exchange={:?} tz={:?}",
            meta.currency, meta.exchange, meta.timezone
        );
    }
    Ok(())
}
