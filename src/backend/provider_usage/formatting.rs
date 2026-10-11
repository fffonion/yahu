fn fmt_provider_int(value: f64) -> String {
    if value >= 1_000_000.0 {
        format!("{:.2}m", value / 1_000_000.0)
    } else if value >= 10_000.0 {
        format!("{:.1}k", value / 1_000.0)
    } else {
        format!("{}", value.round() as i64)
    }
}

fn fmt_provider_money(value: f64, currency: char) -> String {
    let value = if value.abs() < 0.0005 { 0.0 } else { value };
    format!("{currency}{value:.2}")
}

fn provider_reset_duration(seconds: f64) -> String {
    let minutes = ((seconds.max(0.0) + 59.0) / 60.0).floor() as i64;
    if minutes < 60 {
        format!("{minutes}分钟")
    } else {
        let hours = (minutes + 59) / 60;
        if hours > 24 {
            format!("{}天", (hours + 23) / 24)
        } else {
            format!("{hours}小时")
        }
    }
}

fn provider_reset_text_local(value: i64) -> String {
    if value <= 0 {
        return "-".to_string();
    }
    // CommandCode returns reset_at in milliseconds while most providers use
    // Unix seconds. Normalize the millisecond form before subtracting now.
    let timestamp = if value >= 100_000_000_000 {
        value / 1000
    } else {
        value
    };
    let seconds = timestamp.saturating_sub(chrono::Utc::now().timestamp()) as f64;
    provider_reset_duration(seconds)
}

fn provider_number(value: &Value) -> Option<f64> {
    value
        .as_f64()
        .or_else(|| value.as_str()?.trim().parse::<f64>().ok())
        .filter(|number| number.is_finite())
}

fn json_string_or_number(value: Option<&Value>) -> Option<String> {
    let value = value?;
    if let Some(text) = value.as_str().map(str::trim).filter(|text| !text.is_empty()) {
        return Some(text.to_string());
    }
    if let Some(number) = value.as_i64() {
        return Some(number.to_string());
    }
    value.as_u64().map(|number| number.to_string())
}

fn reset_from_ms(value: &Value, key: &str) -> Option<String> {
    Some(kimi_time_text(value.get(key)))
}

fn reset_at_from_ms(value: &Value, key: &str) -> Option<i64> {
    provider_reset_at(value, key)
}

fn codex_timestamp(value: &Value) -> Option<i64> {
    let timestamp = if let Some(raw) = value.as_str() {
        if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(raw) {
            dt.timestamp()
        } else {
            provider_number(value)? as i64
        }
    } else {
        provider_number(value)? as i64
    };
    Some(normalize_provider_timestamp(timestamp))
}

fn provider_reset_at(value: &Value, key: &str) -> Option<i64> {
    value.get(key).and_then(codex_timestamp)
}

fn normalize_provider_timestamp(timestamp: i64) -> i64 {
    if timestamp >= 100_000_000_000 {
        timestamp / 1000
    } else {
        timestamp
    }
}
