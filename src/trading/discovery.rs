use crate::types::{Signal, TrendDirection};
use chrono::Utc;
use std::time::Duration;
use tokio::time::sleep;
use serde_json::Value;
use log::{info, error};
use crate::api::binance::Binance;
use std::collections::HashSet;
use crate::types::MARKET_TREND;
use crate::config;

pub async fn discover_signals(binance: &Binance, assets: &[String], transaction_amounts: &[f64], trend: TrendDirection,) -> Vec<Signal> {
    let mut signals = Vec::new();

    let timestamp = Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
    info!("[{}] Starting market scan...", timestamp);

    // ✅ Get open order symbols
    let mut invested_tokens: HashSet<String> = match binance.get_open_order_symbols().await {
        Ok(symbols) => symbols.into_iter().collect(),
        Err(e) => {
            error!("Failed to fetch open order symbols: {}", e);
            HashSet::new()
        }
    };

    // ✅ Add currently held tokens (wallet spot balances turned into pairs)
    match binance.get_spot_balances().await {
        Ok(holdings) => {
            let held_pairs = expand_holdings_to_pairs(&holdings, assets);
            for pair in held_pairs {
                invested_tokens.insert(pair);
            }
        }
        Err(e) => {
            error!("Failed to fetch spot balances: {}", e);
        }
    }

    let all_tickers = match binance.get_all_ticker_24hr().await {
        Ok(tickers) => tickers,
        Err(e) => {
            error!("Error fetching 24hr tickers: {}", e);
            sleep(Duration::from_secs(1)).await;
            return signals;
        }
    };
    let positive_count = all_tickers.iter()
    .filter(|t| t.priceChangePercent.parse::<f64>().unwrap_or(0.0) > 0.0)
    .count();
    let total = all_tickers.len();
    let ratio = positive_count as f64 / total as f64;

    let trend_str = if ratio >= 0.5 {
        "Positive"
    } else {
        "Negative"
    };

    let mut mt = MARKET_TREND.write().await;
    *mt = trend_str.to_string();

    /* match trend {
        TrendDirection::Positive => {
            if ratio < 0.5 {
                println!("📉 Market is negative ({:.1}% green). Skipping Positive trend trades.", ratio * 100.0);
                return signals;
            }
        }
        TrendDirection::Negative => {
            if ratio > 0.5 {
                println!("📈 Market is positive ({:.1}% green). Skipping Negative trend trades.", ratio * 100.0);
                return signals;
            }
        }
    }
    */

    let min_volume = config::get_min_volume() as f64;
    let excluded_tokens = config::get_excluded_tokens();

    let tradable_tokens: Vec<(String, f64)> = all_tickers
        .into_iter()
        .filter_map(|ticker| {
            let volume = ticker.quote_volume.parse::<f64>().unwrap_or(0.0);
            let symbol = ticker.symbol.to_uppercase();

            ticker.priceChangePercent.parse::<f64>().ok().and_then(|change| {
                if volume >= min_volume
                    && !invested_tokens.contains(&ticker.symbol)
                    && !excluded_tokens.contains(&ticker.symbol)
                {
                    Some((symbol, change))
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

        if balance < transaction_amounts.get(i).copied().unwrap_or(10.0) {
            continue;
        }   

        let lookback = config::get_lookback_period();
        let recent = config::get_last_hours_period();

        let candidates: Vec<String> = tradable_tokens
            .iter()
            .filter(|(symbol, _)| symbol.ends_with(asset))
            .map(|(symbol, _)| symbol.clone())
            .collect();

        for symbol in candidates {
           /*  
           let supported = match binance.symbol_supports_order_type(&symbol, "TRAILING_STOP_MARKET").await {
                Ok(v) => v,
                Err(e) => {
                    error!("Could not verify order support for {}: {}", symbol, e);
                    false
                }
            };

            if !supported {
                continue;
            }

            */
            


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

fn evaluate_klines(symbol: &str,klines: &[Vec<Value>],lookback: u32,recent: u32,trend: TrendDirection,) -> Option<Signal> {
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

    // 2 strong green candles check
    let last2_open = parse_f64(&klines[klines.len() - 2][1])?;
    let last2_close = parse_f64(&klines[klines.len() - 2][4])?;
    let last1_open = parse_f64(&klines[klines.len() - 1][1])?;
    let last1_close = last_close;
    let last2_pct = ((last2_close - last2_open) / last2_open) * 100.0;
    let last1_pct = ((last1_close - last1_open) / last1_open) * 100.0;

    let two_strong_green =
        last2_close > last2_open &&
        last1_close > last1_open &&
        last2_pct >= 0.5 &&
        last1_pct >= 0.5;

     // Final validation
     let valid = match trend {
        TrendDirection::Positive => {
            overall_growth >= 10.0 &&
            current_trend_up &&
            recent_growth > 0.0 &&
            two_strong_green
        },
        TrendDirection::Negative => {
            overall_growth <= -10.0 &&
            !current_trend_up &&
            recent_growth < 0.0
        },
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

pub fn expand_holdings_to_pairs( holdings: &[(String, f64)], quote_assets: &[String],) -> Vec<String> {
    let mut pairs = Vec::new();

    for (base, amount) in holdings {
        if *amount > 0.0 {
            for quote in quote_assets {
                if base != quote {
                    pairs.push(format!("{}{}", base, quote));
                }
            }
        }
    }

    pairs
}