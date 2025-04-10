use stock_pred::api::binance::Binance;
use stock_pred::trading::discovery::discover_signals;
use stock_pred::logging::init_tracing;
use tokio::time::{sleep, Duration};
#[allow(unused_imports)]
use tracing::{debug, info, span, Level};
use stock_pred::types::TrendDirection;
use stock_pred::config::SHARED_CONFIG;

   
#[tokio::main]
async fn main() {
    // Initialize logging (this sets up the reloadable layer).
    let _guard = init_tracing(false, Level::INFO);
    let binance = Binance::new();
    //let open_orders: Arc<Mutex<Vec<Order>>> = Arc::new(Mutex::new(Vec::new()));
    //let converted_orders: Vec<Order> = open_orders_guard.iter().cloned().map(Order::from).collect();

    // Parse the list of assets from the environment variable QUOTE_ASSETS and transaction amounts from the config.
    let (assets, transaction_amounts) = {
        let config = SHARED_CONFIG.read().unwrap();
        (config.quote_assets.clone(), config.transaction_amounts.clone())
    };
    println!("Assets to scan: {:?}", assets);
    info!("Assets to scan: {:?}", assets);

    // Spawn the market-check loop.
    let market_check_handle = tokio::spawn(async move {
        loop {
            let signals = discover_signals(
                &binance,
                &assets,
                &transaction_amounts,
                //open_orders_clone,
                TrendDirection::Positive,
            )
            .await;

            for signal in signals {
                println!(
                    "Signal: {:<12} | Growth: {:>5.2}% | Recent: {:>5.2}% | Fluct: {:>5.4} (~{:>4.2}%)",
                    signal.symbol,
                    signal.overall_growth,
                    signal.recent_growth,
                    signal.avg_fluct_raw,
                    signal.avg_fluct_pct,
                );
            // Extract values from the shared config
            // Extract values and drop the guard immediately:
            let loop_time = {
                let current_config = SHARED_CONFIG.read().unwrap();
                current_config.loop_time_seconds
            };
            println!("-------------------------------------------------------------------------");
            println!("Sleeping for {} seconds before the next iteration...", loop_time);
            info!("Sleeping for {} seconds before the next iteration...", loop_time);
            // Now call sleep without holding the lock:
            sleep(Duration::from_secs(loop_time)).await;
        }};    
    });
    // Await both loops indefinitely.
    let _ = tokio::join!(market_check_handle);
}