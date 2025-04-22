use reqwest::Client;
use serde::Deserialize;
use tokio_tungstenite::connect_async;
use tokio_stream::StreamExt;
use serde_json::Value;
use url::Url;
use std::env;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;
use chrono::Utc;
use hmac::{Hmac, Mac};
use sha2::Sha256;
type HmacSha256 = Hmac<Sha256>;
use std::collections::HashSet;
use hex::encode as hex_encode;
use dotenv::from_filename;
use tracing::{info,error};
use crate::types::OpenOrder;
use crate::types::Order;
use reqwest::Error;
use crate::config::SHARED_CONFIG;
use std::collections::HashMap;
use tokio::time::Duration;
use tokio::time::sleep;
use reqwest::Error as ReqwestError;         
use std::error::Error as StdError;      

#[derive(Debug, Clone, Default)]
pub struct SymbolFilters {
    pub tick_size: f64,
    pub step_size: f64,
    pub min_qty: f64,
    pub min_price: f64,
    pub min_notional: f64,
}

#[derive(Debug, Deserialize)]
pub struct ExchangeInfo {
    pub symbols: Vec<SymbolInfo>,
}

#[derive(Debug, Deserialize)]
pub struct SymbolInfo {
    pub symbol: String,
    #[serde(rename = "status")]
    pub status: String,
    #[serde(rename = "baseAsset")]
    #[allow(dead_code)]
    pub base_asset: String,
    #[serde(rename = "quoteAsset")]
    pub quote_asset: String,
    #[serde(rename = "orderTypes")]
    pub order_types: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct Balance {
    pub asset: String,
    pub free: String,
    pub locked: String,
}

#[derive(Debug, Deserialize)]
pub struct AccountInfo {
    #[serde(default)]
    pub maker_commission: i64,
    #[serde(default)]
    pub taker_commission: i64,
    #[serde(default)]
    pub buyer_commission: i64,
    #[serde(default)]
    pub seller_commission: i64,
    #[serde(default)]
    pub can_trade: bool,
    #[serde(default)]
    pub can_withdraw: bool,
    #[serde(default)]
    pub can_deposit: bool,
    #[serde(default)]
    pub update_time: u64,
    #[serde(default)]
    pub account_type: String,
    #[serde(default)]
    pub balances: Vec<Balance>,
    #[serde(default)]
    pub permissions: Vec<String>,
}

#[allow(non_snake_case)]
#[derive(Debug, Deserialize)]
pub struct Ticker24hr {
    pub symbol: String,
    pub priceChangePercent: String,
    // Add additional fields if needed.
}

#[derive(Debug, Deserialize)]
struct TickerPrice {
    price: String,
}

#[allow(non_snake_case)]
#[derive(Debug, Deserialize)]
struct OrderResponse {
    executedQty: String,
}

pub struct Binance {
    client: Client,
    base_url: String,
}

#[derive(Debug, Clone)]
pub struct TrackedPosition {
    pub symbol: String,
    pub entry_price: f64,
    pub current_stop_price: f64,
    pub quantity: f64,
}

impl Binance {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
            base_url: "https://api.binance.com/api/v3".to_string(),
        }
    }

     /// Fetches the exchange information from Binance.
    pub async fn get_exchange_info(&self) -> Result<ExchangeInfo, reqwest::Error> {
        let url = format!("{}/exchangeInfo", self.base_url);
        let response = self.client.get(&url).send().await?;
        let info = response.json::<ExchangeInfo>().await?;
        Ok(info)
    }

    /// Returns a list of all trading pairs where the quote asset is USDT and status is TRADING.
    pub async fn get_pairs(&self,quote_asset: &str) -> Result<Vec<SymbolInfo>, reqwest::Error> {
        let exchange_info = self.get_exchange_info().await?;
        let asset_pairs: Vec<SymbolInfo> = exchange_info.symbols.into_iter()
            .filter(|s| s.quote_asset == quote_asset && s.status == "TRADING")
            .collect();
        Ok(asset_pairs)
    }

    /// Fetches aggregated 24hr ticker data for all symbols in one call.
    pub async fn get_all_ticker_24hr(&self) -> Result<Vec<Ticker24hr>, reqwest::Error> {
        let url = format!("{}/ticker/24hr", self.base_url);
        let response = self.client.get(&url).send().await?;
        let tickers = response.json::<Vec<Ticker24hr>>().await?;
        Ok(tickers)
    }

    /// Fetch historical candlestick data (klines) for a given symbol.
    /// `interval` could be "1h", "15m", etc., and `limit` is the number of candles.
    pub async fn get_klines(&self, symbol: &str, interval: &str, limit: u16) -> Result<Vec<Vec<Value>>, reqwest::Error> {
        let url = format!("{}/klines?symbol={}&interval={}&limit={}", self.base_url, symbol, interval, limit);
        let resp = self.client.get(&url).send().await?;
        let klines = resp.json::<Vec<Vec<Value>>>().await?;
        Ok(klines)
    }

    pub async fn subscribe_websocket(symbol: &str) {
        let url = format!("wss://stream.binance.com:9443/ws/{}@ticker", symbol.to_lowercase());
        let (ws_stream, _) = connect_async(Url::parse(&url).unwrap()).await.expect("WebSocket connection failed");
    
        println!("Connected to Binance WebSocket for {}", symbol);
    
        // Instead of splitting, iterate directly over the stream:
        let mut stream = ws_stream;
        while let Some(msg_result) = stream.next().await {
            match msg_result {
                Ok(msg) => {
                    if let Ok(text) = msg.into_text() {
                        println!("Received text: {}", text);
                    }
                }
                Err(e) => {
                    eprintln!("Error receiving message: {}", e);
                    break;
                }
            }
        }
    }

      /// Fetches account information from Binance using a signed request.
    /// The API key and secret are loaded from environment variables.
    pub async fn get_account_info(&self) -> Result<AccountInfo, reqwest::Error> {
        // Load API credentials from environment variables.
        let _ = from_filename("vars.env");
        let api_key = env::var("BINANCE_API_KEY")
            .expect("BINANCE_API_KEY must be set in vars.env");
        let secret_key = env::var("BINANCE_SECRET_KEY")
            .expect("BINANCE_SECRET_KEY must be set in vars.env");

        let endpoint = "/account";
        // Optional: set a recvWindow (default 5000 ms) to specify the allowed time difference.
        let recv_window = 5000;
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards")
            .as_millis();

        // Build the query string with both timestamp and recvWindow.
        let query = format!("timestamp={}&recvWindow={}", timestamp, recv_window);

        // Sign the query string using HMAC-SHA256 with the secret key.
        let mut mac = HmacSha256::new_from_slice(secret_key.as_bytes())
            .expect("HMAC can take key of any size");
        mac.update(query.as_bytes());
        let signature = hex_encode(mac.finalize().into_bytes());

        // Build the full URL including the signature.
        let url = format!("{}{}?{}&signature={}", self.base_url, endpoint, query, signature);
        //print!("{}", url);

        // Send the GET request with the API key in the header.
        let response = self.client
            .get(&url)
            .header("X-MBX-APIKEY", api_key)
            .send()
            .await?;

        // Read the response body as bytes.
        let bytes = response.bytes().await?;
        // Convert bytes to a string for debugging.
        //let raw_body = String::from_utf8_lossy(&bytes);
        //println!("Raw response body:\n{}", raw_body);
        // Deserialize the JSON from the bytes.
        let account_info: AccountInfo = serde_json::from_slice(&bytes)
            .expect("Failed to deserialize account info");
        Ok(account_info)
    }

    pub async fn get_account_balance(&self, asset: &str) -> Result<f64, reqwest::Error> {
        let account_info = self.get_account_info().await?;
        if let Some(balance) = account_info.balances.into_iter().find(|b| b.asset == asset) {
            if let Ok(free) = balance.free.parse::<f64>() {
                return Ok(free);
            }
        }
        Ok(0.0)
    }

    pub async fn get_open_order_symbols(&self) -> Result<Vec<String>, reqwest::Error> {
        let _ = from_filename("vars.env");
        let api_key = env::var("BINANCE_API_KEY").expect("BINANCE_API_KEY not set");
        let secret_key = env::var("BINANCE_SECRET_KEY").expect("BINANCE_SECRET_KEY not set");
    
        let endpoint = "/openOrders";
        let recv_window = 5000;
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis();
    
        let query = format!("timestamp={}&recvWindow={}", timestamp, recv_window);
    
        let mut mac = HmacSha256::new_from_slice(secret_key.as_bytes()).unwrap();
        mac.update(query.as_bytes());
        let signature = hex_encode(mac.finalize().into_bytes());
    
        let url = format!("{}{}?{}&signature={}", self.base_url, endpoint, query, signature);

    
        let response = self.client
            .get(&url)
            .header("X-MBX-APIKEY", api_key)
            .send()
            .await?;
    
        let orders: Vec<OpenOrder> = response.json().await?;

        for order in &orders {
            info!("📘 Open Order: {} | Side: {} | Qty: {} | Price: {} | Type: {}",
                order.symbol,
                order.side,
                order.orig_qty,
                order.price,
                order.type_field,
            );
            println!("📘 Open Order: {} | Side: {} | Qty: {} | Price: {} | Type: {}",
                order.symbol,
                order.side,
                order.orig_qty,
                order.price,
                order.type_field,
            );
        }
        
        // Return only the symbols
        let symbols: Vec<String> = orders.into_iter().map(|o| o.symbol).collect();
        Ok(symbols)
    }
    
    pub async fn place_market_buy_order(&self,symbol: &str,quantity: f64,) -> Result<u64, Box<dyn StdError>> {
        let _ = from_filename("vars.env");
        let api_key = env::var("BINANCE_API_KEY").expect("Missing BINANCE_API_KEY");
        let secret_key = env::var("BINANCE_SECRET_KEY").expect("Missing BINANCE_SECRET_KEY");

        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis();

        let query = format!(
            "symbol={}&side=BUY&type=MARKET&quantity={:.5}&recvWindow=5000&timestamp={}",
            symbol,
            quantity,
            timestamp
        );

        let mut mac = HmacSha256::new_from_slice(secret_key.as_bytes()).unwrap();
        mac.update(query.as_bytes());
        let signature = hex_encode(mac.finalize().into_bytes());

        let url = format!(
            "{}{}?{}&signature={}",
            self.base_url,
            "/order",
            query,
            signature
        );

        let response = self
            .client
            .post(&url)
            .header("X-MBX-APIKEY", api_key)
            .send()
            .await?;

        let status = response.status();
        let body = response.text().await?;

        if status.is_success() {
            let parsed: serde_json::Value = serde_json::from_str(&body).unwrap_or_default();
            let order_id = parsed["orderId"].as_u64().unwrap_or(0);
            println!("✅ Market buy order placed successfully. Order ID: {}", order_id);
            info!("✅ Market buy order placed: {:?}", parsed);
            Ok(order_id)
        } else {
            eprintln!("❌ Failed to place market buy order: {}", body);
            info!("❌ Failed to place market buy order: {}", body);
            return Err(Box::<dyn StdError + Send + Sync>::from("Buy order failed"));
        }
    }

    pub async fn place_trailing_stop_sell_order(&self, symbol: &str, quantity: f64, callback_rate: f64,  activation_price: Option<f64>,) -> Result<u64, Box<dyn std::error::Error>> {
        let _ = from_filename("vars.env");
        let api_key = env::var("BINANCE_API_KEY").expect("Missing BINANCE_API_KEY");
        let secret_key = env::var("BINANCE_SECRET_KEY").expect("Missing BINANCE_SECRET_KEY");

        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis();

        let mut query_parts = vec![
            format!("symbol={}", symbol),
            "side=SELL".to_string(),
            "type=TRAILING_STOP_MARKET".to_string(),
            format!("quantity={:.5}", quantity),
            format!("callbackRate={:.1}", callback_rate),
            "recvWindow=5000".to_string(),
            format!("timestamp={}", timestamp),
        ];

        if let Some(price) = activation_price {
            query_parts.push(format!("activationPrice={}", price));
        }

        let query = query_parts.join("&");

        

        let mut mac = HmacSha256::new_from_slice(secret_key.as_bytes()).unwrap();
        mac.update(query.as_bytes());
        let signature = hex_encode(mac.finalize().into_bytes());

        let url = format!(
            "{}{}?{}&signature={}",
            self.base_url,
            "/order",
            query,
            signature
        );

        let response = self
            .client
            .post(&url)
            .header("X-MBX-APIKEY", api_key)
            .send()
            .await?;

        let status = response.status();
        let body = response.text().await?;

        if status.is_success() {
            let parsed: serde_json::Value = serde_json::from_str(&body).unwrap_or_default();
            let order_id = parsed["orderId"].as_u64().unwrap_or(0);
            println!("✅ Trailing stop order placed successfully. Order ID: {}", order_id);
            info!("✅ Trailing stop order placed successfully: {:?}", parsed);
            Ok(order_id)
        } else {
            eprintln!("❌ Failed to place trailing stop order: {}", body);
            info!("❌ Failed to place trailing stop order: {}", body);
            return Err(Box::new(std::io::Error::new(
                std::io::ErrorKind::Other,
                "Trailing stop order failed",
            )));
        }
    }
    
    pub async fn get_executed_quantity(&self, symbol: &str, order_id: u64) -> Result<f64, Error> {
        let _ = from_filename("vars.env");
        let api_key = env::var("BINANCE_API_KEY").expect("Missing BINANCE_API_KEY");
        let secret_key = env::var("BINANCE_SECRET_KEY").expect("Missing BINANCE_SECRET_KEY");

        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis();

        let query = format!("symbol={}&orderId={}&timestamp={}&recvWindow=5000", symbol, order_id, timestamp);

        let mut mac = HmacSha256::new_from_slice(secret_key.as_bytes()).unwrap();
        mac.update(query.as_bytes());
        let signature = hex_encode(mac.finalize().into_bytes());

        let url = format!("{}{}?{}&signature={}", self.base_url, "/order", query, signature);

        let response = self
            .client
            .get(&url)
            .header("X-MBX-APIKEY", api_key)
            .send()
            .await?;

        let order: OrderResponse = response.json().await?;
        let qty = order.executedQty.parse::<f64>().unwrap_or(0.0);
        Ok((qty * 100000.0).floor() / 100000.0)
    }

    pub async fn execute_trade_with_fallback_stop(&self,symbol: &str, activation_price: Option<f64>,) -> Result<(), Box<dyn StdError>> {
        let quote_asset = &symbol[symbol.len() - 4..];
        let (quote_amount, stop_loss_percent) = {
            let config = SHARED_CONFIG.read().unwrap();
            let i = config.quote_assets.iter().position(|a| a == quote_asset).unwrap_or(0);
            let quote_amount = config.transaction_amounts.get(i).copied().unwrap_or(5.0);
            let stop_loss_percent = config.stop_loss_percent;
            (quote_amount, stop_loss_percent)
        };
    
        // Get filters
        let filters = Binance::get_symbol_filters(self, symbol).await?;
    
        let raw_qty = self.calculate_quantity_for_quote(symbol, quote_amount).await?;
        let quantity = Binance::round_to_step(raw_qty, filters.step_size);
    
        if quantity < filters.min_qty {
            println!("❌ {}: Adjusted quantity {:.5} below minQty {:.5}. Skipping.", symbol, quantity, filters.min_qty);
            return Err(Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("Quantity too low: {} < {}", quantity, filters.min_qty),
            )));
        }
    
        println!("📈 Executing market buy for {} with {:.6} units ({} quote)", symbol, quantity, quote_amount);
        info!("📈 Executing market buy for {} with {:.6} units ({} quote)", symbol, quantity, quote_amount);
    
        // Wait briefly to ensure balance is updated on Binance's end
       // 1. Place market buy
        let buy_order_id = self.place_market_buy_order(symbol, quantity).await?;

        // 2. Wait briefly for wallet to update
        tokio::time::sleep(Duration::from_secs(10)).await;
    
        let base_asset = &symbol[..symbol.len() - 4];
        let confirmed_balance = self.get_account_balance(base_asset).await?;
        let adjusted_balance = Binance::round_to_step(confirmed_balance, filters.step_size);
    
        let current_price = self.get_price(symbol).await?;
    
        let supports_trailing = self
            .symbol_supports_order_type(symbol, "TRAILING_STOP_MARKET")
            .await
            .unwrap_or(false);
    
        if supports_trailing {
            println!("📉 Using TRAILING_STOP_MARKET for {}", symbol);
            info!("📉 Using TRAILING_STOP_MARKET for {}", symbol);
            self.place_trailing_stop_sell_order(symbol, adjusted_balance, stop_loss_percent, activation_price).await?;
        } else {
            println!("📉 Using STOP_LOSS_LIMIT for {}", symbol);
            info!("📉 Using STOP_LOSS_LIMIT for {}", symbol);
            let stop_price = current_price * (1.0 - stop_loss_percent / 100.0);
            let stop_price = Binance::round_to_step(stop_price, filters.tick_size);
            let limit_price = stop_price;
    
            self.place_stop_loss_limit_order(symbol, adjusted_balance, stop_price, limit_price).await?;
        }
    
        println!("✅ Trade + stop setup complete for {}", symbol);
        info!("✅ Trade + stop setup complete for {}", symbol);
        Ok(())
    }
    
    pub async fn count_today_losses(&self) -> Result<u32, Error> {
        let _ = dotenv::from_filename("vars.env");
        let api_key = env::var("BINANCE_API_KEY").expect("Missing BINANCE_API_KEY");
        let secret_key = env::var("BINANCE_SECRET_KEY").expect("Missing BINANCE_SECRET_KEY");

        let now = Utc::now();
        let start_of_day = now.date_naive().and_hms_opt(0, 0, 0).unwrap();
        let start_time = start_of_day.and_utc().timestamp_millis();
        let end_time = now.timestamp_millis();

        let query = format!(
            "startTime={}&endTime={}&timestamp={}&recvWindow=5000",
            start_time,
            end_time,
            end_time
        );

        let mut mac = HmacSha256::new_from_slice(secret_key.as_bytes()).unwrap();
        mac.update(query.as_bytes());
        let signature = hex_encode(mac.finalize().into_bytes());

        let url = format!(
            "{}{}?{}&signature={}",
            self.base_url,
            "/allOrders",
            query,
            signature
        );

        let response = self.client
            .get(&url)
            .header("X-MBX-APIKEY", api_key)
            .send()
            .await?;

            let text = response.text().await?;
            let orders: Vec<Order> = match serde_json::from_str(&text) {
                Ok(data) => data,
                Err(_) => {
                    info!("❌ Unexpected response: {}", text);
                    eprintln!("❌ Unexpected response: {}", text);
                    return Ok(0); // Gracefully assume no losses if response is malformed
                }
            };
        let mut losses = 0u32;
        let mut last_buy_price: Option<f64> = None;

        for order in orders.into_iter().filter(|o| o.status == "FILLED") {
            let qty = order.executed_qty.parse::<f64>().unwrap_or(0.0);
            let quote = order.cummulative_quote_qty.parse::<f64>().unwrap_or(0.0);

            if qty == 0.0 {
                continue;
            }

            let avg_price = quote / qty;

            match order.side.as_str() {
                "BUY" => {
                    last_buy_price = Some(avg_price);
                }
                "SELL" => {
                    if let Some(entry_price) = last_buy_price {
                        if avg_price < entry_price {
                            losses += 1;
                            println!("🔻 Loss detected: bought at {:.2}, sold at {:.2}", entry_price, avg_price);
                            info!("🔻 Loss detected: bought at {:.2}, sold at {:.2}", entry_price, avg_price);
                        } else {
                            info!("✅ Profit: bought at {:.2}, sold at {:.2}", entry_price, avg_price);
                            println!("✅ Profit: bought at {:.2}, sold at {:.2}", entry_price, avg_price);
                        }
                        last_buy_price = None; // clear after one SELL
                    }
                }
                _ => {}
            }
        }

        Ok(losses)
    }

    pub async fn should_pause_for_losses(&self) -> Result<bool, Error> {
        let max_losses = {
            let cfg = SHARED_CONFIG.read().unwrap();
            cfg.max_loss_day
        };

        match self.count_today_losses().await {
            Ok(losses) => {
                println!("Today's confirmed losses: {} (max allowed: {})", losses, max_losses);
                info!(" Today's confirmed losses: {} (max allowed: {})", losses, max_losses);
                Ok(losses >= max_losses)
            }
            Err(e) => {
                eprintln!("Could not check daily losses: {}", e);
                Ok(false) // fail open
            }
        }
    }

    pub async fn calculate_quantity_for_quote(&self,symbol: &str,quote_amount: f64,) -> Result<f64, Box<dyn std::error::Error>> {
        let url = format!("{}/ticker/price?symbol={}", self.base_url, symbol);
        let response = self.client.get(&url).send().await?;
        let ticker: TickerPrice = response.json().await?;

        let price = ticker.price.parse::<f64>().unwrap_or(0.0);
        if price == 0.0 {
            eprintln!("❌ {} returned zero price — skipping.", symbol);
            return Err(Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("Price for {} could not be parsed or was zero", symbol),
        )));
    }

        let quantity = quote_amount / price;
        let rounded = (quantity * 100000.0).floor() / 100000.0; // round down to 5 decimal places

        info!("Calculated quantity for {} at {:.6} price: {:.6} units for {:.2} {}", symbol, price, rounded, quote_amount, &symbol[symbol.len()-4..]);
        println!("Calculated quantity for {} at {:.6} price: {:.6} units for {:.2} {}", symbol, price, rounded, quote_amount, &symbol[symbol.len()-4..]);

        Ok(rounded)
    }

    pub async fn supports_trailing_stop(&self, symbol: &str) -> Result<bool, Error> {
        let url = format!("{}/exchangeInfo?symbol={}", self.base_url, symbol);
        let response = self.client.get(&url).send().await?;
        let data: serde_json::Value = response.json().await?;
    
        let order_types = &data["symbols"][0]["orderTypes"];
        Ok(order_types.as_array()
            .map(|types| types.iter().any(|t| t == "TRAILING_STOP_MARKET"))
            .unwrap_or(false))
    }

    pub async fn get_spot_balances(&self) -> Result<Vec<(String, f64)>, Error> {
        let _ = from_filename("vars.env");
        let api_key = env::var("BINANCE_API_KEY").expect("Missing BINANCE_API_KEY");
        let secret_key = env::var("BINANCE_SECRET_KEY").expect("Missing BINANCE_SECRET_KEY");
    
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis();
    
        let query = format!("timestamp={}&recvWindow=5000", timestamp);
    
        let mut mac = HmacSha256::new_from_slice(secret_key.as_bytes()).unwrap();
        mac.update(query.as_bytes());
        let signature = hex_encode(mac.finalize().into_bytes());
    
        let url = format!("{}{}?{}&signature={}", self.base_url, "/account", query, signature);
    
        let response = self
            .client
            .get(&url)
            .header("X-MBX-APIKEY", api_key)
            .send()
            .await?;
    
        let account: AccountInfo = response.json().await?;
    
        let excluded_assets = {
            let cfg = crate::config::SHARED_CONFIG.read().unwrap();
            cfg.excluded_assets_spot.clone()
        };
        //println!("Excluded assets: {:?}", excluded_assets);
        let threshold = 0.0001; 
        let holdings = account
            .balances
            .into_iter()
            .filter_map(|b| {
                let free = b.free.parse::<f64>().unwrap_or(0.0);
                if free > threshold && !excluded_assets.contains(&b.asset) {
                    Some((b.asset, free))
                } else {
                    None
                }
            })
            .collect();
    
        Ok(holdings)
    }
    
    pub async fn symbol_supports_order_type(&self, symbol: &str, order_type: &str,) -> Result<bool, Error> {
        let url = format!("{}/exchangeInfo?symbol={}", self.base_url, symbol);
        let response = self.client.get(&url).send().await?;
        let info: ExchangeInfo = response.json().await?;

        if let Some(symbol_info) = info.symbols.into_iter().find(|s| s.symbol == symbol) {
            Ok(symbol_info.order_types.contains(&order_type.to_string()))
        } else {
            Ok(false)
        }
    }

    /// Calculates a stop price given a current price and loss percentage
    fn calculate_stop_price(current_price: f64, stop_percent: f64) -> f64 {
    let stop_price = current_price * (1.0 - stop_percent / 100.0);
    (stop_price * 10000.0).floor() / 10000.0 // round to 4 decimals
}

/// Simulated trailing stop for symbols that do not support TRAILING_STOP_MARKET
    pub async fn update_stop_loss_loop(binance: &Binance,mut tracked: HashMap<String, TrackedPosition>, stop_loss_percent: f64,) {
    loop {
        for (symbol, mut position) in tracked.clone() {
            match binance.get_price(&symbol).await {
                Ok(current_price) => {
                    let new_stop = Self::calculate_stop_price(current_price, stop_loss_percent);

                    if new_stop > position.current_stop_price {
                        info!("🔁 Adjusting stop for {}: old {:.4} → new {:.4}", symbol, position.current_stop_price, new_stop);
                        // Here: cancel old STOP_LOSS_LIMIT and place a new one
                        // Placeholder: binance.cancel_order(symbol, order_id).await;
                        // Placeholder: binance.place_stop_loss_limit_order(symbol, quantity, new_stop).await;
                        position.current_stop_price = new_stop;
                        tracked.insert(symbol.clone(), position);
                    } else {
                        info!("✅ No adjustment needed for {}", symbol);
                    }
                }
                Err(e) => {
                    error!("Failed to fetch price for {}: {}", symbol, e);
                }
            }
        }

        // Wait for 15 minutes
        sleep(Duration::from_secs(15 * 60)).await;
    }
}

    pub async fn get_price(&self, symbol: &str) -> Result<f64, ReqwestError> {
        let url = format!("{}/ticker/price?symbol={}", self.base_url, symbol);
        let response = self.client.get(&url).send().await?;
        let ticker: TickerPrice = response.json().await?;
        let price = ticker.price.parse::<f64>().unwrap_or(0.0);
        Ok(price)
    }

    pub async fn place_stop_loss_limit_order(&self,symbol: &str,quantity: f64,stop_price: f64,limit_price: f64,) -> Result<u64, Box<dyn StdError>> {
        let _ = from_filename("vars.env");
        let api_key = env::var("BINANCE_API_KEY")?;
        let secret_key = env::var("BINANCE_SECRET_KEY")?;
    
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)?
            .as_millis();
    
        let query = format!(
            "symbol={}&side=SELL&type=STOP_LOSS_LIMIT&quantity={:.5}&stopPrice={:.4}&price={:.4}&timeInForce=GTC&recvWindow=5000&timestamp={}",
            symbol, quantity, stop_price, limit_price, timestamp
        );
    
        let mut mac = HmacSha256::new_from_slice(secret_key.as_bytes())?;
        mac.update(query.as_bytes());
        let signature = hex_encode(mac.finalize().into_bytes());
    
        let url = format!(
            "{}{}?{}&signature={}",
            self.base_url,
            "/order",
            query,
            signature
        );
    
        let response = self
            .client
            .post(&url)
            .header("X-MBX-APIKEY", api_key)
            .send()
            .await?;
    
        let status = response.status();
        let body = response.text().await?;
    
        if status.is_success() {
            let parsed: serde_json::Value = serde_json::from_str(&body)?;
            let order_id = parsed["orderId"].as_u64().unwrap_or(0);
            println!("✅ STOP_LOSS_LIMIT order placed for {}. Order ID: {}", symbol, order_id);
            info!("✅ STOP_LOSS_LIMIT order placed: {:?}", parsed);
            Ok(order_id)
        } else {
            eprintln!("❌ Failed to place STOP_LOSS_LIMIT for symbol {} order: {}", symbol, body);
            Err(Box::new(std::io::Error::new(
                std::io::ErrorKind::Other,
                "Failed to place stop loss limit order",
            )))
        }
    }

    /// Periodically check held spot tokens and ensure a stop-loss is in place or updated.
    pub async fn manage_stop_loss_limit_loop(&self) {
        loop {
            let timestamp = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S");
            println!("🔁 [{}] Starting stop-loss management loop", timestamp);
            info!("🔁 Starting stop-loss management loop");
            let balances = match self.get_spot_balances().await {
                Ok(b) => b,
                Err(e) => {
                    error!("Failed to fetch balances: {}", e);
                    println!("❌ Failed to fetch balances: {}", e);
                    sleep(Duration::from_secs(60)).await;
                    continue;
                }
            };
    
            let quote_assets = {
                let cfg = SHARED_CONFIG.read().unwrap();
                cfg.quote_assets.clone()
            };
    
            let open_orders = match self.get_open_orders().await {
                Ok(orders) => orders,
                Err(e) => {
                    error!("Failed to fetch full open orders: {}", e);
                    println!("❌ Failed to fetch full open orders: {}", e);
                    vec![]
                }
            };
    
            let trailing_stop_symbols: HashSet<String> = open_orders
                .iter()
                .filter(|o| o.type_field == "TRAILING_STOP_MARKET")
                .map(|o| o.symbol.clone())
                .collect();
    
            let stop_limit_symbols: HashSet<String> = open_orders
                .iter()
                .filter(|o| o.type_field == "STOP_LOSS_LIMIT")
                .map(|o| o.symbol.clone())
                .collect();
    
            for (asset, balance) in balances {
                if quote_assets.contains(&asset) {
                    continue;
                }
    
                for quote in &quote_assets {
                    let symbol = format!("{}{}", asset, quote);
    
                    if trailing_stop_symbols.contains(&symbol) {
                        println!("✅ Trailing stop already active for {}", symbol);
                        info!("✅ Trailing stop already active for {}", symbol);
                        continue;
                    }
    
                    if stop_limit_symbols.contains(&symbol) {
                        continue;
                    }
    
                    let stop_loss_percent = SHARED_CONFIG.read().unwrap().stop_loss_percent;
    
                    match self.get_price(&symbol).await {
                        Ok(price) => {
                            let filters = match Binance::get_symbol_filters(self, &symbol).await {
                                Ok(f) => f,
                                Err(e) => {
                                    println!("❌ Failed to fetch filters for {}: {}", symbol, e);
                                    continue;
                                }
                            };
    
                            let stop_price = Binance::round_to_step(price * (1.0 - stop_loss_percent / 100.0), filters.tick_size);
                            let quantity = Binance::round_to_step(balance, filters.step_size);
                            let notional = stop_price * quantity;
    
                            if quantity < 1.0 || quantity < filters.min_qty || stop_price <= 0.0 || stop_price < filters.min_price || notional < filters.min_notional {
                                //println!("❌ Skipping {} — stop {:.4}, qty {:.4}, notional {:.4} do not meet filters", symbol, stop_price, quantity, notional);
                                //info!("❌ Skipping {} — stop {:.4}, qty {:.4}, notional {:.4} do not meet filters", symbol, stop_price, quantity, notional);
                                continue;
                            }
    
                            println!("🔒 Placing initial stop-loss for {} at {:.4}", symbol, stop_price);
                            info!("🔒 Placing initial stop-loss for {} at {:.4}", symbol, stop_price);
    
                            if let Err(e) = self.place_stop_loss_limit_order(&symbol, quantity, stop_price, stop_price).await {
                                println!("❌ Failed to place stop-loss for {}: {}", symbol, e);
                                error!("❌ Failed to place stop-loss for {}: {}", symbol, e);
                            }
                        }
                        Err(e) => {
                            //println!("❌ Failed to fetch price for {}: {}", symbol, e);
                            error!("❌ Failed to fetch price for {}: {}", symbol, e);
                        }
                    }
                }
            }
    
            for symbol in &stop_limit_symbols {
                if trailing_stop_symbols.contains(symbol) {
                    continue;
                }
    
                let stop_loss_percent = SHARED_CONFIG.read().unwrap().stop_loss_percent;
    
                match self.get_price(symbol).await {
                    Ok(price) => {
                        let filters = match Binance::get_symbol_filters(self, symbol).await {
                            Ok(f) => f,
                            Err(e) => {
                                println!("❌ Failed to fetch filters for {}: {}", symbol, e);
                                continue;
                            }
                        };
    
                        if let Some(existing) = open_orders.iter().find(|o| o.symbol == *symbol && o.type_field == "STOP_LOSS_LIMIT") {
                            let existing_stop = existing.stop_price.parse::<f64>().unwrap_or(0.0);
                            let order_qty = existing.orig_qty.parse::<f64>().unwrap_or(0.0);
                            let quantity = Binance::round_to_step(order_qty, filters.step_size);
                            let stop_price = Binance::round_to_step(price * (1.0 - stop_loss_percent / 100.0), filters.tick_size);
    
                            if quantity == 0.0 {
                                println!("❌ Skipping update for {} — zero quantity", symbol);
                                info!("❌ Skipping update for {} — zero quantity", symbol);
                                continue;
                            }
    
                            println!("📊 {} market price: {:.4}, current stop: {:.4}, new stop: {:.4}", symbol, price, existing_stop, stop_price);
                            info!("📊 {} market price: {:.4}, current stop: {:.4}, new stop: {:.4}", symbol, price, existing_stop, stop_price);
    
                            let rounded_existing = Binance::round_to_step(existing_stop, filters.tick_size);
                            let rounded_new = Binance::round_to_step(stop_price, filters.tick_size);

                            if rounded_new > rounded_existing {
                                println!("🔁 Updating stop-loss for {} from {:.4} to {:.4}", symbol, existing_stop, stop_price);
                                info!("🔁 Updating stop-loss for {} from {:.4} to {:.4}", symbol, existing_stop, stop_price);
    
                                if let Err(e) = self.cancel_order(symbol, existing.order_id).await {
                                    println!("❌ Failed to cancel old stop-loss for {}: {}", symbol, e);
                                    error!("❌ Failed to cancel old stop-loss for {}: {}", symbol, e);
                                    continue;
                                }
    
                                if let Err(e) = self.place_stop_loss_limit_order(symbol, quantity, stop_price, stop_price).await {
                                    println!("❌ Failed to update stop-loss for {}: {}", symbol, e);
                                    error!("❌ Failed to update stop-loss for {}: {}", symbol, e);
                                }
                            } else {
                                println!("✅ No update needed for {} — stop {:.4} is still valid", symbol, existing_stop);
                                info!("✅ No update needed for {} — stop {:.4} is still valid", symbol, existing_stop);
                            }
                        }
                    }
                    Err(e) => {
                        println!("❌ Failed to fetch price for {}: {}", symbol, e);
                        error!("❌ Failed to fetch price for {}: {}", symbol, e);
                    }
                }
            }
    
            let interval = {
                let cfg = SHARED_CONFIG.read().unwrap();
                cfg.stop_loss_loop_seconds
            };
    
            println!("⏱ Sleeping {} seconds before next stop-loss check", interval);
            info!("⏱ Sleeping {} seconds before next stop-loss check", interval);
            sleep(Duration::from_secs(interval)).await;
        }
    }

    pub async fn get_symbol_filters(binance: &Binance, symbol: &str) -> Result<SymbolFilters, Error> {
        let url = format!("{}/exchangeInfo?symbol={}", binance.base_url, symbol);
        let response = binance.client.get(&url).send().await?;
        let json: serde_json::Value = response.json().await?;
    
        let filters = &json["symbols"][0]["filters"];
    
        let mut tick_size = 0.0;
        let mut step_size = 0.0;
        let mut min_qty = 0.0;
        let mut min_price = 0.0;
        let mut min_notional = 0.0;
    
        for f in filters.as_array().unwrap_or(&vec![]) {
            if let Some(filter_type) = f.get("filterType").and_then(|v| v.as_str()) {
                match filter_type {
                    "PRICE_FILTER" => {
                        tick_size = f["tickSize"].as_str().unwrap_or("0.0").parse().unwrap_or(0.0);
                        min_price = f["minPrice"].as_str().unwrap_or("0.0").parse().unwrap_or(0.0);
                    },
                    "LOT_SIZE" => {
                        step_size = f["stepSize"].as_str().unwrap_or("0.0").parse().unwrap_or(0.0);
                        min_qty = f["minQty"].as_str().unwrap_or("0.0").parse().unwrap_or(0.0);
                    },
                    "MIN_NOTIONAL" => {
                        min_notional = f["minNotional"].as_str().unwrap_or("0.0").parse().unwrap_or(0.0);
                    },
                    _ => {}
                }
            }
        }
    
        Ok(SymbolFilters {
            tick_size,
            step_size,
            min_qty,
            min_price,
            min_notional,
        })
    }
    
    pub fn round_to_step(value: f64, step: f64) -> f64 {
        (value / step).floor() * step
    }

    pub async fn get_open_orders(&self) -> Result<Vec<OpenOrder>, Error> {
        let _ = from_filename("vars.env");
        let api_key = env::var("BINANCE_API_KEY").expect("Missing BINANCE_API_KEY");
        let secret_key = env::var("BINANCE_SECRET_KEY").expect("Missing BINANCE_SECRET_KEY");
    
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis();
    
        let query = format!("timestamp={}&recvWindow=5000", timestamp);
    
        let mut mac = HmacSha256::new_from_slice(secret_key.as_bytes()).unwrap();
        mac.update(query.as_bytes());
        let signature = hex_encode(mac.finalize().into_bytes());
    
        let url = format!(
            "{}{}?{}&signature={}",
            self.base_url,
            "/openOrders",
            query,
            signature
        );
    
        let response = self.client
            .get(&url)
            .header("X-MBX-APIKEY", api_key)
            .send()
            .await?;
    
        let orders: Vec<OpenOrder> = response.json().await?;
        Ok(orders)
    }
    
    pub async fn cancel_order(&self, symbol: &str, order_id: u64) -> Result<(), Box<dyn StdError>> {
        let _ = from_filename("vars.env");
        let api_key = env::var("BINANCE_API_KEY")?;
        let secret_key = env::var("BINANCE_SECRET_KEY")?;
    
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)?
            .as_millis();
    
        let query = format!(
            "symbol={}&orderId={}&recvWindow=5000&timestamp={}",
            symbol, order_id, timestamp
        );
    
        let mut mac = HmacSha256::new_from_slice(secret_key.as_bytes())?;
        mac.update(query.as_bytes());
        let signature = hex_encode(mac.finalize().into_bytes());
    
        let url = format!(
            "{}{}?{}&signature={}",
            self.base_url,
            "/order",
            query,
            signature
        );
    
        let response = self
            .client
            .delete(&url)
            .header("X-MBX-APIKEY", api_key)
            .send()
            .await?;
    
        let status = response.status();
        let body = response.text().await?;
    
        if status.is_success() {
            println!("🗑️ Cancelled order {} on {}", order_id, symbol);
            Ok(())
        } else {
            eprintln!("❌ Failed to cancel order {} on {}: {}", order_id, symbol, body);
            Err(Box::new(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Failed to cancel order: {}", body),
            )))
        }
    }

    pub async fn get_spot_trade_history(&self, symbol: &str, start_time: Option<u64>, end_time: Option<u64>) -> Result<Vec<serde_json::Value>, Box<dyn StdError>> {
        let _ = from_filename("vars.env");
        let api_key = env::var("BINANCE_API_KEY")?;
        let secret_key = env::var("BINANCE_SECRET_KEY")?;
    
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)?
            .as_millis();
    
        let mut query = format!("symbol={}&timestamp={}", symbol, timestamp);
        if let Some(start) = start_time {
            query.push_str(&format!("&startTime={}", start));
        }
        if let Some(end) = end_time {
            query.push_str(&format!("&endTime={}", end));
        }
    
        let mut mac = HmacSha256::new_from_slice(secret_key.as_bytes())?;
        mac.update(query.as_bytes());
        let signature = hex_encode(mac.finalize().into_bytes());
    
        let url = format!(
            "{}{}?{}&signature={}",
            self.base_url,
            "/myTrades",
            query,
            signature
        );
    
        let response = self.client
            .get(&url)
            .header("X-MBX-APIKEY", api_key)
            .send()
            .await?;
    
        let trades: Vec<serde_json::Value> = response.json().await?;
        Ok(trades)
    }
}






    