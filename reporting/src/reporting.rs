use std::collections::{BTreeMap, HashMap};
use std::fs::{self, File};
use std::io::{BufRead, BufReader, Write};
use chrono::{NaiveDateTime, Datelike};

#[derive(Debug)]
pub struct Trade {
    pub timestamp: NaiveDateTime,
    pub symbol: String,
    pub action: String,
    pub price: f64,
    pub qty: f64,
    pub quote: f64,
    pub stop_loss: f64,
    pub reason: String,
    pub trend: String,
}

#[derive(Debug)]
pub struct CompletedTrade {
    pub symbol: String,
    pub entry_time: NaiveDateTime,
    pub exit_time: NaiveDateTime,
    pub entry_price: f64,
    pub exit_price: f64,
    pub pnl_percent: f64,
    pub trend: String,
    pub reason: String,
}

pub fn parse_log_line(line: &str) -> Option<Trade> {
    let parts: Vec<&str> = line.split(',').collect();
    if parts.len() < 9 || parts[0] == "timestamp" {
        return None;
    }

    let timestamp = NaiveDateTime::parse_from_str(parts[0].trim(), "%Y-%m-%dT%H:%M:%S%.fZ").ok()?;
    let symbol = parts[1].trim().to_string();
    let action = parts[2].trim().to_string();
    let price = parts[3].trim().parse().ok()?;
    let qty = parts[4].trim().parse().ok()?;
    let quote = parts[5].trim().parse().ok()?;
    let stop_loss = parts[6].trim().parse().ok()?;
    let reason = parts[7].trim().to_string();
    let trend = parts[8].trim().to_string();

    Some(Trade {
        timestamp,
        symbol,
        action,
        price,
        qty,
        quote,
        stop_loss,
        reason,
        trend,
    })
}

pub fn load_trades_from_dir(dir_path: &str) -> Vec<Trade> {
    let mut trades = Vec::new();
    if let Ok(entries) = fs::read_dir(dir_path) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().map(|ext| ext == "csv").unwrap_or(false) {
                if let Ok(file) = File::open(&path) {
                    let reader = BufReader::new(file);
                    for line in reader.lines().flatten() {
                        if let Some(trade) = parse_log_line(&line) {
                            trades.push(trade);
                        }
                    }
                }
            }
        }
    }
    trades
}

pub fn match_trades(trades: &[Trade]) -> Vec<CompletedTrade> {
    let mut open_trades: HashMap<String, Trade> = HashMap::new();
    let mut completed = Vec::new();

    for trade in trades.iter().filter(|t| t.action == "BUY" || t.action == "SELL") {
        match trade.action.as_str() {
            "BUY" => {
                open_trades.insert(trade.symbol.clone(), trade.clone());
            }
            "SELL" => {
                if let Some(entry) = open_trades.remove(&trade.symbol) {
                    let pnl_percent = (trade.price - entry.price) / entry.price * 100.0;
                    completed.push(CompletedTrade {
                        symbol: trade.symbol.clone(),
                        entry_time: entry.timestamp,
                        exit_time: trade.timestamp,
                        entry_price: entry.price,
                        exit_price: trade.price,
                        pnl_percent,
                        trend: entry.trend,
                        reason: trade.reason.clone(),
                    });
                }
            }
            _ => {}
        }
    }

    completed
}

pub fn summarize_by_month(completed: &[CompletedTrade]) -> BTreeMap<String, (usize, usize, usize, f64)> {
    let mut summary = BTreeMap::new();

    for trade in completed {
        let month = format!("{}-{:02}", trade.exit_time.year(), trade.exit_time.month());
        let entry = summary.entry(month).or_insert((0, 0, 0, 0.0));
        entry.0 += 1;
        if trade.pnl_percent > 0.0 {
            entry.1 += 1;
        } else {
            entry.2 += 1;
        }
        entry.3 += trade.pnl_percent;
    }

    summary
}

pub fn print_monthly_summary(summary: &BTreeMap<String, (usize, usize, usize, f64)>) {
    println!("{:<8} {:<6} {:<6} {:<6} {:<10}", "Month", "Total", "Wins", "Losses", "AvgPnL(%)");
    println!("{}", "-".repeat(40));

    for (month, (total, wins, losses, sum_pnl)) in summary {
        let avg_pnl = if *total > 0 { sum_pnl / *total as f64 } else { 0.0 };
        println!("{:<8} {:<6} {:<6} {:<6} {:<10.2}", month, total, wins, losses, avg_pnl);
    }
}


fn main() {
    let folder = get_trade_log_folder();
    let trades = load_trades_from_dir(&folder);
    let completed = match_trades(&trades);
    let monthly = summarize_by_month(&completed);
    print_monthly_summary(&monthly);
}