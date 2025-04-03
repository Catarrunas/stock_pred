use reqwest::Client;
use serde::Deserialize;
use tokio_tungstenite::connect_async;
use tokio_stream::StreamExt;
use serde_json::Value;
use url::Url;



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
    pub free: f64,
    pub locked: f64,
}

#[derive(Debug, Deserialize)]
pub struct AccountInfo {
    pub maker_commission: i64,
    pub taker_commission: i64,
    pub buyer_commission: i64,
    pub seller_commission: i64,
    pub can_trade: bool,
    pub can_withdraw: bool,
    pub can_deposit: bool,
    pub update_time: u64,
    pub account_type: String,
    pub balances: Vec<Balance>,
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
    pub async fn get_usdt_pairs(&self) -> Result<Vec<SymbolInfo>, reqwest::Error> {
        let exchange_info = self.get_exchange_info().await?;
        let usdt_pairs: Vec<SymbolInfo> = exchange_info.symbols.into_iter()
            .filter(|s| s.quote_asset == "USDT" && s.status == "TRADING")
            .collect();
        Ok(usdt_pairs)
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

    /// Checks the available balance for a given asset (placeholder).
    /// For a real implementation, this requires signed requests.
    pub async fn get_account_balance(&self, _asset: &str) -> Result<f64, reqwest::Error> {
        // For demonstration purposes, return a dummy balance.
        // Replace this with a signed API call to GET /api/v3/account.
        Ok(1000.0)
    }

 /*/ /// Fetches account information using the signed /api/v3/account endpoint.
    /// Note: This function requires proper API key and secret.
    pub async fn get_account_info(&self, api_key: &str, secret_key: &str) -> Result<AccountInfo, reqwest::Error> {
        let base_url = "https://api.binance.com";
        let endpoint = "/api/v3/account";

        // Create a timestamp in milliseconds.
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards")
            .as_millis();

        // Build the query string with the timestamp.
        let mut params = Serializer::new(String::new())
            .append_pair("timestamp", &timestamp.to_string())
            .finish();

        // Generate the signature using HMAC SHA256.
        let mut mac = HmacSha256::new_from_slice(secret_key.as_bytes())
            .expect("HMAC can take key of any size");
        mac.update(params.as_bytes());
        let signature = hex::encode(mac.finalize().into_bytes());

        // Append the signature to the query string.
        params.push_str("&signature=");
        params.push_str(&signature);

        // Construct the full URL.
        let url = format!("{}{}?{}", base_url, endpoint, params);

        // Make the GET request with the API key in the header.
        let response = self.client
            .get(&url)
            .header("X-MBX-APIKEY", api_key)
            .send()
            .await?;

        // Parse and return the JSON response.
        let account_info = response.json::<AccountInfo>().await?;
        Ok(account_info)
    }*/
}