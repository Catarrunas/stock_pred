use dotenv::from_filename;
use once_cell::sync::Lazy;
use std::env;
use std::sync::{Arc, RwLock};
use notify::{Watcher};


#[derive(Debug, Clone)]
pub struct Config {
    pub transaction_amount: f64,
    pub stop_loss_percent: f64,
    pub max_open_trades: usize,
    pub lookback_period: u16,
    pub last_hours_period: u16,
    pub loop_time_seconds: u64,
    pub bt_lookback_options: Vec<u16>,
    pub bt_recent_options: Vec<u16>,
    pub bt_stop_loss_options: Vec<u16>,
    pub order_update_interval: u64,
    pub quote_assets: Vec<String>,
    pub transaction_amounts: Vec<f64>
}

impl Config {
    /// Loads configuration from the "vars.env" file.
    pub fn load() -> Self {
        // Load the environment variables from vars.env.
        let _ = from_filename("vars.env");

        let transaction_amount = env::var("TRANSACTION_AMOUNT")
            .unwrap_or_else(|_| "100".to_string())
            .parse::<f64>()
            .unwrap_or(100.0);
        let stop_loss_percent = env::var("STOP_LOSS_PERCENT")
            .unwrap_or_else(|_| "5".to_string())
            .parse::<f64>()
            .unwrap_or(5.0);
        let max_open_trades = env::var("MAX_OPEN_TRADES")
            .unwrap_or_else(|_| "5".to_string())
            .parse::<usize>()
            .unwrap_or(5);
        let lookback_period = env::var("LOOKBACK_PERIOD")
            .unwrap_or_else(|_| "48".to_string())
            .parse::<u16>()
            .unwrap_or(48);
        let last_hours_period = env::var("LAST_HOURS_PERIOD")
            .unwrap_or_else(|_| "4".to_string())
            .parse::<u16>()
            .unwrap_or(4);
        let loop_time_seconds = env::var("LOOP_TIME_SECONDS")
            .unwrap_or_else(|_| "3600".to_string())
            .parse::<u64>()
            .unwrap_or(3600);
        let order_update_interval = env::var("ORDER_UPDATE_INTERVAL")
            .unwrap_or_else(|_| "900".to_string())  // default 900 seconds (15 minutes)
            .parse::<u64>()
            .unwrap_or(900);
        // Parse backtesting options from environment variables.
        let bt_lookback_options = env::var("BT_LOOKBACK_OPTIONS")
            .unwrap_or_else(|_| "6,8,12".to_string())
            .split(',')
            .filter_map(|s| s.trim().parse::<u16>().ok())
            .collect::<Vec<u16>>();
        let bt_recent_options = env::var("BT_RECENT_OPTIONS")
            .unwrap_or_else(|_| "2,4,6".to_string())
            .split(',')
            .filter_map(|s| s.trim().parse::<u16>().ok())
            .collect::<Vec<u16>>();
        let bt_stop_loss_options = env::var("BT_STOP_LOSS_OPTIONS")
            .unwrap_or_else(|_| "2,4,6".to_string())
            .split(',')
            .filter_map(|s| s.trim().parse::<u16>().ok())
            .collect::<Vec<u16>>();
        let quote_assets = env::var("QUOTE_ASSETS")
            .unwrap_or_else(|_| "USDC".to_string())
            .split(',')
            .map(|s| s.trim().to_string())
            .collect::<Vec<String>>();
        let transaction_amounts = env::var("TRANSACTION_AMOUNTS")
            .unwrap_or_else(|_| "100".to_string())
            .split(',')
            .filter_map(|s| s.trim().parse::<f64>().ok())
            .collect::<Vec<f64>>();

        Config {
            transaction_amount,
            stop_loss_percent,
            max_open_trades,
            lookback_period,
            last_hours_period,
            loop_time_seconds,
            bt_lookback_options,
            bt_recent_options,
            bt_stop_loss_options,
            order_update_interval,
            quote_assets,
            transaction_amounts,
        }
    }
}

/// Returns the current transaction amount.
pub fn get_transaction_amount() -> f64 {
    SHARED_CONFIG.read().unwrap().transaction_amount
}

/// Returns the current stop loss percent.
pub fn get_stop_loss_percent() -> f64 {
    SHARED_CONFIG.read().unwrap().stop_loss_percent
}

/// Returns the current maximum number of open trades.
pub fn get_max_open_trades() -> usize {
    SHARED_CONFIG.read().unwrap().max_open_trades
}

/// Returns the current lookback period (number of candles).
pub fn get_lookback_period() -> u16 {
    SHARED_CONFIG.read().unwrap().lookback_period
}

/// Returns the current last hours period (number of candles for recent trend).
pub fn get_last_hours_period() -> u16 {
    SHARED_CONFIG.read().unwrap().last_hours_period
}

/// Returns the current loop time (in seconds) for market-check iterations.
pub fn get_loop_time_seconds() -> u64 {
    SHARED_CONFIG.read().unwrap().loop_time_seconds
}

/// Returns backtesting lookback options from the BT_LOOKBACK_OPTIONS env variable.
pub fn get_lookback_options() -> Vec<u16> {
    // Ensure vars.env is loaded
    let _ = from_filename("vars.env");
    let opts = env::var("BT_LOOKBACK_OPTIONS").unwrap_or_else(|_| "6,8,12".to_string());
    opts.split(',')
        .filter_map(|s| s.trim().parse::<u16>().ok())
        .collect()
}

/// Returns backtesting recent options from the BT_RECENT_OPTIONS env variable.
pub fn get_recent_options() -> Vec<u16> {
    let _ = from_filename("vars.env");
    let opts = env::var("BT_RECENT_OPTIONS").unwrap_or_else(|_| "2,4,6".to_string());
    opts.split(',')
        .filter_map(|s| s.trim().parse::<u16>().ok())
        .collect()
}

pub fn get_bt_stop_loss_options() -> Vec<f64> {
    // Load from vars.env if not already loaded.
    let _ = from_filename("vars.env");
    let opts = env::var("BT_STOP_LOSS_PERCENT").unwrap_or_else(|_| "3,5,10".to_string());
    opts.split(',')
        .filter_map(|s| s.trim().parse::<f64>().ok())
        .collect()
}

pub type SharedConfig = Arc<RwLock<Config>>;

pub static SHARED_CONFIG: Lazy<SharedConfig> = Lazy::new(|| Arc::new(RwLock::new(Config::load())));

/// Spawns a file watcher that monitors "vars.env" for changes and reloads the configuration.
pub fn watch_config(shared_config: SharedConfig) {
    let config_file = "vars.env";
    let (tx, rx) = std::sync::mpsc::channel();

    let mut watcher: notify::RecommendedWatcher =
        notify::RecommendedWatcher::new(tx, notify::Config::default())
            .expect("Failed to create watcher");

    watcher
        .watch(std::path::Path::new(config_file), notify::RecursiveMode::NonRecursive)
        .expect("Failed to watch config file");

    // Spawn a thread to listen for file changes.
    std::thread::spawn(move || {
        // Bind the watcher to a variable so it remains in scope.
        let _watcher = watcher;
        loop {
            match rx.recv() {
                Ok(event) => {
                    println!("Configuration file changed. Reloading... Event: {:?}", event);
                    // Remove the environment variable so dotenv can load a new value.
                    std::env::remove_var("LOOP_TIME_SECONDS");
                    // Reload the configuration.
                    let new_config = Config::load();
                    if let Ok(mut config) = shared_config.write() {
                        *config = new_config;
                        println!("New configuration: {:?}", *config);
                    }
                    // Throttle rapid events.
                    std::thread::sleep(std::time::Duration::from_millis(200));
                }
                Err(e) => {
                    println!("Config watch error: {:?}", e);
                    // Optionally, you can break the loop or continue.
                }
            }
        }
    });
}