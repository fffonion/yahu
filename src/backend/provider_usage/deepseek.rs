const DEEPSEEK_PLATFORM_BASE: &str = "https://platform.deepseek.com";
#[derive(Clone, Default)]
struct DeepseekUsageCounters {
    cache_hit: f64,
    cache_miss: f64,
    response: f64,
    cost: f64,
}

fn deepseek_rows(totals: &HashMap<String, DeepseekUsageCounters>) -> Vec<ProviderUsageRow> {
    ["deepseek-v4-pro", "deepseek-v4-flash"]
        .into_iter()
        .filter_map(|model| {
            let counters = totals.get(model).cloned().unwrap_or_default();
            let input = counters.cache_hit + counters.cache_miss;
            if input <= 0.0 && counters.response <= 0.0 && counters.cost <= 0.0 {
                return None;
            }
            Some(ProviderUsageRow {
                label: short_ds_model(model),
                hit_rate: deepseek_cache_hit_rate(counters.cache_hit, counters.cache_miss),
                input: Some(fmt_provider_int(input)),
                output: Some(fmt_provider_int(counters.response)),
                cost_or_pct: Some(fmt_provider_money(counters.cost, '¥')),
            })
        })
        .collect()
}

fn deepseek_cache_hit_rate(cache_hit: f64, cache_miss: f64) -> Option<String> {
    let total = cache_hit + cache_miss;
    if !(cache_hit.is_finite() && cache_miss.is_finite())
        || cache_hit < 0.0
        || cache_miss < 0.0
        || total <= 0.0
    {
        return None;
    }
    Some(format!("{:.1}%", cache_hit / total * 100.0))
}

fn short_ds_model(model: &str) -> String {
    model.rsplit('-').next().unwrap_or(model).to_string()
}

fn deepseek_unwrap_biz(value: &Value) -> Value {
    let data = value.get("data").unwrap_or(value);
    data.get("biz_data").cloned().unwrap_or_else(|| data.clone())
}

async fn fetch_deepseek_usage(state: &AppState) -> ProviderUsageSection {
    let mut section = ProviderUsageSection {
        provider: "deepseek".into(),
        title: "DeepSeek API 用量".into(),
        ..Default::default()
    };
    let cookie = provider_env_value(&state.hermes_home, "DEEPSEEK_COOKIE");
    let token = provider_env_value(&state.hermes_home, "DEEPSEEK_TOKEN");
    if cookie.is_empty() || token.is_empty() {
        section
            .errors
            .push("缺少 DEEPSEEK_COOKIE 或 DEEPSEEK_TOKEN".into());
        return section;
    }
    let headers = |extra: Vec<(String, String)>| {
        let mut all = vec![
            ("Cookie".to_string(), cookie.clone()),
            ("Authorization".to_string(), format!("Bearer {token}")),
            ("Accept".to_string(), "application/json".into()),
            (
                "User-Agent".to_string(),
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/148.0.0.0 Safari/537.36".into(),
            ),
        ];
        all.extend(extra);
        all
    };

    let now = chrono::Utc::now();
    let month = now.format("%-m").to_string();
    let year = now.format("%Y").to_string();
    let today = now.format("%Y-%m-%d").to_string();

    let get = |path: &'static str, params: String| -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Value, String>> + Send + '_>> {
        let url = format!("{DEEPSEEK_PLATFORM_BASE}{path}?{params}");
        let hs = headers(Vec::new());
        Box::pin(async move { provider_http_get_json(&state.client, &url, &hs).await })
    };

    let summary = get("/api/v0/users/get_user_summary", String::new()).await;
    let amount = get(
        "/api/v0/usage/amount",
        format!("month={month}&year={year}"),
    )
    .await;
    let cost = get(
        "/api/v0/usage/cost",
        format!("month={month}&year={year}"),
    )
    .await;

    // Balance from user_summary.normal_wallets[0].balance.
    let balance = summary
        .as_ref()
        .ok()
        .map(deepseek_unwrap_biz)
        .and_then(|biz| {
            biz.get("normal_wallets")
                .and_then(Value::as_array)
                .and_then(|wallets| wallets.first())
                .and_then(|wallet| wallet.get("balance"))
                .and_then(provider_number)
        });
    fn collect_counters(item: &Value) -> (String, DeepseekUsageCounters) {
        let model = item
            .get("model")
            .and_then(Value::as_str)
            .unwrap_or("?")
            .to_string();
        let mut counters = DeepseekUsageCounters::default();
        if let Some(list) = item.get("usage").and_then(Value::as_array) {
            for entry in list {
                let typ = entry.get("type").and_then(Value::as_str).unwrap_or("");
                let amount = entry.get("amount").and_then(provider_number).unwrap_or(0.0);
                match typ {
                    "PROMPT_CACHE_HIT_TOKEN" => counters.cache_hit += amount,
                    "PROMPT_CACHE_MISS_TOKEN" => counters.cache_miss += amount,
                    "RESPONSE_TOKEN" => counters.response += amount,
                    _ => {}
                }
            }
        }
        if let Some(list) = item.get("cost").and_then(Value::as_array) {
            for entry in list {
                let typ = entry.get("type").and_then(Value::as_str).unwrap_or("");
                if typ != "REQUEST" {
                    counters.cost += entry.get("amount").and_then(provider_number).unwrap_or(0.0);
                }
            }
        }
        (model, counters)
    }

    fn merge_maps(target: &mut HashMap<String, DeepseekUsageCounters>, source: Value, with_cost: bool) {
        let unwrapped = deepseek_unwrap_biz(&source);
        let items: Vec<Value> = if let Some(items) = unwrapped.get("total").and_then(Value::as_array) {
            items.clone()
        } else if unwrapped.is_array() {
            unwrapped.as_array().cloned().unwrap_or_default()
        } else {
            Vec::new()
        };
        for item in &items {
            let (model, mut counters) = collect_counters(item);
            if !with_cost {
                counters.cost = 0.0;
            }
            let entry = target.entry(model).or_insert(DeepseekUsageCounters {
                cache_hit: 0.0,
                cache_miss: 0.0,
                response: 0.0,
                cost: 0.0,
            });
            entry.cache_hit += counters.cache_hit;
            entry.cache_miss += counters.cache_miss;
            entry.response += counters.response;
            entry.cost += counters.cost;
        }
    }

    let mut month_totals: HashMap<String, DeepseekUsageCounters> = HashMap::new();
    let mut day_totals: HashMap<String, DeepseekUsageCounters> = HashMap::new();
    if let Ok(amount_value) = &amount {
        merge_maps(&mut month_totals, amount_value.clone(), false);
        let unwrapped = deepseek_unwrap_biz(amount_value);
        if let Some(days) = unwrapped.get("days").and_then(Value::as_array) {
            for day in days {
                if day.get("date").and_then(Value::as_str) == Some(today.as_str())
                    && let Some(items) = day.get("data").and_then(Value::as_array)
                {
                    for item in items {
                        let (model, counters) = collect_counters(item);
                        let entry = day_totals.entry(model).or_insert(DeepseekUsageCounters {
                            cache_hit: 0.0,
                            cache_miss: 0.0,
                            response: 0.0,
                            cost: 0.0,
                        });
                        entry.cache_hit += counters.cache_hit;
                        entry.cache_miss += counters.cache_miss;
                        entry.response += counters.response;
                    }
                }
            }
        }
    }
    // Costs live on the /usage/cost payload.
    if let Ok(cost_value) = &cost {
        let unwrapped = deepseek_unwrap_biz(cost_value);
        let total_items: Vec<Value> = if let Some(items) =
            unwrapped.get("total").and_then(Value::as_array)
        {
            items.clone()
        } else if let Some(first) = unwrapped.as_array().and_then(|list| list.first()) {
            first
                .get("total")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default()
        } else {
            Vec::new()
        };
        for item in &total_items {
            let model = item.get("model").and_then(Value::as_str).unwrap_or("?");
            let cost_total: f64 = item
                .get("usage")
                .and_then(Value::as_array)
                .map(|entries| {
                    entries
                        .iter()
                        .filter(|entry| entry.get("type").and_then(Value::as_str) != Some("REQUEST"))
                        .filter_map(|entry| entry.get("amount").and_then(provider_number))
                        .sum()
                })
                .unwrap_or(0.0);
            if let Some(entry) = month_totals.get_mut(model) {
                entry.cost = cost_total;
            }
        }
        let days: Vec<Value> = if let Some(days) = unwrapped.get("days").and_then(Value::as_array) {
            days.clone()
        } else if let Some(first) = unwrapped.as_array().and_then(|list| list.first()) {
            first.get("days").and_then(Value::as_array).cloned().unwrap_or_default()
        } else {
            Vec::new()
        };
        for day in &days {
            if day.get("date").and_then(Value::as_str) == Some(today.as_str())
                && let Some(items) = day.get("data").and_then(Value::as_array)
            {
                for item in items {
                    let model = item.get("model").and_then(Value::as_str).unwrap_or("?");
                    let cost_today: f64 = item
                        .get("usage")
                        .and_then(Value::as_array)
                        .map(|entries| {
                            entries
                                .iter()
                                .filter(|entry| {
                                    entry.get("type").and_then(Value::as_str) != Some("REQUEST")
                                })
                                .filter_map(|entry| entry.get("amount").and_then(provider_number))
                                .sum()
                        })
                        .unwrap_or(0.0);
                    if let Some(entry) = day_totals.get_mut(model) {
                        entry.cost = cost_today;
                    }
                }
            }
        }
    }

    section.rows = deepseek_rows(&month_totals);
    let day_input: f64 = day_totals.values().map(|c| c.cache_hit + c.cache_miss).sum();
    let day_output: f64 = day_totals.values().map(|c| c.response).sum();
    let day_cost: f64 = day_totals.values().map(|c| c.cost).sum();
    section.windows.push(ProviderUsageWindow {
        window: "今日用量/费用".into(),
        used: Some(format!("{} in / {} out / {}", fmt_provider_int(day_input), fmt_provider_int(day_output), fmt_provider_money(day_cost, '¥'))),
        reset: None,
        reset_at: None,
    });
    if let Some(balance) = balance {
        section
            .description
            .push_str(&format!("余额 **{}**", fmt_provider_money(balance, '¥')));
    }
    section
}
