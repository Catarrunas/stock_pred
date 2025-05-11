use chrono::{DateTime, Datelike, NaiveDate, Utc};
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use csv::Reader;
use stock_pred::config::get_trade_log_folder;

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

fn main() {
    let folder = get_trade_log_folder();
    let trades = load_trades_from_dir(Path::new(&folder));
    let realized = generate_realized_report(&trades);

    println!("📊 === Realized Profit Summary ===");
    println!("Total Realized Trades: {}", realized.len());

    println!("📆 Daily Summary:");
    for (date, (profit, _)) in summarize_by_day(&realized) {
        println!("{} → Profit: {:.2} USDC", date, profit);
    }

    println!("📅 Weekly Summary:");
    for ((year, week), (profit, _)) in summarize_by_week(&realized) {
        println!("Week {}-W{:02} → Profit: {:.2} USDC", year, week, profit);
    }

    println!("🗓 Monthly Summary:");
    for ((year, month), (profit, _)) in summarize_by_month(&realized) {
        println!("{}-{:02} → Profit: {:.2} USDC", year, month, profit);
    }

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

    let mut token_profits: HashMap<String, f64> = HashMap::new();

    for trade in &realized {
        token_profits
            .entry(trade.symbol.clone())
            .and_modify(|p| *p += trade.profit)
            .or_insert(trade.profit);
    }

    let mut total_tokens = 0;
    let mut winning_tokens = 0;
    let mut losing_tokens = 0;

    for (_token, profit) in &token_profits {
        total_tokens += 1;
        if *profit >= 0.0 {
            winning_tokens += 1;
        } else {
            losing_tokens += 1;
        }
    }

    if let Some((worst_token, worst_profit)) =
        token_profits.iter().min_by(|a, b| a.1.partial_cmp(b.1).unwrap())
    {
        println!("\n🔻 Token with most total loss: {} → {:.2} USDC", worst_token, worst_profit);
    }

    let win_ratio = (winning_tokens as f64 / total_tokens as f64) * 100.0;
    let loss_ratio = (losing_tokens as f64 / total_tokens as f64) * 100.0;

    println!(
        "\n📈 Win/Loss Ratio: {:.1}% win vs {:.1}% loss ({} tokens total)",
        win_ratio, loss_ratio, total_tokens
    );

    print_trades_for_symbol("KAITOUSDC", &trades);
}
