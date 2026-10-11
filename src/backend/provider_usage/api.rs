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
    if !force && !account_scoped {
        let now = chrono::Utc::now().timestamp();
        if let Some(mut cached) = cached_section.and_then(|section| provider_cached_section(section, now)) {
            cached.captured_at = unix_now_seconds();
            return cached;
        }
    }
    let mut section = match provider {
        "agentrouter" | "justwoker" | "hcnsec" => fetch_newapi_usage(state, provider).await,
        "vyceai" => fetch_vyceai_usage_from_url(state, "https://vyceai.com/user/dashboard").await,
        "openrouter" => fetch_openrouter_usage(state).await,
        "deepseek" => fetch_deepseek_usage(state).await,
        "atlascloud" => fetch_atlascloud_usage(state).await,
        "minimax" => fetch_minimax_usage(state).await,
        "stepfun" => fetch_stepfun_usage(state).await,
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

async fn provider_usage_handler(
    State(state): State<Arc<AppState>>,
    Query(query): Query<ProviderUsageQuery>,
) -> Response<Body> {
    let providers = provider_usage_catalog(&state.hermes_home);
    let Some(provider) = query.provider.as_deref().filter(|provider| !provider.is_empty()) else {
        let mut sections = shared_cache_sections(&state);
        if let Some(cached) = state.provider_usage_cache.payload.read().await.clone() {
            for section in cached.sections {
                if !vyceai_cached_section_is_current(&section, chrono::Utc::now().timestamp()) {
                    continue;
                }
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
