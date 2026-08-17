const INSIGHTS_DEFAULT_DAYS: usize = 7;
const INSIGHTS_MAX_DAYS: usize = 30;
const INSIGHTS_HOURS: usize = 24;
const INSIGHTS_PAGE_SIZE: usize = 200;
const INSIGHTS_SCAN_LIMIT: usize = 5_000;
const INSIGHTS_REQUEST_TIMEOUT: Duration = Duration::from_secs(10);
const INSIGHTS_ACTIVITY_OVERLAP_SECONDS: f64 = 60.0;
const INSIGHTS_MESSAGE_ID_OVERLAP: i64 = 100;
const INSIGHTS_BASELINE_CLEANUP_SECONDS: f64 = 7.0 * 86_400.0;
const MODELS_DEV_API_URL: &str = "https://models.dev/api.json";
const MODEL_PRICE_CACHE_TTL: Duration = Duration::from_secs(6 * 60 * 60);
const INSIGHTS_SNAPSHOT_INTERVAL: Duration = Duration::from_secs(5 * 60);
const INSIGHTS_SNAPSHOT_RETENTION_SECONDS: f64 = 35.0 * 86_400.0;
const INSIGHTS_SNAPSHOT_DB: &str = "state/yahu-insights-usage.db";
const INSIGHTS_PROVIDER_BACKFILL_VERSION: f64 = 3.0;
const INSIGHTS_HISTORICAL_BACKFILL_VERSION: f64 = 1.0;
const INSIGHTS_USAGE_SCHEMA_VERSION: f64 = 3.0;

type ModelPriceCatalog = HashMap<String, ModelPrice>;

fn default_provider() -> String {
    "unknown".to_string()
}

fn custom_provider_aliases(hermes_home: &Path) -> HashMap<String, String> {
    let config_path = hermes_home.join("config.yaml");
    let Ok(raw) = std::fs::read_to_string(config_path) else {
        return HashMap::new();
    };
    let Ok(config) = serde_yaml::from_str::<serde_yaml::Value>(&raw) else {
        return HashMap::new();
    };
    let Some(providers) = config.get("custom_providers").and_then(|value| value.as_sequence()) else {
        return HashMap::new();
    };
    let mut aliases = HashMap::new();
    let mut ambiguous = HashSet::new();
    for provider in providers {
        let Some(name) = provider.get("name").and_then(|value| value.as_str()).map(str::trim).filter(|value| !value.is_empty()) else {
            continue;
        };
        let Some(base_url) = provider.get("base_url").and_then(|value| value.as_str()).map(str::trim).filter(|value| !value.is_empty()) else {
            continue;
        };
        let key = base_url.trim_end_matches('/').to_ascii_lowercase();
        if ambiguous.contains(&key) {
            continue;
        }
        if aliases.get(&key).is_some_and(|existing| existing != name) {
            aliases.remove(&key);
            ambiguous.insert(key);
        } else {
            aliases.insert(key, name.to_string());
        }
    }
    aliases
}

fn resolve_provider_name(
    provider: &str,
    base_url: Option<&str>,
    aliases: &HashMap<String, String>,
) -> String {
    let provider = provider.trim();
    if provider.is_empty() {
        return "unknown".to_string();
    }
    if let Some(name) = provider.strip_prefix("custom:").map(str::trim).filter(|value| !value.is_empty()) {
        return name.to_string();
    }
    if provider == "custom"
        && let Some(name) = base_url
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .and_then(|value| aliases.get(&value.trim_end_matches('/').to_ascii_lowercase()))
    {
        return name.clone();
    }
    provider.to_string()
}

fn resolve_session_provider(
    billing_provider: &str,
    billing_base_url: Option<&str>,
    model_config: Option<&str>,
    aliases: &HashMap<String, String>,
) -> String {
    if let Some(config) = model_config.and_then(|raw| serde_json::from_str::<serde_json::Value>(raw).ok()) {
        let runtime = config.get("gateway_runtime");
        let provider = runtime
            .and_then(|value| value.get("provider"))
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty());
        let base_url = runtime
            .and_then(|value| value.get("base_url"))
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty());
        if let Some(provider) = provider {
            return resolve_provider_name(provider, base_url, aliases);
        }
    }
    resolve_provider_name(billing_provider, billing_base_url, aliases)
}

fn session_provider_labels(state_db_path: &Path, aliases: &HashMap<String, String>) -> HashMap<String, String> {
    let Ok(conn) = rusqlite::Connection::open_with_flags(
        state_db_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    ) else {
        return HashMap::new();
    };
    let Ok(mut statement) = conn.prepare("SELECT id, billing_provider, billing_base_url, model_config FROM sessions") else {
        return HashMap::new();
    };
    let Ok(rows) = statement.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, Option<String>>(1)?,
            row.get::<_, Option<String>>(2)?,
            row.get::<_, Option<String>>(3)?,
        ))
    }) else {
        return HashMap::new();
    };
    rows.filter_map(Result::ok)
        .map(|(id, provider, base_url, model_config)| {
            (
                id,
                resolve_session_provider(provider.as_deref().unwrap_or("unknown"), base_url.as_deref(), model_config.as_deref(), aliases),
            )
        })
        .collect()
}

#[derive(Clone, Default, Deserialize, Serialize)]
struct UsageCounter {
    session_id: String,
    #[serde(default)]
    root_session_id: String,
    model: String,
    #[serde(default = "default_provider")]
    provider: String,
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
            session_id: session_id.clone(),
            root_session_id: row
                .get("root_session_id")
                .and_then(serde_json::Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .unwrap_or(&session_id)
                .to_string(),
            model: row
                .get("model")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("unknown")
                .trim()
                .to_string(),
            provider: row
                .get("provider")
                .and_then(serde_json::Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .unwrap_or("unknown")
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
            root_session_id: self.root_session_id.clone(),
            model: self.model.clone(),
            provider: self.provider.clone(),
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
            root_session_id: self.root_session_id.clone(),
            model: self.model.clone(),
            provider: self.provider.clone(),
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
            "root_session_id": self.root_session_id,
            "source": self.source,
            "model": self.model,
            "provider": self.provider,
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

fn backfill_snapshot_providers(snapshot_path: &Path, state_db_path: &Path) -> anyhow::Result<usize> {
    if !snapshot_path.exists() || !state_db_path.exists() {
        return Ok(0);
    }

    let state_conn = rusqlite::Connection::open_with_flags(
        state_db_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    state_conn.busy_timeout(Duration::from_secs(5))?;
    let aliases = custom_provider_aliases(state_db_path.parent().unwrap_or_else(|| Path::new(".")));
    let mut session_providers = HashMap::new();
    let mut model_providers: HashMap<String, HashSet<String>> = HashMap::new();
    let mut statement = state_conn.prepare(
        "SELECT id, COALESCE(model, 'unknown'), billing_provider, billing_base_url, model_config
         FROM sessions
         WHERE (billing_provider IS NOT NULL AND TRIM(billing_provider) != '') OR model_config IS NOT NULL",
    )?;
    let state_rows = statement.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, Option<String>>(3)?,
            row.get::<_, Option<String>>(4)?,
        ))
    })?;
    for row in state_rows {
        let (session_id, model, provider, base_url, model_config) = row?;
        let provider = resolve_session_provider(&provider, base_url.as_deref(), model_config.as_deref(), &aliases);
        if provider == "unknown" {
            continue;
        }
        session_providers.insert(session_id, provider.clone());
        model_providers.entry(model).or_default().insert(provider);
    }
    drop(statement);
    drop(state_conn);

    let conn = rusqlite::Connection::open(snapshot_path)?;
    prepare_insights_snapshot_db(&conn)?;
    let already_backfilled = conn
        .query_row(
            "SELECT value FROM insights_meta WHERE key = 'provider_backfill_version'",
            [],
            |row| row.get::<_, f64>(0),
        )
        .optional()?;
    if already_backfilled.is_some_and(|version| version >= INSIGHTS_PROVIDER_BACKFILL_VERSION) {
        return Ok(0);
    }

    let tx = conn.unchecked_transaction()?;
    let mut changed = 0usize;
    for (table, column) in [
        ("insights_events", "row_json"),
        ("insights_baselines", "counters_json"),
        ("insights_initial_baselines", "counters_json"),
    ] {
        let query = format!("SELECT rowid, {column} FROM {table}");
        let mut rows = tx.prepare(&query)?;
        let values = rows
            .query_map([], |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?)))?
            .collect::<Result<Vec<_>, _>>()?;
        drop(rows);
        let update = format!("UPDATE {table} SET {column} = ?1 WHERE rowid = ?2");
        for (rowid, raw_json) in values {
            let Ok(mut value) = serde_json::from_str::<serde_json::Value>(&raw_json) else {
                continue;
            };
            let Some(session_id) = value
                .get("id")
                .or_else(|| value.get("session_id"))
                .and_then(serde_json::Value::as_str) else {
                continue;
            };
            if value.get("root_session_id").is_some() {
                continue;
            }
            let model = value
                .get("model")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("unknown");
            let provider = session_providers
                .get(session_id)
                .cloned()
                .or_else(|| {
                    model_providers
                        .get(model)
                        .filter(|providers| providers.len() == 1)
                        .and_then(|providers| providers.iter().next().cloned())
                })
                .unwrap_or_else(|| "unknown".to_string());
            if value.get("provider").and_then(serde_json::Value::as_str) == Some(provider.as_str()) {
                continue;
            }
            value["provider"] = serde_json::Value::String(provider);
            tx.execute(&update, rusqlite::params![serde_json::to_string(&value)?, rowid])?;
            changed += 1;
        }
    }
    tx.execute(
        "INSERT INTO insights_meta(key, value) VALUES('provider_backfill_version', ?1)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        [INSIGHTS_PROVIDER_BACKFILL_VERSION],
    )?;
    tx.commit()?;
    Ok(changed)
}

#[cfg(test)]
fn persist_insights_snapshot(
    path: &Path,
    captured_at: f64,
    rows: &[serde_json::Value],
) -> anyhow::Result<()> {
    persist_insights_snapshot_with_message_cursor(path, captured_at, rows, None)
}

fn persist_insights_snapshot_with_message_cursor(
    path: &Path,
    captured_at: f64,
    rows: &[serde_json::Value],
    last_message_id: Option<i64>,
) -> anyhow::Result<()> {
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
    tx.execute(
        "INSERT INTO insights_meta(key, value) VALUES('usage_schema_version', ?1)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        [INSIGHTS_USAGE_SCHEMA_VERSION],
    )?;

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
                    baseline.provider.clone_from(&current.provider);
                    baseline.source.clone_from(&current.source);
                    baseline
                })
            } else {
                Some(UsageCounter {
                    session_id: current.session_id.clone(),
                    model: current.model.clone(),
                    provider: current.provider.clone(),
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
    tx.execute(
        "INSERT INTO insights_meta(key, value) VALUES('last_captured_at', ?1)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        [captured_at],
    )?;
    if let Some(last_message_id) = last_message_id {
        tx.execute(
            "INSERT INTO insights_meta(key, value) VALUES('last_message_id', ?1)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            [last_message_id as f64],
        )?;
    }
    tx.commit()?;
    Ok(())
}

fn cleanup_deleted_insights_baselines(
    snapshot_path: &Path,
    state_db_path: &Path,
    now: f64,
) -> anyhow::Result<usize> {
    let mut snapshot_conn = rusqlite::Connection::open(snapshot_path)?;
    prepare_insights_snapshot_db(&snapshot_conn)?;
    let last_cleanup_at = snapshot_conn
        .query_row(
            "SELECT value FROM insights_meta WHERE key = 'last_baseline_cleanup_at'",
            [],
            |row| row.get::<_, f64>(0),
        )
        .optional()?;
    if last_cleanup_at.is_some_and(|last_cleanup_at| {
        let age = now - last_cleanup_at;
        (0.0..INSIGHTS_BASELINE_CLEANUP_SECONDS).contains(&age)
    }) {
        return Ok(0);
    }

    let stale_cutoff = now - INSIGHTS_BASELINE_CLEANUP_SECONDS;
    let stale_session_ids = {
        let mut statement = snapshot_conn
            .prepare("SELECT session_id, counters_json FROM insights_baselines WHERE last_seen < ?1")?;
        statement
            .query_map([stale_cutoff], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?
            .collect::<Result<Vec<_>, _>>()?
    };
    let state_conn = rusqlite::Connection::open_with_flags(
        state_db_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    state_conn.busy_timeout(Duration::from_secs(5))?;
    let mut exists_statement =
        state_conn.prepare("SELECT EXISTS(SELECT 1 FROM sessions WHERE id = ?1)")?;
    let mut deleted_session_ids = Vec::new();
    for (storage_id, counters_json) in stale_session_ids {
        let root_session_id = serde_json::from_str::<UsageCounter>(&counters_json)
            .map(|counter| counter.root_session_id)
            .ok()
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| storage_id.clone());
        let exists = exists_statement.query_row([&root_session_id], |row| row.get::<_, bool>(0))?;
        if !exists {
            deleted_session_ids.push(storage_id);
        }
    }
    drop(exists_statement);
    drop(state_conn);

    let tx = snapshot_conn.transaction()?;
    for session_id in &deleted_session_ids {
        tx.execute("DELETE FROM insights_baselines WHERE session_id = ?1", [session_id])?;
        tx.execute(
            "DELETE FROM insights_initial_baselines WHERE session_id = ?1",
            [session_id],
        )?;
    }
    tx.execute(
        "INSERT INTO insights_meta(key, value) VALUES('last_baseline_cleanup_at', ?1)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        [now],
    )?;
    tx.commit()?;
    Ok(deleted_session_ids.len())
}

fn load_insights_usage_rows(
    path: &Path,
    min_timestamp: f64,
) -> anyhow::Result<(Vec<serde_json::Value>, Option<f64>, Option<f64>)> {
    if !path.exists() {
        return Ok((Vec::new(), None, None));
    }
    let conn = rusqlite::Connection::open(path)?;
    prepare_insights_snapshot_db(&conn)?;
    let schema_version = conn
        .query_row(
            "SELECT value FROM insights_meta WHERE key = 'usage_schema_version'",
            [],
            |row| row.get::<_, f64>(0),
        )
        .optional()?;
    if !schema_version.is_some_and(|value| value >= INSIGHTS_USAGE_SCHEMA_VERSION) {
        return Ok((Vec::new(), None, None));
    }
    let coverage_started_at = conn
        .query_row(
            "SELECT value FROM insights_meta WHERE key = 'coverage_started_at'",
            [],
            |row| row.get::<_, f64>(0),
        )
        .optional()?;
    let latest_snapshot_at = match conn
        .query_row(
            "SELECT value FROM insights_meta WHERE key = 'last_captured_at'",
            [],
            |row| row.get::<_, f64>(0),
        )
        .optional()?
    {
        Some(value) => Some(value),
        None => conn.query_row(
            "SELECT MAX(captured_at) FROM insights_baselines",
            [],
            |row| row.get::<_, Option<f64>>(0),
        )?,
    };
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
    Ok((usage_rows, coverage_started_at, latest_snapshot_at))
}

#[derive(Default)]
struct InsightsCaptureCursor {
    last_captured_at: Option<f64>,
    last_message_id: Option<i64>,
}

fn load_insights_capture_cursor(path: &Path) -> anyhow::Result<InsightsCaptureCursor> {
    if !path.exists() {
        return Ok(InsightsCaptureCursor::default());
    }
    let conn = rusqlite::Connection::open(path)?;
    prepare_insights_snapshot_db(&conn)?;
    let schema_version = conn
        .query_row(
            "SELECT value FROM insights_meta WHERE key = 'usage_schema_version'",
            [],
            |row| row.get::<_, f64>(0),
        )
        .optional()?;
    if !schema_version.is_some_and(|value| value >= INSIGHTS_USAGE_SCHEMA_VERSION) {
        conn.execute_batch(
            "DELETE FROM insights_events;
             DELETE FROM insights_baselines;
             DELETE FROM insights_initial_baselines;
             DELETE FROM insights_meta;",
        )?;
        conn.execute(
            "INSERT INTO insights_meta(key, value) VALUES('usage_schema_version', ?1)",
            [INSIGHTS_USAGE_SCHEMA_VERSION],
        )?;
        return Ok(InsightsCaptureCursor::default());
    }
    let last_captured_at = conn
        .query_row(
            "SELECT value FROM insights_meta WHERE key = 'last_captured_at'",
            [],
            |row| row.get::<_, f64>(0),
        )
        .optional()?;
    let last_message_id = conn
        .query_row(
            "SELECT CAST(value AS INTEGER) FROM insights_meta WHERE key = 'last_message_id'",
            [],
            |row| row.get::<_, i64>(0),
        )
        .optional()?;
    Ok(InsightsCaptureCursor {
        last_captured_at,
        last_message_id,
    })
}

fn usage_counter_id(session_id: &str, model: &str, provider: &str, base_url: &str, billing_mode: &str, task: &str) -> String {
    format!("usage:{session_id}\u{1f}{model}\u{1f}{provider}\u{1f}{base_url}\u{1f}{billing_mode}\u{1f}{task}")
}

fn insights_usage_row_from_sql(
    row: &rusqlite::Row<'_>,
    has_model_usage: bool,
    aliases: &HashMap<String, String>,
) -> rusqlite::Result<serde_json::Value> {
    let session_id = row.get::<_, String>(0)?;
    let model = row.get::<_, String>(3)?;
    let provider_raw = row.get::<_, String>(4)?;
    let base_url = row.get::<_, Option<String>>(5)?;
    let billing_mode = row.get::<_, String>(6)?;
    let model_config = row.get::<_, Option<String>>(7)?;
    let task = row.get::<_, String>(8)?;
    let provider = if has_model_usage {
        resolve_provider_name(&provider_raw, base_url.as_deref(), aliases)
    } else {
        resolve_session_provider(&provider_raw, base_url.as_deref(), model_config.as_deref(), aliases)
    };
    let id = if has_model_usage {
        usage_counter_id(&session_id, &model, &provider, base_url.as_deref().unwrap_or(""), &billing_mode, &task)
    } else {
        session_id.clone()
    };
    Ok(serde_json::json!({
        "id": id,
        "root_session_id": session_id,
        "source": row.get::<_, String>(1)?,
        "model": model,
        "provider": provider,
        "started_at": row.get::<_, f64>(2)?,
        "input_tokens": row.get::<_, i64>(9)?,
        "output_tokens": row.get::<_, i64>(10)?,
        "cache_read_tokens": row.get::<_, i64>(11)?,
        "cache_write_tokens": row.get::<_, i64>(12)?,
        "reasoning_tokens": row.get::<_, i64>(13)?,
        "api_call_count": row.get::<_, i64>(14)?,
        "tool_call_count": 0,
        "estimated_cost_usd": row.get::<_, f64>(15)?,
        "actual_cost_usd": row.get::<_, f64>(16)?,
    }))
}

fn fetch_changed_sessions_for_insights(
    path: &Path,
    previous_message_id: i64,
) -> anyhow::Result<(Vec<serde_json::Value>, i64)> {
    let conn = rusqlite::Connection::open_with_flags(
        path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    conn.busy_timeout(Duration::from_secs(5))?;
    let high_water = conn.query_row("SELECT COALESCE(MAX(id), 0) FROM messages", [], |row| {
        row.get::<_, i64>(0)
    })?;
    let lower_bound = previous_message_id
        .saturating_sub(INSIGHTS_MESSAGE_ID_OVERLAP)
        .max(0);
    let aliases = custom_provider_aliases(path.parent().unwrap_or_else(|| Path::new(".")));
    let has_model_usage = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'session_model_usage')",
        [],
        |row| row.get::<_, bool>(0),
    )?;
    let query = if has_model_usage {
        "WITH changed(session_id) AS (
             SELECT DISTINCT session_id FROM messages WHERE id > ?1 AND id <= ?2
         )
         SELECT s.id, s.source, s.started_at,
                COALESCE(u.model, 'unknown'), COALESCE(u.billing_provider, ''),
                u.billing_base_url, COALESCE(u.billing_mode, ''), NULL, COALESCE(u.task, ''),
                COALESCE(u.input_tokens, 0), COALESCE(u.output_tokens, 0),
                COALESCE(u.cache_read_tokens, 0), COALESCE(u.cache_write_tokens, 0),
                COALESCE(u.reasoning_tokens, 0), COALESCE(u.api_call_count, 0),
                COALESCE(u.estimated_cost_usd, 0), COALESCE(u.actual_cost_usd, 0)
         FROM sessions s
         JOIN changed c ON c.session_id = s.id
         JOIN session_model_usage u ON u.session_id = s.id
         WHERE s.archived = 0 AND s.source != 'tool'
           AND COALESCE(s.end_reason, '') != 'compression'"
    } else {
        "WITH changed(session_id) AS (
             SELECT DISTINCT session_id FROM messages WHERE id > ?1 AND id <= ?2
         )
         SELECT s.id, s.source, s.started_at,
                COALESCE(s.model, 'unknown'), COALESCE(s.billing_provider, ''),
                s.billing_base_url, '', s.model_config, '',
                COALESCE(s.input_tokens, 0), COALESCE(s.output_tokens, 0),
                COALESCE(s.cache_read_tokens, 0), COALESCE(s.cache_write_tokens, 0),
                COALESCE(s.reasoning_tokens, 0), COALESCE(s.api_call_count, 0),
                COALESCE(s.estimated_cost_usd, 0), COALESCE(s.actual_cost_usd, 0)
         FROM sessions s
         JOIN changed c ON c.session_id = s.id
         WHERE s.archived = 0 AND s.source != 'tool'
           AND COALESCE(s.end_reason, '') != 'compression'"
    };
    let mut statement = conn.prepare(query)?;
    let rows = statement
        .query_map(rusqlite::params![lower_bound, high_water], |row| {
            insights_usage_row_from_sql(row, has_model_usage, &aliases)
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok((rows, high_water))
}

fn fetch_historical_sessions_for_insights(
    path: &Path,
    min_timestamp: f64,
    max_timestamp: f64,
) -> anyhow::Result<Vec<serde_json::Value>> {
    let conn = rusqlite::Connection::open_with_flags(
        path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    conn.busy_timeout(Duration::from_secs(5))?;
    let aliases = custom_provider_aliases(path.parent().unwrap_or_else(|| Path::new(".")));
    let has_model_usage = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'session_model_usage')",
        [],
        |row| row.get::<_, bool>(0),
    )?;
    let query = if has_model_usage {
        "SELECT s.id, s.source, s.started_at,
                COALESCE(u.model, 'unknown'), COALESCE(u.billing_provider, ''),
                u.billing_base_url, COALESCE(u.billing_mode, ''), NULL, COALESCE(u.task, ''),
                COALESCE(u.input_tokens, 0), COALESCE(u.output_tokens, 0),
                COALESCE(u.cache_read_tokens, 0), COALESCE(u.cache_write_tokens, 0),
                COALESCE(u.reasoning_tokens, 0), COALESCE(u.api_call_count, 0),
                COALESCE(u.estimated_cost_usd, 0), COALESCE(u.actual_cost_usd, 0)
         FROM sessions s
         JOIN session_model_usage u ON u.session_id = s.id
         WHERE s.started_at >= ?1 AND s.started_at < ?2
           AND s.archived = 0 AND s.source != 'tool'
           AND COALESCE(s.end_reason, '') != 'compression'"
    } else {
        "SELECT s.id, s.source, s.started_at,
                COALESCE(s.model, 'unknown'), COALESCE(s.billing_provider, ''),
                s.billing_base_url, '', s.model_config, '',
                COALESCE(s.input_tokens, 0), COALESCE(s.output_tokens, 0),
                COALESCE(s.cache_read_tokens, 0), COALESCE(s.cache_write_tokens, 0),
                COALESCE(s.reasoning_tokens, 0), COALESCE(s.api_call_count, 0),
                COALESCE(s.estimated_cost_usd, 0), COALESCE(s.actual_cost_usd, 0)
         FROM sessions s
         WHERE s.started_at >= ?1 AND s.started_at < ?2
           AND s.archived = 0 AND s.source != 'tool'
           AND COALESCE(s.end_reason, '') != 'compression'"
    };
    let mut statement = conn.prepare(query)?;
    statement
        .query_map(rusqlite::params![min_timestamp, max_timestamp], |row| {
            insights_usage_row_from_sql(row, has_model_usage, &aliases)
        })?
        .collect::<Result<Vec<_>, _>>()
        .map_err(Into::into)
}

fn backfill_historical_insights(
    snapshot_path: &Path,
    state_db_path: &Path,
) -> anyhow::Result<usize> {
    if !snapshot_path.exists() || !state_db_path.exists() {
        return Ok(0);
    }
    let mut snapshot_conn = rusqlite::Connection::open(snapshot_path)?;
    prepare_insights_snapshot_db(&snapshot_conn)?;
    let tx = snapshot_conn.transaction()?;
    let already_backfilled = tx
        .query_row(
            "SELECT value FROM insights_meta WHERE key = 'historical_backfill_version'",
            [],
            |row| row.get::<_, f64>(0),
        )
        .optional()?
        .is_some_and(|version| version >= INSIGHTS_HISTORICAL_BACKFILL_VERSION);
    if already_backfilled {
        return Ok(0);
    }
    let Some(coverage_started_at) = tx
        .query_row(
            "SELECT value FROM insights_meta WHERE key = 'coverage_started_at'",
            [],
            |row| row.get::<_, f64>(0),
        )
        .optional()?
    else {
        return Ok(0);
    };

    let mut recorded_deltas: HashMap<String, UsageCounter> = HashMap::new();
    let mut statement = tx.prepare("SELECT row_json FROM insights_events")?;
    let event_rows = statement
        .query_map([], |row| row.get::<_, String>(0))?
        .collect::<Result<Vec<_>, _>>()?;
    drop(statement);
    for value in event_rows {
        let Ok(json) = serde_json::from_str::<serde_json::Value>(&value) else { continue };
        let Some(counter) = UsageCounter::from_api_row(&json) else { continue };
        recorded_deltas
            .entry(counter.session_id.clone())
            .or_default()
            .add_assign(&counter);
    }

    let historical_rows = fetch_historical_sessions_for_insights(
        state_db_path,
        coverage_started_at - INSIGHTS_SNAPSHOT_RETENTION_SECONDS,
        coverage_started_at,
    )?;
    let mut inserted = 0usize;
    for row in historical_rows {
        let Some(current) = UsageCounter::from_api_row(&row) else { continue };
        let recorded = recorded_deltas.get(&current.session_id).cloned().unwrap_or_default();
        let historical = current.subtract(&recorded);
        if !historical.has_delta() {
            continue;
        }
        let event = historical.to_event_json(current.started_at);
        tx.execute(
            "INSERT INTO insights_events(captured_at, row_json) VALUES(?1, ?2)",
            rusqlite::params![current.started_at, serde_json::to_string(&event)?],
        )?;
        inserted += 1;
    }
    tx.execute(
        "INSERT INTO insights_meta(key, value) VALUES('historical_backfill_version', ?1)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        [INSIGHTS_HISTORICAL_BACKFILL_VERSION],
    )?;
    tx.commit()?;
    Ok(inserted)
}

fn insights_snapshot_is_fresh(latest_snapshot_at: Option<f64>, now: f64) -> bool {
    latest_snapshot_at.is_some_and(|captured_at| {
        let age = now - captured_at;
        age >= 0.0 && age < INSIGHTS_SNAPSHOT_INTERVAL.as_secs_f64()
    })
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
    provider: String,
    totals: UsageTotals,
    daily: BTreeMap<String, UsageTotals>,
    hourly: BTreeMap<String, UsageTotals>,
}

#[derive(Default, Deserialize)]
struct InsightsUsageQuery {
    period: Option<usize>,
    days: Option<usize>,
    tz_offset: Option<i32>,
    refresh: Option<bool>,
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
    let force_refresh = query.refresh.unwrap_or(false);
    if force_refresh
        && let Err(err) = refresh_insights_snapshot_and_wait(state.clone(), now).await
    {
        warn!("explicit insights snapshot refresh failed: {err}");
    }
    let snapshot_path = state.hermes_home.join(INSIGHTS_SNAPSHOT_DB);
    let backfill_snapshot_path = snapshot_path.clone();
    let backfill_state_path = state.hermes_home.join("state.db");
    match tokio::task::spawn_blocking(move || {
        backfill_snapshot_providers(&backfill_snapshot_path, &backfill_state_path)
    })
    .await
    {
        Ok(Ok(changed)) if changed > 0 => info!("backfilled providers in {changed} yahu Insights rows"),
        Ok(Ok(_)) => {}
        Ok(Err(err)) => warn!("Insights provider backfill failed: {err}"),
        Err(err) => warn!("Insights provider backfill task failed: {err}"),
    }
    let min_timestamp = now - (INSIGHTS_MAX_DAYS as f64 * 86_400.0);
    let snapshot_data = tokio::task::spawn_blocking(move || {
        load_insights_usage_rows(&snapshot_path, min_timestamp)
    })
    .await;
    let (rows, coverage_started_at, latest_snapshot_at) = match snapshot_data {
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
    if !force_refresh && !insights_snapshot_is_fresh(latest_snapshot_at, now) {
        start_insights_snapshot_refresh(state.clone(), now);
    }
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
    let path = state.hermes_home.join(INSIGHTS_SNAPSHOT_DB);
    let cursor_path = path.clone();
    let cursor =
        tokio::task::spawn_blocking(move || load_insights_capture_cursor(&cursor_path)).await??;
    let state_db_path = state.hermes_home.join("state.db");

    let (rows, last_message_id) = if let Some(previous_message_id) = cursor.last_message_id {
        let read_path = state_db_path.clone();
        let (rows, high_water) = tokio::task::spawn_blocking(move || {
            fetch_changed_sessions_for_insights(&read_path, previous_message_id)
        })
        .await??;
        (rows, Some(high_water))
    } else {
        if state_db_path.exists() {
            let read_path = state_db_path.clone();
            let (rows, high_water) = tokio::task::spawn_blocking(move || {
                fetch_changed_sessions_for_insights(&read_path, 0)
            })
            .await??;
            (rows, Some(high_water))
        } else {
            let activity_cutoff = cursor
                .last_captured_at
                .map(|value| (value - INSIGHTS_ACTIVITY_OVERLAP_SECONDS).max(0.0));
            (
                fetch_sessions_for_insights_snapshot(state, activity_cutoff).await?,
                None,
            )
        }
    };

    let persist_path = path.clone();
    tokio::task::spawn_blocking(move || {
        persist_insights_snapshot_with_message_cursor(
            &persist_path,
            captured_at,
            &rows,
            last_message_id,
        )
    })
    .await??;
    if state_db_path.exists() {
        let backfill_snapshot_path = path.clone();
        let backfill_state_path = state_db_path.clone();
        let backfilled = tokio::task::spawn_blocking(move || {
            backfill_historical_insights(&backfill_snapshot_path, &backfill_state_path)
        })
        .await??;
        if backfilled > 0 {
            info!("backfilled {backfilled} historical Insights usage rows from state.db");
        }
        tokio::task::spawn_blocking(move || {
            cleanup_deleted_insights_baselines(&path, &state_db_path, captured_at)
        })
        .await??;
    }
    Ok(())
}

fn start_insights_snapshot_refresh(state: Arc<AppState>, captured_at: f64) -> bool {
    let Ok(guard) = state.insights_snapshot_refresh.clone().try_lock_owned() else {
        return false;
    };
    tokio::spawn(async move {
        let _guard = guard;
        if let Err(err) = capture_insights_snapshot(&state, captured_at).await {
            warn!("insights snapshot capture failed: {err}");
        }
    });
    true
}

async fn refresh_insights_snapshot_and_wait(
    state: Arc<AppState>,
    captured_at: f64,
) -> anyhow::Result<()> {
    let refresh_lock = state.insights_snapshot_refresh.clone();
    match refresh_lock.clone().try_lock_owned() {
        Ok(guard) => {
            let _guard = guard;
            capture_insights_snapshot(&state, captured_at).await
        }
        Err(_) => {
            let _guard = refresh_lock.lock_owned().await;
            Ok(())
        }
    }
}

async fn run_insights_snapshot_collector(state: Arc<AppState>) {
    let mut ticker = interval(INSIGHTS_SNAPSHOT_INTERVAL);
    ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);
    loop {
        ticker.tick().await;
        start_insights_snapshot_refresh(state.clone(), unix_now_seconds());
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

async fn fetch_sessions_for_insights_snapshot(
    state: &AppState,
    activity_cutoff: Option<f64>,
) -> anyhow::Result<Vec<serde_json::Value>> {
    let provider_labels = session_provider_labels(
        &state.hermes_home.join("state.db"),
        &custom_provider_aliases(&state.hermes_home),
    );
    let mut rows = Vec::new();
    let mut offset = 0usize;
    loop {
        let url = api_sessions_url(
            state.api_url.trim_end_matches('/'),
            INSIGHTS_PAGE_SIZE,
            offset,
            "",
        )?;
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
        let mut data = body
            .get("data")
            .and_then(|value| value.as_array())
            .cloned()
            .unwrap_or_default();
        for row in &mut data {
            let Some(session_id) = row.get("id").and_then(serde_json::Value::as_str) else {
                continue;
            };
            let is_custom = row
                .get("provider")
                .and_then(serde_json::Value::as_str)
                .is_some_and(|provider| provider == "custom" || provider.starts_with("custom:"));
            if is_custom
                && let Some(provider) = provider_labels.get(session_id)
            {
                row["provider"] = serde_json::Value::String(provider.clone());
            }
        }
        let data_len = data.len();
        let reached_cutoff = activity_cutoff.is_some_and(|cutoff| {
            data.iter()
                .any(|row| session_activity_timestamp(row) < cutoff)
        });
        rows.extend(data.into_iter().filter(|row| {
            activity_cutoff
                .is_none_or(|cutoff| session_activity_timestamp(row) >= cutoff)
                && is_client_visible_session(row, false)
        }));
        let has_more = body
            .get("has_more")
            .and_then(|value| value.as_bool())
            .unwrap_or(data_len == INSIGHTS_PAGE_SIZE);
        offset = offset.saturating_add(INSIGHTS_PAGE_SIZE);
        if reached_cutoff || !has_more || data_len == 0 || offset >= INSIGHTS_SCAN_LIMIT {
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
    Ok(fetch_sessions_for_insights_snapshot(state, None)
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
    let mut models: HashMap<(String, String), ModelUsage> = HashMap::new();
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
        let provider_name = row
            .get("provider")
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
        let model = models.entry((model_name.clone(), provider_name.clone())).or_insert_with(|| ModelUsage {
            model: model_name.clone(),
            provider: provider_name,
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
    model_rows.sort_by(|a, b| {
        b.totals
            .total_tokens()
            .cmp(&a.totals.total_tokens())
            .then(a.model.cmp(&b.model))
            .then(a.provider.cmp(&b.provider))
    });

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
                    serde_json::json!({"model": model.model, "provider": model.provider, "totals": value.to_json()})
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
            "provider": model.provider,
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

fn session_activity_timestamp(row: &serde_json::Value) -> f64 {
    let last_active = json_timestamp(row, "last_active");
    if last_active > 0.0 {
        last_active
    } else {
        json_timestamp(row, "started_at")
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
