use crate::types::{Signal, TrendDirection};
use crate::config::SHARED_CONFIG;
use chrono::Utc;
use std::time::Duration;
use tokio::time::sleep;
use serde_json::Value;
use log::{info, error};
use crate::api::binance::Binance;
use std::collections::HashSet;

pub async fn discover_signals(
    binance: &Binance,
    assets: &[String],
    transaction_amounts: &[f64],
    trend: TrendDirection,
) -> Vec<Signal> {
    let mut signals = Vec::new();

    let timestamp = Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
    info!("[{}] Starting market scan...", timestamp);
    println!("[{}] Starting market scan...", timestamp);

    let all_tickers = match binance.get_all_ticker_24hr().await {
        Ok(tickers) => tickers,
        Err(e) => {
            error!("Error fetching 24hr tickers: {}", e);
            sleep(Duration::from_secs(1)).await;
            return signals;
        }
    };

     // ✅ Get all open order symbols
     let invested_tokens: HashSet<String> = match binance.get_open_order_symbols().await {
        Ok(symbols) => symbols.into_iter().collect(),
        Err(e) => {
            error!("Failed to fetch open order symbols: {}", e);
            HashSet::new()
        }
    };


    //reduced scope
    // ✅ Keep only symbols with valid priceChangePercent and not already invested
    let tradable_tokens: Vec<(String, f64)> = all_tickers
        .into_iter()
        .filter_map(|ticker| {
            ticker.priceChangePercent.parse::<f64>().ok().and_then(|change| {
                if !invested_tokens.contains(&ticker.symbol) {
                    Some((ticker.symbol, change))
                } else {
                    None
                }
            })
        })
        .collect();

    for (i, asset) in assets.iter().enumerate() {
        let balance = match binance.get_account_balance(asset).await {
            Ok(b) => b,
            Err(e) => {
                error!("Error fetching balance for {}: {}", asset, e);
                continue;
            }
        };

        let amount = transaction_amounts.get(i).copied().unwrap_or_else(|| {
            let config = SHARED_CONFIG.read().unwrap();
            config.transaction_amount
        });

        if balance < amount {
            continue;
        }

        let (lookback, recent) = {
            let config = SHARED_CONFIG.read().unwrap();
            (config.lookback_period, config.last_hours_period)
        };

        let candidates: Vec<String> = tradable_tokens
            .iter()
            .filter(|(symbol, _)| symbol.ends_with(asset))
            .map(|(symbol, _)| symbol.clone())
            .collect();

        for symbol in candidates {
            match binance.get_klines(&symbol, "1h", lookback).await {
                Ok(klines) => {
                    if let Some(signal) = evaluate_klines(
                        &symbol,
                        &klines,
                        lookback as u32,
                        recent as u32,
                        trend,
                    ) {
                        signals.push(signal);
                    }
                }
                Err(e) => {
                    error!("Error fetching klines for {}: {}", symbol, e);
                }
            }
        }
    }

    signals
}

fn evaluate_klines(symbol: &str,klines: &[Vec<Value>],lookback: u32,recent: u32,trend: TrendDirection,) -> Option<Signal> 
{
    if klines.len() < lookback as usize {
        return None;
    }

    let open = parse_f64(&klines[0][1])?;
    let prev_close = parse_f64(&klines[klines.len() - 2][4])?;
    let last_close = parse_f64(&klines[klines.len() - 1][4])?;

    let overall_growth = ((last_close - open) / open) * 100.0;
    let current_trend_up = last_close > prev_close;

    let recent_candles = &klines[klines.len() - recent as usize..];
    let recent_open = parse_f64(&recent_candles[0][1])?;
    let recent_close = parse_f64(&recent_candles.last().unwrap()[4])?;
    let recent_growth = ((recent_close - recent_open) / recent_open) * 100.0;

    let valid = match trend {
        TrendDirection::Positive => overall_growth >= 10.0 && current_trend_up && recent_growth > 0.0,
        TrendDirection::Negative => overall_growth <= -10.0 && !current_trend_up && recent_growth < 0.0,
    };

    if !valid {
        return None;
    }

    let (avg_fluct_raw, avg_fluct_pct) = calculate_fluctuations(klines);

    Some(Signal {
        symbol: symbol.to_string(),
        overall_growth,
        recent_growth,
        avg_fluct_raw,
        avg_fluct_pct,
    })
}

fn parse_f64(value: &Value) -> Option<f64> {
    value.as_str()?.parse::<f64>().ok()
}

fn calculate_fluctuations(klines: &[Vec<Value>]) -> (f64, f64) {
    let mut raw = vec![];
    let mut pct = vec![];

    for candle in klines {
        let high = parse_f64(&candle[2]).unwrap_or(0.0);
        let low = parse_f64(&candle[3]).unwrap_or(0.0);
        if high > 0.0 && low > 0.0 {
            let diff = high - low;
            raw.push(diff);
            pct.push((diff / low) * 100.0);
        }
    }

    let avg_raw = raw.iter().sum::<f64>() / raw.len().max(1) as f64;
    let avg_pct = pct.iter().sum::<f64>() / pct.len().max(1) as f64;

    (avg_raw, avg_pct)
}