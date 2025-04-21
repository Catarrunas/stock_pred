use stock_pred::api::binance::Binance;
use std::time::Duration;
use std::time::Instant;
use tokio::time::sleep;
use stock_pred::types::GlobalLossTracker;

#[tokio::main]
async fn main() {
    let binance = Binance::new();
    let mut loss_tracker = GlobalLossTracker::new(); // Initialize the loss tracker

    if loss_tracker.is_on_cooldown() {
        println!("Bot is in global cooldown. Skipping this cycle.");
        let remaining = loss_tracker
            .cooldown_until
            .unwrap()
            .saturating_duration_since(Instant::now());
        sleep(remaining).await; 
    }

    if let Ok(true) = binance.should_pause_for_losses().await {
        println!("⛔ Daily loss threshold reached. Entering cooldown for 24h.");
        loss_tracker.cooldown_until = Some(Instant::now() + Duration::from_secs(60));
        sleep(Duration::from_secs(120)).await;
    }
    /*

    let balances = match binance.get_spot_balances().await {
        Ok(balances) => balances,
        Err(e) => {
            eprintln!("❌ Failed to fetch spot balances: {}", e);
            return;
        }
    };
    

    for (asset, amount) in balances {
        println!("✅ You hold {:.4} {}", amount, asset);
    }

    let open_orders = match binance.get_open_order_symbols().await {
        Ok(symbols) => symbols,
        Err(e) => {
            eprintln!("❌ Failed to fetch open orders: {}", e);
            vec![]
        }
    };
      binance.manage_stop_loss_limit_loop().await;
    let open_orders = match binance.get_open_order_symbols().await {
        Ok(symbols) => symbols,
        Err(e) => {
            eprintln!("❌ Failed to fetch open orders: {}", e);
            vec![]
        }
    };
*/
    // Place a tradeß
    //let _ = binance.execute_trade_with_trailing_stop("ACTUSDC", 5.0, 5.0, None).await;
    //let _ = binance.place_market_buy_order("ACTUSDC",  35.0 ).await;

    //let _ = binance.calculate_quantity_for_quote("ACTUSDC", 35.0).await;
    
   //let _ = binance.place_trailing_stop_sell_order("ACTUSDC", 34.96675 ,  5.0 , None).await;

    // Check open orders right after

    // 🔍 Call the function
   
  
}
    