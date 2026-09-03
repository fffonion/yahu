// Provider usage panel: query external AI provider consoles for quota/usage
// data. Credentials are read from ~/.hermes/.env (and ~/.hermes/auth.json
// credential pools). Ported from ~/workspace/api-usage/token_usage.py.

use sha2::Digest;

const PROVIDER_USAGE_TTL: Duration = Duration::from_secs(30 * 60);
const PROVIDER_USAGE_TIMEOUT: Duration = Duration::from_secs(15);

const OPENROUTER_API_BASE: &str = "https://openrouter.ai/api/v1";
const DEEPSEEK_PLATFORM_BASE: &str = "https://platform.deepseek.com";
const ATLASCLOUD_BALANCE_URL: &str = "https://api.atlascloud.ai/public/v1/balance";
const ATLASCLOUD_MODEL_USAGE_URL: &str = "https://api.atlascloud.ai/public/v1/model-usage";
const ATLASCLOUD_MODEL_COSTS_URL: &str = "https://api.atlascloud.ai/public/v1/model-costs";
const MINIMAX_PLAN_REMAINS_URL: &str =
    "https://www.minimaxi.com/v1/api/openplatform/coding_plan/remains";
const MINIMAX_USAGE_SUMMARY_URL: &str =
    "https://www.minimaxi.com/backend/account/token_plan/usage_summary";
const KIMI_API_BASE: &str = "https://www.kimi.com";
const MIMO_API_BASE: &str = "https://platform.xiaomimimo.com/api/v1";
const COMMANDCODE_API_BASE: &str = "https://api.commandcode.ai";
const GROK_WEB_CREDITS_URL: &str =
    "https://grok.com/grok_api_v2.GrokBuildBilling/GetGrokCreditsConfig";
const XAI_OAUTH_TOKEN_URL: &str = "https://auth.x.ai/oauth2/token";
const XAI_OAUTH_CLIENT_ID: &str = "b1a00492-073a-47ea-816f-4c329264a828";
const ZED_CLOUD_ME_URL: &str = "https://cloud.zed.dev/client/users/me";

#[derive(Serialize, Deserialize, Clone, Default)]
struct ProviderUsageRow {
    label: String,
    hit_rate: Option<String>,
    input: Option<String>,
    output: Option<String>,
    cost_or_pct: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
struct ProviderUsageWindow {
    window: String,
    used: Option<String>,
    reset: Option<String>,
    #[serde(default)]
    reset_at: Option<i64>,
}

#[derive(Serialize, Deserialize, Clone, Default)]
struct ProviderUsageSection {
    provider: String,
    title: String,
    description: String,
    #[serde(default)]
    captured_at: f64,
    rows: Vec<ProviderUsageRow>,
    windows: Vec<ProviderUsageWindow>,
    errors: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Default)]
struct SharedProviderCacheEntry {
    cached_at: f64,
    section: ProviderUsageSection,
}

#[derive(Serialize, Deserialize, Clone, Default)]
struct SharedProviderCacheFile {
    entries: HashMap<String, SharedProviderCacheEntry>,
    #[serde(default)]
    commandcode_accounts: HashMap<String, CommandCodeAccountCache>,
}

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CommandCodeCachePlan {
    refresh_whoami: bool,
    refresh_subscription: bool,
    refresh_summary: bool,
}

#[derive(Serialize, Clone, Default)]
struct ProviderUsageProvider {
    provider: String,
    title: String,
    configured: bool,
    query_ready: bool,
    credential_hint: String,
    setup_hint: String,
}

#[derive(Serialize, Clone, Default)]
struct ProviderUsagePayload {
    fetched_at: f64,
    providers: Vec<ProviderUsageProvider>,
    sections: Vec<ProviderUsageSection>,
}

#[derive(Default)]
struct ProviderUsageCache {
    fetched_at: Arc<RwLock<Option<Instant>>>,
    payload: Arc<RwLock<Option<ProviderUsagePayload>>>,
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

#[derive(Clone)]
struct AuthCredentialEntry {
    label: String,
    access_token: String,
    refresh_token: Option<String>,
    user_id: Option<String>,
}

fn auth_json_credential_entries(
    hermes_home: &Path,
    provider: &str,
) -> Vec<AuthCredentialEntry> {
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
    let mut credentials: Vec<AuthCredentialEntry> = entries
        .iter()
        .enumerate()
        .filter_map(|(index, entry)| {
            let access_token = entry
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
            let refresh_token = entry
                .get("refresh_token")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|token| !token.is_empty())
                .map(str::to_string);
            let user_id = json_string_or_number(entry.get("user_id"));
            Some(AuthCredentialEntry {
                label,
                access_token: access_token.to_string(),
                refresh_token,
                user_id,
            })
        })
        .collect();
    credentials.sort_by_key(|entry| entry.label.to_lowercase());
    credentials
}

fn auth_json_credential_pool(
    hermes_home: &Path,
    provider: &str,
) -> Vec<(String, String)> {
    auth_json_credential_entries(hermes_home, provider)
        .into_iter()
        .map(|entry| (entry.label, entry.access_token))
        .collect()
}

fn any_provider_env_value(hermes_home: &Path, keys: &[&str]) -> bool {
    keys.iter()
        .any(|key| !provider_env_value(hermes_home, key).is_empty())
}

fn any_custom_provider_api_key(hermes_home: &Path, names: &[&str]) -> bool {
    names
        .iter()
        .any(|name| !config_provider_api_key(hermes_home, name).is_empty())
}

fn provider_usage_catalog(hermes_home: &Path) -> Vec<ProviderUsageProvider> {
    let commandcode_pool = !auth_json_credential_pool(hermes_home, "commandcode").is_empty();
    let codex_pool = !auth_json_credential_pool(hermes_home, "openai-codex").is_empty();
    let grok_pool = !auth_json_credential_pool(hermes_home, "xai-oauth").is_empty();
    let zed_pro_pool = !auth_json_credential_pool(hermes_home, "zed-pro").is_empty();
    let entries = [
        (
            "openrouter",
            "OpenRouter API 用量",
            any_provider_env_value(hermes_home, &["OPENROUTER_API_KEY", "OPENROUTER_MANAGEMENT_KEY"])
                || any_custom_provider_api_key(hermes_home, &["openrouter"]),
            !provider_env_value(hermes_home, "OPENROUTER_MANAGEMENT_KEY").is_empty(),
            "OPENROUTER_MANAGEMENT_KEY",
            "配置 OPENROUTER_MANAGEMENT_KEY；从 OpenRouter 控制台 Settings → Management Keys 创建。",
        ),
        (
            "deepseek",
            "DeepSeek 用量",
            any_provider_env_value(hermes_home, &["DEEPSEEK_API_KEY", "DEEPSEEK_COOKIE", "DEEPSEEK_TOKEN"])
                || any_custom_provider_api_key(hermes_home, &["deepseek"]),
            any_provider_env_value(hermes_home, &["DEEPSEEK_COOKIE"])
                && any_provider_env_value(hermes_home, &["DEEPSEEK_TOKEN"]),
            "DEEPSEEK_COOKIE + DEEPSEEK_TOKEN",
            "配置 DEEPSEEK_COOKIE 与 DEEPSEEK_TOKEN；登录 platform.deepseek.com 后，从浏览器开发者工具的已登录会话复制对应 Cookie 和认证请求字段。",
        ),
        (
            "atlascloud",
            "AtlasCloud 用量",
            any_provider_env_value(hermes_home, &["ATLASCLOUD_API_KEY"])
                || any_custom_provider_api_key(hermes_home, &["atlascloud"]),
            !provider_env_value(hermes_home, "ATLASCLOUD_API_KEY").is_empty()
                || !config_provider_api_key(hermes_home, "atlascloud").is_empty(),
            "ATLASCLOUD_API_KEY 或 config.yaml custom_providers.api_key",
            "配置 ATLASCLOUD_API_KEY，或在 config.yaml 的 custom_providers 中填写 atlascloud 的 api_key；从 AtlasCloud 控制台 API Keys 获取。",
        ),
        (
            "mimo",
            "MiMo 用量",
            any_provider_env_value(hermes_home, &["MIMO_API_KEY", "MIMO_COOKIE", "MIMO_PASSTOKEN"])
                || any_custom_provider_api_key(hermes_home, &["mimo"]),
            !provider_env_value(hermes_home, "MIMO_COOKIE").is_empty()
                || !provider_env_value(hermes_home, "MIMO_PASSTOKEN").is_empty(),
            "MiMo 认证信息",
            "配置 MiMo 平台会话，或提供 Xiaomi 账号认证种子；系统会自动换取 MiMo 会话后查询用量。",
        ),
        (
            "minimax",
            "MiniMax 用量",
            any_provider_env_value(hermes_home, &["MINIMAX_API_KEY", "MINIMAX_COOKIE", "MINIMAX_GROUP_ID"])
                || any_custom_provider_api_key(hermes_home, &["minimax"]),
            any_provider_env_value(hermes_home, &["MINIMAX_COOKIE"])
                && any_provider_env_value(hermes_home, &["MINIMAX_GROUP_ID"]),
            "MINIMAX_COOKIE + MINIMAX_GROUP_ID",
            "配置完整的 MINIMAX_COOKIE 与 MINIMAX_GROUP_ID；登录 minimaxi.com 后从浏览器开发者工具复制 Cookie，Group ID 可在开放平台账户页面查看。",
        ),
        (
            "kimi",
            "Kimi Code 用量",
            any_provider_env_value(hermes_home, &["KIMI_API_KEY", "KIMI_AUTH_TOKEN"])
                || any_custom_provider_api_key(hermes_home, &["kimi"]),
            !provider_env_value(hermes_home, "KIMI_AUTH_TOKEN").is_empty(),
            "KIMI_AUTH_TOKEN",
            "配置 KIMI_AUTH_TOKEN；登录 kimi.com 后从浏览器开发者工具复制已登录会话的认证 token。",
        ),
        (
            "opencode",
            "OpenCode Go 用量",
            any_provider_env_value(
                hermes_home,
                &[
                    "OPENCODE_GO_API_KEY",
                    "OPENCODE_GO_WORKSPACE_ID",
                    "OPENCODE_GO_AUTH_COOKIE",
                    "OPENCODE_WORKSPACE_ID",
                    "OPENCODE_AUTH_COOKIE",
                ],
            ) || any_custom_provider_api_key(hermes_home, &["opencode-go", "opencode"]),
            any_provider_env_value(hermes_home, &["OPENCODE_GO_WORKSPACE_ID", "OPENCODE_WORKSPACE_ID"])
                && any_provider_env_value(hermes_home, &["OPENCODE_GO_AUTH_COOKIE", "OPENCODE_AUTH_COOKIE"]),
            "OPENCODE_GO_WORKSPACE_ID + OPENCODE_GO_AUTH_COOKIE",
            "配置 OpenCode Go workspace ID 和 OPENCODE_GO_AUTH_COOKIE；workspace ID 来自 workspace 地址，Cookie 从已登录 OpenCode 会话复制。",
        ),
        (
            "commandcode",
            "CommandCode 用量",
            commandcode_pool
                || any_provider_env_value(hermes_home, &["COMMANDCODE_API_KEY"])
                || any_custom_provider_api_key(hermes_home, &["commandcode"]),
            commandcode_pool || !provider_env_value(hermes_home, "COMMANDCODE_API_KEY").is_empty(),
            "COMMANDCODE_API_KEY 或 auth.json credential_pool.commandcode",
            "配置 COMMANDCODE_API_KEY，或在 auth.json 的 credential_pool.commandcode 中加入账号凭据；API key 从 CommandCode 控制台创建。",
        ),
        (
            "codex",
            "Codex 用量",
            codex_pool
                || any_provider_env_value(hermes_home, &["OPENAI_API_KEY"])
                || any_custom_provider_api_key(hermes_home, &["openai-codex", "codex"]),
            codex_pool,
            "auth.json credential_pool.openai-codex",
            "配置 auth.json 的 credential_pool.openai-codex；通过 Hermes 的 Codex OAuth 登录流程获取账号凭据。",
        ),
        (
            "grok",
            "Grok 用量",
            grok_pool
                || any_provider_env_value(hermes_home, &["XAI_API_KEY"])
                || any_custom_provider_api_key(hermes_home, &["xai", "grok"]),
            grok_pool,
            "auth.json credential_pool.xai-oauth",
            "配置 auth.json 的 credential_pool.xai-oauth，保留 access_token 和 refresh_token；通过 Hermes 的 xAI OAuth 登录流程获取。",
        ),
        (
            "zed-pro",
            "Zed Pro 用量",
            zed_pro_pool,
            zed_pro_pool,
            "auth.json credential_pool.zed-pro",
            "配置 auth.json 的 credential_pool.zed-pro，保留 access_token 与 user_id；通过 Hermes 的 Zed 登录流程获取账号凭据。",
        ),
    ];
    entries
        .into_iter()
        .map(|(provider, title, configured, query_ready, credential_hint, setup_hint)| {
            ProviderUsageProvider {
                provider: provider.into(),
                title: title.into(),
                configured,
                query_ready,
                credential_hint: credential_hint.into(),
                setup_hint: setup_hint.into(),
            }
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

async fn provider_http_get_text(
    client: &reqwest::Client,
    url: &str,
    headers: &[(String, String)],
) -> Result<String, String> {
    let mut request = client.get(url).timeout(PROVIDER_USAGE_TIMEOUT);
    for (name, value) in headers {
        request = request.header(name.as_str(), value.as_str());
    }
    let response = request.send().await.map_err(|err| err.to_string())?;
    let status = response.status();
    let body = response.text().await.map_err(|err| err.to_string())?;
    if !status.is_success() {
        return Err(format!("HTTP {status}"));
    }
    Ok(body)
}

async fn provider_http_get_json_retry(
    client: &reqwest::Client,
    url: &str,
    headers: &[(String, String)],
) -> Result<Value, String> {
    match provider_http_get_json(client, url, headers).await {
        Ok(value) => Ok(value),
        Err(first_error) => {
            tokio::time::sleep(Duration::from_millis(300)).await;
            provider_http_get_json(client, url, headers)
                .await
                .map_err(|second_error| format!("{first_error}; retry: {second_error}"))
        }
    }
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

async fn provider_http_post_bytes(
    client: &reqwest::Client,
    url: &str,
    body: &[u8],
    headers: &[(String, String)],
) -> Result<Vec<u8>, String> {
    let mut request = client
        .post(url)
        .body(body.to_vec())
        .timeout(PROVIDER_USAGE_TIMEOUT);
    for (name, value) in headers {
        request = request.header(name.as_str(), value.as_str());
    }
    let response = request.send().await.map_err(|err| err.to_string())?;
    let status = response.status();
    let body = response.bytes().await.map_err(|err| err.to_string())?;
    if !status.is_success() {
        return Err(format!("HTTP {status}"));
    }
    Ok(body.to_vec())
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

fn provider_reset_duration(seconds: f64) -> String {
    let minutes = ((seconds.max(0.0) + 59.0) / 60.0).floor() as i64;
    if minutes < 60 {
        format!("{minutes}分钟")
    } else {
        let hours = (minutes + 59) / 60;
        if hours > 24 {
            format!("{}天", (hours + 23) / 24)
        } else {
            format!("{hours}小时")
        }
    }
}

fn provider_reset_text_local(value: i64) -> String {
    if value <= 0 {
        return "-".to_string();
    }
    // CommandCode returns reset_at in milliseconds while most providers use
    // Unix seconds. Normalize the millisecond form before subtracting now.
    let timestamp = if value >= 100_000_000_000 {
        value / 1000
    } else {
        value
    };
    let seconds = timestamp.saturating_sub(chrono::Utc::now().timestamp()) as f64;
    provider_reset_duration(seconds)
}

fn provider_number(value: &Value) -> Option<f64> {
    value
        .as_f64()
        .or_else(|| value.as_str()?.trim().parse::<f64>().ok())
        .filter(|number| number.is_finite())
}

fn json_string_or_number(value: Option<&Value>) -> Option<String> {
    let value = value?;
    if let Some(text) = value.as_str().map(str::trim).filter(|text| !text.is_empty()) {
        return Some(text.to_string());
    }
    if let Some(number) = value.as_i64() {
        return Some(number.to_string());
    }
    value.as_u64().map(|number| number.to_string())
}

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

fn minimax_used_percent(model: &Value, period: &str) -> Option<f64> {
    let (total_key, usage_key, remaining_key, boost_key) = match period {
        "interval" => (
            "current_interval_total_count",
            "current_interval_usage_count",
            "current_interval_remaining_percent",
            "interval_boost_permille",
        ),
        "weekly" => (
            "current_weekly_total_count",
            "current_weekly_usage_count",
            "current_weekly_remaining_percent",
            "weekly_boost_permille",
        ),
        _ => return None,
    };
    let total = provider_number(model.get(total_key).unwrap_or(&Value::Null)).unwrap_or(0.0);
    if total > 0.0 {
        let used = provider_number(model.get(usage_key).unwrap_or(&Value::Null)).unwrap_or(0.0);
        return Some(((total - used) / total * 100.0).clamp(0.0, 100.0));
    }
    let boost = provider_number(model.get(boost_key).unwrap_or(&Value::Null)).unwrap_or(1000.0);
    provider_number(model.get(remaining_key).unwrap_or(&Value::Null)).map(|remaining| {
        ((100.0 - remaining).max(0.0) * boost / 1000.0).max(0.0)
    })
}

fn minimax_capability(model_name: &str) -> &'static str {
    match model_name {
        "general" => "文本",
        "video" => "视频",
        "speech-hd" => "语音",
        "coding-plan-vlm" => "图片理解",
        "coding-plan-search" => "网络搜索",
        _ => "其他",
    }
}

fn minimax_summary_metrics(summary: &Value, today: chrono::NaiveDate) -> Vec<(String, f64)> {
    let series = summary
        .get("daily_token_usage")
        .and_then(Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(provider_number)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if series.is_empty() {
        return Vec::new();
    }
    let mut start_date = None;
    let mut day_index = series.len() - 1;
    if let Some(active) = summary.get("most_active_day") {
        let active_tokens = active.get("token_count").and_then(provider_number);
        let active_date = active
            .get("date")
            .and_then(Value::as_str)
            .and_then(|value| value.get(..10))
            .and_then(|value| chrono::NaiveDate::parse_from_str(value, "%Y-%m-%d").ok());
        if let (Some(active_tokens), Some(active_date)) = (active_tokens, active_date) {
            let (index, _) = series
                .iter()
                .enumerate()
                .map(|(index, value)| (index, (value - active_tokens).abs()))
                .min_by(|left, right| left.1.total_cmp(&right.1))
                .unwrap_or((series.len() - 1, 0.0));
            start_date = Some(active_date - chrono::Duration::days(index as i64));
            let offset = (today - start_date.unwrap() ).num_days();
            if offset >= 0 {
                day_index = (offset as usize).min(series.len() - 1);
            }
        }
    }
    let week_start = day_index.saturating_sub(6);
    let week_tokens = series[week_start..=day_index].iter().sum::<f64>();
    let month_tokens = if let Some(start) = start_date {
        let values = series[..=day_index]
            .iter()
            .enumerate()
            .filter(|(index, _)| {
                let date = start + chrono::Duration::days(*index as i64);
                date.year() == today.year() && date.month() == today.month()
            })
            .map(|(_, value)| *value)
            .sum::<f64>();
        if values > 0.0 {
            values
        } else {
            series[day_index.saturating_sub(29)..=day_index]
                .iter()
                .sum::<f64>()
        }
    } else {
        series[day_index.saturating_sub(29)..=day_index]
            .iter()
            .sum::<f64>()
    };
    vec![
        ("日".into(), series[day_index]),
        ("周".into(), week_tokens),
        ("月".into(), month_tokens),
    ]
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
        match provider_http_get_json_retry(&state.client, MINIMAX_PLAN_REMAINS_URL, &headers)
            .await
        {
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
    if let Ok(summary) = provider_http_get_json_retry(&state.client, MINIMAX_USAGE_SUMMARY_URL, &headers)
        .await
        && summary.pointer("/base_resp/status_code").and_then(Value::as_i64) == Some(0)
    {
        for (label, tokens) in minimax_summary_metrics(&summary, chrono::Local::now().date_naive()) {
            let value = fmt_provider_int(tokens);
            if label == "周"
                && let Some(row) = section.rows.iter_mut().find(|row| row.label == "周额度")
            {
                row.output = Some(value);
                continue;
            }
            let row_label = match label.as_str() {
                "周" => "周额度".to_string(),
                "月" => "月额度".to_string(),
                _ => format!("{label}用量"),
            };
            section.rows.push(ProviderUsageRow {
                label: row_label,
                output: Some(value),
                ..Default::default()
            });
        }
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
        if minimax_capability(&name) != "文本" {
            continue;
        }
        if let Some(used_pct) = minimax_used_percent(&model, "interval") {
            let value = format!("{used_pct:.1}%");
            section.windows.push(ProviderUsageWindow {
                window: "5h额度".into(),
                used: Some(value),
                reset: reset_from_ms(&model, "end_time"),
                reset_at: reset_at_from_ms(&model, "end_time"),
            });
        }
        if let Some(used_pct) = minimax_used_percent(&model, "weekly") {
            let value = format!("{used_pct:.1}%");
            section.windows.push(ProviderUsageWindow {
                window: "周额度".into(),
                used: Some(value),
                reset: reset_from_ms(&model, "weekly_end_time"),
                reset_at: reset_at_from_ms(&model, "weekly_end_time"),
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
            window: "5h额度".into(),
            used: Some(ratio_pct(num_field(five_h, "ratio"))),
            reset: reset_from_ms(five_h, "resetTime"),
            reset_at: reset_at_from_ms(five_h, "resetTime"),
        });
    }
    if let Some(weekly) = payload_enabled(data.get("ratelimitCode7d")) {
        section.windows.push(ProviderUsageWindow {
            window: "周额度".into(),
            used: Some(ratio_pct(num_field(weekly, "ratio"))),
            reset: reset_from_ms(weekly, "resetTime"),
            reset_at: reset_at_from_ms(weekly, "resetTime"),
        });
    }
    if let Some(balance) = data.get("subscriptionBalance").filter(|v| v.is_object()) {
        section.windows.push(ProviderUsageWindow {
            window: "月额度".into(),
            used: Some(ratio_pct(num_field(balance, "amountUsedRatio"))),
            reset: reset_from_ms(balance, "expireTime"),
            reset_at: reset_at_from_ms(balance, "expireTime"),
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

fn reset_at_from_ms(value: &Value, key: &str) -> Option<i64> {
    provider_reset_at(value, key)
}

fn provider_reset_text_seconds(seconds: f64) -> String {
    provider_reset_duration(seconds)
}

fn mimo_service_cookie_path(hermes_home: &std::path::Path) -> std::path::PathBuf {
    hermes_home.join("state/mimo_service_token.json")
}

fn load_mimo_service_cookie(hermes_home: &std::path::Path) -> Option<String> {
    let body = std::fs::read_to_string(mimo_service_cookie_path(hermes_home)).ok()?;
    let values = serde_json::from_str::<HashMap<String, String>>(&body).ok()?;
    let required = [
        "api-platform_serviceToken",
        "api-platform_slh",
        "api-platform_ph",
        "userId",
    ];
    required
        .iter()
        .all(|key| values.get(*key).is_some_and(|value| !value.is_empty()))
        .then(|| {
            required
                .iter()
                .filter_map(|key| values.get(*key).map(|value| format!("{key}={value}")))
                .collect::<Vec<_>>()
                .join("; ")
        })
}

fn save_mimo_service_cookie(
    hermes_home: &std::path::Path,
    values: &HashMap<String, String>,
) -> Result<(), String> {
    let path = mimo_service_cookie_path(hermes_home);
    let parent = path
        .parent()
        .ok_or_else(|| "MiMo 会话缓存路径无效".to_string())?;
    std::fs::create_dir_all(parent).map_err(|err| format!("创建 MiMo 会话缓存目录失败：{err}"))?;
    let tmp = path.with_extension("json.tmp");
    let body = serde_json::to_vec(values).map_err(|err| format!("序列化 MiMo 会话失败：{err}"))?;
    std::fs::write(&tmp, body).map_err(|err| format!("写入 MiMo 会话缓存失败：{err}"))?;
    std::fs::rename(tmp, path).map_err(|err| format!("保存 MiMo 会话缓存失败：{err}"))
}

async fn refresh_mimo_service_cookie(state: &AppState) -> Result<String, String> {
    let hermes_home = state.hermes_home.clone();
    let passtoken = provider_env_value(&hermes_home, "MIMO_PASSTOKEN");
    let user_id = provider_env_value(&hermes_home, "MIMO_USER_ID");
    let device_id = provider_env_value(&hermes_home, "MIMO_DEVICE_ID");
    if passtoken.is_empty() {
        return Err("缺少 MiMo 账号认证信息".into());
    }
    let helper = hermes_home
        .parent()
        .unwrap_or(hermes_home.as_path())
        .join("workspace/api-usage/mimo_auth_helper.py");
    if !helper.is_file() {
        return Err("缺少 MiMo 认证 helper".into());
    }
    let python = hermes_home.join("hermes-agent/venv/bin/python");
    let output = tokio::task::spawn_blocking(move || {
        let executable = if python.is_file() {
            python
        } else {
            std::path::PathBuf::from("python3")
        };
        std::process::Command::new(executable)
            .arg(helper)
            .env("MIMO_PASSTOKEN", passtoken)
            .env("MIMO_USER_ID", user_id)
            .env("MIMO_DEVICE_ID", device_id)
            .output()
    })
    .await
    .map_err(|err| format!("MiMo 认证 helper 失败：{err}"))?
    .map_err(|err| format!("MiMo 认证 helper 启动失败：{err}"))?;
    if !output.status.success() {
        return Err("MiMo 认证交换失败，请重新登录 Xiaomi 账号".into());
    }
    let value = String::from_utf8_lossy(&output.stdout)
        .lines()
        .rev()
        .find_map(|line| serde_json::from_str::<HashMap<String, String>>(line).ok())
        .ok_or_else(|| "MiMo 认证交换未返回会话".to_string())?;
    let required = [
        "api-platform_serviceToken",
        "api-platform_slh",
        "api-platform_ph",
        "userId",
    ];
    if required.iter().any(|key| value.get(*key).is_none_or(String::is_empty)) {
        return Err("MiMo 认证交换返回的会话不完整".into());
    }
    save_mimo_service_cookie(&hermes_home, &value)?;
    Ok(required
        .iter()
        .filter_map(|key| value.get(*key).map(|item| format!("{key}={item}")))
        .collect::<Vec<_>>()
        .join("; "))
}

fn mimo_headers(cookie: &str, referer: &str) -> Vec<(String, String)> {
    vec![
        ("Cookie".to_string(), cookie.to_string()),
        ("accept".to_string(), "application/json".into()),
        (
            "user-agent".to_string(),
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/148.0.0.0 Safari/537.36".into(),
        ),
        ("origin".to_string(), "https://platform.xiaomimimo.com".into()),
        ("referer".to_string(), referer.into()),
        ("x-timezone".to_string(), "Asia/Shanghai".into()),
    ]
}

fn mimo_auth_error(error: &str) -> bool {
    error.contains("HTTP 401") || error.contains("HTTP 403") || error.contains("Unauthorized")
}

async fn fetch_mimo_usage(state: &AppState) -> ProviderUsageSection {
    let mut section = ProviderUsageSection {
        provider: "mimo".into(),
        title: "MiMo 额度".into(),
        ..Default::default()
    };
    let mut cookie = load_mimo_service_cookie(&state.hermes_home)
        .or_else(|| {
            let configured = provider_env_value(&state.hermes_home, "MIMO_COOKIE");
            (!configured.is_empty()).then_some(configured)
        })
        .unwrap_or_default();
    if cookie.is_empty() && !provider_env_value(&state.hermes_home, "MIMO_PASSTOKEN").is_empty() {
        if let Ok(refreshed) = refresh_mimo_service_cookie(state).await {
            cookie = refreshed;
        }
    }
    if cookie.is_empty() {
        section.errors.push("缺少 MiMo 认证信息".into());
        return section;
    }

    let detail_result = provider_http_get_json(
        &state.client,
        &format!("{MIMO_API_BASE}/tokenPlan/detail"),
        &mimo_headers(&cookie, "https://platform.xiaomimimo.com/console/plan-manage"),
    )
    .await;
    let detail = match detail_result {
        Ok(value) => value,
        Err(first_error) if mimo_auth_error(&first_error) => {
            match refresh_mimo_service_cookie(state).await {
                Ok(refreshed) => {
                    cookie = refreshed;
                    match provider_http_get_json(
                        &state.client,
                        &format!("{MIMO_API_BASE}/tokenPlan/detail"),
                        &mimo_headers(&cookie, "https://platform.xiaomimimo.com/console/plan-manage"),
                    )
                    .await
                    {
                        Ok(value) => value,
                        Err(error) => {
                            section.errors.push(format!("MiMo 认证后查询失败：{error}"));
                            return section;
                        }
                    }
                }
                Err(error) => {
                    section.errors.push(format!("MiMo 认证交换失败：{error}"));
                    return section;
                }
            }
        }
        Err(err) => {
            section.errors.push(format!("查询失败：{err}"));
            return section;
        }
    };
    let detail = {
        let code = detail.get("code").and_then(Value::as_i64).unwrap_or(-1);
        if !(code == 0 || code == 200) {
            section.errors.push(format!("MiMo 用量接口返回错误码：{code}"));
            return section;
        }
        detail.get("data").cloned().unwrap_or(Value::Null)
    };

    let usage_summary = provider_http_get_json(
        &state.client,
        &format!("{MIMO_API_BASE}/tokenPlan/usage"),
        &mimo_headers(&cookie, "https://platform.xiaomimimo.com/console/plan-manage"),
    )
    .await
    .ok()
    .and_then(|value| value.get("data").cloned())
    .unwrap_or(Value::Null);
    let plan_used = detail_usage_number(&detail, "usage", "plan_total_token", "used")
        .or_else(|| detail_usage_number(&usage_summary, "usage", "plan_total_token", "used"));
    let plan_limit = detail_usage_number(&detail, "usage", "plan_total_token", "limit")
        .or_else(|| detail_usage_number(&usage_summary, "usage", "plan_total_token", "limit"));

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
    section.description = format!("MiMo {plan_name} · {renew_tag}");

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

    let mut month_buckets: BTreeMap<String, MimoBucket> = BTreeMap::new();
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
            &mimo_headers(&cookie, "https://platform.xiaomimimo.com/console/plan-manage"),
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
        }
    }

    let total_model_credit: f64 = month_buckets
        .iter()
        .map(|(model, bucket)| bucket_credits(model, bucket))
        .sum();
    if let Some(percent) = mimo_usage_percent(plan_used, plan_limit) {
        section.windows.push(ProviderUsageWindow {
            window: "月额度".into(),
            used: Some(format!("{percent:.2}%")),
            reset: mimo_reset_duration(period_end_raw),
            reset_at: mimo_reset_at(period_end_raw),
        });
    }
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

fn mimo_usage_percent(used: Option<f64>, limit: Option<f64>) -> Option<f64> {
    match (used, limit) {
        (Some(used), Some(limit)) if limit > 0.0 => Some((used / limit * 100.0).max(0.0)),
        _ => None,
    }
}

fn mimo_reset_at(value: &str) -> Option<i64> {
    chrono::DateTime::parse_from_rfc3339(value)
        .map(|date| date.timestamp())
        .or_else(|_| {
            chrono::NaiveDateTime::parse_from_str(value, "%Y-%m-%d %H:%M:%S")
                .map(|date| date.and_utc().timestamp())
        })
        .ok()
}

fn mimo_reset_duration(value: &str) -> Option<String> {
    let parsed = mimo_reset_at(value)?;
    Some(provider_reset_duration((parsed - chrono::Utc::now().timestamp()).max(0) as f64))
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

#[cfg(test)]
fn opencode_has_no_usage_data(html: &str) -> bool {
    let normalized = html.to_ascii_lowercase();
    normalized.contains("no usage data available")
        || normalized.contains("make your first api call")
}

fn opencode_dashboard_urls(workspace_id: &str, override_url: &str) -> Vec<String> {
    if !override_url.trim().is_empty() {
        return vec![override_url.trim().to_string()];
    }
    vec![
        format!("https://opencode.ai/workspace/{workspace_id}/usage"),
        format!("https://opencode.ai/workspace/{workspace_id}/go"),
    ]
}

fn opencode_percent_text(value: f64) -> String {
    let mut text = format!("{value:.2}");
    while text.contains('.') && text.ends_with('0') {
        text.pop();
    }
    if text.ends_with('.') {
        text.pop();
    }
    format!("{text}%")
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
        section.errors.push("缺少 OpenCode Go 查询凭据".into());
        return section;
    };
    let cookie_header = if auth_cookie.starts_with("auth=") || auth_cookie.contains(';') {
        auth_cookie
    } else {
        format!("auth={auth_cookie}")
    };
    let headers = vec![
        (
            "Accept".to_string(),
            "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8".into(),
        ),
        (
            "User-Agent".to_string(),
            "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 Hermes-token-usage/1.0".into(),
        ),
        ("Cookie".to_string(), cookie_header),
    ];
    let override_url = provider_env_value(&state.hermes_home, "OPENCODE_GO_DASHBOARD_URL");
    let urls = opencode_dashboard_urls(&workspace_id, &override_url);
    let mut errors = Vec::new();
    for url in urls {
        let html = match provider_http_get_text(&state.client, &url, &headers).await {
            Ok(html) => html,
            Err(err) => {
                errors.push(format!("{}：{err}", url.rsplit('/').next().unwrap_or("页面")));
                continue;
            }
        };
        let mut windows = Vec::new();
        let now = chrono::Utc::now().timestamp();
        for (name, label) in [("rolling", "5h额度"), ("weekly", "周额度"), ("monthly", "月额度")] {
            let Some((percent, reset_seconds)) = opencode_window(&html, name) else {
                continue;
            };
            let reset_at = now.saturating_add(reset_seconds.max(0.0) as i64);
            windows.push(ProviderUsageWindow {
                window: label.into(),
                used: Some(opencode_percent_text(percent)),
                reset: Some(provider_reset_text_seconds(reset_seconds.max(0.0))),
                reset_at: Some(reset_at),
            });
        }
        if !windows.is_empty() {
            section.windows = windows;
            return section;
        }
        errors.push(format!("{}：缺少 5h额度/周额度/月额度窗口", url.rsplit('/').next().unwrap_or("页面")));
    }
    if !errors.is_empty() {
        section.errors.push(format!("查询失败：{}", errors.join("；")));
    }
    section
}

fn opencode_window(html: &str, name: &str) -> Option<(f64, f64)> {
    // SolidJS SSR shape: rollingUsage:$R[12]={usagePercent:12.3,resetInSec:3600,...}
    let marker = format!("{name}Usage:$R[");
    let start = html.find(&marker)? + marker.len();
    let rest = &html[start..];
    let index_end = rest.find(']')?;
    let body_start = rest[index_end + 1..].find('{')? + index_end + 1;
    let body_end = rest[body_start..].find('}')? + body_start;
    let body = &rest[body_start + 1..body_end];
    let parse_field = |key: &str| -> Option<f64> {
        let marker = format!("{key}:");
        let index = body.find(&marker)? + marker.len();
        let tail = &body[index..];
        let end = tail
            .find(|ch: char| !(ch.is_ascii_digit() || ch == '.' || ch == '-'))
            .unwrap_or(tail.len());
        tail[..end].parse::<f64>().ok().filter(|value| value.is_finite())
    };
    let percent = parse_field("usagePercent")?;
    let reset = parse_field("resetInSec")?;
    Some((percent, reset.max(0.0)))
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
            if token_matches {
                if let Some(cached) = cached_section.and_then(|section| {
                    provider_cached_account_section(section, &label, multi_account, now)
                }) {
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
            }
            Some((label, token, account_cache))
        })
        .collect::<Vec<_>>();
    let mut fetched: Vec<(String, Result<(Value, CommandCodeAccountCache), String>)> =
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
    let prefix = if normalized.contains("/backend-api") {
        format!("{normalized}/wham")
    } else {
        format!("{normalized}/api/codex")
    };
    prefix
}

fn codex_backend_reset_credits_url(base_url: &str) -> String {
    format!(
        "{}/rate-limit-reset-credits",
        codex_backend_prefix(base_url)
    )
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

fn codex_timestamp(value: &Value) -> Option<i64> {
    let timestamp = if let Some(raw) = value.as_str() {
        if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(raw) {
            dt.timestamp()
        } else {
            provider_number(value)? as i64
        }
    } else {
        provider_number(value)? as i64
    };
    Some(normalize_provider_timestamp(timestamp))
}

fn provider_reset_at(value: &Value, key: &str) -> Option<i64> {
    value.get(key).and_then(codex_timestamp)
}

fn normalize_provider_timestamp(timestamp: i64) -> i64 {
    if timestamp >= 100_000_000_000 {
        timestamp / 1000
    } else {
        timestamp
    }
}

fn provider_account_window_matches(window: &ProviderUsageWindow, label: &str) -> bool {
    window.window == label || window.window.starts_with(&format!("{label} "))
}

fn provider_used_percent(window: &ProviderUsageWindow) -> Option<f64> {
    window
        .used
        .as_deref()?
        .trim()
        .trim_end_matches('%')
        .trim()
        .parse::<f64>()
        .ok()
        .filter(|value| value.is_finite())
}

fn provider_section_should_skip_upstream(section: &ProviderUsageSection, now: i64) -> bool {
    section.windows.iter().any(|window| {
        provider_used_percent(window).is_some_and(|used| used >= 100.0)
            && window.reset_at.is_some_and(|reset_at| reset_at > now)
    })
}

fn provider_account_should_skip_upstream(
    section: &ProviderUsageSection,
    label: &str,
    account_scoped: bool,
    now: i64,
) -> bool {
    section
        .windows
        .iter()
        .filter(|window| !account_scoped || provider_account_window_matches(window, label))
        .any(|window| {
            provider_used_percent(window).is_some_and(|used| used >= 100.0)
                && window.reset_at.is_some_and(|reset_at| reset_at > now)
        })
}

fn refresh_provider_cached_reset_times(
    section: &mut ProviderUsageSection,
    now: i64,
) {
    for window in &mut section.windows {
        if let Some(reset_at) = window.reset_at {
            window.reset = Some(provider_reset_duration((reset_at - now).max(0) as f64));
        }
    }
}

fn provider_cached_section(
    section: &ProviderUsageSection,
    now: i64,
) -> Option<ProviderUsageSection> {
    if !provider_section_should_skip_upstream(section, now) {
        return None;
    }
    let mut cached = section.clone();
    refresh_provider_cached_reset_times(&mut cached, now);
    Some(cached)
}

fn provider_cached_account_section(
    section: &ProviderUsageSection,
    label: &str,
    account_scoped: bool,
    now: i64,
) -> Option<ProviderUsageSection> {
    if !provider_account_should_skip_upstream(section, label, account_scoped, now) {
        return None;
    }
    let mut cached = section.clone();
    if account_scoped {
        cached
            .windows
            .retain(|window| provider_account_window_matches(window, label));
    }
    refresh_provider_cached_reset_times(&mut cached, now);
    Some(cached)
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

async fn fetch_codex_usage(
    state: &AppState,
    cached_section: Option<&ProviderUsageSection>,
    force: bool,
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
    }
    // Fanout only accounts that are not currently exhausted. Accounts at 100%
    // remain served from their cached windows until one of those windows resets.
    let mut fetched: Vec<(String, Result<Value, String>)> =
        futures_util::future::join_all(live_accounts.iter().map(|(label, token)| async move {
            jitter_delay().await;
            (label.clone(), codex_query_account(state, token).await)
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
                        section
                            .errors
                            .push(format!("{label}：Reset 查询失败：{error}"));
                    } else if let Some(description) = codex_reset_credits_description(reset_credits)
                    {
                        reset_descriptions.push(format!("{label}：{description}"));
                    }
                }
            }
            Err(err) => section.errors.push(format!("{label}：查询失败：{err}")),
        }
    }
    section.description = reset_descriptions.join("；");
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

/// Random 200-1500ms delay so concurrent account queries don't hit upstream
/// consoles at the same instant (mirrors the script's jitter).
async fn jitter_delay() {
    let millis = 200 + (std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos() as u64 % 1_300)
        .unwrap_or(0));
    sleep(Duration::from_millis(millis)).await;
}

fn grok_proto_varint(data: &[u8], offset: &mut usize) -> Result<u64, String> {
    let mut value = 0u64;
    for shift in (0..70).step_by(7) {
        let byte = *data
            .get(*offset)
            .ok_or_else(|| "gRPC 响应的 varint 不完整".to_string())?;
        *offset += 1;
        value |= u64::from(byte & 0x7f) << shift;
        if byte & 0x80 == 0 {
            return Ok(value);
        }
    }
    Err("gRPC 响应的 varint 溢出".to_string())
}

fn grok_proto_field(data: &[u8], wanted: u32) -> Option<(u8, Vec<u8>)> {
    let mut offset = 0;
    while offset < data.len() {
        let tag = grok_proto_varint(data, &mut offset).ok()?;
        let field = u32::try_from(tag >> 3).ok()?;
        let wire = u8::try_from(tag & 7).ok()?;
        let value = match wire {
            0 => grok_proto_varint(data, &mut offset).ok()?.to_le_bytes().to_vec(),
            1 => {
                let end = offset.checked_add(8)?;
                let value = data.get(offset..end)?.to_vec();
                offset = end;
                value
            }
            2 => {
                let length = usize::try_from(grok_proto_varint(data, &mut offset).ok()?).ok()?;
                let end = offset.checked_add(length)?;
                let value = data.get(offset..end)?.to_vec();
                offset = end;
                value
            }
            5 => {
                let end = offset.checked_add(4)?;
                let value = data.get(offset..end)?.to_vec();
                offset = end;
                value
            }
            _ => return None,
        };
        if field == wanted {
            return Some((wire, value));
        }
    }
    None
}

fn grok_proto_timestamp(data: &[u8]) -> Option<chrono::DateTime<chrono::Utc>> {
    let (_, seconds) = grok_proto_field(data, 1)?;
    let seconds = i64::try_from(u64::from_le_bytes(seconds.try_into().ok()?)).ok()?;
    let nanos = grok_proto_field(data, 2)
        .and_then(|(_, value)| u64::from_le_bytes(value.try_into().ok()?).try_into().ok())
        .unwrap_or(0);
    chrono::DateTime::from_timestamp(seconds, nanos)
}

fn grok_web_billing_snapshot(body: &[u8]) -> Result<Value, String> {
    if body.len() < 5 || body[0] & 0x80 != 0 {
        return Err("gRPC 响应缺少有效的数据帧".to_string());
    }
    let length = u32::from_be_bytes(body[1..5].try_into().unwrap()) as usize;
    let end = 5usize
        .checked_add(length)
        .filter(|end| *end <= body.len())
        .ok_or_else(|| "gRPC 响应数据帧被截断".to_string())?;
    let credits = grok_proto_field(&body[5..end], 1)
        .filter(|(wire, _)| *wire == 2)
        .map(|(_, value)| value)
        .ok_or_else(|| "gRPC 响应缺少额度信息".to_string())?;
    let used_percent = grok_proto_field(&credits, 1)
        .filter(|(wire, value)| *wire == 5 && value.len() == 4)
        .map(|(_, value)| f32::from_le_bytes(value.try_into().unwrap()) as f64)
        .unwrap_or(0.0);
    if !used_percent.is_finite() || !(0.0..=100.0).contains(&used_percent) {
        return Err("gRPC 响应包含无效的额度比例".to_string());
    }
    let reset = grok_proto_field(&credits, 5)
        .filter(|(wire, _)| *wire == 2)
        .and_then(|(_, value)| grok_proto_timestamp(&value))
        .or_else(|| {
            grok_proto_field(&credits, 8)
                .filter(|(wire, _)| *wire == 2)
                .and_then(|(_, value)| {
                    grok_proto_field(&value, 3)
                        .filter(|(wire, _)| *wire == 2)
                        .and_then(|(_, timestamp)| grok_proto_timestamp(&timestamp))
                })
        })
        .ok_or_else(|| "gRPC 响应缺少额度重置时间".to_string())?;
    Ok(serde_json::json!({
        "config": {
            "currentPeriod": {
                "type": "USAGE_PERIOD_TYPE_WEEKLY",
                "end": reset.to_rfc3339(),
            },
            "creditUsagePercent": used_percent,
        }
    }))
}

async fn grok_query_web_billing(state: &AppState, token: &str) -> Result<Value, String> {
    let headers = vec![
        ("Authorization".to_string(), format!("Bearer {token}")),
        ("Accept".to_string(), "application/grpc-web+proto".to_string()),
        (
            "Content-Type".to_string(),
            "application/grpc-web+proto".to_string(),
        ),
        ("X-Grpc-Web".to_string(), "1".to_string()),
        (
            "Origin".to_string(),
            "https://grok.com".to_string(),
        ),
        (
            "Referer".to_string(),
            "https://grok.com/".to_string(),
        ),
        (
            "User-Agent".to_string(),
            "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/140.0.0.0 Safari/537.36".to_string(),
        ),
    ];
    let body = provider_http_post_bytes(
        &state.client,
        GROK_WEB_CREDITS_URL,
        &[0, 0, 0, 0, 0],
        &headers,
    )
    .await?;
    grok_web_billing_snapshot(&body)
}

async fn grok_refresh_access_token(
    state: &AppState,
    refresh_token: &str,
) -> Result<(String, String), String> {
    let response = state
        .client
        .post(XAI_OAUTH_TOKEN_URL)
        .timeout(PROVIDER_USAGE_TIMEOUT)
        .form(&[
            ("grant_type", "refresh_token"),
            ("client_id", XAI_OAUTH_CLIENT_ID),
            ("refresh_token", refresh_token),
        ])
        .send()
        .await
        .map_err(|err| err.to_string())?;
    let status = response.status();
    if !status.is_success() {
        return Err(format!("HTTP {}", status.as_u16()));
    }
    let payload: Value = response.json().await.map_err(|err| err.to_string())?;
    let access_token = payload
        .get("access_token")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|token| !token.is_empty())
        .ok_or_else(|| "刷新响应缺少 access_token".to_string())?;
    let next_refresh = payload
        .get("refresh_token")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|token| !token.is_empty())
        .unwrap_or(refresh_token);
    Ok((access_token.to_string(), next_refresh.to_string()))
}

async fn grok_query_account(
    state: &AppState,
    account: &AuthCredentialEntry,
) -> Result<Value, String> {
    let first = grok_query_web_billing(state, &account.access_token).await;
    match first {
        Ok(payload) => Ok(payload),
        Err(err) if err.starts_with("HTTP 401") => {
            let Some(refresh_token) = account.refresh_token.as_deref() else {
                return Err(err);
            };
            let (access_token, _next_refresh) =
                grok_refresh_access_token(state, refresh_token).await?;
            grok_query_web_billing(state, &access_token).await
        }
        Err(err) => Err(err),
    }
}

fn zed_plan_label(raw: &str) -> String {
    match raw.trim().to_ascii_lowercase().as_str() {
        "" => String::new(),
        "zed_student" => "Student".into(),
        "zed_pro" => "Pro".into(),
        "zed_pro_trial" => "Pro Trial".into(),
        "zed_free" => "Free".into(),
        other => other
            .strip_prefix("zed_")
            .unwrap_or(other)
            .replace('_', " "),
    }
}

fn zed_usage_limit(value: Option<&Value>) -> Option<f64> {
    let value = value?;
    if value
        .as_str()
        .is_some_and(|text| text.eq_ignore_ascii_case("unlimited"))
    {
        return None;
    }
    if let Some(number) = provider_number(value) {
        return Some(number);
    }
    let object = value.as_object()?;
    if object.contains_key("unlimited") {
        return None;
    }
    object
        .get("limited")
        .or_else(|| object.get("limit"))
        .and_then(provider_number)
}

fn zed_iso_timestamp(value: Option<&Value>) -> Option<i64> {
    let raw = value
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|text| !text.is_empty())?;
    chrono::DateTime::parse_from_rfc3339(raw)
        .ok()
        .map(|stamp| stamp.timestamp())
}

fn zed_short_date(value: Option<&Value>) -> Option<String> {
    let stamp = zed_iso_timestamp(value)?;
    let when = chrono::DateTime::<chrono::Utc>::from_timestamp(stamp, 0)?;
    Some(format!("{}/{}", when.format("%m"), when.format("%d")))
}

fn zed_count_text(used: Option<f64>, limit: Option<f64>) -> Option<String> {
    let used = used?;
    Some(match limit {
        Some(limit) if limit > 0.0 => format!("{used:.0}/{limit:.0}"),
        Some(_) => format!("{used:.0}"),
        None => format!("{used:.0}/无限"),
    })
}

fn zed_plan_name(payload: &Value) -> String {
    let plan = payload.get("plan");
    let raw = plan
        .and_then(|value| value.get("plan_v3"))
        .and_then(Value::as_str)
        .or_else(|| {
            plan.and_then(|value| value.get("plan_v2"))
                .and_then(Value::as_str)
        })
        .or_else(|| {
            plan.and_then(|value| value.get("plan")).and_then(|value| {
                value
                    .as_str()
                    .or_else(|| value.get("plan").and_then(Value::as_str))
            })
        })
        .or_else(|| {
            payload
                .get("plans_by_organization")
                .and_then(Value::as_object)
                .and_then(|plans| plans.values().find_map(Value::as_str))
        })
        .unwrap_or("");
    zed_plan_label(raw)
}

fn zed_account_snapshot(
    payload: &Value,
) -> Result<(String, Vec<ProviderUsageWindow>), String> {
    let plan = payload.get("plan").cloned().unwrap_or(Value::Null);
    let plan_name = zed_plan_name(payload);
    if plan_name.is_empty() && plan.is_null() {
        return Err("返回内容缺少套餐信息".into());
    }
    let period = plan
        .get("subscription_period")
        .cloned()
        .unwrap_or(Value::Null);
    let reset_at = zed_iso_timestamp(period.get("ended_at"));
    let reset = reset_at.map(provider_reset_text_local);
    let usage = plan.get("usage").cloned().unwrap_or(Value::Null);
    let model_requests = usage.get("model_requests").cloned().unwrap_or(Value::Null);
    let edit_predictions = usage
        .get("edit_predictions")
        .cloned()
        .unwrap_or(Value::Null);
    let model_used = model_requests.get("used").and_then(provider_number);
    let model_limit = zed_usage_limit(model_requests.get("limit"));
    let edit_used = edit_predictions.get("used").and_then(provider_number);
    let edit_limit = zed_usage_limit(edit_predictions.get("limit"));
    let mut details = Vec::new();
    if !plan_name.is_empty() {
        details.push(plan_name);
    }
    if let (Some(start), Some(end)) = (
        zed_short_date(period.get("started_at")),
        zed_short_date(period.get("ended_at")),
    ) {
        details.push(format!("账期 {start}–{end}"));
    }
    if let Some(text) = zed_count_text(model_used, model_limit) {
        details.push(format!("模型请求 {text}"));
    }
    if let Some(text) = zed_count_text(edit_used, edit_limit) {
        details.push(format!("补全 {text}"));
    }
    let mut windows = Vec::new();
    if let (Some(used), Some(limit)) = (model_used, model_limit)
        && limit > 0.0
    {
        let percent = used / limit * 100.0;
        if percent.is_finite() {
            windows.push(ProviderUsageWindow {
                window: "月额度".into(),
                used: Some(format!("{percent:.1}%")),
                reset,
                reset_at,
            });
        }
    }
    Ok((details.join(" · "), windows))
}

async fn zed_query_account(
    state: &AppState,
    account: &AuthCredentialEntry,
) -> Result<Value, String> {
    let user_id = account
        .user_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "凭据缺少 user_id".to_string())?;
    let headers = [
        (
            "Authorization".into(),
            format!("{user_id} {}", account.access_token),
        ),
        ("Accept".into(), "application/json".into()),
        ("Content-Type".into(), "application/json".into()),
        ("User-Agent".into(), "yahu/zed-pro".into()),
    ];
    provider_http_get_json(&state.client, ZED_CLOUD_ME_URL, &headers).await
}

async fn fetch_zed_pro_usage(
    state: &AppState,
    cached_section: Option<&ProviderUsageSection>,
    force: bool,
) -> ProviderUsageSection {
    let mut section = ProviderUsageSection {
        provider: "zed-pro".into(),
        title: "Zed Pro 用量".into(),
        ..Default::default()
    };
    let accounts = auth_json_credential_entries(&state.hermes_home, "zed-pro");
    if accounts.is_empty() {
        section.errors.push("未找到 zed-pro 账号凭据".into());
        return section;
    }
    let now = chrono::Utc::now().timestamp();
    let multi_account = accounts.len() > 1;
    let mut skipped_accounts = Vec::new();
    let live_accounts = accounts
        .into_iter()
        .filter_map(|account| {
            if !force
                && let Some(cached) = cached_section.and_then(|section| {
                    provider_cached_account_section(section, &account.label, multi_account, now)
                })
            {
                skipped_accounts.push((account.label, cached));
                None
            } else {
                Some(account)
            }
        })
        .collect::<Vec<_>>();
    let mut fetched: Vec<(String, Result<Value, String>)> = futures_util::future::join_all(
        live_accounts.iter().map(|account| async {
            jitter_delay().await;
            (account.label.clone(), zed_query_account(state, account).await)
        }),
    )
    .await;
    fetched.sort_by(|a, b| a.0.to_lowercase().cmp(&b.0.to_lowercase()));
    section.description = cached_section
        .map(|cached| cached.description.clone())
        .unwrap_or_default();
    for (_, cached) in skipped_accounts {
        section.windows.extend(cached.windows);
        if section.description.is_empty() && !cached.description.is_empty() {
            section.description = cached.description;
        }
    }
    for (label, result) in fetched {
        match result {
            Ok(payload) => match zed_account_snapshot(&payload) {
                Ok((description, windows)) => {
                    for window in windows {
                        section.windows.push(ProviderUsageWindow {
                            window: if multi_account {
                                format!("{label} {}", window.window)
                            } else {
                                window.window
                            },
                            used: window.used,
                            reset: window.reset,
                            reset_at: window.reset_at,
                        });
                    }
                    if section.description.is_empty() && !description.is_empty() {
                        section.description = if multi_account {
                            format!("{label} {description}")
                        } else {
                            description
                        };
                    }
                }
                Err(err) => section.errors.push(format!("{label}：{err}")),
            },
            Err(err) => section.errors.push(format!("{label}：查询失败：{err}")),
        }
    }
    section
}

fn grok_billing_snapshot(
    payload: &Value,
) -> Result<(String, f64, String, String, Option<i64>), String> {
    let config = payload.get("config").cloned().unwrap_or(Value::Null);
    let period = config.get("currentPeriod").cloned().unwrap_or(Value::Null);
    let period_type = period
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or("");
    let reset_raw = period
        .get("end")
        .or_else(|| config.get("billingPeriodEnd"))
        .cloned()
        .unwrap_or(Value::Null);
    if period_type.is_empty() || reset_raw.is_null() {
        return Err("返回内容缺少可识别的额度周期".into());
    }
    let used_percent = config
        .get("creditUsagePercent")
        .or_else(|| config.get("credit_usage_percent"))
        .and_then(provider_number)
        .or_else(|| {
            config
                .get("productUsage")
                .and_then(Value::as_array)
                .and_then(|products| {
                    products
                        .iter()
                        .filter_map(|product| {
                            product
                                .get("usagePercent")
                                .or_else(|| product.get("usage_percent"))
                                .and_then(provider_number)
                        })
                        .max_by(f64::total_cmp)
                })
        })
        .ok_or_else(|| "返回内容未提供周额度使用比例".to_string())?
        .clamp(0.0, 100.0);
    let window = if period_type.contains("WEEKLY") {
        "周额度"
    } else if period_type.contains("MONTHLY") {
        "月额度"
    } else {
        "5h额度"
    }
    .to_string();
    let reset_at = codex_timestamp(&reset_raw);
    let parsed_reset: Option<chrono::DateTime<chrono::Utc>> =
        reset_at.and_then(|timestamp| chrono::DateTime::from_timestamp(timestamp, 0));
    let reset = parsed_reset
        .map(|dt| {
            let local = dt.with_timezone(&chrono::Local);
            format!("{}/{} {}", local.month(), local.day(), local.format("%H:%M"))
        })
        .unwrap_or_else(|| "-".into());
    let tier = payload
        .get("subscriptionTier")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    Ok((window, used_percent, reset, tier, reset_at))
}

async fn fetch_grok_usage(
    state: &AppState,
    cached_section: Option<&ProviderUsageSection>,
    force: bool,
) -> ProviderUsageSection {
    let mut section = ProviderUsageSection {
        provider: "grok".into(),
        title: "Grok Build 额度".into(),
        ..Default::default()
    };
    let accounts = auth_json_credential_entries(&state.hermes_home, "xai-oauth");
    if accounts.is_empty() {
        section.errors.push("未找到 xai-oauth OAuth 会话".into());
        return section;
    }
    let now = chrono::Utc::now().timestamp();
    let multi_account = accounts.len() > 1;
    let mut skipped_accounts = Vec::new();
    let live_accounts = accounts
        .into_iter()
        .filter_map(|account| {
            if !force
                && let Some(cached) = cached_section.and_then(|section| {
                    provider_cached_account_section(section, &account.label, multi_account, now)
                })
            {
                skipped_accounts.push((account.label, cached));
                None
            } else {
                Some(account)
            }
        })
        .collect::<Vec<_>>();
    let mut fetched: Vec<(String, Result<Value, String>)> = futures_util::future::join_all(
        live_accounts.iter().map(|account| async {
            jitter_delay().await;
            (account.label.clone(), grok_query_account(state, account).await)
        }),
    )
    .await;
    fetched.sort_by(|a, b| a.0.to_lowercase().cmp(&b.0.to_lowercase()));
    section.description = cached_section
        .map(|cached| cached.description.clone())
        .unwrap_or_default();
    for (_, cached) in skipped_accounts {
        section.windows.extend(cached.windows);
    }
    for (label, result) in fetched {
        match result {
            Ok(payload) => match grok_billing_snapshot(&payload) {
                Ok((window, used_percent, reset, tier, reset_at)) => {
                    section.windows.push(ProviderUsageWindow {
                        window: if multi_account { format!("{label} {window}") } else { window },
                        used: Some(format!("{used_percent:.1}%")),
                        reset: Some(reset),
                        reset_at,
                    });
                    if section.description.is_empty() && !tier.is_empty() {
                        section.description = tier;
                    }
                }
                Err(err) => section.errors.push(format!("{label}：{err}")),
            },
            Err(err) => section.errors.push(format!("{label}：查询失败：{err}")),
        }
    }
    section
}

fn provider_setup_section(meta: &ProviderUsageProvider) -> ProviderUsageSection {
    ProviderUsageSection {
        provider: meta.provider.clone(),
        title: meta.title.clone(),
        description: format!("{}。{}", meta.credential_hint, meta.setup_hint),
        ..Default::default()
    }
}

async fn fetch_provider_usage_section(
    state: &AppState,
    provider: &str,
    cached_section: Option<&ProviderUsageSection>,
    force: bool,
) -> ProviderUsageSection {
    let account_scoped = matches!(provider, "commandcode" | "codex" | "grok" | "zed-pro");
    if !account_scoped {
        let now = chrono::Utc::now().timestamp();
        if let Some(mut cached) = cached_section.and_then(|section| provider_cached_section(section, now)) {
            cached.captured_at = unix_now_seconds();
            return cached;
        }
    }
    let mut section = match provider {
        "openrouter" => fetch_openrouter_usage(state).await,
        "deepseek" => fetch_deepseek_usage(state).await,
        "atlascloud" => fetch_atlascloud_usage(state).await,
        "minimax" => fetch_minimax_usage(state).await,
        "kimi" => fetch_kimi_usage(state).await,
        "mimo" => fetch_mimo_usage(state).await,
        "opencode" => fetch_opencode_usage(state).await,
        "commandcode" => fetch_commandcode_usage(state, cached_section).await,
        "codex" => fetch_codex_usage(state, cached_section, force).await,
        "grok" => fetch_grok_usage(state, cached_section, force).await,
        "zed-pro" => fetch_zed_pro_usage(state, cached_section, force).await,
        _ => ProviderUsageSection::default(),
    };
    section.captured_at = unix_now_seconds();
    section
}

async fn collect_provider_usage_for(
    state: &AppState,
    provider: &str,
    providers: Vec<ProviderUsageProvider>,
    force: bool,
) -> ProviderUsagePayload {
    let cached_section = shared_cached_section(state, provider).map(|entry| entry.section);
    let section = if let Some(meta) = providers.iter().find(|meta| meta.provider == provider) {
        if meta.query_ready {
            fetch_provider_usage_section(state, provider, cached_section.as_ref(), force).await
        } else {
            provider_setup_section(meta)
        }
    } else {
        fetch_provider_usage_section(state, provider, cached_section.as_ref(), force).await
    };
    ProviderUsagePayload {
        fetched_at: unix_now_seconds(),
        providers,
        sections: vec![section],
    }
}

fn shared_provider_cache_path(state: &AppState) -> std::path::PathBuf {
    state.hermes_home.join("state/token_usage_cache.json")
}

fn read_shared_provider_cache(state: &AppState) -> SharedProviderCacheFile {
    let Ok(raw) = std::fs::read_to_string(shared_provider_cache_path(state)) else {
        return SharedProviderCacheFile::default();
    };
    serde_json::from_str(&raw).unwrap_or_default()
}

fn write_shared_provider_cache(state: &AppState, cache: &SharedProviderCacheFile) {
    let path = shared_provider_cache_path(state);
    let Some(parent) = path.parent() else { return };
    if std::fs::create_dir_all(parent).is_err() {
        return;
    }
    let Ok(raw) = serde_json::to_vec_pretty(cache) else { return };
    let tmp = path.with_extension("json.tmp");
    if std::fs::write(&tmp, raw).is_ok() {
        let _ = std::fs::rename(tmp, path);
    }
}

fn shared_cached_section(state: &AppState, provider: &str) -> Option<SharedProviderCacheEntry> {
    let mut entry = read_shared_provider_cache(state).entries.get(provider).cloned()?;
    if (provider == "deepseek"
        && entry.section.description.is_empty()
        && entry.section.rows.is_empty()
        && entry.section.windows.is_empty())
        || (provider == "openrouter" && !entry.section.description.contains("余额"))
    {
        return None;
    }
    if entry.section.captured_at <= 0.0 {
        entry.section.captured_at = entry.cached_at;
    }
    Some(entry)
}

fn shared_cache_sections(state: &AppState) -> Vec<ProviderUsageSection> {
    read_shared_provider_cache(state)
        .entries
        .into_values()
        .filter_map(|mut entry| {
            if (entry.section.provider == "deepseek"
                && entry.section.description.is_empty()
                && entry.section.rows.is_empty()
                && entry.section.windows.is_empty())
                || (entry.section.provider == "openrouter" && !entry.section.description.contains("余额"))
            {
                return None;
            }
            if entry.section.captured_at <= 0.0 {
                entry.section.captured_at = entry.cached_at;
            }
            Some(entry.section)
        })
        .collect()
}

fn save_shared_provider_section(state: &AppState, section: &ProviderUsageSection, cached_at: f64) {
    let mut cache = read_shared_provider_cache(state);
    cache.entries.insert(
        section.provider.clone(),
        SharedProviderCacheEntry {
            cached_at,
            section: section.clone(),
        },
    );
    write_shared_provider_cache(state, &cache);
}

async fn cached_provider_payload(
    state: &AppState,
    provider: &str,
) -> Option<ProviderUsagePayload> {
    let guard = state.provider_usage_cache.fetched_at.read().await;
    let fetched_at = (*guard)?;
    if fetched_at.elapsed() >= PROVIDER_USAGE_TTL {
        return None;
    }
    let payload = state.provider_usage_cache.payload.read().await.clone()?;
    payload
        .sections
        .iter()
        .any(|section| section.provider == provider)
        .then_some(payload)
}

fn merge_provider_usage_payload(
    mut cached: ProviderUsagePayload,
    fresh: &ProviderUsagePayload,
) -> ProviderUsagePayload {
    cached.fetched_at = fresh.fetched_at;
    cached.providers = fresh.providers.clone();
    for section in &fresh.sections {
        if let Some(existing) = cached
            .sections
            .iter_mut()
            .find(|existing| existing.provider == section.provider)
        {
            *existing = section.clone();
        } else {
            cached.sections.push(section.clone());
        }
    }
    cached
}

async fn provider_usage_handler(
    State(state): State<Arc<AppState>>,
    Query(query): Query<ProviderUsageQuery>,
) -> Response<Body> {
    let providers = provider_usage_catalog(&state.hermes_home);
    let Some(provider) = query.provider.as_deref().filter(|provider| !provider.is_empty()) else {
        let mut sections = shared_cache_sections(&state);
        if let Some(cached) = state.provider_usage_cache.payload.read().await.clone() {
            for section in cached.sections {
                if let Some(existing) = sections.iter_mut().find(|item| item.provider == section.provider) {
                    *existing = section;
                } else {
                    sections.push(section);
                }
            }
        }
        return (
            StatusCode::OK,
            Json(ProviderUsagePayload {
                fetched_at: sections
                    .iter()
                    .filter_map(|section| shared_cached_section(&state, &section.provider).map(|entry| entry.cached_at))
                    .max_by(f64::total_cmp)
                    .unwrap_or_else(unix_now_seconds),
                providers,
                sections,
            }),
        )
            .into_response();
    };
    let force = query.refresh.unwrap_or(false);
    if !force
        && let Some(payload) = cached_provider_payload(&state, provider).await
    {
        return (StatusCode::OK, Json(payload)).into_response();
    }
    if !force
        && let Some(entry) = shared_cached_section(&state, provider)
    {
        return (
            StatusCode::OK,
            Json(ProviderUsagePayload {
                fetched_at: entry.cached_at,
                providers,
                sections: vec![entry.section],
            }),
        )
            .into_response();
    }
    let fresh = collect_provider_usage_for(&state, provider, providers, force).await;
    let cached = state
        .provider_usage_cache
        .payload
        .read()
        .await
        .clone()
        .unwrap_or_default();
    let merged = merge_provider_usage_payload(cached, &fresh);
    *state.provider_usage_cache.payload.write().await = Some(merged);
    *state.provider_usage_cache.fetched_at.write().await = Some(Instant::now());
    if let Some(section) = fresh.sections.first() {
        save_shared_provider_section(&state, section, fresh.fetched_at);
    }
    (StatusCode::OK, Json(fresh)).into_response()
}

#[derive(Deserialize)]
struct ProviderUsageQuery {
    refresh: Option<bool>,
    provider: Option<String>,
}

#[cfg(test)]
mod mimo_auth_tests {
    use super::{
        grok_billing_snapshot, mimo_auth_error, mimo_headers, mimo_usage_percent,
        provider_reset_duration, provider_reset_text_local, should_skip_codex_cached_account,
        grok_web_billing_snapshot,
    };

    #[test]
    fn recognizes_expired_mimo_session_responses() {
        assert!(mimo_auth_error("HTTP 401 <html>Unauthorized</html>"));
        assert!(mimo_auth_error("HTTP 403 forbidden"));
        assert!(!mimo_auth_error("HTTP 500 upstream failure"));
    }

    #[test]
    fn reset_durations_switch_to_days_after_24_hours() {
        assert_eq!(provider_reset_duration(23.0 * 3600.0), "23小时");
        assert_eq!(provider_reset_duration(24.0 * 3600.0), "24小时");
        assert_eq!(provider_reset_duration(142.0 * 3600.0), "6天");
    }

    #[test]
    fn reset_text_local_accepts_millisecond_timestamps() {
        let reset_at_millis = (chrono::Utc::now().timestamp() + 2 * 24 * 3600) * 1000;
        assert_eq!(provider_reset_text_local(reset_at_millis), "2天");
    }

    #[test]
    fn forced_codex_refresh_does_not_reuse_quota_wall_cache() {
        let now = chrono::Utc::now().timestamp();
        let section = super::ProviderUsageSection {
            provider: "codex".into(),
            windows: vec![super::ProviderUsageWindow {
                window: "account 周额度".into(),
                used: Some("100%".into()),
                reset: Some("60分钟".into()),
                reset_at: Some(now + 3600),
            }],
            ..Default::default()
        };
        assert!(should_skip_codex_cached_account(&section, "account", now, false));
        assert!(!should_skip_codex_cached_account(&section, "account", now, true));
    }

    #[test]
    fn grok_web_frame_ratio_uses_percentage_points() {
        let body = b"\x00\x00\x00\x00\x15\x0a\x13\x0d\x00\x00\x80\x3f\x2a\x0c\x08\xff\xc8\xc7\xd4\x06\x10\x90\x99\x88\x9c\x01";
        let payload = grok_web_billing_snapshot(body).unwrap();
        let (_, used, _, _, _) = grok_billing_snapshot(&payload).unwrap();
        assert!((used - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn grok_web_frame_with_omitted_ratio_preserves_reset() {
        let body = b"\x00\x00\x00\x00\x48\x0a\x46\x12\x00\x1a\x00\x22\x0c\x08\xff\xc8\xc7\xd4\x06\x10\x90\x99\x88\x9c\x01\x2a\x0c\x08\xff\xbd\xec\xd4\x06\x10\x90\x99\x88\x9c\x01\x42\x1e\x08\x02\x12\x0c\x08\xff\xc8\xc7\xd4\x06\x10\x90\x99\x88\x9c\x01\x1a\x0c\x08\xff\xbd\xec\xd4\x06\x10\x90\x99\x88\x9c\x01\x58\x01\x62\x00\x68\x01\x80\x00\x00";
        let payload = grok_web_billing_snapshot(body).unwrap();
        let (window, used, reset, _, _) = grok_billing_snapshot(&payload).unwrap();
        assert_eq!(window, "周额度");
        assert_eq!(used, 0.0);
        assert_eq!(reset, "9/5 03:41");
    }

    #[test]
    fn grok_missing_usage_percent_is_not_reported_as_zero() {
        let payload = serde_json::json!({
            "config": {
                "currentPeriod": {
                    "type": "USAGE_PERIOD_TYPE_WEEKLY",
                    "end": "2026-09-04T19:41:51.327290+00:00"
                },
                "onDemandCap": {"val": 0},
                "onDemandUsed": {"val": 0}
            }
        });
        let error = grok_billing_snapshot(&payload).unwrap_err();
        assert!(error.contains("未提供周额度使用比例"));
    }

    #[test]
    fn grok_product_usage_percent_can_fill_missing_combined_percent() {
        let payload = serde_json::json!({
            "config": {
                "currentPeriod": {
                    "type": "USAGE_PERIOD_TYPE_WEEKLY",
                    "end": "2026-09-04T19:41:51.327290+00:00"
                },
                "productUsage": [
                    {"product": "GrokBuild", "usagePercent": 37.5},
                    {"product": "GrokChat", "usagePercent": 12.0}
                ]
            }
        });
        let (_, used_percent, _, _, _) = grok_billing_snapshot(&payload).unwrap();
        assert!((used_percent - 37.5).abs() < f64::EPSILON);
    }

    #[test]
    fn mimo_monthly_percent_uses_plan_token_usage_and_limit() {
        let percent = mimo_usage_percent(Some(8469.0), Some(10000.0)).unwrap();
        assert!((percent - 84.69).abs() < 0.000_001);
    }

    #[test]
    fn mimo_monthly_percent_preserves_a_real_overage_from_the_plan_fields() {
        let percent = mimo_usage_percent(Some(10_948.0), Some(10_000.0)).unwrap();
        assert!((percent - 109.48).abs() < 0.000_001);
    }
    #[test]
    fn builds_mimo_headers_without_extra_credentials() {
        let headers = mimo_headers(
            "api-platform_ph=redacted",
            "https://platform.xiaomimimo.com/",
        );
        assert!(headers
            .iter()
            .any(|(name, value)| name == "Cookie" && value == "api-platform_ph=redacted"));
        assert!(headers.iter().all(|(name, _)| name != "Authorization"));
    }
}

#[cfg(test)]
mod zed_pro_tests {
    use super::{json_string_or_number, zed_account_snapshot, zed_usage_limit};
    use serde_json::{json, Value};

    #[test]
    fn student_plan_does_not_invent_a_monthly_percent() {
        let payload = json!({
            "plan": {
                "plan": "zed_free",
                "plan_v2": "zed_free",
                "plan_v3": "zed_student",
                "subscription_period": {
                    "started_at": "2026-08-30T00:00:00Z",
                    "ended_at": "2026-09-30T00:00:00Z"
                },
                "usage": {
                    "model_requests": { "used": 0, "limit": { "limited": 0 } },
                    "edit_predictions": { "used": 0, "limit": "unlimited" }
                }
            }
        });
        let (description, windows) = zed_account_snapshot(&payload).unwrap();
        assert!(description.contains("Student"));
        assert!(description.contains("账期 08/30–09/30"));
        assert!(description.contains("模型请求 0"));
        assert!(description.contains("补全 0/无限"));
        assert!(windows.is_empty());
    }

    #[test]
    fn pro_plan_reports_monthly_percent_from_model_requests() {
        let payload = json!({
            "plan": {
                "plan_v3": "zed_pro",
                "subscription_period": {
                    "started_at": "2026-08-01T00:00:00Z",
                    "ended_at": "2026-09-01T00:00:00Z"
                },
                "usage": {
                    "model_requests": { "used": 50, "limit": { "limited": 500 } },
                    "edit_predictions": { "used": 12, "limit": { "limited": 100 } }
                }
            }
        });
        let (description, windows) = zed_account_snapshot(&payload).unwrap();
        assert!(description.contains("Pro"));
        assert!(description.contains("模型请求 50/500"));
        assert!(description.contains("补全 12/100"));
        assert_eq!(windows.len(), 1);
        assert_eq!(windows[0].window, "月额度");
        assert_eq!(windows[0].used.as_deref(), Some("10.0%"));
        assert!(windows[0].reset_at.is_some());
    }

    #[test]
    fn missing_plan_is_an_error() {
        let err = zed_account_snapshot(&json!({})).unwrap_err();
        assert!(err.contains("套餐"));
    }

    #[test]
    fn usage_limit_treats_unlimited_and_zero_as_non_percent() {
        assert_eq!(zed_usage_limit(Some(&json!("unlimited"))), None);
        assert_eq!(zed_usage_limit(Some(&json!({"unlimited": {}}))), None);
        assert_eq!(zed_usage_limit(Some(&json!({"limited": 0}))), Some(0.0));
        assert_eq!(zed_usage_limit(Some(&json!({"limited": 80}))), Some(80.0));
    }

    #[test]
    fn user_id_accepts_number_or_string() {
        assert_eq!(json_string_or_number(Some(&json!(42))), Some("42".into()));
        assert_eq!(json_string_or_number(Some(&json!("42"))), Some("42".into()));
        assert_eq!(json_string_or_number(Some(&Value::Null)), None);
    }
}
