use dotenv::from_filename;
use std::env;
use tracing_subscriber;
use std::fs::{OpenOptions, create_dir_all};
use std::io::Write;
use chrono::Utc;

pub fn log_trade_event(
    symbol: &str,
    action: &str,
    price: f64,
    qty: f64,
    quote: f64,
    stop_loss: f64,
    reason: &str,
) {
    let timestamp = Utc::now().to_rfc3339();
    let date = Utc::now().format("%Y-%m-%d").to_string();
     // Load environment variables from vars.env.
    let _ = from_filename("vars.env");
     // Read log file settings from the environment.
    let folder = env::var("TRADE_LOG_FOLDER").unwrap_or_else(|_| "trade_log".to_string());
    let path = format!("{}/{}.csv", folder, date);

    let row = format!(
        "{},{},{:.4},{:.4},{:.4},{:.4},{},{}\n",
        timestamp, symbol, action, price, qty, quote, stop_loss, reason
    );

    std::thread::spawn(move || {
        if let Err(e) = create_dir_all(&folder) {
            eprintln!("❌ Failed to create log dir: {}", e);
            return;
        }

        if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(&path) {
            if let Err(e) = file.write_all(row.as_bytes()) {
                eprintln!("❌ Failed to write log row: {}", e);
            }
        } else {
            eprintln!("❌ Could not open log file: {}", path);
        }
    });
}

/// Initialize tracing
pub fn init_tracing(
    stdout: bool,
    filter: tracing::Level,
) -> tracing_appender::non_blocking::WorkerGuard {
     // Load environment variables from vars.env.
     let _ = from_filename("vars.env");
     // Read log file settings from the environment.
     let log_dir = env::var("LOG_DIR").unwrap_or_else(|_| "log".to_string());
     let log_file = env::var("LOG_FILE").unwrap_or_else(|_| "app.log".to_string());

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
