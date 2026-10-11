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

#[derive(Serialize, Deserialize, Clone, Debug)]
struct ProviderUsageCredit {
    account: String,
    balance: f64,
}

#[derive(Serialize, Deserialize, Clone, Default)]
struct ProviderUsageSection {
    provider: String,
    title: String,
    description: String,
    #[serde(default)]
    captured_at: f64,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    credits: Vec<ProviderUsageCredit>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pools: Vec<SenseNovaPool>,
    rows: Vec<ProviderUsageRow>,
    windows: Vec<ProviderUsageWindow>,
    errors: Vec<String>,
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
    stepfun_auth: Arc<Mutex<Option<StepFunAuthState>>>,
    vyceai_refresh: Arc<Mutex<()>>,
    zed_daily_refresh: Arc<Mutex<()>>,
    sensenova_daily_refresh: Arc<Mutex<()>>,
    sensenova_auth: Arc<Mutex<Option<SenseNovaAuthState>>>,
}
