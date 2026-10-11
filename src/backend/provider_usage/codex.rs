fn codex_backend_usage_url(base_url: &str) -> String {
    format!("{}/usage", codex_backend_prefix(base_url))
}

fn codex_backend_prefix(base_url: &str) -> String {
    let mut normalized = base_url.trim().trim_end_matches('/').to_string();
    if normalized.is_empty() {
        normalized = "https://chatgpt.com/backend-api/codex".into();
    }
    if normalized.ends_with("/codex") {
        normalized.truncate(normalized.len() - "/codex".len());
    }
    if normalized.contains("/backend-api") {
        format!("{normalized}/wham")
    } else {
        format!("{normalized}/api/codex")
    }
}

fn codex_backend_reset_credits_url(base_url: &str) -> String {
    format!(
        "{}/rate-limit-reset-credits",
        codex_backend_prefix(base_url)
    )
}

fn codex_usage_credit(account: &str, payload: &Value) -> Option<ProviderUsageCredit> {
    let balance = payload.pointer("/credits/balance").and_then(provider_number)
        .filter(|balance| balance.is_finite() && *balance > 0.0)?;
    Some(ProviderUsageCredit { account: account.to_string(), balance })
}

fn codex_reset_credits_description(value: &Value) -> Option<String> {
    let credits = value.get("credits").and_then(Value::as_array);
    let has_count = value.get("available_count").is_some();
    if credits.is_none() && !has_count {
        return None;
    }
    let count_from = |key: &str| {
        value
            .get(key)
            .and_then(provider_number)
            .map(|number| number.max(0.0).round() as i64)
    };
    let available = count_from("available_count").or_else(|| {
        credits.map(|items| {
            items
                .iter()
                .filter(|credit| credit.get("status").and_then(Value::as_str) == Some("available"))
                .count() as i64
        })
    })?;
    let mut expiry_timestamps = credits
        .into_iter()
        .flat_map(|items| items.iter())
        .filter(|credit| credit.get("status").and_then(Value::as_str) == Some("available"))
        .filter_map(|credit| credit.get("expires_at"))
        .filter_map(codex_timestamp)
        .filter(|timestamp| *timestamp > chrono::Utc::now().timestamp())
        .collect::<Vec<_>>();
    expiry_timestamps.sort_unstable();

    let mut parts = vec![format!("Reset：{available}个")];
    if let Some(applicable) = count_from("applicable_available_count") {
        parts.push(format!("当前可用：{applicable}个"));
    }
    if !expiry_timestamps.is_empty() {
        let expiries = expiry_timestamps
            .iter()
            .map(|timestamp| format!("{}后", provider_reset_text_local(*timestamp)))
            .collect::<Vec<_>>()
            .join("、");
        parts.push(format!("到期：{expiries}"));
    }
    Some(parts.join("；"))
}

fn should_skip_codex_cached_account(
    section: &ProviderUsageSection,
    label: &str,
    now: i64,
    force: bool,
) -> bool {
    !force && provider_cached_account_section(section, label, true, now).is_some()
}

fn jwt_chatgpt_account_id(token: &str) -> Option<String> {
    let parts: Vec<&str> = token.split('.').collect();
    let payload = parts.get(1)?;
    let decoded = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(payload)
        .ok()?;
    let claims: Value = serde_json::from_slice(&decoded).ok()?;
    claims
        .get("https://api.openai.com/auth")
        .and_then(|auth| auth.get("chatgpt_account_id"))
        .and_then(Value::as_str)
        .map(str::to_string)
}

fn codex_account_description(
    description: &str,
    label: &str,
    labels: &[String],
) -> Option<String> {
    let marker = format!("{label}：");
    let start = description.find(&marker)?;
    let tail = &description[start..];
    let end = labels
        .iter()
        .filter(|other| other.as_str() != label)
        .filter_map(|other| tail.find(&format!("；{other}：")))
        .min()
        .unwrap_or(tail.len());
    Some(tail[..end].trim_matches('；').to_string())
}

fn codex_cached_reset_description(
    cached_section: Option<&ProviderUsageSection>,
    label: &str,
    labels: &[String],
) -> Option<String> {
    cached_section.and_then(|section| codex_account_description(&section.description, label, labels))
}

async fn fetch_codex_usage(
    state: &AppState,
    cached_section: Option<&ProviderUsageSection>,
    force: bool,
) -> ProviderUsageSection {
    fetch_codex_usage_from_url(state, cached_section, force, "https://chatgpt.com/backend-api/codex").await
}

async fn fetch_codex_usage_from_url(
    state: &AppState,
    cached_section: Option<&ProviderUsageSection>,
    force: bool,
    base_url: &str,
) -> ProviderUsageSection {
    let mut section = ProviderUsageSection {
        provider: "codex".into(),
        title: "Codex 额度".into(),
        ..Default::default()
    };
    let accounts = auth_json_credential_pool(&state.hermes_home, "openai-codex");
    if accounts.is_empty() {
        section
            .errors
            .push("未找到 auth.json credential_pool/openai-codex".into());
        return section;
    }
    let now = chrono::Utc::now().timestamp();
    let labels = accounts.iter().map(|(label, _)| label.clone()).collect::<Vec<_>>();
    let mut skipped_accounts = Vec::new();
    let live_accounts = accounts
        .into_iter()
        .filter_map(|(label, token)| {
            if let Some(cached) = cached_section
                .filter(|section| should_skip_codex_cached_account(section, &label, now, force))
                .and_then(|section| provider_cached_account_section(section, &label, true, now))
            {
                skipped_accounts.push((label, cached));
                None
            } else {
                Some((label, token))
            }
        })
        .collect::<Vec<_>>();
    for (_, cached) in &skipped_accounts {
        section.windows.extend(cached.windows.clone());
        section.credits.extend(cached.credits.clone());
    }
    // Fanout only accounts that are not currently exhausted. Accounts at 100%
    // remain served from their cached windows until one of those windows resets.
    let mut fetched: Vec<(String, Result<Value, String>)> =
        futures_util::future::join_all(live_accounts.iter().map(|(label, token)| async move {
            jitter_delay().await;
            (label.clone(), codex_query_account(state, token, base_url).await)
        }))
        .await;
    fetched.sort_by(|a, b| a.0.to_lowercase().cmp(&b.0.to_lowercase()));
    let mut reset_descriptions = skipped_accounts
        .iter()
        .filter_map(|(label, cached)| {
            codex_account_description(&cached.description, label, &labels)
        })
        .collect::<Vec<_>>();

    for (label, result) in &fetched {
        match result {
            Ok(payload) => {
                if let Some(credit) = codex_usage_credit(label, payload) {
                    section.credits.push(credit);
                }
                for (key, fallback) in [
                    ("primary_window", "5h额度"),
                    ("secondary_window", "周额度"),
                ] {
                    let Some(window) = payload.pointer(&format!("/rate_limit/{key}")) else {
                        continue;
                    };
                    let Some(used) = window.get("used_percent").and_then(Value::as_f64) else {
                        continue;
                    };
                    let seconds = window
                        .get("limit_window_seconds")
                        .and_then(Value::as_i64)
                        .unwrap_or(0);
                    let window_label = match seconds {
                        604_800 => "周额度".to_string(),
                        2_592_000 | 2_628_000 | 2_629_800 | 2_630_000 => "月额度".to_string(),
                        _ => fallback.to_string(),
                    };
                    let reset_at = window.get("reset_at").and_then(codex_timestamp);
                    let reset = reset_at
                        .map(provider_reset_text_local)
                        .unwrap_or_else(|| "-".into());
                    section.windows.push(ProviderUsageWindow {
                        window: format!("{label} {window_label}"),
                        used: Some(format!("{used:.0}%")),
                        reset: Some(reset),
                        reset_at,
                    });
                }
                if let Some(reset_credits) = payload.get("_reset_credits") {
                    if let Some(error) = reset_credits.get("_error").and_then(Value::as_str) {
                        if let Some(cached_description) =
                            codex_cached_reset_description(cached_section, label, &labels)
                        {
                            reset_descriptions.push(cached_description);
                        } else {
                            section
                                .errors
                                .push(format!("{label}：Reset 查询失败：{error}"));
                        }
                    } else if let Some(description) = codex_reset_credits_description(reset_credits)
                    {
                        reset_descriptions.push(format!("{label}：{description}"));
                    }
                }
            }
            Err(err) => {
                if let Some(cached) = cached_section {
                    section.credits.extend(cached.credits.iter()
                        .filter(|credit| credit.account.as_str() == label.as_str()).cloned());
                }
                section.errors.push(format!("{label}：查询失败：{err}"));
            }
        }
    }
    section.description = reset_descriptions.join("；");
    section
}

async fn codex_query_account(state: &AppState, token: &str, base_url: &str) -> Result<Value, String> {
    let account_id = jwt_chatgpt_account_id(token);
    let mut headers = vec![
        ("Authorization".to_string(), format!("Bearer {token}")),
        ("Accept".to_string(), "application/json".into()),
        ("User-Agent".to_string(), "codex-cli".into()),
    ];
    if let Some(account_id) = &account_id {
        headers.push(("ChatGPT-Account-Id".to_string(), account_id.clone()));
    }
    let mut payload =
        provider_http_get_json(&state.client, &codex_backend_usage_url(base_url), &headers).await?;
    let reset_credits = match provider_http_get_json(
        &state.client,
        &codex_backend_reset_credits_url(base_url),
        &headers,
    )
    .await
    {
        Ok(value) => value,
        Err(error) => serde_json::json!({"_error": error}),
    };
    if let Some(object) = payload.as_object_mut() {
        object.insert("_reset_credits".to_string(), reset_credits);
    }
    Ok(payload)
}
