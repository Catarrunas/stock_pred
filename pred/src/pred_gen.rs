use dotenv::dotenv;
use stock_pred::api::binance::Binance;
use stock_pred::config::SHARED_CONFIG;

#[tokio::main]
async fn main() {
    // Load environment variables from vars.env (or .env)
    dotenv().ok();

    // Optionally initialize logging if you have that set up.
    // init_tracing(false, Level::INFO);

    let binance = Binance::new();

     // 4. For each filtered token, fetch klines and analyze detailed growth.
     let interval = "1h";
     // Assuming the interval is "1h"
     // Extract values from the shared config
     let (lookback_period, last_hours_period) = {
         let current_config = SHARED_CONFIG.read().unwrap();
         (current_config.lookback_period, current_config.last_hours_period)
     };

    // Fetch aggregated 24hr ticker data from Binance.
    let all_tickers = match binance.get_all_ticker_24hr().await {
        Ok(tickers) => tickers,
        Err(e) => {
            eprintln!("Error fetching aggregated ticker data: {}", e);
            return;
        }
    };

    // For testing, filter tickers that have a positive 24h price change.
    let filtered_tickers: Vec<_> = all_tickers.into_iter()
        .filter(|ticker| {
            // Parse the priceChangePercent as f64; if parsing fails, default to 0.0.
            ticker.priceChangePercent.parse::<f64>().unwrap_or(0.0) > 0.0
        })
        .collect();

    println!("Filtered tickers meeting criteria:\n{:#?}", filtered_tickers.len());

    for token_symbol in filtered_tickers {
        match binance.get_klines(&token_symbol.symbol, interval, lookback_period).await {
            Ok(klines) => {
                if klines.len() < lookback_period as usize {
                    eprintln!("Not enough data for {}.", token_symbol.symbol);
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
                    let overall_growth = ((last_close - open_price) / open_price) * 100.0;
                    let current_trend_up = last_close > prev_close;
    
                    if overall_growth >= 10.0 && current_trend_up {
                        // Use last_hours_period for recent trend analysis.
                        let recent_range = last_hours_period as usize;
                        let last_recent_candles = &klines[klines.len() - recent_range..];
                        let first_recent_open_str = last_recent_candles[0][1].as_str().unwrap_or("0");
                        let last_recent_close_str = last_recent_candles[last_recent_candles.len() - 1][4].as_str().unwrap_or("0");
    
                        if let (Ok(first_recent_open), Ok(last_recent_close)) = (
                            first_recent_open_str.parse::<f64>(),
                            last_recent_close_str.parse::<f64>(),
                        ) {
                            let recent_growth = ((last_recent_close - first_recent_open) / first_recent_open) * 100.0;
                            if recent_growth > 0.0 {
                                println!(
                                    "🚀 {} is pumping with {:.2}% overall growth in the last {} hours and {:.2}% growth in the last {} hours!",
                                    token_symbol.symbol, overall_growth, lookback_period, recent_growth, last_hours_period
                                );
                                
    
                                // Optionally, calculate average fluctuations.
                                let mut raw_fluctuations = Vec::new();
                                let mut percent_fluctuations = Vec::new();
                                for candle in &klines {
                                    let high_str = candle[2].as_str().unwrap_or("0");
                                    let low_str = candle[3].as_str().unwrap_or("0");
                                    if let (Ok(high), Ok(low)) = (high_str.parse::<f64>(), low_str.parse::<f64>()) {
                                        let diff = high - low;
                                        raw_fluctuations.push(diff);
                                        if low > 0.0 {
                                            percent_fluctuations.push((diff / low) * 100.0);
                                        }
                                    }
                                }
                                if !raw_fluctuations.is_empty() && !percent_fluctuations.is_empty() {
                                    let avg_raw_fluctuation: f64 =
                                        raw_fluctuations.iter().sum::<f64>() / raw_fluctuations.len() as f64;
                                    let avg_percent_fluctuation: f64 =
                                        percent_fluctuations.iter().sum::<f64>() / percent_fluctuations.len() as f64;
                                    println!(
                                        "For token {}: Average raw fluctuation: {:.4}, Average percent fluctuation: {:.2}%",
                                        token_symbol.symbol, avg_raw_fluctuation, avg_percent_fluctuation
                                    );
                                }
                            }
                        } else {
                            eprintln!("Failed to parse 4-hour data for {}.", token_symbol.symbol);
                        }
                    } else {
                        // Optionally log that token did not meet growth criteria.
                    }
                } else {
                    eprintln!("Failed to parse price data for {}.", token_symbol.symbol);
                }
            }
            Err(e) => eprintln!("Error fetching klines for {}: {}", token_symbol.symbol, e),
        }
    }
}