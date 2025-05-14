use std::env;
use tracing_subscriber;
use std::fs::{OpenOptions, create_dir_all};
use std::io::Write;
use crate::config::*;
use chrono::{Duration, Utc};
use std::time::UNIX_EPOCH;
use std::fs;


pub async fn log_trade_event(symbol: &str,action: &str,price: f64,qty: f64, quote: f64, stop_loss: f64, reason: &str, trend: &str,) {
    let timestamp = Utc::now().to_rfc3339();
    let date = Utc::now().format("%Y-%m-%d").to_string();

    // Read folder path from env
    let folder = env::var("TRADE_LOG_FOLDER").unwrap_or_else(|_| "logs/trades".to_string());
    let path = format!("{}/{}.csv", folder, date);

    //let mode = get_trading_mode().await;
    let row = format!(
        "{},{},{:.4},{:.4},{:.4},{:.4},{},{},{}\n",
        timestamp, symbol, action, price, qty, quote, stop_loss, reason, trend
    );

    std::thread::spawn(move || {
        if let Err(e) = create_dir_all(&folder) {
            eprintln!("❌ Failed to create log dir: {}", e);
            return;
        }

        let new_file = !std::path::Path::new(&path).exists();

        if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(&path) {
            if new_file {
                let _ = writeln!(file, "timestamp,symbol,action,price,qty,quote,stop_loss,reason,trend");
            }

            if let Err(e) = file.write_all(row.as_bytes()) {
                eprintln!("❌ Failed to write log row: {}", e);
            } else if let Err(e) = file.flush() {
                eprintln!("❌ Failed to flush log file: {}", e);
            }
        } else {
            eprintln!("❌ Could not open log file: {}", path);
        }
    });
}

/// Initialize tracing
pub fn init_tracing( stdout: bool,  filter: tracing::Level,) -> tracing_appender::non_blocking::WorkerGuard {
     // Read log file settings from the environment.
     let log_dir = get_log_folder();
     let log_file = get_log_file();

     // Decide which output should be used
    let (writer, guard) = if stdout {
        let (writer, guard) = tracing_appender::non_blocking(std::io::stdout());
        (writer, guard)
    } else {
        let file_appender = tracing_appender::rolling::daily(&log_dir, &log_file);
        let (writer, guard) = tracing_appender::non_blocking(file_appender);
        (writer, guard)
    };

    // Initialize tracing instance
    tracing_subscriber::fmt()
        .with_writer(writer)
        .with_max_level(filter)
        .with_ansi(stdout)
        .with_target(false)
        .with_file(false)
        .with_thread_ids(true)
        .with_thread_names(true)
        .init();

    guard
}

pub fn cleanup_old_logs(folder: &str) {
    let cutoff = Utc::now() - Duration::days(365);

    if let Ok(entries) = fs::read_dir(folder) {
        for entry in entries.flatten() {
            let path = entry.path();

            // Only consider .log or .txt files
            if path.extension().map(|ext| ext == "log" || ext == "txt").unwrap_or(false) {
                if let Ok(metadata) = fs::metadata(&path) {
                    if let Ok(modified) = metadata.modified() {
                        if let Ok(modified_time) = modified.duration_since(UNIX_EPOCH) {
                            let modified_date = chrono::DateTime::<Utc>::from_timestamp(modified_time.as_secs() as i64, 0)
                            .unwrap()
                            .naive_utc();
                            if modified_date < cutoff.naive_utc() {
                                if let Err(e) = fs::remove_file(&path) {
                                    eprintln!("❌ Failed to delete {}: {}", path.display(), e);
                                } else {
                                    println!("🗑 Deleted old log file: {}", path.display());
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}