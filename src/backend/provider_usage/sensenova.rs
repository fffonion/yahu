#[derive(Serialize, Deserialize, Clone, Debug)]
struct SenseNovaQuota {
    window: String,
    limit: f64,
    used: f64,
    remaining: f64,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
struct SenseNovaPool {
    name: String,
    grant_balance: Option<f64>,
    nearest_grant_expiry: Option<i64>,
    nearest_grant_expiring_balance: Option<f64>,
    quotas: Vec<SenseNovaQuota>,
}

fn sensenova_nonnegative(value: Option<&Value>) -> Option<f64> {
    value
        .and_then(provider_number)
        .filter(|number| *number >= 0.0)
}

fn sensenova_timestamp(value: Option<&Value>) -> Option<i64> {
    let number = sensenova_nonnegative(value)?;
    if number <= 0.0 || number.fract() != 0.0 || number >= i64::MAX as f64 {
        return None;
    }
    let timestamp = number as i64;
    chrono::DateTime::from_timestamp(timestamp, 0).map(|_| timestamp)
}

fn sensenova_usage_section(value: &Value, now: i64) -> ProviderUsageSection {
    let mut section = ProviderUsageSection {
        provider: "sensenova".into(),
        title: "SenseNova Token Plan 用量".into(),
        ..Default::default()
    };
    let Some(pools) = value.get("pools").and_then(Value::as_array) else {
        section
            .errors
            .push("SenseNova 积分响应缺少有效的 pools".into());
        return section;
    };

    if pools.is_empty() {
        section
            .errors
            .push("SenseNova 当前没有可查询的 Token Plan 积分池".into());
    }
    for pool in pools {
        let Some(name) = pool
            .get("name")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|name| !name.is_empty())
        else {
            section.errors.push("SenseNova 积分池缺少名称".into());
            continue;
        };
        let mut summary = SenseNovaPool {
            name: name.into(),
            grant_balance: sensenova_nonnegative(pool.get("grant_balance")),
            nearest_grant_expiry: sensenova_timestamp(pool.get("nearest_grant_expiry")),
            nearest_grant_expiring_balance: sensenova_nonnegative(
                pool.get("nearest_grant_expiring_balance"),
            ),
            quotas: Vec::new(),
        };
        for (key, label) in [("window_5h", "5h额度"), ("window_7d", "周额度")] {
            let Some(window) = pool.get(key).filter(|value| !value.is_null()) else {
                continue;
            };
            let quota = (|| {
                let limit =
                    sensenova_nonnegative(window.get("limit")).filter(|limit| *limit > 0.0)?;
                let used = sensenova_nonnegative(window.get("used"))?;
                let remaining = sensenova_nonnegative(window.get("remaining"))?;
                Some(SenseNovaQuota {
                    window: label.into(),
                    limit,
                    used,
                    remaining,
                })
            })();
            let Some(quota) = quota else {
                section
                    .errors
                    .push("SenseNova 积分池存在无效的额度字段".into());
                continue;
            };
            let reset_at = sensenova_timestamp(window.get("reset_at"));
            section.windows.push(ProviderUsageWindow {
                window: format!("{name} {label}"),
                used: Some(format!(
                    "{:.2}%",
                    (quota.used / quota.limit * 100.0).clamp(0.0, 100.0)
                )),
                reset: reset_at
                    .map(|at| provider_reset_duration(at.saturating_sub(now).max(0) as f64)),
                reset_at,
            });
            summary.quotas.push(quota);
        }
        if summary.quotas.is_empty() {
            section
                .errors
                .push("SenseNova 积分池没有有效额度窗口".into());
        }
        section.pools.push(summary);
    }
    section
}

const SENSENOVA_POOL_USAGE_URL: &str =
    "https://platform.sensenova.cn/lite/console/v1/tokenplan/pool-usage";

async fn fetch_sensenova_usage_from_url(state: &AppState, url: &str) -> ProviderUsageSection {
    let _guard = state
        .provider_usage_cache
        .sensenova_daily_refresh
        .lock()
        .await;
    let mut failure = ProviderUsageSection {
        provider: "sensenova".into(),
        title: "SenseNova Token Plan 用量".into(),
        ..Default::default()
    };
    let token = provider_env_value(&state.hermes_home, "SENSENOVA_ACCESS_TOKEN");
    if token.is_empty() {
        failure.errors.push("缺少 SENSENOVA_ACCESS_TOKEN".into());
        return failure;
    }
    let response = match state
        .client
        .get(url)
        .timeout(PROVIDER_USAGE_TIMEOUT)
        .header("Accept", "application/json")
        .header("Accept-Language", "zh-CN")
        .header("Referer", "https://platform.sensenova.cn/console")
        .bearer_auth(token)
        .send()
        .await
    {
        Ok(response) => response,
        Err(_) => {
            failure.errors.push("SenseNova 积分请求失败".into());
            return failure;
        }
    };
    if !response.status().is_success() {
        failure.errors.push(format!(
            "SenseNova 积分查询返回 HTTP {}",
            response.status().as_u16()
        ));
        return failure;
    }
    match response.json::<Value>().await {
        Ok(value) => {
            let now = chrono::Utc::now().timestamp();
            let mut section = sensenova_usage_section(&value, now);
            if section.errors.is_empty() {
                let identity = value
                    .pointer("/plan/id")
                    .filter(|id| !id.is_null())
                    .map(Value::to_string)
                    .unwrap_or_default();
                let counters = section
                    .pools
                    .iter()
                    .filter_map(|pool| {
                        let quota = pool.quotas.iter().find(|q| q.window == "周额度")?;
                        let reset = section
                            .windows
                            .iter()
                            .find(|w| w.window == format!("{} 周额度", pool.name))?
                            .reset_at?;
                        Some((pool.name.clone(), (reset, quota.used)))
                    })
                    .collect::<HashMap<_, _>>();
                if counters.len() != section.pools.len() {
                    section
                        .errors
                        .push("今日积分差值缺少完整的周额度计数器".into());
                } else if let Err(err) =
                    provider_observed_daily(state, &mut section, &identity, "Credit", counters, now)
                {
                    section.errors.push(err);
                }
            }
            section
        }
        Err(_) => {
            failure
                .errors
                .push("SenseNova 积分响应未返回有效 JSON".into());
            failure
        }
    }
}

async fn fetch_sensenova_usage(state: &AppState) -> ProviderUsageSection {
    fetch_sensenova_usage_from_url(state, SENSENOVA_POOL_USAGE_URL).await
}
