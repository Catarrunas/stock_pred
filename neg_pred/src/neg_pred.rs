use stock_pred::api::binance::Binance;
use stock_pred::trading::discovery::discover_signals;
use stock_pred::logging::init_tracing;
use tokio::time::{sleep, Duration};
#[allow(unused_imports)]
use tracing::{debug, info, span, Level};
use stock_pred::types::TrendDirection;
use stock_pred::config::SHARED_CONFIG;

   
/* 
// Move update_orders_loop outside of main.
async fn update_orders_loop(open_orders: Arc<Mutex<Vec<Order>>>) {
    loop {
        {
            // Await the lock on the orders.
            let timestamp = Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
            println!("[{}] Running stop loss update loop", timestamp);
            info!("[{}] Running stop loss update loop", timestamp);
            let mut orders = open_orders.lock().await;
            for order in orders.iter_mut() {
                let simulated_current_price = order.purchase_price * 1.05;
                //let stop_loss_percent = get_stop_loss_percent();
                 let stop_loss_percent: f64 = 5.0;
                // Await the async update_stop_loss function.
                execution::update_stop_loss(order, simulated_current_price, stop_loss_percent).await;
                print!("Order {} updated. ", order.token);
                info!("Order {} updated. ", order.token);
            }
        }
        // Read the current order update interval from the shared config.
        let order_update_interval = {
            let config = SHARED_CONFIG.read().unwrap();
            config.order_update_interval
        };
        println!("Sleeping for {} seconds before updating orders...", order_update_interval);
        sleep(Duration::from_secs(order_update_interval)).await;
    }
}*/

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
    //let open_orders_clone = Arc::clone(&open_orders);
    let market_check_handle = tokio::spawn(async move {
        loop {
            let signals = discover_signals(
                &binance,
                &assets,
                &transaction_amounts,
                //open_orders_clone,
                TrendDirection::Negative,
            )
            .await;
            for signal in signals {
                println!(
                    "🔻 Bearish: {:<12} | Drop: {:>6.2}% | Recent: {:>6.2}% | Fluct: {:>7.4} (~{:>5.2}%)",
                    signal.symbol,
                    signal.overall_growth,
                    signal.recent_growth,
                    signal.avg_fluct_raw,
                    signal.avg_fluct_pct,
                );
            }
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
        }});    
    // Await both loops indefinitely.
    let _ = tokio::join!(market_check_handle);
    
}