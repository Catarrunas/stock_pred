use std::error::Error;
use serde_json::Value;
use stock_pred::api::binance::Binance;
use tokio::time::{sleep, Duration};
use clap::Parser;

/// Enum to indicate the type of trend.
#[derive(Debug, Clone, Copy)]
pub enum TrendType {
    Positive,
    Negative,
}

impl TrendType {
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "negative" => TrendType::Negative,
            _ => TrendType::Positive, // default to positive
        }
    }
}

#[derive(Debug)]
pub struct Candle {
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
}

#[derive(Debug)]
pub struct Trade {
    pub entry_price: f64,
    pub exit_price: f64,
    pub multiplier: f64,
    pub entry_index: usize,
    /// If None, the trade closed at the final candle.
    pub exit_index: Option<usize>,
}

/// Parses raw candle data (Vec<Vec<Value>>) from Binance into a Vec<Candle>.
fn parse_candles(raw: Vec<Vec<Value>>) -> Vec<Candle> {
    raw.into_iter()
        .filter_map(|candle| {
            let open = candle.get(1)?.as_str()?.parse::<f64>().ok()?;
            let high = candle.get(2)?.as_str()?.parse::<f64>().ok()?;
            let low = candle.get(3)?.as_str()?.parse::<f64>().ok()?;
            let close = candle.get(4)?.as_str()?.parse::<f64>().ok()?;
            Some(Candle { open, high, low, close })
        })
        .collect()
}

/// Simulates a trailing stop trade for positive trends.
/// Entry at candle open; updates highest price; exits when candle low falls below (highest * (1-stop_loss_percent/100)).
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
            i = j + 1;
        } else {
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

/// Simulates a trailing stop trade for negative trends.
/// Entry at candle open; updates lowest price; exits when candle high rises above (lowest * (1 + stop_loss_percent/100)).
fn simulate_trailing_trade_negative(candles: &[Candle], stop_loss_percent: f64) -> (f64, Vec<Trade>) {
    let mut final_multiplier = 1.0;
    let mut trades = Vec::new();
    let mut i = 0;

    while i < candles.len() {
        let entry_price = candles[i].open;
        let mut lowest_price = entry_price;
        let mut exit_index = None;

        for j in i..candles.len() {
            let candle = &candles[j];
            if candle.low < lowest_price {
                lowest_price = candle.low;
            }
            let stop_level = lowest_price * (1.0 + stop_loss_percent / 100.0);
            if candle.high >= stop_level {
                exit_index = Some(j);
                break;
            }
        }

        if let Some(j) = exit_index {
            let exit_price = lowest_price * (1.0 + stop_loss_percent / 100.0);
            let trade_multiplier = exit_price / entry_price;
            final_multiplier *= trade_multiplier;
            trades.push(Trade {
                entry_price,
                exit_price,
                multiplier: trade_multiplier,
                entry_index: i,
                exit_index: Some(j),
            });
            i = j + 1;
        } else {
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

/// Unified backtest function that uses trailing stop simulation for both positive and negative trends.
pub async fn backtest_trade(
    binance: &Binance,
    token_symbol: &str,
    interval: &str,
    limit: u16,
    stop_loss_percent: f64,
    trend: TrendType,
) -> Result<(f64, Vec<Trade>), Box<dyn Error>> {
    // Fetch historical klines from Binance.
    let raw_klines = binance.get_klines(token_symbol, interval, limit).await?;
    if raw_klines.is_empty() {
        return Err("No kline data received".into());
    }
    let candles = parse_candles(raw_klines);
    if candles.is_empty() {
        return Err("No candle data available after parsing".into());
    }

    // Simulate the trade based on the trend type.
    let (final_multiplier, trades) = match trend {
        TrendType::Positive => simulate_trailing_trade(&candles, stop_loss_percent),
        TrendType::Negative => simulate_trailing_trade_negative(&candles, stop_loss_percent),
    };

    Ok((final_multiplier, trades))
}

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// The trading pair (e.g. FARMUSDT)
    token: String,
    /// The kline interval (e.g. 1h, 15m)
    interval: String,
    /// The number of candles to fetch (e.g. 48)
    limit: u16,
    /// Trend type: "positive" or "negative"
    trend: String,
    /// The stop loss percentage to simulate (e.g. 5 for 5%)
    stop_loss: f64,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();
    let binance = Binance::new();
    let trend = TrendType::from_str(&args.trend);

    println!(
        "Running backtest for {} over {} candles with interval {} for {:?} trend and stop loss {}%...",
        args.token, args.limit, args.interval, trend, args.stop_loss
    );

    match backtest_trade(&binance, &args.token, &args.interval, args.limit, args.stop_loss, trend).await {
        Ok((multiplier, trades)) => {
            let total_profit = (multiplier - 1.0) * 100.0;
            println!("Backtest result: Final multiplier = {:.4} (Total Profit: {:+.2}%)", multiplier, total_profit);
            println!("Trade details:");
            for trade in trades {
                match trade.exit_index {
                    Some(_idx) => println!(
                        "  Trade from candle {}: entry at {:.2}, exit at {:.2}, multiplier: {:.4}",
                        trade.entry_index + 1, trade.entry_price, trade.exit_price, trade.multiplier
                    ),
                    None => println!(
                        "  Final trade starting at candle {}: entry at {:.2}, exit at {:.2} (final), multiplier: {:.4}",
                        trade.entry_index + 1, trade.entry_price, trade.exit_price, trade.multiplier
                    ),
                }
            }
        },
        Err(e) => eprintln!("Backtest error: {}", e),
    }

    sleep(Duration::from_secs(1)).await;
    Ok(())
}