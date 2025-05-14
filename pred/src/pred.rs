use stock_pred::api::binance::Binance;
use stock_pred::logging::init_tracing;
use tokio::time::{sleep, Duration};
#[allow(unused_imports)]
use tracing::{debug, info, span, Level};
use stock_pred::config::SHARED_CONFIG;
use stock_pred::trading::discovery::discover_signals;
use stock_pred::types::TrendDirection;
use stock_pred::config::is_trading_day;
use chrono::Datelike;
use stock_pred::config;
use stock_pred::config::watch_config;


   
#[tokio::main]
async fn main() {
    println!("Starting progam");
    info!("Starting progam:");
    let _guard = init_tracing(false, Level::INFO);
    watch_config(SHARED_CONFIG.clone());
    let binance = Binance::new();
   // let mut loss_tracker = GlobalLossTracker::new(); // Initialize the loss tracker
    // Parse the list of assets from the environment variable QUOTE_ASSETS and transaction amounts from the config.
    let assets = config::get_quote_assets();
    let transaction_amounts = config::get_transaction_amounts();
    println!("Assets to scan: {:?}", assets);
    info!("Assets to scan: {:?}", assets);
    

    // Spawn the market-check loop.
    let market_check_handle = tokio::spawn(async move {
        loop {
            // 🛑 Check if trading is allowed today
            if !is_trading_day() {
                println!("⛔ Skipping trading — {} is excluded", chrono::Local::now().weekday());
                tokio::time::sleep(Duration::from_secs(60 * 60 * 4)).await;
                continue;
            }
            /*
            if loss_tracker.is_on_cooldown() {
                println!("Bot is in global cooldown. Skipping this cycle.");
                info!("Bot is in global cooldown. Skipping this cycle.");
                let remaining = loss_tracker
                    .cooldown_until
                    .unwrap()
                    .saturating_duration_since(Instant::now());
                sleep(remaining).await;
                continue;
            }
        
            if let Ok(true) = binance.should_pause_for_losses().await {
                println!("⛔ Daily loss threshold reached. Entering cooldown for 24h.");
                info!("⛔ Daily loss threshold reached. Entering cooldown for 24h.");
                loss_tracker.cooldown_until = Some(Instant::now() + Duration::from_secs(60 * 60 * 24));
                sleep(Duration::from_secs(120)).await;
                continue;
            }
         */
            let signals = discover_signals(&binance,&assets, &transaction_amounts,
                //open_orders_clone,
                TrendDirection::Positive,
            ).await;

            for signal in signals {
                println!(
                    "Signal: {:<12} | Growth: {:>5.2}% | Recent: {:>5.2}% | Fluct: {:>5.4} (~{:>4.2}%)",
                    signal.symbol,
                    signal.overall_growth,
                    signal.recent_growth,
                    signal.avg_fluct_raw,
                    signal.avg_fluct_pct,
                );
                

                //Execute trade and trailing stop logic
                if let Err(e) = binance
                .execute_trade_with_fallback_stop(
                    &signal.symbol,
                    None,    // no activation price, trail immediately
                )
                .await{
                    eprintln!("❌ Failed to execute for token {} : {}", signal.symbol, e);
                    info!("❌ Failed to execute trade for token {} : {}", signal.symbol, e);
                }
            } 
            
            // Extract values from the shared config
            // Extract values and drop the guard immediately:
            let loop_time = config::get_loop_time_seconds();
            println!("-------------------------------------------------------------------------");
            println!("Sleeping for {} seconds before the next iteration...", loop_time);
            info!("Sleeping for {} seconds before the next iteration...", loop_time);
            // Now call sleep without holding the lock:
            sleep(Duration::from_secs(loop_time)).await;
        }});    
    // 🛡️ Stop-loss check loop
    let stop_loss_loop = {
        let binance2 = Binance::new();
        tokio::spawn(async move {
            binance2.manage_stop_loss_limit_loop().await;
        })
    };

    // ⏱ Run both loops
    let result = tokio::join!(market_check_handle, stop_loss_loop);
    if let Err(e) = result.1 {
        eprintln!("❌ stop_loss_loop panicked: {:?}", e);
    }
    if let Err(e) = result.0 {
        eprintln!("❌ market_check_handle panicked: {:?}", e);
    }
}