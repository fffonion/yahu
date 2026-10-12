const MIMO_API_BASE: &str = "https://platform.xiaomimimo.com/api/v1";
fn mimo_service_cookie_path(hermes_home: &std::path::Path) -> std::path::PathBuf {
    hermes_home.join("state/mimo_service_token.json")
}

fn load_mimo_service_cookie(hermes_home: &std::path::Path) -> Option<String> {
    let body = std::fs::read_to_string(mimo_service_cookie_path(hermes_home)).ok()?;
    let values = serde_json::from_str::<HashMap<String, String>>(&body).ok()?;
    let required = [
        "api-platform_serviceToken",
        "api-platform_slh",
        "api-platform_ph",
        "userId",
    ];
    required
        .iter()
        .all(|key| values.get(*key).is_some_and(|value| !value.is_empty()))
        .then(|| {
            required
                .iter()
                .filter_map(|key| values.get(*key).map(|value| format!("{key}={value}")))
                .collect::<Vec<_>>()
                .join("; ")
        })
}

fn save_mimo_service_cookie(
    hermes_home: &std::path::Path,
    values: &HashMap<String, String>,
) -> Result<(), String> {
    save_private_provider_session(&mimo_service_cookie_path(hermes_home), values)
        .map_err(|_| "MiMo 会话缓存保存失败".to_string())
}

async fn refresh_mimo_service_cookie(state: &AppState) -> Result<String, String> {
    let hermes_home = state.hermes_home.clone();
    let passtoken = provider_env_value(&hermes_home, "MIMO_PASSTOKEN");
    let user_id = provider_env_value(&hermes_home, "MIMO_USER_ID");
    let device_id = provider_env_value(&hermes_home, "MIMO_DEVICE_ID");
    if passtoken.is_empty() {
        return Err("缺少 MiMo 账号认证信息".into());
    }
    let helper = hermes_home
        .parent()
        .unwrap_or(hermes_home.as_path())
        .join("workspace/api-usage/mimo_auth_helper.py");
    if !helper.is_file() {
        return Err("缺少 MiMo 认证 helper".into());
    }
    let python = hermes_home.join("hermes-agent/venv/bin/python");
    let output = tokio::task::spawn_blocking(move || {
        let executable = if python.is_file() {
            python
        } else {
            std::path::PathBuf::from("python3")
        };
        std::process::Command::new(executable)
            .arg(helper)
            .env("MIMO_PASSTOKEN", passtoken)
            .env("MIMO_USER_ID", user_id)
            .env("MIMO_DEVICE_ID", device_id)
            .output()
    })
    .await
    .map_err(|err| format!("MiMo 认证 helper 失败：{err}"))?
    .map_err(|err| format!("MiMo 认证 helper 启动失败：{err}"))?;
    if !output.status.success() {
        return Err("MiMo 认证交换失败，请重新登录 Xiaomi 账号".into());
    }
    let value = String::from_utf8_lossy(&output.stdout)
        .lines()
        .rev()
        .find_map(|line| serde_json::from_str::<HashMap<String, String>>(line).ok())
        .ok_or_else(|| "MiMo 认证交换未返回会话".to_string())?;
    let required = [
        "api-platform_serviceToken",
        "api-platform_slh",
        "api-platform_ph",
        "userId",
    ];
    if required.iter().any(|key| value.get(*key).is_none_or(String::is_empty)) {
        return Err("MiMo 认证交换返回的会话不完整".into());
    }
    save_mimo_service_cookie(&hermes_home, &value)?;
    Ok(required
        .iter()
        .filter_map(|key| value.get(*key).map(|item| format!("{key}={item}")))
        .collect::<Vec<_>>()
        .join("; "))
}

fn mimo_headers(cookie: &str, referer: &str) -> Vec<(String, String)> {
    vec![
        ("Cookie".to_string(), cookie.to_string()),
        ("accept".to_string(), "application/json".into()),
        (
            "user-agent".to_string(),
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/148.0.0.0 Safari/537.36".into(),
        ),
        ("origin".to_string(), "https://platform.xiaomimimo.com".into()),
        ("referer".to_string(), referer.into()),
        ("x-timezone".to_string(), "Asia/Shanghai".into()),
    ]
}

fn mimo_auth_error(error: &str) -> bool {
    error.contains("HTTP 401") || error.contains("HTTP 403") || error.contains("Unauthorized")
}

async fn fetch_mimo_usage(state: &AppState) -> ProviderUsageSection {
    let mut section = ProviderUsageSection {
        provider: "mimo".into(),
        title: "MiMo 额度".into(),
        ..Default::default()
    };
    let mut cookie = load_mimo_service_cookie(&state.hermes_home)
        .or_else(|| {
            let configured = provider_env_value(&state.hermes_home, "MIMO_COOKIE");
            (!configured.is_empty()).then_some(configured)
        })
        .unwrap_or_default();
    if cookie.is_empty()
        && !provider_env_value(&state.hermes_home, "MIMO_PASSTOKEN").is_empty()
        && let Ok(refreshed) = refresh_mimo_service_cookie(state).await
    {
        cookie = refreshed;
    }
    if cookie.is_empty() {
        section.errors.push("缺少 MiMo 认证信息".into());
        return section;
    }

    let detail_result = provider_http_get_json(
        &state.client,
        &format!("{MIMO_API_BASE}/tokenPlan/detail"),
        &mimo_headers(&cookie, "https://platform.xiaomimimo.com/console/plan-manage"),
    )
    .await;
    let detail = match detail_result {
        Ok(value) => value,
        Err(first_error) if mimo_auth_error(&first_error) => {
            match refresh_mimo_service_cookie(state).await {
                Ok(refreshed) => {
                    cookie = refreshed;
                    match provider_http_get_json(
                        &state.client,
                        &format!("{MIMO_API_BASE}/tokenPlan/detail"),
                        &mimo_headers(&cookie, "https://platform.xiaomimimo.com/console/plan-manage"),
                    )
                    .await
                    {
                        Ok(value) => value,
                        Err(error) => {
                            section.errors.push(format!("MiMo 认证后查询失败：{error}"));
                            return section;
                        }
                    }
                }
                Err(error) => {
                    section.errors.push(format!("MiMo 认证交换失败：{error}"));
                    return section;
                }
            }
        }
        Err(err) => {
            section.errors.push(format!("查询失败：{err}"));
            return section;
        }
    };
    let detail = {
        let code = detail.get("code").and_then(Value::as_i64).unwrap_or(-1);
        if !(code == 0 || code == 200) {
            section.errors.push(format!("MiMo 用量接口返回错误码：{code}"));
            return section;
        }
        detail.get("data").cloned().unwrap_or(Value::Null)
    };

    let usage_summary = provider_http_get_json(
        &state.client,
        &format!("{MIMO_API_BASE}/tokenPlan/usage"),
        &mimo_headers(&cookie, "https://platform.xiaomimimo.com/console/plan-manage"),
    )
    .await
    .ok()
    .and_then(|value| value.get("data").cloned())
    .unwrap_or(Value::Null);
    let plan_used = detail_usage_number(&detail, "usage", "plan_total_token", "used")
        .or_else(|| detail_usage_number(&usage_summary, "usage", "plan_total_token", "used"));
    let plan_limit = detail_usage_number(&detail, "usage", "plan_total_token", "limit")
        .or_else(|| detail_usage_number(&usage_summary, "usage", "plan_total_token", "limit"));

    let plan_name = detail
        .get("planName")
        .and_then(Value::as_str)
        .unwrap_or("-")
        .to_string();
    let auto_renew = detail
        .get("enableAutoRenew")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let renew_tag = if detail
        .get("expired")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        "已过期"
    } else if auto_renew {
        "自动续费"
    } else {
        "不续费"
    };
    section.description = format!("MiMo {plan_name} · {renew_tag}");

    // Per-model billing-cycle usage rows.
    let period_end_raw = detail
        .get("currentPeriodEnd")
        .and_then(Value::as_str)
        .unwrap_or("");
    let (period_end_date, period_start_date) = parse_mimo_period(period_end_raw);
    let months_to_fetch = mimo_months_in_range(period_start_date, period_end_date);
    let ph = extract_cookie_value(&cookie, "api-platform_ph");

    #[derive(Default)]
    struct MimoBucket {
        hit: f64,
        miss: f64,
        out: f64,
    }
    const MIMO_COSTS: &[(&str, f64, f64, f64)] = &[
        ("mimo-v2.5-pro", 2.5, 300.0, 600.0),
        ("mimo-v2.5", 2.0, 100.0, 200.0),
        ("mimo-v2-pro", 140.0, 700.0, 2100.0),
        ("mimo-v2-omni", 56.0, 280.0, 1400.0),
    ];
    let bucket_credits = |model: &str, bucket: &MimoBucket| -> f64 {
        MIMO_COSTS
            .iter()
            .find(|(name, _, _, _)| *name == model)
            .map(|(_, hit_cost, miss_cost, out_cost)| {
                bucket.hit * hit_cost + bucket.miss * miss_cost + bucket.out * out_cost
            })
            .unwrap_or(0.0)
    };

    let mut month_buckets: BTreeMap<String, MimoBucket> = BTreeMap::new();
    for (year, month) in &months_to_fetch {
        let url = format!(
            "{MIMO_API_BASE}/usage/token-plan/list{}",
            if ph.is_empty() {
                String::new()
            } else {
                format!("?api-platform_ph={}", urlencoding_lite(&ph))
            }
        );
        let body = serde_json::json!({"year": year, "month": month});
        let rows = provider_http_post_json(
            &state.client,
            &url,
            body,
            &mimo_headers(&cookie, "https://platform.xiaomimimo.com/console/plan-manage"),
        )
        .await;
        let Ok(rows) = rows else {
            continue;
        };
        let code = rows.get("code").and_then(Value::as_i64).unwrap_or(-1);
        if !(code == 0 || code == 200) {
            continue;
        }
        for row in rows.get("data").and_then(Value::as_array).cloned().unwrap_or_default() {
            let model = row
                .get("model")
                .and_then(Value::as_str)
                .unwrap_or("?")
                .to_string();
            let date = row
                .get("date")
                .and_then(Value::as_str)
                .unwrap_or("")
                .chars()
                .take(10)
                .collect::<String>();
            let row_date = chrono::NaiveDate::parse_from_str(&date, "%Y-%m-%d").ok();
            let hit = row.get("inputHitToken").and_then(Value::as_f64).unwrap_or(0.0);
            let miss = row
                .get("inputMissToken")
                .and_then(Value::as_f64)
                .unwrap_or(0.0);
            let out = row.get("outputToken").and_then(Value::as_f64).unwrap_or(0.0);
            if let Some(row_date) = row_date
                && row_date >= period_start_date
                && row_date <= period_end_date
            {
                let entry = month_buckets.entry(model.clone()).or_default();
                entry.hit += hit;
                entry.miss += miss;
                entry.out += out;
            }
        }
    }

    let total_model_credit: f64 = month_buckets
        .iter()
        .map(|(model, bucket)| bucket_credits(model, bucket))
        .sum();
    if let Some(percent) = mimo_usage_percent(plan_used, plan_limit) {
        section.windows.push(ProviderUsageWindow {
            window: "月额度".into(),
            used: Some(format!("{percent:.2}%")),
            reset: mimo_reset_duration(period_end_raw),
            reset_at: mimo_reset_at(period_end_raw),
        });
    }
    for (model, bucket) in &month_buckets {
        let input = bucket.hit + bucket.miss;
        let credits = bucket_credits(model, bucket);
        let pct = match (plan_used, plan_limit) {
            (Some(_), Some(limit)) if limit > 0.0 && total_model_credit > 0.0 => {
                let share = credits / total_model_credit * plan_used.unwrap_or(0.0);
                format!("{:.2}%", share / limit * 100.0)
            }
            _ => "-".to_string(),
        };
        section.rows.push(ProviderUsageRow {
            label: model.clone(),
            hit_rate: (input > 0.0).then(|| format!("{:.1}%", bucket.hit / input * 100.0)),
            input: Some(fmt_provider_int(input)),
            output: Some(fmt_provider_int(bucket.out)),
            cost_or_pct: (!pct.eq("-")).then_some(pct),
        });
    }
    section
}

fn detail_usage_number(detail: &Value, group: &str, item_name: &str, field: &str) -> Option<f64> {
    detail
        .get(group)
        .and_then(|group| group.get("items"))
        .and_then(Value::as_array)?
        .iter()
        .find(|item| item.get("name").and_then(Value::as_str) == Some(item_name))
        .and_then(|item| item.get(field))
        .and_then(Value::as_f64)
}

fn mimo_usage_percent(used: Option<f64>, limit: Option<f64>) -> Option<f64> {
    match (used, limit) {
        (Some(used), Some(limit)) if limit > 0.0 => Some((used / limit * 100.0).max(0.0)),
        _ => None,
    }
}

fn mimo_reset_at(value: &str) -> Option<i64> {
    chrono::DateTime::parse_from_rfc3339(value)
        .map(|date| date.timestamp())
        .or_else(|_| {
            chrono::NaiveDateTime::parse_from_str(value, "%Y-%m-%d %H:%M:%S")
                .map(|date| date.and_utc().timestamp())
        })
        .ok()
}

fn mimo_reset_duration(value: &str) -> Option<String> {
    let parsed = mimo_reset_at(value)?;
    Some(provider_reset_duration((parsed - chrono::Utc::now().timestamp()).max(0) as f64))
}

fn parse_mimo_period(period_end: &str) -> (chrono::NaiveDate, chrono::NaiveDate) {
    let parsed = chrono::NaiveDate::parse_from_str(
        period_end.chars().take(10).collect::<String>().as_str(),
        "%Y-%m-%d",
    );
    match parsed {
        Ok(end) => {
            let start = end - chrono::Duration::days(30);
            (end, start)
        }
        Err(_) => {
            let today = chrono::Local::now().date_naive();
            (today, today.with_day(1).unwrap_or(today))
        }
    }
}

fn mimo_months_in_range(
    start: chrono::NaiveDate,
    end: chrono::NaiveDate,
) -> Vec<(i32, u32)> {
    let mut months = Vec::new();
    let mut cursor = chrono::NaiveDate::from_ymd_opt(start.year(), start.month(), 1)
        .unwrap_or(start);
    let end_month = chrono::NaiveDate::from_ymd_opt(end.year(), end.month(), 1).unwrap_or(end);
    while cursor <= end_month {
        months.push((cursor.year(), cursor.month()));
        cursor = if cursor.month() == 12 {
            chrono::NaiveDate::from_ymd_opt(cursor.year() + 1, 1, 1).unwrap_or(cursor)
        } else {
            chrono::NaiveDate::from_ymd_opt(cursor.year(), cursor.month() + 1, 1)
                .unwrap_or(cursor)
        };
    }
    months
}
