const GROK_CLI_BASE_URL: &str = "https://cli-chat-proxy.grok.com/v1";
const XAI_OAUTH_TOKEN_URL: &str = "https://auth.x.ai/oauth2/token";
const XAI_OAUTH_CLIENT_ID: &str = "b1a00492-073a-47ea-816f-4c329264a828";

fn grok_cli_request(
    client: &reqwest::Client,
    url: &str,
    token: &str,
    user_id: Option<&str>,
) -> reqwest::RequestBuilder {
    let mut request = client
        .get(url)
        .bearer_auth(token)
        .header("X-XAI-Token-Auth", "xai-grok-cli")
        .header("Accept", "application/json")
        .header("x-grok-client-version", env!("CARGO_PKG_VERSION"))
        .header("x-grok-client-mode", "headless")
        .header("User-Agent", concat!("yahu/", env!("CARGO_PKG_VERSION")))
        .timeout(PROVIDER_USAGE_TIMEOUT);
    if let Some(user_id) = user_id {
        request = request.header("x-userid", user_id);
    }
    request
}

async fn grok_cli_json(request: reqwest::RequestBuilder) -> Result<Value, String> {
    let response = request
        .send()
        .await
        .map_err(|_| "Grok 额度接口请求失败".to_string())?;
    let status = response.status();
    if !status.is_success() {
        let detail = match status.as_u16() {
            401 => "额度查询认证失败",
            403 => "无额度查询权限",
            429 => "额度查询限流",
            502..=504 => "额度服务暂不可用",
            _ => "额度查询失败",
        };
        return Err(format!("HTTP {}：Grok {detail}", status.as_u16()));
    }
    response
        .json::<Value>()
        .await
        .map_err(|_| "Grok 额度接口返回无效 JSON".to_string())
}

async fn grok_query_billing_from_base(
    client: &reqwest::Client,
    base: &str,
    account: &AuthCredentialEntry,
) -> Result<Value, String> {
    let base = base.trim_end_matches('/');
    let user_id = match account
        .user_id
        .as_deref()
        .filter(|id| !id.trim().is_empty())
    {
        Some(user_id) => user_id.to_string(),
        None => {
            let profile = grok_cli_json(grok_cli_request(
                client,
                &format!("{base}/user"),
                &account.access_token,
                None,
            ))
            .await?;
            profile
                .get("userId")
                .and_then(Value::as_str)
                .filter(|id| !id.trim().is_empty())
                .ok_or_else(|| "Grok 用户信息缺少 userId".to_string())?
                .to_string()
        }
    };
    grok_cli_json(grok_cli_request(
        client,
        &format!("{base}/billing?format=credits"),
        &account.access_token,
        Some(&user_id),
    ))
    .await
}

async fn grok_refresh_access_token(
    client: &reqwest::Client,
    token_url: &str,
    refresh_token: &str,
) -> Result<(String, String), String> {
    let response = client
        .post(token_url)
        .timeout(PROVIDER_USAGE_TIMEOUT)
        .form(&[
            ("grant_type", "refresh_token"),
            ("client_id", XAI_OAUTH_CLIENT_ID),
            ("refresh_token", refresh_token),
        ])
        .send()
        .await
        .map_err(|err| err.to_string())?;
    let status = response.status();
    if !status.is_success() {
        return Err(format!("HTTP {}", status.as_u16()));
    }
    let payload: Value = response.json().await.map_err(|err| err.to_string())?;
    let access_token = payload
        .get("access_token")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|token| !token.is_empty())
        .ok_or_else(|| "刷新响应缺少 access_token".to_string())?;
    let next_refresh = payload
        .get("refresh_token")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|token| !token.is_empty())
        .unwrap_or(refresh_token);
    Ok((access_token.to_string(), next_refresh.to_string()))
}

async fn grok_query_account_from_base(
    client: &reqwest::Client,
    base: &str,
    token_url: &str,
    account: &AuthCredentialEntry,
) -> Result<Value, String> {
    match grok_query_billing_from_base(client, base, account).await {
        Err(error) if error.starts_with("HTTP 401：") => {
            let Some(refresh_token) = account.refresh_token.as_deref() else {
                return Err(error);
            };
            let (access_token, _) =
                grok_refresh_access_token(client, token_url, refresh_token).await?;
            let mut renewed = account.clone();
            renewed.access_token = access_token;
            grok_query_billing_from_base(client, base, &renewed).await
        }
        result => result,
    }
}

async fn grok_query_account(
    state: &AppState,
    account: &AuthCredentialEntry,
) -> Result<Value, String> {
    tokio::time::timeout(
        PROVIDER_USAGE_TIMEOUT,
        grok_query_account_from_base(
            &state.client,
            GROK_CLI_BASE_URL,
            XAI_OAUTH_TOKEN_URL,
            account,
        ),
    )
    .await
    .map_err(|_| "Grok 额度查询超时".to_string())?
}

fn grok_billing_snapshot(
    payload: &Value,
) -> Result<(String, f64, String, String, Option<i64>), String> {
    let config = payload.get("config").cloned().unwrap_or(Value::Null);
    let period = config.get("currentPeriod").cloned().unwrap_or(Value::Null);
    let period_type = period.get("type").and_then(Value::as_str).unwrap_or("");
    let reset_raw = period
        .get("end")
        .or_else(|| config.get("billingPeriodEnd"))
        .cloned()
        .unwrap_or(Value::Null);
    if period_type.is_empty() || reset_raw.is_null() {
        return Err("返回内容缺少可识别的额度周期".into());
    }
    let used_percent = config
        .get("creditUsagePercent")
        .or_else(|| config.get("credit_usage_percent"))
        .and_then(provider_number)
        .or_else(|| {
            config
                .get("productUsage")
                .and_then(Value::as_array)
                .and_then(|products| {
                    products
                        .iter()
                        .filter_map(|product| {
                            product
                                .get("usagePercent")
                                .or_else(|| product.get("usage_percent"))
                                .and_then(provider_number)
                        })
                        .max_by(f64::total_cmp)
                })
        })
        .ok_or_else(|| "返回内容未提供周额度使用比例".to_string())?
        .clamp(0.0, 100.0);
    let window = if period_type.contains("WEEKLY") {
        "周额度"
    } else if period_type.contains("MONTHLY") {
        "月额度"
    } else {
        "5h额度"
    }
    .to_string();
    let reset_at = codex_timestamp(&reset_raw);
    let parsed_reset: Option<chrono::DateTime<chrono::Utc>> =
        reset_at.and_then(|timestamp| chrono::DateTime::from_timestamp(timestamp, 0));
    let reset = parsed_reset
        .map(|dt| {
            let local = dt.with_timezone(&chrono::Local);
            format!(
                "{}/{} {}",
                local.month(),
                local.day(),
                local.format("%H:%M")
            )
        })
        .unwrap_or_else(|| "-".into());
    let tier = payload
        .get("subscriptionTier")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    Ok((window, used_percent, reset, tier, reset_at))
}

async fn fetch_grok_usage(
    state: &AppState,
    cached_section: Option<&ProviderUsageSection>,
    force: bool,
) -> ProviderUsageSection {
    let mut section = ProviderUsageSection {
        provider: "grok".into(),
        title: "Grok Build 额度".into(),
        ..Default::default()
    };
    let accounts = auth_json_credential_entries(&state.hermes_home, "xai-oauth");
    if accounts.is_empty() {
        section.errors.push("未找到 xai-oauth OAuth 会话".into());
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
                grok_query_account(state, account).await,
            )
        }))
        .await;
    fetched.sort_by(|a, b| a.0.to_lowercase().cmp(&b.0.to_lowercase()));
    section.description = cached_section
        .map(|cached| cached.description.clone())
        .unwrap_or_default();
    for (_, cached) in skipped_accounts {
        section.windows.extend(cached.windows);
    }
    for (label, result) in fetched {
        match result {
            Ok(payload) => match grok_billing_snapshot(&payload) {
                Ok((window, used_percent, reset, tier, reset_at)) => {
                    section.windows.push(ProviderUsageWindow {
                        window: if multi_account {
                            format!("{label} {window}")
                        } else {
                            window
                        },
                        used: Some(format!("{used_percent:.1}%")),
                        reset: Some(reset),
                        reset_at,
                    });
                    if section.description.is_empty() && !tier.is_empty() {
                        section.description = tier;
                    }
                }
                Err(err) => section.errors.push(format!("{label}：{err}")),
            },
            Err(err) => section.errors.push(format!("{label}：查询失败：{err}")),
        }
    }
    section
}
