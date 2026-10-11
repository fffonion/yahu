const PROVIDER_USAGE_TTL: Duration = Duration::from_secs(30 * 60);
const NEWAPI_STATUS_CACHE_TTL_SECONDS: f64 = 24.0 * 60.0 * 60.0;
#[derive(Serialize, Deserialize, Clone, Default)]
struct SharedProviderCacheEntry {
    cached_at: f64,
    section: ProviderUsageSection,
}

#[derive(Serialize, Deserialize, Clone, Default)]
struct NewApiQuotaCacheEntry {
    quota_per_unit: f64,
    cached_at: f64,
}

#[derive(Serialize, Deserialize, Clone, Default)]
struct SharedProviderCacheFile {
    entries: HashMap<String, SharedProviderCacheEntry>,
    #[serde(default)]
    commandcode_accounts: HashMap<String, CommandCodeAccountCache>,
    #[serde(default)]
    newapi_quota_per_unit: HashMap<String, NewApiQuotaCacheEntry>,
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

fn provider_quota_wall_percent(section: &ProviderUsageSection) -> f64 {
    if section.provider == "commandcode" {
        COMMANDCODE_QUOTA_WALL_PERCENT
    } else {
        100.0
    }
}

fn provider_section_should_skip_upstream(section: &ProviderUsageSection, now: i64) -> bool {
    let quota_wall_percent = provider_quota_wall_percent(section);
    section.windows.iter().any(|window| {
        provider_used_percent(window).is_some_and(|used| used >= quota_wall_percent)
            && window.reset_at.is_some_and(|reset_at| reset_at > now)
    })
}

fn provider_account_should_skip_upstream(
    section: &ProviderUsageSection,
    label: &str,
    account_scoped: bool,
    now: i64,
) -> bool {
    let quota_wall_percent = provider_quota_wall_percent(section);
    section
        .windows
        .iter()
        .filter(|window| !account_scoped || provider_account_window_matches(window, label))
        .any(|window| {
            provider_used_percent(window).is_some_and(|used| used >= quota_wall_percent)
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
        cached.credits.retain(|credit| credit.account == label);
    }
    refresh_provider_cached_reset_times(&mut cached, now);
    Some(cached)
}

fn shared_provider_cache_path(state: &AppState) -> std::path::PathBuf {
    state.hermes_home.join("state/token_usage_cache.json")
}

fn cached_newapi_quota_per_unit(
    state: &AppState,
    provider: &str,
    now: f64,
) -> Option<f64> {
    let cache = read_shared_provider_cache(state);
    let entry = cache.newapi_quota_per_unit.get(provider)?;
    (entry.quota_per_unit > 0.0
        && entry.cached_at > 0.0
        && now >= entry.cached_at
        && now - entry.cached_at < NEWAPI_STATUS_CACHE_TTL_SECONDS)
        .then_some(entry.quota_per_unit)
}

fn save_newapi_quota_per_unit(
    state: &AppState,
    provider: &str,
    quota_per_unit: f64,
    cached_at: f64,
) {
    if quota_per_unit <= 0.0 || cached_at <= 0.0 {
        return;
    }
    let mut cache = read_shared_provider_cache(state);
    cache.newapi_quota_per_unit.insert(
        provider.to_string(),
        NewApiQuotaCacheEntry {
            quota_per_unit,
            cached_at,
        },
    );
    write_shared_provider_cache(state, &cache);
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
    if !vyceai_cached_section_is_current(&entry.section, chrono::Utc::now().timestamp()) {
        return None;
    }
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
            if !vyceai_cached_section_is_current(&entry.section, chrono::Utc::now().timestamp()) {
                return None;
            }
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
    let mut payload = state.provider_usage_cache.payload.read().await.clone()?;
    payload.sections.retain(|section| vyceai_cached_section_is_current(section, chrono::Utc::now().timestamp()));
    payload
        .sections
        .iter()
        .any(|section| section.provider == provider
            && vyceai_cached_section_is_current(section, chrono::Utc::now().timestamp()))
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
