use serde::{Deserialize, Serialize};
use std::time::Instant;
use chrono::NaiveDate;
use chrono::Utc;
use std::collections::HashMap;
use tokio::sync::Mutex;
use lazy_static::lazy_static;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OpenOrder {
    pub symbol: String,
    #[serde(rename = "type")]
    pub type_field: String,
    pub side: String,
    pub price: String,
    #[serde(rename = "origQty")]
    pub orig_qty: String,
    #[serde(rename = "executedQty")]
    pub executed_qty: String,
    pub status: String,
    #[serde(rename = "timeInForce")]
    pub time_in_force: String,
    #[serde(rename = "stopPrice")]
    pub stop_price: String,
    #[serde(rename = "icebergQty")]
    pub iceberg_qty: String,
    pub time: u64,
    #[serde(rename = "updateTime")]
    pub update_time: u64,
    #[serde(rename = "isWorking")]
    pub is_working: bool,
    #[serde(rename = "origQuoteOrderQty")]
    pub orig_quote_order_qty: String,
    #[serde(rename = "orderId")]
    pub order_id: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Order {
    pub symbol: String,
    pub side: String,
    pub status: String,
    #[serde(rename = "type")]
    pub type_field: String,
    #[serde(rename = "executedQty")]
    pub executed_qty: String,
    #[serde(rename = "cummulativeQuoteQty")]
    pub cummulative_quote_qty: String,
    #[serde(rename = "updateTime")]
    pub update_time: u64,
}

#[derive(Debug, Clone)]
pub struct Signal {
    pub symbol: String,
    pub overall_growth: f64,
    pub recent_growth: f64,
    pub avg_fluct_raw: f64,
    pub avg_fluct_pct: f64,
}

#[derive(Debug, Clone, Copy)]
pub enum TrendDirection {
    Positive,
    Negative,
}

#[derive(Debug)]
pub struct GlobalLossTracker {
    pub consecutive_losses: u32,
    pub last_reset_date: NaiveDate,
    pub cooldown_until: Option<Instant>,
}

lazy_static! {
    pub static ref PURCHASE_PRICES: Mutex<HashMap<String, f64>> = Mutex::new(HashMap::new());
}

impl GlobalLossTracker {
    pub fn new() -> Self {
        Self {
            consecutive_losses: 0,
            last_reset_date: Utc::now().date_naive(),
            cooldown_until: None,
        }
    }

    pub fn reset_if_new_day(&mut self) {
        let today = Utc::now().date_naive();
        if self.last_reset_date != today {
            self.consecutive_losses = 0;
            self.last_reset_date = today;
            self.cooldown_until = None;
        }
    }

    pub fn record_loss(&mut self, max_losses: u32, cooldown_seconds: u64) -> bool {
        self.reset_if_new_day();
        self.consecutive_losses += 1;
        if self.consecutive_losses >= max_losses {
            self.cooldown_until = Some(Instant::now() + std::time::Duration::from_secs(cooldown_seconds));
            return true;
        }
        false
    }

    pub fn is_on_cooldown(&self) -> bool {
        match self.cooldown_until {
            Some(until) => Instant::now() < until,
            None => false,
        }
    }
}