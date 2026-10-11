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

#[derive(Clone)]
struct SenseNovaAuthState {
    source_access: String,
    source_refresh: String,
    access: String,
    refresh: String,
    expires_at: Option<i64>,
}

fn sensenova_access_expiry(access: &str) -> Option<i64> {
    let mut parts = access.split('.');
    parts.next()?;
    let payload = parts.next()?;
    parts.next()?;
    if parts.next().is_some() {
        return None;
    }
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(payload.trim_end_matches('='))
        .ok()?;
    serde_json::from_slice::<Value>(&bytes)
        .ok()?
        .get("exp")?
        .as_i64()
}

async fn sensenova_current_auth(state: &AppState) -> SenseNovaAuthState {
    let source_access = provider_env_value(&state.hermes_home, "SENSENOVA_ACCESS_TOKEN");
    let source_refresh = provider_env_value(&state.hermes_home, "SENSENOVA_REFRESH_TOKEN");
    let mut cached = state.provider_usage_cache.sensenova_auth.lock().await;
    if let Some(auth) = cached.as_ref()
        && auth.source_access == source_access
        && auth.source_refresh == source_refresh
    {
        return auth.clone();
    }
    let auth = SenseNovaAuthState {
        expires_at: sensenova_access_expiry(&source_access),
        access: source_access.clone(),
        refresh: source_refresh.clone(),
        source_access,
        source_refresh,
    };
    *cached = Some(auth.clone());
    auth
}

fn sensenova_persist_auth(
    home: &Path,
    prior: &SenseNovaAuthState,
    access: &str,
    refresh: &str,
) -> Result<(), String> {
    use std::io::Write as _;
    let path = home.join(".env");
    let path = path.canonicalize().unwrap_or(path);
    let raw = match std::fs::read_to_string(&path) {
        Ok(raw) => raw,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(_) => return Err("SenseNova 续期凭据读取失败".into()),
    };
    let updates = [
        ("SENSENOVA_ACCESS_TOKEN", &prior.access, access),
        ("SENSENOVA_REFRESH_TOKEN", &prior.refresh, refresh),
    ];
    for (key, expected, _) in updates {
        if let Some(value) = raw
            .lines()
            .filter(|line| !line.trim_start().starts_with('#'))
            .find_map(|line| {
                let (name, value) = line.split_once('=')?;
                (name.trim() == key).then(|| strip_env_quotes(value))
            })
            && !value.is_empty()
            && value != *expected
        {
            return Err("SenseNova 凭据已被更新，请重试".into());
        }
    }
    let mut seen = [false; 2];
    let mut text = String::new();
    for line in raw.split_inclusive('\n') {
        let key = (!line.trim_start().starts_with('#'))
            .then(|| line.split_once('=').map(|(key, _)| key.trim()))
            .flatten();
        if let Some(index) = updates.iter().position(|(name, _, _)| Some(*name) == key) {
            if !seen[index] {
                text.push_str(&format!("{}={}\n", updates[index].0, updates[index].2));
                seen[index] = true;
            }
        } else {
            text.push_str(line);
        }
    }
    if !text.is_empty() && !text.ends_with('\n') {
        text.push('\n');
    }
    for (index, (key, _, value)) in updates.iter().enumerate() {
        if !seen[index] {
            text.push_str(&format!("{key}={value}\n"));
        }
    }
    let save = || -> std::io::Result<()> {
        let mut file = tempfile::NamedTempFile::new_in(path.parent().unwrap_or(home))?;
        file.write_all(text.as_bytes())?;
        file.as_file().sync_all()?;
        file.persist(&path).map_err(|e| e.error)?;
        Ok(())
    };
    save().map_err(|_| "SenseNova 续期凭据保存失败".to_string())
}

async fn sensenova_refresh_auth(
    state: &AppState,
    usage_url: &str,
    prior: &SenseNovaAuthState,
) -> Result<SenseNovaAuthState, String> {
    if prior.refresh.is_empty() {
        return Err("SenseNova 登录已过期，缺少 SENSENOVA_REFRESH_TOKEN".into());
    }
    let mut url =
        reqwest::Url::parse(usage_url).map_err(|_| "SenseNova 续期地址无效".to_string())?;
    url.set_path("/oauth2/token");
    url.set_query(None);
    url.set_fragment(None);
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(PROVIDER_USAGE_TIMEOUT)
        .build()
        .map_err(|_| "SenseNova 续期请求初始化失败".to_string())?;
    let response = client
        .post(url)
        .form(&[
            ("grant_type", "refresh_token"),
            ("client_id", "nova"),
            ("refresh_token", prior.refresh.as_str()),
        ])
        .send()
        .await
        .map_err(|_| "SenseNova 登录续期请求失败".to_string())?;
    if !response.status().is_success() {
        return Err(format!(
            "SenseNova 登录续期返回 HTTP {}，请重新登录控制台",
            response.status().as_u16()
        ));
    }
    let value = response
        .json::<Value>()
        .await
        .map_err(|_| "SenseNova 续期响应无效".to_string())?;
    let token = |key: &str| {
        value
            .get(key)
            .and_then(Value::as_str)
            .filter(|v| !v.is_empty() && v.trim() == *v && !v.contains(['\r', '\n']))
    };
    let access = token("access_token")
        .ok_or_else(|| "SenseNova 续期响应缺少有效 access token".to_string())?;
    let refresh = if value.get("refresh_token").is_some() {
        token("refresh_token")
            .ok_or_else(|| "SenseNova 续期响应缺少有效 refresh token".to_string())?
    } else {
        &prior.refresh
    };
    sensenova_persist_auth(&state.hermes_home, prior, access, refresh)?;
    let expires_at = sensenova_access_expiry(access).or_else(|| {
        value
            .get("expires_in")
            .and_then(Value::as_i64)
            .filter(|seconds| *seconds > 0)
            .and_then(|seconds| chrono::Utc::now().timestamp().checked_add(seconds))
    });
    let auth = SenseNovaAuthState {
        expires_at,
        source_access: provider_env_value(&state.hermes_home, "SENSENOVA_ACCESS_TOKEN"),
        source_refresh: provider_env_value(&state.hermes_home, "SENSENOVA_REFRESH_TOKEN"),
        access: access.into(),
        refresh: refresh.into(),
    };
    *state.provider_usage_cache.sensenova_auth.lock().await = Some(auth.clone());
    Ok(auth)
}

async fn sensenova_pool_response(
    state: &AppState,
    url: &str,
    access: &str,
) -> Result<reqwest::Response, String> {
    state
        .client
        .get(url)
        .timeout(PROVIDER_USAGE_TIMEOUT)
        .header("Accept", "application/json")
        .header("Accept-Language", "zh-CN")
        .header("Referer", "https://platform.sensenova.cn/console")
        .bearer_auth(access)
        .send()
        .await
        .map_err(|_| "SenseNova 积分请求失败".to_string())
}

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
    let mut auth = sensenova_current_auth(state).await;
    if auth.access.is_empty() {
        failure.errors.push("缺少 SENSENOVA_ACCESS_TOKEN".into());
        return failure;
    }
    let proactive = !auth.refresh.is_empty()
        && auth
            .expires_at
            .is_some_and(|expires| expires <= chrono::Utc::now().timestamp() + 120);
    if proactive {
        auth = match sensenova_refresh_auth(state, url, &auth).await {
            Ok(auth) => auth,
            Err(error) => {
                failure.errors.push(error);
                return failure;
            }
        };
    }
    let response = match sensenova_pool_response(state, url, &auth.access).await {
        Ok(response) => response,
        Err(error) => {
            failure.errors.push(error);
            return failure;
        }
    };
    let response = if response.status() == reqwest::StatusCode::UNAUTHORIZED
        && !auth.refresh.is_empty()
        && !proactive
    {
        auth = match sensenova_refresh_auth(state, url, &auth).await {
            Ok(auth) => auth,
            Err(error) => {
                failure.errors.push(error);
                return failure;
            }
        };
        match sensenova_pool_response(state, url, &auth.access).await {
            Ok(response) => response,
            Err(error) => {
                failure.errors.push(error);
                return failure;
            }
        }
    } else {
        response
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
