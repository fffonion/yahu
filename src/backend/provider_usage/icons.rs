/// Providers that run the New API gateway software (github.com/Calcium-Ion/new-api)
/// share its project logo. The bytes are embedded so no provider site is contacted.
const NEWAPI_VARIANT_PROVIDERS: &[&str] = &["agentrouter", "justwoker", "hcnsec"];
const NEWAPI_LOGO_BYTES: &[u8] = include_bytes!("../assets/newapi-logo.png");
/// WorkBuddy ships a crisp local SVG: the favicon service only offers a 40x40
/// JPEG with a white matte, which reads as a white box on the dark card.
const WORKBUDDY_ICON_BYTES: &[u8] = include_bytes!("../assets/workbuddy.svg");
const PROVIDER_ICON_CACHE_TTL: Duration = Duration::from_secs(24 * 60 * 60);
const MAX_PROVIDER_ICON_BODY: usize = 4 * 1024 * 1024;

#[derive(Clone)]
struct ProviderIconCacheEntry {
    body: Vec<u8>,
    content_type: String,
    fetched_at: Instant,
}

static PROVIDER_ICON_CACHE: OnceLock<Mutex<HashMap<String, ProviderIconCacheEntry>>> = OnceLock::new();

fn provider_icon_cache() -> &'static Mutex<HashMap<String, ProviderIconCacheEntry>> {
    PROVIDER_ICON_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Icons served from the binary so no provider site is contacted.
fn provider_icon_embedded_asset(provider: &str) -> Option<(&'static [u8], &'static str)> {
    if provider == "workbuddy" {
        return Some((WORKBUDDY_ICON_BYTES, "image/svg+xml"));
    }
    None
}

fn provider_icon_url(provider: &str) -> Option<String> {
    if NEWAPI_VARIANT_PROVIDERS.contains(&provider) || provider_icon_embedded_asset(provider).is_some() {
        return None;
    }
    if provider == "sensenova" {
        return Some("https://platform.sensenova.cn/favicon.ico".into());
    }
    let domains = [
        ("vyceai", "vyceai.com"),
        ("openrouter", "openrouter.ai"),
        ("deepseek", "deepseek.com"),
        ("atlascloud", "atlascloud.ai"),
        ("mimo", "mimo.xiaomi.com"),
        ("minimax", "minimax.io"),
        ("stepfun", "stepfun.com"),
        ("kimi", "kimi.com"),
        ("opencode", "opencode.ai"),
        ("opendesign", "open-design.ai"),
        ("commandcode", "commandcode.ai"),
        ("codex", "openai.com"),
        ("grok", "grok.com"),
        ("zed-pro", "zed.dev"),
    ];
    let domain = domains
        .iter()
        .find_map(|(name, domain)| (*name == provider).then_some(*domain))
        .unwrap_or(provider);
    if domain.is_empty()
        || !domain
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_'))
    {
        return None;
    }
    Some(format!(
        "https://www.google.com/s2/favicons?domain={domain}&sz=64"
    ))
}

fn provider_icon_embedded() -> Response<Body> {
    provider_icon_embedded_bytes(NEWAPI_LOGO_BYTES, "image/png")
}

fn provider_icon_embedded_bytes(body: &'static [u8], content_type: &'static str) -> Response<Body> {
    let mut response = Response::new(Body::from(body));
    *response.status_mut() = StatusCode::OK;
    let headers = response.headers_mut();
    headers.insert(header::CONTENT_TYPE, HeaderValue::from_static(content_type));
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("public, max-age=86400"),
    );
    headers.insert(
        axum::http::HeaderName::from_static("x-yahu-cache"),
        HeaderValue::from_static("EMBEDDED"),
    );
    response
}

fn provider_icon_response(entry: &ProviderIconCacheEntry, cache_status: &'static str) -> Response<Body> {
    let mut response = Response::new(Body::from(entry.body.clone()));
    *response.status_mut() = StatusCode::OK;
    let headers = response.headers_mut();
    if let Ok(value) = HeaderValue::from_str(&entry.content_type) {
        headers.insert(header::CONTENT_TYPE, value);
    }
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("public, max-age=86400"),
    );
    headers.insert(
        axum::http::HeaderName::from_static("x-yahu-cache"),
        HeaderValue::from_static(cache_status),
    );
    response
}

async fn provider_icon(State(state): State<Arc<AppState>>, AxumPath(provider): AxumPath<String>) -> Response<Body> {
    let provider = provider.to_ascii_lowercase();
    if NEWAPI_VARIANT_PROVIDERS.contains(&provider.as_str()) {
        return provider_icon_embedded();
    }
    if let Some((body, content_type)) = provider_icon_embedded_asset(&provider) {
        return provider_icon_embedded_bytes(body, content_type);
    }
    let Some(upstream_url) = provider_icon_url(&provider) else {
        return json_error(StatusCode::NOT_FOUND, "unknown provider icon");
    };
    let cache = provider_icon_cache();
    let mut cached = cache.lock().await;
    if let Some(entry) = cached.get(&provider)
        && entry.fetched_at.elapsed() < PROVIDER_ICON_CACHE_TTL
    {
        return provider_icon_response(entry, "HIT");
    }

    let response = match state
        .client
        .get(upstream_url)
        .timeout(Duration::from_secs(15))
        .header(header::ACCEPT, "image/png,image/*")
        .send()
        .await
    {
        Ok(response) => response,
        Err(err) => {
            return json_error(
                StatusCode::BAD_GATEWAY,
                &format!("provider icon proxy failed: {err}"),
            );
        }
    };
    if !response.status().is_success() {
        return json_error(
            StatusCode::BAD_GATEWAY,
            &format!("provider icon proxy returned {}", response.status()),
        );
    }
    let content_type = response
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .filter(|value| value.starts_with("image/"))
        .unwrap_or("image/png")
        .to_string();
    let body = match response.bytes().await {
        Ok(body) if body.len() <= MAX_PROVIDER_ICON_BODY => body.to_vec(),
        Ok(_) => {
            return json_error(StatusCode::BAD_GATEWAY, "provider icon is too large");
        }
        Err(err) => {
            return json_error(
                StatusCode::BAD_GATEWAY,
                &format!("provider icon response failed: {err}"),
            );
        }
    };
    cached.insert(
        provider.clone(),
        ProviderIconCacheEntry {
            body,
            content_type,
            fetched_at: Instant::now(),
        },
    );
    provider_icon_response(cached.get(&provider).expect("icon cache populated"), "MISS")
}

async fn agentrouter_logo(State(state): State<Arc<AppState>>) -> Response<Body> {
    provider_icon(State(state), AxumPath("agentrouter".to_string())).await
}
