use chrono::{DateTime, Datelike, NaiveDate, Utc};
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use csv::Reader;
use stock_pred::config::get_trade_log_folder;
use itertools::Itertools;

#[derive(Debug, Deserialize, Clone)]
pub struct TradeLogEntry {
    pub timestamp: DateTime<Utc>,
    pub symbol: String,
    pub action: String,
    pub price: f64,
    pub qty: f64,
    pub quote: f64,
    pub stop_loss: f64,
}

#[derive(Debug, Default, Clone)]
pub struct RealizedTrade {
    pub symbol: String,
    pub buy_price: f64,
    pub sell_price: f64,
    pub qty: f64,
    pub profit: f64,
    pub profit_pct: f64,
    pub timestamp: DateTime<Utc>,
    pub trend: String,
}

pub fn load_trades_from_dir(folder: &Path) -> Vec<TradeLogEntry> {
    let mut trades: Vec<TradeLogEntry> = vec![];
    println!("📁 Scanning {:?}", folder);
    if let Ok(entries) = fs::read_dir(folder) {
        for entry in entries.flatten() {
            if entry.path().extension().map_or(false, |ext| ext == "csv") {
                if let Ok(file) = fs::File::open(entry.path()) {
                    let mut rdr = Reader::from_reader(file);
                    for result in rdr.deserialize::<TradeLogEntry>() {
                        if let Ok(entry) = result {
                            trades.push(entry);
                        }
                    }
                }
            }
        }
    }
    trades.sort_by_key(|e| e.timestamp);
    trades
}

pub fn generate_realized_report(trades: &[TradeLogEntry]) -> Vec<RealizedTrade> {
    let mut result = vec![];
    let mut state: HashMap<String, (Option<TradeLogEntry>, Option<TradeLogEntry>)> = HashMap::new();

    for entry in trades {
        match entry.action.as_str() {
            "BUY" => {
                state.insert(entry.symbol.clone(), (Some(entry.clone()), None));
            }
            "SET_" => {
                if let Some((Some(buy), _)) = state.get(&entry.symbol) {
                    if entry.timestamp > buy.timestamp {
                        state.insert(entry.symbol.clone(), (Some(buy.clone()), Some(entry.clone())));
                    }
                }
            }
            "SELL" => {
                if let Some((Some(buy), Some(set))) = state.get(&entry.symbol) {
                    let sell_price = set.stop_loss;
                    let qty = buy.qty;
                    let profit = (sell_price - buy.price) * qty;
                    let profit_pct = ((sell_price / buy.price) - 1.0) * 100.0;

                    result.push(RealizedTrade {
                        symbol: entry.symbol.clone(),
                        buy_price: buy.price,
                        sell_price,
                        qty,
                        profit,
                        profit_pct,
                        timestamp: entry.timestamp,
                        trend: entry.action.clone(),
                    });
                }
                state.remove(&entry.symbol);
            }
            _ => {}
        }
    }

    result
}

pub fn summarize_by_day(trades: &[RealizedTrade]) -> HashMap<NaiveDate, (f64, f64)> {
    let mut map = HashMap::new();
    for trade in trades {
        let date = trade.timestamp.date_naive();
        let entry = map.entry(date).or_insert((0.0, 0.0));
        entry.0 += trade.profit;
        entry.1 += trade.qty;
    }
    map
}

pub fn summarize_by_week(trades: &[RealizedTrade]) -> HashMap<(i32, u32), (f64, f64)> {
    let mut map = HashMap::new();
    for trade in trades {
        let week = trade.timestamp.iso_week();
        let entry = map.entry((week.year(), week.week())).or_insert((0.0, 0.0));
        entry.0 += trade.profit;
        entry.1 += trade.qty;
    }
    map
}

pub fn summarize_by_month(trades: &[RealizedTrade]) -> HashMap<(i32, u32), (f64, f64)> {
    let mut map = HashMap::new();
    for trade in trades {
        let key = (trade.timestamp.year(), trade.timestamp.month());
        let entry = map.entry(key).or_insert((0.0, 0.0));
        entry.0 += trade.profit;
        entry.1 += trade.qty;
    }
    map
}

pub fn print_trades_for_symbol(symbol: &str, trades: &[TradeLogEntry]) {
    println!("\n🔍 Realized trades for token: {}\n", symbol);

    let mut buy: Option<&TradeLogEntry> = None;
    let mut set: Option<&TradeLogEntry> = None;
    let mut total_profit = 0.0;

    for trade in trades.iter().filter(|t| t.symbol == symbol) {
        match trade.action.as_str() {
            "BUY" => {
                buy = Some(trade);
                set = None;
            }
            "SET_" => {
                if let Some(b) = buy {
                    if trade.timestamp > b.timestamp {
                        set = Some(trade);
                    }
                }
            }
            "SELL" => {
                if let (Some(b), Some(s)) = (buy, set) {
                    let sell_price = s.stop_loss;
                    let qty = b.qty;
                    let profit = (sell_price - b.price) * qty;
                    let profit_pct = ((sell_price / b.price) - 1.0) * 100.0;
                    total_profit += profit;

                    println!(
                        "📅 {} → {} | 🟢 Buy @ {:.5} → Sell @ {:.5} | Qty: {:<7.4} | Profit: {:>6.2} USDC ({:+.2}%)",
                        b.timestamp.format("%Y-%m-%d %H:%M"),
                        trade.timestamp.format("%Y-%m-%d %H:%M"),
                        b.price,
                        sell_price,
                        qty,
                        profit,
                        profit_pct
                    );
                }
                buy = None;
                set = None;
            }
            _ => {}
        }
    }

    println!("\n💰 Total profit on {}: {:.2} USDC", symbol, total_profit);
}

fn print_grouped_summary<F, K>(trades: &[RealizedTrade], key_fn: F)   where F: Fn(&RealizedTrade) -> K,  K: std::cmp::Ord + std::hash::Hash + std::fmt::Display,{
        let mut grouped: HashMap<K, Vec<&RealizedTrade>> = HashMap::new();
        for trade in trades {
            grouped.entry(key_fn(trade)).or_default().push(trade);
        }

        let mut sorted: Vec<_> = grouped.into_iter().collect();
        sorted.sort_by(|a, b| a.0.cmp(&b.0));

        for (key, group) in sorted {
            let profit: f64 = group.iter().map(|t| t.profit).sum();
            let avg_pct: f64 = group.iter().map(|t| t.profit_pct).sum::<f64>() / group.len() as f64;
            let color = if avg_pct >= 0.0 { "\x1b[32m" } else { "\x1b[31m" };
            let reset = "\x1b[0m";
            println!("{} → Profit: {:.2} USDC → W/L: {}{:+.1}%{}",key, profit, color, avg_pct, reset);
        }
}

fn main() {
    let folder = get_trade_log_folder();
    let trades = load_trades_from_dir(Path::new(&folder));
    let realized = generate_realized_report(&trades);
    let args: Vec<String> = std::env::args().collect();
    let symbol_filter = args.get(1).map(|s| s.to_uppercase());

    let args: Vec<String> = std::env::args().collect();

    if args.len() == 2 && (args[1] == "help" || args[1] == "-h") {
        println!(
            "📘 Reporting CLI Usage:\n\n  \
            reporting                  → Full report (daily/weekly/monthly + summaries)\n  \
            reporting SYMBOL           → Show detailed trades for a specific token (e.g. APEUSDC)\n  \
            reporting day YYYY-MM-DD   → Show closed trades for a specific day\n  \
            reporting help | h      → Show this help message"
        );
        return;
    }

    if args.get(1).map(String::as_str) == Some("day") {
        if let Some(date_str) = args.get(2) {
            if let Ok(date) = chrono::NaiveDate::parse_from_str(date_str, "%Y-%m-%d") {
                let folder = get_trade_log_folder();
                let trades = load_trades_from_dir(Path::new(&folder));
                let realized = generate_realized_report(&trades);
                let day_trades: Vec<_> = realized
                    .into_iter()
                    .filter(|t| t.timestamp.date_naive() == date)
                    .collect();

                if day_trades.is_empty() {
                    println!("No trades closed on {}", date);
                    std::process::exit(0);
                }

                println!("\n📆 Closed trades on {}:", date);
                let mut profit_by_token: HashMap<String, f64> = HashMap::new();
                for trade in &day_trades {
                    profit_by_token
                        .entry(trade.symbol.clone())
                        .and_modify(|p| *p += trade.profit)
                        .or_insert(trade.profit);
                }

                for (symbol, profit) in profit_by_token.iter().sorted_by_key(|(s, _)| *s) {
                    let color = if *profit >= 0.0 { "\x1b[32m" } else { "\x1b[31m" };
                    let reset = "\x1b[0m";
                    println!("{} → Profit: {}{:.2} USDC{}",symbol, color, profit, reset);
                }
                let total_profit: f64 = day_trades.iter().map(|t| t.profit).sum();
                let wins = day_trades.iter().filter(|t| t.profit >= 0.0).count();
                let total = day_trades.len();
                let win_pct = (wins as f64 / total as f64) * 100.0;

                println!("\n💰 Total profit on {}: {:.2} USDC",date, total_profit);
                println!("📊 Win/Loss ratio: {} wins / {} total → {:.1}% win rate",wins, total, win_pct);

                std::process::exit(0);
            } else {
                println!("❌ Invalid date format. Use YYYY-MM-DD.");
                std::process::exit(1);
            }
        }
    }

    if let Some(symbol) = symbol_filter {
        print_trades_for_symbol(&symbol, &trades);
        return;
    }

    println!("📊 === Realized Profit Summary ===");
    println!("Total Realized Trades: {}", realized.len());

    println!("📆 Daily Summary:");
    print_grouped_summary(&realized, |t| t.timestamp.date_naive());
    

    println!("📅 Weekly Summary:");
    print_grouped_summary(&realized, |t| {
        let w = t.timestamp.iso_week();
        format!("Week {}-W{:02}", w.year(), w.week())
    });

    println!("🗓 Monthly Summary:");
    print_grouped_summary(&realized, |t| {
        format!("{}-{:02}", t.timestamp.year(), t.timestamp.month())
    });

    // Token-level profit summary
    let mut profit_by_token = std::collections::HashMap::new();
    for trade in &realized {
        profit_by_token
            .entry(trade.symbol.clone())
            .and_modify(|p| *p += trade.profit)
            .or_insert(trade.profit);
    }

    if let Some((best_token, best_profit)) = profit_by_token.iter().max_by(|a, b| a.1.partial_cmp(b.1).unwrap()) {
        println!("\n🚀 Most profitable token: {} → {:.2} USDC", best_token, best_profit);
    }

    if let Some((worst_token, worst_profit)) = profit_by_token.iter().min_by(|a, b| a.1.partial_cmp(b.1).unwrap()) {
        println!("❌ Least profitable token: {} → {:.2} USDC", worst_token, worst_profit);
    }

    let total_tokens = profit_by_token.len();
    let winning = profit_by_token.iter().filter(|(_, p)| **p >= 0.0).count();
    let win_ratio = (winning as f64 / total_tokens as f64) * 100.0;
    let loss_ratio = 100.0 - win_ratio;

    println!("\n📈 Token win/loss ratio: {:.1}% win vs {:.1}% loss ({} tokens)", win_ratio, loss_ratio, total_tokens);

    let mut stats = std::collections::HashMap::new();
    for trade in &realized {
        let trend = if trade.trend.eq_ignore_ascii_case("positive") {"Positive"} else {"Negative"};
        let entry = stats.entry(trend).or_insert((0, 0)); // (wins, total)
        if trade.profit >= 0.0 {
            entry.0 += 1; // win
        }
        entry.1 += 1; // total
    }

    for (trend, (wins, total)) in stats {
        let win_pct = (wins as f64 / total as f64) * 100.0;
        println!("📈 Trend: {} → {}/{} wins ({:.1}%)", trend, wins, total, win_pct);
    }
}
