//! P4 example: live streaming + authentication (mirrors yfinance `live` and
//! `Auth`).
//!
//! Streaming runs against the real Yahoo streamer; we cap it at ~6s for the
//! demo:
//!
//! ```sh
//! cargo run --example p4
//! ```
//!
//! Authentication requires real Yahoo `T`/`Y` cookies (set `YF_T`/`YF_Y` env
//! vars or edit the call below). Without cookies, `check_login` returns
//! `false` and the tier is `None`.

use std::time::Duration;

use adaq_data_stock_us::{Client, LiveWebSocket, PricingData};

#[tokio::main]
async fn main() -> adaq_data_stock_us::Result<()> {
    let client = Client::new()?;

    // --- Authentication (optional; needs real cookies) ---
    let auth = client.auth();
    // If you have cookies, inject them:
    // auth.set_login_cookies(std::env::var("YF_T").unwrap(), std::env::var("YF_Y").unwrap()).await?;
    println!("logged in: {}", auth.check_login().await?);
    println!("tier: {:?}", auth.subscription_tier().await?);
    println!("user guid: {:?}", auth.user().await?);

    // --- Live stream (capped at 6s for the demo) ---
    let ws = LiveWebSocket::new().verbose(true);
    let task = tokio::spawn(async move {
        ws.stream(&["AAPL", "MSFT"], |tick: PricingData| {
            println!(
                "tick {} price={:.2} chg%={:.2} vol={}",
                tick.id, tick.price, tick.change_percent, tick.day_volume
            );
        })
        .await
    });
    match tokio::time::timeout(Duration::from_secs(6), task).await {
        Ok(Ok(Ok(()))) => println!("stream ended cleanly"),
        Ok(Ok(Err(e))) => eprintln!("stream error: {e}"),
        Ok(Err(e)) => eprintln!("task join error: {e}"),
        Err(_) => println!("stream demo capped at 6s"),
    }

    Ok(())
}
