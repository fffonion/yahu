// Provider usage panel: query external AI provider consoles for quota/usage
// data. Credentials are read from ~/.hermes/.env (and ~/.hermes/auth.json
// credential pools). Ported from ~/workspace/api-usage/token_usage.py.

const PROVIDER_USAGE_TTL: Duration = Duration::from_secs(5 * 60);
const PROVIDER_USAGE_TIMEOUT: Duration = Duration::from_secs(15);

const OPENROUTER_API_BASE: &str = "https://openrouter.ai/api/v1";
const DEEPSEEK_PLATFORM_BASE: &str = "https://platform.deepseek.com";
const ATLASCLOUD_BALANCE_URL: &str = "https://api.atlascloud.ai/public/v1/balance";
const ATLASCLOUD_MODEL_USAGE_URL: &str = "https://api.atlascloud.ai/public/v1/model-usage";
const MINIMAX_PLAN_REMAINS_URL: &str =
    "https://www.minimaxi.com/v1/api/openplatform/coding_plan/remains";
const KIMI_API_BASE: &str = "https://www.kimi.com";
const MIMO_API_BASE: &str = "https://platform.xiaomimimo.com/api/v1";
const COMMANDCODE_API_BASE: &str = "https://api.commandcode.ai";
const GROK_BILLING_URL: &str = "https://cli-chat-proxy.grok.com/v1/billing?format=credits";

#[derive(Serialize, Clone, Default)]
struct ProviderUsageRow {
    label: String,
    hit_rate: Option<String>,
    input: Option<String>,
    output: Option<String>,
    cost_or_pct: Option<String>,
}

#[derive(Serialize, Clone)]
struct ProviderUsageWindow {
    window: String,
    used: Option<String>,
    reset: Option<String>,
}

#[derive(Serialize, Clone, Default)]
struct ProviderUsageSection {
    provider: String,
    title: String,
    description: String,
    rows: Vec<ProviderUsageRow>,
    windows: Vec<ProviderUsageWindow>,
    errors: Vec<String>,
}

#[derive(Serialize, Clone, Default)]
struct ProviderUsagePayload {
    fetched_at: f64,
    sections: Vec<ProviderUsageSection>,
}

#[derive(Default)]
struct ProviderUsageCache {
    fetched_at: Arc<RwLock<Option<Instant>>>,
    payload: Arc<RwLock<Option<ProviderUsagePayload>>>,
    in_flight: Arc<Mutex<()>>,
}

fn provider_env_value(hermes_home: &Path, key: &str) -> String {
    if let Ok(value) = env::var(key)
        && !value.trim().is_empty()
    {
        return strip_env_quotes(value.trim());
    }
    let path = hermes_home.join(".env");
    let Ok(raw) = std::fs::read_to_string(path) else {
        return String::new();
    };
    for line in raw.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((k, v)) = line.split_once('=') else {
            continue;
        };
        if k.trim() == key {
            return strip_env_quotes(v.trim());
        }
    }
    String::new()
}

fn strip_env_quotes(value: &str) -> String {
    let trimmed = value.trim();
    // .env values may be wrapped in matching quotes (shell style or JSON style).
    if trimmed.len() >= 2
        && ((trimmed.starts_with('"') && trimmed.ends_with('"'))
            || (trimmed.starts_with('\'') && trimmed.ends_with('\'')))
    {
        return trimmed[1..trimmed.len() - 1].to_string();
    }
    trimmed.to_string()
}

/// Read a named custom provider's api_key from ~/.hermes/config.yaml.
fn config_provider_api_key(hermes_home: &Path, provider_name: &str) -> String {
    let path = hermes_home.join("config.yaml");
    let Ok(raw) = std::fs::read_to_string(path) else {
        return String::new();
    };
    let Ok(config) = serde_yaml::from_str::<serde_yaml::Value>(&raw) else {
        return String::new();
    };
    let Some(providers) = config
        .get("custom_providers")
        .and_then(|value| value.as_sequence())
    else {
        return String::new();
    };
    let entry = providers.iter().find(|entry| {
        entry
            .get("name")
            .and_then(|value| value.as_str())
            .is_some_and(|name| name.eq_ignore_ascii_case(provider_name))
    });
    let Some(entry) = entry else {
        return String::new();
    };
    entry
        .get("api_key")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|key| !key.is_empty())
        .unwrap_or_default()
        .to_string()
}

fn auth_json_credential_pool(
    hermes_home: &Path,
    provider: &str,
) -> Vec<(String, String)> {
    let path = hermes_home.join("auth.json");
    let Ok(raw) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    let Ok(value) = serde_json::from_str::<Value>(&raw) else {
        return Vec::new();
    };
    let Some(entries) = value
        .get("credential_pool")
        .and_then(|pool| pool.get(provider))
        .and_then(Value::as_array)
    else {
        return Vec::new();
    };
    entries
        .iter()
        .enumerate()
        .filter_map(|(index, entry)| {
            let token = entry
                .get("access_token")
                .or_else(|| entry.get("api_key"))
                .or_else(|| entry.get("token"))
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|token| !token.is_empty())?;
            let label = entry
                .get("label")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|label| !label.is_empty())
                .map(str::to_string)
                .unwrap_or_else(|| format!("账号{}", index + 1));
            Some((label, token.to_string()))
        })
        .collect()
}

async fn provider_http_get_json(
    client: &reqwest::Client,
    url: &str,
    headers: &[(String, String)],
) -> Result<Value, String> {
    let mut request = client.get(url).timeout(PROVIDER_USAGE_TIMEOUT);
    for (name, value) in headers {
        request = request.header(name.as_str(), value.as_str());
    }
    let response = request.send().await.map_err(|err| err.to_string())?;
    let status = response.status();
    let body = response.text().await.map_err(|err| err.to_string())?;
    if !status.is_success() {
        let snippet: String = body.chars().take(160).collect();
        return Err(format!("HTTP {status} {snippet}"));
    }
    serde_json::from_str::<Value>(&body).map_err(|err| format!("响应不是 JSON：{err}"))
}

async fn provider_http_post_json(
    client: &reqwest::Client,
    url: &str,
    body: Value,
    headers: &[(String, String)],
) -> Result<Value, String> {
    let mut request = client.post(url).json(&body).timeout(PROVIDER_USAGE_TIMEOUT);
    for (name, value) in headers {
        request = request.header(name.as_str(), value.as_str());
    }
    let response = request.send().await.map_err(|err| err.to_string())?;
    let status = response.status();
    let text = response.text().await.map_err(|err| err.to_string())?;
    if !status.is_success() {
        let snippet: String = text.chars().take(160).collect();
        return Err(format!("HTTP {status} {snippet}"));
    }
    serde_json::from_str::<Value>(&text).map_err(|err| format!("响应不是 JSON：{err}"))
}

fn fmt_provider_int(value: f64) -> String {
    if value >= 1_000_000.0 {
        format!("{:.2}m", value / 1_000_000.0)
    } else if value >= 10_000.0 {
        format!("{:.1}k", value / 1_000.0)
    } else {
        format!("{}", value.round() as i64)
    }
}

fn fmt_provider_money(value: f64, currency: char) -> String {
    format!("{currency}{:.2}", value)
}

fn provider_reset_text_local(value: i64) -> String {
    if value <= 0 {
        return "-".to_string();
    }
    let Some(dt) = chrono::DateTime::from_timestamp(value, 0) else {
        return "-".to_string();
    };
    let local = dt.with_timezone(&chrono::Local);
    format!("{}/{} {}", local.month(), local.day(), local.format("%H:%M"))
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
        let total = credits.pointer("/data/total_credits").and_then(Value::as_f64)?;
        let used = credits.pointer("/data/total_usage").and_then(Value::as_f64)?;
        Some(total - used)
    }) {
        section.description = format!(
            "余额 **{}**；{}",
            fmt_provider_money(balance, '$'),
            section.description
        );
    }
    if !day_buckets.is_empty() {
        section.windows.push(ProviderUsageWindow {
            window: "最新日".into(),
            used: Some(fmt_provider_int(
                day_buckets.values().map(|b| b.input + b.output).sum::<f64>(),
            )),
            reset: None,
        });
    }
    section
}

fn deepseek_unwrap_biz(value: &Value) -> Value {
    match value {
        Value::Object(map) => map.get("biz_data").cloned().unwrap_or_else(|| value.clone()),
        _ => value.clone(),
    }
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
                .and_then(Value::as_f64)
        });
    struct UsageCounters {
        cache_hit: f64,
        cache_miss: f64,
        response: f64,
        cost: f64,
    }
    fn collect_counters(item: &Value) -> (String, UsageCounters) {
        let model = item
            .get("model")
            .and_then(Value::as_str)
            .unwrap_or("?")
            .to_string();
        let mut counters = UsageCounters {
            cache_hit: 0.0,
            cache_miss: 0.0,
            response: 0.0,
            cost: 0.0,
        };
        if let Some(list) = item.get("usage").and_then(Value::as_array) {
            for entry in list {
                let typ = entry.get("type").and_then(Value::as_str).unwrap_or("");
                let amount = entry.get("amount").and_then(Value::as_f64).unwrap_or(0.0);
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
                    counters.cost += entry.get("amount").and_then(Value::as_f64).unwrap_or(0.0);
                }
            }
        }
        (model, counters)
    }

    fn merge_maps(target: &mut HashMap<String, UsageCounters>, source: Value, with_cost: bool) {
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
            let entry = target.entry(model).or_insert(UsageCounters {
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

    let mut month_totals: HashMap<String, UsageCounters> = HashMap::new();
    let mut day_totals: HashMap<String, UsageCounters> = HashMap::new();
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
                        let entry = day_totals.entry(model).or_insert(UsageCounters {
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
                        .filter_map(|entry| entry.get("amount").and_then(Value::as_f64))
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
                                .filter_map(|entry| entry.get("amount").and_then(Value::as_f64))
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

    fn ds_rows(totals: &HashMap<String, UsageCounters>) -> Vec<ProviderUsageRow> {
        let preferred = ["deepseek-v4-pro", "deepseek-v4-flash"];
        let mut ordered: Vec<String> = Vec::new();
        for name in preferred {
            if totals.keys().any(|model| model == name) {
                ordered.push(name.to_string());
            }
        }
        for model in totals.keys() {
            if !ordered.contains(model) {
                ordered.push(model.clone());
            }
        }
        ordered
            .into_iter()
            .filter_map(|model| {
                let counters = totals.get(&model)?;
                let input = counters.cache_hit + counters.cache_miss;
                Some(ProviderUsageRow {
                    label: short_ds_model(&model),
                    hit_rate: deepseek_cache_hit_rate(counters.cache_hit, counters.cache_miss),
                    input: Some(fmt_provider_int(input)),
                    output: Some(fmt_provider_int(counters.response)),
                    cost_or_pct: Some(fmt_provider_money(counters.cost, '¥')),
                })
            })
            .collect()
    }

    section.rows = ds_rows(&month_totals);
    if !day_totals.is_empty() {
        let day_input: f64 = day_totals.values().map(|c| c.cache_hit + c.cache_miss).sum();
        let day_output: f64 = day_totals.values().map(|c| c.response).sum();
        let day_cost: f64 = day_totals.values().map(|c| c.cost).sum();
        section.windows.push(ProviderUsageWindow {
            window: "今日".into(),
            used: Some(format!(
                "{} in / {} out / {}",
                fmt_provider_int(day_input),
                fmt_provider_int(day_output),
                fmt_provider_money(day_cost, '¥')
            )),
            reset: None,
        });
    }
    if let Some(balance) = balance {
        section
            .description
            .push_str(&format!("余额 **{}**", fmt_provider_money(balance, '¥')));
    }
    section
}

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
            let tokens = result.pointer("/usage/tokens").cloned().unwrap_or(Value::Null);
            let read_number = |key: &str| {
                tokens
                    .get(key)
                    .and_then(Value::as_f64)
                    .unwrap_or(0.0)
            };
            let entry = destination.entry(model).or_default();
            entry.input += read_number("input");
            entry.output += read_number("output");
            entry.cache_read += read_number("cache_read");
            let creation = read_number("cache_creation");
            let creation_1h = read_number("cache_creation_1h");
            entry.cache_creation += creation.max(creation_1h);
        }
    }

    fn atlas_rows(values: &BTreeMap<String, ModelTokens>) -> Vec<ProviderUsageRow> {
        let mut entries: Vec<(&String, &ModelTokens)> = values.iter().collect();
        entries.sort_by_key(|(_, tokens)| std::cmp::Reverse((tokens.input + tokens.output) as i64));
        let rows_iter = entries.into_iter();
        rows_iter
            .map(|(model, tokens)| ProviderUsageRow {
                label: model.rsplit('/').next().unwrap_or(model).to_string(),
                hit_rate: {
                    let context = tokens.input + tokens.cache_read + tokens.cache_creation;
                    (context > 0.0)
                        .then(|| format!("{:.1}%", tokens.cache_read / context * 100.0))
                },
                input: Some(fmt_provider_int(tokens.input + tokens.cache_read + tokens.cache_creation)),
                output: Some(fmt_provider_int(tokens.output)),
                cost_or_pct: None,
            })
            .collect()
    }

    section.rows = atlas_rows(&month_map);
    if !today_map.is_empty() {
        section.windows.push(ProviderUsageWindow {
            window: "今日".into(),
            used: Some(fmt_provider_int(
                today_map
                    .values()
                    .map(|tokens| tokens.input + tokens.output + tokens.cache_read + tokens.cache_creation)
                    .sum::<f64>(),
            )),
            reset: None,
        });
    }
    let available = balance.ok().and_then(|payload| {
        payload
            .get("available")
            .and_then(|value| {
                value
                    .get("value")
                    .and_then(Value::as_f64)
                    .or_else(|| value.as_f64())
            })
    });
    if let Some(available) = available {
        section
            .description
            .push_str(&format!("余额 **{}**", fmt_provider_money(available, '$')));
    }
    section
}

async fn fetch_minimax_usage(state: &AppState) -> ProviderUsageSection {
    let mut section = ProviderUsageSection {
        provider: "minimax".into(),
        title: "MiniMax 额度".into(),
        ..Default::default()
    };
    let cookie = provider_env_value(&state.hermes_home, "MINIMAX_COOKIE");
    let group_id = provider_env_value(&state.hermes_home, "MINIMAX_GROUP_ID");
    if cookie.is_empty() || group_id.is_empty() {
        section
            .errors
            .push("缺少 MINIMAX_COOKIE 或 MINIMAX_GROUP_ID".into());
        return section;
    }
    // .env stores the full cookie string ("_token=...; ..."); only prefix a
    // bare JWT.
    let cookie_header = if cookie.contains('=') {
        cookie
    } else {
        format!("_token={cookie}")
    };
    let headers = vec![
        ("Cookie".to_string(), cookie_header),
        ("x-group-id".to_string(), group_id),
        ("Accept".to_string(), "application/json".into()),
        (
            "User-Agent".to_string(),
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/148.0.0.0 Safari/537.36".into(),
        ),
        ("Referer".to_string(), "https://platform.minimaxi.com/".into()),
    ];
    let payload =
        match provider_http_get_json(&state.client, MINIMAX_PLAN_REMAINS_URL, &headers).await {
            Ok(value) => value,
            Err(err) => {
                section.errors.push(format!("查询失败：{err}"));
                return section;
            }
        };
    if payload.pointer("/base_resp/status_code").and_then(Value::as_i64) != Some(0) {
        let msg = payload
            .pointer("/base_resp/status_msg")
            .and_then(Value::as_str)
            .unwrap_or("未知错误");
        section.errors.push(format!("API 错误：{msg}"));
        return section;
    }
    for model in payload
        .get("model_remains")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
    {
        let name = model
            .get("model_name")
            .and_then(Value::as_str)
            .unwrap_or("?")
            .to_string();
        let interval_total = model
            .get("current_interval_total_count")
            .and_then(Value::as_f64)
            .unwrap_or(0.0);
        let interval_remaining = model
            .get("current_interval_usage_count")
            .and_then(Value::as_f64)
            .unwrap_or(0.0);
        let weekly_total = model
            .get("current_weekly_total_count")
            .and_then(Value::as_f64)
            .unwrap_or(0.0);
        let weekly_remaining = model
            .get("current_weekly_usage_count")
            .and_then(Value::as_f64)
            .unwrap_or(0.0);
        if interval_total > 0.0 {
            let used_pct = ((interval_total - interval_remaining) / interval_total * 100.0).max(0.0);
            section.windows.push(ProviderUsageWindow {
                window: format!("5h·{name}"),
                used: Some(format!("{used_pct:.1}%")),
                reset: None,
            });
        }
        if weekly_total > 0.0 {
            let used_pct = ((weekly_total - weekly_remaining) / weekly_total * 100.0).max(0.0);
            section.windows.push(ProviderUsageWindow {
                window: format!("周·{name}"),
                used: Some(format!("{used_pct:.1}%")),
                reset: None,
            });
        }
    }
    section
}

async fn fetch_kimi_usage(state: &AppState) -> ProviderUsageSection {
    let mut section = ProviderUsageSection {
        provider: "kimi".into(),
        title: "Kimi Code 额度".into(),
        ..Default::default()
    };
    let token = provider_env_value(&state.hermes_home, "KIMI_AUTH_TOKEN");
    if token.is_empty() {
        section.errors.push("缺少 KIMI_AUTH_TOKEN".into());
        return section;
    }
    let headers = vec![
        ("authorization".to_string(), format!("Bearer {token}")),
        ("content-type".to_string(), "application/json".into()),
        ("origin".to_string(), "https://www.kimi.com".into()),
        (
            "referer".to_string(),
            "https://www.kimi.com/code/console".into(),
        ),
        ("r-timezone".to_string(), "Asia/Shanghai".into()),
        ("x-msh-platform".to_string(), "web".into()),
    ];
    let data = match provider_http_post_json(
        &state.client,
        &format!("{KIMI_API_BASE}/apiv2/kimi.gateway.membership.v2.MembershipService/GetSubscriptionStats"),
        serde_json::json!({}),
        &headers,
    )
    .await
    {
        Ok(value) => value,
        Err(err) => {
            section.errors.push(format!("查询失败：{err}"));
            return section;
        }
    };
    let ratio_pct = |value: Option<f64>| -> String {
        match value {
            Some(ratio) if (0.0..=1.0).contains(&ratio) => format!("{:.0}%", ratio * 100.0),
            Some(ratio) => format!("{ratio:.0}%"),
            None => "-".into(),
        }
    };
    if let Some(five_h) = payload_enabled(data.get("ratelimitCode5h")) {
        section.windows.push(ProviderUsageWindow {
            window: "5h".into(),
            used: Some(ratio_pct(num_field(five_h, "ratio"))),
            reset: reset_from_ms(five_h, "resetTime"),
        });
    }
    if let Some(weekly) = payload_enabled(data.get("ratelimitCode7d")) {
        section.windows.push(ProviderUsageWindow {
            window: "Week".into(),
            used: Some(ratio_pct(num_field(weekly, "ratio"))),
            reset: reset_from_ms(weekly, "resetTime"),
        });
    }
    if let Some(balance) = data.get("subscriptionBalance").filter(|v| v.is_object()) {
        section.windows.push(ProviderUsageWindow {
            window: "Month".into(),
            used: Some(ratio_pct(num_field(balance, "amountUsedRatio"))),
            reset: reset_from_ms(balance, "expireTime"),
        });
    }
    section
}

fn payload_enabled(value: Option<&Value>) -> Option<&Value> {
    let value = value?;
    value.get("enabled").and_then(Value::as_bool)?.then_some(value)
}

fn num_field(value: &Value, key: &str) -> Option<f64> {
    value.get(key).and_then(Value::as_f64)
}

fn kimi_time_text(value: Option<&Value>) -> String {
    let ms = value.and_then(Value::as_f64).unwrap_or(0.0);
    if ms <= 0.0 {
        return "-".into();
    }
    provider_reset_text_local((ms / 1000.0) as i64)
}

fn reset_from_ms(value: &Value, key: &str) -> Option<String> {
    Some(kimi_time_text(value.get(key)))
}

fn provider_reset_text_seconds(seconds: f64) -> String {
    let now = chrono::Local::now();
    let reset = now + chrono::Duration::seconds(seconds.max(0.0) as i64);
    format!("{}/{} {}", reset.month(), reset.day(), reset.format("%H:%M"))
}

async fn fetch_mimo_usage(state: &AppState) -> ProviderUsageSection {
    let mut section = ProviderUsageSection {
        provider: "mimo".into(),
        title: "MiMo 额度".into(),
        ..Default::default()
    };
    let cookie = provider_env_value(&state.hermes_home, "MIMO_COOKIE");
    if cookie.is_empty() {
        section.errors.push("缺少 MIMO_COOKIE".into());
        return section;
    }
    let headers = |referer: &str| {
        vec![
            ("Cookie".to_string(), cookie.clone()),
            ("accept".to_string(), "application/json".into()),
            (
                "user-agent".to_string(),
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/148.0.0.0 Safari/537.36".into(),
            ),
            ("origin".to_string(), "https://platform.xiaomimimo.com".into()),
            ("referer".to_string(), referer.into()),
            ("x-timezone".to_string(), "Asia/Shanghai".into()),
        ]
    };

    let detail = provider_http_get_json(
        &state.client,
        &format!("{MIMO_API_BASE}/tokenPlan/detail"),
        &headers("https://platform.xiaomimimo.com/console/plan-manage"),
    )
    .await;
    let detail = match detail {
        Ok(value) => {
            let code = value.get("code").and_then(Value::as_i64).unwrap_or(-1);
            if !(code == 0 || code == 200) {
                section
                    .errors
                    .push("cookie 已过期，请重新获取 MIMO_COOKIE".into());
                return section;
            }
            value.get("data").cloned().unwrap_or(Value::Null)
        }
        Err(err) => {
            section.errors.push(format!("查询失败：{err}"));
            return section;
        }
    };

    let plan_used = detail_usage_number(&detail, "usage", "plan_total_token", "used");
    let plan_limit = detail_usage_number(&detail, "usage", "plan_total_token", "limit");
    let balance_pct = match (plan_used, plan_limit) {
        (Some(used), Some(limit)) if limit > 0.0 => {
            format!("{:.2}%", used / limit * 100.0)
        }
        _ => "-".to_string(),
    };
    let plan_name = detail
        .get("planName")
        .and_then(Value::as_str)
        .unwrap_or("-")
        .to_string();
    let auto_renew = detail
        .get("enableAutoRenew")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let renew_tag = if detail
        .get("expired")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        "已过期"
    } else if auto_renew {
        "自动续费"
    } else {
        "不续费"
    };
    section.description = format!("MiMo {plan_name} · {renew_tag} · 用量 **{balance_pct}**");

    // Per-model billing-cycle usage rows.
    let period_end_raw = detail
        .get("currentPeriodEnd")
        .and_then(Value::as_str)
        .unwrap_or("");
    let (period_end_date, period_start_date) = parse_mimo_period(period_end_raw);
    let months_to_fetch = mimo_months_in_range(period_start_date, period_end_date);
    let ph = extract_cookie_value(&cookie, "api-platform_ph");

    #[derive(Default)]
    struct MimoBucket {
        hit: f64,
        miss: f64,
        out: f64,
    }
    const MIMO_COSTS: &[(&str, f64, f64, f64)] = &[
        ("mimo-v2.5-pro", 2.5, 300.0, 600.0),
        ("mimo-v2.5", 2.0, 100.0, 200.0),
        ("mimo-v2-pro", 140.0, 700.0, 2100.0),
        ("mimo-v2-omni", 56.0, 280.0, 1400.0),
    ];
    let bucket_credits = |model: &str, bucket: &MimoBucket| -> f64 {
        MIMO_COSTS
            .iter()
            .find(|(name, _, _, _)| *name == model)
            .map(|(_, hit_cost, miss_cost, out_cost)| {
                bucket.hit * hit_cost + bucket.miss * miss_cost + bucket.out * out_cost
            })
            .unwrap_or(0.0)
    };

    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    let mut month_buckets: BTreeMap<String, MimoBucket> = BTreeMap::new();
    let mut day_buckets: BTreeMap<String, MimoBucket> = BTreeMap::new();
    for (year, month) in &months_to_fetch {
        let url = format!(
            "{MIMO_API_BASE}/usage/token-plan/list{}",
            if ph.is_empty() {
                String::new()
            } else {
                format!("?api-platform_ph={}", urlencoding_lite(&ph))
            }
        );
        let body = serde_json::json!({"year": year, "month": month});
        let rows = provider_http_post_json(
            &state.client,
            &url,
            body,
            &headers("https://platform.xiaomimimo.com/console/plan-manage"),
        )
        .await;
        let Ok(rows) = rows else {
            continue;
        };
        let code = rows.get("code").and_then(Value::as_i64).unwrap_or(-1);
        if !(code == 0 || code == 200) {
            continue;
        }
        for row in rows.get("data").and_then(Value::as_array).cloned().unwrap_or_default() {
            let model = row
                .get("model")
                .and_then(Value::as_str)
                .unwrap_or("?")
                .to_string();
            let date = row
                .get("date")
                .and_then(Value::as_str)
                .unwrap_or("")
                .chars()
                .take(10)
                .collect::<String>();
            let row_date = chrono::NaiveDate::parse_from_str(&date, "%Y-%m-%d").ok();
            let hit = row.get("inputHitToken").and_then(Value::as_f64).unwrap_or(0.0);
            let miss = row
                .get("inputMissToken")
                .and_then(Value::as_f64)
                .unwrap_or(0.0);
            let out = row.get("outputToken").and_then(Value::as_f64).unwrap_or(0.0);
            if let Some(row_date) = row_date
                && row_date >= period_start_date
                && row_date <= period_end_date
            {
                let entry = month_buckets.entry(model.clone()).or_default();
                entry.hit += hit;
                entry.miss += miss;
                entry.out += out;
            }
            if date == today {
                let entry = day_buckets.entry(model).or_default();
                entry.hit += hit;
                entry.miss += miss;
                entry.out += out;
            }
        }
    }

    let total_model_credit: f64 = month_buckets
        .iter()
        .map(|(model, bucket)| bucket_credits(model, bucket))
        .sum();
    for (model, bucket) in &month_buckets {
        let input = bucket.hit + bucket.miss;
        let credits = bucket_credits(model, bucket);
        let pct = match (plan_used, plan_limit) {
            (Some(_), Some(limit)) if limit > 0.0 && total_model_credit > 0.0 => {
                let share = credits / total_model_credit * plan_used.unwrap_or(0.0);
                format!("{:.2}%", share / limit * 100.0)
            }
            _ => "-".to_string(),
        };
        section.rows.push(ProviderUsageRow {
            label: model.clone(),
            hit_rate: (input > 0.0).then(|| format!("{:.1}%", bucket.hit / input * 100.0)),
            input: Some(fmt_provider_int(input)),
            output: Some(fmt_provider_int(bucket.out)),
            cost_or_pct: (!pct.eq("-")).then_some(pct),
        });
    }
    for (model, bucket) in &day_buckets {
        let input = bucket.hit + bucket.miss;
        section.windows.push(ProviderUsageWindow {
            window: format!("今日·{model}"),
            used: Some(fmt_provider_int(input + bucket.out)),
            reset: None,
        });
    }
    section
}

fn extract_cookie_value(cookie: &str, name: &str) -> String {
    cookie
        .split(';')
        .filter_map(|part| part.trim().split_once('='))
        .find_map(|(key, value)| (key.trim() == name).then(|| value.trim().trim_matches('"').to_string()))
        .unwrap_or_default()
}

fn urlencoding_lite(value: &str) -> String {
    let mut out = String::new();
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char)
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

fn detail_usage_number(detail: &Value, group: &str, item_name: &str, field: &str) -> Option<f64> {
    detail
        .get(group)
        .and_then(|group| group.get("items"))
        .and_then(Value::as_array)?
        .iter()
        .find(|item| item.get("name").and_then(Value::as_str) == Some(item_name))
        .and_then(|item| item.get(field))
        .and_then(Value::as_f64)
}

fn parse_mimo_period(period_end: &str) -> (chrono::NaiveDate, chrono::NaiveDate) {
    let parsed = chrono::NaiveDate::parse_from_str(
        period_end.chars().take(10).collect::<String>().as_str(),
        "%Y-%m-%d",
    );
    match parsed {
        Ok(end) => {
            let start = end - chrono::Duration::days(30);
            (end, start)
        }
        Err(_) => {
            let today = chrono::Local::now().date_naive();
            (today, today.with_day(1).unwrap_or(today))
        }
    }
}

fn mimo_months_in_range(
    start: chrono::NaiveDate,
    end: chrono::NaiveDate,
) -> Vec<(i32, u32)> {
    let mut months = Vec::new();
    let mut cursor = chrono::NaiveDate::from_ymd_opt(start.year(), start.month(), 1)
        .unwrap_or(start);
    let end_month = chrono::NaiveDate::from_ymd_opt(end.year(), end.month(), 1).unwrap_or(end);
    while cursor <= end_month {
        months.push((cursor.year(), cursor.month()));
        cursor = if cursor.month() == 12 {
            chrono::NaiveDate::from_ymd_opt(cursor.year() + 1, 1, 1).unwrap_or(cursor)
        } else {
            chrono::NaiveDate::from_ymd_opt(cursor.year(), cursor.month() + 1, 1)
                .unwrap_or(cursor)
        };
    }
    months
}

async fn fetch_opencode_usage(state: &AppState) -> ProviderUsageSection {
    let mut section = ProviderUsageSection {
        provider: "opencode".into(),
        title: "OpenCode Go 额度".into(),
        ..Default::default()
    };
    let workspace_id = ["OPENCODE_GO_WORKSPACE_ID", "OPENCODE_WORKSPACE_ID"]
        .iter()
        .map(|key| provider_env_value(&state.hermes_home, key))
        .find(|value| !value.is_empty());
    let auth_cookie = ["OPENCODE_GO_AUTH_COOKIE", "OPENCODE_AUTH_COOKIE"]
        .iter()
        .map(|key| provider_env_value(&state.hermes_home, key))
        .find(|value| !value.is_empty());
    let (Some(workspace_id), Some(auth_cookie)) = (workspace_id, auth_cookie) else {
        section
            .errors
            .push("缺少 OPENCODE_GO_WORKSPACE_ID 或 OPENCODE_GO_AUTH_COOKIE".into());
        return section;
    };
    let cookie_header = if auth_cookie.starts_with("auth=") || auth_cookie.contains(';') {
        auth_cookie
    } else {
        format!("auth={auth_cookie}")
    };
    let url = format!("https://opencode.ai/workspace/{workspace_id}/usage");
    let headers = vec![
        ("Accept".to_string(), "text/html".into()),
        (
            "User-Agent".to_string(),
            "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36".into(),
        ),
        ("Cookie".to_string(), cookie_header),
    ];
    let mut request = state.client.get(&url).timeout(PROVIDER_USAGE_TIMEOUT);
    for (name, value) in &headers {
        request = request.header(name.as_str(), value.as_str());
    }
    let response = request.send().await;
    let html = match response {
        Ok(response) => match response.text().await {
            Ok(text) => text,
            Err(err) => {
                section.errors.push(format!("读取失败：{err}"));
                return section;
            }
        },
        Err(err) => {
            section.errors.push(format!("查询失败：{err}"));
            return section;
        }
    };
    for (name, label) in [("rolling", "5h"), ("weekly", "周"), ("monthly", "月")] {
        let Some(window) = opencode_window(&html, name) else {
            continue;
        };
        section.windows.push(ProviderUsageWindow {
            window: label.into(),
            used: Some(format!("{:.2}%", window.0).trim_end_matches('0').trim_end_matches('.').to_string() + "%"),
            reset: Some(provider_reset_text_seconds(window.1)),
        });
    }
    if section.windows.is_empty() {
        section.errors.push("页面缺少 5h/周/月窗口".into());
    }
    section
}

fn opencode_window(html: &str, name: &str) -> Option<(f64, f64)> {
    // SolidJS SSR shape: rollingUsage:$R[12]={usagePercent:12.3,resetInSec:3600,...}
    let marker = format!("{name}Usage:$R[");
    let start = html.find(&marker)? + marker.len();
    let rest = &html[start..];
    let open = rest.find('{')?;
    let close = rest[open..].find('}')? + open;
    let body = &rest[open + 1..close];
    let parse_field = |key: &str| -> Option<f64> {
        let marker = format!("{key}:");
        let index = body.find(&marker)? + marker.len();
        let tail = &body[index..];
        let end = tail
            .find(|ch: char| !(ch.is_ascii_digit() || ch == '.' || ch == '-'))
            .unwrap_or(tail.len());
        tail[..end].parse::<f64>().ok()
    };
    Some((parse_field("usagePercent")?, parse_field("resetInSec")?))
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

async fn fetch_commandcode_usage(state: &AppState) -> ProviderUsageSection {
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

    // Fanout all accounts concurrently with per-account jitter, mirroring the
    // token-usage script's ThreadPoolExecutor behavior; results render in
    // stable label order.
    let mut fetched: Vec<(String, Result<Value, String>)> =
        futures_util::future::join_all(accounts.iter().map(|(label, token)| async {
            jitter_delay().await;
            (label.clone(), commandcode_query_account(state, token).await)
        }))
        .await;
    fetched.sort_by(|a, b| a.0.to_lowercase().cmp(&b.0.to_lowercase()));

    for (label, result) in &fetched {
        match result {
            Ok(snapshot) => {
                let weekly = commandcode_window_text(snapshot, "weekly");
                let monthly = snapshot
                    .get("usage_percent")
                    .and_then(Value::as_f64)
                    .map(|value| format!("{value:.0}%"))
                    .unwrap_or_else(|| "-".into());
                let reset = snapshot
                    .get("current_period_end")
                    .and_then(|value| {
                        value
                            .as_str()
                            .map(str::to_string)
                            .or_else(|| value.as_f64().map(|number| number.to_string()))
                    })
                    .map(|raw| {
                        provider_reset_text_local(raw.parse::<i64>().unwrap_or(0))
                    })
                    .unwrap_or_else(|| "-".into());
                section.windows.push(ProviderUsageWindow {
                    window: label.clone(),
                    used: Some(format!("{weekly} / {monthly}")),
                    reset: Some(reset),
                });
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
) -> Result<Value, String> {
    let whoami = commandcode_get(state, "/alpha/whoami", api_key, &[]).await?;
    let org_id = whoami
        .get("org")
        .and_then(|org| org.get("id"))
        .and_then(Value::as_str)
        .map(str::to_string);
    let mut scoped: Vec<(&str, String)> = Vec::new();
    if let Some(org_id) = &org_id {
        scoped.push(("orgId", org_id.clone()));
    }
    let credits = commandcode_get(state, "/alpha/billing/credits", api_key, &scoped).await?;
    let subscription =
        commandcode_get(state, "/alpha/billing/subscriptions", api_key, &scoped).await?;
    let subscription_data = subscription.get("data").cloned().unwrap_or(Value::Null);
    let since = subscription_data
        .get("currentPeriodStart")
        .and_then(Value::as_str)
        .map(str::to_string);
    if let Some(since) = since {
        scoped.push(("since", since));
    }
    let summary = commandcode_get(state, "/alpha/usage/summary", api_key, &scoped).await?;

    let credits_obj = credits.get("credits").cloned().unwrap_or(Value::Null);
    let number = |value: &Value, key: &str| -> f64 {
        value.get(key).and_then(Value::as_f64).unwrap_or(0.0).max(0.0)
    };
    let monthly_remaining = number(&credits_obj, "monthlyCredits");
    let purchased_remaining = number(&credits_obj, "purchasedCredits");
    let free_remaining = number(&credits_obj, "freeCredits");
    let total_remaining = monthly_remaining + purchased_remaining + free_remaining;
    let summary_obj = summary.get("data").cloned().unwrap_or_else(|| summary.clone());
    let total_cost = summary_obj
        .get("totalCost")
        .and_then(Value::as_f64)
        .unwrap_or(0.0)
        .max(0.0);

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

    Ok(serde_json::json!({
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
        "current_period_end": subscription_data.get("currentPeriodEnd").cloned().unwrap_or(Value::Null),
    }))
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

fn codex_backend_usage_url(base_url: &str) -> String {
    let mut normalized = base_url.trim().trim_end_matches('/').to_string();
    if normalized.is_empty() {
        normalized = "https://chatgpt.com/backend-api/codex".into();
    }
    if normalized.ends_with("/codex") {
        normalized.truncate(normalized.len() - "/codex".len());
    }
    let prefix = if normalized.contains("/backend-api") {
        format!("{normalized}/wham")
    } else {
        format!("{normalized}/api/codex")
    };
    format!("{prefix}/usage")
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

fn jwt_sub_claim(token: &str) -> Option<String> {
    let parts: Vec<&str> = token.split('.').collect();
    let payload = parts.get(1)?;
    let decoded = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(payload)
        .ok()?;
    let claims: Value = serde_json::from_slice(&decoded).ok()?;
    claims
        .get("sub")
        .and_then(Value::as_str)
        .map(str::to_string)
}

async fn fetch_codex_usage(state: &AppState) -> ProviderUsageSection {
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
    // Fanout all accounts concurrently with per-account jitter, mirroring the
    // token-usage script's ThreadPoolExecutor behavior; results render in
    // stable label order.
    let mut fetched: Vec<(String, Result<Value, String>)> =
        futures_util::future::join_all(accounts.iter().map(|(label, token)| async move {
            jitter_delay().await;
            (label.clone(), codex_query_account(state, token).await)
        }))
        .await;
    fetched.sort_by(|a, b| a.0.to_lowercase().cmp(&b.0.to_lowercase()));

    for (label, result) in &fetched {
        match result {
            Ok(payload) => {
                for (key, fallback) in [
                    ("primary_window", "Session"),
                    ("secondary_window", "Weekly"),
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
                        604_800 => "Weekly".to_string(),
                        2_592_000 | 2_628_000 | 2_629_800 | 2_630_000 => "Month".to_string(),
                        _ => fallback.to_string(),
                    };
                    let reset = window
                        .get("reset_at")
                        .and_then(|value| {
                            value.as_str().map(str::to_string).or_else(|| {
                                value.as_f64().map(|number| number.to_string())
                            })
                        })
                        .and_then(|raw| {
                            chrono::DateTime::parse_from_rfc3339(&raw)
                                .ok()
                                .map(|dt| dt.with_timezone(&chrono::Utc))
                                .or_else(|| {
                                    raw.parse::<f64>().ok().and_then(|ts| {
                                        chrono::DateTime::from_timestamp(ts as i64, 0)
                                    })
                                })
                        })
                        .map(|dt| provider_reset_text_local(dt.timestamp()))
                        .unwrap_or_else(|| "-".into());
                    section.windows.push(ProviderUsageWindow {
                        window: format!("{label}·{window_label}"),
                        used: Some(format!("{used:.0}%")),
                        reset: Some(reset),
                    });
                }
            }
            Err(err) => section.errors.push(format!("{label}：查询失败：{err}")),
        }
    }
    section
}

async fn codex_query_account(state: &AppState, token: &str) -> Result<Value, String> {
    let base_url = "https://chatgpt.com/backend-api/codex";
    let account_id = jwt_chatgpt_account_id(token);
    let mut headers = vec![
        ("Authorization".to_string(), format!("Bearer {token}")),
        ("Accept".to_string(), "application/json".into()),
        ("User-Agent".to_string(), "codex-cli".into()),
    ];
    if let Some(account_id) = &account_id {
        headers.push(("ChatGPT-Account-Id".to_string(), account_id.clone()));
    }
    provider_http_get_json(&state.client, &codex_backend_usage_url(base_url), &headers).await
}

/// Random 200-1500ms delay so concurrent account queries don't hit upstream
/// consoles at the same instant (mirrors the script's jitter).
async fn jitter_delay() {
    let millis = 200 + (std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos() as u64 % 1_300)
        .unwrap_or(0));
    sleep(Duration::from_millis(millis)).await;
}

async fn fetch_grok_usage(state: &AppState) -> ProviderUsageSection {
    let mut section = ProviderUsageSection {
        provider: "grok".into(),
        title: "Grok Build 额度".into(),
        ..Default::default()
    };
    let accounts = auth_json_credential_pool(&state.hermes_home, "xai-oauth");
    let Some((_, token)) = accounts.first() else {
        section.errors.push("未找到 xai-oauth OAuth 会话".into());
        return section;
    };
    let user_id = jwt_sub_claim(token);
    let mut headers = vec![
        ("Authorization".to_string(), format!("Bearer {token}")),
        ("X-XAI-Token-Auth".to_string(), "xai-grok-cli".into()),
        ("Accept".to_string(), "application/json".into()),
    ];
    if let Some(user_id) = &user_id {
        headers.push(("x-userid".to_string(), user_id.clone()));
    }
    let payload = match provider_http_get_json(&state.client, GROK_BILLING_URL, &headers).await {
        Ok(value) => value,
        Err(err) => {
            section.errors.push(format!("查询失败：{err}"));
            return section;
        }
    };
    let config = payload.get("config").cloned().unwrap_or(Value::Null);
    let period = config.get("currentPeriod").cloned().unwrap_or(Value::Null);
    let period_type = period
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let reset_raw = period
        .get("end")
        .or_else(|| config.get("billingPeriodEnd"))
        .cloned()
        .unwrap_or(Value::Null);
    if period_type.is_empty() || reset_raw.is_null() {
        section
            .errors
            .push("返回内容缺少可识别的额度周期".into());
        return section;
    }
    let used_percent = config
        .get("creditUsagePercent")
        .and_then(Value::as_f64)
        .unwrap_or(0.0)
        .clamp(0.0, 100.0);
    let window = if period_type.contains("WEEKLY") {
        "周"
    } else if period_type.contains("MONTHLY") {
        "月"
    } else {
        "周期"
    };
    let parsed_reset: Option<chrono::DateTime<chrono::Utc>> = reset_raw
        .as_str()
        .and_then(|raw| chrono::DateTime::parse_from_rfc3339(raw).ok())
        .map(|dt| dt.with_timezone(&chrono::Utc))
        .or_else(|| {
            reset_raw
                .as_f64()
                .and_then(|ts| chrono::DateTime::from_timestamp(ts as i64, 0))
        });
    let reset = parsed_reset
        .map(|dt| {
            let local = dt.with_timezone(&chrono::Local);
            format!("{}/{} {}", local.month(), local.day(), local.format("%H:%M"))
        })
        .unwrap_or_else(|| "-".into());
    let tier = payload
        .get("subscriptionTier")
        .and_then(Value::as_str)
        .unwrap_or("");
    section.description = if tier.is_empty() {
        format!("Grok Build · {window}额度 **{used_percent:.1}%**")
    } else {
        format!("Grok Build · {tier} · {window}额度 **{used_percent:.1}%**")
    };
    section.windows.push(ProviderUsageWindow {
        window: window.into(),
        used: Some(format!("{used_percent:.1}%")),
        reset: Some(reset),
    });
    section
}

async fn collect_provider_usage(state: &AppState) -> ProviderUsagePayload {
    let fetches: Vec<std::pin::Pin<Box<dyn std::future::Future<Output = ProviderUsageSection> + Send + '_>>> = vec![
        Box::pin(fetch_openrouter_usage(state)),
        Box::pin(fetch_deepseek_usage(state)),
        Box::pin(fetch_atlascloud_usage(state)),
        Box::pin(fetch_mimo_usage(state)),
        Box::pin(fetch_minimax_usage(state)),
        Box::pin(fetch_kimi_usage(state)),
        Box::pin(fetch_opencode_usage(state)),
        Box::pin(fetch_commandcode_usage(state)),
        Box::pin(fetch_codex_usage(state)),
        Box::pin(fetch_grok_usage(state)),
    ];
    let sections = futures_util::future::join_all(fetches).await;
    ProviderUsagePayload {
        fetched_at: unix_now_seconds(),
        sections,
    }
}

async fn provider_usage_handler(
    State(state): State<Arc<AppState>>,
    Query(query): Query<ProviderUsageQuery>,
) -> Response<Body> {
    let force = query.refresh.unwrap_or(false);
    async fn cached_payload(state: &AppState) -> Option<ProviderUsagePayload> {
        let guard = state.provider_usage_cache.fetched_at.read().await;
        let fetched_at = (*guard)?;
        if fetched_at.elapsed() >= PROVIDER_USAGE_TTL {
            return None;
        }
        state.provider_usage_cache.payload.read().await.clone()
    }
    if !force
        && let Some(payload) = cached_payload(&state).await
    {
        return (StatusCode::OK, Json(payload)).into_response();
    }
    // Serialize refreshes so parallel UI tabs don't stampede upstream consoles.
    let _guard = state.provider_usage_cache.in_flight.lock().await;
    if !force
        && let Some(payload) = cached_payload(&state).await
    {
        return (StatusCode::OK, Json(payload)).into_response();
    }
    let payload = collect_provider_usage(&state).await;
    *state.provider_usage_cache.payload.write().await = Some(payload.clone());
    *state.provider_usage_cache.fetched_at.write().await = Some(Instant::now());
    (StatusCode::OK, Json(payload)).into_response()
}

#[derive(Deserialize)]
struct ProviderUsageQuery {
    refresh: Option<bool>,
}
