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

fn load_sensenova_service_auth(home: &Path) -> Option<SenseNovaAuthState> {
    let value: Value = serde_json::from_slice(&std::fs::read(sensenova_service_token_path(home)).ok()?).ok()?;
    let username = provider_env_value(home, "SENSENOVA_USERNAME");
    let saved_username = value.get("username").and_then(Value::as_str).unwrap_or_default();
    if !saved_username.is_empty() && !username.is_empty() && saved_username != username {
        return None;
    }
    let access = value.get("access_token")?.as_str()?.to_string();
    let refresh = value.get("refresh_token")?.as_str()?.to_string();
    Some(SenseNovaAuthState {
        expires_at: value.get("expires_at").and_then(Value::as_i64).or_else(|| sensenova_access_expiry(&access)),
        source_access: access.clone(), source_refresh: refresh.clone(), access, refresh,
    })
}

fn sensenova_password_configured(home: &Path) -> bool {
    !provider_env_value(home, "SENSENOVA_USERNAME").is_empty()
        && !provider_env_value(home, "SENSENOVA_PASSWORD").is_empty()
}

async fn sensenova_current_auth(state: &AppState) -> SenseNovaAuthState {
    let auth = load_sensenova_service_auth(&state.hermes_home).unwrap_or(SenseNovaAuthState {
        source_access: String::new(), source_refresh: String::new(), access: String::new(),
        refresh: String::new(), expires_at: None,
    });
    let mut cached = state.provider_usage_cache.sensenova_auth.lock().await;
    if let Some(current) = cached.as_ref()
        && current.source_access == auth.source_access
        && current.source_refresh == auth.source_refresh
        && current.expires_at == auth.expires_at
    {
        return current.clone();
    }
    *cached = Some(auth.clone());
    auth
}

fn sensenova_service_token_path(home: &Path) -> PathBuf {
    home.join("state/sensenova_service_token.json")
}

fn sensenova_persist_auth(
    home: &Path,
    prior: &SenseNovaAuthState,
    access: &str,
    refresh: &str,
    expires_at: Option<i64>,
) -> Result<(), String> {
    let path = sensenova_service_token_path(home);
    if let Ok(raw) = std::fs::read(&path)
        && let Ok(value) = serde_json::from_slice::<Value>(&raw)
        && (value.get("access_token").and_then(Value::as_str).unwrap_or_default() != prior.access
            || value.get("refresh_token").and_then(Value::as_str).unwrap_or_default() != prior.refresh)
    {
        return Err("SenseNova 会话已被更新，请重试".into());
    }
    save_private_provider_session(&path, &serde_json::json!({
        "username": provider_env_value(home, "SENSENOVA_USERNAME"),
        "access_token": access,
        "refresh_token": refresh,
        "expires_at": expires_at,
    })).map_err(|_| "SenseNova 会话缓存保存失败".to_string())
}

#[derive(Debug)]
enum SenseNovaRefreshError {
    InvalidGrant,
    Other(String),
}

impl From<String> for SenseNovaRefreshError {
    fn from(value: String) -> Self { Self::Other(value) }
}

async fn sensenova_refresh_auth(
    state: &AppState,
    usage_url: &str,
    prior: &SenseNovaAuthState,
) -> Result<SenseNovaAuthState, SenseNovaRefreshError> {
    if prior.refresh.is_empty() {
        return Err(SenseNovaRefreshError::Other("SenseNova 会话缺少 refresh token".into()));
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
        let status = response.status();
        if matches!(status.as_u16(), 400 | 401)
            && response.json::<Value>().await.ok().and_then(|value| value.get("error").and_then(Value::as_str).map(str::to_string)).as_deref() == Some("invalid_grant")
        {
            return Err(SenseNovaRefreshError::InvalidGrant);
        }
        return Err(SenseNovaRefreshError::Other(format!("SenseNova 登录续期返回 HTTP {}", status.as_u16())));
    }
    let value = response
        .json::<Value>()
        .await
        .map_err(|_| "SenseNova 续期响应无效".to_string())?;
    sensenova_apply_auth(state, prior, &value).await.map_err(Into::into)
}

async fn sensenova_apply_auth(
    state: &AppState,
    prior: &SenseNovaAuthState,
    value: &Value,
) -> Result<SenseNovaAuthState, String> {
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
    let expires_at = sensenova_access_expiry(access).or_else(|| {
        value
            .get("expires_in")
            .and_then(Value::as_i64)
            .filter(|seconds| *seconds > 0)
            .and_then(|seconds| chrono::Utc::now().timestamp().checked_add(seconds))
    });
    sensenova_persist_auth(&state.hermes_home, prior, access, refresh, expires_at)?;
    let auth = SenseNovaAuthState {
        expires_at,
        source_access: access.into(),
        source_refresh: refresh.into(),
        access: access.into(),
        refresh: refresh.into(),
    };
    *state.provider_usage_cache.sensenova_auth.lock().await = Some(auth.clone());
    Ok(auth)
}

fn sensenova_login_endpoints(usage_url: &str) -> Result<Value, String> {
    let url = reqwest::Url::parse(usage_url).map_err(|_| "SenseNova 认证地址无效".to_string())?;
    let base = url.origin().ascii_serialization();
    if base == "https://platform.sensenova.cn" {
        return Ok(serde_json::json!({
            "oauth_base": base,
            "iam_base": "https://iam.sensecoreapi.cn/iam/authn",
            "jwks_url": "https://signin.sensecore.cn/.well-known/jwks.json",
        }));
    }
    if url.scheme() == "http" && matches!(url.host_str(), Some("127.0.0.1" | "localhost" | "::1")) {
        return Ok(serde_json::json!({
            "oauth_base": base,
            "iam_base": format!("{base}/iam/authn"),
            "jwks_url": format!("{base}/.well-known/jwks.json"),
        }));
    }
    Err("SenseNova 不支持此认证服务地址".into())
}

async fn sensenova_password_auth(
    state: &AppState,
    usage_url: &str,
    prior: &SenseNovaAuthState,
) -> Result<SenseNovaAuthState, String> {
    if !sensenova_password_configured(&state.hermes_home) {
        return Err("SenseNova 需要 SENSENOVA_USERNAME 和 SENSENOVA_PASSWORD".into());
    }
    let agent_dir = hermes_agent_dir(state);
    let endpoints = sensenova_login_endpoints(usage_url)?;
    let output = timeout(
        Duration::from_secs(90),
        Command::new(hermes_python_command(&agent_dir))
            .arg("-c")
            .arg(include_str!("sensenova_login.py"))
            .env("SENSENOVA_USERNAME", provider_env_value(&state.hermes_home, "SENSENOVA_USERNAME"))
            .env("SENSENOVA_PASSWORD", provider_env_value(&state.hermes_home, "SENSENOVA_PASSWORD"))
            .env("SENSENOVA_AUTH_ENDPOINTS", endpoints.to_string())
            .kill_on_drop(true)
            .output(),
    ).await.map_err(|_| "SenseNova 密码登录超时".to_string())?
        .map_err(|_| "SenseNova 密码登录组件启动失败".to_string())?;
    if !output.status.success() {
        return Err("SenseNova 密码登录失败或需要交互验证".into());
    }
    let value: Value = serde_json::from_slice(&output.stdout)
        .map_err(|_| "SenseNova 密码登录响应无效".to_string())?;
    if value.get("refresh_token").and_then(Value::as_str).is_none_or(str::is_empty) {
        return Err("SenseNova 密码登录未返回 refresh token".into());
    }
    sensenova_apply_auth(state, prior, &value).await
}

async fn sensenova_renew_auth(
    state: &AppState,
    usage_url: &str,
    prior: &SenseNovaAuthState,
) -> Result<SenseNovaAuthState, String> {
    if prior.refresh.is_empty()
        || sensenova_access_expiry(&prior.refresh).is_some_and(|expiry| expiry <= chrono::Utc::now().timestamp())
    {
        return sensenova_password_auth(state, usage_url, prior).await;
    }
    match sensenova_refresh_auth(state, usage_url, prior).await {
        Ok(auth) => Ok(auth),
        Err(SenseNovaRefreshError::InvalidGrant) => sensenova_password_auth(state, usage_url, prior).await,
        Err(SenseNovaRefreshError::Other(message)) => Err(message),
    }
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
        let renewed = sensenova_renew_auth(state, url, &auth).await;
        auth = match renewed {
            Ok(auth) => auth,
            Err(error) => { failure.errors.push(error); return failure; }
        };
    }
    let proactive = auth
            .expires_at
            .is_some_and(|expires| expires <= chrono::Utc::now().timestamp() + 120);
    if proactive {
        auth = match sensenova_renew_auth(state, url, &auth).await {
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
        && !proactive
    {
        auth = match sensenova_renew_auth(state, url, &auth).await {
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
