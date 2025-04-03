use serde_json::Value;

pub fn compute_rsi(prices: &[f64], period: usize) -> Option<f64> {
    if prices.len() < period + 1 {
        return None;
    }
    let mut gains = Vec::new();
    let mut losses = Vec::new();
    for i in 1..prices.len() {
        let change = prices[i] - prices[i - 1];
        if change > 0.0 {
            gains.push(change);
            losses.push(0.0);
        } else {
            gains.push(0.0);
            losses.push(-change);
        }
    }
    let avg_gain: f64 = gains.iter().take(period).sum::<f64>() / period as f64;
    let avg_loss: f64 = losses.iter().take(period).sum::<f64>() / period as f64;
    if avg_loss == 0.0 {
        return Some(100.0);
    }
    let rs = avg_gain / avg_loss;
    Some(100.0 - (100.0 / (1.0 + rs)))
}

pub fn compute_average_volume(klines: &[Vec<Value>]) -> Option<f64> {
    let mut total = 0.0;
    let mut count = 0;
    for kline in klines {
        if let Some(volume_str) = kline.get(5).and_then(|v| v.as_str()) {
            if let Ok(volume) = volume_str.parse::<f64>() {
                total += volume;
                count += 1;
            }
        }
    }
    if count > 0 {
        Some(total / count as f64)
    } else {
        None
    }
}