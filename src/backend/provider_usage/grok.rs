const GROK_WEB_CREDITS_URL: &str =
    "https://grok.com/grok_api_v2.GrokBuildBilling/GetGrokCreditsConfig";
const XAI_OAUTH_TOKEN_URL: &str = "https://auth.x.ai/oauth2/token";
const XAI_OAUTH_CLIENT_ID: &str = "b1a00492-073a-47ea-816f-4c329264a828";
fn grok_proto_varint(data: &[u8], offset: &mut usize) -> Result<u64, String> {
    let mut value = 0u64;
    for shift in (0..70).step_by(7) {
        let byte = *data
            .get(*offset)
            .ok_or_else(|| "gRPC 响应的 varint 不完整".to_string())?;
        *offset += 1;
        value |= u64::from(byte & 0x7f) << shift;
        if byte & 0x80 == 0 {
            return Ok(value);
        }
    }
    Err("gRPC 响应的 varint 溢出".to_string())
}

fn grok_proto_field(data: &[u8], wanted: u32) -> Option<(u8, Vec<u8>)> {
    let mut offset = 0;
    while offset < data.len() {
        let tag = grok_proto_varint(data, &mut offset).ok()?;
        let field = u32::try_from(tag >> 3).ok()?;
        let wire = u8::try_from(tag & 7).ok()?;
        let value = match wire {
            0 => grok_proto_varint(data, &mut offset).ok()?.to_le_bytes().to_vec(),
            1 => {
                let end = offset.checked_add(8)?;
                let value = data.get(offset..end)?.to_vec();
                offset = end;
                value
            }
            2 => {
                let length = usize::try_from(grok_proto_varint(data, &mut offset).ok()?).ok()?;
                let end = offset.checked_add(length)?;
                let value = data.get(offset..end)?.to_vec();
                offset = end;
                value
            }
            5 => {
                let end = offset.checked_add(4)?;
                let value = data.get(offset..end)?.to_vec();
                offset = end;
                value
            }
            _ => return None,
        };
        if field == wanted {
            return Some((wire, value));
        }
    }
    None
}

fn grok_proto_timestamp(data: &[u8]) -> Option<chrono::DateTime<chrono::Utc>> {
    let (_, seconds) = grok_proto_field(data, 1)?;
    let seconds = i64::try_from(u64::from_le_bytes(seconds.try_into().ok()?)).ok()?;
    let nanos = grok_proto_field(data, 2)
        .and_then(|(_, value)| u64::from_le_bytes(value.try_into().ok()?).try_into().ok())
        .unwrap_or(0);
    chrono::DateTime::from_timestamp(seconds, nanos)
}

fn grok_web_billing_snapshot(body: &[u8]) -> Result<Value, String> {
    if body.len() < 5 || body[0] & 0x80 != 0 {
        return Err("gRPC 响应缺少有效的数据帧".to_string());
    }
    let length = u32::from_be_bytes(body[1..5].try_into().unwrap()) as usize;
    let end = 5usize
        .checked_add(length)
        .filter(|end| *end <= body.len())
        .ok_or_else(|| "gRPC 响应数据帧被截断".to_string())?;
    let credits = grok_proto_field(&body[5..end], 1)
        .filter(|(wire, _)| *wire == 2)
        .map(|(_, value)| value)
        .ok_or_else(|| "gRPC 响应缺少额度信息".to_string())?;
    let used_percent = grok_proto_field(&credits, 1)
        .filter(|(wire, value)| *wire == 5 && value.len() == 4)
        .map(|(_, value)| f32::from_le_bytes(value.try_into().unwrap()) as f64)
        .unwrap_or(0.0);
    if !used_percent.is_finite() || !(0.0..=100.0).contains(&used_percent) {
        return Err("gRPC 响应包含无效的额度比例".to_string());
    }
    let reset = grok_proto_field(&credits, 5)
        .filter(|(wire, _)| *wire == 2)
        .and_then(|(_, value)| grok_proto_timestamp(&value))
        .or_else(|| {
            grok_proto_field(&credits, 8)
                .filter(|(wire, _)| *wire == 2)
                .and_then(|(_, value)| {
                    grok_proto_field(&value, 3)
                        .filter(|(wire, _)| *wire == 2)
                        .and_then(|(_, timestamp)| grok_proto_timestamp(&timestamp))
                })
        })
        .ok_or_else(|| "gRPC 响应缺少额度重置时间".to_string())?;
    Ok(serde_json::json!({
        "config": {
            "currentPeriod": {
                "type": "USAGE_PERIOD_TYPE_WEEKLY",
                "end": reset.to_rfc3339(),
            },
            "creditUsagePercent": used_percent,
        }
    }))
}

async fn grok_query_web_billing(state: &AppState, token: &str) -> Result<Value, String> {
    let headers = vec![
        ("Authorization".to_string(), format!("Bearer {token}")),
        ("Accept".to_string(), "application/grpc-web+proto".to_string()),
        (
            "Content-Type".to_string(),
            "application/grpc-web+proto".to_string(),
        ),
        ("X-Grpc-Web".to_string(), "1".to_string()),
        (
            "Origin".to_string(),
            "https://grok.com".to_string(),
        ),
        (
            "Referer".to_string(),
            "https://grok.com/".to_string(),
        ),
        (
            "User-Agent".to_string(),
            "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/140.0.0.0 Safari/537.36".to_string(),
        ),
    ];
    let body = provider_http_post_bytes(
        &state.client,
        GROK_WEB_CREDITS_URL,
        &[0, 0, 0, 0, 0],
        &headers,
    )
    .await?;
    grok_web_billing_snapshot(&body)
}

async fn grok_refresh_access_token(
    state: &AppState,
    refresh_token: &str,
) -> Result<(String, String), String> {
    let response = state
        .client
        .post(XAI_OAUTH_TOKEN_URL)
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

async fn grok_query_account(
    state: &AppState,
    account: &AuthCredentialEntry,
) -> Result<Value, String> {
    let first = grok_query_web_billing(state, &account.access_token).await;
    match first {
        Ok(payload) => Ok(payload),
        Err(err) if err.starts_with("HTTP 401") => {
            let Some(refresh_token) = account.refresh_token.as_deref() else {
                return Err(err);
            };
            let (access_token, _next_refresh) =
                grok_refresh_access_token(state, refresh_token).await?;
            grok_query_web_billing(state, &access_token).await
        }
        Err(err) => Err(err),
    }
}

fn grok_billing_snapshot(
    payload: &Value,
) -> Result<(String, f64, String, String, Option<i64>), String> {
    let config = payload.get("config").cloned().unwrap_or(Value::Null);
    let period = config.get("currentPeriod").cloned().unwrap_or(Value::Null);
    let period_type = period
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or("");
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
            format!("{}/{} {}", local.month(), local.day(), local.format("%H:%M"))
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
    let mut fetched: Vec<(String, Result<Value, String>)> = futures_util::future::join_all(
        live_accounts.iter().map(|account| async {
            jitter_delay().await;
            (account.label.clone(), grok_query_account(state, account).await)
        }),
    )
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
                        window: if multi_account { format!("{label} {window}") } else { window },
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
