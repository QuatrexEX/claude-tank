use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UsageData {
    pub five_hour: f64,
    pub five_hour_reset: Option<String>,
    pub seven_day: f64,
    pub seven_day_reset: Option<String>,
    pub opus: f64,
    pub opus_reset: Option<String>,
    pub sonnet: f64,
    pub sonnet_reset: Option<String>,
    pub extra_enabled: bool,
    pub extra_monthly_limit: Option<f64>,
    pub extra_used: Option<f64>,
    pub extra_utilization: Option<f64>,
}

pub fn format_tooltip(data: &UsageData, gauge_mode: &str, plan: &str) -> String {
    let (v5, v7, label) = if gauge_mode == "remaining" {
        (100.0 - data.five_hour, 100.0 - data.seven_day, "left")
    } else {
        (data.five_hour, data.seven_day, "used")
    };

    format!(
        "Claude Tank — {plan}\n5h: {v5:.0}% {label}  |  7d: {v7:.0}% {label}"
    )
}
