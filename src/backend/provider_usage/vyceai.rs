fn fmt_vyceai_cost(cost: f64) -> String {
    if cost > 0.0 && cost < 0.0001 {
        format!("${cost}")
    } else if cost < 1.0 && cost > 0.0 {
        format!("${cost:.4}")
    } else {
        fmt_provider_money(cost, '$')
    }
}

fn vyceai_usage_section(payload: &Value) -> ProviderUsageSection {
    let mut section = ProviderUsageSection {
        provider: "vyceai".into(),
        title: "Vyce AI 用量".into(),
        ..Default::default()
    };
    let stats = &payload["stats"];
    let number = |value: &Value| provider_number(value).filter(|value| *value >= 0.0);
    let (Some(balance), Some(spent), Some(calls), Some(models)) = (
        number(&stats["availableBalance"]),
        number(&stats["totalSpent"]),
        number(&stats["totalRequests"]),
        stats["modelUsage"].as_object(),
    ) else {
        section.errors.push("Vyce AI dashboard 缺少有效的账户用量字段".into());
        return section;
    };
    section.description = format!(
        "余额 **{}**；累计费用 {} · 调用次数 {calls:.0}（累计）",
        fmt_provider_money(balance, '$'),
        fmt_provider_money(spent, '$'),
    );
    let mut rows = Vec::new();
    for (model, usage) in models {
        let (Some(input), Some(output), Some(cost)) = (
            number(&usage["inputTokens"]),
            number(&usage["outputTokens"]),
            number(&usage["cost"]),
        ) else {
            if section.errors.is_empty() {
                section.errors.push("Vyce AI dashboard 存在无效的模型用量字段".into());
            }
            continue;
        };
        rows.push((cost, ProviderUsageRow {
            label: model.clone(),
            input: Some(fmt_provider_int(input)),
            output: Some(fmt_provider_int(output)),
            cost_or_pct: Some(fmt_vyceai_cost(cost)),
            ..Default::default()
        }));
    }
    rows.sort_by(|a, b| b.0.total_cmp(&a.0).then_with(|| a.1.label.cmp(&b.1.label)));
    section.rows = rows.into_iter().map(|(_, row)| row).collect();
    section
}

#[derive(Serialize, Deserialize, Clone, Default)]
struct VyceAiModelCounters {
    input_tokens: f64,
    output_tokens: f64,
    cost: f64,
}

#[derive(Serialize, Deserialize, Default)]
struct VyceAiDailySnapshot {
    account_id: String,
    day_start: i64,
    observed_since: i64,
    captured_at: i64,
    total_spent: f64,
    total_requests: f64,
    models: HashMap<String, VyceAiModelCounters>,
    today_spent: f64,
    today_requests: f64,
    today_models: HashMap<String, VyceAiModelCounters>,
}

fn vyceai_day_start(timestamp: i64) -> i64 {
    timestamp - (timestamp + 8 * 3600).rem_euclid(86400)
}

fn vyceai_positive_delta(high_water: &mut f64, current: f64) -> f64 {
    let delta = (current - *high_water).max(0.0);
    *high_water = high_water.max(current);
    delta
}

fn vyceai_daily_usage_section(state: &AppState, payload: &Value, now: i64) -> ProviderUsageSection {
    let mut section = vyceai_usage_section(payload);
    if !section.errors.is_empty() {
        section.rows.clear();
        section.description.clear();
        return section;
    }
    let Some(account_id) = payload["user"]["id"].as_str().filter(|id| !id.is_empty()) else {
        section.rows.clear();
        section.errors.push("Vyce AI dashboard 缺少账户标识，无法计算差值".into());
        return section;
    };
    let path = state.hermes_home.join("state/vyceai-usage-snapshot.json");
    let previous = match std::fs::read(&path) {
        Ok(raw) => match serde_json::from_slice::<VyceAiDailySnapshot>(&raw) {
            Ok(snapshot) => Some(snapshot),
            Err(_) => {
                section.rows.clear();
                section.errors.push("Vyce AI 用量快照无法解析".into());
                return section;
            }
        },
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => None,
        Err(_) => {
            section.rows.clear();
            section.errors.push("Vyce AI 用量快照读取失败".into());
            return section;
        }
    };
    let models = payload["stats"]["modelUsage"].as_object().expect("validated model counters")
        .iter().map(|(model, usage)| (model.clone(), VyceAiModelCounters {
            input_tokens: provider_number(&usage["inputTokens"]).expect("validated input"),
            output_tokens: provider_number(&usage["outputTokens"]).expect("validated output"),
            cost: provider_number(&usage["cost"]).expect("validated cost"),
        })).collect::<HashMap<_, _>>();
    let spent = provider_number(&payload["stats"]["totalSpent"]).expect("validated spent");
    let calls = provider_number(&payload["stats"]["totalRequests"]).expect("validated calls");
    let mut snapshot = if let Some(mut previous) = previous.filter(|snapshot| {
        snapshot.account_id == account_id && snapshot.day_start == vyceai_day_start(now)
    }) {
        if now >= previous.captured_at {
            previous.today_spent += vyceai_positive_delta(&mut previous.total_spent, spent);
            previous.today_requests += vyceai_positive_delta(&mut previous.total_requests, calls);
            for (model, current) in &models {
                let high_water = previous.models.entry(model.clone()).or_default();
                let today = previous.today_models.entry(model.clone()).or_default();
                today.input_tokens += vyceai_positive_delta(&mut high_water.input_tokens, current.input_tokens);
                today.output_tokens += vyceai_positive_delta(&mut high_water.output_tokens, current.output_tokens);
                today.cost += vyceai_positive_delta(&mut high_water.cost, current.cost);
            }
            previous.captured_at = now;
        }
        previous
    } else {
        VyceAiDailySnapshot {
            account_id: account_id.into(),
            day_start: vyceai_day_start(now),
            observed_since: now,
            captured_at: now,
            total_spent: spent,
            total_requests: calls,
            models,
            ..Default::default()
        }
    };
    // Only normalized counters and an opaque account ID reach this file.
    let persisted = (|| -> std::io::Result<()> {
        std::fs::create_dir_all(path.parent().expect("snapshot parent"))?;
        let raw = serde_json::to_vec(&snapshot)?;
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, raw)?;
        std::fs::rename(tmp, &path)
    })();
    if persisted.is_err() {
        section.rows.clear();
        section.errors.push("Vyce AI 用量快照保存失败".into());
        return section;
    }
    let input: f64 = snapshot.today_models.values().map(|model| model.input_tokens).sum();
    let output: f64 = snapshot.today_models.values().map(|model| model.output_tokens).sum();
    let usage = snapshot.today_models.drain().filter(|(_, model)| {
        model.input_tokens > 0.0 || model.output_tokens > 0.0 || model.cost > 0.0
    }).map(|(model, counters)| (model, serde_json::json!({
        "inputTokens": counters.input_tokens, "outputTokens": counters.output_tokens, "cost": counters.cost
    }))).collect::<serde_json::Map<_, _>>();
    section = vyceai_usage_section(&serde_json::json!({"stats": {
        "availableBalance": payload["stats"]["availableBalance"],
        "totalSpent": snapshot.today_spent, "totalRequests": snapshot.today_requests,
        "modelUsage": usage
    }}));
    let since = chrono::DateTime::from_timestamp(snapshot.observed_since, 0)
        .expect("valid capture timestamp")
        .with_timezone(&chrono::FixedOffset::east_opt(8 * 3600).expect("Shanghai offset"))
        .format("%H:%M");
    let balance = provider_number(&payload["stats"]["availableBalance"]).expect("validated balance");
    section.description = format!("余额 **{}**；今日调用次数 {:.0} · 采集自 {since}（差值）",
        fmt_provider_money(balance, '$'), snapshot.today_requests);
    section.captured_at = snapshot.captured_at as f64;
    section.windows.push(ProviderUsageWindow {
        window: "今日用量（差值）".into(),
        used: Some(format!("{} / {} / {}", fmt_provider_int(input), fmt_provider_int(output),
            if snapshot.today_spent > 0.0 && snapshot.today_spent < 0.01 {
                fmt_vyceai_cost(snapshot.today_spent)
            } else {
                fmt_provider_money(snapshot.today_spent, '$')
            })),
        reset: None,
        reset_at: Some(snapshot.day_start + 86400),
    });
    section
}

async fn fetch_vyceai_usage_from_url(state: &AppState, url: &str) -> ProviderUsageSection {
    let _guard = state.provider_usage_cache.vyceai_refresh.lock().await;
    let mut section = ProviderUsageSection {
        provider: "vyceai".into(),
        title: "Vyce AI 用量".into(),
        ..Default::default()
    };
    let cookie = provider_env_value(&state.hermes_home, "VYCEAI_COOKIE");
    let session = extract_cookie_value(&cookie, "session");
    if session.is_empty() {
        section.errors.push("VYCEAI_COOKIE 缺少 session".into());
        return section;
    }
    // Never retain or expose the response body: dashboard includes live API keys.
    let response = state.client.get(url)
        .timeout(PROVIDER_USAGE_TIMEOUT)
        .header(header::AUTHORIZATION, format!("Bearer {session}"))
        .header(header::ACCEPT, "application/json")
        .header(header::REFERER, "https://vyceai.com/dashboard-v2")
        .send().await;
    let response = match response {
        Ok(response) => response,
        Err(_) => {
            section.errors.push("Vyce AI dashboard 请求失败".into());
            return section;
        }
    };
    if !response.status().is_success() {
        section.errors.push(format!("Vyce AI dashboard 返回 HTTP {}", response.status().as_u16()));
        return section;
    }
    match response.json::<Value>().await {
        Ok(payload) => vyceai_daily_usage_section(state, &payload, chrono::Utc::now().timestamp()),
        Err(_) => {
            section.errors.push("Vyce AI dashboard 未返回有效 JSON".into());
            section
        }
    }
}

fn vyceai_next_snapshot_delay(now: i64) -> Duration {
    Duration::from_secs((300 - now.rem_euclid(300)) as u64)
}

async fn run_vyceai_snapshot_collector(state: Arc<AppState>) {
    loop {
        let cookie = provider_env_value(&state.hermes_home, "VYCEAI_COOKIE");
        if !extract_cookie_value(&cookie, "session").is_empty() {
            let section = fetch_vyceai_usage_from_url(&state, "https://vyceai.com/user/dashboard").await;
            if !section.errors.is_empty() {
                warn!("Vyce AI daily snapshot capture failed");
            }
        }
        tokio::time::sleep(vyceai_next_snapshot_delay(chrono::Utc::now().timestamp())).await;
    }
}

fn vyceai_cached_section_is_current(section: &ProviderUsageSection, now: i64) -> bool {
    if section.provider != "vyceai" {
        return provider_daily_cached_section_is_current(section, now);
    }
    let captured_at = section.captured_at as i64;
    captured_at > 0
        && captured_at <= now
        && now - captured_at < 300
        && vyceai_day_start(captured_at) == vyceai_day_start(now)
        && section.windows.iter().any(|window| window.window == "今日用量（差值）")
}
