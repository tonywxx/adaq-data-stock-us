use adaq_data_stock_us::TickerId;
use adaq_data_stock_us::blocking::Client;

fn main() -> adaq_data_stock_us::Result<()> {
    let client = Client::new()?;

    // Bare symbol
    let aapl = client.ticker("AAPL");
    println!("{}", aapl.symbol());

    // (symbol, MIC) pair -> "OR.PA"
    let or = client.ticker_from_mic("OR", "XPAR")?;
    println!("{}", or.symbol());

    // ISIN -> "AAPL"  (US0378331005)
    let by_isin = client.ticker_from_isin("US0378331005")?;
    println!("{}", by_isin.symbol());

    // Any identifier
    let _ = client.ticker_from_id(TickerId::Symbol("MSFT".into()))?;
    Ok(())
}
