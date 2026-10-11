use newapi::{
    NewApiAuth, NewApiClient, NewApiTimeRange, NewApiUsageRecord, aggregate_usage,
};

fn is_same_shanghai_day(timestamp: i64, now: chrono::DateTime<chrono::Utc>) -> bool {
    let timezone = chrono::FixedOffset::east_opt(8 * 60 * 60).expect("valid Shanghai offset");
    let Some(timestamp) = chrono::DateTime::<chrono::Utc>::from_timestamp(timestamp, 0) else {
        return false;
    };
    timestamp.with_timezone(&timezone).date_naive() == now.with_timezone(&timezone).date_naive()
}

fn newapi_usage_section(
    provider: &str,
    title: &str,
    records: &[NewApiUsageRecord],
    quota_per_unit: f64,
    remaining_quota: f64,
    used_quota: f64,
    now: chrono::DateTime<chrono::Utc>,
) -> ProviderUsageSection {
    let aggregates = aggregate_usage(records);
    let today_quota: f64 = records
        .iter()
        .filter(|record| is_same_shanghai_day(record.created_at, now))
        .map(|record| record.quota)
        .sum();
    let balance = if quota_per_unit > 0.0 {
        fmt_provider_money(remaining_quota / quota_per_unit, '$')
    } else {
        "-".to_string()
    };
    let used = if quota_per_unit > 0.0 {
        fmt_provider_money(used_quota / quota_per_unit, '$')
    } else {
        "-".to_string()
    };
    let today_cost = if quota_per_unit > 0.0 {
        fmt_provider_money(today_quota / quota_per_unit, '$')
    } else {
        "-".to_string()
    };
    let rows = aggregates
        .into_iter()
        .map(|row| ProviderUsageRow {
            label: row.model_name,
            input: Some(fmt_provider_int(row.token_used)),
            output: Some(fmt_provider_int(row.count)),
            cost_or_pct: (quota_per_unit > 0.0)
                .then(|| fmt_provider_money(row.quota / quota_per_unit, '$')),
            ..Default::default()
        })
        .collect();
    ProviderUsageSection {
        provider: provider.into(),
        title: title.into(),
        description: format!("余额 **{balance}**；累计已用 **{used}**"),
        rows,
        windows: vec![ProviderUsageWindow {
            window: "今日用量/费用".into(),
            used: Some(today_cost),
            reset: None,
            reset_at: None,
        }],
        ..Default::default()
    }
}

async fn fetch_newapi_usage(state: &AppState, provider: &str) -> ProviderUsageSection {
    let (title, user_keys, cookie_keys, token_keys, base_keys, default_base) = match provider {
        "hcnsec" => (
            "hcnsec 用量", &["HCNSEC_USER_ID"][..], &["HCNSEC_SESSION_COOKIE"][..],
            &["HCNSEC_ACCESS_TOKEN"][..], &["HCNSEC_BASE_URL"][..],
            "https://api.hcnsec.cn",
        ),
        "justwoker" => (
            "JustWoker 用量", &["JUSTWOKER_USER_ID"][..], &["JUSTWOKER_SESSION_COOKIE"][..],
            &["JUSTWOKER_ACCESS_TOKEN"][..], &["JUSTWOKER_BASE_URL"][..],
            "https://api.justwoker.icu",
        ),
        _ => (
            "AgenRouter 用量", &["AGENTROUTER_USER_ID", "AGENTROUTER_NEWAPI_USER_ID"][..],
            &["AGENTROUTER_SESSION_COOKIE", "AGENTROUTER_COOKIE"][..],
            &["AGENTROUTER_ACCESS_TOKEN", "AGENTROUTER_TOKEN"][..],
            &["AGENTROUTER_BASE_URL", "AGENTROUTER_NEWAPI_BASE_URL"][..],
            "https://ps.air-outer.com",
        ),
    };
    let mut section = ProviderUsageSection {
        provider: provider.into(),
        title: title.into(),
        ..Default::default()
    };
    let user_id = first_provider_env_value(&state.hermes_home, user_keys);
    let cookie = first_provider_env_value(&state.hermes_home, cookie_keys);
    let token = first_provider_env_value(&state.hermes_home, token_keys);
    if user_id.is_empty() || (cookie.is_empty() && token.is_empty()) {
        section.errors.push(format!("缺少 {} 与 {}（或 {}）", user_keys[0], cookie_keys[0], token_keys[0]));
        return section;
    }
    let auth = if !cookie.is_empty() {
        NewApiAuth::SessionCookie { cookie, user_id }
    } else {
        NewApiAuth::Bearer { token, user_id }
    };
    let base_url = first_provider_env_value(&state.hermes_home, base_keys);
    let base_url = if base_url.is_empty() {
        default_base.to_string()
    } else {
        base_url
    };
    let client = NewApiClient::new(&state.client, base_url, auth);
    let now = unix_now_seconds();
    let quota_per_unit = if let Some(cached) = cached_newapi_quota_per_unit(state, provider, now) {
        Some(cached)
    } else {
        match client.fetch_status().await {
            Ok(status) => {
                save_newapi_quota_per_unit(state, provider, status.quota_per_unit, now);
                Some(status.quota_per_unit)
            }
            Err(err) => {
                section.errors.push(format!("status 查询失败：{err}"));
                None
            }
        }
    };
    let user = match client.fetch_user().await {
        Ok(user) => Some(user),
        Err(err) => {
            section.errors.push(format!("账户余额查询失败：{err}"));
            None
        }
    };
    let now_utc = chrono::Utc::now();
    let range = NewApiTimeRange::shanghai_days(now_utc, 7);
    match client.fetch_usage(&range).await {
        Ok(records) => {
            let mut usage = newapi_usage_section(
                provider, title, &records,
                quota_per_unit.unwrap_or(0.0),
                user.as_ref().map(|user| user.quota).unwrap_or(0.0),
                user.as_ref().map(|user| user.used_quota).unwrap_or(0.0),
                now_utc,
            );
            usage.errors = section.errors;
            usage
        }
        Err(err) => {
            section.errors.push(format!("用量查询失败：{err}"));
            section
        }
    }
}
