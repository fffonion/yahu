const KIMI_API_BASE: &str = "https://www.kimi.com";
async fn fetch_kimi_usage(state: &AppState) -> ProviderUsageSection {
    let mut section = ProviderUsageSection {
        provider: "kimi".into(),
        title: "Kimi Code 额度".into(),
        ..Default::default()
    };
    let token = provider_env_value(&state.hermes_home, "KIMI_AUTH_TOKEN");
    if token.is_empty() {
        section.errors.push("缺少 KIMI_AUTH_TOKEN".into());
        return section;
    }
    let headers = vec![
        ("authorization".to_string(), format!("Bearer {token}")),
        ("content-type".to_string(), "application/json".into()),
        ("origin".to_string(), "https://www.kimi.com".into()),
        (
            "referer".to_string(),
            "https://www.kimi.com/code/console".into(),
        ),
        ("r-timezone".to_string(), "Asia/Shanghai".into()),
        ("x-msh-platform".to_string(), "web".into()),
    ];
    let data = match provider_http_post_json(
        &state.client,
        &format!("{KIMI_API_BASE}/apiv2/kimi.gateway.membership.v2.MembershipService/GetSubscriptionStats"),
        serde_json::json!({}),
        &headers,
    )
    .await
    {
        Ok(value) => value,
        Err(err) => {
            section.errors.push(format!("查询失败：{err}"));
            return section;
        }
    };
    let ratio_pct = |value: Option<f64>| -> String {
        match value {
            Some(ratio) if (0.0..=1.0).contains(&ratio) => format!("{:.0}%", ratio * 100.0),
            Some(ratio) => format!("{ratio:.0}%"),
            None => "-".into(),
        }
    };
    if let Some(five_h) = payload_enabled(data.get("ratelimitCode5h")) {
        section.windows.push(ProviderUsageWindow {
            window: "5h额度".into(),
            used: Some(ratio_pct(num_field(five_h, "ratio"))),
            reset: reset_from_ms(five_h, "resetTime"),
            reset_at: reset_at_from_ms(five_h, "resetTime"),
        });
    }
    if let Some(weekly) = payload_enabled(data.get("ratelimitCode7d")) {
        section.windows.push(ProviderUsageWindow {
            window: "周额度".into(),
            used: Some(ratio_pct(num_field(weekly, "ratio"))),
            reset: reset_from_ms(weekly, "resetTime"),
            reset_at: reset_at_from_ms(weekly, "resetTime"),
        });
    }
    if let Some(balance) = data.get("subscriptionBalance").filter(|v| v.is_object()) {
        section.windows.push(ProviderUsageWindow {
            window: "月额度".into(),
            used: Some(ratio_pct(num_field(balance, "amountUsedRatio"))),
            reset: reset_from_ms(balance, "expireTime"),
            reset_at: reset_at_from_ms(balance, "expireTime"),
        });
    }
    section
}

fn payload_enabled(value: Option<&Value>) -> Option<&Value> {
    let value = value?;
    value.get("enabled").and_then(Value::as_bool)?.then_some(value)
}

fn num_field(value: &Value, key: &str) -> Option<f64> {
    value.get(key).and_then(Value::as_f64)
}

fn kimi_time_text(value: Option<&Value>) -> String {
    let ms = value.and_then(Value::as_f64).unwrap_or(0.0);
    if ms <= 0.0 {
        return "-".into();
    }
    provider_reset_text_local((ms / 1000.0) as i64)
}
