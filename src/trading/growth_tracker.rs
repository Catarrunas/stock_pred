pub struct GrowthTracker {
    pub last_price: Option<f64>,
    pub growth_threshold: f64,
}

impl GrowthTracker {
    pub fn new(growth_threshold: f64) -> Self {
        Self {
            last_price: None,
            growth_threshold,
        }
    }

    pub fn update(&mut self, current_price: f64) {
        if let Some(previous_price) = self.last_price {
            let change = ((current_price - previous_price) / previous_price) * 100.0;
            println!("Price Change: {:.2}% ({} → {})", change, previous_price, current_price);

            if change >= self.growth_threshold {
                println!("📈 Possible Pump Incoming! Growth {:.2}% exceeds threshold {:.2}%", change, self.growth_threshold);
            }
        }
        self.last_price = Some(current_price);
    }
}