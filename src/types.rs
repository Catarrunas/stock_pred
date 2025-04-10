use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OpenOrder {
    pub symbol: String,
    #[serde(rename = "type")]
    pub type_field: String,
    pub side: String,
    pub price: String,
    pub orig_qty: String,
    pub executed_qty: String,
    pub status: String,
    pub time_in_force: String,
    pub stop_price: String,
    pub iceberg_qty: String,
    pub time: u64,
    pub update_time: u64,
    pub is_working: bool,
    pub orig_quote_order_qty: String,
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
