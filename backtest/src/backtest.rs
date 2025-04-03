use stock_pred::api::binance::Binance;
use stock_pred::config::SHARED_CONFIG;
use clap::Parser;
use tokio::time::{sleep, Duration};
use serde_json::Value;

/// A simple backtest tool for simulating a trade based on historical data.
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// The trading pair (e.g. FARMUSDT)
    token: String,
    /// The kline interval (e.g. 1h, 15m)
    interval: String,
    /// The number of candles to fetch (e.g. 48)
    limit: u16,
}

#[derive(Debug)]
struct Candle {
    open: f64,
    high: f64,
    low: f64,
    close: f64,
}

#[derive(Debug)]
pub struct Trade {
    pub entry_price: f64,
    pub exit_price: f64,
    pub multiplier: f64,
    pub entry_index: usize,
    /// If None, it means the trade was closed at the final candle.
    pub exit_index: Option<usize>,
}

pub async fn backtest_trade(
    binance: &Binance,
    token_symbol: &str,
    interval: &str,
    limit: u16,
) -> Result<f64, Box<dyn std::error::Error>> {
    // Fetch historical kline data for the token.
    let klines = binance.get_klines(token_symbol, interval, limit).await?;
    
    if klines.is_empty() {
        return Err("No kline data received".into());
    }
    
    // Assume each kline is a Vec<serde_json::Value> where:
    // index 1 is the open price and index 4 is the close price.
    let first_candle = &klines[0];
    let last_candle = klines.last().unwrap();
    
    let open_price_str = first_candle[1].as_str().unwrap_or("0");
    let close_price_str = last_candle[4].as_str().unwrap_or("0");
    
    let open_price = open_price_str.parse::<f64>()?;
    let close_price = close_price_str.parse::<f64>()?;
    
    // Calculate the gain percentage.
    let gain_percentage = ((close_price - open_price) / open_price) * 100.0;
    
    Ok(gain_percentage)
}

pub async fn parameter_sweep(
    binance: &Binance,
    token: &str,
    interval: &str,
    lookback_options: &[u16],
    recent_options: &[u16],
) {
    for &lookback in lookback_options {
        for &recent in recent_options {
            // For demonstration, we simply call backtest_trade using the lookback value.
            // You might want to extend backtest_trade or write a new function that also
            // factors in the 'recent' parameter in its analysis.
            match backtest_trade(binance, token, interval, lookback).await {
                Ok(gain) => {
                    println!(
                        "For token {} with lookback {} and recent {}: Gain = {:.2}%",
                        token, lookback, recent, gain
                    );
                }
                Err(e) => eprintln!("Error for lookback {} and recent {}: {}", lookback, recent, e),
            }
        }
    }
    
    sleep(Duration::from_secs(1)).await;
}

/// Parses raw candle data (Vec<Vec<Value>>) from Binance into a Vec<Candle>.
fn parse_candles(raw: Vec<Vec<Value>>) -> Vec<Candle> {
    raw.into_iter()
        .filter_map(|candle| {
            // Expecting each candle to have at least 5 fields: 
            // 0: timestamp, 1: open, 2: high, 3: low, 4: close.
            let open = candle.get(1)?.as_str()?.parse::<f64>().ok()?;
            let high = candle.get(2)?.as_str()?.parse::<f64>().ok()?;
            let low = candle.get(3)?.as_str()?.parse::<f64>().ok()?;
            let close = candle.get(4)?.as_str()?.parse::<f64>().ok()?;
            Some(Candle { open, high, low, close })
        })
        .collect()
}

/// Simulates a trailing stop trade with re-entry over a series of candles.
/// For each trade:
/// - Entry at the candle's open price.
/// - Update the highest price reached.
/// - If the candle's low falls below (highest_price * (1 - stop_loss_percent/100)),
///   exit at that stop level and re-enter at the next candle's open.
/// Returns the final multiplier for your investment.
fn simulate_trailing_trade(candles: &[Candle], stop_loss_percent: f64) -> (f64, Vec<Trade>) {
    let mut final_multiplier = 1.0;
    let mut trades = Vec::new();
    let mut i = 0;

    while i < candles.len() {
        let entry_price = candles[i].open;
        let mut highest_price = entry_price;
        let mut exit_index = None;

        for j in i..candles.len() {
            let candle = &candles[j];
            if candle.high > highest_price {
                highest_price = candle.high;
            }
            let stop_level = highest_price * (1.0 - stop_loss_percent / 100.0);
            if candle.low <= stop_level {
                exit_index = Some(j);
                break;
            }
        }

        if let Some(j) = exit_index {
            // Exit at the stop level.
            let exit_price = highest_price * (1.0 - stop_loss_percent / 100.0);
            let trade_multiplier = exit_price / entry_price;
            final_multiplier *= trade_multiplier;
            trades.push(Trade {
                entry_price,
                exit_price,
                multiplier: trade_multiplier,
                entry_index: i,
                exit_index: Some(j),
            });
            // Re-enter at the next candle.
            i = j + 1;
        } else {
            // If no stop was triggered, exit at the close of the last candle.
            let exit_price = candles[candles.len() - 1].close;
            let trade_multiplier = exit_price / entry_price;
            final_multiplier *= trade_multiplier;
            trades.push(Trade {
                entry_price,
                exit_price,
                multiplier: trade_multiplier,
                entry_index: i,
                exit_index: None,
            });
            break;
        }
    }
    (final_multiplier, trades)
}
/*
#[tokio::main]
async fn main() {
    // Parse command-line arguments.
    let args = Args::parse();

    // Create an instance of the Binance API.
    let binance = Binance::new();
    let current_config = SHARED_CONFIG.read().unwrap();
    let bt_lookback_options = current_config.bt_lookback_options.clone();
    let bt_recent_options = current_config.bt_recent_options.clone();
    //let lookback_options = config::get_lookback_options();
    //let recent_options = config::get_recent_options();

    println!(
        "Running backtest for {} over {} candles with interval {}...",
        args.token, args.limit, args.interval
    );
    match backtest_trade(&binance, &args.token, &args.interval, args.limit).await {
        Ok(gain) => println!("Backtest result: {:.2}% gain", gain),
        Err(e) => eprintln!("Backtest error: {}", e),
    }

     // Define ranges to test:
     let lookback_options = vec![6, 8, 12];  // e.g., test 6, 8, 12 candles (hours)
     let recent_options = vec![2, 4, 6];       // test 2, 4, 6 candles for recent trend
     parameter_sweep(&binance, &args.token, &args.interval, &bt_lookback_options, &bt_recent_options).await;
}
*/

#[tokio::main]
async fn main() {
    let args = Args::parse();
    let binance = Binance::new();
    let current_config = SHARED_CONFIG.read().unwrap();
    // Get the stop loss options from configuration (e.g., "3,5,10").
    let stop_loss_options = current_config.bt_stop_loss_options.clone();
    println!("Testing stop loss options: {:?}", stop_loss_options);
    
    println!(
        "Fetching {} candles for {} with interval {}...",
        args.limit, args.token, args.interval
    );
    // Fetch candles from Binance.
    let raw_candles = match binance.get_klines(&args.token, &args.interval, args.limit).await {
        Ok(data) => data,
        Err(e) => {
            eprintln!("Error fetching klines: {}", e);
            return;
        }
    };

    let candles = parse_candles(raw_candles);
    if candles.is_empty() {
        eprintln!("No candle data available for simulation.");
        return;
    }

    println!("Fetched {} candles.", candles.len());
    

    for &stop_loss_percent in stop_loss_options.iter() {
        // Convert stop_loss_percent to f64 if needed.
        let (final_multiplier, trade_details) = simulate_trailing_trade(&candles, stop_loss_percent as f64);
        let total_profit = (final_multiplier - 1.0) * 100.0;
        println!("Stop loss {}% -> Final multiplier: {:.4} (Total Profit: {:+.2}%)", 
            stop_loss_percent, final_multiplier, total_profit);
        println!("Trade details:");
        for trade in trade_details {
            match trade.exit_index {
                Some(idx) => {
                    println!(
                        "  Trade from candle {}: entry at {:.2}, exit at {:.2}, multiplier: {:.4}",
                        trade.entry_index + 1, trade.entry_price, trade.exit_price, trade.multiplier
                    );
                },
                None => {
                    println!(
                        "  Final trade starting at candle {}: entry at {:.2}, exit at {:.2} (final), multiplier: {:.4}",
                        trade.entry_index + 1, trade.entry_price, trade.exit_price, trade.multiplier
                    );
                }
            }
        }
        println!("-------------------------------------------------");
    }

    tokio::time::sleep(Duration::from_secs(1)).await;
}