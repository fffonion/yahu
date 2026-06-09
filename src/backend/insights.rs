const INSIGHTS_DAYS: usize = 30;
const INSIGHTS_PAGE_SIZE: usize = 200;
const INSIGHTS_SCAN_LIMIT: usize = 5_000;
const INSIGHTS_REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Clone, Default)]
struct UsageTotals {
    sessions: i64,
    input: i64,
    output: i64,
    cache_read: i64,
    cache_write: i64,
    reasoning: i64,
    api_calls: i64,
    tool_calls: i64,
    estimated_cost: f64,
    actual_cost: f64,
}

impl UsageTotals {
    fn add_row(&mut self, row: &serde_json::Value) {
        self.sessions += 1;
        self.input += json_i64(row, "input_tokens");
        self.output += json_i64(row, "output_tokens");
        self.cache_read += json_i64(row, "cache_read_tokens");
        self.cache_write += json_i64(row, "cache_write_tokens");
        self.reasoning += json_i64(row, "reasoning_tokens");
        self.api_calls += json_i64(row, "api_call_count");
        self.tool_calls += json_i64(row, "tool_call_count");
        self.estimated_cost += json_f64(row, "estimated_cost_usd");
        self.actual_cost += json_f64(row, "actual_cost_usd");
    }

    fn add_totals(&mut self, other: &UsageTotals) {
        self.sessions += other.sessions;
        self.input += other.input;
        self.output += other.output;
        self.cache_read += other.cache_read;
        self.cache_write += other.cache_write;
        self.reasoning += other.reasoning;
        self.api_calls += other.api_calls;
        self.tool_calls += other.tool_calls;
        self.estimated_cost += other.estimated_cost;
        self.actual_cost += other.actual_cost;
    }

    fn total_tokens(&self) -> i64 {
        self.input + self.output + self.cache_read + self.cache_write + self.reasoning
    }

    fn cache_hit_rate(&self) -> f64 {
        let denom = self.input + self.cache_read;
        if denom <= 0 {
            0.0
        } else {
            self.cache_read as f64 / denom as f64
        }
    }

    fn avg_tokens_per_session(&self) -> f64 {
        if self.sessions <= 0 {
            0.0
        } else {
            self.total_tokens() as f64 / self.sessions as f64
        }
    }

    fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "sessions": self.sessions,
            "input": self.input,
            "output": self.output,
            "cache_read": self.cache_read,
            "cache_write": self.cache_write,
            "reasoning": self.reasoning,
            "api_calls": self.api_calls,
            "tool_calls": self.tool_calls,
            "estimated_cost_usd": self.estimated_cost,
            "actual_cost_usd": self.actual_cost,
            "total_tokens": self.total_tokens(),
            "cache_hit_rate": self.cache_hit_rate(),
            "avg_tokens_per_session": self.avg_tokens_per_session(),
        })
    }
}

#[derive(Clone)]
struct DailyUsage {
    date: String,
    label: String,
}

#[derive(Clone)]
struct ModelUsage {
    model: String,
    totals: UsageTotals,
    daily: BTreeMap<String, UsageTotals>,
}

async fn insights_usage(State(state): State<Arc<AppState>>) -> Response<Body> {
    let now = unix_now_seconds();
    match fetch_recent_sessions_for_insights(&state, now).await {
        Ok(rows) => Json(aggregate_usage_insights(&rows, now)).into_response(),
        Err(err) => json_error(
            StatusCode::BAD_GATEWAY,
            &format!("insights API request failed: {err}"),
        ),
    }
}

async fn fetch_recent_sessions_for_insights(
    state: &AppState,
    now: f64,
) -> anyhow::Result<Vec<serde_json::Value>> {
    let min_ts = now - (INSIGHTS_DAYS as f64 * 86_400.0);
    let mut rows = Vec::new();
    let mut offset = 0usize;
    loop {
        let url = api_sessions_url(state.api_url.trim_end_matches('/'), INSIGHTS_PAGE_SIZE, offset, "")?;
        let mut req = state.client.get(url);
        if let Some(key) = &state.api_key
            && !key.is_empty()
        {
            req = req.bearer_auth(key);
        }
        let resp = timeout(INSIGHTS_REQUEST_TIMEOUT, req.send()).await??;
        if !resp.status().is_success() {
            anyhow::bail!("session list request failed: {}", resp.status());
        }
        let body = resp.json::<serde_json::Value>().await?;
        let data = body
            .get("data")
            .and_then(|value| value.as_array())
            .cloned()
            .unwrap_or_default();
        let data_len = data.len();
        let mut visible_seen = 0usize;
        let mut visible_older = 0usize;
        for row in data {
            if !is_client_visible_session(&row) {
                continue;
            }
            visible_seen += 1;
            let ts = session_usage_timestamp(&row);
            if ts >= min_ts {
                rows.push(row);
            } else {
                visible_older += 1;
            }
        }
        let has_more = body
            .get("has_more")
            .and_then(|value| value.as_bool())
            .unwrap_or(data_len == INSIGHTS_PAGE_SIZE);
        offset = offset.saturating_add(INSIGHTS_PAGE_SIZE);
        let all_visible_older = visible_seen > 0 && visible_seen == visible_older;
        if !has_more || data_len == 0 || offset >= INSIGHTS_SCAN_LIMIT || all_visible_older {
            return Ok(rows);
        }
    }
}

fn aggregate_usage_insights(rows: &[serde_json::Value], now: f64) -> serde_json::Value {
    let days = insight_days(now);
    let mut totals = UsageTotals::default();
    let mut models: HashMap<String, ModelUsage> = HashMap::new();
    let mut sources: HashMap<String, UsageTotals> = HashMap::new();

    for row in rows {
        let ts = session_usage_timestamp(row);
        let Some(day) = usage_day_key(ts) else { continue };
        if !days.iter().any(|item| item.date == day) {
            continue;
        }
        totals.add_row(row);
        let model_name = row
            .get("model")
            .and_then(|value| value.as_str())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("unknown")
            .to_string();
        let source_name = row
            .get("source")
            .and_then(|value| value.as_str())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("unknown")
            .to_string();
        let model = models.entry(model_name.clone()).or_insert_with(|| ModelUsage {
            model: model_name.clone(),
            totals: UsageTotals::default(),
            daily: days
                .iter()
                .map(|item| (item.date.clone(), UsageTotals::default()))
                .collect(),
        });
        model.totals.add_row(row);
        model.daily.entry(day.clone()).or_default().add_row(row);
        sources.entry(source_name).or_default().add_row(row);
    }

    let mut model_rows: Vec<_> = models.into_values().collect();
    model_rows.sort_by(|a, b| b.totals.total_tokens().cmp(&a.totals.total_tokens()).then(a.model.cmp(&b.model)));

    let mut daily_totals: Vec<serde_json::Value> = Vec::new();
    for item in &days {
        let mut day_total = UsageTotals::default();
        for model in &model_rows {
            if let Some(value) = model.daily.get(&item.date) {
                day_total.add_totals(value);
            }
        }
        daily_totals.push(serde_json::json!({
            "date": item.date,
            "label": item.label,
            "totals": day_total.to_json(),
        }));
    }

    let periods = [30usize, 7, 1]
        .into_iter()
        .map(|period| {
            let period_dates: HashSet<String> = days
                .iter()
                .rev()
                .take(period)
                .map(|item| item.date.clone())
                .collect();
            let mut period_total = UsageTotals::default();
            let model_totals: Vec<_> = model_rows
                .iter()
                .map(|model| {
                    let mut value = UsageTotals::default();
                    for date in &period_dates {
                        if let Some(day) = model.daily.get(date) {
                            value.add_totals(day);
                        }
                    }
                    period_total.add_totals(&value);
                    serde_json::json!({"model": model.model, "totals": value.to_json()})
                })
                .filter(|value| value["totals"]["total_tokens"].as_i64().unwrap_or(0) > 0)
                .collect();
            serde_json::json!({
                "days": period,
                "totals": period_total.to_json(),
                "models": model_totals,
            })
        })
        .collect::<Vec<_>>();

    let source_rows = {
        let mut rows: Vec<_> = sources.into_iter().collect();
        rows.sort_by(|a, b| b.1.total_tokens().cmp(&a.1.total_tokens()).then(a.0.cmp(&b.0)));
        rows.into_iter()
            .map(|(source, totals)| serde_json::json!({"source": source, "totals": totals.to_json()}))
            .collect::<Vec<_>>()
    };

    serde_json::json!({
        "object": "yahu.insights.usage",
        "generated_at": now,
        "window_days": INSIGHTS_DAYS,
        "totals": totals.to_json(),
        "daily": daily_totals,
        "models": model_rows.into_iter().map(|model| serde_json::json!({
            "model": model.model,
            "totals": model.totals.to_json(),
            "daily": days.iter().map(|item| {
                let empty = UsageTotals::default();
                let totals = model.daily.get(&item.date).unwrap_or(&empty);
                serde_json::json!({"date": item.date, "label": item.label, "totals": totals.to_json()})
            }).collect::<Vec<_>>(),
        })).collect::<Vec<_>>(),
        "sources": source_rows,
        "periods": periods,
    })
}

fn insight_days(now: f64) -> Vec<DailyUsage> {
    let today = chrono::DateTime::<chrono::Utc>::from_timestamp(now as i64, 0)
        .unwrap_or_else(chrono::Utc::now)
        .date_naive();
    (0..INSIGHTS_DAYS)
        .rev()
        .map(|offset| {
            let date = today - chrono::Duration::days(offset as i64);
            DailyUsage {
                date: date.format("%Y-%m-%d").to_string(),
                label: date.format("%m/%d").to_string(),
            }
        })
        .collect()
}

fn unix_now_seconds() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::ZERO)
        .as_secs_f64()
}

fn usage_day_key(ts: f64) -> Option<String> {
    chrono::DateTime::<chrono::Utc>::from_timestamp(ts as i64, 0)
        .map(|date| date.date_naive().format("%Y-%m-%d").to_string())
}

fn session_usage_timestamp(row: &serde_json::Value) -> f64 {
    json_f64(row, "last_active")
        .max(json_f64(row, "ended_at"))
        .max(json_f64(row, "started_at"))
}

fn json_i64(row: &serde_json::Value, key: &str) -> i64 {
    row.get(key)
        .and_then(|value| value.as_i64().or_else(|| value.as_f64().map(|v| v as i64)))
        .unwrap_or(0)
}

fn json_f64(row: &serde_json::Value, key: &str) -> f64 {
    row.get(key).and_then(|value| value.as_f64()).unwrap_or(0.0)
}
