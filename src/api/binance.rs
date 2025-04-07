use reqwest::Client;
use serde::Deserialize;
use tokio_tungstenite::connect_async;
use tokio_stream::StreamExt;
use serde_json::Value;
use url::Url;
use std::env;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;
use hmac::{Hmac, Mac};
use sha2::Sha256;
type HmacSha256 = Hmac<Sha256>;
use hex::encode as hex_encode;
use dotenv::from_filename;

#[derive(Debug, Deserialize)]
pub struct ExchangeInfo {
    pub symbols: Vec<SymbolInfo>,
}

#[derive(Debug, Deserialize)]
pub struct SymbolInfo {
    pub symbol: String,
    pub status: String,
    pub base_asset: String,
    pub quote_asset: String,
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

pub struct Binance {
    client: Client,
    base_url: String,
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
}