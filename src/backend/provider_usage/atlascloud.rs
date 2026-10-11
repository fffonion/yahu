const ATLASCLOUD_BALANCE_URL: &str = "https://api.atlascloud.ai/public/v1/balance";
const ATLASCLOUD_MODEL_USAGE_URL: &str = "https://api.atlascloud.ai/public/v1/model-usage";
const ATLASCLOUD_MODEL_COSTS_URL: &str = "https://api.atlascloud.ai/public/v1/model-costs";
async fn fetch_atlascloud_usage(state: &AppState) -> ProviderUsageSection {
    let mut section = ProviderUsageSection {
        provider: "atlascloud".into(),
        title: "AtlasCloud 用量".into(),
        ..Default::default()
    };
    let api_key = ["ATLASCLOUD_API_KEY", "ATLAS_CLOUD_API_KEY"]
        .iter()
        .map(|key| provider_env_value(&state.hermes_home, key))
        .find(|value| !value.is_empty())
        .filter(|value| !value.is_empty())
        .or_else(|| {
            auth_json_credential_pool(&state.hermes_home, "custom:atlascloud")
                .first()
                .map(|(_, token)| token.clone())
                .filter(|token| !token.is_empty())
        })
        .filter(|value| !value.is_empty())
        .or_else(|| {
            let key = config_provider_api_key(&state.hermes_home, "atlascloud");
            (!key.is_empty()).then_some(key)
        });
    let Some(api_key) = api_key.filter(|value| !value.is_empty()) else {
        section.errors.push("缺少 ATLASCLOUD_API_KEY".into());
        return section;
    };

    let now = chrono::Utc::now();
    let today = now.format("%Y-%m-%d").to_string();
    let month_start = now.format("%Y-%m-01").to_string();
    let tomorrow = (now.date_naive() + chrono::Duration::days(1)).to_string();

    let headers = vec![
        ("Authorization".to_string(), format!("Bearer {api_key}")),
        ("User-Agent".to_string(), "curl/8.0".into()),
        ("Accept".to_string(), "application/json".into()),
    ];

    let balance = provider_http_get_json(&state.client, ATLASCLOUD_BALANCE_URL, &headers).await;
    if let Err(err) = &balance {
        section.errors.push(format!("余额查询失败：{err}"));
    }

    // Paginate through model-usage daily buckets grouped by model.
    let mut buckets: Vec<Value> = Vec::new();
    let mut page: Option<String> = None;
    loop {
        let mut query = format!(
            "start_date={month_start}&end_date={tomorrow}&scope=self&group_by[]=model&limit=1000"
        );
        if let Some(page_value) = &page {
            query.push_str(&format!("&page={page_value}"));
        }
        let payload = provider_http_get_json(
            &state.client,
            &format!("{ATLASCLOUD_MODEL_USAGE_URL}?{query}"),
            &headers,
        )
        .await;
        let payload = match payload {
            Ok(value) => value,
            Err(err) => {
                section.errors.push(format!("用量查询失败：{err}"));
                break;
            }
        };
        if let Some(items) = payload.get("data").and_then(Value::as_array) {
            buckets.extend(items.iter().cloned());
        }
        let has_more = payload.get("has_more").and_then(Value::as_bool).unwrap_or(false);
        page = payload
            .get("next_page")
            .and_then(|value| {
                value.as_str().map(str::to_string).or_else(|| {
                    value.as_i64().map(|number| number.to_string())
                })
            });
        if !has_more || page.is_none() || buckets.len() > 5000 {
            break;
        }
    }

    #[derive(Default)]
    struct ModelTokens {
        input: f64,
        output: f64,
        cache_creation: f64,
        cache_read: f64,
        requests: f64,
        images: f64,
        video_seconds: f64,
        cost: f64,
    }
    let mut month_map: BTreeMap<String, ModelTokens> = BTreeMap::new();
    let mut today_map: BTreeMap<String, ModelTokens> = BTreeMap::new();
    for bucket in &buckets {
        let date = bucket.get("date").and_then(Value::as_str).unwrap_or("");
        let destination = if date == today { &mut today_map } else { &mut month_map };
        for result in bucket
            .get("results")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let model = result
                .get("model")
                .map(|value| match value {
                    Value::Object(map) => map
                        .get("name")
                        .or_else(|| map.get("id"))
                        .and_then(Value::as_str)
                        .unwrap_or("?")
                        .to_string(),
                    Value::String(name) => name.clone(),
                    _ => "?".to_string(),
                });
            let Some(model) = model else { continue };
            let usage = result.get("usage").cloned().unwrap_or(Value::Null);
            let tokens = usage.get("tokens").cloned().unwrap_or(Value::Null);
            let read_number = |key: &str| {
                tokens
                    .get(key)
                    .and_then(Value::as_f64)
                    .unwrap_or(0.0)
            };
            let usage_number = |value: Option<&Value>| value.and_then(provider_number).unwrap_or(0.0);
            let entry = destination.entry(model).or_default();
            entry.requests += usage_number(usage.get("requests"));
            entry.images += usage_number(usage.get("images").and_then(|value| value.get("count")));
            entry.video_seconds += usage_number(usage.get("video").and_then(|value| value.get("seconds")));
            entry.input += read_number("input");
            entry.output += read_number("output");
            entry.cache_read += read_number("cache_read");
            let creation = read_number("cache_creation");
            let creation_1h = read_number("cache_creation_1h");
            entry.cache_creation += creation.max(creation_1h);
        }
    }

    let mut cost_map: BTreeMap<String, f64> = BTreeMap::new();
    let mut today_cost_map: BTreeMap<String, f64> = BTreeMap::new();
    let mut cost_page: Option<String> = None;
    loop {
        let mut query = format!(
            "start_date={month_start}&end_date={tomorrow}&scope=self&group_by[]=model&limit=1000"
        );
        if let Some(page_value) = &cost_page {
            query.push_str(&format!("&page={page_value}"));
        }
        let payload = match provider_http_get_json(
            &state.client,
            &format!("{ATLASCLOUD_MODEL_COSTS_URL}?{query}"),
            &headers,
        )
        .await
        {
            Ok(value) => value,
            Err(err) => {
                section.errors.push(format!("费用查询失败：{err}"));
                break;
            }
        };
        for bucket in payload.get("data").and_then(Value::as_array).into_iter().flatten() {
            for result in bucket.get("results").and_then(Value::as_array).into_iter().flatten() {
                let model = result
                    .get("model")
                    .and_then(|value| value.get("name").or_else(|| value.get("id")))
                    .and_then(Value::as_str);
                let Some(model) = model else { continue };
                let amount = result.pointer("/amount/value").and_then(provider_number).unwrap_or(0.0);
                let model_name = model.to_string();
                *cost_map.entry(model_name.clone()).or_default() += amount;
                if bucket.get("date").and_then(Value::as_str) == Some(today.as_str()) {
                    *today_cost_map.entry(model_name).or_default() += amount;
                }
            }
        }
        let has_more = payload.get("has_more").and_then(Value::as_bool).unwrap_or(false);
        cost_page = payload.get("next_page").and_then(|value| {
            value.as_str().map(str::to_string).or_else(|| value.as_i64().map(|number| number.to_string()))
        });
        if !has_more || cost_page.is_none() || cost_map.len() > 5000 {
            break;
        }
    }
    for (model, tokens) in month_map.iter_mut() {
        tokens.cost = cost_map.get(model).copied().unwrap_or(0.0);
    }
    for (model, tokens) in today_map.iter_mut() {
        tokens.cost = today_cost_map.get(model).copied().unwrap_or(0.0);
    }

    fn atlas_rows(values: &BTreeMap<String, ModelTokens>) -> Vec<ProviderUsageRow> {
        let mut entries: Vec<(&String, &ModelTokens)> = values.iter().collect();
        entries.sort_by_key(|(_, tokens)| std::cmp::Reverse((tokens.input + tokens.output) as i64));
        let rows_iter = entries.into_iter();
        rows_iter
            .map(|(model, tokens)| {
                let is_image = tokens.images > 0.0;
                let is_video = tokens.video_seconds > 0.0;
                ProviderUsageRow {
                label: model.rsplit('/').next().unwrap_or(model).to_string(),
                hit_rate: {
                    let context = tokens.input + tokens.cache_read + tokens.cache_creation;
                    (context > 0.0)
                        .then(|| format!("{:.1}%", tokens.cache_read / context * 100.0))
                },
                input: if is_image {
                    Some(format!("{} 张", fmt_provider_int(tokens.images)))
                } else if is_video {
                    Some(format!("{} 秒", fmt_provider_int(tokens.video_seconds)))
                } else {
                    Some(fmt_provider_int(tokens.input + tokens.cache_read + tokens.cache_creation))
                },
                output: if is_image || is_video {
                    Some(format!("{} 次", fmt_provider_int(tokens.requests)))
                } else {
                    Some(fmt_provider_int(tokens.output))
                },
                cost_or_pct: (tokens.cost > 0.0).then(|| fmt_provider_money(tokens.cost, '$')),
                }
            })
            .collect()
    }

    section.rows = atlas_rows(&month_map);
    let today_input: f64 = today_map.values().map(|tokens| tokens.input + tokens.cache_read + tokens.cache_creation).sum();
    let today_output: f64 = today_map.values().map(|tokens| tokens.output).sum();
    let today_cost: f64 = today_map.values().map(|tokens| tokens.cost).sum();
    section.windows.push(ProviderUsageWindow {
        window: "今日用量/费用".into(),
        used: Some(format!("{} in / {} out / {}", fmt_provider_int(today_input), fmt_provider_int(today_output), fmt_provider_money(today_cost, '$'))),
        reset: None,
        reset_at: None,
    });
    let available = balance.ok().and_then(|payload| {
        payload.get("available").and_then(|value| {
            value
                .get("value")
                .and_then(provider_number)
                .or_else(|| provider_number(value))
        })
    });
    if let Some(available) = available {
        section
            .description
            .push_str(&format!("余额 **{}**", fmt_provider_money(available, '$')));
    }
    section
}
