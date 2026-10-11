const MINIMAX_PLAN_REMAINS_URL: &str =
    "https://www.minimaxi.com/v1/api/openplatform/coding_plan/remains";
const MINIMAX_USAGE_SUMMARY_URL: &str =
    "https://www.minimaxi.com/backend/account/token_plan/usage_summary";
fn minimax_used_percent(model: &Value, period: &str) -> Option<f64> {
    let (total_key, usage_key, remaining_key, boost_key) = match period {
        "interval" => (
            "current_interval_total_count",
            "current_interval_usage_count",
            "current_interval_remaining_percent",
            "interval_boost_permille",
        ),
        "weekly" => (
            "current_weekly_total_count",
            "current_weekly_usage_count",
            "current_weekly_remaining_percent",
            "weekly_boost_permille",
        ),
        _ => return None,
    };
    let total = provider_number(model.get(total_key).unwrap_or(&Value::Null)).unwrap_or(0.0);
    if total > 0.0 {
        let used = provider_number(model.get(usage_key).unwrap_or(&Value::Null)).unwrap_or(0.0);
        return Some(((total - used) / total * 100.0).clamp(0.0, 100.0));
    }
    let boost = provider_number(model.get(boost_key).unwrap_or(&Value::Null)).unwrap_or(1000.0);
    provider_number(model.get(remaining_key).unwrap_or(&Value::Null)).map(|remaining| {
        ((100.0 - remaining).max(0.0) * boost / 1000.0).max(0.0)
    })
}

fn minimax_capability(model_name: &str) -> &'static str {
    match model_name {
        "general" => "文本",
        "video" => "视频",
        "speech-hd" => "语音",
        "coding-plan-vlm" => "图片理解",
        "coding-plan-search" => "网络搜索",
        _ => "其他",
    }
}

fn minimax_summary_metrics(summary: &Value, today: chrono::NaiveDate) -> Vec<(String, f64)> {
    let series = summary
        .get("daily_token_usage")
        .and_then(Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(provider_number)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if series.is_empty() {
        return Vec::new();
    }
    let mut start_date = None;
    let mut day_index = series.len() - 1;
    if let Some(active) = summary.get("most_active_day") {
        let active_tokens = active.get("token_count").and_then(provider_number);
        let active_date = active
            .get("date")
            .and_then(Value::as_str)
            .and_then(|value| value.get(..10))
            .and_then(|value| chrono::NaiveDate::parse_from_str(value, "%Y-%m-%d").ok());
        if let (Some(active_tokens), Some(active_date)) = (active_tokens, active_date) {
            let (index, _) = series
                .iter()
                .enumerate()
                .map(|(index, value)| (index, (value - active_tokens).abs()))
                .min_by(|left, right| left.1.total_cmp(&right.1))
                .unwrap_or((series.len() - 1, 0.0));
            start_date = Some(active_date - chrono::Duration::days(index as i64));
            let offset = (today - start_date.unwrap() ).num_days();
            if offset >= 0 {
                day_index = (offset as usize).min(series.len() - 1);
            }
        }
    }
    let week_start = day_index.saturating_sub(6);
    let week_tokens = series[week_start..=day_index].iter().sum::<f64>();
    let month_tokens = if let Some(start) = start_date {
        let values = series[..=day_index]
            .iter()
            .enumerate()
            .filter(|(index, _)| {
                let date = start + chrono::Duration::days(*index as i64);
                date.year() == today.year() && date.month() == today.month()
            })
            .map(|(_, value)| *value)
            .sum::<f64>();
        if values > 0.0 {
            values
        } else {
            series[day_index.saturating_sub(29)..=day_index]
                .iter()
                .sum::<f64>()
        }
    } else {
        series[day_index.saturating_sub(29)..=day_index]
            .iter()
            .sum::<f64>()
    };
    vec![
        ("日".into(), series[day_index]),
        ("周".into(), week_tokens),
        ("月".into(), month_tokens),
    ]
}

fn minimax_request_headers(cookie: &str) -> Result<Vec<(String, String)>, &'static str> {
    if cookie.is_empty() {
        return Err("缺少 MINIMAX_COOKIE");
    }
    let group_id = extract_cookie_value(cookie, "minimax_group_id_v2");
    if group_id.is_empty() {
        return Err("MINIMAX_COOKIE 缺少 minimax_group_id_v2");
    }
    Ok(vec![
        ("Cookie".to_string(), cookie.to_string()),
        ("x-group-id".to_string(), group_id),
        ("Accept".to_string(), "application/json".into()),
        (
            "User-Agent".to_string(),
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/148.0.0.0 Safari/537.36".into(),
        ),
        ("Referer".to_string(), "https://platform.minimaxi.com/".into()),
    ])
}

async fn fetch_minimax_usage(state: &AppState) -> ProviderUsageSection {
    let mut section = ProviderUsageSection {
        provider: "minimax".into(),
        title: "MiniMax 额度".into(),
        ..Default::default()
    };
    let cookie = provider_env_value(&state.hermes_home, "MINIMAX_COOKIE");
    let headers = match minimax_request_headers(&cookie) {
        Ok(headers) => headers,
        Err(message) => {
            section.errors.push(message.into());
            return section;
        }
    };
    let payload =
        match provider_http_get_json_retry(&state.client, MINIMAX_PLAN_REMAINS_URL, &headers)
            .await
        {
            Ok(value) => value,
            Err(err) => {
                section.errors.push(format!("查询失败：{err}"));
                return section;
            }
        };
    if payload.pointer("/base_resp/status_code").and_then(Value::as_i64) != Some(0) {
        let msg = payload
            .pointer("/base_resp/status_msg")
            .and_then(Value::as_str)
            .unwrap_or("未知错误");
        section.errors.push(format!("API 错误：{msg}"));
        return section;
    }
    if let Ok(summary) = provider_http_get_json_retry(&state.client, MINIMAX_USAGE_SUMMARY_URL, &headers)
        .await
        && summary.pointer("/base_resp/status_code").and_then(Value::as_i64) == Some(0)
    {
        for (label, tokens) in minimax_summary_metrics(&summary, chrono::Local::now().date_naive()) {
            let value = fmt_provider_int(tokens);
            if label == "周"
                && let Some(row) = section.rows.iter_mut().find(|row| row.label == "周额度")
            {
                row.output = Some(value);
                continue;
            }
            let row_label = match label.as_str() {
                "周" => "周额度".to_string(),
                "月" => "月额度".to_string(),
                _ => format!("{label}用量"),
            };
            section.rows.push(ProviderUsageRow {
                label: row_label,
                output: Some(value),
                ..Default::default()
            });
        }
    }
    for model in payload
        .get("model_remains")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
    {
        let name = model
            .get("model_name")
            .and_then(Value::as_str)
            .unwrap_or("?")
            .to_string();
        if minimax_capability(&name) != "文本" {
            continue;
        }
        if let Some(used_pct) = minimax_used_percent(&model, "interval") {
            let value = format!("{used_pct:.1}%");
            section.windows.push(ProviderUsageWindow {
                window: "5h额度".into(),
                used: Some(value),
                reset: reset_from_ms(&model, "end_time"),
                reset_at: reset_at_from_ms(&model, "end_time"),
            });
        }
        if let Some(used_pct) = minimax_used_percent(&model, "weekly") {
            let value = format!("{used_pct:.1}%");
            section.windows.push(ProviderUsageWindow {
                window: "周额度".into(),
                used: Some(value),
                reset: reset_from_ms(&model, "weekly_end_time"),
                reset_at: reset_at_from_ms(&model, "weekly_end_time"),
            });
        }
    }
    section
}
