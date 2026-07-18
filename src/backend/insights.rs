const INSIGHTS_DEFAULT_DAYS: usize = 7;
const INSIGHTS_MAX_DAYS: usize = 30;
const INSIGHTS_HOURS: usize = 24;
const INSIGHTS_PAGE_SIZE: usize = 200;
const INSIGHTS_SCAN_LIMIT: usize = 5_000;
const INSIGHTS_REQUEST_TIMEOUT: Duration = Duration::from_secs(10);
const MODELS_DEV_API_URL: &str = "https://models.dev/api.json";
const MODEL_PRICE_CACHE_TTL: Duration = Duration::from_secs(6 * 60 * 60);
const INSIGHTS_SNAPSHOT_INTERVAL: Duration = Duration::from_secs(5 * 60);
const INSIGHTS_SNAPSHOT_RETENTION_SECONDS: f64 = 35.0 * 86_400.0;
const INSIGHTS_SNAPSHOT_DB: &str = "state/yahu-insights-usage.db";

type ModelPriceCatalog = HashMap<String, ModelPrice>;

#[derive(Clone, Default, Deserialize, Serialize)]
struct UsageCounter {
    session_id: String,
    model: String,
    source: String,
    #[serde(default)]
    started_at: f64,
    input_tokens: i64,
    output_tokens: i64,
    cache_read_tokens: i64,
    cache_write_tokens: i64,
    reasoning_tokens: i64,
    api_call_count: i64,
    tool_call_count: i64,
    estimated_cost_usd: f64,
    actual_cost_usd: f64,
}

impl UsageCounter {
    fn from_api_row(row: &serde_json::Value) -> Option<Self> {
        let session_id = row.get("id")?.as_str()?.trim().to_string();
        if session_id.is_empty() {
            return None;
        }
        Some(Self {
            session_id,
            model: row
                .get("model")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("unknown")
                .trim()
                .to_string(),
            source: row
                .get("source")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("unknown")
                .trim()
                .to_string(),
            started_at: json_timestamp(row, "started_at"),
            input_tokens: json_i64(row, "input_tokens"),
            output_tokens: json_i64(row, "output_tokens"),
            cache_read_tokens: json_i64(row, "cache_read_tokens"),
            cache_write_tokens: json_i64(row, "cache_write_tokens"),
            reasoning_tokens: json_i64(row, "reasoning_tokens"),
            api_call_count: json_i64(row, "api_call_count"),
            tool_call_count: json_i64(row, "tool_call_count"),
            estimated_cost_usd: json_f64(row, "estimated_cost_usd"),
            actual_cost_usd: json_f64(row, "actual_cost_usd"),
        })
    }

    fn delta_from(&self, previous: Option<&Self>) -> Self {
        fn delta_i64(current: i64, previous: i64) -> i64 {
            if current >= previous { current - previous } else { current.max(0) }
        }
        fn delta_f64(current: f64, previous: f64) -> f64 {
            if current >= previous { current - previous } else { current.max(0.0) }
        }
        let previous = previous.cloned().unwrap_or_default();
        Self {
            session_id: self.session_id.clone(),
            model: self.model.clone(),
            source: self.source.clone(),
            started_at: self.started_at,
            input_tokens: delta_i64(self.input_tokens, previous.input_tokens),
            output_tokens: delta_i64(self.output_tokens, previous.output_tokens),
            cache_read_tokens: delta_i64(self.cache_read_tokens, previous.cache_read_tokens),
            cache_write_tokens: delta_i64(self.cache_write_tokens, previous.cache_write_tokens),
            reasoning_tokens: delta_i64(self.reasoning_tokens, previous.reasoning_tokens),
            api_call_count: delta_i64(self.api_call_count, previous.api_call_count),
            tool_call_count: delta_i64(self.tool_call_count, previous.tool_call_count),
            estimated_cost_usd: delta_f64(self.estimated_cost_usd, previous.estimated_cost_usd),
            actual_cost_usd: delta_f64(self.actual_cost_usd, previous.actual_cost_usd),
        }
    }

    fn subtract(&self, used: &Self) -> Self {
        Self {
            session_id: self.session_id.clone(),
            model: self.model.clone(),
            source: self.source.clone(),
            started_at: self.started_at,
            input_tokens: (self.input_tokens - used.input_tokens).max(0),
            output_tokens: (self.output_tokens - used.output_tokens).max(0),
            cache_read_tokens: (self.cache_read_tokens - used.cache_read_tokens).max(0),
            cache_write_tokens: (self.cache_write_tokens - used.cache_write_tokens).max(0),
            reasoning_tokens: (self.reasoning_tokens - used.reasoning_tokens).max(0),
            api_call_count: (self.api_call_count - used.api_call_count).max(0),
            tool_call_count: (self.tool_call_count - used.tool_call_count).max(0),
            estimated_cost_usd: (self.estimated_cost_usd - used.estimated_cost_usd).max(0.0),
            actual_cost_usd: (self.actual_cost_usd - used.actual_cost_usd).max(0.0),
        }
    }

    fn add_assign(&mut self, other: &Self) {
        self.input_tokens += other.input_tokens;
        self.output_tokens += other.output_tokens;
        self.cache_read_tokens += other.cache_read_tokens;
        self.cache_write_tokens += other.cache_write_tokens;
        self.reasoning_tokens += other.reasoning_tokens;
        self.api_call_count += other.api_call_count;
        self.tool_call_count += other.tool_call_count;
        self.estimated_cost_usd += other.estimated_cost_usd;
        self.actual_cost_usd += other.actual_cost_usd;
    }

    fn has_delta(&self) -> bool {
        self.input_tokens > 0
            || self.output_tokens > 0
            || self.cache_read_tokens > 0
            || self.cache_write_tokens > 0
            || self.reasoning_tokens > 0
            || self.api_call_count > 0
            || self.tool_call_count > 0
            || self.estimated_cost_usd > 0.0
            || self.actual_cost_usd > 0.0
    }

    fn to_event_json(&self, captured_at: f64) -> serde_json::Value {
        serde_json::json!({
            "id": self.session_id,
            "source": self.source,
            "model": self.model,
            "started_at": captured_at,
            "last_active": captured_at,
            "input_tokens": self.input_tokens,
            "output_tokens": self.output_tokens,
            "cache_read_tokens": self.cache_read_tokens,
            "cache_write_tokens": self.cache_write_tokens,
            "reasoning_tokens": self.reasoning_tokens,
            "api_call_count": self.api_call_count,
            "tool_call_count": self.tool_call_count,
            "estimated_cost_usd": self.estimated_cost_usd,
            "actual_cost_usd": self.actual_cost_usd,
        })
    }
}

fn prepare_insights_snapshot_db(conn: &rusqlite::Connection) -> anyhow::Result<()> {
    conn.busy_timeout(Duration::from_secs(5))?;
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS insights_meta (
             key TEXT PRIMARY KEY,
             value REAL NOT NULL
         );
         CREATE TABLE IF NOT EXISTS insights_baselines (
             session_id TEXT PRIMARY KEY,
             captured_at REAL NOT NULL,
             last_seen REAL NOT NULL,
             counters_json TEXT NOT NULL
         );
         CREATE TABLE IF NOT EXISTS insights_initial_baselines (
             session_id TEXT PRIMARY KEY,
             counters_json TEXT NOT NULL
         );
         CREATE TABLE IF NOT EXISTS insights_events (
             id INTEGER PRIMARY KEY AUTOINCREMENT,
             captured_at REAL NOT NULL,
             row_json TEXT NOT NULL
         );
         CREATE INDEX IF NOT EXISTS insights_events_captured_at
             ON insights_events(captured_at);",
    )?;
    Ok(())
}

fn persist_insights_snapshot(path: &Path, captured_at: f64, rows: &[serde_json::Value]) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut conn = rusqlite::Connection::open(path)?;
    prepare_insights_snapshot_db(&conn)?;
    let tx = conn.transaction()?;
    let coverage_started_at = tx
        .query_row(
            "SELECT value FROM insights_meta WHERE key = 'coverage_started_at'",
            [],
            |row| row.get::<_, f64>(0),
        )
        .optional()?;
    if coverage_started_at.is_none() {
        tx.execute(
            "INSERT INTO insights_meta(key, value) VALUES('coverage_started_at', ?1)",
            [captured_at],
        )?;
    }

    let needs_initial_backfill = coverage_started_at.is_some()
        && tx.query_row(
            "SELECT EXISTS(
                 SELECT 1 FROM insights_baselines b
                 LEFT JOIN insights_initial_baselines i ON i.session_id = b.session_id
                 WHERE i.session_id IS NULL
             )",
            [],
            |row| row.get::<_, bool>(0),
        )?;
    let mut recorded_deltas: HashMap<String, UsageCounter> = HashMap::new();
    if needs_initial_backfill {
        let mut statement = tx.prepare("SELECT row_json FROM insights_events ORDER BY id")?;
        let event_rows = statement
            .query_map([], |row| row.get::<_, String>(0))?
            .filter_map(Result::ok)
            .collect::<Vec<_>>();
        drop(statement);
        for value in event_rows {
            let Ok(json) = serde_json::from_str::<serde_json::Value>(&value) else { continue };
            let Some(counter) = UsageCounter::from_api_row(&json) else { continue };
            recorded_deltas
                .entry(counter.session_id.clone())
                .or_default()
                .add_assign(&counter);
        }
    }

    for current in rows.iter().filter_map(UsageCounter::from_api_row) {
        let previous = tx
            .query_row(
                "SELECT counters_json FROM insights_baselines WHERE session_id = ?1",
                [&current.session_id],
                |row| row.get::<_, String>(0),
            )
            .optional()?
            .and_then(|value| serde_json::from_str::<UsageCounter>(&value).ok());
        let has_initial = tx
            .query_row(
                "SELECT 1 FROM insights_initial_baselines WHERE session_id = ?1",
                [&current.session_id],
                |_| Ok(()),
            )
            .optional()?
            .is_some();
        if !has_initial {
            let initial = if coverage_started_at.is_none() {
                Some(current.clone())
            } else if current.started_at <= coverage_started_at.unwrap_or(0.0) {
                previous.as_ref().map(|value| {
                    let recorded = recorded_deltas.get(&current.session_id).cloned().unwrap_or_default();
                    let mut baseline = value.subtract(&recorded);
                    baseline.started_at = current.started_at;
                    baseline.model.clone_from(&current.model);
                    baseline.source.clone_from(&current.source);
                    baseline
                })
            } else {
                Some(UsageCounter {
                    session_id: current.session_id.clone(),
                    model: current.model.clone(),
                    source: current.source.clone(),
                    started_at: current.started_at,
                    ..UsageCounter::default()
                })
            };
            if let Some(initial) = initial {
                tx.execute(
                    "INSERT OR IGNORE INTO insights_initial_baselines(session_id, counters_json) VALUES(?1, ?2)",
                    rusqlite::params![initial.session_id, serde_json::to_string(&initial)?],
                )?;
            }
        }
        if coverage_started_at.is_some() {
            let delta = current.delta_from(previous.as_ref());
            if delta.has_delta() {
                let row_json = serde_json::to_string(&delta.to_event_json(captured_at))?;
                tx.execute(
                    "INSERT INTO insights_events(captured_at, row_json) VALUES(?1, ?2)",
                    rusqlite::params![captured_at, row_json],
                )?;
            }
        }
        tx.execute(
            "INSERT INTO insights_baselines(session_id, captured_at, last_seen, counters_json)
             VALUES(?1, ?2, ?2, ?3)
             ON CONFLICT(session_id) DO UPDATE SET
                 captured_at = excluded.captured_at,
                 last_seen = excluded.last_seen,
                 counters_json = excluded.counters_json",
            rusqlite::params![current.session_id, captured_at, serde_json::to_string(&current)?],
        )?;
    }

    let retention_cutoff = captured_at - INSIGHTS_SNAPSHOT_RETENTION_SECONDS;
    tx.execute("DELETE FROM insights_events WHERE captured_at < ?1", [retention_cutoff])?;
    tx.execute("DELETE FROM insights_baselines WHERE last_seen < ?1", [retention_cutoff])?;
    tx.execute(
        "DELETE FROM insights_initial_baselines
         WHERE session_id NOT IN (SELECT session_id FROM insights_baselines)",
        [],
    )?;
    tx.commit()?;
    Ok(())
}

fn load_insights_usage_rows(
    path: &Path,
    min_timestamp: f64,
) -> anyhow::Result<(Vec<serde_json::Value>, Option<f64>)> {
    if !path.exists() {
        return Ok((Vec::new(), None));
    }
    let conn = rusqlite::Connection::open(path)?;
    prepare_insights_snapshot_db(&conn)?;
    let coverage_started_at = conn
        .query_row(
            "SELECT value FROM insights_meta WHERE key = 'coverage_started_at'",
            [],
            |row| row.get::<_, f64>(0),
        )
        .optional()?;
    let mut usage_rows = {
        let mut statement = conn.prepare(
            "SELECT row_json FROM insights_events WHERE captured_at >= ?1 ORDER BY captured_at, id",
        )?;
        statement
            .query_map([min_timestamp], |row| row.get::<_, String>(0))?
            .filter_map(Result::ok)
            .filter_map(|value| serde_json::from_str::<serde_json::Value>(&value).ok())
            .collect::<Vec<_>>()
    };
    let fallback_rows = {
        let mut statement = conn.prepare("SELECT counters_json FROM insights_initial_baselines")?;
        statement
            .query_map([], |row| row.get::<_, String>(0))?
            .filter_map(Result::ok)
            .filter_map(|value| serde_json::from_str::<UsageCounter>(&value).ok())
            .filter(|counter| counter.started_at >= min_timestamp && counter.has_delta())
            .map(|counter| counter.to_event_json(counter.started_at))
            .collect::<Vec<_>>()
    };
    usage_rows.extend(fallback_rows);
    Ok((usage_rows, coverage_started_at))
}

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
    cost: f64,
    unpriced_tokens: i64,
}

#[derive(Clone, Copy)]
struct ModelPrice {
    input_per_million: f64,
    output_per_million: f64,
    cache_read_per_million: f64,
    cache_write_per_million: f64,
}

impl ModelPrice {
    fn estimate(&self, input: i64, output: i64, cache_read: i64, cache_write: i64) -> f64 {
        ((input.max(0) as f64 * self.input_per_million)
            + (output.max(0) as f64 * self.output_per_million)
            + (cache_read.max(0) as f64 * self.cache_read_per_million)
            + (cache_write.max(0) as f64 * self.cache_write_per_million))
            / 1_000_000.0
    }

    fn has_billable_price(&self) -> bool {
        self.input_per_million > 0.0
            || self.output_per_million > 0.0
            || self.cache_read_per_million > 0.0
            || self.cache_write_per_million > 0.0
    }
}

impl UsageTotals {
    fn add_row(&mut self, row: &serde_json::Value, prices: &ModelPriceCatalog) {
        let input = json_i64(row, "input_tokens");
        let output = json_i64(row, "output_tokens");
        let cache_read = json_i64(row, "cache_read_tokens");
        let cache_write = json_i64(row, "cache_write_tokens");
        let reasoning = json_i64(row, "reasoning_tokens");
        let actual_cost = json_f64(row, "actual_cost_usd");
        let api_estimated_cost = json_f64(row, "estimated_cost_usd");
        let catalog_estimated_cost = row
            .get("model")
            .and_then(|value| value.as_str())
            .and_then(|model| model_price_for_model(prices, model))
            .map(|price| price.estimate(input, output, cache_read, cache_write));
        let estimated_cost = catalog_estimated_cost.or((api_estimated_cost > 0.0).then_some(api_estimated_cost));
        let row_cost = estimated_cost.or((actual_cost > 0.0).then_some(actual_cost));
        self.sessions += 1;
        self.input += input;
        self.output += output;
        self.cache_read += cache_read;
        self.cache_write += cache_write;
        self.reasoning += reasoning;
        self.api_calls += json_i64(row, "api_call_count");
        self.tool_calls += json_i64(row, "tool_call_count");
        self.estimated_cost += estimated_cost.unwrap_or(0.0);
        self.actual_cost += actual_cost;
        self.cost += row_cost.unwrap_or(0.0);
        if row_cost.is_none() && input + output + cache_read + cache_write + reasoning > 0 {
            self.unpriced_tokens += input + output + cache_read + cache_write + reasoning;
        }
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
        self.cost += other.cost;
        self.unpriced_tokens += other.unpriced_tokens;
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
            "cost_usd": self.cost,
            "unpriced_tokens": self.unpriced_tokens,
            "total_tokens": self.total_tokens(),
            "cache_hit_rate": self.cache_hit_rate(),
            "avg_tokens_per_session": self.avg_tokens_per_session(),
        })
    }
}

fn normalize_model_price_key(model: &str) -> String {
    model
        .trim()
        .to_ascii_lowercase()
        .replace(['_', '/'], "-")
}

fn insert_model_price(catalog: &mut ModelPriceCatalog, key: &str, price: ModelPrice) {
    let normalized = normalize_model_price_key(key);
    if normalized.is_empty() {
        return;
    }
    if let Some(existing) = catalog.get(&normalized)
        && existing.has_billable_price()
        && !price.has_billable_price()
    {
        return;
    }
    catalog.insert(normalized, price);
}

fn model_price_for_model(catalog: &ModelPriceCatalog, model: &str) -> Option<ModelPrice> {
    catalog.get(&normalize_model_price_key(model)).copied()
}

fn model_price_catalog_from_models_dev(body: &serde_json::Value) -> ModelPriceCatalog {
    let mut catalog = ModelPriceCatalog::new();
    let Some(providers) = body.as_object() else { return catalog };
    for (provider_id, provider) in providers {
        let Some(models) = provider.get("models").and_then(|value| value.as_object()) else {
            continue;
        };
        for (model_key, model) in models {
            let Some(price) = model_price_from_models_dev_model(model) else {
                continue;
            };
            insert_model_price(&mut catalog, model_key, price);
            if let Some(model_id) = model.get("id").and_then(|value| value.as_str()) {
                insert_model_price(&mut catalog, model_id, price);
                insert_model_price(&mut catalog, &format!("{provider_id}/{model_id}"), price);
                insert_model_price(&mut catalog, &format!("{provider_id}-{model_id}"), price);
            }
            if let Some(model_name) = model.get("name").and_then(|value| value.as_str()) {
                insert_model_price(&mut catalog, model_name, price);
            }
        }
    }
    catalog
}

fn model_price_from_models_dev_model(model: &serde_json::Value) -> Option<ModelPrice> {
    let cost = model.get("cost")?;
    Some(ModelPrice {
        input_per_million: json_cost_f64(cost, "input"),
        output_per_million: json_cost_f64(cost, "output"),
        cache_read_per_million: json_cost_f64(cost, "cache_read"),
        cache_write_per_million: json_cost_f64(cost, "cache_write"),
    })
}

fn json_cost_f64(row: &serde_json::Value, key: &str) -> f64 {
    match row.get(key) {
        Some(value) => value.as_f64().or_else(|| {
            value
                .as_str()
                .and_then(|text| text.trim().trim_start_matches('$').parse::<f64>().ok())
        }),
        None => None,
    }
    .unwrap_or(0.0)
}

#[derive(Clone)]
struct DailyUsage {
    date: String,
    label: String,
}

#[derive(Clone)]
struct HourlyUsage {
    hour: String,
    label: String,
}

#[derive(Clone)]
struct ModelUsage {
    model: String,
    totals: UsageTotals,
    daily: BTreeMap<String, UsageTotals>,
    hourly: BTreeMap<String, UsageTotals>,
}

#[derive(Default, Deserialize)]
struct InsightsUsageQuery {
    period: Option<usize>,
    days: Option<usize>,
    tz_offset: Option<i32>,
}

fn normalize_insights_period(value: Option<usize>) -> usize {
    match value {
        Some(1) => 1,
        Some(7) => 7,
        Some(30) => 30,
        _ => INSIGHTS_DEFAULT_DAYS,
    }
}

fn normalize_insights_timezone_offset(value: Option<i32>) -> i32 {
    value.unwrap_or(0).clamp(-14 * 60, 14 * 60)
}

async fn insights_usage(
    State(state): State<Arc<AppState>>,
    Query(query): Query<InsightsUsageQuery>,
) -> Response<Body> {
    let now = unix_now_seconds();
    let period_days = normalize_insights_period(query.period.or(query.days));
    let timezone_offset = normalize_insights_timezone_offset(query.tz_offset);
    if let Err(err) = capture_insights_snapshot(&state, now).await {
        warn!("insights usage snapshot capture failed: {err}");
    }
    let snapshot_path = state.hermes_home.join(INSIGHTS_SNAPSHOT_DB);
    let min_timestamp = now - (INSIGHTS_MAX_DAYS as f64 * 86_400.0);
    let snapshot_data = tokio::task::spawn_blocking(move || {
        load_insights_usage_rows(&snapshot_path, min_timestamp)
    })
    .await;
    let (rows, coverage_started_at) = match snapshot_data {
        Ok(Ok(value)) => value,
        Ok(Err(err)) => {
            return json_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                &format!("insights snapshot read failed: {err}"),
            );
        }
        Err(err) => {
            return json_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                &format!("insights snapshot task failed: {err}"),
            );
        }
    };
    let prices = match fetch_models_dev_price_catalog(&state).await {
        Ok(prices) => prices,
        Err(err) => {
            warn!("models.dev price fetch failed: {err}");
            ModelPriceCatalog::new()
        }
    };
    let mut body = aggregate_usage_insights_with_prices_at_offset(
        &rows,
        now,
        &prices,
        period_days,
        timezone_offset,
    );
    if let Some(object) = body.as_object_mut() {
        object.insert("coverage_started_at".to_string(), serde_json::json!(coverage_started_at));
        object.insert(
            "coverage_complete".to_string(),
            serde_json::json!(coverage_started_at.is_some_and(|value| {
                value <= now - (period_days as f64 * 86_400.0)
            })),
        );
    }
    Json(body).into_response()
}

async fn capture_insights_snapshot(state: &AppState, captured_at: f64) -> anyhow::Result<()> {
    let rows = fetch_sessions_for_insights_snapshot(state).await?;
    let path = state.hermes_home.join(INSIGHTS_SNAPSHOT_DB);
    tokio::task::spawn_blocking(move || persist_insights_snapshot(&path, captured_at, &rows)).await??;
    Ok(())
}

async fn run_insights_snapshot_collector(state: Arc<AppState>) {
    let mut ticker = interval(INSIGHTS_SNAPSHOT_INTERVAL);
    ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);
    loop {
        ticker.tick().await;
        if let Err(err) = capture_insights_snapshot(&state, unix_now_seconds()).await {
            warn!("background insights snapshot capture failed: {err}");
        }
    }
}

async fn fetch_models_dev_price_catalog(state: &AppState) -> anyhow::Result<ModelPriceCatalog> {
    fetch_models_dev_price_catalog_from_url(state, MODELS_DEV_API_URL).await
}

async fn fetch_models_dev_price_catalog_from_url(
    state: &AppState,
    url: &str,
) -> anyhow::Result<ModelPriceCatalog> {
    {
        let cache = state.model_price_cache.read().await;
        if let Some(body) = fresh_model_cache_body(&cache, MODEL_PRICE_CACHE_TTL) {
            return Ok(model_price_catalog_from_models_dev(&body));
        }
    }

    let fetch_result = async {
        let resp = timeout(
            INSIGHTS_REQUEST_TIMEOUT,
            state
                .client
                .get(url)
                .header(header::USER_AGENT, concat!("yahu/", env!("CARGO_PKG_VERSION")))
                .send(),
        )
        .await??;
        if !resp.status().is_success() {
            anyhow::bail!("models.dev price request failed: {}", resp.status());
        }
        resp.json::<serde_json::Value>().await.map_err(Into::into)
    }
    .await;

    match fetch_result {
        Ok(body) => {
            let mut cache = state.model_price_cache.write().await;
            cache.fetched_at = Some(std::time::Instant::now());
            cache.body = Some(body.clone());
            Ok(model_price_catalog_from_models_dev(&body))
        }
        Err(err) => {
            let stale_body = {
                let cache = state.model_price_cache.read().await;
                cache.body.clone()
            };
            if let Some(body) = stale_body {
                warn!("models.dev price refresh failed; using stale price cache: {err}");
                return Ok(model_price_catalog_from_models_dev(&body));
            }
            Err(err)
        }
    }
}

async fn fetch_sessions_for_insights_snapshot(state: &AppState) -> anyhow::Result<Vec<serde_json::Value>> {
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
        rows.extend(data.into_iter().filter(is_client_visible_session));
        let has_more = body
            .get("has_more")
            .and_then(|value| value.as_bool())
            .unwrap_or(data_len == INSIGHTS_PAGE_SIZE);
        offset = offset.saturating_add(INSIGHTS_PAGE_SIZE);
        if !has_more || data_len == 0 || offset >= INSIGHTS_SCAN_LIMIT {
            return Ok(rows);
        }
    }
}

#[cfg(test)]
async fn fetch_recent_sessions_for_insights(
    state: &AppState,
    now: f64,
    window_days: usize,
) -> anyhow::Result<Vec<serde_json::Value>> {
    let min_ts = now - (window_days as f64 * 86_400.0);
    Ok(fetch_sessions_for_insights_snapshot(state)
        .await?
        .into_iter()
        .filter(|row| session_usage_timestamp(row) >= min_ts)
        .collect())
}

#[cfg(test)]
fn aggregate_usage_insights_with_prices(
    rows: &[serde_json::Value],
    now: f64,
    prices: &ModelPriceCatalog,
    period_days: usize,
) -> serde_json::Value {
    aggregate_usage_insights_with_prices_at_offset(rows, now, prices, period_days, 0)
}

fn aggregate_usage_insights_with_prices_at_offset(
    rows: &[serde_json::Value],
    now: f64,
    prices: &ModelPriceCatalog,
    period_days: usize,
    timezone_offset_minutes: i32,
) -> serde_json::Value {
    let days = insight_days_at_offset(now, period_days, timezone_offset_minutes);
    let hours = insight_hours_at_offset(now, timezone_offset_minutes);
    let hour_keys: HashSet<String> = hours.iter().map(|item| item.hour.clone()).collect();
    let mut totals = UsageTotals::default();
    let mut models: HashMap<String, ModelUsage> = HashMap::new();
    let mut sources: HashMap<String, UsageTotals> = HashMap::new();

    for row in rows {
        let ts = session_usage_timestamp(row);
        let Some(day) = usage_day_key_at_offset(ts, timezone_offset_minutes) else { continue };
        let hour = usage_hour_key_at_offset(ts, timezone_offset_minutes)
            .filter(|value| hour_keys.contains(value));
        let bucket_day = if period_days == 1 {
            if hour.is_none() {
                continue;
            }
            days.first().map(|item| item.date.clone()).unwrap_or(day)
        } else {
            if !days.iter().any(|item| item.date == day) {
                continue;
            }
            day
        };
        totals.add_row(row, prices);
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
            hourly: hours
                .iter()
                .map(|item| (item.hour.clone(), UsageTotals::default()))
                .collect(),
        });
        model.totals.add_row(row, prices);
        model.daily.entry(bucket_day).or_default().add_row(row, prices);
        if let Some(hour) = hour {
            model.hourly.entry(hour).or_default().add_row(row, prices);
        }
        sources.entry(source_name).or_default().add_row(row, prices);
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

    let mut hourly_totals: Vec<serde_json::Value> = Vec::new();
    for item in &hours {
        let mut hour_total = UsageTotals::default();
        for model in &model_rows {
            if let Some(value) = model.hourly.get(&item.hour) {
                hour_total.add_totals(value);
            }
        }
        hourly_totals.push(serde_json::json!({
            "hour": item.hour,
            "label": item.label,
            "totals": hour_total.to_json(),
        }));
    }

    let periods = [period_days]
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
            let mut period_sources: HashMap<String, UsageTotals> = HashMap::new();
            for row in rows {
                let row_timestamp = session_usage_timestamp(row);
                let in_period = if period == 1 {
                    usage_hour_key_at_offset(row_timestamp, timezone_offset_minutes)
                        .is_some_and(|hour| hour_keys.contains(&hour))
                } else {
                    usage_day_key_at_offset(row_timestamp, timezone_offset_minutes)
                        .is_some_and(|day| period_dates.contains(&day))
                };
                if !in_period {
                    continue;
                }
                let source_name = row
                    .get("source")
                    .and_then(|value| value.as_str())
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .unwrap_or("unknown")
                    .to_string();
                period_sources.entry(source_name).or_default().add_row(row, prices);
            }
            let source_rows = {
                let mut rows: Vec<_> = period_sources.into_iter().collect();
                rows.sort_by(|a, b| b.1.total_tokens().cmp(&a.1.total_tokens()).then(a.0.cmp(&b.0)));
                rows.into_iter()
                    .map(|(source, totals)| serde_json::json!({"source": source, "totals": totals.to_json()}))
                    .collect::<Vec<_>>()
            };
            serde_json::json!({
                "days": period,
                "totals": period_total.to_json(),
                "models": model_totals,
                "sources": source_rows,
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
        "window_days": period_days,
        "totals": totals.to_json(),
        "daily": daily_totals,
        "hourly": hourly_totals,
        "models": model_rows.into_iter().map(|model| serde_json::json!({
            "model": model.model,
            "totals": model.totals.to_json(),
            "daily": days.iter().map(|item| {
                let empty = UsageTotals::default();
                let totals = model.daily.get(&item.date).unwrap_or(&empty);
                serde_json::json!({"date": item.date, "label": item.label, "totals": totals.to_json()})
            }).collect::<Vec<_>>(),
            "hourly": hours.iter().map(|item| {
                let empty = UsageTotals::default();
                let totals = model.hourly.get(&item.hour).unwrap_or(&empty);
                serde_json::json!({"hour": item.hour, "label": item.label, "totals": totals.to_json()})
            }).collect::<Vec<_>>(),
        })).collect::<Vec<_>>(),
        "sources": source_rows,
        "periods": periods,
    })
}

fn insight_days_at_offset(now: f64, window_days: usize, timezone_offset_minutes: i32) -> Vec<DailyUsage> {
    let adjusted_now = now as i64 - (timezone_offset_minutes as i64 * 60);
    let today = chrono::DateTime::<chrono::Utc>::from_timestamp(adjusted_now, 0)
        .unwrap_or_else(chrono::Utc::now)
        .date_naive();
    (0..window_days.clamp(1, INSIGHTS_MAX_DAYS))
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

fn insight_hours_at_offset(now: f64, timezone_offset_minutes: i32) -> Vec<HourlyUsage> {
    let adjusted_now = now as i64 - (timezone_offset_minutes as i64 * 60);
    let current_hour = adjusted_now.div_euclid(3600) * 3600;
    (0..INSIGHTS_HOURS)
        .rev()
        .filter_map(|offset| {
            chrono::DateTime::<chrono::Utc>::from_timestamp(
                current_hour - (offset as i64 * 3600),
                0,
            )
            .map(|hour| HourlyUsage {
                hour: hour.format("%Y-%m-%dT%H:00:00Z").to_string(),
                label: hour.format("%H:00").to_string(),
            })
        })
        .collect()
}

fn unix_now_seconds() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::ZERO)
        .as_secs_f64()
}

fn usage_day_key_at_offset(ts: f64, timezone_offset_minutes: i32) -> Option<String> {
    let adjusted = ts as i64 - (timezone_offset_minutes as i64 * 60);
    chrono::DateTime::<chrono::Utc>::from_timestamp(adjusted, 0)
        .map(|date| date.date_naive().format("%Y-%m-%d").to_string())
}

fn usage_hour_key_at_offset(ts: f64, timezone_offset_minutes: i32) -> Option<String> {
    let adjusted = ts as i64 - (timezone_offset_minutes as i64 * 60);
    chrono::DateTime::<chrono::Utc>::from_timestamp(adjusted, 0)
        .map(|date| date.format("%Y-%m-%dT%H:00:00Z").to_string())
}

fn session_usage_timestamp(row: &serde_json::Value) -> f64 {
    let started_at = json_timestamp(row, "started_at");
    if started_at > 0.0 {
        started_at
    } else {
        json_timestamp(row, "last_active").max(json_timestamp(row, "ended_at"))
    }
}

fn json_timestamp(row: &serde_json::Value, key: &str) -> f64 {
    row.get(key)
        .and_then(|value| {
            value.as_f64().or_else(|| {
                let text = value.as_str()?;
                text.parse::<f64>().ok().or_else(|| {
                    chrono::DateTime::parse_from_rfc3339(text)
                        .ok()
                        .map(|timestamp| timestamp.timestamp_millis() as f64 / 1000.0)
                })
            })
        })
        .unwrap_or(0.0)
}

fn json_i64(row: &serde_json::Value, key: &str) -> i64 {
    row.get(key)
        .and_then(|value| value.as_i64().or_else(|| value.as_f64().map(|v| v as i64)))
        .unwrap_or(0)
}

fn json_f64(row: &serde_json::Value, key: &str) -> f64 {
    row.get(key).and_then(|value| value.as_f64()).unwrap_or(0.0)
}
