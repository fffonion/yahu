const COMMANDCODE_API_BASE: &str = "https://api.commandcode.ai";
const COMMANDCODE_QUOTA_WALL_PERCENT: f64 = 99.0;
#[derive(Serialize, Deserialize, Clone, Default)]
struct CommandCodeAccountCache {
    #[serde(default)]
    token_fingerprint: String,
    #[serde(default)]
    whoami_cached: bool,
    #[serde(default)]
    org_id: Option<String>,
    #[serde(default)]
    current_period_start: Option<String>,
    #[serde(default)]
    current_period_end: Option<i64>,
    #[serde(default)]
    plan_id: Option<String>,
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    total_cost: Option<f64>,
}

type CommandCodeFetchedAccount = (String, Result<(Value, CommandCodeAccountCache), String>);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CommandCodeCachePlan {
    refresh_whoami: bool,
    refresh_subscription: bool,
    refresh_summary: bool,
}

const COMMANDCODE_PLAN_TOTAL_CREDITS: &[(&str, f64)] = &[
    ("individual-ultra", 300.0),
    ("individual-max", 150.0),
    ("individual-provider", 15.0),
    ("individual-pro-v1", 80.0),
    ("individual-pro", 30.0),
    ("individual-goat", 70.0),
    ("individual-go", 10.0),
    ("teams-pro", 40.0),
];

async fn commandcode_get(
    state: &AppState,
    path: &str,
    api_key: &str,
    params: &[(&str, String)],
) -> Result<Value, String> {
    let mut url = format!("{COMMANDCODE_API_BASE}{path}");
    if !params.is_empty() {
        let query = params
            .iter()
            .map(|(key, value)| format!("{key}={}", urlencoding_lite(value)))
            .collect::<Vec<_>>()
            .join("&");
        url.push('?');
        url.push_str(&query);
    }
    provider_http_get_json(
        &state.client,
        &url,
        &[
            ("Accept".to_string(), "application/json".into()),
            ("User-Agent".to_string(), "cli".into()),
            ("Authorization".to_string(), format!("Bearer {api_key}")),
        ],
    )
    .await
}

fn commandcode_plan_total(plan_id: &str) -> Option<f64> {
    let normalized = plan_id.trim().to_lowercase().replace('_', "-");
    COMMANDCODE_PLAN_TOTAL_CREDITS
        .iter()
        .find(|(prefix, _)| normalized.starts_with(prefix))
        .map(|(_, total)| *total)
}

fn commandcode_reset_text(snapshot: &Value, key: &str) -> String {
    let Some(value) = snapshot.get(key) else {
        return "-".into();
    };
    if let Some(timestamp) = codex_timestamp(value) {
        return provider_reset_text_local(timestamp);
    }
    "-".into()
}

fn commandcode_token_fingerprint(api_key: &str) -> String {
    let mut digest = sha2::Sha256::new();
    digest.update(api_key.as_bytes());
    format!("{:x}", digest.finalize())
}

fn commandcode_cache_plan(
    cached: Option<&CommandCodeAccountCache>,
    token_fingerprint: &str,
    now: i64,
) -> CommandCodeCachePlan {
    let same_token = cached.is_some_and(|cached| cached.token_fingerprint == token_fingerprint);
    let refresh_whoami = !same_token || !cached.is_some_and(|cached| cached.whoami_cached);
    let period_valid = same_token
        && cached
            .and_then(|cached| cached.current_period_end)
            .is_some_and(|reset_at| reset_at > now);
    let refresh_subscription = !period_valid;
    let refresh_summary = refresh_subscription
        || !same_token
        || cached.and_then(|cached| cached.total_cost).is_none();
    CommandCodeCachePlan {
        refresh_whoami,
        refresh_subscription,
        refresh_summary,
    }
}

async fn fetch_commandcode_usage(
    state: &AppState,
    cached_section: Option<&ProviderUsageSection>,
) -> ProviderUsageSection {
    let mut section = ProviderUsageSection {
        provider: "commandcode".into(),
        title: "CommandCode 额度".into(),
        ..Default::default()
    };
    let mut accounts = auth_json_credential_pool(&state.hermes_home, "commandcode");
    if accounts.is_empty() {
        let env_key = provider_env_value(&state.hermes_home, "COMMANDCODE_API_KEY");
        if !env_key.is_empty() {
            accounts.push(("环境变量".into(), env_key));
        }
    }
    if accounts.is_empty() {
        section
            .errors
            .push("未找到 auth.json credential_pool/commandcode 或 COMMANDCODE_API_KEY".into());
        return section;
    }

    let now = chrono::Utc::now().timestamp();
    let multi_account = accounts.len() > 1;
    let mut cached_account_metadata = read_shared_provider_cache(state).commandcode_accounts;
    let mut skipped_accounts = Vec::new();
    let live_accounts = accounts
        .into_iter()
        .filter_map(|(label, token)| {
            let account_cache = cached_account_metadata.get(&label).cloned();
            let token_fingerprint = commandcode_token_fingerprint(&token);
            let token_matches = account_cache
                .as_ref()
                .is_none_or(|cache| cache.token_fingerprint == token_fingerprint);
            if token_matches
                && let Some(cached) = cached_section.and_then(|section| {
                    provider_cached_account_section(section, &label, multi_account, now)
                })
            {
                if account_cache.is_none() {
                    cached_account_metadata.insert(
                        label.clone(),
                        CommandCodeAccountCache {
                            token_fingerprint,
                            ..Default::default()
                        },
                    );
                }
                skipped_accounts.push((label, cached));
                return None;
            }
            Some((label, token, account_cache))
        })
        .collect::<Vec<_>>();
    let mut fetched: Vec<CommandCodeFetchedAccount> =
        futures_util::future::join_all(
            live_accounts
                .iter()
                .map(|(label, token, account_cache)| async {
                    jitter_delay().await;
                    (
                        label.clone(),
                        commandcode_query_account(state, token, account_cache.as_ref(), now).await,
                    )
                }),
        )
        .await;
    fetched.sort_by(|a, b| a.0.to_lowercase().cmp(&b.0.to_lowercase()));
    let mut shared_cache = read_shared_provider_cache(state);
    shared_cache.commandcode_accounts = cached_account_metadata;
    for (label, result) in &fetched {
        if let Ok((_, account_cache)) = result {
            shared_cache
                .commandcode_accounts
                .insert(label.clone(), account_cache.clone());
        }
    }
    write_shared_provider_cache(state, &shared_cache);
    for (_, cached) in skipped_accounts {
        section.windows.extend(cached.windows);
    }
    let window_name = |label: &str, period: &str| {
        if multi_account {
            format!("{label} {period}")
        } else {
            period.to_string()
        }
    };

    for (label, result) in &fetched {
        match result {
            Ok((snapshot, _)) => {
                let five_hour = commandcode_window_text(snapshot, "five_hour");
                if five_hour != "-" {
                    section.windows.push(ProviderUsageWindow {
                        window: window_name(label, "5h额度"),
                        used: Some(five_hour),
                        reset: Some(commandcode_reset_text(snapshot, "five_hour_reset_at")),
                        reset_at: snapshot
                            .get("five_hour_reset_at")
                            .and_then(codex_timestamp),
                    });
                }
                let weekly = commandcode_window_text(snapshot, "weekly");
                if weekly != "-" {
                    section.windows.push(ProviderUsageWindow {
                        window: window_name(label, "周额度"),
                        used: Some(weekly),
                        reset: Some(commandcode_reset_text(snapshot, "weekly_reset_at")),
                        reset_at: snapshot.get("weekly_reset_at").and_then(codex_timestamp),
                    });
                }
                if let Some(monthly) = snapshot.get("usage_percent").and_then(Value::as_f64) {
                    section.windows.push(ProviderUsageWindow {
                        window: window_name(label, "月额度"),
                        used: Some(format!("{monthly:.0}%")),
                        reset: Some(commandcode_reset_text(snapshot, "current_period_end")),
                        reset_at: snapshot
                            .get("current_period_end")
                            .and_then(codex_timestamp),
                    });
                }
            }
            Err(err) => section
                .errors
                .push(format!("{label}：查询失败：{err}")),
        }
    }
    section
}

async fn commandcode_query_account(
    state: &AppState,
    api_key: &str,
    cached: Option<&CommandCodeAccountCache>,
    now: i64,
) -> Result<(Value, CommandCodeAccountCache), String> {
    let token_fingerprint = commandcode_token_fingerprint(api_key);
    let plan = commandcode_cache_plan(cached, &token_fingerprint, now);
    let cached = cached.cloned().unwrap_or_default();
    let (org_id, whoami_cached) = if plan.refresh_whoami {
        let whoami = commandcode_get(state, "/alpha/whoami", api_key, &[]).await?;
        (
            whoami
                .get("org")
                .and_then(|org| org.get("id"))
                .and_then(Value::as_str)
                .map(str::to_string),
            true,
        )
    } else {
        (cached.org_id.clone(), cached.whoami_cached)
    };
    let mut scoped: Vec<(&str, String)> = Vec::new();
    if let Some(org_id) = &org_id {
        scoped.push(("orgId", org_id.clone()));
    }
    let (credits, subscription_data) = if plan.refresh_subscription {
        let (credits_result, subscription_result) = tokio::join!(
            commandcode_get(state, "/alpha/billing/credits", api_key, &scoped),
            commandcode_get(state, "/alpha/billing/subscriptions", api_key, &scoped),
        );
        let credits = credits_result?;
        let subscription = subscription_result?;
        (
            credits,
            subscription.get("data").cloned().unwrap_or(Value::Null),
        )
    } else {
        let credits = commandcode_get(state, "/alpha/billing/credits", api_key, &scoped).await?;
        let subscription_data = serde_json::json!({
            "currentPeriodStart": cached.current_period_start,
            "currentPeriodEnd": cached.current_period_end,
            "planId": cached.plan_id,
            "status": cached.status,
        });
        (credits, subscription_data)
    };
    let since = subscription_data
        .get("currentPeriodStart")
        .and_then(Value::as_str)
        .map(str::to_string);
    if let Some(since) = since {
        scoped.push(("since", since));
    }
    let total_cost = if plan.refresh_summary {
        let summary = commandcode_get(state, "/alpha/usage/summary", api_key, &scoped).await?;
        let summary_obj = summary.get("data").cloned().unwrap_or(summary);
        summary_obj
            .get("totalCost")
            .and_then(Value::as_f64)
            .unwrap_or(0.0)
            .max(0.0)
    } else {
        cached.total_cost.unwrap_or(0.0)
    };

    let credits_obj = credits.get("credits").cloned().unwrap_or(Value::Null);
    let number = |value: &Value, key: &str| -> f64 {
        value.get(key).and_then(Value::as_f64).unwrap_or(0.0).max(0.0)
    };
    let monthly_remaining = number(&credits_obj, "monthlyCredits");
    let purchased_remaining = number(&credits_obj, "purchasedCredits");
    let free_remaining = number(&credits_obj, "freeCredits");
    let total_remaining = monthly_remaining + purchased_remaining + free_remaining;

    let plan_id = subscription_data
        .get("planId")
        .or_else(|| credits_obj.get("planId"))
        .and_then(Value::as_str)
        .unwrap_or("");
    let active = subscription_data.get("status").and_then(Value::as_str) == Some("active");
    let plan_total = active.then(|| commandcode_plan_total(plan_id)).flatten();

    let window_limits = credits.get("windowLimits").cloned().unwrap_or(Value::Null);
    let window_number = |window: &Value, key: &str| -> Option<f64> {
        window.get(key).and_then(Value::as_f64).map(|v| v.max(0.0))
    };
    let window_reset_at = |window: &Value| -> Option<i64> {
        let raw = window.get("resetAt")?;
        let value = match raw {
            Value::Number(_) => raw.as_f64()?,
            Value::String(text) => text.parse::<f64>().ok()?,
            _ => return None,
        };
        (value > 0.0).then_some(value as i64)
    };
    let five_hour = window_limits.get("fiveHour").cloned().unwrap_or(Value::Null);
    let weekly = window_limits.get("weekly").cloned().unwrap_or(Value::Null);

    let (total_pool, used_credits, usage_percent) = if let Some(plan_total) = plan_total {
        let pool = plan_total.max(monthly_remaining) + purchased_remaining + free_remaining;
        let used = (pool - total_remaining).max(0.0);
        let pct = (pool > 0.0).then(|| (used / pool * 100.0).clamp(0.0, 100.0));
        (pool, used, pct)
    } else {
        let pool = total_remaining + total_cost;
        let pct = (pool > 0.0).then(|| (total_cost / pool * 100.0).clamp(0.0, 100.0));
        (pool, total_cost, pct)
    };
    let current_period_end = subscription_data.get("currentPeriodEnd").cloned().unwrap_or(Value::Null);
    let account_cache = CommandCodeAccountCache {
        token_fingerprint,
        whoami_cached,
        org_id,
        current_period_start: subscription_data
            .get("currentPeriodStart")
            .and_then(Value::as_str)
            .map(str::to_string),
        current_period_end: codex_timestamp(&current_period_end),
        plan_id: (!plan_id.is_empty()).then(|| plan_id.to_string()),
        status: subscription_data
            .get("status")
            .and_then(Value::as_str)
            .map(str::to_string),
        total_cost: Some(total_cost),
    };

    Ok((
        serde_json::json!({
            "plan_total": plan_total,
            "total_pool": total_pool,
            "used_credits": used_credits,
            "usage_percent": usage_percent,
            "total_cost": total_cost,
            "weekly_used": window_number(&weekly, "used"),
            "weekly_cap": window_number(&weekly, "cap"),
            "weekly_reset_at": window_reset_at(&weekly),
            "five_hour_used": window_number(&five_hour, "used"),
            "five_hour_cap": window_number(&five_hour, "cap"),
            "five_hour_reset_at": window_reset_at(&five_hour),
            "current_period_end": current_period_end,
        }),
        account_cache,
    ))
}

fn commandcode_window_text(snapshot: &Value, window: &str) -> String {
    let used = snapshot.get(format!("{window}_used")).and_then(Value::as_f64);
    let cap = snapshot.get(format!("{window}_cap")).and_then(Value::as_f64);
    match (used, cap) {
        (Some(used), Some(cap)) if cap > 0.0 => {
            format!("{:.0}%", (used / cap * 100.0).clamp(0.0, 100.0))
        }
        _ => "-".into(),
    }
}
