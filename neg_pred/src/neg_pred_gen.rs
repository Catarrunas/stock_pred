use dotenv::dotenv;
use stock_pred::api::binance::Binance;
use stock_pred::config::SHARED_CONFIG;

#[tokio::main]
async fn main() {
    // Load environment variables from vars.env (or .env)
    dotenv().ok();

    // Create your Binance instance.
    let binance = Binance::new();

    // Define the interval and fetch the shared config values.
    let interval = "1h";
    let (lookback_period, last_hours_period) = {
        let current_config = SHARED_CONFIG.read().unwrap();
        (current_config.lookback_period, current_config.last_hours_period)
    };

    // Fetch aggregated 24hr ticker data.
    let all_tickers = match binance.get_all_ticker_24hr().await {
        Ok(tickers) => tickers,
        Err(_e) => {
            return;
        }
    };

    // For negative trends, filter tickers with a negative 24h price change.
    let filtered_tickers: Vec<_> = all_tickers.into_iter()
        .filter(|ticker| {
            // Parse the priceChangePercent as f64; if parsing fails, default to 0.0.
            ticker.priceChangePercent.parse::<f64>().unwrap_or(0.0) < 0.0
        })
        .collect();

    println!("Filtered tickers meeting negative trend criteria: {}", filtered_tickers.len());

    // Process each filtered token.
    for token in filtered_tickers {
        // token is of type Ticker24hr; use its symbol for the klines call.
        match binance.get_klines(&token.symbol, interval, lookback_period).await {
            Ok(klines) => {
                if klines.len() < lookback_period as usize {
                    eprintln!("Not enough data for {}.", token.symbol);
                    continue;
                }
                let first_candle = &klines[0];
                let second_last_candle = &klines[klines.len() - 2];
                let last_candle = &klines[klines.len() - 1];

                let open_price_str = first_candle[1].as_str().unwrap_or("0");
                let last_close_str = last_candle[4].as_str().unwrap_or("0");
                let prev_close_str = second_last_candle[4].as_str().unwrap_or("0");

                if let (Ok(open_price), Ok(last_close), Ok(prev_close)) = (
                    open_price_str.parse::<f64>(),
                    last_close_str.parse::<f64>(),
                    prev_close_str.parse::<f64>(),
                ) {
                    let overall_change = ((last_close - open_price) / open_price) * 100.0;
                    let current_trend_down = last_close < prev_close;

                    // For a dump, we expect an overall negative change.
                    if overall_change <= -10.0 && current_trend_down {
                        // Check recent trend over the last_hours_period.
                        let recent_range = last_hours_period as usize;
                        let last_recent_candles = &klines[klines.len() - recent_range..];
                        let first_recent_open_str = last_recent_candles[0][1].as_str().unwrap_or("0");
                        let last_recent_close_str = last_recent_candles[last_recent_candles.len() - 1][4].as_str().unwrap_or("0");

                        if let (Ok(first_recent_open), Ok(last_recent_close)) = (
                            first_recent_open_str.parse::<f64>(),
                            last_recent_close_str.parse::<f64>(),
                        ) {
                            let recent_change = ((last_recent_close - first_recent_open) / first_recent_open) * 100.0;
                            if recent_change < 0.0 {
                                println!(
                                    "🔻 {} is dumping with {:.2}% overall change and {:.2}% recent change over the last {} hours!",
                                    token.symbol, overall_change, recent_change, last_hours_period
                                );
                                // Optionally, compute further statistics such as average fluctuations...
                            }
                        } else {
                            eprintln!("Failed to parse recent price data for {}.", token.symbol);
                        }
                    } else {
                        // Optionally log that token did not meet negative criteria.
                    }
                } else {
                    eprintln!("Failed to parse price data for {}.", token.symbol);
                }
            },
            Err(e) => eprintln!("Error fetching klines for {}: {}", token.symbol, e),
        }
    }

    // Optionally, sleep or loop if this is part of an ongoing market-check process.
}