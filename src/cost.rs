use crate::types::Usage;

/// Per-model pricing (USD per million tokens)
#[derive(Debug, Clone)]
pub struct ModelPricing {
    pub input: f64,
    pub output: f64,
    pub cache_read: f64,
    pub cache_write: f64,
    pub context_window: u64,
}

impl ModelPricing {
    fn lookup(model: &str) -> Self {
        let mut m = model.to_lowercase();
        
        let has_1m_suffix = m.contains("[1m]");
        if has_1m_suffix {
            m = m.replace("[1m]", "");
        }

        let mut pricing = if m.contains("opus-4-6") || m.contains("opus-4-5") {
            Self { input: 5.0, output: 25.0, cache_read: 0.5, cache_write: 6.25, context_window: 200_000 }
        } else if m.contains("opus") {
            Self { input: 15.0, output: 75.0, cache_read: 1.5, cache_write: 18.75, context_window: 200_000 }
        } else if m.contains("sonnet") {
            Self { input: 3.0, output: 15.0, cache_read: 0.3, cache_write: 3.75, context_window: 200_000 }
        } else if m.contains("haiku-4") {
            Self { input: 1.0, output: 5.0, cache_read: 0.1, cache_write: 1.25, context_window: 200_000 }
        } else if m.contains("haiku") {
            Self { input: 0.8, output: 4.0, cache_read: 0.08, cache_write: 1.0, context_window: 200_000 }
        } else if m.contains("gpt-4o-mini") {
            Self { input: 0.15, output: 0.6, cache_read: 0.075, cache_write: 0.15, context_window: 128_000 }
        } else if m.contains("gpt-4o") || m.contains("gpt-4-turbo") {
            Self { input: 2.5, output: 10.0, cache_read: 1.25, cache_write: 2.5, context_window: 128_000 }
        } else if m.contains("gpt-4") {
            Self { input: 30.0, output: 60.0, cache_read: 15.0, cache_write: 30.0, context_window: 8_192 }
        } else if m.contains("o1") || m.contains("o3") || m.contains("o4-mini") {
            Self { input: 3.0, output: 12.0, cache_read: 1.5, cache_write: 3.0, context_window: 128_000 }
        } else if m.contains("gemini-2.5-pro") {
            Self { input: 1.25, output: 10.0, cache_read: 0.31, cache_write: 1.25, context_window: 2_097_152 }
        } else if m.contains("gemini-2.5-flash") || m.contains("gemini-2.0-flash") {
            Self { input: 0.15, output: 0.6, cache_read: 0.037, cache_write: 0.15, context_window: 1_048_576 }
        } else if m.contains("gemini") {
            Self { input: 0.15, output: 0.6, cache_read: 0.037, cache_write: 0.15, context_window: 1_048_576 }
        } else {
            // Unknown model — use sonnet-tier as default
            Self { input: 3.0, output: 15.0, cache_read: 0.3, cache_write: 3.75, context_window: 128_000 }
        };

        if has_1m_suffix {
            pricing.context_window = 1_000_000;
        }

        pricing
    }

    pub fn calculate(&self, usage: &Usage) -> f64 {
        let inp = (usage.input_tokens as f64 / 1_000_000.0) * self.input;
        let out = (usage.output_tokens as f64 / 1_000_000.0) * self.output;
        let cr = (usage.cache_read_input_tokens as f64 / 1_000_000.0) * self.cache_read;
        let cw = (usage.cache_creation_input_tokens as f64 / 1_000_000.0) * self.cache_write;
        inp + out + cr + cw
    }
}

/// Accumulates cost across the entire session
#[derive(Debug, Clone)]
pub struct CostTracker {
    pub total_cost_usd: f64,
    pub total_usage: Usage,
    model: String,
    pricing: ModelPricing,
}

impl CostTracker {
    pub fn new(model: &str) -> Self {
        Self {
            total_cost_usd: 0.0,
            total_usage: Usage::default(),
            model: model.to_string(),
            pricing: ModelPricing::lookup(model),
        }
    }

    pub fn add(&mut self, usage: &Usage) -> f64 {
        let cost = self.pricing.calculate(usage);
        self.total_cost_usd += cost;
        self.total_usage.accumulate(usage);
        cost
    }

    pub fn format_cost(&self) -> String {
        if self.total_cost_usd > 0.5 {
            format!("${:.2}", self.total_cost_usd)
        } else {
            format!("${:.4}", self.total_cost_usd)
        }
    }

    pub fn format_summary(&self) -> String {
        let pct = if self.pricing.context_window > 0 {
            (self.total_usage.input_tokens as f64 / self.pricing.context_window as f64) * 100.0
        } else {
            0.0
        };
        
        format!(
            "Cost: {} | Tokens: {} in ({:.1}% of context) / {} out | Model: {}",
            self.format_cost(),
            self.total_usage.input_tokens,
            pct,
            self.total_usage.output_tokens,
            self.model,
        )
    }
}
