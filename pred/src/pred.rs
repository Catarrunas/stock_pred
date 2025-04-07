use stock_pred::api::binance::Binance;
use stock_pred::trading::execution;
use stock_pred::trading::execution::Order;
use stock_pred::config::{SHARED_CONFIG, watch_config};
use stock_pred::logging::init_tracing;
use std::collections::HashSet;
use std::sync::Arc;
use tokio::time::{sleep, Duration};
use chrono::Utc;
use tokio::sync::Mutex;
use tracing::{debug, info, span, Level};

   

// Move update_orders_loop outside of main.
async fn update_orders_loop(open_orders: Arc<Mutex<Vec<Order>>>) {
    loop {
        {
            // Await the lock on the orders.
            let timestamp = Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
            println!("[{}] Running stop loss update loop", timestamp);
            info!("[{}] Running stop loss update loop", timestamp);
            let mut orders = open_orders.lock().await;
            for order in orders.iter_mut() {
                let simulated_current_price = order.purchase_price * 1.05;
                //let stop_loss_percent = get_stop_loss_percent();
                 let stop_loss_percent: f64 = 5.0;
                // Await the async update_stop_loss function.
                execution::update_stop_loss(order, simulated_current_price, stop_loss_percent).await;
                print!("Order {} updated. ", order.token);
                info!("Order {} updated. ", order.token);
            }
        }
        // Read the current order update interval from the shared config.
        let order_update_interval = {
            let config = SHARED_CONFIG.read().unwrap();
            config.order_update_interval
        };
        println!("Sleeping for {} seconds before updating orders...", order_update_interval);
        sleep(Duration::from_secs(order_update_interval)).await;
    }
}

#[tokio::main]
async fn main() {
    // Initialize logging (this sets up the reloadable layer).
    let _guard = init_tracing(false, Level::INFO);
    info!("Application started with logging!");
    
    let binance = Binance::new();

    // Parse the list of assets from the environment variable QUOTE_ASSETS and transaction amounts from the config.
    let (assets, transaction_amounts) = {
        let config = SHARED_CONFIG.read().unwrap();
        (config.quote_assets.clone(), config.transaction_amounts.clone())
    };
    println!("Assets to scan: {:?}", assets);
    info!("Assets to scan: {:?}", assets);


    // Use Arc with tokio::sync::Mutex for async-friendly shared state.
    let open_orders: Arc<Mutex<Vec<Order>>> = Arc::new(Mutex::new(Vec::new()));

    // Spawn the market-check loop.
    let binance_clone = binance; // clone if needed
    let open_orders_clone = Arc::clone(&open_orders);
    let market_check_handle = tokio::spawn(async move {
        loop {

            let timestamp = Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
            println!("[{}] Starting market check iteration...", timestamp);
            info!("[{}] Starting market check iteration...", timestamp);

             // 1. Since we fecth all tickers for all assers we can get this 1 time then work with the filtered tokens later.
             let all_tickers = match binance_clone.get_all_ticker_24hr().await {
                Ok(tickers) => tickers,
                Err(e) => {
                    eprintln!("Error fetching aggregated ticker data: {}", e);
                    info!("Error fetching aggregated ticker data: {}", e);
                    // Sleep a short time to avoid hammering the API:
                    sleep(Duration::from_secs(1)).await;
                    continue;
                }
            };

            // 2. Filter tokens: pairs with positive 24hr change and not already invested.
            let invested_tokens: HashSet<String> = {
                let orders = open_orders_clone.lock().await;
                orders.iter().map(|order| order.token.clone()).collect()
            };

            let all_positive_tokens: Vec<String> = all_tickers
                .into_iter()
                .filter_map(|ticker| {
                    if let Ok(change_percent) = ticker.priceChangePercent.parse::<f64>() {
                        if change_percent > 0.0 && !invested_tokens.contains(&ticker.symbol) {
                            Some(ticker.symbol)
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                })
                .collect();

            println!("Filtered tokens: {} tokens with positive 24r hours change",all_positive_tokens.len());
            info!("Filtered tokens: {} tokens with positive 24r hours change", all_positive_tokens.len());
        
            
            for (i, asset) in assets.iter().enumerate() {
                println!("Processing asset: {}", asset);
                info!("Processing asset: {}", asset);
        
                // 3. Check available balance for the asset.
                let available_balance = match binance_clone.get_account_balance(asset).await {
                    Ok(balance) => balance,
                    Err(e) => {
                        eprintln!("Error fetching balance for {}: {}", asset, e);
                        info!("Error fetching balance for {}: {}", asset, e);
                        continue;
                    }
                };
        
                println!("Asset: {} has balance: {}", asset, available_balance);
                info!("Asset: {} has balance: {}", asset, available_balance);
            
                // Get the per-asset transaction amount.
                // If there isn't one for the current index, fallback to a global transaction_amount.
                let asset_transaction_amount = transaction_amounts.get(i).copied().unwrap_or_else(|| {
                    let config = SHARED_CONFIG.read().unwrap();
                    config.transaction_amount
                });
                
                if available_balance < asset_transaction_amount {
                    println!("Balance for {} (${:.2}) is below the required transaction amount (${:.2}). Skipping asset.", asset, available_balance, asset_transaction_amount);
                    info!("Balance for {} (${:.2}) is below the required transaction amount (${:.2}). Skipping asset.", asset, available_balance, asset_transaction_amount);
                    continue;
                }
                
                // 4. For each filtered token, fetch klines and analyze detailed growth.
                let interval = "1h";
                // Assuming the interval is "1h"
                // Extract values from the shared config
                let (lookback_period, last_hours_period) = {
                    let current_config = SHARED_CONFIG.read().unwrap();
                    (current_config.lookback_period, current_config.last_hours_period)
                };

                let filtered_tokens: Vec<String> = all_positive_tokens.clone()
                    .into_iter()
                    .filter(|symbol| symbol.ends_with(asset))
                    .collect();         

                println!("Asset {} have {} filtered tokens",asset,filtered_tokens.len());
                info!("Asset {} have {} filtered tokens",asset, filtered_tokens.len());

                for token_symbol in filtered_tokens {
                    match binance_clone.get_klines(&token_symbol, interval, lookback_period).await {
                        Ok(klines) => {
                            if klines.len() < lookback_period as usize {
                                eprintln!("Not enough data for {}.", token_symbol);
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
                                // Use the last_hours_period for recent trend.
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
                                                println!("🚀 {} is pumping with {:.2}% overall growth in the last {} hours and {:.2}% growth in the last {} hours!",token_symbol, overall_growth, lookback_period, recent_growth, last_hours_period);
                                                info!("🚀 {} is pumping with {:.2}% overall growth in the last {} hours and {:.2}% growth in the last {} hours!",token_symbol, overall_growth, lookback_period, recent_growth, last_hours_period);
                                                // Optionally, add further analysis here...
                                                // For instance, calculate average fluctuation:
                                                let mut raw_fluctuations = Vec::new();
                                                let mut percent_fluctuations = Vec::new();
                                                for candle in &klines {
                                                        // Assuming candle format: [timestamp, open, high, low, close, ...]
                                                        let high_str = candle[2].as_str().unwrap_or("0");
                                                        let low_str = candle[3].as_str().unwrap_or("0");
                                                        if let (Ok(high), Ok(low)) = (high_str.parse::<f64>(), low_str.parse::<f64>()) {
                                                            let diff = high - low;
                                                            raw_fluctuations.push(diff);
                                                            // Compute percentage fluctuation relative to low (or you can use open or average price)
                                                            if low > 0.0 {
                                                                percent_fluctuations.push((diff / low) * 100.0);
                                                            }
                                                        }
                                                    }
                                                    if !raw_fluctuations.is_empty() && !percent_fluctuations.is_empty() {
                                                        let avg_raw_fluctuation: f64 = raw_fluctuations.iter().sum::<f64>() / raw_fluctuations.len() as f64;
                                                        let avg_percent_fluctuation: f64 = percent_fluctuations.iter().sum::<f64>() / percent_fluctuations.len() as f64;
                                                        println!("For token {}: Average raw fluctuation: {:.4}, Average percent fluctuation: {:.2}%",token_symbol, avg_raw_fluctuation, avg_percent_fluctuation);
                                                        info!("For token {}: Average raw fluctuation: {:.4}, Average percent fluctuation: {:.2}%",token_symbol, avg_raw_fluctuation, avg_percent_fluctuation);

                                                        // Here, compare avg_percent_fluctuation with your current stop loss percentage.
                                                        // For example, if your stop loss is set at 2% and avg_percent_fluctuation is 5%,
                                                        // you might consider adjusting the stop loss to avoid premature triggering.
                                                    // Continue with your other analysis, e.g., checking overall growth.
                                                }
                                            } else {
                                                //println!("{} meets overall criteria but not the 4-hour trend.", token_symbol);
                                            }
                                        } else {
                                            eprintln!("Failed to parse 4-hour data for {}.", token_symbol);
                                        }
                                    } else {
                                        //println!("{} does not meet the pump criteria.", token_symbol);
                                    }
                                } else {
                                    eprintln!("Failed to parse price data for {}.", token_symbol);
                                }
                            },
                            Err(e) => eprintln!("Error fetching klines for {}: {}", token_symbol, e),
                        }
                    }
            }
        // Extract values from the shared config
        // Extract values and drop the guard immediately:
        let loop_time = {
           let current_config = SHARED_CONFIG.read().unwrap();
           current_config.loop_time_seconds
           };
           println!("-------------------------------------------------------------------------");
           println!("Sleeping for {} seconds before the next iteration...", loop_time);
           info!("Sleeping for {} seconds before the next iteration...", loop_time);
           // Now call sleep without holding the lock:
           sleep(Duration::from_secs(loop_time)).await;
        }
    });


    // Spawn the stop-loss update loop.
    let open_orders_for_update = Arc::clone(&open_orders);
    let stop_loss_update_handle = tokio::spawn(async move {
        update_orders_loop(open_orders_for_update).await;
    });

    // Await both loops indefinitely.
    let _ = tokio::join!(market_check_handle, stop_loss_update_handle);
    //let _ = tokio::join!(market_check_handle);
}