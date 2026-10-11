const STEPFUN_PLAN_USAGE_URL: &str =
    "https://platform.stepfun.com/api/step.openapi.devcenter.Dashboard/QueryStepPlanUsages";
const STEPFUN_PLAN_RATE_LIMIT_URL: &str =
    "https://platform.stepfun.com/api/step.openapi.devcenter.Dashboard/QueryStepPlanRateLimit";
const STEPFUN_REFRESH_TOKEN_URL: &str =
    "https://platform.stepfun.com/passport/proto.api.passport.v1.PassportService/RefreshToken";

#[derive(Default)]
struct StepFunAuthState {
    source_fingerprint: String,
    token: String,
}

fn stepfun_cookie_value(cookie: &str, name: &str) -> Option<String> {
    cookie.split(';').find_map(|part| {
        let (key, value) = part.trim().split_once('=')?;
        (key.eq_ignore_ascii_case(name) && !value.trim().is_empty()).then(|| value.trim().to_string())
    })
}

fn stepfun_token_device_id(token: &str) -> Option<String> {
    for jwt in token.rsplit("...") {
        let Some(payload) = jwt.split('.').nth(1) else {
            continue;
        };
        let payload = payload.trim_end_matches('=');
        let Ok(decoded) = base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(payload) else {
            continue;
        };
        let Ok(claims) = serde_json::from_slice::<Value>(&decoded) else {
            continue;
        };
        if let Some(device_id) = claims
            .get("device_id")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            return Some(device_id.to_string());
        }
    }
    None
}

fn stepfun_combined_token_from_response(value: &Value) -> Result<String, String> {
    let raw_token = |name: &str, alternate: &str| {
        value
            .get(name)
            .or_else(|| value.get(alternate))
            .and_then(|token| {
                token
                    .get("raw")
                    .or_else(|| token.get("value"))
                    .and_then(Value::as_str)
                    .or_else(|| token.as_str())
            })
            .map(str::trim)
            .filter(|token| !token.is_empty())
            .map(str::to_string)
    };
    let access = raw_token("accessToken", "access_token")
        .ok_or_else(|| "RefreshToken 响应缺少 access token".to_string())?;
    let refresh = raw_token("refreshToken", "refresh_token");
    Ok(refresh
        .filter(|token| !token.is_empty())
        .map(|refresh| format!("{access}...{refresh}"))
        .unwrap_or(access))
}

fn stepfun_cookie_with_auth(cookie: &str, token: &str, web_id: &str) -> String {
    let mut parts = vec![format!("Oasis-Token={token}"), format!("Oasis-Webid={web_id}")];
    parts.extend(cookie.split(';').filter_map(|part| {
        let trimmed = part.trim();
        let key = trimmed.split_once('=').map(|(key, _)| key.trim())?;
        if key.eq_ignore_ascii_case("Oasis-Token") || key.eq_ignore_ascii_case("Oasis-Webid") {
            return None;
        }
        (!trimmed.is_empty()).then(|| trimmed.to_string())
    }));
    parts.join("; ")
}

fn stepfun_request_headers_for_token(cookie: &str, token: &str) -> Vec<(String, String)> {
    let cookie_web_id = stepfun_cookie_value(cookie, "Oasis-Webid");
    let web_id = stepfun_token_device_id(token)
        .or(cookie_web_id)
        .unwrap_or_default();
    let request_cookie = if token.trim().is_empty() {
        cookie.to_string()
    } else {
        stepfun_cookie_with_auth(cookie, token, &web_id)
    };
    let mut headers = vec![
        ("accept".into(), "*/*".into()),
        (
            "accept-language".into(),
            "en-US,en;q=0.9,zh;q=0.8,zh-CN;q=0.7".into(),
        ),
        ("cache-control".into(), "no-cache".into()),
        ("connect-protocol-version".into(), "1".into()),
        ("content-type".into(), "application/json".into()),
        ("cookie".into(), request_cookie),
        ("dnt".into(), "1".into()),
        ("oasis-appid".into(), "10300".into()),
        ("oasis-platform".into(), "web".into()),
        ("origin".into(), "https://platform.stepfun.com".into()),
        ("pragma".into(), "no-cache".into()),
        ("priority".into(), "u=1, i".into()),
        (
            "referer".into(),
            "https://platform.stepfun.com/account-overview".into(),
        ),
        (
            "sec-ch-ua".into(),
            "\"Google Chrome\";v=\"153\", \"Not_A Brand\";v=\"8\", \"Chromium\";v=\"153\""
                .into(),
        ),
        ("sec-ch-ua-mobile".into(), "?0".into()),
        ("sec-ch-ua-platform".into(), "\"Windows\"".into()),
        ("sec-fetch-dest".into(), "empty".into()),
        ("sec-fetch-mode".into(), "cors".into()),
        ("sec-fetch-site".into(), "same-origin".into()),
        (
            "user-agent".into(),
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/153.0.0.0 Safari/537.36"
                .into(),
        ),
    ];
    if !web_id.is_empty() {
        headers.push(("oasis-webid".into(), web_id));
    }
    if !token.trim().is_empty() {
        headers.push(("oasis-token".into(), token.to_string()));
    }
    headers
}

fn stepfun_token_fingerprint(token: &str) -> String {
    let mut digest = sha2::Sha256::new();
    digest.update(token.as_bytes());
    format!("{:x}", digest.finalize())
}

async fn stepfun_active_token(state: &AppState, source_token: &str) -> String {
    let source_fingerprint = stepfun_token_fingerprint(source_token);
    let mut guard = state.provider_usage_cache.stepfun_auth.lock().await;
    if let Some(auth) = guard.as_ref()
        && auth.source_fingerprint == source_fingerprint
        && !auth.token.is_empty()
    {
        return auth.token.clone();
    }
    let token = source_token.to_string();
    *guard = Some(StepFunAuthState {
        source_fingerprint,
        token: token.clone(),
    });
    token
}

fn stepfun_auth_error(error: &str) -> bool {
    let error = error.to_ascii_lowercase();
    error.contains("http 401")
        || error.contains("unauthorized")
        || error.contains("token expired")
        || error.contains("session expired")
        || error.contains("auth failed")
        || error.contains("oasis-token")
}

fn stepfun_public_http_error(error: &str) -> String {
    let Some(rest) = error.strip_prefix("HTTP ") else {
        return "StepFun 请求失败".to_string();
    };
    let Some(status) = rest.split_whitespace().next() else {
        return "StepFun 请求失败".to_string();
    };
    if status.chars().all(|character| character.is_ascii_digit()) {
        format!("HTTP {status}")
    } else {
        "StepFun 请求失败".to_string()
    }
}

fn stepfun_result_needs_refresh(result: &Result<Value, String>) -> bool {
    match result {
        Err(error) => stepfun_auth_error(error),
        Ok(value) => {
            let unauthorized_status = [value.get("status"), value.get("code")]
                .into_iter()
                .flatten()
                .find_map(provider_number)
                .is_some_and(|code| code == 401.0);
            unauthorized_status
                || stepfun_response_status(value)
                    .err()
                    .is_some_and(|error| stepfun_auth_error(&error))
        }
    }
}

async fn stepfun_refresh_token(
    state: &AppState,
    cookie: &str,
    source_token: &str,
    failed_token: &str,
) -> Result<String, String> {
    let source_fingerprint = stepfun_token_fingerprint(source_token);
    let mut guard = state.provider_usage_cache.stepfun_auth.lock().await;
    if let Some(auth) = guard.as_ref()
        && auth.source_fingerprint == source_fingerprint
        && auth.token != failed_token
        && !auth.token.is_empty()
    {
        return Ok(auth.token.clone());
    }

    let headers = stepfun_request_headers_for_token(cookie, failed_token);
    let response = provider_http_post_json(
        &state.client,
        STEPFUN_REFRESH_TOKEN_URL,
        serde_json::json!({}),
        &headers,
    )
    .await
    .map_err(|_| "StepFun token 刷新请求失败".to_string())?;
    let token = stepfun_combined_token_from_response(&response)
        .map_err(|_| "StepFun token 刷新响应无有效 token".to_string())?;
    if token == failed_token {
        return Err("StepFun token 刷新未返回新 token".to_string());
    }
    *guard = Some(StepFunAuthState {
        source_fingerprint,
        token: token.clone(),
    });
    Ok(token)
}

async fn stepfun_post_json_with_refresh(
    state: &AppState,
    cookie: &str,
    source_token: &str,
    url: &str,
    body: Value,
) -> Result<Value, String> {
    let token = stepfun_active_token(state, source_token).await;
    let headers = stepfun_request_headers_for_token(cookie, &token);
    let result = provider_http_post_json(&state.client, url, body.clone(), &headers).await;
    if !stepfun_result_needs_refresh(&result) {
        return result.map_err(|error| stepfun_public_http_error(&error));
    }

    let refreshed = stepfun_refresh_token(state, cookie, source_token, &token).await?;
    let retry_headers = stepfun_request_headers_for_token(cookie, &refreshed);
    let retry = provider_http_post_json(&state.client, url, body, &retry_headers).await;
    if stepfun_result_needs_refresh(&retry) {
        return Err("StepFun token 刷新后仍未授权".to_string());
    }
    retry.map_err(|error| stepfun_public_http_error(&error))
}

fn stepfun_current_month_range_millis() -> (i64, i64) {
    let now = chrono::Utc::now();
    let start = chrono::NaiveDate::from_ymd_opt(now.year(), now.month(), 1)
        .and_then(|date| date.and_hms_opt(0, 0, 0))
        .map(|value| value.and_utc().timestamp_millis())
        .unwrap_or_else(|| now.timestamp_millis());
    (start, now.timestamp_millis())
}

fn stepfun_record_time_label(from_time: Option<i64>, to_time: Option<i64>) -> String {
    let format_time = |value: Option<i64>| {
        value
            .and_then(chrono::DateTime::<chrono::Utc>::from_timestamp_millis)
            .map(|time| time.format("%m-%d %H:%M").to_string())
    };
    match (format_time(from_time), format_time(to_time)) {
        (Some(from), Some(to)) => format!("{from} – {to}"),
        (Some(from), None) => from,
        (None, Some(to)) => to,
        (None, None) => "-".into(),
    }
}

fn stepfun_credit_text(value: f64) -> String {
    if value.abs() >= 1_000_000.0 {
        fmt_provider_int(value)
    } else {
        format!("{value:.2}")
    }
}

fn stepfun_response_status(value: &Value) -> Result<(), String> {
    let status_ok = match value.get("status") {
        None => true,
        Some(status) => status
            .as_i64()
            .map(|code| code == 1)
            .or_else(|| {
                status.as_str().map(|text| {
                    matches!(
                        text.to_ascii_uppercase().as_str(),
                        "OK" | "SUCCESS" | "STATUS_OK" | "STATUS_SUCCESS"
                    )
                })
            })
            .unwrap_or(false),
    };
    if status_ok {
        return Ok(());
    }
    let desc = value
        .get("desc")
        .and_then(Value::as_str)
        .filter(|text| !text.trim().is_empty())
        .unwrap_or("StepFun 返回失败状态");
    Err(desc.to_string())
}

fn stepfun_value_field<'a>(value: &'a Value, camel: &str, snake: &str) -> Option<&'a Value> {
    value.get(camel).or_else(|| value.get(snake))
}

#[derive(Clone, Copy, Debug)]
struct StepFunPlanUsageSnapshot {
    used_percent: f64,
    reset_at: Option<i64>,
}

fn stepfun_left_rate_to_used_percent(left_rate: f64) -> f64 {
    let fraction = if left_rate > 1.0 {
        left_rate / 100.0
    } else {
        left_rate
    };
    ((1.0 - fraction.clamp(0.0, 1.0)) * 100.0).clamp(0.0, 100.0)
}

fn stepfun_plan_rate_limit_response(
    value: &Value,
) -> Result<Option<StepFunPlanUsageSnapshot>, String> {
    stepfun_response_status(value)?;
    let Some(plan) = value
        .get("planCreditRateLimit")
        .or_else(|| value.get("plan_credit_rate_limit"))
    else {
        return Ok(None);
    };
    let left_rate = stepfun_value_field(
        plan,
        "subscriptionCreditLeftRate",
        "subscription_credit_left_rate",
    )
    .and_then(provider_number);
    let reset_at = stepfun_value_field(
        plan,
        "subscriptionCreditResetTime",
        "subscription_credit_reset_time",
    )
    .and_then(codex_timestamp);
    let bucket_rate = plan
        .get("creditBuckets")
        .or_else(|| plan.get("credit_buckets"))
        .and_then(Value::as_array)
        .map(|buckets| {
            buckets.iter().fold((0.0, 0.0), |(total, residual), bucket| {
                let bucket_total = stepfun_value_field(bucket, "creditTotal", "credit_total")
                    .and_then(provider_number)
                    .unwrap_or(0.0)
                    .max(0.0);
                let bucket_residual =
                    stepfun_value_field(bucket, "creditResidual", "credit_residual")
                        .and_then(provider_number)
                        .unwrap_or(0.0)
                        .clamp(0.0, bucket_total);
                (total + bucket_total, residual + bucket_residual)
            })
        })
        .and_then(|(total, residual)| (total > 0.0).then_some(residual / total));
    let used_percent = left_rate
        .or(bucket_rate)
        .map(stepfun_left_rate_to_used_percent);
    Ok(used_percent.map(|used_percent| StepFunPlanUsageSnapshot {
        used_percent,
        reset_at,
    }))
}

fn stepfun_credit_millions_text(value: f64) -> String {
    let millions = (value.max(0.0) / 1_000_000.0 * 100.0).round() / 100.0;
    if millions.fract() == 0.0 {
        format!("{millions:.0}M")
    } else {
        format!("{millions:.2}M")
    }
}

fn stepfun_record_is_today(
    from_time: Option<i64>,
    to_time: Option<i64>,
    now: chrono::DateTime<chrono::Utc>,
) -> bool {
    let Some(first) = from_time.or(to_time) else {
        return false;
    };
    let second = to_time.or(from_time).unwrap_or(first);
    let start = normalize_provider_timestamp(first);
    let end = normalize_provider_timestamp(second);
    if start <= 0 || end <= 0 {
        return false;
    }
    let (start, end) = (start.min(end), start.max(end));
    let timezone = chrono::FixedOffset::east_opt(8 * 60 * 60).expect("valid Shanghai offset");
    let local_date = now.with_timezone(&timezone).date_naive();
    let day_start = local_date
        .and_hms_opt(0, 0, 0)
        .map(|value| value.and_utc().timestamp() - timezone.local_minus_utc() as i64)
        .unwrap_or_else(|| now.timestamp());
    let day_end = day_start + 24 * 60 * 60;
    start < day_end && end >= day_start
}

fn stepfun_response_records(value: &Value) -> Result<(Vec<Value>, usize), String> {
    stepfun_response_status(value)?;
    let records = value
        .get("records")
        .and_then(Value::as_array)
        .cloned()
        .ok_or_else(|| "StepFun 响应缺少 records".to_string())?;
    let total = value
        .get("total")
        .and_then(provider_number)
        .unwrap_or(records.len() as f64)
        .max(0.0) as usize;
    Ok((records, total))
}

fn stepfun_usage_section_from_records_at(
    records: &[Value],
    _reported_total: usize,
    now: chrono::DateTime<chrono::Utc>,
    plan: Option<StepFunPlanUsageSnapshot>,
) -> ProviderUsageSection {
    #[derive(Default)]
    struct ModelUsage {
        calls: f64,
        credits: f64,
        from_time: Option<i64>,
        to_time: Option<i64>,
        model_type: Option<String>,
    }

    let mut models: BTreeMap<String, ModelUsage> = BTreeMap::new();
    let mut today_calls = 0.0;
    let mut today_credits = 0.0;
    for record in records {
        let model = record
            .get("modelId")
            .or_else(|| record.get("model_id"))
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("unknown")
            .to_string();
        let calls = record
            .get("calls")
            .and_then(provider_number)
            .unwrap_or(0.0)
            .max(0.0);
        let credits = record
            .get("creditConsumed")
            .or_else(|| record.get("credit_consumed"))
            .and_then(provider_number)
            .unwrap_or(0.0)
            .max(0.0);
        let from_time = record
            .get("fromTime")
            .or_else(|| record.get("from_time"))
            .and_then(provider_number)
            .map(|value| value as i64);
        let to_time = record
            .get("toTime")
            .or_else(|| record.get("to_time"))
            .and_then(provider_number)
            .map(|value| value as i64);
        let model_type = record
            .get("modelType")
            .or_else(|| record.get("model_type"))
            .and_then(|value| json_string_or_number(Some(value)));
        let entry = models.entry(model).or_default();
        entry.calls += calls;
        entry.credits += credits;
        entry.from_time = match (entry.from_time, from_time) {
            (Some(current), Some(next)) => Some(current.min(next)),
            (None, next) => next,
            (current, None) => current,
        };
        entry.to_time = match (entry.to_time, to_time) {
            (Some(current), Some(next)) => Some(current.max(next)),
            (None, next) => next,
            (current, None) => current,
        };
        if entry.model_type.is_none() {
            entry.model_type = model_type;
        }
        if stepfun_record_is_today(from_time, to_time, now) {
            today_calls += calls;
            today_credits += credits;
        }
    }

    let mut model_rows: Vec<(String, ModelUsage)> = models.into_iter().collect();
    model_rows.sort_by(|(left_label, left), (right_label, right)| {
        right
            .credits
            .total_cmp(&left.credits)
            .then_with(|| left_label.cmp(right_label))
    });
    let rows: Vec<ProviderUsageRow> = model_rows
        .into_iter()
        .map(|(model, usage)| ProviderUsageRow {
            label: model,
            hit_rate: Some(stepfun_record_time_label(usage.from_time, usage.to_time)),
            input: Some(fmt_provider_int(usage.calls)),
            output: Some(stepfun_credit_text(usage.credits)),
            cost_or_pct: usage.model_type,
        })
        .collect();

    let mut windows: Vec<ProviderUsageWindow> = plan
        .into_iter()
        .map(|snapshot| ProviderUsageWindow {
            window: "Plan 已用".into(),
            used: Some(format!("{:.0}%", snapshot.used_percent)),
            reset: None,
            reset_at: snapshot.reset_at,
        })
        .collect();
    windows.push(ProviderUsageWindow {
        window: "今日积分用量".into(),
        used: Some(format!("{today_credits} Credit")),
        reset: None,
        reset_at: Some(provider_daily_day_start(now.timestamp()) + 86400),
    });
    ProviderUsageSection {
        provider: "stepfun".into(),
        title: "StepFun Step Plan 用量".into(),
        description: format!(
            "今日 {} · 调用次数 {}",
            stepfun_credit_millions_text(today_credits),
            fmt_provider_int(today_calls),
        ),
        rows,
        windows,
        ..Default::default()
    }
}

async fn fetch_stepfun_usage(state: &AppState) -> ProviderUsageSection {
    let mut section = ProviderUsageSection {
        provider: "stepfun".into(),
        title: "StepFun Step Plan 用量".into(),
        ..Default::default()
    };
    let cookie = first_provider_env_value(&state.hermes_home, &["STEPFUN_COOKIE", "STEPFUN_WEB_COOKIE"]);
    if cookie.is_empty() {
        section.errors.push("缺少 STEPFUN_COOKIE（网页 Cookie）".into());
        return section;
    }
    let source_token = match stepfun_cookie_value(&cookie, "Oasis-Token") {
        Some(token) => token,
        None => {
            section.errors.push("Cookie 缺少 Oasis-Token".into());
            return section;
        }
    };
    let (start_time, to_time) = stepfun_current_month_range_millis();
    const PAGE_SIZE: usize = 100;
    const MAX_PAGES: usize = 100;
    let usage_body = |page: usize| {
        serde_json::json!({
            "startTime": start_time.to_string(),
            "toTime": to_time.to_string(),
            "page": page,
            "pageSize": PAGE_SIZE,
            "granularHour": 1,
        })
    };

    let rate_request = stepfun_post_json_with_refresh(
        state,
        &cookie,
        &source_token,
        STEPFUN_PLAN_RATE_LIMIT_URL,
        serde_json::json!({}),
    );
    let first_usage_request = stepfun_post_json_with_refresh(
        state,
        &cookie,
        &source_token,
        STEPFUN_PLAN_USAGE_URL,
        usage_body(1),
    );

    let (rate_result, first_usage_result) = tokio::join!(rate_request, first_usage_request);
    let plan = match rate_result {
        Ok(value) => match stepfun_plan_rate_limit_response(&value) {
            Ok(snapshot) => snapshot,
            Err(error) => {
                section.errors.push(format!("Step Plan 额度解析失败：{error}"));
                None
            }
        },
        Err(error) => {
            section.errors.push(format!("Step Plan 额度查询失败：{error}"));
            None
        }
    };

    let mut first_usage_result = Some(first_usage_result);
    let mut records = Vec::new();
    let mut reported_total = 0usize;
    for page in 1..=MAX_PAGES {
        let response_result = if page == 1 {
            first_usage_result
                .take()
                .expect("StepFun first usage response is available")
        } else {
            stepfun_post_json_with_refresh(
                state,
                &cookie,
                &source_token,
                STEPFUN_PLAN_USAGE_URL,
                usage_body(page),
            )
            .await
        };
        let response = match response_result {
            Ok(value) => value,
            Err(error) => {
                section.errors.push(format!("Step Plan 用量查询失败：{error}"));
                return section;
            }
        };
        let (page_records, total) = match stepfun_response_records(&response) {
            Ok(value) => value,
            Err(error) => {
                section.errors.push(format!("Step Plan 响应解析失败：{error}"));
                return section;
            }
        };
        reported_total = reported_total.max(total);
        let page_len = page_records.len();
        records.extend(page_records);
        if page_len == 0 || records.len() >= reported_total || page_len < PAGE_SIZE {
            break;
        }
    }
    let mut usage_section = stepfun_usage_section_from_records_at(
        &records,
        reported_total,
        chrono::Utc::now(),
        plan,
    );
    usage_section.errors = section.errors;
    usage_section
}
