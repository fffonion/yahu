const ZED_CLOUD_ME_URL: &str = "https://cloud.zed.dev/client/users/me";
const ZED_BILLING_USAGE_URL: &str = "https://cloud.zed.dev/frontend/billing/usage";
fn zed_plan_label(raw: &str) -> String {
    match raw.trim().to_ascii_lowercase().as_str() {
        "" => String::new(),
        "zed_student" | "token_based_zed_student" => "Student".into(),
        "zed_pro" | "token_based_zed_pro" => "Pro".into(),
        "zed_pro_trial" | "token_based_zed_pro_trial" => "Pro Trial".into(),
        "zed_business" | "token_based_zed_business" => "Business".into(),
        "zed_free" | "token_based_zed_free" => "Free".into(),
        other => other
            .strip_prefix("zed_")
            .unwrap_or(other)
            .replace('_', " "),
    }
}

fn zed_usage_limit(value: Option<&Value>) -> Option<f64> {
    let value = value?;
    if value
        .as_str()
        .is_some_and(|text| text.eq_ignore_ascii_case("unlimited"))
    {
        return None;
    }
    if let Some(number) = provider_number(value) {
        return Some(number);
    }
    let object = value.as_object()?;
    if object.contains_key("unlimited") {
        return None;
    }
    object
        .get("limited")
        .or_else(|| object.get("limit"))
        .and_then(provider_number)
}

fn zed_iso_timestamp(value: Option<&Value>) -> Option<i64> {
    let raw = value
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|text| !text.is_empty())?;
    chrono::DateTime::parse_from_rfc3339(raw)
        .ok()
        .map(|stamp| stamp.timestamp())
}

fn zed_short_date(value: Option<&Value>) -> Option<String> {
    let stamp = zed_iso_timestamp(value)?;
    let when = chrono::DateTime::<chrono::Utc>::from_timestamp(stamp, 0)?;
    Some(format!("{}/{}", when.format("%m"), when.format("%d")))
}

fn zed_count_text(used: Option<f64>, limit: Option<f64>) -> Option<String> {
    let used = used?;
    Some(match limit {
        Some(limit) if limit > 0.0 => format!("{used:.0}/{limit:.0}"),
        Some(_) => format!("{used:.0}"),
        None => format!("{used:.0}/无限"),
    })
}

fn zed_plan_name(payload: &Value) -> String {
    let plan = payload.get("plan");
    let raw = plan
        .and_then(|value| value.get("plan_v3"))
        .and_then(Value::as_str)
        .or_else(|| {
            plan.and_then(|value| value.get("plan_v2"))
                .and_then(Value::as_str)
        })
        .or_else(|| {
            plan.and_then(|value| value.get("plan")).and_then(|value| {
                value
                    .as_str()
                    .or_else(|| value.get("plan").and_then(Value::as_str))
            })
        })
        .or_else(|| {
            payload
                .get("plans_by_organization")
                .and_then(Value::as_object)
                .and_then(|plans| plans.values().find_map(Value::as_str))
        })
        .unwrap_or("");
    zed_plan_label(raw)
}

fn zed_subscription_reset(payload: &Value) -> (Option<String>, Option<i64>) {
    let reset_at = payload
        .get("plan")
        .and_then(|plan| plan.get("subscription_period"))
        .and_then(|period| period.get("ended_at"))
        .and_then(|value| zed_iso_timestamp(Some(value)));
    let reset = reset_at.map(provider_reset_text_local);
    (reset, reset_at)
}

fn zed_account_snapshot(payload: &Value) -> Result<(String, Vec<ProviderUsageWindow>), String> {
    let plan = payload.get("plan").cloned().unwrap_or(Value::Null);
    let plan_name = zed_plan_name(payload);
    if plan_name.is_empty() && plan.is_null() {
        return Err("返回内容缺少套餐信息".into());
    }
    let period = plan
        .get("subscription_period")
        .cloned()
        .unwrap_or(Value::Null);
    let (reset, reset_at) = zed_subscription_reset(payload);
    let usage = plan.get("usage").cloned().unwrap_or(Value::Null);
    let model_requests = usage.get("model_requests").cloned().unwrap_or(Value::Null);
    let edit_predictions = usage
        .get("edit_predictions")
        .cloned()
        .unwrap_or(Value::Null);
    let model_used = model_requests.get("used").and_then(provider_number);
    let model_limit = zed_usage_limit(model_requests.get("limit"));
    let edit_used = edit_predictions.get("used").and_then(provider_number);
    let edit_limit = zed_usage_limit(edit_predictions.get("limit"));
    let mut details = Vec::new();
    if !plan_name.is_empty() {
        details.push(plan_name);
    }
    if let (Some(start), Some(end)) = (
        zed_short_date(period.get("started_at")),
        zed_short_date(period.get("ended_at")),
    ) {
        details.push(format!("账期 {start}–{end}"));
    }
    if let Some(text) = zed_count_text(model_used, model_limit) {
        details.push(format!("模型请求 {text}"));
    }
    if let Some(text) = zed_count_text(edit_used, edit_limit) {
        details.push(format!("补全 {text}"));
    }
    let mut windows = Vec::new();
    if let (Some(used), Some(limit)) = (model_used, model_limit)
        && limit > 0.0
    {
        let percent = used / limit * 100.0;
        if percent.is_finite() {
            windows.push(ProviderUsageWindow {
                window: "月额度".into(),
                used: Some(format!("{percent:.1}%")),
                reset,
                reset_at,
            });
        }
    }
    Ok((details.join(" · "), windows))
}

async fn zed_query_account(
    state: &AppState,
    account: &AuthCredentialEntry,
) -> Result<Value, String> {
    let user_id = account
        .user_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "凭据缺少 user_id".to_string())?;
    let headers = [
        (
            "Authorization".into(),
            format!("{user_id} {}", account.access_token),
        ),
        ("Accept".into(), "application/json".into()),
        ("Content-Type".into(), "application/json".into()),
        ("User-Agent".into(), "yahu/zed-pro".into()),
    ];
    provider_http_get_json(&state.client, ZED_CLOUD_ME_URL, &headers).await
}

fn zed_session_cookie_header(raw: &str) -> Option<String> {
    let value = raw
        .trim()
        .strip_prefix("Cookie:")
        .map(str::trim)
        .unwrap_or(raw.trim());
    if value.is_empty() {
        return None;
    }
    if value.starts_with("zed.session=") || value.contains(';') {
        Some(value.to_string())
    } else {
        Some(format!("zed.session={value}"))
    }
}

async fn zed_query_billing_usage(state: &AppState, raw_cookie: &str) -> Result<Value, String> {
    let cookie = zed_session_cookie_header(raw_cookie)
        .ok_or_else(|| "缺少 ZED_SESSION_COOKIE".to_string())?;
    let headers = [
        ("Accept".into(), "application/json".into()),
        ("Content-Type".into(), "application/json".into()),
        ("Origin".into(), "https://dashboard.zed.dev".into()),
        ("Referer".into(), "https://dashboard.zed.dev/".into()),
        ("User-Agent".into(), "yahu/zed-pro".into()),
        ("Cookie".into(), cookie),
    ];
    provider_http_get_json(&state.client, ZED_BILLING_USAGE_URL, &headers).await
}

fn zed_billing_snapshot(payload: &Value) -> Result<(String, Option<ProviderUsageWindow>), String> {
    let current_usage = payload
        .get("current_usage")
        .ok_or_else(|| "返回内容缺少 current_usage".to_string())?;
    let token_spend = current_usage.get("token_spend");
    let used = token_spend
        .and_then(|value| value.get("spend_in_cents"))
        .and_then(provider_number)
        .or_else(|| {
            current_usage
                .get("token_spend_in_cents")
                .and_then(provider_number)
        })
        .or_else(|| {
            payload
                .get("token_spend_in_cents")
                .and_then(provider_number)
        })
        .ok_or_else(|| "返回内容缺少 token spend".to_string())?;
    let limit = token_spend
        .and_then(|value| value.get("limit_in_cents"))
        .and_then(provider_number)
        .or_else(|| {
            current_usage
                .get("token_spend_limit_in_cents")
                .and_then(provider_number)
        })
        .or_else(|| payload.get("limit_in_cents").and_then(provider_number));
    let mut details = Vec::new();
    let spend_text = if let Some(limit) = limit.filter(|value| value.is_finite() && *value >= 0.0) {
        format!(
            "余额 **{}/{}**",
            fmt_provider_money(used / 100.0, '$'),
            fmt_provider_money(limit / 100.0, '$')
        )
    } else {
        format!("余额 **{}**", fmt_provider_money(used / 100.0, '$'))
    };
    details.push(spend_text);
    let window = limit
        .filter(|value| value.is_finite() && *value > 0.0)
        .and_then(|limit| {
            let percent = used / limit * 100.0;
            percent.is_finite().then(|| ProviderUsageWindow {
                window: "月额度".into(),
                used: Some(format!("{percent:.1}%")),
                reset: None,
                reset_at: None,
            })
        });
    Ok((details.join(" · "), window))
}

fn zed_merge_billing_description(existing: &str, billing: &str) -> String {
    billing
        .split(" · ")
        .chain(existing.split(" · "))
        .map(str::trim)
        .find(|part| part.starts_with("余额 **"))
        .unwrap_or_default()
        .to_string()
}

async fn fetch_zed_pro_usage(
    state: &AppState,
    cached_section: Option<&ProviderUsageSection>,
    force: bool,
) -> ProviderUsageSection {
    let mut section = ProviderUsageSection {
        provider: "zed-pro".into(),
        title: "Zed Pro 用量".into(),
        ..Default::default()
    };
    let _guard = state.provider_usage_cache.zed_daily_refresh.lock().await;
    let accounts = auth_json_credential_entries(&state.hermes_home, "zed-pro");
    let billing_identity = accounts
        .iter()
        .filter_map(|a| a.user_id.as_deref())
        .collect::<Vec<_>>()
        .join("|");
    let session_cookie = provider_env_value(&state.hermes_home, "ZED_SESSION_COOKIE");
    if accounts.is_empty() && session_cookie.is_empty() {
        section
            .errors
            .push("未找到 zed-pro 账号凭据或 ZED_SESSION_COOKIE".into());
        return section;
    }
    let now = chrono::Utc::now().timestamp();
    let multi_account = accounts.len() > 1;
    let mut skipped_accounts = Vec::new();
    let live_accounts = accounts
        .into_iter()
        .filter_map(|account| {
            if !force
                && let Some(cached) = cached_section.and_then(|section| {
                    provider_cached_account_section(section, &account.label, multi_account, now)
                })
            {
                skipped_accounts.push((account.label, cached));
                None
            } else {
                Some(account)
            }
        })
        .collect::<Vec<_>>();
    let mut fetched: Vec<(String, Result<Value, String>)> =
        futures_util::future::join_all(live_accounts.iter().map(|account| async {
            jitter_delay().await;
            (
                account.label.clone(),
                zed_query_account(state, account).await,
            )
        }))
        .await;
    fetched.sort_by(|a, b| a.0.to_lowercase().cmp(&b.0.to_lowercase()));
    section.description = cached_section
        .map(|cached| cached.description.clone())
        .unwrap_or_default();
    for (_, cached) in skipped_accounts {
        section.windows.extend(cached.windows);
        if section.description.is_empty() && !cached.description.is_empty() {
            section.description = cached.description;
        }
    }
    let mut subscription_reset = None;
    let mut subscription_reset_at = None;
    for (label, result) in fetched {
        match result {
            Ok(payload) => {
                let (account_reset, account_reset_at) = zed_subscription_reset(&payload);
                if subscription_reset.is_none() {
                    subscription_reset = account_reset;
                    subscription_reset_at = account_reset_at;
                }
                match zed_account_snapshot(&payload) {
                    Ok((description, windows)) => {
                        for window in windows {
                            section.windows.push(ProviderUsageWindow {
                                window: if multi_account {
                                    format!("{label} {}", window.window)
                                } else {
                                    window.window
                                },
                                used: window.used,
                                reset: window.reset,
                                reset_at: window.reset_at,
                            });
                        }
                        if section.description.is_empty() && !description.is_empty() {
                            section.description = if multi_account {
                                format!("{label} {description}")
                            } else {
                                description
                            };
                        }
                    }
                    Err(err) => section.errors.push(format!("{label}：{err}")),
                }
            }
            Err(err) => section.errors.push(format!("{label}：查询失败：{err}")),
        }
    }
    if session_cookie.is_empty() {
        section
            .errors
            .push("用量查询：缺少 ZED_SESSION_COOKIE".into());
    } else {
        match zed_query_billing_usage(state, &session_cookie).await {
            Ok(payload) => match zed_billing_snapshot(&payload) {
                Ok((description, window)) => {
                    section.windows.retain(|window| {
                        window.window != "Hosted AI 月额度" && window.window != "月额度"
                    });
                    if let Some(mut window) = window {
                        window.reset = subscription_reset.clone();
                        window.reset_at = subscription_reset_at;
                        section.windows.push(window);
                    }
                    section.description =
                        zed_merge_billing_description(&section.description, &description);
                    let spent = payload
                        .pointer("/current_usage/token_spend/spend_in_cents")
                        .or_else(|| payload.pointer("/current_usage/token_spend_in_cents"))
                        .or_else(|| payload.get("token_spend_in_cents"))
                        .and_then(provider_number);
                    if let Some(spent) = spent {
                        let epoch = subscription_reset_at
                            .or_else(|| section.windows.iter().find_map(|w| w.reset_at))
                            .unwrap_or(0);
                        if let Err(err) = provider_observed_daily(
                            state,
                            &mut section,
                            &billing_identity,
                            "USD",
                            HashMap::from([("billing".into(), (epoch, spent / 100.0))]),
                            now,
                        ) {
                            section.errors.push(err);
                        }
                    }
                }
                Err(err) => section.errors.push(format!("用量查询：{err}")),
            },
            Err(err) => section.errors.push(format!("用量查询：查询失败：{err}")),
        }
    }
    section.description = section
        .description
        .split(" · ")
        .find(|part| part.starts_with("余额 **"))
        .unwrap_or_default()
        .to_string();
    section
}
