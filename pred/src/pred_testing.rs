use stock_pred::api::binance::Binance;

#[tokio::main]
async fn main() {
    let binance = Binance::new();

 
    
    binance.manage_stop_loss_limit_loop().await;

    // Place a tradeß
    //let _ = binance.execute_trade_with_trailing_stop("ACTUSDC", 5.0, 5.0, None).await;
    //let _ = binance.place_market_buy_order("ACTUSDC",  35.0 ).await;

    //let _ = binance.calculate_quantity_for_quote("ACTUSDC", 35.0).await;
    
   //let _ = binance.place_trailing_stop_sell_order("ACTUSDC", 34.96675 ,  5.0 , None).await;

    // Check open orders right after

    // 🔍 Call the function
   
  
}
    