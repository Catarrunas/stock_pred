// src/trading/execution.rs

#[derive(Debug)]
pub struct Order {
    pub token: String,
    pub purchase_price: f64,
    pub stop_loss_price: f64,
}

/// Simulate buying a token. In a real system, this would call the Binance API.
/// 
/// # Arguments
/// * `token_symbol` - The trading pair (e.g. "BTCUSDT").
/// * `transaction_amount` - The dollar amount to invest.
/// * `stop_loss_percent` - The stop loss percentage to use.
pub async fn buy_token(token_symbol: &str, transaction_amount: f64, stop_loss_percent: f64) -> Result<Order, &'static str> {
    // Simulate fetching the current market price.
    // Replace with an API call to get the current price.
    let current_price = 100.0;  // for example purposes

    // Calculate stop loss price.
    let stop_loss_price = current_price * (1.0 - stop_loss_percent / 100.0);
    
    println!(
        "Buying {} for ${:.2} at ${:.2} per unit. Initial stop loss set at ${:.2} ({}% below purchase price).",
        token_symbol, transaction_amount, current_price, stop_loss_price, stop_loss_percent
    );

    // Here, you would send a market order to Binance.
    // For now, we simulate a successful order by returning an Order struct.
    Ok(Order {
        token: token_symbol.to_string(),
        purchase_price: current_price,
        stop_loss_price,
    })
}

/// Check the current market price and update the trailing stop loss if the price has increased.
/// In a real implementation, this would cancel the old stop loss order and place a new one via the API.
///
/// # Arguments
/// * `order` - The current open order.
/// * `current_price` - The latest market price fetched from an API.
/// * `stop_loss_percent` - The same percentage used to calculate the trailing stop loss.
pub async fn update_stop_loss(order: &mut Order, current_price: f64, stop_loss_percent: f64) {
    // Only update if the current price is above the purchase price.
    if current_price > order.purchase_price {
        let new_stop_loss = current_price * (1.0 - stop_loss_percent / 100.0);
        if new_stop_loss > order.stop_loss_price {
            println!(
                "Updating stop loss for {}: Old stop loss ${:.2} -> New stop loss ${:.2}",
                order.token, order.stop_loss_price, new_stop_loss
            );
            // In a real implementation, cancel the existing stop loss order and submit a new one.
            order.stop_loss_price = new_stop_loss;
        } else {
            println!("{}: Current price ${:.2} did not move enough to adjust stop loss (current stop loss remains ${:.2}).", order.token, current_price, order.stop_loss_price);
        }
    } else {
        println!("{}: Current price ${:.2} is not above purchase price ${:.2}. No stop loss update.", order.token, current_price, order.purchase_price);
    }
}