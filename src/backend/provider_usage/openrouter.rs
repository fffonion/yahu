const OPENROUTER_API_BASE: &str = "https://openrouter.ai/api/v1";
async fn fetch_openrouter_usage(state: &AppState) -> ProviderUsageSection {
    let mut section = ProviderUsageSection {
        provider: "openrouter".into(),
        title: "OpenRouter API 用量".into(),
        ..Default::default()
    };
    let key = provider_env_value(&state.hermes_home, "OPENROUTER_MANAGEMENT_KEY");
    if key.is_empty() {
        section.errors.push("缺少 OPENROUTER_MANAGEMENT_KEY".into());
        return section;
    }
    let headers = vec![
        ("Authorization".to_string(), format!("Bearer {key}")),
        ("Accept".to_string(), "application/json".into()),
    ];
    let activity = match provider_http_get_json(
        &state.client,
        &format!("{OPENROUTER_API_BASE}/activity"),
        &headers,
    )
    .await
    {
        Ok(value) => value,
        Err(err) => {
            section.errors.push(format!("activity 查询失败：{err}"));
            return section;
        }
    };
    let credits = provider_http_get_json(
        &state.client,
        &format!("{OPENROUTER_API_BASE}/credits"),
        &headers,
    )
    .await
    .ok();

    let month_prefix = {
        let now = chrono::Utc::now();
        now.format("%Y-%m-").to_string()
    };
    let items: Vec<Value> = activity
        .get("data")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter(|item| {
            item.get("date")
                .and_then(Value::as_str)
                .is_some_and(|date| date.starts_with(&month_prefix))
        })
        .collect();
    let latest_date = items
        .iter()
        .filter_map(|item| item.get("date").and_then(Value::as_str))
        .map(|date| date.chars().take(10).collect::<String>())
        .max()
        .unwrap_or_default();

    #[derive(Default)]
    struct Bucket {
        input: f64,
        output: f64,
        cost: f64,
    }
    let mut month_buckets: BTreeMap<String, Bucket> = BTreeMap::new();
    let mut day_buckets: BTreeMap<String, Bucket> = BTreeMap::new();
    let mut total_cost = 0.0;
    for item in &items {
        let model = item
            .get("model")
            .or_else(|| item.get("model_permaslug"))
            .and_then(Value::as_str)
            .unwrap_or("?")
            .to_string();
        let prompt = item
            .get("prompt_tokens")
            .and_then(Value::as_f64)
            .unwrap_or(0.0);
        let completion = item
            .get("completion_tokens")
            .and_then(Value::as_f64)
            .unwrap_or(0.0);
        let usage_cost = item.get("usage").and_then(Value::as_f64).unwrap_or(0.0);
        total_cost += usage_cost;
        let date = item
            .get("date")
            .and_then(Value::as_str)
            .map(|value| value.chars().take(10).collect::<String>())
            .unwrap_or_default();
        let bucket = month_buckets.entry(model.clone()).or_default();
        bucket.input += prompt;
        bucket.output += completion;
        bucket.cost += usage_cost;
        if !latest_date.is_empty() && date == latest_date {
            let bucket = day_buckets.entry(model).or_default();
            bucket.input += prompt;
            bucket.output += completion;
            bucket.cost += usage_cost;
        }
    }

    fn rows_from(buckets: &BTreeMap<String, Bucket>) -> Vec<ProviderUsageRow> {
        let mut rows: Vec<(String, &Bucket)> = buckets.iter().map(|(model, b)| (model.clone(), b)).collect();
        rows.sort_by(|a, b| b.1.cost.total_cmp(&a.1.cost));
        rows.into_iter()
            .take(4)
            .map(|(model, bucket)| ProviderUsageRow {
                label: model.rsplit('/').next().unwrap_or(&model).to_string(),
                hit_rate: None,
                input: Some(fmt_provider_int(bucket.input)),
                output: Some(fmt_provider_int(bucket.output)),
                cost_or_pct: Some(fmt_provider_money(bucket.cost, '$')),
            })
            .collect()
    }

    section.rows = rows_from(&month_buckets);
    section.description = format!("本月支出 **{}**", fmt_provider_money(total_cost, '$'));
    if let Some(balance) = credits.as_ref().and_then(|credits| {
        let total = credits.pointer("/data/total_credits").and_then(provider_number)?;
        let used = credits.pointer("/data/total_usage").and_then(provider_number)?;
        Some(total - used)
    }) {
        section.description = format!(
            "余额 **{}**；{}",
            fmt_provider_money(balance, '$'),
            section.description
        );
    }
    let day_cost: f64 = day_buckets.values().map(|b| b.cost).sum();
    section.windows.push(ProviderUsageWindow {
        window: "今日用量/费用".into(),
        used: Some(format!("{} in / {} out / {}", fmt_provider_int(day_buckets.values().map(|bucket| bucket.input).sum()), fmt_provider_int(day_buckets.values().map(|bucket| bucket.output).sum()), fmt_provider_money(day_cost, '$'))),
        reset: None,
        reset_at: None,
    });
    section
}
